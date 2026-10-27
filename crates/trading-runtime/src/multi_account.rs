//! Non-atomic fan-out of a single strategy intent to independent account runtimes.
//!
//! Each target owns its own OMS, risk evaluator, account state, venue binding, and
//! event publisher. A failure for one account is recorded in that account's result
//! and does not prevent later accounts from being evaluated. This coordinator is
//! deliberately sequential: it does not promise simultaneous submissions or fills.

use domain::{AccountId, OrderStatus, TradeIntent, TradeIntentId};
use events::EventPublisher;
use venue_connectors::VenueAdapter;

use crate::{
    InstrumentSizingSnapshotProvider, IntentOutcome, RuntimeError, RuntimeInstrumentSizingContext,
    RuntimeResult, TradingRuntime,
};

/// One account's independent execution route for multi-account intent fan-out.
pub struct AccountExecutionTarget<'a> {
    pub runtime: &'a mut TradingRuntime,
    pub venue: &'a dyn VenueAdapter,
    pub events_out: &'a EventPublisher,
    /// If set, the account must obtain instrument-aware sizing from this provider.
    /// Provider failure is recorded for this account and never falls back to legacy sizing.
    pub sizing_provider: Option<&'a dyn InstrumentSizingSnapshotProvider>,
}

/// Outcome for one account. Errors are retained per account so partial success is explicit.
#[derive(Debug)]
pub struct AccountExecutionResult {
    pub account_id: AccountId,
    pub result: RuntimeResult<IntentOutcome>,
}

/// Aggregate fan-out outcome. It is a report, not an atomic transaction.
#[derive(Debug)]
pub struct MultiAccountOutcome {
    pub intent_id: TradeIntentId,
    pub results: Vec<AccountExecutionResult>,
}

impl MultiAccountOutcome {
    pub fn succeeded(&self) -> usize {
        self.results
            .iter()
            .filter(|result| {
                matches!(
                    &result.result,
                    Ok(outcome)
                        if outcome.decision.is_executable()
                            && matches!(
                                outcome.order_status.as_ref(),
                                Some(OrderStatus::Accepted
                                    | OrderStatus::PartiallyFilled
                                    | OrderStatus::Filled)
                            )
                )
            })
            .count()
    }

    pub fn failed_or_blocked(&self) -> usize {
        self.results.len().saturating_sub(self.succeeded())
    }

    pub fn is_partial(&self) -> bool {
        self.succeeded() > 0 && self.failed_or_blocked() > 0
    }
}

/// Execute one intent independently against a configured set of account runtimes.
///
/// Preflight validation happens for every target before the first execution:
/// - the list must not be empty;
/// - account IDs must be unique (one intent may only be fanned out once per account);
/// - each venue's declared name must match the runtime's configured venue ID.
///
/// After preflight, each account is attempted in input order. Runtime, event, persistence,
/// risk, snapshot-provider, and venue failures are isolated in that account's result.
/// No cross-account rollback is attempted or implied.
pub async fn execute_intent_across_accounts(
    intent: &TradeIntent,
    targets: &mut [AccountExecutionTarget<'_>],
) -> RuntimeResult<MultiAccountOutcome> {
    if targets.is_empty() {
        return Err(RuntimeError::Invariant(
            "multi-account execution requires at least one account target".into(),
        ));
    }

    let mut seen = std::collections::HashSet::with_capacity(targets.len());
    for target in targets.iter() {
        let account_id = target.runtime.config().account_id.clone();
        if !seen.insert(account_id.clone()) {
            return Err(RuntimeError::Invariant(format!(
                "duplicate account target in multi-account fan-out: {account_id}"
            )));
        }
        let configured_venue = target.runtime.config().venue_id.as_str();
        if target.venue.venue_name() != configured_venue {
            return Err(RuntimeError::Invariant(format!(
                "venue binding mismatch for account {account_id}: configured {configured_venue}, adapter reports {}",
                target.venue.venue_name()
            )));
        }
    }

    let mut results = Vec::with_capacity(targets.len());
    for target in targets.iter_mut() {
        let account_id = target.runtime.config().account_id.clone();
        let result = match target.sizing_provider {
            Some(provider) => {
                let snapshot: RuntimeResult<RuntimeInstrumentSizingContext> = provider
                    .load_snapshot(&account_id, &intent.instrument_id)
                    .await;
                match snapshot {
                    Ok(snapshot) => target.runtime
                        .execute_intent_with_instrument_sizing(
                            intent,
                            target.venue,
                            target.events_out,
                            &snapshot,
                        )
                        .await,
                    Err(error) => Err(error),
                }
            }
            None => Err(RuntimeError::Invariant(
                "multi-account execution requires an instrument-aware snapshot provider; legacy sizing is disabled".into(),
            )),
        };
        results.push(AccountExecutionResult { account_id, result });
    }

    Ok(MultiAccountOutcome {
        intent_id: intent.id.clone(),
        results,
    })
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;
    use domain::{AccountId, RiskPolicy, StrategyVersion, VenueId};
    use rust_decimal::Decimal;
    use crate::RuntimeConfig;
    use venue_connectors::SimulatedVenue;

    fn runtime(account: &str, venue: &str) -> TradingRuntime {
        let account_id = AccountId::new(account).unwrap();
        let venue_id = VenueId::new(venue).unwrap();
        let policy = RiskPolicy::try_new(
            Decimal::ONE,
            Decimal::from(10),
            Decimal::from(100),
            Decimal::from(10),
            Decimal::from(5),
            Decimal::from(10),
            10,
        )
        .unwrap();
        TradingRuntime::new(RuntimeConfig::new(account_id, venue_id, policy))
    }

    #[tokio::test]
    async fn rejects_empty_target_list_before_any_execution() {
        let intent = test_intent();
        let result = execute_intent_across_accounts(&intent, &mut []).await;
        assert!(matches!(result, Err(RuntimeError::Invariant(_))));
    }

    #[tokio::test]
    async fn rejects_duplicate_account_targets_before_any_execution() {
        let mut first = runtime("same-account", "simulated");
        let mut second = runtime("same-account", "simulated");
        let venue_one = SimulatedVenue::new(VenueId::new("simulated").unwrap());
        let venue_two = SimulatedVenue::new(VenueId::new("simulated").unwrap());
        let (events_one, _subscriber_one) = events::event_channel(8);
        let (events_two, _subscriber_two) = events::event_channel(8);
        let mut targets = [
            AccountExecutionTarget { runtime: &mut first, venue: &venue_one, events_out: &events_one, sizing_provider: None },
            AccountExecutionTarget { runtime: &mut second, venue: &venue_two, events_out: &events_two, sizing_provider: None },
        ];
        let result = execute_intent_across_accounts(&test_intent(), &mut targets).await;
        assert!(matches!(result, Err(RuntimeError::Invariant(message)) if message.contains("duplicate account")));
        drop(targets);
        assert!(first.oms().orders().is_empty());
        assert!(second.oms().orders().is_empty());
    }

    #[tokio::test]
    async fn rejects_mismatched_venue_binding_before_any_execution() {
        let mut account = runtime("account-one", "expected-venue");
        let venue = SimulatedVenue::new(VenueId::new("different-venue").unwrap());
        let (events, _subscriber) = events::event_channel(8);
        let mut targets = [AccountExecutionTarget {
            runtime: &mut account,
            venue: &venue,
            events_out: &events,
            sizing_provider: None,
        }];
        let result = execute_intent_across_accounts(&test_intent(), &mut targets).await;
        assert!(matches!(result, Err(RuntimeError::Invariant(message)) if message.contains("venue binding mismatch")));
        drop(targets);
        assert!(account.oms().orders().is_empty());
    }


    #[tokio::test]
    async fn one_account_error_does_not_prevent_later_account_attempt() {
        let mut first = runtime("account-failing-events", "simulated");
        let mut second = runtime("account-later-target", "simulated");
        let venue_one = SimulatedVenue::new(VenueId::new("simulated").unwrap());
        let venue_two = SimulatedVenue::new(VenueId::new("simulated").unwrap());
        let (events_one, subscriber_one) = events::event_channel(8);
        drop(subscriber_one);
        let (events_two, _subscriber_two) = events::event_channel(8);
        let mut targets = [
            AccountExecutionTarget {
                runtime: &mut first,
                venue: &venue_one,
                events_out: &events_one,
                sizing_provider: None,
            },
            AccountExecutionTarget {
                runtime: &mut second,
                venue: &venue_two,
                events_out: &events_two,
                sizing_provider: None,
            },
        ];

        let outcome = execute_intent_across_accounts(&test_intent(), &mut targets)
            .await
            .unwrap();

        assert_eq!(outcome.results.len(), 2);
        assert!(outcome.results[0].result.is_err());
        assert!(outcome.results[1].result.is_err());
        assert_eq!(outcome.results[1].account_id.as_str(), "account-later-target");
        assert!(matches!(
            &outcome.results[1].result,
            Err(RuntimeError::Invariant(message)) if message.contains("snapshot provider")
        ));
        drop(targets);
        assert!(first.oms().orders().is_empty());
        assert!(second.oms().orders().is_empty());
    }

    fn test_intent() -> TradeIntent {
        use domain::{DeploymentId, InstrumentId, OrderSide, StrategyId, TradeIntentId};
        TradeIntent::builder(
            TradeIntentId::new("fanout-intent").unwrap(),
            StrategyId::new("fanout-strategy").unwrap(),
            StrategyVersion::new("v1").unwrap(),
            DeploymentId::new("fanout-deployment").unwrap(),
            InstrumentId::new("BTCUSDT").unwrap(),
            OrderSide::Buy,
            chrono::Utc::now(),
        )
        .entry_price(Decimal::from(100))
        .unwrap()
        .stop_loss(Decimal::from(95))
        .unwrap()
        .take_profit(Decimal::from(110))
        .unwrap()
        .build()
        .unwrap()
    }
}

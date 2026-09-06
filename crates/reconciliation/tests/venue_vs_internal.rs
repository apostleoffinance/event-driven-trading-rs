//! Phase 10: reconcile internal vs SimulatedVenue — alert only, never overwrite.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use account_engine::{AccountEngine, InMemoryAccountEngine};
use chrono::Utc;
use domain::{
    AccountId, ClientOrderId, DeploymentId, InstrumentId, OrderSide, OrderType, RiskPolicy,
    StrategyId, StrategyVersion, TimeInForce, TradeIntent, TradeIntentId, VenueId,
};
use events::{event_channel, TradingEvent};
use execution_engine::{ExecutionEngine, NewOrderRequest};
use reconciliation::{InternalSnapshot, Reconciler, ReconciliationBreak};
use risk_engine::{DefaultRiskEvaluator, RiskEvaluator};
use rust_decimal::Decimal;
use venue_connectors::{submit_approved_order, SimulatedVenue, VenueAdapter};

async fn seeded_account(
    capital: Decimal,
) -> (InMemoryAccountEngine, SimulatedVenue, AccountId, VenueId) {
    let venue_id = VenueId::new("simulated").unwrap();
    let venue = SimulatedVenue::new(venue_id.clone());
    let account_id = AccountId::new("recon-account-001").unwrap();
    let mut accounts = InMemoryAccountEngine::new();
    accounts
        .open_prop_on_simulated(account_id.clone(), capital, Utc::now())
        .unwrap();
    let state = accounts.get(&account_id).unwrap().state.clone();
    venue.register_account(state).await.unwrap();
    (accounts, venue, account_id, venue_id)
}

#[tokio::test]
async fn clean_match_emits_started_and_completed() {
    let (accounts, venue, account_id, venue_id) = seeded_account(Decimal::from(100_000)).await;
    let state = accounts.get(&account_id).unwrap().state.clone();
    let internal = InternalSnapshot::new(account_id.clone(), venue_id.clone(), state);

    let (pub_, mut sub) = event_channel(32);
    let report = Reconciler::new()
        .reconcile(&venue, &internal, Some(&pub_))
        .await
        .unwrap();

    assert!(report.is_clean());
    drop(pub_);

    let mut names = Vec::new();
    while let Some(env) = sub.recv().await {
        names.push(env.event.name().to_string());
    }
    assert_eq!(
        names,
        vec![
            "ReconciliationStarted".to_string(),
            "ReconciliationCompleted".to_string()
        ]
    );
}

#[tokio::test]
async fn equity_mismatch_alerts_without_overwriting_account() {
    let (mut accounts, venue, account_id, venue_id) = seeded_account(Decimal::from(100_000)).await;

    let intent = TradeIntent::builder(
        TradeIntentId::new("ti-recon").unwrap(),
        StrategyId::new("btc-mean-reversion").unwrap(),
        StrategyVersion::new("v1").unwrap(),
        DeploymentId::new("deployment-recon").unwrap(),
        InstrumentId::new("BTCUSDT").unwrap(),
        OrderSide::Buy,
        Utc::now(),
    )
    .entry_price(Decimal::from(100))
    .unwrap()
    .target_quantity(Decimal::from(2))
    .unwrap()
    .build()
    .unwrap();

    let policy = RiskPolicy::try_new(
        Decimal::from(2),
        Decimal::from(10),
        Decimal::from(50),
        Decimal::from(2),
        Decimal::from(5),
        Decimal::from(10),
        5,
    )
    .unwrap();

    let account_state = accounts.get(&account_id).unwrap().state.clone();
    let decision = DefaultRiskEvaluator::new()
        .evaluate(&domain::RiskRequest::from_intent(
            &intent,
            account_id.clone(),
            account_state,
            policy,
            Utc::now(),
        ))
        .unwrap();

    let mut oms = ExecutionEngine::new();
    submit_approved_order(
        &mut oms,
        &venue,
        NewOrderRequest {
            client_order_id: ClientOrderId::new("clid-recon").unwrap(),
            account_id: account_id.clone(),
            venue_id: venue_id.clone(),
            deployment_id: intent.deployment_id.clone(),
            strategy_id: intent.strategy_id.clone(),
            trade_intent_id: intent.id.clone(),
            instrument_id: intent.instrument_id.clone(),
            side: intent.side,
            order_type: OrderType::Market,
            time_in_force: TimeInForce::Ioc,
            price: intent.entry_price,
            risk_decision: decision,
            created_at: Utc::now(),
        },
    )
    .await
    .unwrap();

    let venue_positions = venue.positions(&account_id).await.unwrap();
    accounts
        .set_open_position_count(&account_id, venue_positions.len() as u32, Utc::now())
        .unwrap();

    let equity_before = accounts.get(&account_id).unwrap().state.equity;
    let balance_before = accounts.get(&account_id).unwrap().state.balance;

    // Internal positions match venue qty, but equity/balance were never fee-synced.
    let internal = InternalSnapshot::new(
        account_id.clone(),
        venue_id.clone(),
        accounts.get(&account_id).unwrap().state.clone(),
    )
    .with_positions(venue_positions);

    let (pub_, mut sub) = event_channel(64);
    let report = Reconciler::new()
        .reconcile(&venue, &internal, Some(&pub_))
        .await
        .unwrap();

    assert!(!report.is_clean());
    assert!(report.breaks.iter().any(|b| matches!(
        b,
        ReconciliationBreak::AccountEquityMismatch { .. }
            | ReconciliationBreak::AccountBalanceMismatch { .. }
    )));

    // Critical invariant: no silent overwrite.
    let after = accounts.get(&account_id).unwrap().state.clone();
    assert_eq!(after.equity, equity_before);
    assert_eq!(after.balance, balance_before);

    drop(pub_);
    let mut saw_alert = false;
    let mut saw_completed = false;
    while let Some(env) = sub.recv().await {
        match env.event {
            TradingEvent::ReconciliationAlert { .. } => saw_alert = true,
            TradingEvent::ReconciliationCompleted { .. } => saw_completed = true,
            _ => {}
        }
    }
    assert!(saw_alert);
    assert!(saw_completed);
}

#[tokio::test]
async fn venue_fetch_failure_emits_failed() {
    let venue_id = VenueId::new("simulated").unwrap();
    let venue = SimulatedVenue::new(venue_id.clone());
    let account_id = AccountId::new("missing-account").unwrap();

    // Internal thinks the account exists; venue was never registered.
    let mut accounts = InMemoryAccountEngine::new();
    accounts
        .open_prop_on_simulated(account_id.clone(), Decimal::from(10_000), Utc::now())
        .unwrap();
    let internal = InternalSnapshot::new(
        account_id.clone(),
        venue_id,
        accounts.get(&account_id).unwrap().state.clone(),
    );

    let (pub_, mut sub) = event_channel(16);
    let err = Reconciler::new()
        .reconcile(&venue, &internal, Some(&pub_))
        .await
        .unwrap_err();
    assert!(err.to_string().contains("not found") || err.to_string().contains("venue"));

    drop(pub_);
    let mut saw_failed = false;
    while let Some(env) = sub.recv().await {
        if matches!(env.event, TradingEvent::ReconciliationFailed { .. }) {
            saw_failed = true;
        }
        assert!(!matches!(
            env.event,
            TradingEvent::ReconciliationCompleted { .. }
        ));
    }
    assert!(saw_failed);
}

#[test]
fn position_quantity_break_is_detected() {
    use domain::{Position, PositionId, PositionSide};
    use reconciliation::compare_snapshots;

    let account_id = AccountId::new("a1").unwrap();
    let instrument = InstrumentId::new("BTCUSDT").unwrap();
    let now = Utc::now();

    let mut internal_state =
        domain::AccountState::new(account_id.clone(), Decimal::from(1000), now).unwrap();
    internal_state.open_position_count = 1;
    let external_state = internal_state.clone();

    let internal_pos = Position::open(
        PositionId::new("p-int").unwrap(),
        account_id.clone(),
        instrument.clone(),
        PositionSide::Long,
        Decimal::from(2),
        Decimal::from(100),
        None,
        now,
    )
    .unwrap();
    let mut external_pos = internal_pos.clone();
    external_pos.quantity = Decimal::from(3);

    let breaks = compare_snapshots(
        &internal_state,
        &[internal_pos],
        &[],
        &external_state,
        &[external_pos],
        &[],
        &reconciliation::CompareTolerances::default(),
    );

    assert!(breaks
        .iter()
        .any(|b| matches!(b, ReconciliationBreak::PositionQuantityMismatch { .. })));
}

//! Single-intent pipeline: TradeIntent → Risk → OMS → Venue → Position.

use account_engine::AccountEngine;
use chrono::{DateTime, Utc};
use async_trait::async_trait;
use domain::{AccountId, InstrumentId, OrderStatus, RiskDecision, RiskRequest, TradeIntent};
use events::{EventPublisher, TradingEvent};
use execution_engine::NewOrderRequest;
use risk_engine::{InstrumentSizingContext, RiskEvaluator};
use venue_connectors::{submit_approved_order, VenueAdapter};

use crate::error::RuntimeResult;
use crate::ids::client_order_id_for_intent;
use crate::TradingRuntime;

/// Outcome of processing one trade intent through risk + execution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IntentOutcome {
    pub intent: TradeIntent,
    pub decision: RiskDecision,
    pub order_status: Option<OrderStatus>,
}

/// Instrument-aware sizing inputs explicitly bound to the account and instrument
/// they describe. The caller remains responsible for sourcing trusted, fresh data.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeInstrumentSizingContext {
    pub account_id: AccountId,
    pub instrument_id: InstrumentId,
    /// Identifier of the trusted snapshot source (for audit and operations).
    pub source_id: String,
    /// Capture time of the oldest required input in this bundle.
    pub captured_at: DateTime<Utc>,
    /// Explicit configured freshness bound; unknown/zero bounds are invalid.
    pub max_age_seconds: i64,
    pub sizing: InstrumentSizingContext,
}

impl RuntimeInstrumentSizingContext {
    fn is_fresh_at(&self, now: DateTime<Utc>) -> bool {
        if self.source_id.trim().is_empty() || self.max_age_seconds <= 0 || self.captured_at > now {
            return false;
        }
        now.signed_duration_since(self.captured_at).num_seconds() <= self.max_age_seconds
    }
}

/// Runtime boundary for loading an account/instrument-scoped sizing snapshot.
/// Implementations must obtain data from authoritative configured sources and
/// return an error when any required input is missing or inconsistent.
#[async_trait]
pub trait InstrumentSizingSnapshotProvider: Send + Sync {
    async fn load_snapshot(
        &self,
        account_id: &AccountId,
        instrument_id: &InstrumentId,
    ) -> RuntimeResult<RuntimeInstrumentSizingContext>;
}

impl TradingRuntime {
    /// Evaluate risk and, when executable, submit through OMS + venue.
    ///
    /// Fail-closed: if account state cannot be loaded, returns
    /// [`RiskDecision::Halted`] and does not submit.
    pub async fn execute_intent(
        &mut self,
        intent: &TradeIntent,
        venue: &dyn VenueAdapter,
        events_out: &EventPublisher,
    ) -> RuntimeResult<IntentOutcome> {
        self.execute_intent_inner(intent, venue, events_out, None)
            .await
    }

    /// Load sizing inputs from the configured provider and execute only with the
    /// resulting account/instrument-bound snapshot. Provider failures propagate
    /// before any OMS order is created; this method never falls back to legacy sizing.
    pub async fn execute_intent_with_snapshot_provider(
        &mut self,
        intent: &TradeIntent,
        venue: &dyn VenueAdapter,
        events_out: &EventPublisher,
        provider: &dyn InstrumentSizingSnapshotProvider,
    ) -> RuntimeResult<IntentOutcome> {
        let snapshot = provider
            .load_snapshot(&self.config.account_id, &intent.instrument_id)
            .await?;
        self.execute_intent_with_instrument_sizing(intent, venue, events_out, &snapshot)
            .await
    }

    /// Evaluate and execute an intent using the explicit instrument-aware sizing
    /// contract. The caller must supply a context from its trusted snapshot layer;
    /// stale or invalid inputs reject before OMS/venue submission. This method
    /// never falls back to legacy price-distance sizing.
    pub async fn execute_intent_with_instrument_sizing(
        &mut self,
        intent: &TradeIntent,
        venue: &dyn VenueAdapter,
        events_out: &EventPublisher,
        sizing_context: &RuntimeInstrumentSizingContext,
    ) -> RuntimeResult<IntentOutcome> {
        self.execute_intent_inner(intent, venue, events_out, Some(sizing_context))
            .await
    }

    async fn execute_intent_inner(
        &mut self,
        intent: &TradeIntent,
        venue: &dyn VenueAdapter,
        events_out: &EventPublisher,
        sizing_context: Option<&RuntimeInstrumentSizingContext>,
    ) -> RuntimeResult<IntentOutcome> {
        if let Some(store) = &self.store {
            store.save_trade_intent(intent).await?;
        }

        events_out
            .publish(TradingEvent::RiskCheckRequested {
                trade_intent_id: intent.id.clone(),
                account_id: self.config.account_id.clone(),
                deployment_id: intent.deployment_id.clone(),
            })
            .await?;

        // Fail closed: missing account state → Halted (no new risk).
        let account_state = match self.accounts.get(&self.config.account_id) {
            Ok(record) => record.state.clone(),
            Err(err) => {
                let decision = RiskDecision::Halted {
                    reason: format!("account state unavailable (fail closed): {err}"),
                };
                self.publish_risk_decision(intent, &decision, events_out)
                    .await?;
                if let Some(store) = &self.store {
                    store
                        .save_risk_decision(&intent.id, &self.config.account_id, &decision)
                        .await?;
                }
                return Ok(IntentOutcome {
                    intent: intent.clone(),
                    decision,
                    order_status: None,
                });
            }
        };

        let risk_req = RiskRequest::from_intent(
            intent,
            self.config.account_id.clone(),
            account_state,
            self.config.risk_policy.clone(),
            Utc::now(),
        )
        .with_exposure(self.config.current_exposure)
        .with_session_allowed(self.config.session_allowed);

        let decision = match sizing_context {
            Some(snapshot)
                if snapshot.account_id != self.config.account_id
                    || snapshot.instrument_id != intent.instrument_id
                    || !snapshot.is_fresh_at(Utc::now()) =>
            {
                RiskDecision::Rejected {
                    reason: domain::RiskRejectReason::DependencyUnavailable,
                }
            }
            Some(snapshot) => self
                .risk
                .evaluate_with_instrument_sizing(&risk_req, &snapshot.sizing)?,
            None => self.risk.evaluate(&risk_req)?,
        };
        self.publish_risk_decision(intent, &decision, events_out)
            .await?;

        if let Some(store) = &self.store {
            store
                .save_risk_decision(&intent.id, &self.config.account_id, &decision)
                .await?;
        }

        if !decision.is_executable() {
            return Ok(IntentOutcome {
                intent: intent.clone(),
                decision,
                order_status: None,
            });
        }

        let client_order_id = client_order_id_for_intent(&intent.id)?;
        let outcome = submit_approved_order(
            &mut self.oms,
            venue,
            NewOrderRequest {
                client_order_id: client_order_id.clone(),
                account_id: self.config.account_id.clone(),
                venue_id: self.config.venue_id.clone(),
                deployment_id: intent.deployment_id.clone(),
                strategy_id: intent.strategy_id.clone(),
                trade_intent_id: intent.id.clone(),
                instrument_id: intent.instrument_id.clone(),
                side: intent.side,
                order_type: self.config.order_type,
                time_in_force: self.config.time_in_force,
                price: intent.entry_price,
                risk_decision: decision.clone(),
                instrument_spec: self.config.instrument_spec.clone(),
                created_at: Utc::now(),
            },
        )
        .await?;

        events_out
            .publish(TradingEvent::OrderCreated {
                order: outcome.order.clone(),
            })
            .await?;
        events_out
            .publish(TradingEvent::OrderSubmitted {
                order: outcome.order.clone(),
                venue_id: self.config.venue_id.clone(),
                client_order_id,
            })
            .await?;

        match outcome.order.status {
            OrderStatus::Rejected => {
                events_out
                    .publish(TradingEvent::OrderRejected {
                        order: outcome.order.clone(),
                        reason: "order rejected".into(),
                    })
                    .await?;
            }
            OrderStatus::Failed => {
                events_out
                    .publish(TradingEvent::OrderFailed {
                        order: outcome.order.clone(),
                        reason: "order failed".into(),
                    })
                    .await?;
            }
            OrderStatus::Unknown => {
                events_out
                    .publish(TradingEvent::OrderUnknown {
                        order: outcome.order.clone(),
                        reason: "ambiguous venue outcome; reconcile before assuming failure".into(),
                    })
                    .await?;
                if let Some(store) = &self.store {
                    store.save_order(&outcome.order).await?;
                    for audit in self.oms.audit_for_order(&outcome.order.id) {
                        store
                            .save_audit_event(
                                audit,
                                Some(&self.config.account_id),
                                Some(&intent.id),
                            )
                            .await?;
                    }
                }
                return Ok(IntentOutcome {
                    intent: intent.clone(),
                    decision,
                    order_status: Some(OrderStatus::Unknown),
                });
            }
            OrderStatus::Accepted | OrderStatus::PartiallyFilled | OrderStatus::Filled => {
                events_out
                    .publish(TradingEvent::OrderAccepted {
                        order: outcome.order.clone(),
                        venue_order_id: None,
                    })
                    .await?;
            }
            _ => {}
        }

        let fills: Vec<_> = self
            .oms
            .fills_for(&outcome.order.id)
            .into_iter()
            .cloned()
            .collect();
        let fill_count = fills.len();

        for (idx, fill) in fills.iter().enumerate() {
            let is_last = idx + 1 == fill_count;
            let event = if is_last && outcome.order.status == OrderStatus::Filled {
                TradingEvent::OrderFilled {
                    order: outcome.order.clone(),
                    fill: fill.clone(),
                }
            } else {
                TradingEvent::OrderPartiallyFilled {
                    order: outcome.order.clone(),
                    fill: fill.clone(),
                }
            };
            events_out.publish(event).await?;
        }

        if let Some(store) = &self.store {
            store.save_order(&outcome.order).await?;
            for fill in &fills {
                store.save_fill(fill).await?;
            }
            for audit in self.oms.audit_for_order(&outcome.order.id) {
                store
                    .save_audit_event(audit, Some(&self.config.account_id), Some(&intent.id))
                    .await?;
            }
        }

        let positions = venue.positions(&self.config.account_id).await?;
        for position in &positions {
            if let Some(store) = &self.store {
                store.upsert_position(position).await?;
            }
            events_out
                .publish(TradingEvent::PositionOpened {
                    position: position.clone(),
                })
                .await?;
        }

        self.accounts.set_open_position_count(
            &self.config.account_id,
            positions.len() as u32,
            Utc::now(),
        )?;

        let record = self.accounts.get(&self.config.account_id)?;
        let updated = record.state.clone();
        let account = record.account.clone();
        let starting_capital = record.config.starting_capital;
        let label = record.config.label.clone();

        if let Some(store) = &self.store {
            store
                .upsert_account(&account, starting_capital, label.as_deref(), &updated)
                .await?;
        }

        events_out
            .publish(TradingEvent::AccountUpdated {
                account_id: self.config.account_id.clone(),
                state: updated,
            })
            .await?;

        Ok(IntentOutcome {
            intent: intent.clone(),
            decision,
            order_status: Some(outcome.order.status),
        })
    }

    async fn publish_risk_decision(
        &self,
        intent: &TradeIntent,
        decision: &RiskDecision,
        events_out: &EventPublisher,
    ) -> RuntimeResult<()> {
        let account_id = self.config.account_id.clone();
        let trade_intent_id = intent.id.clone();
        match decision {
            RiskDecision::Approved { quantity } => {
                events_out
                    .publish(TradingEvent::RiskApproved {
                        trade_intent_id,
                        account_id,
                        quantity: *quantity,
                        decision: decision.clone(),
                    })
                    .await?;
            }
            RiskDecision::Resized { quantity, reason } => {
                events_out
                    .publish(TradingEvent::RiskResized {
                        trade_intent_id,
                        account_id,
                        quantity: *quantity,
                        reason: reason.clone(),
                        decision: decision.clone(),
                    })
                    .await?;
            }
            RiskDecision::Rejected { reason } => {
                events_out
                    .publish(TradingEvent::RiskRejected {
                        trade_intent_id,
                        account_id,
                        reason: reason.clone(),
                        decision: decision.clone(),
                    })
                    .await?;
            }
            RiskDecision::Halted { reason } => {
                events_out
                    .publish(TradingEvent::TradingHalted {
                        account_id: Some(account_id),
                        strategy_id: Some(intent.strategy_id.clone()),
                        reason: reason.clone(),
                    })
                    .await?;
            }
        }
        Ok(())
    }
}


#[cfg(test)]
mod sizing_snapshot_tests {
    #![allow(clippy::unwrap_used)]

    use super::*;
    use chrono::Duration;
    use rust_decimal::Decimal;
    use risk_engine::LossModel;

    fn snapshot(now: DateTime<Utc>) -> RuntimeInstrumentSizingContext {
        RuntimeInstrumentSizingContext {
            account_id: AccountId::new("account-a").unwrap(),
            instrument_id: InstrumentId::new("BTCUSDT").unwrap(),
            source_id: "test-provider".to_string(),
            captured_at: now,
            max_age_seconds: 10,
            sizing: InstrumentSizingContext {
                loss_model: LossModel::Linear {
                    contract_multiplier: Decimal::ONE,
                    pnl_to_account_rate: Decimal::ONE,
                },
                inputs_fresh: true,
                notional_to_account_rate: Decimal::ONE,
                quantity_step: Decimal::ONE,
                min_quantity: Decimal::ONE,
                max_quantity: Some(Decimal::from(100)),
                min_notional_account_currency: None,
                estimated_cost_per_quantity: Decimal::ZERO,
                slippage_gap_buffer_per_quantity: Decimal::ZERO,
                pending_order_exposure: Decimal::ZERO,
                available_margin: None,
                margin_per_quantity: None,
            },
        }
    }

    #[test]
    fn timestamped_snapshot_freshness_is_explicit_and_deterministic() {
        let now = Utc::now();
        assert!(snapshot(now).is_fresh_at(now));
        assert!(snapshot(now - Duration::seconds(10)).is_fresh_at(now));
        assert!(!snapshot(now - Duration::seconds(11)).is_fresh_at(now));
        assert!(!snapshot(now + Duration::seconds(1)).is_fresh_at(now));

        let mut unknown_bound = snapshot(now);
        unknown_bound.max_age_seconds = 0;
        assert!(!unknown_bound.is_fresh_at(now));

        let mut unknown_source = snapshot(now);
        unknown_source.source_id = "  ".to_string();
        assert!(!unknown_source.is_fresh_at(now));
    }
}

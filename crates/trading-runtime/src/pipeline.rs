//! Single-intent pipeline: TradeIntent → Risk → OMS → Venue → Position.

use std::collections::HashMap;

use account_engine::AccountEngine;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use domain::{
    AccountId, InstrumentId, OrderSide, OrderStatus, RiskDecision, RiskRequest, TradeIntent,
};
use events::{EventPublisher, TradingEvent};
use execution_engine::NewOrderRequest;
use risk_engine::{InstrumentSizingContext, InstrumentSizingOutcome, LossModel, RiskEvaluator};
use rust_decimal::Decimal;
use serde_json::{json, Value};
use tokio::sync::RwLock;
use venue_connectors::{submit_approved_order, VenueAdapter};

use crate::error::RuntimeResult;
use crate::ids::client_order_id_for_account_intent;
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
    /// Explicit conversion routes used by the trusted source for loss and notional.
    pub pnl_conversion_path: String,
    pub notional_conversion_path: String,
    /// True only when the provider validated supported instrument/order capabilities.
    pub venue_capabilities_validated: bool,
    /// True only when netting/hedging semantics are known and conservatively modeled.
    pub account_position_semantics_known: bool,
    /// Capture time of the oldest required input in this bundle.
    pub captured_at: DateTime<Utc>,
    /// Explicit configured freshness bound; unknown/zero bounds are invalid.
    pub max_age_seconds: i64,
    pub sizing: InstrumentSizingContext,
    /// Broker-side entry price from the same fresh quote used for sizing, when supplied.
    pub entry_price: Option<Decimal>,
}

impl RuntimeInstrumentSizingContext {
    fn is_fresh_at(&self, now: DateTime<Utc>) -> bool {
        if self.source_id.trim().is_empty()
            || self.pnl_conversion_path.trim().is_empty()
            || self.notional_conversion_path.trim().is_empty()
            || self.max_age_seconds <= 0
            || self.captured_at > now
            || !self.sizing.inputs_fresh
        {
            return false;
        }
        now.signed_duration_since(self.captured_at)
            <= chrono::Duration::seconds(self.max_age_seconds)
    }

    fn is_eligible_at(&self, now: DateTime<Utc>) -> bool {
        self.venue_capabilities_validated
            && self.account_position_semantics_known
            && self.is_fresh_at(now)
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

    /// Side-aware providers override this to select bid for sells and ask for buys.
    async fn load_snapshot_for_side(
        &self,
        account_id: &AccountId,
        instrument_id: &InstrumentId,
        _side: OrderSide,
    ) -> RuntimeResult<RuntimeInstrumentSizingContext> {
        self.load_snapshot(account_id, instrument_id).await
    }
}

/// Runtime-owned cache for validated account/instrument sizing snapshots.
/// Venue and market-data adapters publish snapshots here; execution reads only
/// the latest snapshot and rejects missing, stale, or out-of-order data.
#[derive(Debug, Default)]
pub struct InstrumentSizingSnapshotCache {
    snapshots: RwLock<HashMap<(AccountId, InstrumentId), RuntimeInstrumentSizingContext>>,
}

impl InstrumentSizingSnapshotCache {
    pub async fn publish_snapshot(
        &self,
        snapshot: RuntimeInstrumentSizingContext,
    ) -> RuntimeResult<()> {
        if !snapshot.is_eligible_at(Utc::now()) {
            return Err(crate::RuntimeError::Invariant(
                "refusing to cache invalid or stale instrument sizing snapshot".to_string(),
            ));
        }
        let key = (snapshot.account_id.clone(), snapshot.instrument_id.clone());
        let mut snapshots = self.snapshots.write().await;
        if let Some(existing) = snapshots.get(&key) {
            if snapshot.captured_at < existing.captured_at {
                return Err(crate::RuntimeError::Invariant(
                    "refusing to replace a newer sizing snapshot with older data".to_string(),
                ));
            }
            if snapshot.captured_at == existing.captured_at && snapshot != *existing {
                return Err(crate::RuntimeError::Invariant(
                    "conflicting sizing snapshots share the same capture timestamp".to_string(),
                ));
            }
        }
        snapshots.insert(key, snapshot);
        Ok(())
    }
}

#[async_trait]
impl InstrumentSizingSnapshotProvider for InstrumentSizingSnapshotCache {
    async fn load_snapshot(
        &self,
        account_id: &AccountId,
        instrument_id: &InstrumentId,
    ) -> RuntimeResult<RuntimeInstrumentSizingContext> {
        let key = (account_id.clone(), instrument_id.clone());
        let snapshot = self
            .snapshots
            .read()
            .await
            .get(&key)
            .cloned()
            .ok_or_else(|| {
                crate::RuntimeError::Invariant(
                    "no instrument sizing snapshot available for account/instrument".to_string(),
                )
            })?;
        if !snapshot.is_eligible_at(Utc::now()) {
            return Err(crate::RuntimeError::Invariant(
                "cached instrument sizing snapshot is stale".to_string(),
            ));
        }
        Ok(snapshot)
    }
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
            .load_snapshot_for_side(&self.config.account_id, &intent.instrument_id, intent.side)
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

        let mut risk_req = RiskRequest::from_intent(
            intent,
            self.config.account_id.clone(),
            account_state,
            self.config.risk_policy.clone(),
            Utc::now(),
        )
        .with_exposure(self.config.current_exposure)
        .with_session_allowed(self.config.session_allowed);

        if let Some(entry_price) = sizing_context.and_then(|snapshot| snapshot.entry_price) {
            risk_req.entry_price = Some(entry_price);
        }

        let evaluated_at = Utc::now();
        let (decision, sizing_outcome) = match sizing_context {
            Some(snapshot)
                if self.store.is_none()
                    || snapshot.account_id != self.config.account_id
                    || snapshot.instrument_id != intent.instrument_id
                    || !snapshot.is_eligible_at(evaluated_at)
                    || risk_req.account_state.updated_at > evaluated_at
                    || evaluated_at.signed_duration_since(risk_req.account_state.updated_at)
                        > chrono::Duration::seconds(snapshot.max_age_seconds) =>
            {
                (
                    RiskDecision::Rejected {
                        reason: domain::RiskRejectReason::DependencyUnavailable,
                    },
                    None,
                )
            }
            Some(snapshot) => self
                .risk
                .evaluate_with_instrument_sizing_detailed(&risk_req, &snapshot.sizing)?,
            None => (self.risk.evaluate(&risk_req)?, None),
        };
        self.publish_risk_decision(intent, &decision, events_out)
            .await?;

        if let Some(store) = &self.store {
            store
                .save_risk_decision(&intent.id, &self.config.account_id, &decision)
                .await?;
            if let Some(snapshot) = sizing_context {
                store
                    .save_sizing_audit(
                        &intent.id,
                        &self.config.account_id,
                        &intent.instrument_id,
                        &snapshot.source_id,
                        snapshot.captured_at,
                        evaluated_at,
                        sizing_audit_payload(
                            &risk_req,
                            snapshot,
                            &decision,
                            sizing_outcome.as_ref(),
                            evaluated_at,
                        ),
                    )
                    .await?;
            }
        }

        if !decision.is_executable() {
            return Ok(IntentOutcome {
                intent: intent.clone(),
                decision,
                order_status: None,
            });
        }

        let client_order_id =
            client_order_id_for_account_intent(&self.config.account_id, &intent.id)?;
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
                price: risk_req.entry_price,
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
    use risk_engine::LossModel;
    use rust_decimal::Decimal;

    fn snapshot(now: DateTime<Utc>) -> RuntimeInstrumentSizingContext {
        RuntimeInstrumentSizingContext {
            account_id: AccountId::new("account-a").unwrap(),
            instrument_id: InstrumentId::new("BTCUSDT").unwrap(),
            source_id: "test-provider".to_string(),
            pnl_conversion_path: "TEST:quote_to_account".to_string(),
            notional_conversion_path: "TEST:price_to_account".to_string(),
            venue_capabilities_validated: true,
            account_position_semantics_known: true,
            captured_at: now,
            max_age_seconds: 10,
            entry_price: None,
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
                pending_order_margin: Decimal::ZERO,
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

    #[tokio::test]
    async fn cache_rejects_stale_and_out_of_order_snapshots() {
        let now = Utc::now();
        let cache = InstrumentSizingSnapshotCache::default();
        let mut fresh = snapshot(now);
        fresh.max_age_seconds = 60;
        cache.publish_snapshot(fresh.clone()).await.unwrap();

        let mut older = fresh.clone();
        older.captured_at = now - Duration::seconds(1);
        assert!(cache.publish_snapshot(older).await.is_err());

        let mut stale = fresh.clone();
        stale.captured_at = now - Duration::seconds(120);
        assert!(cache.publish_snapshot(stale).await.is_err());

        let mut missing_conversion = fresh.clone();
        missing_conversion.pnl_conversion_path = " ".to_string();
        assert!(cache.publish_snapshot(missing_conversion).await.is_err());

        let mut unknown_capabilities = fresh.clone();
        unknown_capabilities.venue_capabilities_validated = false;
        assert!(cache.publish_snapshot(unknown_capabilities).await.is_err());

        let mut unknown_semantics = fresh;
        unknown_semantics.account_position_semantics_known = false;
        assert!(cache.publish_snapshot(unknown_semantics).await.is_err());
    }

    #[tokio::test]
    async fn cache_load_fails_closed_for_missing_instrument() {
        let cache = InstrumentSizingSnapshotCache::default();
        let result = cache
            .load_snapshot(
                &AccountId::new("account-a").unwrap(),
                &InstrumentId::new("ETHUSDT").unwrap(),
            )
            .await;
        assert!(result.is_err());
    }
}

fn optional_decimal(value: Option<Decimal>) -> Option<String> {
    value.map(|decimal| decimal.to_string())
}

fn sizing_audit_payload(
    request: &RiskRequest,
    snapshot: &RuntimeInstrumentSizingContext,
    decision: &RiskDecision,
    outcome: Option<&InstrumentSizingOutcome>,
    evaluated_at: chrono::DateTime<Utc>,
) -> Value {
    let loss_model = match snapshot.sizing.loss_model {
        LossModel::Linear {
            contract_multiplier,
            pnl_to_account_rate,
        } => json!({
            "type": "linear",
            "contract_multiplier": contract_multiplier.to_string(),
            "pnl_to_account_rate": pnl_to_account_rate.to_string()
        }),
        LossModel::TickValue {
            tick_size,
            tick_value_per_quantity,
            tick_value_to_account_rate,
        } => json!({
            "type": "tick_value",
            "tick_size": tick_size.to_string(),
            "tick_value_per_quantity": tick_value_per_quantity.to_string(),
            "tick_value_to_account_rate": tick_value_to_account_rate.to_string()
        }),
    };
    let sizing_result = match outcome {
        Some(InstrumentSizingOutcome::Approved {
            quantity,
            risk_budget,
            modeled_loss,
        }) => json!({
            "status": "approved",
            "quantity": quantity.to_string(),
            "risk_budget": risk_budget.to_string(),
            "modeled_loss": modeled_loss.to_string()
        }),
        Some(InstrumentSizingOutcome::Resized {
            quantity,
            risk_budget,
            modeled_loss,
            reason,
        }) => json!({
            "status": "resized",
            "quantity": quantity.to_string(),
            "risk_budget": risk_budget.to_string(),
            "modeled_loss": modeled_loss.to_string(),
            "limit_reason": format!("{:?}", reason)
        }),
        Some(InstrumentSizingOutcome::Rejected { reason }) => json!({
            "status": "rejected",
            "reject_reason": format!("{:?}", reason)
        }),
        None => json!({
            "status": "not_sized",
            "detail": "risk gate, identity, or freshness rejected before sizing"
        }),
    };
    let decision_kind = match decision {
        RiskDecision::Approved { .. } => "approved",
        RiskDecision::Resized { .. } => "resized",
        RiskDecision::Rejected { .. } => "rejected",
        RiskDecision::Halted { .. } => "halted",
    };

    json!({
        "schema_version": 1,
        "sizing_model_version": 1,
        "risk_policy_version": null,
        "evaluated_at": evaluated_at.to_rfc3339(),
        "account_id": request.account_id.as_str(),
        "instrument_id": request.instrument_id.as_str(),
        "trade_intent_id": request.trade_intent_id.as_str(),
        "snapshot": {
            "source_id": snapshot.source_id,
            "pnl_conversion_path": snapshot.pnl_conversion_path,
            "notional_conversion_path": snapshot.notional_conversion_path,
            "venue_capabilities_validated": snapshot.venue_capabilities_validated,
            "account_position_semantics_known": snapshot.account_position_semantics_known,
            "captured_at": snapshot.captured_at.to_rfc3339(),
            "max_age_seconds": snapshot.max_age_seconds,
            "inputs_fresh_flag": snapshot.sizing.inputs_fresh
        },
        "request": {
            "side": format!("{:?}", request.side),
            "entry_price": optional_decimal(request.entry_price),
            "stop_loss": optional_decimal(request.stop_loss),
            "requested_quantity": optional_decimal(request.requested_quantity),
            "requested_notional": optional_decimal(request.requested_notional),
            "equity": request.account_state.equity.to_string(),
            "account_balance": request.account_state.balance.to_string(),
            "account_state_updated_at": request.account_state.updated_at.to_rfc3339(),
            "used_margin": request.account_state.used_margin.to_string(),
            "daily_realized_pnl": request.account_state.daily_realized_pnl.to_string(),
            "peak_equity": request.account_state.peak_equity.to_string(),
            "account_kill_switch": request.account_state.kill_switch,
            "session_allowed": request.session_allowed,
            "current_exposure": request.current_exposure.to_string(),
            "risk_per_trade_pct": request.policy.max_risk_per_trade_pct.to_string(),
            "position_cap_pct": request.policy.max_position_size_pct.to_string(),
            "exposure_cap_pct": request.policy.max_total_exposure_pct.to_string(),
            "max_leverage": request.policy.max_leverage.to_string()
        },
        "risk_policy_snapshot": {
            "version": null,
            "max_risk_per_trade_pct": request.policy.max_risk_per_trade_pct.to_string(),
            "max_position_size_pct": request.policy.max_position_size_pct.to_string(),
            "max_total_exposure_pct": request.policy.max_total_exposure_pct.to_string(),
            "max_leverage": request.policy.max_leverage.to_string(),
            "max_daily_loss_pct": request.policy.max_daily_loss_pct.to_string(),
            "max_drawdown_pct": request.policy.max_drawdown_pct.to_string(),
            "max_open_positions": request.policy.max_open_positions,
            "allowed_instruments": request.policy.allowed_instruments.as_ref().map(|items| {
                items.iter().map(|id| id.as_str().to_string()).collect::<Vec<_>>()
            }),
            "restricted_instruments": request.policy.restricted_instruments.iter()
                .map(|id| id.as_str().to_string()).collect::<Vec<_>>(),
            "global_kill_switch": request.policy.global_kill_switch,
            "account_kill_switch": request.policy.account_kill_switch,
            "strategy_kill_switch": request.policy.strategy_kill_switch
        },
        "loss_model": loss_model,
        "constraints": {
            "notional_to_account_rate": snapshot.sizing.notional_to_account_rate.to_string(),
            "quantity_step": snapshot.sizing.quantity_step.to_string(),
            "min_quantity": snapshot.sizing.min_quantity.to_string(),
            "max_quantity": optional_decimal(snapshot.sizing.max_quantity),
            "min_notional_account_currency": optional_decimal(snapshot.sizing.min_notional_account_currency),
            "estimated_cost_per_quantity": snapshot.sizing.estimated_cost_per_quantity.to_string(),
            "slippage_gap_buffer_per_quantity": snapshot.sizing.slippage_gap_buffer_per_quantity.to_string(),
            "pending_order_exposure": snapshot.sizing.pending_order_exposure.to_string(),
            "available_margin": optional_decimal(snapshot.sizing.available_margin),
            "pending_order_margin": snapshot.sizing.pending_order_margin.to_string(),
            "margin_per_quantity": optional_decimal(snapshot.sizing.margin_per_quantity)
        },
        "decision": {
            "kind": decision_kind,
            "detail": format!("{:?}", decision)
        },
        "sizing_result": sizing_result
    })
}

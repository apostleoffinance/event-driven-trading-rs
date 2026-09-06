//! Single-intent pipeline: TradeIntent → Risk → OMS → Venue → Position.

use account_engine::AccountEngine;
use chrono::Utc;
use domain::{OrderStatus, RiskDecision, RiskRequest, TradeIntent};
use events::{EventPublisher, TradingEvent};
use execution_engine::NewOrderRequest;
use risk_engine::RiskEvaluator;
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

impl TradingRuntime {
    /// Evaluate risk and, when executable, submit through OMS + venue.
    pub async fn execute_intent(
        &mut self,
        intent: &TradeIntent,
        venue: &dyn VenueAdapter,
        events_out: &EventPublisher,
    ) -> RuntimeResult<IntentOutcome> {
        if let Some(store) = &self.store {
            store.save_trade_intent(intent).await?;
        }

        let account_state = self.accounts.get(&self.config.account_id)?.state.clone();

        events_out
            .publish(TradingEvent::RiskCheckRequested {
                trade_intent_id: intent.id.clone(),
                account_id: self.config.account_id.clone(),
                deployment_id: intent.deployment_id.clone(),
            })
            .await?;

        let risk_req = RiskRequest::from_intent(
            intent,
            self.config.account_id.clone(),
            account_state,
            self.config.risk_policy.clone(),
            Utc::now(),
        )
        .with_exposure(self.config.current_exposure)
        .with_session_allowed(self.config.session_allowed);

        let decision = self.risk.evaluate(&risk_req)?;
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

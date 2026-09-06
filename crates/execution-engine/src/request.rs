//! Order creation request — requires account, venue, client id, and risk approval.

use chrono::{DateTime, Utc};
use domain::{
    AccountId, ClientOrderId, DeploymentId, InstrumentId, InstrumentSpec, OrderSide, OrderType,
    RiskDecision, StrategyId, TimeInForce, TradeIntentId, VenueId,
};
use rust_decimal::Decimal;

use crate::error::{ExecutionError, ExecutionResult};

/// Request to create an order after risk approval.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewOrderRequest {
    pub client_order_id: ClientOrderId,
    pub account_id: AccountId,
    pub venue_id: VenueId,
    pub deployment_id: DeploymentId,
    pub strategy_id: StrategyId,
    pub trade_intent_id: TradeIntentId,
    pub instrument_id: InstrumentId,
    pub side: OrderSide,
    pub order_type: OrderType,
    pub time_in_force: TimeInForce,
    pub price: Option<Decimal>,
    /// Must be [`RiskDecision::Approved`] or [`RiskDecision::Resized`].
    pub risk_decision: RiskDecision,
    /// When set, quantity/price are validated against market-structure rules.
    pub instrument_spec: Option<InstrumentSpec>,
    pub created_at: DateTime<Utc>,
}

impl NewOrderRequest {
    pub fn validated_quantity(&self) -> ExecutionResult<Decimal> {
        match &self.risk_decision {
            RiskDecision::Approved { quantity } | RiskDecision::Resized { quantity, .. } => {
                if *quantity <= Decimal::ZERO {
                    return Err(ExecutionError::RiskNotExecutable(
                        "approved quantity must be positive".to_string(),
                    ));
                }
                if let Some(spec) = &self.instrument_spec {
                    spec.validate_order_params(*quantity, self.price)?;
                }
                Ok(*quantity)
            }
            RiskDecision::Rejected { reason } => Err(ExecutionError::RiskNotExecutable(format!(
                "rejected: {reason:?}"
            ))),
            RiskDecision::Halted { reason } => Err(ExecutionError::RiskNotExecutable(format!(
                "halted: {reason}"
            ))),
        }
    }
}

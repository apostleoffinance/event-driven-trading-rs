//! Trade intent: strategy output prior to risk sizing and order creation.
//!
//! Strategies express direction and optional desired exposure. They must not
//! compute account-level position size or risk limits.

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use crate::error::{DomainError, DomainResult};
use crate::ids::{DeploymentId, InstrumentId, StrategyId, StrategyVersion, TradeIntentId};
use crate::money::{require_non_negative, require_positive};
use crate::order::OrderSide;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TradeIntent {
    pub id: TradeIntentId,
    pub strategy_id: StrategyId,
    pub strategy_version: StrategyVersion,
    pub deployment_id: DeploymentId,
    pub instrument_id: InstrumentId,
    pub side: OrderSide,
    pub target_quantity: Option<Decimal>,
    pub target_notional: Option<Decimal>,
    pub entry_price: Option<Decimal>,
    pub stop_loss: Option<Decimal>,
    pub take_profit: Option<Decimal>,
    /// Optional confidence in \[0, 1\].
    pub confidence: Option<Decimal>,
    pub timestamp: DateTime<Utc>,
}

impl TradeIntent {
    pub fn builder(
        id: TradeIntentId,
        strategy_id: StrategyId,
        strategy_version: StrategyVersion,
        deployment_id: DeploymentId,
        instrument_id: InstrumentId,
        side: OrderSide,
        timestamp: DateTime<Utc>,
    ) -> TradeIntentBuilder {
        TradeIntentBuilder {
            id,
            strategy_id,
            strategy_version,
            deployment_id,
            instrument_id,
            side,
            target_quantity: None,
            target_notional: None,
            entry_price: None,
            stop_loss: None,
            take_profit: None,
            confidence: None,
            timestamp,
        }
    }
}

#[derive(Debug)]
pub struct TradeIntentBuilder {
    id: TradeIntentId,
    strategy_id: StrategyId,
    strategy_version: StrategyVersion,
    deployment_id: DeploymentId,
    instrument_id: InstrumentId,
    side: OrderSide,
    target_quantity: Option<Decimal>,
    target_notional: Option<Decimal>,
    entry_price: Option<Decimal>,
    stop_loss: Option<Decimal>,
    take_profit: Option<Decimal>,
    confidence: Option<Decimal>,
    timestamp: DateTime<Utc>,
}

impl TradeIntentBuilder {
    pub fn target_quantity(mut self, qty: Decimal) -> DomainResult<Self> {
        self.target_quantity = Some(require_positive(qty, "target_quantity")?);
        Ok(self)
    }

    pub fn target_notional(mut self, notional: Decimal) -> DomainResult<Self> {
        self.target_notional = Some(require_positive(notional, "target_notional")?);
        Ok(self)
    }

    pub fn entry_price(mut self, price: Decimal) -> DomainResult<Self> {
        self.entry_price = Some(require_positive(price, "entry_price")?);
        Ok(self)
    }

    pub fn stop_loss(mut self, price: Decimal) -> DomainResult<Self> {
        self.stop_loss = Some(require_positive(price, "stop_loss")?);
        Ok(self)
    }

    pub fn take_profit(mut self, price: Decimal) -> DomainResult<Self> {
        self.take_profit = Some(require_positive(price, "take_profit")?);
        Ok(self)
    }

    pub fn confidence(mut self, value: Decimal) -> DomainResult<Self> {
        require_non_negative(value, "confidence")?;
        if value > Decimal::ONE {
            return Err(DomainError::Validation(
                "confidence must be between 0 and 1 inclusive".to_string(),
            ));
        }
        self.confidence = Some(value);
        Ok(self)
    }

    pub fn build(self) -> DomainResult<TradeIntent> {
        if self.target_quantity.is_none() && self.target_notional.is_none() {
            // Strategies may emit directional intents without size; risk will size.
            // Allowed by design — no error.
        }
        Ok(TradeIntent {
            id: self.id,
            strategy_id: self.strategy_id,
            strategy_version: self.strategy_version,
            deployment_id: self.deployment_id,
            instrument_id: self.instrument_id,
            side: self.side,
            target_quantity: self.target_quantity,
            target_notional: self.target_notional,
            entry_price: self.entry_price,
            stop_loss: self.stop_loss,
            take_profit: self.take_profit,
            confidence: self.confidence,
            timestamp: self.timestamp,
        })
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;
    use crate::ids::*;

    fn base_builder() -> TradeIntentBuilder {
        TradeIntent::builder(
            TradeIntentId::new("ti-1").unwrap(),
            StrategyId::new("btc-mean-reversion").unwrap(),
            StrategyVersion::new("v1").unwrap(),
            DeploymentId::new("deployment-001").unwrap(),
            InstrumentId::new("BTCUSDT").unwrap(),
            OrderSide::Buy,
            Utc::now(),
        )
    }

    #[test]
    fn builds_intent_without_size() {
        let intent = base_builder().build().unwrap();
        assert!(intent.target_quantity.is_none());
    }

    #[test]
    fn rejects_invalid_confidence() {
        let err = base_builder().confidence(Decimal::from(2)).unwrap_err();
        assert!(matches!(err, DomainError::Validation(_)));
    }
}

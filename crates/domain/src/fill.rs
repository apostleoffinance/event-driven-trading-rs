//! Fill / execution report domain type.

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use crate::error::DomainResult;
use crate::ids::{FillId, InstrumentId, OrderId};
use crate::money::{require_non_negative, require_positive};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Fill {
    pub id: FillId,
    pub order_id: OrderId,
    pub instrument_id: InstrumentId,
    pub price: Decimal,
    pub quantity: Decimal,
    pub fee: Decimal,
    pub timestamp: DateTime<Utc>,
}

impl Fill {
    pub fn new(
        id: FillId,
        order_id: OrderId,
        instrument_id: InstrumentId,
        price: Decimal,
        quantity: Decimal,
        fee: Decimal,
        timestamp: DateTime<Utc>,
    ) -> DomainResult<Self> {
        Ok(Self {
            id,
            order_id,
            instrument_id,
            price: require_positive(price, "fill_price")?,
            quantity: require_positive(quantity, "fill_quantity")?,
            fee: require_non_negative(fee, "fill_fee")?,
            timestamp,
        })
    }

    pub fn notional(&self) -> Decimal {
        (self.price * self.quantity).round_dp(8)
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;
    use crate::ids::*;
    use rust_decimal::Decimal;

    #[test]
    fn fill_rejects_negative_fee() {
        let err = Fill::new(
            FillId::new("f1").unwrap(),
            OrderId::new("o1").unwrap(),
            InstrumentId::new("BTCUSDT").unwrap(),
            Decimal::from(100),
            Decimal::ONE,
            -Decimal::ONE,
            Utc::now(),
        )
        .unwrap_err();
        assert!(matches!(err, crate::error::DomainError::InvalidMoney(_)));
    }
}

//! Position domain types.

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use crate::error::{DomainError, DomainResult};
use crate::ids::{AccountId, InstrumentId, PositionId};
use crate::money::{require_non_negative, require_positive};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PositionSide {
    Long,
    Short,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Position {
    pub id: PositionId,
    pub account_id: AccountId,
    pub instrument_id: InstrumentId,
    pub side: PositionSide,
    pub quantity: Decimal,
    pub avg_entry_price: Decimal,
    pub stop_loss: Option<Decimal>,
    pub mark_price: Decimal,
    pub realized_pnl: Decimal,
    pub opened_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl Position {
    #[allow(clippy::too_many_arguments)]
    pub fn open(
        id: PositionId,
        account_id: AccountId,
        instrument_id: InstrumentId,
        side: PositionSide,
        quantity: Decimal,
        entry_price: Decimal,
        stop_loss: Option<Decimal>,
        opened_at: DateTime<Utc>,
    ) -> DomainResult<Self> {
        let quantity = require_positive(quantity, "quantity")?;
        let entry_price = require_positive(entry_price, "entry_price")?;
        if let Some(sl) = stop_loss {
            require_positive(sl, "stop_loss")?;
        }
        Ok(Self {
            id,
            account_id,
            instrument_id,
            side,
            quantity,
            avg_entry_price: entry_price,
            stop_loss,
            mark_price: entry_price,
            realized_pnl: Decimal::ZERO,
            opened_at,
            updated_at: opened_at,
        })
    }

    pub fn unrealized_pnl(&self) -> Decimal {
        let diff = self.mark_price - self.avg_entry_price;
        let signed = match self.side {
            PositionSide::Long => diff,
            PositionSide::Short => -diff,
        };
        (signed * self.quantity).round_dp(8)
    }

    pub fn update_mark(&mut self, price: Decimal, at: DateTime<Utc>) -> DomainResult<()> {
        self.mark_price = require_positive(price, "mark_price")?;
        self.updated_at = at;
        Ok(())
    }

    pub fn reduce(
        &mut self,
        qty: Decimal,
        exit_price: Decimal,
        at: DateTime<Utc>,
    ) -> DomainResult<Decimal> {
        let qty = require_positive(qty, "reduce_qty")?;
        let exit_price = require_positive(exit_price, "exit_price")?;
        if qty > self.quantity {
            return Err(DomainError::Invariant(
                "cannot reduce position by more than open quantity".to_string(),
            ));
        }
        let diff = exit_price - self.avg_entry_price;
        let signed = match self.side {
            PositionSide::Long => diff,
            PositionSide::Short => -diff,
        };
        let pnl = (signed * qty).round_dp(8);
        self.realized_pnl += pnl;
        self.quantity = require_non_negative(self.quantity - qty, "quantity")?;
        self.mark_price = exit_price;
        self.updated_at = at;
        Ok(pnl)
    }

    pub fn is_flat(&self) -> bool {
        self.quantity == Decimal::ZERO
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;
    use crate::ids::*;
    use rust_decimal::Decimal;

    #[test]
    fn long_unrealized_pnl() {
        let mut pos = Position::open(
            PositionId::new("p1").unwrap(),
            AccountId::new("a1").unwrap(),
            InstrumentId::new("BTCUSDT").unwrap(),
            PositionSide::Long,
            Decimal::from(2),
            Decimal::from(100),
            None,
            Utc::now(),
        )
        .unwrap();
        pos.update_mark(Decimal::from(110), Utc::now()).unwrap();
        assert_eq!(pos.unrealized_pnl(), Decimal::from(20));
    }
}

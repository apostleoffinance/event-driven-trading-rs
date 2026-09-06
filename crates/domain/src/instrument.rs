//! Instrument domain. Cross-asset variants exist for future use;
//! only crypto is required for the current prop/crypto path.
//!
//! [`InstrumentSpec`] encodes market-structure precision rules. Orders that
//! violate the spec must be rejected or normalized **before** venue submission.

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use crate::error::{DomainError, DomainResult};
use crate::ids::InstrumentId;
use crate::money::{require_non_negative, require_positive};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum InstrumentType {
    Crypto,
    Fx,
    Equity,
    Future,
    Option,
    Rate,
    Credit,
}

/// Explicit market-structure constraints for an instrument.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstrumentSpec {
    pub tick_size: Decimal,
    pub lot_size: Decimal,
    pub min_quantity: Decimal,
    pub min_notional: Option<Decimal>,
    pub price_precision: u32,
    pub quantity_precision: u32,
}

impl InstrumentSpec {
    pub fn try_new(
        tick_size: Decimal,
        lot_size: Decimal,
        min_quantity: Decimal,
        min_notional: Option<Decimal>,
        price_precision: u32,
        quantity_precision: u32,
    ) -> DomainResult<Self> {
        let tick_size = require_positive(tick_size, "tick_size")?;
        let lot_size = require_positive(lot_size, "lot_size")?;
        let min_quantity = require_positive(min_quantity, "min_quantity")?;
        if let Some(n) = min_notional {
            require_non_negative(n, "min_notional")?;
        }
        if min_quantity < lot_size {
            return Err(DomainError::Validation(
                "min_quantity must be >= lot_size".to_string(),
            ));
        }
        Ok(Self {
            tick_size,
            lot_size,
            min_quantity,
            min_notional,
            price_precision,
            quantity_precision,
        })
    }

    /// Sensible defaults for crypto spot paper trading (fine granularity).
    pub fn crypto_spot_default() -> DomainResult<Self> {
        Self::try_new(
            Decimal::new(1, 2),   // 0.01 tick
            Decimal::new(1, 8),   // 1e-8 lot
            Decimal::new(1, 8),   // min qty
            Some(Decimal::from(5)),
            8,
            8,
        )
    }

    /// Reject quantities that violate lot / min / precision constraints.
    pub fn validate_quantity(&self, quantity: Decimal) -> DomainResult<Decimal> {
        let quantity = require_positive(quantity, "quantity")?;
        if quantity < self.min_quantity {
            return Err(DomainError::Validation(format!(
                "quantity {quantity} below min_quantity {}",
                self.min_quantity
            )));
        }
        if !is_multiple_of(quantity, self.lot_size) {
            return Err(DomainError::Validation(format!(
                "quantity {quantity} is not a multiple of lot_size {}",
                self.lot_size
            )));
        }
        Ok(quantity)
    }

    /// Reject prices that violate tick constraints.
    pub fn validate_price(&self, price: Decimal) -> DomainResult<Decimal> {
        let price = require_positive(price, "price")?;
        if !is_multiple_of(price, self.tick_size) {
            return Err(DomainError::Validation(format!(
                "price {price} is not a multiple of tick_size {}",
                self.tick_size
            )));
        }
        Ok(price)
    }

    /// Reject when quantity × price is below min notional (when configured).
    pub fn validate_notional(&self, quantity: Decimal, price: Decimal) -> DomainResult<()> {
        if let Some(min) = self.min_notional {
            let notional = quantity * price;
            if notional < min {
                return Err(DomainError::Validation(format!(
                    "notional {notional} below min_notional {min}"
                )));
            }
        }
        Ok(())
    }

    /// Full pre-submit check for quantity and optional limit price.
    pub fn validate_order_params(
        &self,
        quantity: Decimal,
        price: Option<Decimal>,
    ) -> DomainResult<()> {
        let quantity = self.validate_quantity(quantity)?;
        if let Some(px) = price {
            let px = self.validate_price(px)?;
            self.validate_notional(quantity, px)?;
        }
        Ok(())
    }
}

fn is_multiple_of(value: Decimal, step: Decimal) -> bool {
    if step <= Decimal::ZERO {
        return false;
    }
    let ratio = value / step;
    ratio == ratio.trunc()
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Instrument {
    pub id: InstrumentId,
    pub instrument_type: InstrumentType,
    pub symbol: String,
    pub base: Option<String>,
    pub quote: Option<String>,
    pub enabled: bool,
    /// Market-structure rules; required for live/paper submission validation.
    pub spec: InstrumentSpec,
}

impl Instrument {
    pub fn crypto_spot(
        id: InstrumentId,
        symbol: impl Into<String>,
        base: impl Into<String>,
        quote: impl Into<String>,
    ) -> DomainResult<Self> {
        Self::crypto_spot_with_spec(id, symbol, base, quote, InstrumentSpec::crypto_spot_default()?)
    }

    pub fn crypto_spot_with_spec(
        id: InstrumentId,
        symbol: impl Into<String>,
        base: impl Into<String>,
        quote: impl Into<String>,
        spec: InstrumentSpec,
    ) -> DomainResult<Self> {
        let symbol = symbol.into();
        if symbol.trim().is_empty() {
            return Err(DomainError::Validation(
                "instrument symbol must be non-empty".to_string(),
            ));
        }
        Ok(Self {
            id,
            instrument_type: InstrumentType::Crypto,
            symbol,
            base: Some(base.into()),
            quote: Some(quote.into()),
            enabled: true,
            spec,
        })
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;
    use crate::ids::InstrumentId;

    #[test]
    fn crypto_spot_btc() {
        let inst = Instrument::crypto_spot(
            InstrumentId::new("BTCUSDT").unwrap(),
            "BTCUSDT",
            "BTC",
            "USDT",
        )
        .unwrap();
        assert_eq!(inst.instrument_type, InstrumentType::Crypto);
        assert!(inst.spec.lot_size > Decimal::ZERO);
    }

    #[test]
    fn rejects_bad_lot() {
        let spec = InstrumentSpec::try_new(
            Decimal::new(1, 2),
            Decimal::new(1, 3),
            Decimal::new(1, 3),
            None,
            2,
            3,
        )
        .unwrap();
        let err = spec
            .validate_quantity(Decimal::new(15, 4)) // 0.0015 — not multiple of 0.001
            .unwrap_err();
        assert!(matches!(err, DomainError::Validation(_)));
    }

    #[test]
    fn accepts_valid_qty_and_price() {
        let spec = InstrumentSpec::crypto_spot_default().unwrap();
        spec.validate_order_params(Decimal::ONE, Some(Decimal::from(100)))
            .unwrap();
    }
}

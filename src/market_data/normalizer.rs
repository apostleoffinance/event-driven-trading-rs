use rust_decimal::Decimal;
use crate::error::{TradingError, Result};
use super::event::PriceEvent;

pub struct PriceValidator;

impl PriceValidator {
    pub fn validate(event: &PriceEvent) -> Result<()> {
        if event.price <= Decimal::ZERO {
            return Err(TradingError::Validation("Price must be positive".to_string()));
        }

        if event.volume < Decimal::ZERO {
            return Err(TradingError::Validation("Volume cannot be negative".to_string()));
        }

        if event.symbol.is_empty() {
            return Err(TradingError::Validation("Symbol cannot be empty".to_string()));
        }

        Ok(())
    }

    pub fn normalize(mut event: PriceEvent) -> Result<PriceEvent> {
        Self::validate(&event)?;
        event.price = event.price.round_dp(8);
        Ok(event)
    }
}

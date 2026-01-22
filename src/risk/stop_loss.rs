use rust_decimal::Decimal;
use crate::error::{TradingError, Result};

/// Manages stop loss logic to prevent catastrophic losses
pub struct StopLossManager;

impl StopLossManager {
    /// Calculate stop loss price given entry price and stop loss distance
    pub fn calculate_stop_loss(
        entry_price: Decimal,
        stop_loss_distance: Decimal,
        is_long: bool,
    ) -> Result<Decimal> {
        if entry_price <= Decimal::ZERO {
            return Err(TradingError::Validation(
                "Entry price must be positive".to_string(),
            ));
        }

        if stop_loss_distance <= Decimal::ZERO {
            return Err(TradingError::Validation(
                "Stop loss distance must be positive".to_string(),
            ));
        }

        let stop_loss = if is_long {
            // For long position: stop loss is below entry
            entry_price - stop_loss_distance
        } else {
            // For short position: stop loss is above entry
            entry_price + stop_loss_distance
        };

        if stop_loss <= Decimal::ZERO {
            return Err(TradingError::Validation(
                "Stop loss price would be invalid".to_string(),
            ));
        }

        Ok(stop_loss.round_dp(8))
    }

    /// Check if stop loss has been hit
    pub fn is_stop_hit(
        current_price: Decimal,
        stop_loss_price: Decimal,
        is_long: bool,
    ) -> Result<bool> {
        if current_price <= Decimal::ZERO || stop_loss_price <= Decimal::ZERO {
            return Err(TradingError::Validation(
                "Prices must be positive".to_string(),
            ));
        }

        let hit = if is_long {
            current_price <= stop_loss_price
        } else {
            current_price >= stop_loss_price
        };

        Ok(hit)
    }

    /// Calculate unrealized loss/gain
    pub fn calculate_pnl(
        entry_price: Decimal,
        current_price: Decimal,
        position_size: Decimal,
        is_long: bool,
    ) -> Result<Decimal> {
        if entry_price <= Decimal::ZERO || current_price <= Decimal::ZERO || position_size <= Decimal::ZERO {
            return Err(TradingError::Validation(
                "All prices and position size must be positive".to_string(),
            ));
        }

        let price_diff = if is_long {
            current_price - entry_price
        } else {
            entry_price - current_price
        };

        let pnl = price_diff * position_size;
        Ok(pnl.round_dp(8))
    }
}

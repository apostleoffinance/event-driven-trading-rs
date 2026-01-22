use rust_decimal::Decimal;
use crate::error::{TradingError, Result};

/// Calculates position size based on risk management rules
pub struct PositionSizer;

impl PositionSizer {
    /// Calculate position size based on:
    /// - Account balance
    /// - Risk per trade (e.g., 2%)
    /// - Stop loss distance (price difference)
    ///
    /// Formula: Position Size = (Account Balance × Risk %) / Stop Loss Distance
    pub fn calculate(
        account_balance: Decimal,
        risk_percentage: Decimal,
        stop_loss_distance: Decimal,
    ) -> Result<Decimal> {
        // Validate inputs
        if account_balance <= Decimal::ZERO {
            return Err(TradingError::Validation(
                "Account balance must be positive".to_string(),
            ));
        }

        if risk_percentage <= Decimal::ZERO || risk_percentage > Decimal::from(100) {
            return Err(TradingError::Validation(
                "Risk percentage must be between 0 and 100".to_string(),
            ));
        }

        if stop_loss_distance <= Decimal::ZERO {
            return Err(TradingError::Validation(
                "Stop loss distance must be positive".to_string(),
            ));
        }

        // Calculate risk amount
        let risk_amount = account_balance * (risk_percentage / Decimal::from(100));

        // Calculate position size
        let position_size = risk_amount / stop_loss_distance;

        Ok(position_size.round_dp(8))
    }

    /// Calculate max position size as percentage of account
    /// Prevents over-leveraging
    pub fn max_position_size(
        account_balance: Decimal,
        max_position_percentage: Decimal,
    ) -> Result<Decimal> {
        if account_balance <= Decimal::ZERO {
            return Err(TradingError::Validation(
                "Account balance must be positive".to_string(),
            ));
        }

        if max_position_percentage <= Decimal::ZERO || max_position_percentage > Decimal::from(100) {
            return Err(TradingError::Validation(
                "Max position percentage must be between 0 and 100".to_string(),
            ));
        }

        let max_size = account_balance * (max_position_percentage / Decimal::from(100));
        Ok(max_size.round_dp(8))
    }
}

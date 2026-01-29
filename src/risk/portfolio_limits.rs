use rust_decimal::Decimal;
use crate::config::strategy_config::RiskParams;
use crate::error::{TradingError, Result};

/// Manages portfolio-level risk limits
#[derive(Debug, Clone)]
pub struct PortfolioLimits {
    pub max_daily_loss: Decimal,
    pub max_position_size: Decimal,
    pub max_leverage: Decimal,
    pub max_open_positions: usize,
}

impl PortfolioLimits {
    /// Build limits from risk profile parameters and account balance
    pub fn from_risk_params(account_balance: Decimal, params: RiskParams) -> Result<Self> {
        if account_balance <= Decimal::ZERO {
            return Err(TradingError::Validation(
                "Account balance must be positive".to_string(),
            ));
        }

        let max_daily_loss = account_balance * (params.max_daily_loss / Decimal::from(100));
        let max_position_size = account_balance * (params.max_position_size / Decimal::from(100));

        Self::new(
            max_daily_loss,
            max_position_size,
            params.max_leverage,
            params.max_open_positions,
        )
    }

    pub fn new(
        max_daily_loss: Decimal,
        max_position_size: Decimal,
        max_leverage: Decimal,
        max_open_positions: usize,
    ) -> Result<Self> {
        if max_daily_loss <= Decimal::ZERO {
            return Err(TradingError::Validation(
                "Max daily loss must be positive".to_string(),
            ));
        }

        if max_position_size <= Decimal::ZERO {
            return Err(TradingError::Validation(
                "Max position size must be positive".to_string(),
            ));
        }

        if max_leverage < Decimal::ONE {
            return Err(TradingError::Validation(
                "Max leverage must be at least 1.0".to_string(),
            ));
        }

        if max_open_positions == 0 {
            return Err(TradingError::Validation(
                "Max open positions must be at least 1".to_string(),
            ));
        }

        Ok(Self {
            max_daily_loss,
            max_position_size,
            max_leverage,
            max_open_positions,
        })
    }

    /// Check if daily loss limit has been exceeded
    pub fn is_daily_loss_exceeded(&self, current_daily_loss: Decimal) -> Result<bool> {
        if current_daily_loss < Decimal::ZERO {
            return Err(TradingError::Validation(
                "Daily loss cannot be negative".to_string(),
            ));
        }

        Ok(current_daily_loss.abs() > self.max_daily_loss)
    }

    /// Check if position size exceeds limit
    pub fn is_position_too_large(&self, position_size: Decimal) -> Result<bool> {
        if position_size <= Decimal::ZERO {
            return Err(TradingError::Validation(
                "Position size must be positive".to_string(),
            ));
        }

        Ok(position_size > self.max_position_size)
    }

    /// Check if leverage exceeds limit
    pub fn is_leverage_exceeded(&self, used_leverage: Decimal) -> Result<bool> {
        if used_leverage <= Decimal::ZERO {
            return Err(TradingError::Validation(
                "Leverage must be positive".to_string(),
            ));
        }

        Ok(used_leverage > self.max_leverage)
    }

    /// Check if max open positions limit reached
    pub fn can_open_new_position(&self, current_open_positions: usize) -> Result<bool> {
        Ok(current_open_positions < self.max_open_positions)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_portfolio_limits_creation() {
        let limits = PortfolioLimits::new(
            Decimal::from(1000),
            Decimal::from(5000),
            Decimal::from(2),
            10,
        );

        assert!(limits.is_ok());
    }

    #[test]
    fn test_portfolio_limits_invalid_daily_loss() {
        let limits = PortfolioLimits::new(
            Decimal::from(-1000),
            Decimal::from(5000),
            Decimal::from(2),
            10,
        );

        assert!(limits.is_err());
    }
}

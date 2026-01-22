use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use crate::error::{TradingError, Result};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StrategyConfig {
    pub strategy_type: StrategyType,
    pub symbol: String,
    pub risk_profile: RiskProfile,  // Changed from risk_percentage
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum StrategyType {
    MeanReversion {
        threshold: Decimal,
        window_size: usize,
    },
    MovingAverage {
        short_window: usize,
        long_window: usize,
    },
}

/// Risk profiles - institutional-grade risk management
/// Users choose a profile, not raw numbers
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum RiskProfile {
    /// Conservative: Low volatility target, small positions
    /// Best for: Learning, paper trading, risk-averse traders
    Conservative,
    /// Balanced: Moderate volatility target, medium positions
    /// Best for: Most traders, balanced approach
    Balanced,
    /// Aggressive: High volatility target, larger positions
    /// Best for: Experienced traders, high conviction trades
    Aggressive,
}

impl RiskProfile {
    /// Get risk parameters for this profile
    /// NOTE: All Decimal values are hardcoded literals verified at compile time
    pub fn params(&self) -> RiskParams {
        match self {
            RiskProfile::Conservative => RiskParams {
                max_risk_per_trade: Decimal::new(1, 0),         // 1% per trade
                max_daily_loss: Decimal::new(5, 0),             // 5% daily limit
                max_drawdown: Decimal::new(10, 0),              // 10% max drawdown
                max_position_size: Decimal::new(1, 0),          // 1% of account per position
                max_open_positions: 3,                          // Max 3 positions
                max_leverage: Decimal::from(1),                 // No leverage
            },
            RiskProfile::Balanced => RiskParams {
                max_risk_per_trade: Decimal::new(2, 0),         // 2% per trade
                max_daily_loss: Decimal::new(10, 0),            // 10% daily limit
                max_drawdown: Decimal::new(20, 0),              // 20% max drawdown
                max_position_size: Decimal::new(2, 0),          // 2% of account per position
                max_open_positions: 5,                          // Max 5 positions
                max_leverage: Decimal::new(15, 1),              // 1.5x leverage
            },
            RiskProfile::Aggressive => RiskParams {
                max_risk_per_trade: Decimal::new(3, 0),         // 3% per trade
                max_daily_loss: Decimal::new(15, 0),            // 15% daily limit
                max_drawdown: Decimal::new(30, 0),              // 30% max drawdown
                max_position_size: Decimal::new(5, 0),          // 5% of account per position
                max_open_positions: 10,                         // Max 10 positions
                max_leverage: Decimal::from(2),                 // 2x leverage
            },
        }
    }

    pub fn description(&self) -> &str {
        match self {
            RiskProfile::Conservative => "Conservative (1% per trade, 5% daily limit, no leverage)",
            RiskProfile::Balanced => "Balanced (2% per trade, 10% daily limit, 1.5x leverage)",
            RiskProfile::Aggressive => "Aggressive (3% per trade, 15% daily limit, 2x leverage)",
        }
    }
}

/// Risk parameters for a profile
#[derive(Debug, Clone, Copy)]
pub struct RiskParams {
    pub max_risk_per_trade: Decimal,      // % of account per trade
    pub max_daily_loss: Decimal,          // % daily loss limit
    pub max_drawdown: Decimal,            // % maximum drawdown
    pub max_position_size: Decimal,       // % of account per position
    pub max_open_positions: usize,        // Max concurrent positions
    pub max_leverage: Decimal,            // Maximum leverage allowed
}

impl StrategyConfig {
    pub fn validate(&self) -> Result<()> {
        if self.symbol.is_empty() {
            return Err(TradingError::Validation(
                "Symbol cannot be empty".to_string(),
            ));
        }

        // Validate strategy parameters based on type
        match &self.strategy_type {
            StrategyType::MeanReversion { threshold, window_size } => {
                if *threshold <= Decimal::ZERO || *threshold >= Decimal::ONE {
                    return Err(TradingError::Validation(
                        "MeanReversion threshold must be between 0 and 1".to_string(),
                    ));
                }
                if *window_size == 0 {
                    return Err(TradingError::Validation(
                        "Window size must be greater than 0".to_string(),
                    ));
                }
            }
            StrategyType::MovingAverage { short_window, long_window } => {
                if *short_window == 0 || *long_window == 0 {
                    return Err(TradingError::Validation(
                        "Window sizes must be greater than 0".to_string(),
                    ));
                }
                if short_window >= long_window {
                    return Err(TradingError::Validation(
                        "Short window must be less than long window".to_string(),
                    ));
                }
            }
        }

        Ok(())
    }

    pub fn get_risk_params(&self) -> RiskParams {
        self.risk_profile.params()
    }
}

use crate::config::strategy_config::{StrategyConfig, StrategyType};
use crate::error::Result;
use super::strategy::Strategy;
use super::mean_reversion::MeanReversionStrategy;

pub struct StrategyFactory;

impl StrategyFactory {
    pub fn create_strategy(config: &StrategyConfig) -> Result<Box<dyn Strategy>> {
        let risk_params = config.get_risk_params();
        
        match &config.strategy_type {
            StrategyType::MeanReversion { threshold, window_size } => {
                let strategy = MeanReversionStrategy::new(
                    *threshold,
                    *window_size,
                    risk_params.max_risk_per_trade,
                )?;
                Ok(Box::new(strategy))
            }
            StrategyType::MovingAverage { .. } => {
                Err(crate::error::TradingError::Validation(
                    "MovingAverage strategy not implemented yet".to_string(),
                ))
            }
        }
    }
}

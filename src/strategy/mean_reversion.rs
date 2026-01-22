use rust_decimal::Decimal;
use crate::market_data::event::PriceEvent;
use crate::error::{Result, TradingError};
use super::strategy::{Strategy, Signal};

/// Mean reversion strategy
/// Buys when price is below average, sells when above average
pub struct MeanReversionStrategy {
    name: String,
    threshold: Decimal,      // Deviation threshold (e.g., 0.02 for 2%)
    window_size: usize,      // Number of prices to track
    prices: Vec<Decimal>,
    risk_percentage: Decimal, // Risk per trade (e.g., 2%)
}

impl MeanReversionStrategy {
    pub fn new(threshold: Decimal, window_size: usize, risk_percentage: Decimal) -> Result<Self> {
        if threshold <= Decimal::ZERO || threshold >= Decimal::ONE {
            return Err(TradingError::Validation(
                "Threshold must be between 0 and 1".to_string(),
            ));
        }

        if window_size == 0 {
            return Err(TradingError::Validation(
                "Window size must be greater than 0".to_string(),
            ));
        }

        if risk_percentage <= Decimal::ZERO || risk_percentage > Decimal::from(100) {
            return Err(TradingError::Validation(
                "Risk percentage must be between 0 and 100".to_string(),
            ));
        }

        Ok(Self {
            name: "MeanReversion".to_string(),
            threshold,
            window_size,
            prices: Vec::with_capacity(window_size),
            risk_percentage,
        })
    }

    fn calculate_mean(&self) -> Option<Decimal> {
        if self.prices.is_empty() {
            return None;
        }

        let sum: Decimal = self.prices.iter().sum();
        Some(sum / Decimal::from(self.prices.len() as u32))
    }

    fn calculate_deviation(&self, current_price: Decimal, mean: Decimal) -> Decimal {
        (current_price - mean).abs() / mean
    }
}

impl Strategy for MeanReversionStrategy {
    fn signal(&self, event: &PriceEvent) -> Result<Signal> {
        if self.prices.is_empty() {
            // Not enough data yet
            return Ok(Signal::Hold);
        }

        let mean = match self.calculate_mean() {
            Some(m) => m,
            None => return Ok(Signal::Hold),
        };

        let deviation = self.calculate_deviation(event.price, mean);

        // Buy if price is below mean by threshold
        if event.price < mean && deviation > self.threshold {
            Ok(Signal::Buy)
        }
        // Sell if price is above mean by threshold
        else if event.price > mean && deviation > self.threshold {
            Ok(Signal::Sell)
        }
        // Hold otherwise
        else {
            Ok(Signal::Hold)
        }
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn get_risk_params(&self, current_price: Decimal) -> Result<(Decimal, Decimal, Decimal)> {
        // Entry price is current price
        let entry_price = current_price;

        // Stop loss is 2% below entry for simplicity
        let stop_loss_distance = entry_price * Decimal::from_str_exact("0.02")
            .map_err(|e| TradingError::Decimal(e))?;

        // Position size based on 2% risk
        let position_size = entry_price; // Simplified: 1 unit at current price

        Ok((entry_price, stop_loss_distance, position_size))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mean_reversion_creation() {
        let strategy = MeanReversionStrategy::new(
            Decimal::new(2, 2),  // 0.02 with scale 2
            10,
            Decimal::from(2),
        );
        assert!(strategy.is_ok());
    }

    #[test]
    fn test_invalid_threshold() {
        let strategy = MeanReversionStrategy::new(
            Decimal::from(2), // Invalid: > 1
            10,
            Decimal::from(2),
        );
        assert!(strategy.is_err());
    }
}

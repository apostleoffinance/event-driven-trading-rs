use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};
use rust_decimal::Decimal;
use crate::error::{TradingError, Result};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PriceEvent {
    pub symbol: String,
    pub price: Decimal,
    pub timestamp: u64,
    pub volume: Decimal,
}

impl PriceEvent {
    pub fn new(symbol: String, price: Decimal, volume: Decimal) -> Result<Self> {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|e| TradingError::Time(e.to_string()))?
            .as_millis() as u64;

        Ok(Self {
            symbol,
            price,
            timestamp,
            volume,
        })
    }
}

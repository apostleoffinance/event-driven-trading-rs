use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};
use rust_decimal::Decimal;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PriceEvent {
    pub symbol: String,
    pub price: Decimal,
    pub timestamp: u64,
    pub volume: Decimal,
}

impl PriceEvent {
    pub fn new(symbol: String, price: Decimal, volume: Decimal) -> Self {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;

        Self {
            symbol,
            price,
            timestamp,
            volume,
        }
    }
}

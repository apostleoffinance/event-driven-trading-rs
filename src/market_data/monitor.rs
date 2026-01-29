use std::collections::HashMap;
use rust_decimal::Decimal;
use crate::error::{Result, TradingError};
use super::event::PriceEvent;

#[derive(Debug)]
pub struct PriceMonitor {
    last_seen: HashMap<String, (u64, Decimal)>,
    gap_threshold_ms: u64,
}

impl PriceMonitor {
    pub fn new(gap_threshold_ms: u64) -> Self {
        Self {
            last_seen: HashMap::new(),
            gap_threshold_ms,
        }
    }

    /// Returns None for duplicate events; error for gaps.
    pub fn process(&mut self, event: PriceEvent) -> Result<Option<PriceEvent>> {
        if let Some((last_ts, last_price)) = self.last_seen.get(&event.symbol).cloned() {
            if event.timestamp <= last_ts && event.price == last_price {
                return Ok(None);
            }
            if event.timestamp > last_ts && event.timestamp - last_ts > self.gap_threshold_ms {
                return Err(TradingError::MarketData(format!(
                    "Price gap detected for {}: {}ms",
                    event.symbol,
                    event.timestamp - last_ts
                )));
            }
        }

        self.last_seen.insert(event.symbol.clone(), (event.timestamp, event.price));
        Ok(Some(event))
    }
}
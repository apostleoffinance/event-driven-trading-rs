//! Market data event payload used by the async event pipeline.
//!
//! Venue/source agnostic — strategies must not know if data came from
//! Binance, Bybit, a prop feed, REST, or WebSocket.

use chrono::{DateTime, Utc};
use domain::InstrumentId;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use crate::error::{EventsError, EventsResult};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MarketDataEvent {
    pub instrument_id: InstrumentId,
    pub price: Decimal,
    pub volume: Option<Decimal>,
    pub source: String,
    pub exchanged_at: DateTime<Utc>,
    pub received_at: DateTime<Utc>,
}

impl MarketDataEvent {
    pub fn new(
        instrument_id: InstrumentId,
        price: Decimal,
        source: impl Into<String>,
        exchanged_at: DateTime<Utc>,
        received_at: DateTime<Utc>,
    ) -> EventsResult<Self> {
        if price <= Decimal::ZERO {
            return Err(EventsError::Invalid(
                "market data price must be positive".to_string(),
            ));
        }
        let source = source.into();
        if source.trim().is_empty() {
            return Err(EventsError::Invalid(
                "market data source must be non-empty".to_string(),
            ));
        }
        Ok(Self {
            instrument_id,
            price,
            volume: None,
            source,
            exchanged_at,
            received_at,
        })
    }

    pub fn with_volume(mut self, volume: Decimal) -> EventsResult<Self> {
        if volume < Decimal::ZERO {
            return Err(EventsError::Invalid(
                "market data volume must be non-negative".to_string(),
            ));
        }
        self.volume = Some(volume);
        Ok(self)
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;
    use chrono::Utc;
    use domain::InstrumentId;
    use rust_decimal::Decimal;

    #[test]
    fn rejects_non_positive_price() {
        let err = MarketDataEvent::new(
            InstrumentId::new("BTCUSDT").unwrap(),
            Decimal::ZERO,
            "binance",
            Utc::now(),
            Utc::now(),
        )
        .unwrap_err();
        assert!(matches!(err, EventsError::Invalid(_)));
    }
}

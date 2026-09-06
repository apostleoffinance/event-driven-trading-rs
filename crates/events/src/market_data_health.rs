//! Market-data health: freshness and sequence-gap detection.
//!
//! A value being present does not mean the value is valid for trading.

use chrono::{DateTime, Duration, Utc};
use domain::InstrumentId;
use serde::{Deserialize, Serialize};

use crate::market_data::MarketDataEvent;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MarketDataHealthStatus {
    Healthy,
    Stale,
    SequenceGap,
    Missing,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MarketDataHealth {
    pub instrument_id: InstrumentId,
    pub status: MarketDataHealthStatus,
    pub last_sequence: Option<u64>,
    pub last_exchanged_at: Option<DateTime<Utc>>,
    pub last_received_at: Option<DateTime<Utc>>,
    pub detail: String,
}

impl MarketDataHealth {
    pub fn is_tradable(&self) -> bool {
        matches!(self.status, MarketDataHealthStatus::Healthy)
    }
}

/// Tracks per-instrument feed health for fail-closed strategy gating.
#[derive(Debug, Clone)]
pub struct MarketDataHealthTracker {
    max_age: Duration,
    last_by_instrument: std::collections::HashMap<InstrumentId, FeedCursor>,
}

#[derive(Debug, Clone)]
struct FeedCursor {
    sequence: Option<u64>,
    exchanged_at: DateTime<Utc>,
    received_at: DateTime<Utc>,
}

impl MarketDataHealthTracker {
    pub fn new(max_age_secs: i64) -> Self {
        Self {
            max_age: Duration::seconds(max_age_secs.max(1)),
            last_by_instrument: std::collections::HashMap::new(),
        }
    }

    /// Observe a tick; returns health for that instrument after update.
    pub fn observe(&mut self, event: &MarketDataEvent, now: DateTime<Utc>) -> MarketDataHealth {
        let mut status = MarketDataHealthStatus::Healthy;
        let mut detail = "ok".to_string();

        if let Some(prev) = self.last_by_instrument.get(&event.instrument_id) {
            if let (Some(prev_seq), Some(seq)) = (prev.sequence, event.sequence) {
                if seq > prev_seq + 1 {
                    status = MarketDataHealthStatus::SequenceGap;
                    detail = format!("sequence gap: last={prev_seq} got={seq}");
                } else if seq <= prev_seq {
                    status = MarketDataHealthStatus::SequenceGap;
                    detail = format!("non-monotonic sequence: last={prev_seq} got={seq}");
                }
            }
        }

        let age = now.signed_duration_since(event.received_at);
        if age > self.max_age {
            status = MarketDataHealthStatus::Stale;
            detail = format!(
                "stale: age_secs={} max={}",
                age.num_seconds(),
                self.max_age.num_seconds()
            );
        }

        self.last_by_instrument.insert(
            event.instrument_id.clone(),
            FeedCursor {
                sequence: event.sequence,
                exchanged_at: event.exchanged_at,
                received_at: event.received_at,
            },
        );

        MarketDataHealth {
            instrument_id: event.instrument_id.clone(),
            status,
            last_sequence: event.sequence,
            last_exchanged_at: Some(event.exchanged_at),
            last_received_at: Some(event.received_at),
            detail,
        }
    }

    pub fn health_of(
        &self,
        instrument_id: &InstrumentId,
        now: DateTime<Utc>,
    ) -> MarketDataHealth {
        match self.last_by_instrument.get(instrument_id) {
            None => MarketDataHealth {
                instrument_id: instrument_id.clone(),
                status: MarketDataHealthStatus::Missing,
                last_sequence: None,
                last_exchanged_at: None,
                last_received_at: None,
                detail: "no market data observed".into(),
            },
            Some(cursor) => {
                let age = now.signed_duration_since(cursor.received_at);
                if age > self.max_age {
                    MarketDataHealth {
                        instrument_id: instrument_id.clone(),
                        status: MarketDataHealthStatus::Stale,
                        last_sequence: cursor.sequence,
                        last_exchanged_at: Some(cursor.exchanged_at),
                        last_received_at: Some(cursor.received_at),
                        detail: format!("stale: age_secs={}", age.num_seconds()),
                    }
                } else {
                    MarketDataHealth {
                        instrument_id: instrument_id.clone(),
                        status: MarketDataHealthStatus::Healthy,
                        last_sequence: cursor.sequence,
                        last_exchanged_at: Some(cursor.exchanged_at),
                        last_received_at: Some(cursor.received_at),
                        detail: "ok".into(),
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;
    use domain::InstrumentId;
    use rust_decimal::Decimal;

    fn tick(seq: u64, received: DateTime<Utc>) -> MarketDataEvent {
        MarketDataEvent::new(
            InstrumentId::new("BTCUSDT").unwrap(),
            Decimal::from(100),
            "test",
            received,
            received,
        )
        .unwrap()
        .with_sequence(seq)
    }

    #[test]
    fn detects_sequence_gap() {
        let mut tracker = MarketDataHealthTracker::new(30);
        let t0 = Utc::now();
        let h1 = tracker.observe(&tick(1, t0), t0);
        assert!(h1.is_tradable());
        let h2 = tracker.observe(&tick(3, t0), t0);
        assert_eq!(h2.status, MarketDataHealthStatus::SequenceGap);
        assert!(!h2.is_tradable());
    }

    #[test]
    fn detects_stale() {
        let mut tracker = MarketDataHealthTracker::new(5);
        let old = Utc::now() - Duration::seconds(30);
        let now = Utc::now();
        let h = tracker.observe(&tick(1, old), now);
        assert_eq!(h.status, MarketDataHealthStatus::Stale);
    }
}

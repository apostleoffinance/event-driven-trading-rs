//! Synthetic provider. Every value is fabricated and always classified `demo`.
//!
//! Prices are a deterministic function of (instrument, timestamp), so the same
//! window always returns identical candles and overlapping windows agree.

use async_trait::async_trait;

use crate::error::MarketDataError;
use crate::instruments::Instrument;
use crate::model::{Candle, DataClass, ProviderId, ProviderStatus, Timeframe};
use crate::provider::{CandleProvider, ProviderRequest};

pub struct DemoProvider;

fn hash(seed: u64, ts: i64) -> u64 {
    let mut x = seed ^ (ts as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15);
    x ^= x >> 30;
    x = x.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x ^= x >> 27;
    x = x.wrapping_mul(0x94D0_49BB_1331_11EB);
    x ^ (x >> 31)
}

fn unit(h: u64) -> f64 {
    (h >> 11) as f64 / (1u64 << 53) as f64
}

fn seed_of(id: &str) -> u64 {
    id.bytes().fold(0xCBF2_9CE4_8422_2325u64, |h, b| {
        (h ^ u64::from(b)).wrapping_mul(0x100_0000_01B3)
    })
}

fn level(reference: f64, seed: u64, ts: i64) -> f64 {
    let t = ts as f64 / 3_600_000.0; // hours
    let phase = unit(hash(seed, 0)) * std::f64::consts::TAU;
    let wave = 0.012 * (t / 37.0 + phase).sin() + 0.006 * (t / 9.0 + phase * 2.0).sin();
    reference * (1.0 + wave)
}

pub fn generate(
    instrument: &Instrument,
    timeframe: Timeframe,
    from_ms: Option<i64>,
    to_ms: Option<i64>,
    limit: usize,
    now_ms: i64,
) -> Vec<Candle> {
    let step = timeframe.duration_ms();
    let newest_allowed = to_ms.map_or(now_ms, |to| to.min(now_ms));
    let newest = newest_allowed.div_euclid(step) * step;
    let seed = seed_of(instrument.id);
    let has_volume = instrument.volume_kind.is_some();
    let wick = 0.0015;

    let mut out = Vec::with_capacity(limit);
    for i in 0..limit as i64 {
        let ts = newest - i * step;
        if from_ms.is_some_and(|from| ts < from) {
            break;
        }
        let open = level(instrument.demo_reference_price, seed, ts);
        let close = level(instrument.demo_reference_price, seed, ts + step);
        let h = hash(seed, ts);
        let high = open.max(close) * (1.0 + wick * unit(h));
        let low = open.min(close) * (1.0 - wick * unit(h.rotate_left(21)));
        out.push(Candle {
            timestamp_ms: ts,
            open,
            high,
            low,
            close,
            volume: has_volume.then(|| (100.0 + 900.0 * unit(h.rotate_left(42))).round()),
        });
    }
    out.reverse();
    out
}

#[async_trait]
impl CandleProvider for DemoProvider {
    fn id(&self) -> ProviderId {
        ProviderId::Demo
    }

    fn name(&self) -> &'static str {
        "EdgeNest Demo (synthetic)"
    }

    fn classification(&self) -> DataClass {
        DataClass::Demo
    }

    fn status(&self) -> ProviderStatus {
        ProviderStatus::Demo
    }

    fn supports(&self, _: &Instrument, _: Timeframe) -> Result<(), MarketDataError> {
        Ok(())
    }

    async fn fetch_candles(&self, r: &ProviderRequest) -> Result<Vec<Candle>, MarketDataError> {
        Ok(generate(
            r.instrument,
            r.timeframe,
            r.from_ms,
            r.to_ms,
            r.limit,
            r.now_ms,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::instruments::find;
    use crate::validation::validate_series;

    const NOW: i64 = 1_800_000_000_000;

    #[test]
    fn is_deterministic_valid_and_ascending() {
        let eur = find("EURUSD").unwrap();
        let a = generate(eur, Timeframe::M15, None, None, 200, NOW);
        let b = generate(eur, Timeframe::M15, None, None, 200, NOW);
        assert_eq!(a, b);
        assert_eq!(a.len(), 200);
        validate_series(&a).unwrap();
    }

    #[test]
    fn overlapping_windows_agree() {
        let btc = find("BTCUSDT").unwrap();
        let long = generate(btc, Timeframe::H1, None, None, 100, NOW);
        let short = generate(btc, Timeframe::H1, None, None, 10, NOW);
        assert_eq!(&long[90..], &short[..]);
    }

    #[test]
    fn honours_from_and_to() {
        let btc = find("BTCUSDT").unwrap();
        let step = Timeframe::H1.duration_ms();
        let to = NOW - 10 * step;
        let from = to - 5 * step;
        let v = generate(btc, Timeframe::H1, Some(from), Some(to), 500, NOW);
        assert!(v.first().unwrap().timestamp_ms >= from);
        assert!(v.last().unwrap().timestamp_ms <= to);
        assert_eq!(v.len(), 6);
    }

    #[test]
    fn omits_volume_when_instrument_has_none() {
        let mut i = *find("BTCUSDT").unwrap();
        i.volume_kind = None;
        assert!(generate(&i, Timeframe::H1, None, None, 5, NOW)
            .iter()
            .all(|c| c.volume.is_none()));
    }
}

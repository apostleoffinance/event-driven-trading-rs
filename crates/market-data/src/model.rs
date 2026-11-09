//! Canonical market-data contract.
//!
//! Conventions (also documented in `docs/MARKET_DATA_SERVICE.md`):
//! - All timestamps are UTC. Machine fields are epoch **milliseconds**
//!   (`*_ms`); human-oriented fields are RFC 3339 strings.
//! - A candle's `timestamp_ms` is the candle **opening** time (bucket start).
//!   The newest candle may still be forming.
//! - Prices and volume are `f64`: this contract feeds charts only. It must not
//!   be used for position sizing, P&L or any accounting.
//! - Instrument identity (`BTCUSDT`, `EURUSD`, `BTCUSDT-PERP`) is separate from
//!   provider symbol aliases (see `instruments`).

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Timeframe {
    M1,
    M5,
    M15,
    M30,
    H1,
    H4,
    D1,
    W1,
}

impl Timeframe {
    pub const ALL: [Timeframe; 8] = [
        Timeframe::M1,
        Timeframe::M5,
        Timeframe::M15,
        Timeframe::M30,
        Timeframe::H1,
        Timeframe::H4,
        Timeframe::D1,
        Timeframe::W1,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Timeframe::M1 => "M1",
            Timeframe::M5 => "M5",
            Timeframe::M15 => "M15",
            Timeframe::M30 => "M30",
            Timeframe::H1 => "H1",
            Timeframe::H4 => "H4",
            Timeframe::D1 => "D1",
            Timeframe::W1 => "W1",
        }
    }

    pub fn duration_ms(self) -> i64 {
        const MIN: i64 = 60_000;
        match self {
            Timeframe::M1 => MIN,
            Timeframe::M5 => 5 * MIN,
            Timeframe::M15 => 15 * MIN,
            Timeframe::M30 => 30 * MIN,
            Timeframe::H1 => 60 * MIN,
            Timeframe::H4 => 240 * MIN,
            Timeframe::D1 => 1_440 * MIN,
            Timeframe::W1 => 10_080 * MIN,
        }
    }
}

impl fmt::Display for Timeframe {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for Timeframe {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Timeframe::ALL
            .into_iter()
            .find(|tf| tf.as_str().eq_ignore_ascii_case(s.trim()))
            .ok_or_else(|| {
                format!(
                    "unknown timeframe '{s}'; expected one of {}",
                    Timeframe::ALL.map(Timeframe::as_str).join(", ")
                )
            })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AssetClass {
    Forex,
    CryptoSpot,
    CryptoPerpetual,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ProviderId {
    #[serde(rename = "demo")]
    Demo,
    #[serde(rename = "binance-public")]
    BinancePublic,
    #[serde(rename = "mt5-bridge")]
    Mt5Bridge,
    #[serde(rename = "biquote-live")]
    BiQuote,
}

impl ProviderId {
    pub fn as_str(self) -> &'static str {
        match self {
            ProviderId::Demo => "demo",
            ProviderId::BinancePublic => "binance-public",
            ProviderId::Mt5Bridge => "mt5-bridge",
            ProviderId::BiQuote => "biquote-live",
        }
    }
}

impl fmt::Display for ProviderId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for ProviderId {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        [
            ProviderId::Demo,
            ProviderId::BinancePublic,
            ProviderId::Mt5Bridge,
            ProviderId::BiQuote,
        ]
        .into_iter()
        .find(|p| p.as_str().eq_ignore_ascii_case(s.trim()))
        .ok_or_else(|| {
            format!(
                "unknown provider '{s}'; expected demo, binance-public, mt5-bridge or biquote-live"
            )
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VolumeKind {
    /// MT5-style tick count; not traded volume.
    Tick,
    /// Traded volume in the base asset.
    Base,
}

/// What the data in a response is. `Unavailable` is only ever used in error
/// bodies; a successful response is `Demo` or `Live`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DataClass {
    Demo,
    Live,
    Unavailable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Freshness {
    /// Synthetic data: freshness is not meaningful.
    Demo,
    /// Newest candle is the current or previous bucket.
    Fresh,
    /// Newest candle is older than two buckets. May simply mean the market is
    /// closed; the service cannot tell.
    Stale,
    /// A past window was requested explicitly via `to`.
    Historical,
    /// No candles returned, so nothing to judge.
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Candle {
    /// UTC candle opening time, epoch milliseconds.
    pub timestamp_ms: i64,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub volume: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum ProviderStatus {
    /// Synthetic provider, always answers.
    Demo,
    /// Enabled and expected to answer. Says nothing about the last request.
    Available,
    /// Switched off by configuration.
    Disabled { reason: String },
    /// The capability does not exist yet (e.g. MT5 worker has no candle command).
    NotImplemented { reason: String },
    /// Enabled but known to be failing.
    Unavailable { reason: String },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timeframe_round_trips_and_is_case_insensitive() {
        for tf in Timeframe::ALL {
            assert_eq!(tf.as_str().parse::<Timeframe>().unwrap(), tf);
        }
        assert_eq!("m15".parse::<Timeframe>().unwrap(), Timeframe::M15);
        assert!("M2".parse::<Timeframe>().is_err());
        assert!("".parse::<Timeframe>().is_err());
    }

    #[test]
    fn timeframe_durations_are_ordered() {
        let d: Vec<i64> = Timeframe::ALL.iter().map(|t| t.duration_ms()).collect();
        assert!(d.windows(2).all(|w| w[0] < w[1]));
        assert_eq!(Timeframe::H4.duration_ms(), 4 * Timeframe::H1.duration_ms());
    }

    #[test]
    fn provider_ids_parse() {
        assert_eq!("demo".parse::<ProviderId>().unwrap(), ProviderId::Demo);
        assert_eq!(
            "Binance-Public".parse::<ProviderId>().unwrap(),
            ProviderId::BinancePublic
        );
        assert!("binance".parse::<ProviderId>().is_err());
    }

    #[test]
    fn candle_omits_absent_volume() {
        let c = Candle {
            timestamp_ms: 1,
            open: 1.0,
            high: 2.0,
            low: 0.5,
            close: 1.5,
            volume: None,
        };
        assert!(!serde_json::to_string(&c).unwrap().contains("volume"));
    }
}

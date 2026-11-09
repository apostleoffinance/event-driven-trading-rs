//! BiQuote public live market-data provider for FX, metals and crypto candles.
//! Read-only market analysis data; never used for order execution or accounting.

use async_trait::async_trait;
use chrono::{DateTime, SecondsFormat, Utc};
use serde_json::Value;
use std::time::Duration;

use super::binance::{HttpError, HttpGet};
use crate::error::MarketDataError;
use crate::instruments::Instrument;
use crate::model::{Candle, DataClass, ProviderId, ProviderStatus, Timeframe};
use crate::provider::{CandleProvider, ProviderRequest};

const ID: ProviderId = ProviderId::BiQuote;

#[derive(Debug, Clone)]
pub struct BiQuoteConfig {
    pub enabled: bool,
    pub base_url: String,
    pub timeout: Duration,
}

impl Default for BiQuoteConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            base_url: "https://biquote.io".into(),
            timeout: Duration::from_secs(8),
        }
    }
}

pub struct BiQuoteProvider<H: HttpGet> {
    config: BiQuoteConfig,
    http: H,
}

impl<H: HttpGet> BiQuoteProvider<H> {
    pub fn new(config: BiQuoteConfig, http: H) -> Self {
        Self { config, http }
    }

    fn interval(tf: Timeframe) -> &'static str {
        match tf {
            Timeframe::M1 => "1m",
            Timeframe::M5 => "5m",
            Timeframe::M15 => "15m",
            Timeframe::M30 => "30m",
            Timeframe::H1 => "1h",
            Timeframe::H4 => "4h",
            Timeframe::D1 => "1d",
            Timeframe::W1 => "1d",
        }
    }

    fn url(&self, symbol: &str, req: &ProviderRequest) -> String {
        let request_limit = if req.timeframe == Timeframe::W1 {
            req.limit.saturating_mul(7).min(1000)
        } else {
            req.limit.min(1000)
        };
        let mut url = format!(
            "{}/api/{}/ohlc?interval={}&limit={}",
            self.config.base_url.trim_end_matches('/'),
            symbol,
            Self::interval(req.timeframe),
            request_limit
        );
        if let Some(ms) = req.from_ms {
            if let Some(dt) = DateTime::<Utc>::from_timestamp_millis(ms) {
                url.push_str("&from=");
                url.push_str(&dt.to_rfc3339_opts(SecondsFormat::Millis, true));
            }
        }
        if let Some(ms) = req.to_ms {
            if let Some(dt) = DateTime::<Utc>::from_timestamp_millis(ms) {
                url.push_str("&to=");
                url.push_str(&dt.to_rfc3339_opts(SecondsFormat::Millis, true));
            }
        }
        url
    }
}

fn invalid(reason: impl Into<String>) -> MarketDataError {
    MarketDataError::InvalidProviderData {
        provider: ID,
        reason: reason.into(),
    }
}

fn parse_num(v: &Value, field: &str) -> Result<f64, MarketDataError> {
    let n = v
        .as_f64()
        .or_else(|| v.as_str().and_then(|s| s.parse().ok()))
        .filter(|n: &f64| n.is_finite())
        .ok_or_else(|| invalid(format!("BiQuote candle {field} is not numeric")))?;
    Ok(n)
}

fn parse_time(v: &Value) -> Result<i64, MarketDataError> {
    let raw = v
        .as_str()
        .ok_or_else(|| invalid("BiQuote candle openTime is missing"))?;
    DateTime::parse_from_rfc3339(raw)
        .map(|d| d.timestamp_millis())
        .map_err(|_| invalid("BiQuote candle openTime is not RFC3339"))
}

fn parse_bars(body: &str) -> Result<Vec<Candle>, MarketDataError> {
    let json: Value =
        serde_json::from_str(body).map_err(|_| invalid("BiQuote response is not JSON"))?;
    let bars = json
        .get("bars")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid("BiQuote response has no bars array"))?;
    bars.iter()
        .map(|bar| {
            Ok(Candle {
                timestamp_ms: parse_time(&bar["openTime"])?,
                open: parse_num(&bar["open"], "open")?,
                high: parse_num(&bar["high"], "high")?,
                low: parse_num(&bar["low"], "low")?,
                close: parse_num(&bar["close"], "close")?,
                volume: parse_num(&bar["tickVolume"], "tickVolume")
                    .ok()
                    .or_else(|| parse_num(&bar["volume"], "volume").ok()),
            })
        })
        .collect()
}

fn aggregate_weekly(candles: Vec<Candle>) -> Vec<Candle> {
    use chrono::Datelike;
    let mut weeks: std::collections::BTreeMap<i64, Candle> = std::collections::BTreeMap::new();
    for candle in candles {
        let Some(dt) = DateTime::<Utc>::from_timestamp_millis(candle.timestamp_ms) else {
            continue;
        };
        let monday = dt.date_naive()
            - chrono::Duration::days(i64::from(dt.weekday().num_days_from_monday()));
        let Some(start) = monday
            .and_hms_opt(0, 0, 0)
            .map(|d| d.and_utc().timestamp_millis())
        else {
            continue;
        };
        weeks
            .entry(start)
            .and_modify(|week| {
                week.high = week.high.max(candle.high);
                week.low = week.low.min(candle.low);
                week.close = candle.close;
                week.volume = Some(week.volume.unwrap_or(0.0) + candle.volume.unwrap_or(0.0));
            })
            .or_insert(Candle {
                timestamp_ms: start,
                ..candle
            });
    }
    weeks.into_values().collect()
}

#[async_trait]
impl<H: HttpGet> CandleProvider for BiQuoteProvider<H> {
    fn id(&self) -> ProviderId {
        ID
    }

    fn name(&self) -> &'static str {
        "BiQuote Live (external feed)"
    }

    fn classification(&self) -> DataClass {
        DataClass::Live
    }

    fn status(&self) -> ProviderStatus {
        if self.config.enabled {
            ProviderStatus::Available
        } else {
            ProviderStatus::Disabled {
                reason: "BiQuote live data is disabled by configuration".into(),
            }
        }
    }

    fn supports(&self, instrument: &Instrument, _: Timeframe) -> Result<(), MarketDataError> {
        if instrument.alias(ID).is_some() {
            Ok(())
        } else {
            Err(MarketDataError::Unsupported {
                provider: ID,
                reason: format!("{} is not mapped to BiQuote", instrument.id),
            })
        }
    }

    async fn fetch_candles(&self, req: &ProviderRequest) -> Result<Vec<Candle>, MarketDataError> {
        if !self.config.enabled {
            return Err(MarketDataError::ProviderDisabled {
                provider: ID,
                reason: "BiQuote is disabled".into(),
            });
        }
        let symbol = req
            .instrument
            .alias(ID)
            .ok_or_else(|| MarketDataError::Unsupported {
                provider: ID,
                reason: "instrument has no BiQuote symbol".into(),
            })?;
        let response = self
            .http
            .get(&self.url(symbol, req), self.config.timeout)
            .await
            .map_err(|e| match e {
                HttpError::Timeout => MarketDataError::Timeout { provider: ID },
                HttpError::Transport(_) => MarketDataError::ProviderUnavailable {
                    provider: ID,
                    reason: "BiQuote connection failed".into(),
                },
            })?;
        match response.status {
            200 => {
                let candles = parse_bars(&response.body)?;
                Ok(if req.timeframe == Timeframe::W1 {
                    aggregate_weekly(candles)
                } else {
                    candles
                })
            }
            429 => Err(MarketDataError::RateLimited {
                provider: ID,
                retry_after_seconds: response.retry_after_seconds,
            }),
            404 => Err(MarketDataError::Unsupported {
                provider: ID,
                reason: "BiQuote has no candle history for this symbol".into(),
            }),
            status => Err(MarketDataError::Upstream {
                provider: ID,
                status: Some(status),
                message: "BiQuote request failed".into(),
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_candles_with_utc_open_times_and_tick_volume() {
        let body = r#"{"symbol":"EURUSD","interval":"1h","bars":[{"openTime":"2026-11-09T10:00:00Z","open":1.1,"high":1.2,"low":1.0,"close":1.15,"volume":0,"tickVolume":25,"isOpen":true}]}"#;
        let bars = parse_bars(body).unwrap();
        assert_eq!(bars.len(), 1);
        assert_eq!(bars[0].timestamp_ms, 1794218400000);
        assert_eq!(bars[0].volume, Some(25.0));
        assert_eq!(bars[0].high, 1.2);
    }

    #[test]
    fn weekly_interval_uses_daily_bars_for_aggregation() {
        assert_eq!(
            BiQuoteProvider::<super::super::binance::ReqwestHttp>::interval(Timeframe::W1),
            "1d"
        );
    }
}

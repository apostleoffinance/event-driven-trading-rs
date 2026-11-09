use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use chrono::{DateTime, SecondsFormat, Utc};
use serde::{Deserialize, Serialize};

use crate::error::MarketDataError;
use crate::instruments::{self, InstrumentMeta, INSTRUMENTS};
use crate::model::{Candle, DataClass, Freshness, ProviderId, ProviderStatus, Timeframe};
use crate::provider::{CandleProvider, ProviderRequest};
use crate::validation::{sort_ascending, validate_series};

pub trait Clock: Send + Sync {
    fn now_ms(&self) -> i64;
}

pub struct SystemClock;

impl Clock for SystemClock {
    fn now_ms(&self) -> i64 {
        Utc::now().timestamp_millis()
    }
}

#[derive(Debug, Clone)]
pub struct ServiceConfig {
    /// Hard upper bound for one provider call, independent of the provider's
    /// own transport timeout.
    pub request_timeout: Duration,
    pub default_limit: usize,
    pub max_limit: usize,
}

impl Default for ServiceConfig {
    fn default() -> Self {
        Self {
            request_timeout: Duration::from_secs(8),
            default_limit: 300,
            max_limit: 1000,
        }
    }
}

/// Raw, unvalidated query parameters exactly as received.
#[derive(Debug, Default, Clone, Deserialize)]
pub struct CandleQuery {
    pub instrument: Option<String>,
    pub timeframe: Option<String>,
    pub from: Option<String>,
    pub to: Option<String>,
    pub limit: Option<String>,
    pub provider: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ProviderInfo {
    pub id: ProviderId,
    pub name: &'static str,
    pub status: ProviderStatus,
    /// Informational: the provider's own symbol for this instrument.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider_symbol: Option<&'static str>,
}

#[derive(Debug, Serialize)]
pub struct CandleResponse {
    pub instrument: InstrumentMeta,
    pub provider: ProviderInfo,
    pub timeframe: Timeframe,
    /// `demo` or `live`. Errors are `unavailable` and carry no candles.
    pub classification: DataClass,
    pub freshness: Freshness,
    pub count: usize,
    /// Open time of the newest candle (which may still be forming).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_candle_timestamp_ms: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_candle_at: Option<String>,
    /// When the service produced this response (UTC, RFC 3339).
    pub last_updated_at: String,
    pub generated_at_ms: i64,
    /// Oldest first; `timestamp_ms` is the candle opening time (UTC).
    pub candles: Vec<Candle>,
}

#[derive(Debug, Serialize)]
pub struct ProviderListing {
    pub id: ProviderId,
    pub name: &'static str,
    pub classification: DataClass,
    pub status: ProviderStatus,
}

pub struct MarketDataService {
    providers: HashMap<ProviderId, Arc<dyn CandleProvider>>,
    config: ServiceConfig,
    clock: Arc<dyn Clock>,
}

pub fn rfc3339(ms: i64) -> String {
    DateTime::<Utc>::from_timestamp_millis(ms)
        .map(|d| d.to_rfc3339_opts(SecondsFormat::Millis, true))
        .unwrap_or_default()
}

fn parse_time(field: &str, raw: &str) -> Result<i64, MarketDataError> {
    let raw = raw.trim();
    if !raw.is_empty() && raw.bytes().all(|b| b.is_ascii_digit()) {
        return raw
            .parse::<i64>()
            .map_err(|_| MarketDataError::InvalidRequest(format!("'{field}' is out of range")));
    }
    DateTime::parse_from_rfc3339(raw)
        .map(|d| d.timestamp_millis())
        .map_err(|_| {
            MarketDataError::InvalidRequest(format!(
                "'{field}' must be epoch milliseconds or an RFC 3339 UTC timestamp"
            ))
        })
}

struct Parsed {
    instrument: &'static instruments::Instrument,
    timeframe: Timeframe,
    from_ms: Option<i64>,
    to_ms: Option<i64>,
    limit: usize,
    provider: ProviderId,
}

impl MarketDataService {
    pub fn new(
        providers: Vec<Arc<dyn CandleProvider>>,
        config: ServiceConfig,
        clock: Arc<dyn Clock>,
    ) -> Self {
        Self {
            providers: providers.into_iter().map(|p| (p.id(), p)).collect(),
            config,
            clock,
        }
    }

    fn parse(&self, q: &CandleQuery) -> Result<Parsed, MarketDataError> {
        let id = q
            .instrument
            .as_deref()
            .filter(|s| !s.trim().is_empty())
            .ok_or_else(|| MarketDataError::InvalidRequest("'instrument' is required".into()))?;
        let instrument = instruments::find(id)
            .ok_or_else(|| MarketDataError::UnknownInstrument(id.trim().to_string()))?;

        let timeframe = q
            .timeframe
            .as_deref()
            .ok_or_else(|| MarketDataError::InvalidRequest("'timeframe' is required".into()))?
            .parse::<Timeframe>()
            .map_err(MarketDataError::InvalidRequest)?;

        let limit = match q.limit.as_deref() {
            None => self.config.default_limit,
            Some(raw) => raw
                .trim()
                .parse::<usize>()
                .ok()
                .filter(|n| (1..=self.config.max_limit).contains(n))
                .ok_or_else(|| {
                    MarketDataError::InvalidRequest(format!(
                        "'limit' must be an integer between 1 and {}",
                        self.config.max_limit
                    ))
                })?,
        };

        let from_ms = q
            .from
            .as_deref()
            .map(|v| parse_time("from", v))
            .transpose()?;
        let to_ms = q.to.as_deref().map(|v| parse_time("to", v)).transpose()?;
        if let (Some(from), Some(to)) = (from_ms, to_ms) {
            if from >= to {
                return Err(MarketDataError::InvalidRequest(
                    "'from' must be before 'to'".into(),
                ));
            }
        }

        // Demo data is never a default: it has to be asked for by name.
        let provider = match q.provider.as_deref() {
            Some(raw) => raw
                .parse::<ProviderId>()
                .map_err(MarketDataError::InvalidRequest)?,
            None => instrument.default_provider,
        };

        Ok(Parsed {
            instrument,
            timeframe,
            from_ms,
            to_ms,
            limit,
            provider,
        })
    }

    pub async fn get_candles(
        &self,
        query: &CandleQuery,
    ) -> Result<CandleResponse, MarketDataError> {
        let p = self.parse(query)?;
        let provider = self
            .providers
            .get(&p.provider)
            .ok_or_else(|| {
                MarketDataError::InvalidRequest(format!(
                    "provider '{}' is not registered",
                    p.provider
                ))
            })?
            .clone();

        match provider.status() {
            ProviderStatus::Disabled { reason } => {
                return Err(MarketDataError::ProviderDisabled {
                    provider: p.provider,
                    reason,
                })
            }
            ProviderStatus::NotImplemented { reason } => {
                return Err(MarketDataError::NotImplemented {
                    provider: p.provider,
                    reason,
                })
            }
            ProviderStatus::Unavailable { reason } => {
                return Err(MarketDataError::ProviderUnavailable {
                    provider: p.provider,
                    reason,
                })
            }
            ProviderStatus::Demo | ProviderStatus::Available => {}
        }
        provider.supports(p.instrument, p.timeframe)?;

        let now_ms = self.clock.now_ms();
        let request = ProviderRequest {
            instrument: p.instrument,
            timeframe: p.timeframe,
            from_ms: p.from_ms,
            to_ms: p.to_ms,
            limit: p.limit,
            now_ms,
        };

        let mut candles = tokio::time::timeout(
            self.config.request_timeout,
            provider.fetch_candles(&request),
        )
        .await
        .map_err(|_| MarketDataError::Timeout {
            provider: p.provider,
        })??;

        // Normalise: oldest first, confined to the requested window and limit.
        sort_ascending(&mut candles);
        validate_series(&candles).map_err(|e| MarketDataError::InvalidProviderData {
            provider: p.provider,
            reason: e.to_string(),
        })?;
        candles.retain(|c| {
            p.from_ms.is_none_or(|f| c.timestamp_ms >= f)
                && p.to_ms.is_none_or(|t| c.timestamp_ms <= t)
        });
        if candles.len() > p.limit {
            candles.drain(..candles.len() - p.limit);
        }

        let classification = provider.classification();
        let last = candles.last().map(|c| c.timestamp_ms);
        let freshness = Self::freshness(classification, last, p.timeframe, p.to_ms, now_ms);

        Ok(CandleResponse {
            instrument: p.instrument.meta(),
            provider: ProviderInfo {
                id: p.provider,
                name: provider.name(),
                status: provider.status(),
                provider_symbol: provider.symbol_for(p.instrument),
            },
            timeframe: p.timeframe,
            classification,
            freshness,
            count: candles.len(),
            last_candle_timestamp_ms: last,
            last_candle_at: last.map(rfc3339),
            last_updated_at: rfc3339(now_ms),
            generated_at_ms: now_ms,
            candles,
        })
    }

    fn freshness(
        class: DataClass,
        last: Option<i64>,
        tf: Timeframe,
        to_ms: Option<i64>,
        now_ms: i64,
    ) -> Freshness {
        if class == DataClass::Demo {
            return Freshness::Demo;
        }
        let Some(last) = last else {
            return Freshness::Unknown;
        };
        let step = tf.duration_ms();
        if to_ms.is_some_and(|to| now_ms - to > step) {
            return Freshness::Historical;
        }
        if now_ms - last <= 2 * step {
            Freshness::Fresh
        } else {
            Freshness::Stale
        }
    }

    pub fn instruments(&self) -> Vec<InstrumentMeta> {
        INSTRUMENTS.iter().map(|i| i.meta()).collect()
    }

    pub fn providers(&self) -> Vec<ProviderListing> {
        let mut list: Vec<_> = self
            .providers
            .values()
            .map(|p| ProviderListing {
                id: p.id(),
                name: p.name(),
                classification: p.classification(),
                status: p.status(),
            })
            .collect();
        list.sort_by_key(|p| p.id.as_str());
        list
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::providers::binance::{
        BinanceConfig, BinanceProvider, HttpError, HttpGet, HttpResponse,
    };
    use crate::providers::biquote::{BiQuoteConfig, BiQuoteProvider};
    use crate::providers::demo::DemoProvider;
    use crate::providers::mt5::Mt5Provider;
    use async_trait::async_trait;

    const NOW: i64 = 1_800_000_000_000;
    const H: i64 = 3_600_000;

    struct FixedClock;
    impl Clock for FixedClock {
        fn now_ms(&self) -> i64 {
            NOW
        }
    }

    fn candle(ts: i64) -> Candle {
        Candle {
            timestamp_ms: ts,
            open: 10.0,
            high: 11.0,
            low: 9.0,
            close: 10.5,
            volume: Some(1.0),
        }
    }

    struct Scripted {
        id: ProviderId,
        class: DataClass,
        result: Result<Vec<Candle>, MarketDataError>,
        delay: Duration,
    }

    #[async_trait]
    impl CandleProvider for Scripted {
        fn id(&self) -> ProviderId {
            self.id
        }
        fn name(&self) -> &'static str {
            "scripted"
        }
        fn classification(&self) -> DataClass {
            self.class
        }
        fn status(&self) -> ProviderStatus {
            ProviderStatus::Available
        }
        fn supports(
            &self,
            _: &instruments::Instrument,
            _: Timeframe,
        ) -> Result<(), MarketDataError> {
            Ok(())
        }
        async fn fetch_candles(&self, _: &ProviderRequest) -> Result<Vec<Candle>, MarketDataError> {
            tokio::time::sleep(self.delay).await;
            self.result.clone()
        }
    }

    fn service_with(p: Arc<dyn CandleProvider>) -> MarketDataService {
        MarketDataService::new(vec![p], ServiceConfig::default(), Arc::new(FixedClock))
    }

    fn scripted(result: Result<Vec<Candle>, MarketDataError>) -> Arc<dyn CandleProvider> {
        Arc::new(Scripted {
            id: ProviderId::BinancePublic,
            class: DataClass::Live,
            result,
            delay: Duration::ZERO,
        })
    }

    fn q(instrument: &str, tf: &str) -> CandleQuery {
        CandleQuery {
            instrument: Some(instrument.into()),
            timeframe: Some(tf.into()),
            ..Default::default()
        }
    }

    fn default_service() -> MarketDataService {
        MarketDataService::new(
            vec![
                Arc::new(DemoProvider),
                Arc::new(Mt5Provider),
                Arc::new(BinanceProvider::new(BinanceConfig::default(), NeverCalled)),
                Arc::new(BiQuoteProvider::new(
                    BiQuoteConfig {
                        enabled: false,
                        ..Default::default()
                    },
                    NeverCalled,
                )),
            ],
            ServiceConfig::default(),
            Arc::new(FixedClock),
        )
    }

    struct NeverCalled;
    #[async_trait]
    impl HttpGet for NeverCalled {
        async fn get(&self, _: &str, _: Duration) -> Result<HttpResponse, HttpError> {
            panic!("no HTTP request may be made");
        }
    }

    #[tokio::test]
    async fn demo_must_be_requested_explicitly_and_is_classified_demo() {
        let svc = default_service();
        let mut query = q("BTCUSDT", "H1");
        query.provider = Some("demo".into());
        let r = svc.get_candles(&query).await.unwrap();
        assert_eq!(r.classification, DataClass::Demo);
        assert_eq!(r.freshness, Freshness::Demo);
        assert_eq!(r.provider.id, ProviderId::Demo);
        assert_eq!(r.count, 300);
        assert_eq!(r.instrument.id, "BTCUSDT");
    }

    #[tokio::test]
    async fn default_provider_never_falls_back_to_demo() {
        let svc = default_service();
        // Crypto defaults to Binance, which is disabled: error, not demo data.
        let err = svc.get_candles(&q("BTCUSDT", "H1")).await.unwrap_err();
        assert_eq!(err.code(), "provider_disabled");
        assert_eq!(err.body().error.classification, DataClass::Unavailable);
        // Forex defaults to BiQuote. If disabled, it errors rather than falling back to demo data.
        let err = svc.get_candles(&q("EURUSD", "H1")).await.unwrap_err();
        assert_eq!(err.code(), "provider_disabled");
    }

    #[tokio::test]
    async fn provider_selection_and_registration() {
        let svc = service_with(scripted(Ok(vec![candle(NOW - H)])));
        let mut query = q("BTCUSDT", "H1");
        assert_eq!(
            svc.get_candles(&query).await.unwrap().provider.id,
            ProviderId::BinancePublic
        );
        query.provider = Some("mt5-bridge".into());
        assert_eq!(
            svc.get_candles(&query).await.unwrap_err().code(),
            "invalid_request"
        );
        query.provider = Some("kraken".into());
        assert_eq!(
            svc.get_candles(&query).await.unwrap_err().code(),
            "invalid_request"
        );
    }

    #[tokio::test]
    async fn validates_request_parameters() {
        let svc = default_service();
        let bad: Vec<(CandleQuery, &str)> = vec![
            (CandleQuery::default(), "invalid_request"),
            (
                CandleQuery {
                    instrument: Some("BTCUSDT".into()),
                    ..Default::default()
                },
                "invalid_request",
            ),
            (q("BTCUSDT", "M2"), "invalid_request"),
            (q("NOPE", "H1"), "unknown_instrument"),
            (
                CandleQuery {
                    limit: Some("0".into()),
                    ..q("BTCUSDT", "H1")
                },
                "invalid_request",
            ),
            (
                CandleQuery {
                    limit: Some("1001".into()),
                    ..q("BTCUSDT", "H1")
                },
                "invalid_request",
            ),
            (
                CandleQuery {
                    limit: Some("abc".into()),
                    ..q("BTCUSDT", "H1")
                },
                "invalid_request",
            ),
            (
                CandleQuery {
                    limit: Some("-5".into()),
                    ..q("BTCUSDT", "H1")
                },
                "invalid_request",
            ),
            (
                CandleQuery {
                    from: Some("yesterday".into()),
                    ..q("BTCUSDT", "H1")
                },
                "invalid_request",
            ),
            (
                CandleQuery {
                    from: Some("200".into()),
                    to: Some("100".into()),
                    ..q("BTCUSDT", "H1")
                },
                "invalid_request",
            ),
            (
                CandleQuery {
                    from: Some("100".into()),
                    to: Some("100".into()),
                    ..q("BTCUSDT", "H1")
                },
                "invalid_request",
            ),
        ];
        for (query, code) in bad {
            assert_eq!(
                svc.get_candles(&query).await.unwrap_err().code(),
                code,
                "{query:?}"
            );
        }
    }

    #[tokio::test]
    async fn accepts_rfc3339_and_epoch_windows() {
        let svc = default_service();
        let mut query = q("BTCUSDT", "H1");
        query.provider = Some("demo".into());
        query.from = Some("2027-01-01T00:00:00Z".into());
        query.to = Some("1798761600000".into()); // 2027-01-01T00:00:00Z... exclusive check below
        assert_eq!(
            svc.get_candles(&query).await.unwrap_err().code(),
            "invalid_request"
        );
        query.from = Some("2026-12-31T00:00:00Z".into());
        let r = svc.get_candles(&query).await.unwrap();
        assert!(r.count > 0 && r.count <= 25);
        assert_eq!(r.freshness, Freshness::Demo);
    }

    #[tokio::test]
    async fn unsupported_instrument_for_provider_is_rejected() {
        let svc = default_service();
        let mut query = q("BTCUSDT-PERP", "H1");
        query.provider = Some("binance-public".into());
        // Disabled is reported first; enable-path mapping is covered in binance tests.
        assert_eq!(
            svc.get_candles(&query).await.unwrap_err().code(),
            "provider_disabled"
        );
    }

    #[tokio::test]
    async fn provider_timeout_is_reported() {
        let slow = Arc::new(Scripted {
            id: ProviderId::BinancePublic,
            class: DataClass::Live,
            result: Ok(vec![]),
            delay: Duration::from_secs(5),
        });
        let svc = MarketDataService::new(
            vec![slow],
            ServiceConfig {
                request_timeout: Duration::from_millis(50),
                ..Default::default()
            },
            Arc::new(FixedClock),
        );
        let err = svc.get_candles(&q("BTCUSDT", "H1")).await.unwrap_err();
        assert_eq!(err.code(), "provider_timeout");
        assert_eq!(err.http_status(), 504);
    }

    #[tokio::test]
    async fn provider_errors_pass_through_unchanged() {
        let e = MarketDataError::RateLimited {
            provider: ProviderId::BinancePublic,
            retry_after_seconds: Some(3),
        };
        let svc = service_with(scripted(Err(e.clone())));
        assert_eq!(svc.get_candles(&q("BTCUSDT", "H1")).await.unwrap_err(), e);
    }

    #[tokio::test]
    async fn invalid_provider_candles_are_rejected_not_repaired() {
        let mut bad = candle(NOW - H);
        bad.high = 5.0; // high < low
        let svc = service_with(scripted(Ok(vec![bad])));
        assert_eq!(
            svc.get_candles(&q("BTCUSDT", "H1"))
                .await
                .unwrap_err()
                .code(),
            "invalid_provider_data"
        );

        let dup = service_with(scripted(Ok(vec![candle(NOW - H), candle(NOW - H)])));
        assert_eq!(
            dup.get_candles(&q("BTCUSDT", "H1"))
                .await
                .unwrap_err()
                .code(),
            "invalid_provider_data"
        );
    }

    #[tokio::test]
    async fn sorts_unordered_provider_output() {
        let svc = service_with(scripted(Ok(vec![
            candle(NOW - H),
            candle(NOW - 3 * H),
            candle(NOW - 2 * H),
        ])));
        let r = svc.get_candles(&q("BTCUSDT", "H1")).await.unwrap();
        let ts: Vec<_> = r.candles.iter().map(|c| c.timestamp_ms).collect();
        assert_eq!(ts, [NOW - 3 * H, NOW - 2 * H, NOW - H]);
        assert_eq!(r.last_candle_timestamp_ms, Some(NOW - H));
    }

    #[tokio::test]
    async fn empty_result_is_a_valid_unknown_response() {
        let svc = service_with(scripted(Ok(vec![])));
        let r = svc.get_candles(&q("BTCUSDT", "H1")).await.unwrap();
        assert_eq!(
            (r.count, r.freshness, r.classification),
            (0, Freshness::Unknown, DataClass::Live)
        );
        assert!(r.last_candle_at.is_none());
    }

    #[tokio::test]
    async fn live_freshness_levels() {
        let fresh = service_with(scripted(Ok(vec![candle(NOW - H)])));
        assert_eq!(
            fresh
                .get_candles(&q("BTCUSDT", "H1"))
                .await
                .unwrap()
                .freshness,
            Freshness::Fresh
        );
        let stale = service_with(scripted(Ok(vec![candle(NOW - 5 * H)])));
        assert_eq!(
            stale
                .get_candles(&q("BTCUSDT", "H1"))
                .await
                .unwrap()
                .freshness,
            Freshness::Stale
        );
        let hist = service_with(scripted(Ok(vec![candle(NOW - 50 * H)])));
        let query = CandleQuery {
            to: Some((NOW - 40 * H).to_string()),
            ..q("BTCUSDT", "H1")
        };
        assert_eq!(
            hist.get_candles(&query).await.unwrap().freshness,
            Freshness::Historical
        );
    }

    #[tokio::test]
    async fn trims_to_limit_keeping_newest() {
        let v: Vec<_> = (1..=10).map(|i| candle(NOW - i * H)).collect();
        let svc = service_with(scripted(Ok(v)));
        let query = CandleQuery {
            limit: Some("3".into()),
            ..q("BTCUSDT", "H1")
        };
        let r = svc.get_candles(&query).await.unwrap();
        assert_eq!(r.count, 3);
        assert_eq!(r.candles.last().unwrap().timestamp_ms, NOW - H);
    }

    #[test]
    fn lists_providers_with_status() {
        let list = default_service().providers();
        let get = |id| list.iter().find(|p| p.id == id).unwrap();
        assert!(matches!(get(ProviderId::Demo).status, ProviderStatus::Demo));
        assert!(matches!(
            get(ProviderId::BinancePublic).status,
            ProviderStatus::Disabled { .. }
        ));
        assert!(matches!(
            get(ProviderId::Mt5Bridge).status,
            ProviderStatus::NotImplemented { .. }
        ));
    }
}

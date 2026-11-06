//! Binance **public** spot candles (`GET /api/v3/klines`).
//!
//! Public market data only: no API key, no signed or private endpoint, no
//! trading. Disabled unless explicitly enabled by configuration. The HTTP
//! transport is injected so the adapter is tested against fixtures; routine
//! tests never reach the exchange.
//!
//! STATUS: the parsing follows Binance's documented kline format and is covered
//! by fixture tests. It has NOT been verified against the real exchange from
//! this repository.

use std::time::Duration;

use async_trait::async_trait;
use serde_json::Value;

use crate::error::MarketDataError;
use crate::instruments::Instrument;
use crate::model::{Candle, DataClass, ProviderId, ProviderStatus, Timeframe};
use crate::provider::{CandleProvider, ProviderRequest};

const ID: ProviderId = ProviderId::BinancePublic;

/// Binance's documented per-request kline maximum.
pub const BINANCE_MAX_LIMIT: usize = 1000;

#[derive(Debug, Clone)]
pub struct BinanceConfig {
    pub enabled: bool,
    pub base_url: String,
    pub timeout: Duration,
}

impl Default for BinanceConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            base_url: "https://api.binance.com".to_string(),
            timeout: Duration::from_secs(5),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpResponse {
    pub status: u16,
    /// Parsed `Retry-After` header, seconds.
    pub retry_after_seconds: Option<u64>,
    pub body: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HttpError {
    Timeout,
    Transport(String),
}

#[allow(clippy::double_must_use)
#[async_trait]
pub trait HttpGet: Send + Sync {
    async fn get(&self, url: &str, timeout: Duration) -> Result<HttpResponse, HttpError>;
}

pub struct ReqwestHttp {
    client: reqwest::Client,
}

impl ReqwestHttp {
    pub fn new() -> Result<Self, reqwest::Error> {
        Ok(Self {
            client: reqwest::Client::builder()
                .user_agent("edgenest-market-data/0.1")
                .build()?,
        })
    }
}

#[async_trait]
impl HttpGet for ReqwestHttp {
    async fn get(&self, url: &str, timeout: Duration) -> Result<HttpResponse, HttpError> {
        let response = self
            .client
            .get(url)
            .timeout(timeout)
            .send()
            .await
            .map_err(map_reqwest)?;
        let status = response.status().as_u16();
        let retry_after_seconds = response
            .headers()
            .get(reqwest::header::RETRY_AFTER)
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.trim().parse().ok());
        let body = response.text().await.map_err(map_reqwest)?;
        Ok(HttpResponse {
            status,
            retry_after_seconds,
            body,
        })
    }
}

fn map_reqwest(e: reqwest::Error) -> HttpError {
    if e.is_timeout() {
        HttpError::Timeout
    } else {
        // Strip the URL so the message never echoes query strings.
        HttpError::Transport(e.without_url().to_string())
    }
}

pub struct BinanceProvider<H: HttpGet> {
    config: BinanceConfig,
    http: H,
}

impl<H: HttpGet> BinanceProvider<H> {
    pub fn new(config: BinanceConfig, http: H) -> Self {
        Self { config, http }
    }

    /// Explicit timeframe mapping; no string munging.
    pub fn interval(timeframe: Timeframe) -> &'static str {
        match timeframe {
            Timeframe::M1 => "1m",
            Timeframe::M5 => "5m",
            Timeframe::M15 => "15m",
            Timeframe::M30 => "30m",
            Timeframe::H1 => "1h",
            Timeframe::H4 => "4h",
            Timeframe::D1 => "1d",
            Timeframe::W1 => "1w",
        }
    }

    pub fn build_url(&self, symbol: &str, request: &ProviderRequest) -> String {
        let mut url = format!(
            "{}/api/v3/klines?symbol={}&interval={}&limit={}",
            self.config.base_url.trim_end_matches('/'),
            symbol,
            Self::interval(request.timeframe),
            request.limit
        );
        if let Some(from) = request.from_ms {
            url.push_str(&format!("&startTime={from}"));
        }
        if let Some(to) = request.to_ms {
            url.push_str(&format!("&endTime={to}"));
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

fn number(value: &Value, row: usize, field: &str) -> Result<f64, MarketDataError> {
    value
        .as_str()
        .and_then(|s| s.parse::<f64>().ok())
        .filter(|n| n.is_finite())
        .ok_or_else(|| invalid(format!("kline {row}: {field} is not a numeric string")))
}

/// Parse a Binance kline array into canonical candles. Binance sends
/// `[openTime, "open", "high", "low", "close", "volume", closeTime, ...]`.
pub fn parse_klines(body: &str) -> Result<Vec<Candle>, MarketDataError> {
    let value: Value =
        serde_json::from_str(body).map_err(|_| invalid("response body is not valid JSON"))?;
    let rows = value
        .as_array()
        .ok_or_else(|| invalid("expected a JSON array of klines"))?;

    rows.iter()
        .enumerate()
        .map(|(i, row)| {
            let cells = row.as_array().filter(|c| c.len() >= 6).ok_or_else(|| {
                invalid(format!("kline {i}: expected an array of at least 6 fields"))
            })?;
            let timestamp_ms = cells[0]
                .as_i64()
                .ok_or_else(|| invalid(format!("kline {i}: open time is not an integer")))?;
            Ok(Candle {
                timestamp_ms,
                open: number(&cells[1], i, "open")?,
                high: number(&cells[2], i, "high")?,
                low: number(&cells[3], i, "low")?,
                close: number(&cells[4], i, "close")?,
                volume: Some(number(&cells[5], i, "volume")?),
            })
        })
        .collect()
}

fn map_error_status(response: &HttpResponse) -> MarketDataError {
    match response.status {
        429 | 418 => MarketDataError::RateLimited {
            provider: ID,
            retry_after_seconds: response.retry_after_seconds,
        },
        451 => MarketDataError::ProviderUnavailable {
            provider: ID,
            reason: "Binance refused the request for regional/legal reasons (HTTP 451)".into(),
        },
        400 => {
            // Binance error bodies look like {"code":-1121,"msg":"Invalid symbol."}.
            let code = serde_json::from_str::<Value>(&response.body)
                .ok()
                .and_then(|v| v.get("code").and_then(Value::as_i64));
            if code == Some(-1121) {
                MarketDataError::Unsupported {
                    provider: ID,
                    reason: "Binance does not recognise this symbol".into(),
                }
            } else {
                MarketDataError::Upstream {
                    provider: ID,
                    status: Some(400),
                    message: "Binance rejected the request".into(),
                }
            }
        }
        s => MarketDataError::Upstream {
            provider: ID,
            status: Some(s),
            message: format!("unexpected HTTP status {s}"),
        },
    }
}

#[async_trait]
impl<H: HttpGet> CandleProvider for BinanceProvider<H> {
    fn id(&self) -> ProviderId {
        ID
    }

    fn name(&self) -> &'static str {
        "Binance public market data"
    }

    fn classification(&self) -> DataClass {
        DataClass::Live
    }

    fn status(&self) -> ProviderStatus {
        if self.config.enabled {
            ProviderStatus::Available
        } else {
            ProviderStatus::Disabled {
                reason: "Binance public data is disabled. Set MARKET_DATA_BINANCE_ENABLED=true on the market-data service to enable outbound requests.".into(),
            }
        }
    }

    fn supports(&self, instrument: &Instrument, _: Timeframe) -> Result<(), MarketDataError> {
        // Every canonical timeframe has an explicit Binance interval.
        self.symbol_for(instrument)
            .map(|_| ())
            .ok_or_else(|| MarketDataError::Unsupported {
                provider: ID,
                reason: format!("no Binance spot symbol is mapped for '{}'", instrument.id),
            })
    }

    async fn fetch_candles(&self, r: &ProviderRequest) -> Result<Vec<Candle>, MarketDataError> {
        if !self.config.enabled {
            return Err(MarketDataError::ProviderDisabled {
                provider: ID,
                reason: "Binance public data is disabled".into(),
            });
        }
        let symbol = self
            .symbol_for(r.instrument)
            .ok_or_else(|| MarketDataError::Unsupported {
                provider: ID,
                reason: format!("no Binance spot symbol is mapped for '{}'", r.instrument.id),
            })?;
        if r.limit == 0 || r.limit > BINANCE_MAX_LIMIT {
            return Err(MarketDataError::InvalidRequest(format!(
                "limit must be between 1 and {BINANCE_MAX_LIMIT} for this provider"
            )));
        }

        let url = self.build_url(symbol, r);
        let response = self
            .http
            .get(&url, self.config.timeout)
            .await
            .map_err(|e| match e {
                HttpError::Timeout => MarketDataError::Timeout { provider: ID },
                HttpError::Transport(message) => MarketDataError::ProviderUnavailable {
                    provider: ID,
                    reason: format!("network error: {message}"),
                },
            })?;

        if response.status != 200 {
            return Err(map_error_status(&response));
        }
        parse_klines(&response.body)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::instruments::find;
    use std::collections::VecDeque;
    use std::sync::Mutex;

    type Reply = Result<HttpResponse, HttpError>;

    #[derive(Default)]
    struct FakeHttp {
        replies: Mutex<VecDeque<Reply>>,
        urls: Mutex<Vec<String>>,
    }

    impl FakeHttp {
        fn with(reply: Reply) -> Self {
            let f = FakeHttp::default();
            f.replies.lock().unwrap().push_back(reply);
            f
        }
        fn calls(&self) -> usize {
            self.urls.lock().unwrap().len()
        }
    }

    #[async_trait]
    impl HttpGet for FakeHttp {
        async fn get(&self, url: &str, _: Duration) -> Reply {
            self.urls.lock().unwrap().push(url.to_string());
            self.replies
                .lock()
                .unwrap()
                .pop_front()
                .expect("unexpected extra HTTP call")
        }
    }

    fn ok(body: &str) -> Reply {
        Ok(HttpResponse {
            status: 200,
            retry_after_seconds: None,
            body: body.to_string(),
        })
    }

    fn status(code: u16, retry: Option<u64>, body: &str) -> Reply {
        Ok(HttpResponse {
            status: code,
            retry_after_seconds: retry,
            body: body.to_string(),
        })
    }

    fn enabled() -> BinanceConfig {
        BinanceConfig {
            enabled: true,
            ..BinanceConfig::default()
        }
    }

    fn request(id: &str, tf: Timeframe) -> ProviderRequest {
        ProviderRequest {
            instrument: find(id).unwrap(),
            timeframe: tf,
            from_ms: None,
            to_ms: None,
            limit: 2,
            now_ms: 0,
        }
    }

    const FIXTURE: &str = r#"[
      [1700000000000,"67000.10","67100.00","66900.50","67050.25","12.5",1700000899999,"838000.0",100,"6.0","400000.0","0"],
      [1700000900000,"67050.25","67200.00","67000.00","67150.75","8.25",1700001799999,"553000.0",80,"4.0","268000.0","0"]
    ]"#;

    #[test]
    fn parses_documented_kline_format() {
        let c = parse_klines(FIXTURE).unwrap();
        assert_eq!(c.len(), 2);
        assert_eq!(c[0].timestamp_ms, 1_700_000_000_000);
        assert_eq!(c[0].open, 67_000.10);
        assert_eq!(c[0].high, 67_100.0);
        assert_eq!(c[0].low, 66_900.5);
        assert_eq!(c[0].close, 67_050.25);
        assert_eq!(c[0].volume, Some(12.5));
    }

    #[test]
    fn empty_array_is_empty_not_an_error() {
        assert_eq!(parse_klines("[]").unwrap(), vec![]);
    }

    #[test]
    fn rejects_malformed_payloads() {
        for body in [
            "not json",
            "{}",
            r#"{"code":-1121}"#,
            r#"[[1,"1","2","0.5"]]"#,
            r#"[[1.5,"1","2","0.5","1","1"]]"#,
            r#"[["1","1","2","0.5","1","1"]]"#,
            r#"[[1,1,2,0.5,1,1]]"#,
            r#"[[1,"x","2","0.5","1","1"]]"#,
            r#"[[1,"1","2","0.5","1","NaN"]]"#,
            "[1]",
        ] {
            assert!(
                matches!(
                    parse_klines(body),
                    Err(MarketDataError::InvalidProviderData { .. })
                ),
                "should reject: {body}"
            );
        }
    }

    #[test]
    fn maps_every_timeframe_explicitly() {
        let got: Vec<_> = Timeframe::ALL
            .iter()
            .map(|t| BinanceProvider::<FakeHttp>::interval(*t))
            .collect();
        assert_eq!(got, ["1m", "5m", "15m", "30m", "1h", "4h", "1d", "1w"]);
    }

    #[test]
    fn builds_url_with_window_and_trims_base() {
        let mut cfg = enabled();
        cfg.base_url = "https://example.test/".into();
        let p = BinanceProvider::new(cfg, FakeHttp::default());
        let mut r = request("BTCUSDT", Timeframe::M15);
        r.limit = 500;
        r.from_ms = Some(10);
        r.to_ms = Some(20);
        assert_eq!(
            p.build_url("BTCUSDT", &r),
            "https://example.test/api/v3/klines?symbol=BTCUSDT&interval=15m&limit=500&startTime=10&endTime=20"
        );
    }

    #[test]
    fn symbol_mapping_rejects_unmapped_instruments() {
        let p = BinanceProvider::new(enabled(), FakeHttp::default());
        assert!(p.supports(find("ETHUSDT").unwrap(), Timeframe::H1).is_ok());
        for id in ["EURUSD", "BTCUSDT-PERP"] {
            assert!(matches!(
                p.supports(find(id).unwrap(), Timeframe::H1),
                Err(MarketDataError::Unsupported { .. })
            ));
        }
    }

    #[tokio::test]
    async fn disabled_makes_no_http_call() {
        let p = BinanceProvider::new(BinanceConfig::default(), FakeHttp::default());
        assert!(matches!(p.status(), ProviderStatus::Disabled { .. }));
        let err = p
            .fetch_candles(&request("BTCUSDT", Timeframe::H1))
            .await
            .unwrap_err();
        assert!(matches!(err, MarketDataError::ProviderDisabled { .. }));
        assert_eq!(p.http.calls(), 0);
    }

    #[tokio::test]
    async fn fetches_and_requests_the_mapped_symbol() {
        let p = BinanceProvider::new(enabled(), FakeHttp::with(ok(FIXTURE)));
        let c = p
            .fetch_candles(&request("BTCUSDT", Timeframe::M15))
            .await
            .unwrap();
        assert_eq!(c.len(), 2);
        assert!(p.http.urls.lock().unwrap()[0].contains("symbol=BTCUSDT&interval=15m"));
    }

    #[tokio::test]
    async fn maps_http_failures() {
        let cases: Vec<(Reply, &str)> = vec![
            (status(429, Some(30), ""), "rate_limited"),
            (status(418, None, ""), "rate_limited"),
            (status(451, None, ""), "provider_unavailable"),
            (status(500, None, "boom"), "upstream_error"),
            (
                status(400, None, r#"{"code":-1121,"msg":"Invalid symbol."}"#),
                "unsupported_by_provider",
            ),
            (
                status(400, None, r#"{"code":-1100,"msg":"x"}"#),
                "upstream_error",
            ),
            (status(400, None, "garbage"), "upstream_error"),
            (ok("<html>"), "invalid_provider_data"),
            (Err(HttpError::Timeout), "provider_timeout"),
            (
                Err(HttpError::Transport("dns".into())),
                "provider_unavailable",
            ),
        ];
        for (reply, code) in cases {
            let p = BinanceProvider::new(enabled(), FakeHttp::with(reply));
            let err = p
                .fetch_candles(&request("BTCUSDT", Timeframe::H1))
                .await
                .unwrap_err();
            assert_eq!(err.code(), code);
        }
    }

    #[tokio::test]
    async fn rate_limit_keeps_retry_after() {
        let p = BinanceProvider::new(enabled(), FakeHttp::with(status(429, Some(30), "")));
        match p
            .fetch_candles(&request("BTCUSDT", Timeframe::H1))
            .await
            .unwrap_err()
        {
            MarketDataError::RateLimited {
                retry_after_seconds,
                ..
            } => {
                assert_eq!(retry_after_seconds, Some(30))
            }
            other => panic!("unexpected {other:?}"),
        }
    }

    /// Real reqwest client against a loopback socket that never answers:
    /// proves the configured timeout is actually applied. No external network.
    #[tokio::test]
    async fn real_client_times_out_against_silent_local_server() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            let (_socket, _) = listener.accept().await.unwrap();
            tokio::time::sleep(Duration::from_secs(5)).await;
        });
        let cfg = BinanceConfig {
            enabled: true,
            base_url: format!("http://{addr}"),
            timeout: Duration::from_millis(150),
        };
        let p = BinanceProvider::new(cfg, ReqwestHttp::new().unwrap());
        let err = p
            .fetch_candles(&request("BTCUSDT", Timeframe::H1))
            .await
            .unwrap_err();
        assert_eq!(err, MarketDataError::Timeout { provider: ID });
    }
}

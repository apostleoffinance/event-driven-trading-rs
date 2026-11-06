use std::sync::Arc;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use market_data::http::router;
use market_data::providers::binance::{
    BinanceConfig, BinanceProvider, HttpError, HttpGet, HttpResponse,
};
use market_data::providers::demo::DemoProvider;
use market_data::providers::mt5::Mt5Provider;
use market_data::service::{MarketDataService, ServiceConfig, SystemClock};
use serde_json::Value;
use tower::ServiceExt;

struct NoNetwork;

#[async_trait::async_trait]
impl HttpGet for NoNetwork {
    async fn get(&self, _: &str, _: std::time::Duration) -> Result<HttpResponse, HttpError> {
        panic!("tests must not perform network requests");
    }
}

fn app() -> axum::Router {
    let svc = MarketDataService::new(
        vec![
            Arc::new(DemoProvider),
            Arc::new(Mt5Provider),
            Arc::new(BinanceProvider::new(BinanceConfig::default(), NoNetwork)),
        ],
        ServiceConfig::default(),
        Arc::new(SystemClock),
    );
    router(Arc::new(svc), &["http://localhost:3107".to_string()])
}

async fn get(uri: &str) -> (StatusCode, axum::http::HeaderMap, Value) {
    let res = app()
        .oneshot(
            Request::get(uri)
                .header("origin", "http://localhost:3107")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let status = res.status();
    let headers = res.headers().clone();
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    let json = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    (status, headers, json)
}

#[tokio::test]
async fn demo_candles_contract() {
    let (s, h, j) =
        get("/api/v1/market-data/candles?instrument=BTCUSDT&timeframe=M15&limit=5&provider=demo")
            .await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(h["cache-control"], "no-store");
    assert_eq!(h["access-control-allow-origin"], "http://localhost:3107");
    assert_eq!(j["classification"], "demo");
    assert_eq!(j["freshness"], "demo");
    assert_eq!(j["provider"]["id"], "demo");
    assert_eq!(j["timeframe"], "M15");
    assert_eq!(j["instrument"]["id"], "BTCUSDT");
    assert_eq!(j["count"], 5);
    assert_eq!(j["candles"].as_array().unwrap().len(), 5);
    assert!(j["candles"][0]["timestamp_ms"].is_i64());
    assert!(j["last_updated_at"].as_str().unwrap().ends_with('Z'));
}

#[tokio::test]
async fn binance_disabled_returns_unavailable_not_demo() {
    let (s, _, j) = get("/api/v1/market-data/candles?instrument=BTCUSDT&timeframe=H1").await;
    assert_eq!(s, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(j["error"]["code"], "provider_disabled");
    assert_eq!(j["error"]["classification"], "unavailable");
    assert!(j.get("candles").is_none());
}

#[tokio::test]
async fn mt5_reports_missing_capability() {
    let (s, _, j) = get("/api/v1/market-data/candles?instrument=EURUSD&timeframe=H1").await;
    assert_eq!(s, StatusCode::NOT_IMPLEMENTED);
    assert_eq!(j["error"]["code"], "capability_not_implemented");
    assert_eq!(j["error"]["provider"], "mt5-bridge");
}

#[tokio::test]
async fn invalid_requests_map_to_4xx() {
    for (uri, status, code) in [
        ("/api/v1/market-data/candles", 400, "invalid_request"),
        (
            "/api/v1/market-data/candles?instrument=BTCUSDT&timeframe=M2",
            400,
            "invalid_request",
        ),
        (
            "/api/v1/market-data/candles?instrument=NOPE&timeframe=H1",
            404,
            "unknown_instrument",
        ),
        (
            "/api/v1/market-data/candles?instrument=BTCUSDT&timeframe=H1&limit=99999",
            400,
            "invalid_request",
        ),
    ] {
        let (s, _, j) = get(uri).await;
        assert_eq!(s.as_u16(), status, "{uri}");
        assert_eq!(j["error"]["code"], code, "{uri}");
    }
}

#[tokio::test]
async fn lists_providers_and_instruments() {
    let (s, _, j) = get("/api/v1/market-data/providers").await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(j["providers"].as_array().unwrap().len(), 3);
    let (_, _, j) = get("/api/v1/market-data/instruments").await;
    assert!(j["instruments"]
        .as_array()
        .unwrap()
        .iter()
        .any(|i| i["id"] == "BTCUSDT-PERP"));
    let (s, _, _) = get("/healthz").await;
    assert_eq!(s, StatusCode::OK);
}

mod mt5_latest_route {
    use super::*;
    use chrono::{DateTime, SecondsFormat, Utc};
    use market_data::http::router_with_mt5;
    use market_data::mt5_latest::{
        Mt5LatestService, Mt5Scope, Mt5SnapshotRecord, Mt5SnapshotSource, SourceError,
    };
    use market_data::service::Clock;

    const NOW_MS: i64 = 1_793_620_800_000;

    struct FixedClock;
    impl Clock for FixedClock {
        fn now_ms(&self) -> i64 {
            NOW_MS
        }
    }

    struct FakeSource(Result<Option<Mt5SnapshotRecord>, SourceError>);

    #[async_trait::async_trait]
    impl Mt5SnapshotSource for FakeSource {
        async fn load_latest(
            &self,
            _: &Mt5Scope,
        ) -> Result<Option<Mt5SnapshotRecord>, SourceError> {
            self.0.clone()
        }
    }

    fn ts(offset_ms: i64) -> DateTime<Utc> {
        DateTime::<Utc>::from_timestamp_millis(NOW_MS + offset_ms).unwrap()
    }

    fn good_record() -> Mt5SnapshotRecord {
        Mt5SnapshotRecord {
            sequence: 5978,
            captured_at: ts(-5_000),
            snapshot: serde_json::json!({ "quotes": [{
                "symbol": "EURUSD",
                "bid": "1.12115",
                "ask": "1.12126",
                "tick_time": ts(-7_000).to_rfc3339_opts(SecondsFormat::Secs, true),
            }]}),
        }
    }

    async fn get_mt5(
        source: Option<Result<Option<Mt5SnapshotRecord>, SourceError>>,
        uri: &str,
    ) -> (StatusCode, axum::http::HeaderMap, Value) {
        let scoped = source.is_some();
        let svc = MarketDataService::new(
            vec![Arc::new(DemoProvider)],
            ServiceConfig::default(),
            Arc::new(SystemClock),
        );
        let mt5 = Mt5LatestService::new(
            source.map(|r| Arc::new(FakeSource(r)) as Arc<dyn Mt5SnapshotSource>),
            scoped.then(|| Mt5Scope {
                account_id: "a".into(),
                terminal_id: "t".into(),
            }),
            Arc::new(FixedClock),
            60,
            std::time::Duration::from_secs(1),
        );
        let app = router_with_mt5(
            Arc::new(svc),
            Arc::new(mt5),
            &["http://localhost:3107".into()],
        );
        let res = app
            .oneshot(
                Request::get(uri)
                    .header("origin", "http://localhost:3107")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let status = res.status();
        let headers = res.headers().clone();
        let bytes = res.into_body().collect().await.unwrap().to_bytes();
        (
            status,
            headers,
            serde_json::from_slice(&bytes).unwrap_or(Value::Null),
        )
    }

    #[tokio::test]
    async fn returns_quote_contract_without_identity() {
        let (s, h, j) = get_mt5(
            Some(Ok(Some(good_record()))),
            "/api/v1/market-data/mt5/latest",
        )
        .await;
        assert_eq!(s, StatusCode::OK);
        assert_eq!(h.get("cache-control").unwrap(), "no-store");
        assert_eq!(j["status"], "fresh");
        assert_eq!(j["classification"], "live");
        assert_eq!(j["symbol"], "EURUSD");
        assert_eq!(j["bid"], "1.12115");
        assert_eq!(j["sequence"], 5978);
        assert_eq!(j["stale_after_seconds"], 60);
        let text = j.to_string();
        assert!(!text.contains("account") && !text.contains("terminal"));
    }

    #[tokio::test]
    async fn unconfigured_returns_503_with_no_store() {
        let (s, h, _) = get_mt5(None, "/api/v1/market-data/mt5/latest").await;
        assert_eq!(s, StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(h.get("cache-control").unwrap(), "no-store");
    }

    #[tokio::test]
    async fn rejects_query_parameters() {
        let (s, _, _) = get_mt5(
            Some(Ok(Some(good_record()))),
            "/api/v1/market-data/mt5/latest?account_id=x",
        )
        .await;
        assert_eq!(s, StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn timeout_and_corrupt_map_to_504_and_502() {
        let (s, _, _) = get_mt5(
            Some(Err(SourceError::Timeout)),
            "/api/v1/market-data/mt5/latest",
        )
        .await;
        assert_eq!(s, StatusCode::GATEWAY_TIMEOUT);
        let (s, _, _) = get_mt5(
            Some(Err(SourceError::Corrupt)),
            "/api/v1/market-data/mt5/latest",
        )
        .await;
        assert_eq!(s, StatusCode::BAD_GATEWAY);
    }

    #[tokio::test]
    async fn existing_routes_still_work_alongside_mt5() {
        let (s, _, _) = get_mt5(None, "/healthz").await;
        assert_eq!(s, StatusCode::OK);
    }
}

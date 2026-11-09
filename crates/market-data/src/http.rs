use std::sync::Arc;

use axum::extract::{Query, RawQuery, State};
use axum::http::{header, HeaderValue, Method, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Json, Router};
use tower_http::cors::{AllowOrigin, CorsLayer};

use crate::error::MarketDataError;
use crate::fx_conversion::{convert, FxConversionQuery};
use crate::mt5_latest::Mt5LatestService;
use crate::service::{CandleQuery, MarketDataService, SystemClock};

pub type Shared = Arc<MarketDataService>;

impl IntoResponse for MarketDataError {
    fn into_response(self) -> Response {
        let status =
            StatusCode::from_u16(self.http_status()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
        let mut response = (status, Json(self.body())).into_response();
        if let MarketDataError::RateLimited {
            retry_after_seconds: Some(s),
            ..
        } = &self
        {
            if let Ok(v) = HeaderValue::from_str(&s.to_string()) {
                response.headers_mut().insert(header::RETRY_AFTER, v);
            }
        }
        response
    }
}

async fn candles(
    State(svc): State<Shared>,
    Query(q): Query<CandleQuery>,
) -> Result<Response, MarketDataError> {
    Ok(Json(svc.get_candles(&q).await?).into_response())
}

async fn fx_convert(
    State(svc): State<Shared>,
    Query(q): Query<FxConversionQuery>,
) -> Result<Response, MarketDataError> {
    Ok(Json(convert(&svc, q).await?).into_response())
}

async fn providers(State(svc): State<Shared>) -> Response {
    Json(serde_json::json!({ "providers": svc.providers() })).into_response()
}

async fn instruments(State(svc): State<Shared>) -> Response {
    Json(serde_json::json!({ "instruments": svc.instruments() })).into_response()
}

async fn healthz() -> &'static str {
    "ok"
}

/// Scope comes from server configuration only; any query string is rejected.
async fn mt5_latest(
    State(svc): State<Arc<Mt5LatestService>>,
    RawQuery(query): RawQuery,
) -> Result<Response, MarketDataError> {
    if query.is_some_and(|q| !q.is_empty()) {
        return Err(MarketDataError::InvalidRequest(
            "this endpoint accepts no query parameters".into(),
        ));
    }
    Ok(Json(svc.latest().await?).into_response())
}

pub fn router(service: Shared, cors_origins: &[String]) -> Router {
    let mt5 = Arc::new(Mt5LatestService::unconfigured(Arc::new(SystemClock)));
    router_with_mt5(service, mt5, cors_origins)
}

pub fn router_with_mt5(
    service: Shared,
    mt5: Arc<Mt5LatestService>,
    cors_origins: &[String],
) -> Router {
    let origins: Vec<HeaderValue> = cors_origins.iter().filter_map(|o| o.parse().ok()).collect();
    let cors = CorsLayer::new()
        .allow_origin(AllowOrigin::list(origins))
        .allow_methods([Method::GET])
        .allow_headers([header::ACCEPT]);

    let mt5_routes = Router::new()
        .route("/api/v1/market-data/mt5/latest", get(mt5_latest))
        .with_state(mt5);

    Router::new()
        .route("/api/v1/market-data/fx/convert", get(fx_convert))
        .route("/api/v1/market-data/candles", get(candles))
        .route("/api/v1/market-data/providers", get(providers))
        .route("/api/v1/market-data/instruments", get(instruments))
        .route("/healthz", get(healthz))
        .with_state(service)
        .merge(mt5_routes)
        .layer(axum::middleware::map_response(
            |mut r: Response| async move {
                r.headers_mut()
                    .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
                r
            },
        ))
        .layer(cors)
}

//! Owner-scoped persistence for excursion metrics derived from historical candles.

use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use rust_decimal::Decimal;
use serde::Deserialize;

use crate::AppState;

#[derive(Debug, Deserialize)]
pub struct ExcursionWrite {
    pub mfe_price: Decimal,
    pub mae_price: Decimal,
    pub mfe_r: Option<Decimal>,
    pub mae_r: Option<Decimal>,
    pub timeframe: String,
    pub provider: String,
    pub candle_count: i64,
    pub coverage_start: String,
    pub coverage_end: String,
}

pub async fn save(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(trade_id): Path<String>,
    Json(input): Json<ExcursionWrite>,
) -> Response {
    let claims = match state.auth.authenticate(&headers) {
        Ok(claims) => claims,
        Err(error) => return error.into_response(),
    };
    if input.mfe_price <= Decimal::ZERO
        || input.mae_price <= Decimal::ZERO
        || input.candle_count < 1
        || input.timeframe.trim().is_empty()
        || input.provider.trim().is_empty()
    {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "error": {
                    "code": "invalid_excursions",
                    "message": "Valid prices, timeframe, provider and candle_count are required"
                }
            })),
        )
            .into_response();
    }

    let metadata = serde_json::json!({
        "timeframe": input.timeframe,
        "provider": input.provider,
        "candle_count": input.candle_count,
        "coverage_start": input.coverage_start,
        "coverage_end": input.coverage_end,
        "method": "historical_ohlc_extremes",
    });

    match sqlx::query(
        "UPDATE journal_trades SET mfe_price=$3, mae_price=$4, mfe_r=$5, mae_r=$6, mfe_source='historical_candles', mae_source='historical_candles', mfe_precision=$7, mae_precision=$7, source_metadata=COALESCE(source_metadata,'{}'::jsonb) || $8::jsonb, updated_at=NOW() WHERE owner_id=$1 AND id=$2",
    )
    .bind(&claims.sub)
    .bind(&trade_id)
    .bind(input.mfe_price)
    .bind(input.mae_price)
    .bind(input.mfe_r)
    .bind(input.mae_r)
    .bind(&input.timeframe)
    .bind(metadata)
    .execute(state.store.pool())
    .await
    {
        Ok(result) if result.rows_affected() == 1 => Json(serde_json::json!({
            "data": {
                "trade_id": trade_id,
                "saved": true,
                "precision": input.timeframe
            }
        }))
        .into_response(),
        Ok(_) => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({
                "error": {"code": "not_found", "message": "Journal trade not found"}
            })),
        )
            .into_response(),
        Err(_) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({
                "error": {
                    "code": "internal_error",
                    "message": "Unable to save excursion metrics"
                }
            })),
        )
            .into_response(),
    }
}

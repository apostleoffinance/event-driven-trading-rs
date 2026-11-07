use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use chrono::{DateTime, Utc};
use domain::{JournalDirection, JournalTrade, JournalTradeSource, JournalTradeStatus};
use rust_decimal::Decimal;
use serde::Deserialize;
use sqlx::PgPool;

use crate::{
    auth::{ApiAuthError, Claims},
    AppState,
};

#[derive(Debug, Deserialize)]
pub struct ListQuery {
    pub journal_account_id: Option<String>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct TradeWrite {
    pub journal_account_id: String,
    pub setup_id: Option<String>,
    pub symbol: String,
    pub direction: JournalDirection,
    pub status: JournalTradeStatus,
    #[serde(default = "default_source")]
    pub source: JournalTradeSource,
    pub source_system: Option<String>,
    pub source_trade_id: Option<String>,
    pub opened_at: DateTime<Utc>,
    pub closed_at: Option<DateTime<Utc>>,
    pub quantity: Decimal,
    pub entry_price: Decimal,
    pub exit_price: Option<Decimal>,
    pub stop_loss: Option<Decimal>,
    pub take_profit: Option<Decimal>,
    pub gross_pnl: Option<Decimal>,
    #[serde(default)]
    pub commission: Decimal,
    #[serde(default)]
    pub swap: Decimal,
    #[serde(default)]
    pub other_fees: Decimal,
    pub net_pnl: Option<Decimal>,
    #[serde(default = "default_currency")]
    pub pnl_currency: String,
    pub risk_amount: Option<Decimal>,
    pub r_multiple: Option<Decimal>,
}
fn default_source() -> JournalTradeSource {
    JournalTradeSource::Manual
}
fn default_currency() -> String {
    "USD".into()
}

fn owner(state: &AppState, headers: &HeaderMap) -> Result<Claims, ApiAuthError> {
    state.auth.authenticate(headers)
}

pub(crate) fn trade_from(owner_id: String, id: String, input: TradeWrite) -> JournalTrade {
    JournalTrade {
        id,
        owner_id,
        journal_account_id: input.journal_account_id,
        setup_id: input.setup_id,
        symbol: input.symbol.trim().to_uppercase(),
        direction: input.direction,
        status: input.status,
        source: input.source,
        source_system: input.source_system,
        source_trade_id: input.source_trade_id,
        opened_at: input.opened_at,
        closed_at: input.closed_at,
        quantity: input.quantity,
        entry_price: input.entry_price,
        exit_price: input.exit_price,
        stop_loss: input.stop_loss,
        take_profit: input.take_profit,
        gross_pnl: input.gross_pnl,
        commission: input.commission,
        swap: input.swap,
        other_fees: input.other_fees,
        net_pnl: input.net_pnl,
        pnl_currency: input.pnl_currency.to_uppercase(),
        risk_amount: input.risk_amount,
        r_multiple: input.r_multiple,
    }
}

fn bad_request(message: impl Into<String>) -> Response {
    (
        StatusCode::BAD_REQUEST,
        Json(serde_json::json!({"error":{"code":"invalid_request","message":message.into()}})),
    )
        .into_response()
}
fn internal() -> Response {
    (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error":{"code":"internal_error","message":"Internal server error"}}))).into_response()
}
fn not_found() -> Response {
    (
        StatusCode::NOT_FOUND,
        Json(serde_json::json!({"error":{"code":"not_found","message":"Journal trade not found"}})),
    )
        .into_response()
}

pub async fn health() -> &'static str {
    "ok"
}
pub async fn ready(State(state): State<AppState>) -> Response {
    match sqlx::query_scalar::<_, i32>("SELECT 1")
        .fetch_one(state.store.pool())
        .await
    {
        Ok(_) => (StatusCode::OK, Json(serde_json::json!({"status":"ready"}))).into_response(),
        Err(_) => (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({"status":"not_ready"})),
        )
            .into_response(),
    }
}

pub async fn list(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<ListQuery>,
) -> Response {
    let claims = match owner(&state, &headers) {
        Ok(c) => c,
        Err(e) => return e.into_response(),
    };
    match state.store.list_journal_trades(&claims.sub, query.journal_account_id.as_deref(), query.limit.unwrap_or(100), query.offset.unwrap_or(0)).await {
        Ok(trades) => Json(serde_json::json!({"data":trades,"pagination":{"limit":query.limit.unwrap_or(100).clamp(1,500),"offset":query.offset.unwrap_or(0).max(0)}})).into_response(),
        Err(_) => internal(),
    }
}

pub async fn get_one(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Response {
    let claims = match owner(&state, &headers) {
        Ok(c) => c,
        Err(e) => return e.into_response(),
    };
    match state.store.get_journal_trade(&claims.sub, &id).await {
        Ok(trade) => Json(serde_json::json!({"data":trade})).into_response(),
        Err(persistence::PersistenceError::NotFound(_)) => not_found(),
        Err(_) => internal(),
    }
}

pub async fn create(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<TradeWrite>,
) -> Response {
    let claims = match owner(&state, &headers) {
        Ok(c) => c,
        Err(e) => return e.into_response(),
    };
    let trade = trade_from(claims.sub, uuid::Uuid::new_v4().to_string(), input);
    if let Err(e) = trade.validate() {
        return bad_request(e.to_string());
    }
    if !owned_account(
        state.store.pool(),
        &trade.owner_id,
        &trade.journal_account_id,
    )
    .await
    {
        return not_found();
    }
    if let Some(setup_id) = trade.setup_id.as_deref() {
        if !owned_setup(state.store.pool(), &trade.owner_id, setup_id).await {
            return not_found();
        }
    }
    if let (Some(source_system), Some(source_trade_id)) = (
        trade.source_system.as_deref(),
        trade.source_trade_id.as_deref(),
    ) {
        let duplicate = sqlx::query_scalar::<_, bool>(
            "SELECT EXISTS(SELECT 1 FROM journal_trades WHERE journal_account_id=$1 AND source_system=$2 AND source_trade_id=$3)",
        )
        .bind(&trade.journal_account_id)
        .bind(source_system)
        .bind(source_trade_id)
        .fetch_one(state.store.pool())
        .await
        .unwrap_or(false);
        if duplicate {
            return (
                StatusCode::CONFLICT,
                Json(serde_json::json!({"error":{"code":"duplicate_source_trade","message":"A trade with this source identity already exists"}})),
            )
                .into_response();
        }
    }
    match state.store.insert_journal_trade(&trade).await {
        Ok(()) => match state
            .store
            .get_journal_trade(&trade.owner_id, &trade.id)
            .await
        {
            Ok(created) => (
                StatusCode::CREATED,
                Json(serde_json::json!({"data":created})),
            )
                .into_response(),
            Err(persistence::PersistenceError::NotFound(_)) => (
                StatusCode::CONFLICT,
                Json(serde_json::json!({"error":{"code":"duplicate_source_trade","message":"A trade with this source identity already exists"}})),
            )
                .into_response(),
            Err(_) => internal(),
        },
        Err(persistence::PersistenceError::Domain(e)) => bad_request(e.to_string()),
        Err(_) => internal(),
    }
}

pub async fn replace(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(input): Json<TradeWrite>,
) -> Response {
    let claims = match owner(&state, &headers) {
        Ok(c) => c,
        Err(e) => return e.into_response(),
    };
    let trade = trade_from(claims.sub, id.clone(), input);
    if let Err(e) = trade.validate() {
        return bad_request(e.to_string());
    }
    if !owned_account(
        state.store.pool(),
        &trade.owner_id,
        &trade.journal_account_id,
    )
    .await
    {
        return not_found();
    }
    if let Some(setup_id) = trade.setup_id.as_deref() {
        if !owned_setup(state.store.pool(), &trade.owner_id, setup_id).await {
            return not_found();
        }
    }
    let result = update_trade(state.store.pool(), &trade).await;
    match result {
        Ok(false) => not_found(),
        Ok(true) => match state.store.get_journal_trade(&trade.owner_id, &id).await {
            Ok(updated) => Json(serde_json::json!({"data":updated})).into_response(),
            Err(_) => internal(),
        },
        Err(_) => internal(),
    }
}

async fn update_trade(pool: &PgPool, trade: &JournalTrade) -> Result<bool, sqlx::Error> {
    let direction = match trade.direction {
        JournalDirection::Long => "long",
        JournalDirection::Short => "short",
    };
    let status = match trade.status {
        JournalTradeStatus::Open => "open",
        JournalTradeStatus::Closed => "closed",
        JournalTradeStatus::Cancelled => "cancelled",
    };
    let source = match trade.source {
        JournalTradeSource::Manual => "manual",
        JournalTradeSource::Mt5 => "mt5",
        JournalTradeSource::LegacyImport => "legacy_import",
        JournalTradeSource::Other => "other",
    };
    let result = sqlx::query(r#"UPDATE journal_trades SET journal_account_id=$3, setup_id=$4, symbol=$5, direction=$6, status=$7, source=$8,
        source_system=$9, source_trade_id=$10, opened_at=$11, closed_at=$12, quantity=$13, entry_price=$14, exit_price=$15,
        stop_loss=$16, take_profit=$17, gross_pnl=$18, commission=$19, swap=$20, other_fees=$21, net_pnl=$22,
        pnl_currency=$23, risk_amount=$24, r_multiple=$25, updated_at=NOW()
        WHERE owner_id=$1 AND id=$2"#)
        .bind(&trade.owner_id).bind(&trade.id).bind(&trade.journal_account_id).bind(&trade.setup_id).bind(&trade.symbol)
        .bind(direction).bind(status).bind(source).bind(&trade.source_system).bind(&trade.source_trade_id)
        .bind(trade.opened_at).bind(trade.closed_at).bind(trade.quantity).bind(trade.entry_price).bind(trade.exit_price)
        .bind(trade.stop_loss).bind(trade.take_profit).bind(trade.gross_pnl).bind(trade.commission).bind(trade.swap)
        .bind(trade.other_fees).bind(trade.net_pnl).bind(&trade.pnl_currency).bind(trade.risk_amount).bind(trade.r_multiple)
        .execute(pool).await?;
    Ok(result.rows_affected() == 1)
}

pub async fn delete(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Response {
    let claims = match owner(&state, &headers) {
        Ok(c) => c,
        Err(e) => return e.into_response(),
    };
    match state.store.delete_journal_trade(&claims.sub, &id).await {
        Ok(true) => StatusCode::NO_CONTENT.into_response(),
        Ok(false) => not_found(),
        Err(_) => internal(),
    }
}

async fn owned_account(pool: &PgPool, owner_id: &str, account_id: &str) -> bool {
    sqlx::query_scalar::<_, bool>("SELECT EXISTS(SELECT 1 FROM journal_accounts WHERE owner_id=$1 AND id=$2 AND archived=FALSE)")
        .bind(owner_id).bind(account_id).fetch_one(pool).await.unwrap_or(false)
}
async fn owned_setup(pool: &PgPool, owner_id: &str, setup_id: &str) -> bool {
    sqlx::query_scalar::<_, bool>("SELECT EXISTS(SELECT 1 FROM journal_setups WHERE owner_id=$1 AND id=$2 AND archived=FALSE)")
        .bind(owner_id).bind(setup_id).fetch_one(pool).await.unwrap_or(false)
}

use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use rust_decimal::Decimal;
use serde::Deserialize;
use serde_json::Value;

use crate::{
    auth::{ApiAuthError, Claims},
    AppState,
};

fn principal(state: &AppState, headers: &HeaderMap) -> Result<Claims, ApiAuthError> {
    state.auth.authenticate(headers)
}
fn err(status: StatusCode, code: &str, message: &str) -> Response {
    (
        status,
        Json(serde_json::json!({"error":{"code":code,"message":message}})),
    )
        .into_response()
}
fn internal() -> Response {
    err(
        StatusCode::INTERNAL_SERVER_ERROR,
        "internal_error",
        "Internal server error",
    )
}
fn not_found() -> Response {
    err(
        StatusCode::NOT_FOUND,
        "not_found",
        "Journal resource not found",
    )
}

#[derive(Debug, Deserialize)]
pub struct AccountWrite {
    pub name: String,
    pub broker: Option<String>,
    #[serde(default = "default_account_type")]
    pub account_type: String,
    #[serde(default = "default_currency")]
    pub currency: String,
    #[serde(default)]
    pub initial_balance: Decimal,
    #[serde(default)]
    pub external_source: Option<String>,
    #[serde(default)]
    pub external_account_id: Option<String>,
    #[serde(default = "default_rules_json")]
    pub migration_metadata: Value,
}
fn default_account_type() -> String {
    "journal".into()
}
fn default_currency() -> String {
    "USD".into()
}

pub async fn list_accounts(State(state): State<AppState>, headers: HeaderMap) -> Response {
    let claims = match principal(&state, &headers) {
        Ok(c) => c,
        Err(e) => return e.into_response(),
    };
    match sqlx::query_as::<_, (String, String, Option<String>, String, String, Decimal, bool, Value, Option<String>)>(
        "SELECT id,name,broker,account_type,currency,initial_balance,archived,migration_metadata,linked_trading_account_id FROM journal_accounts WHERE owner_id=$1 AND archived=FALSE ORDER BY created_at DESC")
        .bind(claims.sub).fetch_all(state.store.pool()).await {
        Ok(rows) => Json(serde_json::json!({"data":rows.into_iter().map(|(id,name,broker,account_type,currency,initial_balance,archived,migration_metadata,linked_trading_account_id)|serde_json::json!({
            "id":id,"name":name,"broker":broker,"account_type":account_type,"currency":currency,"initial_balance":initial_balance.to_string(),"archived":archived,"migration_metadata":migration_metadata,"linked_trading_account_id":linked_trading_account_id
        })).collect::<Vec<_>>()})).into_response(),
        Err(_) => internal(),
    }
}

pub async fn create_account(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<AccountWrite>,
) -> Response {
    let claims = match principal(&state, &headers) {
        Ok(c) => c,
        Err(e) => return e.into_response(),
    };
    let name = input.name.trim();
    let currency = input.currency.to_uppercase();
    if name.is_empty()
        || !currency_is_valid(&currency)
        || input.initial_balance < Decimal::ZERO
        || !["journal", "paper", "funded", "live", "demo"].contains(&input.account_type.as_str())
        || !input.migration_metadata.is_object()
        || (input.external_source.is_some() != input.external_account_id.is_some())
    {
        return err(
            StatusCode::BAD_REQUEST,
            "invalid_request",
            "name, currency, account_type, or initial_balance is invalid",
        );
    }
    let id = uuid::Uuid::new_v4().to_string();
    let inserted = sqlx::query("INSERT INTO journal_accounts(id,owner_id,name,broker,account_type,currency,initial_balance,external_source,external_account_id,migration_metadata) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10) ON CONFLICT (owner_id,external_source,external_account_id) DO NOTHING")
        .bind(&id).bind(&claims.sub).bind(name).bind(input.broker).bind(input.account_type).bind(&currency).bind(input.initial_balance)
        .bind(&input.external_source).bind(&input.external_account_id).bind(&input.migration_metadata)
        .execute(state.store.pool()).await;
    match inserted {
        Ok(result) if result.rows_affected() == 1 => (StatusCode::CREATED, Json(serde_json::json!({"data":{"id":id,"name":name,"currency":currency,"initial_balance":input.initial_balance.to_string()}}))).into_response(),
        Ok(_) => match sqlx::query_scalar::<_,String>("SELECT id FROM journal_accounts WHERE owner_id=$1 AND external_source=$2 AND external_account_id=$3")
            .bind(&claims.sub).bind(&input.external_source).bind(&input.external_account_id).fetch_optional(state.store.pool()).await {
            Ok(Some(existing_id)) => (StatusCode::OK, Json(serde_json::json!({"data":{"id":existing_id,"name":name,"currency":currency,"initial_balance":input.initial_balance.to_string(),"already_imported":true}}))).into_response(),
            _ => internal(),
        },
        Err(_) => internal(),
    }
}

pub async fn get_account(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Response {
    let claims = match principal(&state, &headers) {
        Ok(c) => c,
        Err(e) => return e.into_response(),
    };
    match sqlx::query_as::<_, (String,String,Option<String>,String,String,Decimal,bool,Value,Option<String>)>(
        "SELECT id,name,broker,account_type,currency,initial_balance,archived,migration_metadata,linked_trading_account_id FROM journal_accounts WHERE owner_id=$1 AND id=$2")
        .bind(claims.sub).bind(id).fetch_optional(state.store.pool()).await {
        Ok(Some((id,name,broker,account_type,currency,initial_balance,archived,migration_metadata,linked_trading_account_id))) => Json(serde_json::json!({"data":{"id":id,"name":name,"broker":broker,"account_type":account_type,"currency":currency,"initial_balance":initial_balance.to_string(),"archived":archived,"migration_metadata":migration_metadata,"linked_trading_account_id":linked_trading_account_id}})).into_response(),
        Ok(None) => not_found(),
        Err(_) => internal(),
    }
}

pub async fn update_account(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(input): Json<AccountWrite>,
) -> Response {
    let claims = match principal(&state, &headers) {
        Ok(c) => c,
        Err(e) => return e.into_response(),
    };
    let name = input.name.trim();
    let currency = input.currency.to_uppercase();
    if name.is_empty()
        || !currency_is_valid(&currency)
        || input.initial_balance < Decimal::ZERO
        || !["journal", "paper", "funded", "live", "demo"].contains(&input.account_type.as_str())
    {
        return err(
            StatusCode::BAD_REQUEST,
            "invalid_request",
            "name, currency, account_type, or initial_balance is invalid",
        );
    }
    match sqlx::query("UPDATE journal_accounts SET name=$3,broker=$4,account_type=$5,currency=$6,initial_balance=$7,updated_at=NOW() WHERE owner_id=$1 AND id=$2 AND archived=FALSE")
        .bind(claims.sub).bind(id).bind(name).bind(input.broker).bind(input.account_type).bind(currency).bind(input.initial_balance)
        .execute(state.store.pool()).await {
        Ok(r) if r.rows_affected()==1 => StatusCode::NO_CONTENT.into_response(),
        Ok(_) => not_found(),
        Err(_) => internal(),
    }
}

/// Merge owner-scoped migration metadata without overwriting ordinary account fields.
/// Used to attach a source-archive reference after an interrupted import.
pub async fn update_account_migration_metadata(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(patch): Json<Value>,
) -> Response {
    let claims = match principal(&state, &headers) {
        Ok(c) => c,
        Err(e) => return e.into_response(),
    };
    if !patch.is_object() {
        return err(
            StatusCode::BAD_REQUEST,
            "invalid_migration_metadata",
            "Migration metadata patch must be a JSON object",
        );
    }
    match sqlx::query(
        "UPDATE journal_accounts SET migration_metadata = migration_metadata || $3, updated_at=NOW() WHERE owner_id=$1 AND id=$2 AND archived=FALSE",
    )
    .bind(claims.sub)
    .bind(id)
    .bind(patch)
    .execute(state.store.pool())
    .await
    {
        Ok(result) if result.rows_affected() == 1 => StatusCode::NO_CONTENT.into_response(),
        Ok(_) => not_found(),
        Err(_) => internal(),
    }
}

pub async fn archive_account(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Response {
    let claims = match principal(&state, &headers) {
        Ok(c) => c,
        Err(e) => return e.into_response(),
    };
    match sqlx::query("UPDATE journal_accounts SET archived=TRUE,updated_at=NOW() WHERE owner_id=$1 AND id=$2 AND archived=FALSE")
        .bind(claims.sub).bind(id).execute(state.store.pool()).await {
        Ok(r) if r.rows_affected()==1 => StatusCode::NO_CONTENT.into_response(),
        Ok(_) => not_found(),
        Err(_) => internal(),
    }
}

fn default_rules_json() -> Value {
    serde_json::json!({})
}

#[derive(Debug, Deserialize)]
pub struct SetupWrite {
    pub name: String,
    pub description: Option<String>,
    #[serde(default = "default_rules_json")]
    pub rules_json: Value,
}


#[derive(Debug, Deserialize)]
pub struct LinkTradingAccountWrite {
    pub linked_trading_account_id: Option<String>,
}

/// Link a Journal ledger to a trading account the authenticated principal already owns.
/// This does not grant execution permission and never creates a trading account.
pub async fn link_trading_account(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(input): Json<LinkTradingAccountWrite>,
) -> Response {
    let claims = match principal(&state, &headers) {
        Ok(c) => c,
        Err(e) => return e.into_response(),
    };
    if let Some(trading_id) = input.linked_trading_account_id.as_deref() {
        let accessible = sqlx::query_scalar::<_, bool>(
            "SELECT EXISTS(SELECT 1 FROM dashboard_account_access WHERE owner_id=$1 AND account_id=$2)",
        )
        .bind(&claims.sub)
        .bind(trading_id)
        .fetch_one(state.store.pool())
        .await;
        match accessible {
            Ok(true) => {}
            Ok(false) => return not_found(),
            Err(_) => return internal(),
        }
        let already_linked = sqlx::query_scalar::<_, bool>(
            "SELECT EXISTS(SELECT 1 FROM journal_accounts WHERE owner_id=$1 AND linked_trading_account_id=$2 AND id<>$3 AND archived=FALSE)",
        )
        .bind(&claims.sub)
        .bind(trading_id)
        .bind(&id)
        .fetch_one(state.store.pool())
        .await;
        match already_linked {
            Ok(true) => return err(StatusCode::CONFLICT, "account_already_linked", "This trading account is already linked to another Journal account."),
            Ok(false) => {}
            Err(_) => return internal(),
        }
    }
    match sqlx::query(
        "UPDATE journal_accounts SET linked_trading_account_id=$3,updated_at=NOW() WHERE owner_id=$1 AND id=$2 AND archived=FALSE",
    )
    .bind(&claims.sub)
    .bind(&id)
    .bind(&input.linked_trading_account_id)
    .execute(state.store.pool())
    .await
    {
        Ok(result) if result.rows_affected() == 1 => Json(serde_json::json!({
            "data": { "journal_account_id": id, "linked_trading_account_id": input.linked_trading_account_id }
        })).into_response(),
        Ok(_) => not_found(),
        Err(_) => internal(),
    }
}

/// Journal-only portfolio values remain separate from broker-reported equity.
/// Accounts linked to execution accounts are excluded to prevent double counting.
pub async fn journal_portfolio(State(state): State<AppState>, headers: HeaderMap) -> Response {
    let claims = match principal(&state, &headers) {
        Ok(c) => c,
        Err(e) => return e.into_response(),
    };
    let rows = sqlx::query_as::<_, (String, String, String, Decimal, Decimal, i64, Option<String>)>(
        r#"
        SELECT a.id,a.name,a.currency,a.initial_balance,
               COALESCE(SUM(CASE WHEN t.status='closed' THEN t.net_pnl ELSE 0 END),0) AS net_pnl,
               COUNT(t.id)::BIGINT AS trade_count,a.linked_trading_account_id
        FROM journal_accounts a
        LEFT JOIN journal_trades t ON t.owner_id=a.owner_id AND t.journal_account_id=a.id
        WHERE a.owner_id=$1 AND a.archived=FALSE
        GROUP BY a.id,a.name,a.currency,a.initial_balance,a.linked_trading_account_id
        ORDER BY a.created_at DESC
        "#,
    )
    .bind(&claims.sub)
    .fetch_all(state.store.pool())
    .await;
    match rows {
        Ok(rows) => Json(serde_json::json!({"data": rows.into_iter().map(|(id,name,currency,initial_balance,net_pnl,trade_count,linked_trading_account_id)| {
            serde_json::json!({
                "account_id":id,"name":name,"currency":currency,
                "initial_balance":initial_balance.to_string(),
                "realized_net_pnl":net_pnl.to_string(),
                "journal_equity":(initial_balance+net_pnl).to_string(),
                "trade_count":trade_count,"linked_trading_account_id":linked_trading_account_id
            })
        }).collect::<Vec<_>>()})).into_response(),
        Err(_) => internal(),
    }
}

pub async fn list_setups(State(state): State<AppState>, headers: HeaderMap) -> Response {
    let claims = match principal(&state, &headers) {
        Ok(c) => c,
        Err(e) => return e.into_response(),
    };
    match sqlx::query_as::<_, (String,String,Option<String>,Value,bool)>("SELECT id,name,description,rules_json,archived FROM journal_setups WHERE owner_id=$1 AND archived=FALSE ORDER BY created_at DESC")
        .bind(claims.sub).fetch_all(state.store.pool()).await {
        Ok(rows) => Json(serde_json::json!({"data":rows.into_iter().map(|(id,name,description,rules_json,archived)|serde_json::json!({"id":id,"name":name,"description":description,"rules_json":rules_json,"archived":archived})).collect::<Vec<_>>()})).into_response(),
        Err(_) => internal(),
    }
}

pub async fn create_setup(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<SetupWrite>,
) -> Response {
    let claims = match principal(&state, &headers) {
        Ok(c) => c,
        Err(e) => return e.into_response(),
    };
    if input.name.trim().is_empty() || !input.rules_json.is_object() {
        return err(
            StatusCode::BAD_REQUEST,
            "invalid_request",
            "setup name must be non-empty and rules_json must be an object",
        );
    }
    let id = uuid::Uuid::new_v4().to_string();
    match sqlx::query("INSERT INTO journal_setups(id,owner_id,name,description,rules_json) VALUES($1,$2,$3,$4,$5)")
        .bind(&id).bind(claims.sub).bind(input.name.trim()).bind(input.description).bind(input.rules_json).execute(state.store.pool()).await {
        Ok(_) => (StatusCode::CREATED,Json(serde_json::json!({"data":{"id":id,"name":input.name.trim()}}))).into_response(),
        Err(_) => err(StatusCode::CONFLICT,"setup_conflict","Setup name may already exist"),
    }
}

pub async fn get_setup(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Response {
    let claims = match principal(&state, &headers) {
        Ok(c) => c,
        Err(e) => return e.into_response(),
    };
    match sqlx::query_as::<_, (String,String,Option<String>,Value,bool)>("SELECT id,name,description,rules_json,archived FROM journal_setups WHERE owner_id=$1 AND id=$2")
        .bind(claims.sub).bind(id).fetch_optional(state.store.pool()).await {
        Ok(Some((id,name,description,rules_json,archived))) => Json(serde_json::json!({"data":{"id":id,"name":name,"description":description,"rules_json":rules_json,"archived":archived}})).into_response(),
        Ok(None) => not_found(),
        Err(_) => internal(),
    }
}

pub async fn update_setup(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(input): Json<SetupWrite>,
) -> Response {
    let claims = match principal(&state, &headers) {
        Ok(c) => c,
        Err(e) => return e.into_response(),
    };
    if input.name.trim().is_empty() || !input.rules_json.is_object() {
        return err(
            StatusCode::BAD_REQUEST,
            "invalid_request",
            "setup name must be non-empty and rules_json must be an object",
        );
    }
    match sqlx::query("UPDATE journal_setups SET name=$3,description=$4,rules_json=$5,updated_at=NOW() WHERE owner_id=$1 AND id=$2 AND archived=FALSE")
        .bind(claims.sub).bind(id).bind(input.name.trim()).bind(input.description).bind(input.rules_json).execute(state.store.pool()).await {
        Ok(r) if r.rows_affected()==1 => StatusCode::NO_CONTENT.into_response(),
        Ok(_) => not_found(),
        Err(_) => err(StatusCode::CONFLICT,"setup_conflict","Setup name may already exist"),
    }
}

pub async fn archive_setup(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Response {
    let claims = match principal(&state, &headers) {
        Ok(c) => c,
        Err(e) => return e.into_response(),
    };
    match sqlx::query("UPDATE journal_setups SET archived=TRUE,updated_at=NOW() WHERE owner_id=$1 AND id=$2 AND archived=FALSE")
        .bind(claims.sub).bind(id).execute(state.store.pool()).await {
        Ok(r) if r.rows_affected()==1 => StatusCode::NO_CONTENT.into_response(),
        Ok(_) => not_found(),
        Err(_) => internal(),
    }
}

fn currency_is_valid(value: &str) -> bool {
    value.len() == 3 && value.chars().all(|c| c.is_ascii_uppercase())
}

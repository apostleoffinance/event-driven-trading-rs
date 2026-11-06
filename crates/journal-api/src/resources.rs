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
    match sqlx::query_as::<_, (String, String, Option<String>, String, String, Decimal, bool)>(
        "SELECT id,name,broker,account_type,currency,initial_balance,archived FROM journal_accounts WHERE owner_id=$1 AND archived=FALSE ORDER BY created_at DESC")
        .bind(claims.sub).fetch_all(state.store.pool()).await {
        Ok(rows) => Json(serde_json::json!({"data":rows.into_iter().map(|(id,name,broker,account_type,currency,initial_balance,archived)|serde_json::json!({
            "id":id,"name":name,"broker":broker,"account_type":account_type,"currency":currency,"initial_balance":initial_balance.to_string(),"archived":archived
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
    {
        return err(
            StatusCode::BAD_REQUEST,
            "invalid_request",
            "name, currency, account_type, or initial_balance is invalid",
        );
    }
    let id = uuid::Uuid::new_v4().to_string();
    match sqlx::query("INSERT INTO journal_accounts(id,owner_id,name,broker,account_type,currency,initial_balance) VALUES($1,$2,$3,$4,$5,$6,$7)")
        .bind(&id).bind(claims.sub).bind(name).bind(input.broker).bind(input.account_type).bind(&currency).bind(input.initial_balance)
        .execute(state.store.pool()).await {
        Ok(_) => (StatusCode::CREATED, Json(serde_json::json!({"data":{"id":id,"name":name,"currency":currency,"initial_balance":input.initial_balance.to_string()}}))).into_response(),
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
    match sqlx::query_as::<_, (String,String,Option<String>,String,String,Decimal,bool)>(
        "SELECT id,name,broker,account_type,currency,initial_balance,archived FROM journal_accounts WHERE owner_id=$1 AND id=$2")
        .bind(claims.sub).bind(id).fetch_optional(state.store.pool()).await {
        Ok(Some((id,name,broker,account_type,currency,initial_balance,archived))) => Json(serde_json::json!({"data":{"id":id,"name":name,"broker":broker,"account_type":account_type,"currency":currency,"initial_balance":initial_balance.to_string(),"archived":archived}})).into_response(),
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

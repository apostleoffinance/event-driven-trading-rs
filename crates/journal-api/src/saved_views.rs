use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use chrono::{DateTime, Utc};
use serde::Deserialize;
use serde_json::Value;

use crate::{
    auth::{ApiAuthError, Claims},
    AppState,
};

#[derive(Debug, Deserialize)]
pub struct ListQuery {
    pub journal_account_id: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct SavedViewWrite {
    pub name: String,
    pub journal_account_id: Option<String>,
    pub filters: Value,
}

fn principal(state: &AppState, headers: &HeaderMap) -> Result<Claims, ApiAuthError> {
    state.auth.authenticate(headers)
}

fn bad_request(message: &str) -> Response {
    (
        StatusCode::BAD_REQUEST,
        Json(serde_json::json!({"error":{"code":"invalid_request","message":message}})),
    )
        .into_response()
}

async fn owns_account(state: &AppState, owner_id: &str, account_id: &str) -> bool {
    sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS(SELECT 1 FROM journal_accounts WHERE owner_id=$1 AND id=$2 AND archived=FALSE)",
    )
    .bind(owner_id)
    .bind(account_id)
    .fetch_one(state.store.pool())
    .await
    .unwrap_or(false)
}

pub async fn list(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<ListQuery>,
) -> Response {
    let claims = match principal(&state, &headers) {
        Ok(claims) => claims,
        Err(error) => return error.into_response(),
    };
    if let Some(account_id) = query.journal_account_id.as_deref() {
        if !owns_account(&state, &claims.sub, account_id).await {
            return (StatusCode::NOT_FOUND, Json(serde_json::json!({"error":{"code":"not_found","message":"Journal account not found"}}))).into_response();
        }
    }
    let scope = query.journal_account_id.unwrap_or_else(|| "*".to_string());
    let rows = sqlx::query_as::<_, (String, String, Option<String>, Value, DateTime<Utc>)>(
        "SELECT id,name,NULLIF(scope_key,'*'),filters,updated_at FROM journal_analytics_saved_views WHERE owner_id=$1 AND scope_key=$2 ORDER BY updated_at DESC,name ASC",
    )
    .bind(claims.sub)
    .bind(scope)
    .fetch_all(state.store.pool())
    .await;
    match rows {
        Ok(rows) => Json(serde_json::json!({"data":rows.into_iter().map(|(id,name,journal_account_id,filters,updated_at)| serde_json::json!({"id":id,"name":name,"journal_account_id":journal_account_id,"filters":filters,"updated_at":updated_at})).collect::<Vec<_>>()})).into_response(),
        Err(_) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error":{"code":"internal_error","message":"Internal server error"}}))).into_response(),
    }
}

pub async fn upsert(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<SavedViewWrite>,
) -> Response {
    let claims = match principal(&state, &headers) {
        Ok(claims) => claims,
        Err(error) => return error.into_response(),
    };
    let name = input.name.trim();
    if name.is_empty() || name.chars().count() > 80 {
        return bad_request("Saved view name must contain between 1 and 80 characters");
    }
    if !input.filters.is_object() {
        return bad_request("Saved view filters must be a JSON object");
    }
    if let Some(account_id) = input.journal_account_id.as_deref() {
        if !owns_account(&state, &claims.sub, account_id).await {
            return (StatusCode::NOT_FOUND, Json(serde_json::json!({"error":{"code":"not_found","message":"Journal account not found"}}))).into_response();
        }
    }
    let scope = input.journal_account_id.unwrap_or_else(|| "*".to_string());
    let row = sqlx::query_as::<_, (String, String, Option<String>, Value, DateTime<Utc>)>(
        "INSERT INTO journal_analytics_saved_views (id,owner_id,scope_key,name,filters) VALUES ($1,$2,$3,$4,$5) ON CONFLICT (owner_id,scope_key,name) DO UPDATE SET filters=EXCLUDED.filters,updated_at=NOW() RETURNING id,name,NULLIF(scope_key,'*'),filters,updated_at",
    )
    .bind(uuid::Uuid::new_v4().to_string())
    .bind(claims.sub)
    .bind(scope)
    .bind(name)
    .bind(sqlx::types::Json(input.filters))
    .fetch_one(state.store.pool())
    .await;
    match row {
        Ok((id,name,journal_account_id,filters,updated_at)) => (
            StatusCode::OK,
            Json(serde_json::json!({"data":{"id":id,"name":name,"journal_account_id":journal_account_id,"filters":filters,"updated_at":updated_at}})),
        ).into_response(),
        Err(_) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error":{"code":"internal_error","message":"Internal server error"}}))).into_response(),
    }
}

pub async fn delete(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Response {
    let claims = match principal(&state, &headers) {
        Ok(claims) => claims,
        Err(error) => return error.into_response(),
    };
    match sqlx::query("DELETE FROM journal_analytics_saved_views WHERE owner_id=$1 AND id=$2")
        .bind(claims.sub)
        .bind(id)
        .execute(state.store.pool())
        .await
    {
        Ok(result) if result.rows_affected() == 1 => StatusCode::NO_CONTENT.into_response(),
        Ok(_) => (StatusCode::NOT_FOUND, Json(serde_json::json!({"error":{"code":"not_found","message":"Saved view not found"}}))).into_response(),
        Err(_) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error":{"code":"internal_error","message":"Internal server error"}}))).into_response(),
    }
}

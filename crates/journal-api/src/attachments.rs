//! Owner-scoped screenshot/attachment metadata API.
//! Binary uploads are performed by the configured object-storage service; this
//! API stores and authorizes attachment references only.
use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use chrono::{DateTime, Utc};
use serde::Deserialize;
use sqlx::Row;

use crate::AppState;

#[derive(Debug, Deserialize)]
pub struct AttachmentWrite {
    pub attachment_type: Option<String>,
    pub storage_provider: String,
    pub storage_key: String,
    pub original_filename: Option<String>,
    pub content_type: Option<String>,
    pub byte_size: Option<i64>,
    pub captured_at: Option<DateTime<Utc>>,
    #[serde(default)]
    pub metadata: serde_json::Value,
}

fn error(code: &str, message: &str, status: StatusCode) -> Response {
    (
        status,
        Json(serde_json::json!({"error":{"code":code,"message":message}})),
    )
        .into_response()
}

pub async fn list(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(trade_id): Path<String>,
) -> Response {
    let claims = match state.auth.authenticate(&headers) {
        Ok(c) => c,
        Err(e) => return e.into_response(),
    };
    match sqlx::query("SELECT id,attachment_type,storage_provider,storage_key,original_filename,content_type,byte_size,captured_at,metadata,created_at FROM journal_trade_attachments WHERE owner_id=$1 AND trade_id=$2 ORDER BY captured_at DESC NULLS LAST,created_at DESC")
        .bind(&claims.sub).bind(&trade_id).fetch_all(state.store.pool()).await {
        Ok(rows) => Json(serde_json::json!({"data":rows.into_iter().map(|r|serde_json::json!({
            "id":r.get::<String,_>("id"),"attachment_type":r.get::<String,_>("attachment_type"),
            "storage_provider":r.get::<String,_>("storage_provider"),"storage_key":r.get::<String,_>("storage_key"),
            "original_filename":r.get::<Option<String>,_>("original_filename"),"content_type":r.get::<Option<String>,_>("content_type"),
            "byte_size":r.get::<Option<i64>,_>("byte_size"),"captured_at":r.get::<Option<DateTime<Utc>>,_>("captured_at"),
            "metadata":r.get::<serde_json::Value,_>("metadata"),"created_at":r.get::<DateTime<Utc>,_>("created_at")
        })).collect::<Vec<_>>()})).into_response(),
        Err(_) => error("internal_error","Unable to list trade attachments",StatusCode::INTERNAL_SERVER_ERROR),
    }
}

pub async fn create(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(trade_id): Path<String>,
    Json(input): Json<AttachmentWrite>,
) -> Response {
    let claims = match state.auth.authenticate(&headers) {
        Ok(c) => c,
        Err(e) => return e.into_response(),
    };
    if input.storage_key.trim().is_empty()
        || input.storage_key.len() > 4096
        || input.byte_size.is_some_and(|v| v < 0)
    {
        return error(
            "invalid_attachment",
            "Storage key must be non-empty and byte_size cannot be negative",
            StatusCode::BAD_REQUEST,
        );
    }
    if !["r2", "s3", "external", "local"].contains(&input.storage_provider.as_str()) {
        return error(
            "invalid_attachment",
            "storage_provider must be r2, s3, external, or local",
            StatusCode::BAD_REQUEST,
        );
    }
    if input.storage_provider == "external"
        && !(input.storage_key.starts_with("https://") || input.storage_key.starts_with("http://"))
    {
        return error(
            "invalid_attachment",
            "External attachments must use an HTTP(S) URL",
            StatusCode::BAD_REQUEST,
        );
    }
    if !input.metadata.is_object() {
        return error(
            "invalid_attachment",
            "metadata must be a JSON object",
            StatusCode::BAD_REQUEST,
        );
    }
    let owned = sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS(SELECT 1 FROM journal_trades WHERE owner_id=$1 AND id=$2)",
    )
    .bind(&claims.sub)
    .bind(&trade_id)
    .fetch_one(state.store.pool())
    .await
    .unwrap_or(false);
    if !owned {
        return error(
            "not_found",
            "Journal trade not found",
            StatusCode::NOT_FOUND,
        );
    }
    let id = uuid::Uuid::new_v4().to_string();
    match sqlx::query("INSERT INTO journal_trade_attachments(id,owner_id,trade_id,attachment_type,storage_provider,storage_key,original_filename,content_type,byte_size,captured_at,metadata) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11)")
        .bind(&id).bind(&claims.sub).bind(&trade_id).bind(input.attachment_type.unwrap_or_else(||"screenshot".into()))
        .bind(&input.storage_provider).bind(&input.storage_key).bind(&input.original_filename).bind(&input.content_type)
        .bind(input.byte_size).bind(input.captured_at).bind(input.metadata).execute(state.store.pool()).await {
        Ok(_) => (StatusCode::CREATED,Json(serde_json::json!({"data":{"id":id,"trade_id":trade_id,"saved":true}}))).into_response(),
        Err(_) => error("internal_error","Unable to save attachment metadata",StatusCode::INTERNAL_SERVER_ERROR),
    }
}

pub async fn delete(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((trade_id, attachment_id)): Path<(String, String)>,
) -> Response {
    let claims = match state.auth.authenticate(&headers) {
        Ok(c) => c,
        Err(e) => return e.into_response(),
    };
    match sqlx::query(
        "DELETE FROM journal_trade_attachments WHERE owner_id=$1 AND trade_id=$2 AND id=$3",
    )
    .bind(&claims.sub)
    .bind(&trade_id)
    .bind(&attachment_id)
    .execute(state.store.pool())
    .await
    {
        Ok(result) if result.rows_affected() == 1 => StatusCode::NO_CONTENT.into_response(),
        Ok(_) => error("not_found", "Attachment not found", StatusCode::NOT_FOUND),
        Err(_) => error(
            "internal_error",
            "Unable to delete attachment",
            StatusCode::INTERNAL_SERVER_ERROR,
        ),
    }
}

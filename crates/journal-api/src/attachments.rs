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
use std::{env, time::Duration};
use aws_credential_types::Credentials;
use aws_sdk_s3::{config::Region, presigning::PresigningConfig, Client};
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


#[derive(Debug, Deserialize)]
pub struct UploadUrlWrite {
    pub original_filename: String,
    pub content_type: String,
    pub byte_size: i64,
    pub attachment_type: Option<String>,
}

async fn object_store() -> Result<(Client, String), String> {
    let endpoint = env::var("JOURNAL_S3_ENDPOINT_URL").map_err(|_| "JOURNAL_S3_ENDPOINT_URL is not configured")?;
    let access = env::var("JOURNAL_S3_ACCESS_KEY_ID").map_err(|_| "JOURNAL_S3_ACCESS_KEY_ID is not configured")?;
    let secret = env::var("JOURNAL_S3_SECRET_ACCESS_KEY").map_err(|_| "JOURNAL_S3_SECRET_ACCESS_KEY is not configured")?;
    let bucket = env::var("JOURNAL_S3_BUCKET").map_err(|_| "JOURNAL_S3_BUCKET is not configured")?;
    let region = env::var("JOURNAL_S3_REGION").unwrap_or_else(|_| "auto".into());
    let credentials = Credentials::new(access, secret, None, None, "journal-env");
    let shared = aws_config::defaults(aws_config::BehaviorVersion::latest())
        .region(Region::new(region))
        .credentials_provider(credentials)
        .load().await;
    let config = aws_sdk_s3::config::Builder::from(&shared)
        .endpoint_url(endpoint)
        .force_path_style(true)
        .build();
    Ok((Client::from_conf(config), bucket))
}

pub async fn create_upload_url(
    State(state): State<AppState>, headers: HeaderMap, Path(trade_id): Path<String>,
    Json(input): Json<UploadUrlWrite>,
) -> Response {
    let claims = match state.auth.authenticate(&headers) { Ok(c) => c, Err(e) => return e.into_response() };
    let ext = match input.content_type.as_str() {
        "image/png" => "png", "image/jpeg" => "jpg", "image/webp" => "webp",
        "image/gif" => "gif", _ => return error("unsupported_image", "Only PNG, JPEG, WebP and GIF screenshots are supported", StatusCode::BAD_REQUEST),
    };
    if input.byte_size <= 0 || input.byte_size > 15 * 1024 * 1024 {
        return error("invalid_image_size", "Screenshot must be between 1 byte and 15 MB", StatusCode::BAD_REQUEST);
    }
    let owned = sqlx::query_scalar::<_, bool>("SELECT EXISTS(SELECT 1 FROM journal_trades WHERE owner_id=$1 AND id=$2)")
        .bind(&claims.sub).bind(&trade_id).fetch_one(state.store.pool()).await.unwrap_or(false);
    if !owned { return error("not_found", "Journal trade not found", StatusCode::NOT_FOUND); }
    let (client, bucket) = match object_store().await {
        Ok(value) => value,
        Err(message) => return error("storage_not_configured", &message, StatusCode::SERVICE_UNAVAILABLE),
    };
    let key = format!("journal/{}/{}/{}.{}", claims.sub, trade_id, uuid::Uuid::new_v4(), ext);
    let config = match PresigningConfig::expires_in(Duration::from_secs(900)) {
        Ok(value) => value, Err(_) => return error("storage_error", "Unable to configure upload expiry", StatusCode::INTERNAL_SERVER_ERROR),
    };
    match client.put_object().bucket(&bucket).key(&key).content_type(&input.content_type).presigned(config).await {
        Ok(request) => Json(serde_json::json!({"data":{"upload_url":request.uri().to_string(),"storage_key":key,"storage_provider":"r2","content_type":input.content_type,"byte_size":input.byte_size,"original_filename":input.original_filename,"attachment_type":input.attachment_type.unwrap_or_else(||"before".into()),"expires_in_seconds":900}})).into_response(),
        Err(_) => error("storage_error", "Unable to create secure screenshot upload URL", StatusCode::BAD_GATEWAY),
    }
}

pub async fn download_url(
    State(state): State<AppState>, headers: HeaderMap, Path((trade_id, attachment_id)): Path<(String, String)>,
) -> Response {
    let claims = match state.auth.authenticate(&headers) { Ok(c) => c, Err(e) => return e.into_response() };
    let row = sqlx::query("SELECT storage_provider,storage_key FROM journal_trade_attachments WHERE owner_id=$1 AND trade_id=$2 AND id=$3")
        .bind(&claims.sub).bind(&trade_id).bind(&attachment_id).fetch_optional(state.store.pool()).await;
    let row = match row { Ok(Some(row)) => row, Ok(None) => return error("not_found","Attachment not found",StatusCode::NOT_FOUND), Err(_) => return error("internal_error","Unable to retrieve attachment",StatusCode::INTERNAL_SERVER_ERROR) };
    let provider: String = row.get("storage_provider");
    let key: String = row.get("storage_key");
    if provider == "external" { return Json(serde_json::json!({"data":{"download_url":key,"expires_in_seconds":0}})).into_response(); }
    let (client,bucket) = match object_store().await { Ok(v)=>v, Err(message)=>return error("storage_not_configured",&message,StatusCode::SERVICE_UNAVAILABLE) };
    let config = match PresigningConfig::expires_in(Duration::from_secs(900)) { Ok(v)=>v, Err(_)=>return error("storage_error","Unable to configure download expiry",StatusCode::INTERNAL_SERVER_ERROR) };
    match client.get_object().bucket(bucket).key(key).presigned(config).await {
        Ok(request)=>Json(serde_json::json!({"data":{"download_url":request.uri().to_string(),"expires_in_seconds":900}})).into_response(),
        Err(_)=>error("storage_error","Unable to create secure screenshot download URL",StatusCode::BAD_GATEWAY),
    }
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
    if input.attachment_type.as_deref().is_some_and(|kind| !["before", "after", "screenshot", "execution", "other"].contains(&kind)) {
        return error("invalid_attachment_type", "attachment_type must be before, after, screenshot, execution or other", StatusCode::BAD_REQUEST);
    }
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
    if input.storage_provider == "r2" || input.storage_provider == "s3" {
        let (client, bucket) = match object_store().await {
            Ok(value) => value,
            Err(message) => return error("storage_not_configured", &message, StatusCode::SERVICE_UNAVAILABLE),
        };
        let object = match client.head_object().bucket(bucket).key(&input.storage_key).send().await {
            Ok(value) => value,
            Err(_) => return error("upload_not_found", "Uploaded screenshot was not found in private object storage", StatusCode::BAD_REQUEST),
        };
        let actual_size = object.content_length().unwrap_or_default();
        let actual_type = object.content_type().unwrap_or_default();
        if actual_size <= 0 || actual_size > 15 * 1024 * 1024 ||
           !actual_type.starts_with("image/") ||
           input.byte_size.is_some_and(|reported| reported != actual_size) ||
           input.content_type.as_deref().is_some_and(|reported| reported != actual_type) {
            return error("upload_validation_failed", "Stored screenshot size or content type does not match the upload metadata", StatusCode::BAD_REQUEST);
        }
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
    match sqlx::query("INSERT INTO journal_trade_attachments(id,owner_id,trade_id,attachment_type,storage_provider,storage_key,original_filename,content_type,byte_size,captured_at,metadata) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11) ON CONFLICT (owner_id,storage_provider,storage_key) DO NOTHING")
        .bind(&id).bind(&claims.sub).bind(&trade_id).bind(input.attachment_type.unwrap_or_else(||"screenshot".into()))
        .bind(&input.storage_provider).bind(&input.storage_key).bind(&input.original_filename).bind(&input.content_type)
        .bind(input.byte_size).bind(input.captured_at).bind(input.metadata).execute(state.store.pool()).await {
        Ok(result) if result.rows_affected() == 1 => (StatusCode::CREATED,Json(serde_json::json!({"data":{"id":id,"trade_id":trade_id,"saved":true}}))).into_response(),
        Ok(_) => match sqlx::query_as::<_,(String,String)>("SELECT id,trade_id FROM journal_trade_attachments WHERE owner_id=$1 AND storage_provider=$2 AND storage_key=$3")
            .bind(&claims.sub).bind(&input.storage_provider).bind(&input.storage_key).fetch_optional(state.store.pool()).await {
            Ok(Some((existing_id,existing_trade))) if existing_trade == trade_id => (StatusCode::OK,Json(serde_json::json!({"data":{"id":existing_id,"trade_id":trade_id,"saved":true,"already_imported":true}}))).into_response(),
            Ok(Some(_)) => error("attachment_conflict","Attachment key already belongs to a different trade",StatusCode::CONFLICT),
            _ => error("internal_error","Unable to resolve attachment metadata",StatusCode::INTERNAL_SERVER_ERROR),
        },
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
    let existing = sqlx::query("SELECT storage_provider,storage_key FROM journal_trade_attachments WHERE owner_id=$1 AND trade_id=$2 AND id=$3")
        .bind(&claims.sub).bind(&trade_id).bind(&attachment_id)
        .fetch_optional(state.store.pool()).await;
    let existing = match existing {
        Ok(Some(row)) => row,
        Ok(None) => return error("not_found", "Attachment not found", StatusCode::NOT_FOUND),
        Err(_) => return error("internal_error", "Unable to retrieve attachment", StatusCode::INTERNAL_SERVER_ERROR),
    };
    let provider: String = existing.get("storage_provider");
    let key: String = existing.get("storage_key");
    if provider == "r2" || provider == "s3" {
        let (client,bucket) = match object_store().await {
            Ok(value) => value,
            Err(message) => return error("storage_not_configured", &message, StatusCode::SERVICE_UNAVAILABLE),
        };
        if client.delete_object().bucket(bucket).key(key).send().await.is_err() {
            return error("storage_error", "Unable to delete screenshot from object storage", StatusCode::BAD_GATEWAY);
        }
    }
    match sqlx::query("DELETE FROM journal_trade_attachments WHERE owner_id=$1 AND trade_id=$2 AND id=$3")
        .bind(&claims.sub).bind(&trade_id).bind(&attachment_id).execute(state.store.pool()).await {
        Ok(result) if result.rows_affected() == 1 => StatusCode::NO_CONTENT.into_response(),
        Ok(_) => error("not_found", "Attachment not found", StatusCode::NOT_FOUND),
        Err(_) => error("internal_error", "Unable to delete attachment metadata", StatusCode::INTERNAL_SERVER_ERROR),
    }
}

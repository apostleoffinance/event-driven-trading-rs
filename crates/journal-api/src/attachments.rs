//! Owner-scoped screenshot metadata and private S3-compatible object operations.

use std::{env, time::Duration};

use axum::{
    extract::{Path, Query, State},
    body::Bytes,
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use chrono::{DateTime, Utc};
use hmac::{Hmac, Mac};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use sqlx::Row;
use url::Url;

use crate::AppState;

type HmacSha256 = Hmac<Sha256>;

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
pub struct UploadQuery {
    pub original_filename: String,
    pub attachment_type: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UploadUrlWrite {
    pub original_filename: String,
    pub content_type: String,
    pub byte_size: i64,
    pub attachment_type: Option<String>,
}

struct ObjectStoreConfig {
    endpoint: Url,
    access_key: String,
    secret_key: String,
    bucket: String,
    region: String,
}

fn object_store() -> Result<ObjectStoreConfig, String> {
    let endpoint = env::var("JOURNAL_S3_ENDPOINT_URL")
        .map_err(|_| "JOURNAL_S3_ENDPOINT_URL is not configured")?;
    let endpoint = Url::parse(&endpoint).map_err(|_| "JOURNAL_S3_ENDPOINT_URL is invalid")?;
    if endpoint.scheme() != "https"
        || endpoint.query().is_some()
        || !endpoint.username().is_empty()
        || endpoint.password().is_some()
    {
        return Err(
            "JOURNAL_S3_ENDPOINT_URL must use HTTPS and must not contain credentials or a query string"
                .into(),
        );
    }

    let access_key = env::var("JOURNAL_S3_ACCESS_KEY_ID")
        .map_err(|_| "JOURNAL_S3_ACCESS_KEY_ID is not configured")?;
    let secret_key = env::var("JOURNAL_S3_SECRET_ACCESS_KEY")
        .map_err(|_| "JOURNAL_S3_SECRET_ACCESS_KEY is not configured")?;
    let bucket =
        env::var("JOURNAL_S3_BUCKET").map_err(|_| "JOURNAL_S3_BUCKET is not configured")?;
    let region = env::var("JOURNAL_S3_REGION").unwrap_or_else(|_| "auto".into());

    Ok(ObjectStoreConfig {
        endpoint,
        access_key,
        secret_key,
        bucket,
        region,
    })
}

fn encode_component(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'~') {
            out.push(byte as char);
        } else {
            out.push_str(&format!("%{byte:02X}"));
        }
    }
    out
}

fn hmac_bytes(key: &[u8], value: &str) -> Result<Vec<u8>, String> {
    let mut mac = HmacSha256::new_from_slice(key).map_err(|_| "Invalid signing key")?;
    mac.update(value.as_bytes());
    Ok(mac.finalize().into_bytes().to_vec())
}

fn presigned_url(
    config: &ObjectStoreConfig,
    method: &str,
    key: &str,
    expires: u32,
) -> Result<String, String> {
    let host = config
        .endpoint
        .host_str()
        .ok_or("Object storage endpoint has no host")?;
    let host_header = match config.endpoint.port() {
        Some(port) => format!("{host}:{port}"),
        None => host.to_string(),
    };
    let prefix = config.endpoint.path().trim_end_matches('/');
    let encoded_key = key
        .split('/')
        .map(encode_component)
        .collect::<Vec<_>>()
        .join("/");
    let canonical_uri = format!(
        "{prefix}/{}/{}",
        encode_component(&config.bucket),
        encoded_key
    );

    let now = Utc::now();
    let date = now.format("%Y%m%d").to_string();
    let timestamp = now.format("%Y%m%dT%H%M%SZ").to_string();
    let scope = format!("{date}/{}/s3/aws4_request", config.region);
    let credential = format!("{}/{scope}", config.access_key);
    let mut params = [
        (
            "X-Amz-Algorithm".to_string(),
            "AWS4-HMAC-SHA256".to_string(),
        ),
        (
            "X-Amz-Credential".to_string(),
            encode_component(&credential),
        ),
        ("X-Amz-Date".to_string(), timestamp.clone()),
        ("X-Amz-Expires".to_string(), expires.to_string()),
        ("X-Amz-SignedHeaders".to_string(), "host".to_string()),
    ];
    params.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)));
    let canonical_query = params
        .iter()
        .map(|(key, value)| format!("{}={}", encode_component(key), value))
        .collect::<Vec<_>>()
        .join("&");
    let canonical_request = format!(
        "{method}\n{canonical_uri}\n{canonical_query}\nhost:{host_header}\n\nhost\nUNSIGNED-PAYLOAD"
    );
    let request_hash = hex::encode(Sha256::digest(canonical_request.as_bytes()));
    let string_to_sign = format!("AWS4-HMAC-SHA256\n{timestamp}\n{scope}\n{request_hash}");

    let k_date = hmac_bytes(format!("AWS4{}", config.secret_key).as_bytes(), &date)?;
    let k_region = hmac_bytes(&k_date, &config.region)?;
    let k_service = hmac_bytes(&k_region, "s3")?;
    let k_signing = hmac_bytes(&k_service, "aws4_request")?;
    let signature = hex::encode(hmac_bytes(&k_signing, &string_to_sign)?);

    Ok(format!(
        "{}://{}{}?{}&X-Amz-Signature={signature}",
        config.endpoint.scheme(),
        host_header,
        canonical_uri,
        canonical_query
    ))
}

async fn signed_object_request(method: &str, key: &str) -> Result<reqwest::Response, String> {
    let config = object_store()?;
    let url = presigned_url(&config, method, key, 900)?;
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(20))
        .build()
        .map_err(|_| "Unable to create object storage HTTP client")?;
    let request = match method {
        "HEAD" => client.head(url),
        "DELETE" => client.delete(url),
        "GET" => client.get(url),
        _ => return Err("Unsupported object storage method".into()),
    };
    request
        .send()
        .await
        .map_err(|_| "Object storage request failed".into())
}

fn error(code: &str, message: &str, status: StatusCode) -> Response {
    (
        status,
        Json(serde_json::json!({"error":{"code":code,"message":message}})),
    )
        .into_response()
}


/// Upload a screenshot through the Journal API instead of sending the browser
/// directly to R2. This avoids browser-to-object-store CORS failures while
/// keeping R2 credentials and the signed URL entirely server-side.
pub async fn upload(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(trade_id): Path<String>,
    Query(input): Query<UploadQuery>,
    body: Bytes,
) -> Response {
    let claims = match state.auth.authenticate(&headers) {
        Ok(claims) => claims,
        Err(error) => return error.into_response(),
    };
    let content_type = match headers
        .get(axum::http::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
    {
        Some(value) => value.split(';').next().unwrap_or("").trim(),
        None => "",
    };
    let extension = match content_type {
        "image/png" => "png",
        "image/jpeg" => "jpg",
        "image/webp" => "webp",
        "image/gif" => "gif",
        _ => {
            return error(
                "unsupported_image",
                "Only PNG, JPEG, WebP and GIF screenshots are supported",
                StatusCode::BAD_REQUEST,
            )
        }
    };
    if body.is_empty() || body.len() > 15 * 1024 * 1024 {
        return error(
            "invalid_image_size",
            "Screenshot must be between 1 byte and 15 MB",
            StatusCode::BAD_REQUEST,
        );
    }
    if input.original_filename.trim().is_empty() || input.original_filename.len() > 255 {
        return error(
            "invalid_filename",
            "Screenshot filename must contain between 1 and 255 characters",
            StatusCode::BAD_REQUEST,
        );
    }
    if input
        .attachment_type
        .as_deref()
        .is_some_and(|kind| !["before", "after"].contains(&kind))
    {
        return error(
            "invalid_attachment_type",
            "attachment_type must be before or after",
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

    let config = match object_store() {
        Ok(config) => config,
        Err(message) => {
            return error(
                "storage_not_configured",
                &message,
                StatusCode::SERVICE_UNAVAILABLE,
            )
        }
    };
    let key = format!(
        "journal/{}/{}/{}.{}",
        claims.sub,
        trade_id,
        uuid::Uuid::new_v4(),
        extension
    );
    let upload_url = match presigned_url(&config, "PUT", &key, 900) {
        Ok(url) => url,
        Err(_) => {
            return error(
                "storage_error",
                "Unable to create secure screenshot upload request",
                StatusCode::INTERNAL_SERVER_ERROR,
            )
        }
    };
    let client = match reqwest::Client::builder()
        .timeout(Duration::from_secs(45))
        .build()
    {
        Ok(client) => client,
        Err(_) => {
            return error(
                "storage_error",
                "Unable to create object storage HTTP client",
                StatusCode::INTERNAL_SERVER_ERROR,
            )
        }
    };
    let uploaded = match client
        .put(upload_url)
        .header(reqwest::header::CONTENT_TYPE, content_type)
        .body(body.clone())
        .send()
        .await
    {
        Ok(response) => response,
        Err(_) => {
            return error(
                "storage_unreachable",
                "Journal API could not connect to Cloudflare R2. Check the R2 endpoint and Render outbound connectivity.",
                StatusCode::BAD_GATEWAY,
            )
        }
    };
    if !uploaded.status().is_success() {
        let status = uploaded.status();
        tracing::warn!(%status, "R2 screenshot upload was rejected");
        return error(
            "storage_rejected",
            "Cloudflare R2 rejected the screenshot upload. Verify the R2 endpoint, token permissions and bucket.",
            StatusCode::BAD_GATEWAY,
        );
    }

    let verified = match signed_object_request("HEAD", &key).await {
        Ok(response) => response,
        Err(_) => {
            return error(
                "storage_verification_failed",
                "Screenshot was uploaded but the Journal API could not verify it in R2.",
                StatusCode::BAD_GATEWAY,
            )
        }
    };
    if !verified.status().is_success() {
        return error(
            "storage_verification_failed",
            "Screenshot upload completed but the object could not be verified in R2.",
            StatusCode::BAD_GATEWAY,
        );
    }

    let id = uuid::Uuid::new_v4().to_string();
    let byte_size = body.len() as i64;
    let attachment_type = input.attachment_type.unwrap_or_else(|| "before".into());
    match sqlx::query(
        "INSERT INTO journal_trade_attachments(id,owner_id,trade_id,attachment_type,storage_provider,storage_key,original_filename,content_type,byte_size,metadata) VALUES($1,$2,$3,$4,'r2',$5,$6,$7,$8,$9)",
    )
    .bind(&id)
    .bind(&claims.sub)
    .bind(&trade_id)
    .bind(&attachment_type)
    .bind(&key)
    .bind(&input.original_filename)
    .bind(content_type)
    .bind(byte_size)
    .bind(serde_json::json!({"upload_method":"journal_api_proxy"}))
    .execute(state.store.pool())
    .await
    {
        Ok(_) => (
            StatusCode::CREATED,
            Json(serde_json::json!({
                "data": {
                    "id": id,
                    "trade_id": trade_id,
                    "saved": true,
                    "storage_provider": "r2",
                    "original_filename": input.original_filename,
                    "byte_size": byte_size
                }
            })),
        )
            .into_response(),
        Err(_) => {
            // Avoid leaving an orphaned private object if metadata persistence fails.
            let _ = signed_object_request("DELETE", &key).await;
            error(
                "internal_error",
                "Screenshot was uploaded but its attachment record could not be saved.",
                StatusCode::INTERNAL_SERVER_ERROR,
            )
        }
    }
}

pub async fn create_upload_url(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(trade_id): Path<String>,
    Json(input): Json<UploadUrlWrite>,
) -> Response {
    let claims = match state.auth.authenticate(&headers) {
        Ok(claims) => claims,
        Err(error) => return error.into_response(),
    };
    let extension = match input.content_type.as_str() {
        "image/png" => "png",
        "image/jpeg" => "jpg",
        "image/webp" => "webp",
        "image/gif" => "gif",
        _ => {
            return error(
                "unsupported_image",
                "Only PNG, JPEG, WebP and GIF screenshots are supported",
                StatusCode::BAD_REQUEST,
            )
        }
    };
    if input.byte_size <= 0 || input.byte_size > 15 * 1024 * 1024 {
        return error(
            "invalid_image_size",
            "Screenshot must be between 1 byte and 15 MB",
            StatusCode::BAD_REQUEST,
        );
    }
    if input
        .attachment_type
        .as_deref()
        .is_some_and(|kind| !["before", "after"].contains(&kind))
    {
        return error(
            "invalid_attachment_type",
            "attachment_type must be before or after",
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

    let config = match object_store() {
        Ok(config) => config,
        Err(message) => {
            return error(
                "storage_not_configured",
                &message,
                StatusCode::SERVICE_UNAVAILABLE,
            )
        }
    };
    let key = format!(
        "journal/{}/{}/{}.{}",
        claims.sub,
        trade_id,
        uuid::Uuid::new_v4(),
        extension
    );
    match presigned_url(&config, "PUT", &key, 900) {
        Ok(upload_url) => Json(serde_json::json!({
            "data": {
                "upload_url": upload_url,
                "storage_key": key,
                "storage_provider": "r2",
                "content_type": input.content_type,
                "byte_size": input.byte_size,
                "original_filename": input.original_filename,
                "attachment_type": input.attachment_type.unwrap_or_else(|| "before".into()),
                "expires_in_seconds": 900
            }
        }))
        .into_response(),
        Err(_) => error(
            "storage_error",
            "Unable to create secure screenshot upload URL",
            StatusCode::INTERNAL_SERVER_ERROR,
        ),
    }
}

pub async fn download_url(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((trade_id, attachment_id)): Path<(String, String)>,
) -> Response {
    let claims = match state.auth.authenticate(&headers) {
        Ok(claims) => claims,
        Err(error) => return error.into_response(),
    };
    let row = sqlx::query(
        "SELECT storage_provider,storage_key FROM journal_trade_attachments WHERE owner_id=$1 AND trade_id=$2 AND id=$3",
    )
    .bind(&claims.sub)
    .bind(&trade_id)
    .bind(&attachment_id)
    .fetch_optional(state.store.pool())
    .await;
    let row = match row {
        Ok(Some(row)) => row,
        Ok(None) => return error("not_found", "Attachment not found", StatusCode::NOT_FOUND),
        Err(_) => {
            return error(
                "internal_error",
                "Unable to retrieve attachment",
                StatusCode::INTERNAL_SERVER_ERROR,
            )
        }
    };
    let provider: String = row.get("storage_provider");
    let key: String = row.get("storage_key");
    if provider == "external" {
        return Json(serde_json::json!({
            "data": {"download_url": key, "expires_in_seconds": 0}
        }))
        .into_response();
    }

    let config = match object_store() {
        Ok(config) => config,
        Err(message) => {
            return error(
                "storage_not_configured",
                &message,
                StatusCode::SERVICE_UNAVAILABLE,
            )
        }
    };
    match presigned_url(&config, "GET", &key, 900) {
        Ok(url) => Json(serde_json::json!({
            "data": {"download_url": url, "expires_in_seconds": 900}
        }))
        .into_response(),
        Err(_) => error(
            "storage_error",
            "Unable to create secure screenshot download URL",
            StatusCode::INTERNAL_SERVER_ERROR,
        ),
    }
}

pub async fn list(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(trade_id): Path<String>,
) -> Response {
    let claims = match state.auth.authenticate(&headers) {
        Ok(claims) => claims,
        Err(error) => return error.into_response(),
    };
    match sqlx::query(
        "SELECT id,attachment_type,storage_provider,storage_key,original_filename,content_type,byte_size,captured_at,metadata,created_at FROM journal_trade_attachments WHERE owner_id=$1 AND trade_id=$2 ORDER BY captured_at DESC NULLS LAST,created_at DESC",
    )
    .bind(&claims.sub)
    .bind(&trade_id)
    .fetch_all(state.store.pool())
    .await
    {
        Ok(rows) => Json(serde_json::json!({
            "data": rows.into_iter().map(|row| serde_json::json!({
                "id": row.get::<String, _>("id"),
                "attachment_type": row.get::<String, _>("attachment_type"),
                "storage_provider": row.get::<String, _>("storage_provider"),
                "storage_key": row.get::<String, _>("storage_key"),
                "original_filename": row.get::<Option<String>, _>("original_filename"),
                "content_type": row.get::<Option<String>, _>("content_type"),
                "byte_size": row.get::<Option<i64>, _>("byte_size"),
                "captured_at": row.get::<Option<DateTime<Utc>>, _>("captured_at"),
                "metadata": row.get::<serde_json::Value, _>("metadata"),
                "created_at": row.get::<DateTime<Utc>, _>("created_at")
            })).collect::<Vec<_>>()
        }))
        .into_response(),
        Err(_) => error(
            "internal_error",
            "Unable to list trade attachments",
            StatusCode::INTERNAL_SERVER_ERROR,
        ),
    }
}

pub async fn create(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(trade_id): Path<String>,
    Json(input): Json<AttachmentWrite>,
) -> Response {
    let claims = match state.auth.authenticate(&headers) {
        Ok(claims) => claims,
        Err(error) => return error.into_response(),
    };
    if input.attachment_type.as_deref().is_some_and(|kind| {
        !["before", "after", "screenshot", "execution", "other"].contains(&kind)
    }) {
        return error(
            "invalid_attachment_type",
            "attachment_type must be before, after, screenshot, execution or other",
            StatusCode::BAD_REQUEST,
        );
    }
    if input.storage_key.trim().is_empty()
        || input.storage_key.len() > 4096
        || input.byte_size.is_some_and(|value| value < 0)
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

    if input.storage_provider == "r2" || input.storage_provider == "s3" {
        let expected_prefix = format!("journal/{}/{}/", claims.sub, trade_id);
        if !input.storage_key.starts_with(&expected_prefix) {
            return error(
                "invalid_storage_key",
                "Private screenshot keys must be scoped to the authenticated owner and trade",
                StatusCode::FORBIDDEN,
            );
        }
        let response = match signed_object_request("HEAD", &input.storage_key).await {
            Ok(response) => response,
            Err(message) => return error("storage_error", &message, StatusCode::BAD_GATEWAY),
        };
        if !response.status().is_success() {
            return error(
                "upload_not_found",
                "Uploaded screenshot was not found in private object storage",
                StatusCode::BAD_REQUEST,
            );
        }
        let actual_size = response.content_length().unwrap_or_default() as i64;
        let actual_type = response
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .unwrap_or("");
        if actual_size <= 0
            || actual_size > 15 * 1024 * 1024
            || !actual_type.starts_with("image/")
            || input.byte_size.is_some_and(|value| value != actual_size)
            || input
                .content_type
                .as_deref()
                .is_some_and(|value| value != actual_type)
        {
            return error(
                "upload_validation_failed",
                "Stored screenshot size or content type does not match upload metadata",
                StatusCode::BAD_REQUEST,
            );
        }
    }

    let id = uuid::Uuid::new_v4().to_string();
    match sqlx::query(
        "INSERT INTO journal_trade_attachments(id,owner_id,trade_id,attachment_type,storage_provider,storage_key,original_filename,content_type,byte_size,captured_at,metadata) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11) ON CONFLICT (owner_id,storage_provider,storage_key) DO NOTHING",
    )
    .bind(&id)
    .bind(&claims.sub)
    .bind(&trade_id)
    .bind(input.attachment_type.unwrap_or_else(|| "screenshot".into()))
    .bind(&input.storage_provider)
    .bind(&input.storage_key)
    .bind(&input.original_filename)
    .bind(&input.content_type)
    .bind(input.byte_size)
    .bind(input.captured_at)
    .bind(input.metadata)
    .execute(state.store.pool())
    .await
    {
        Ok(result) if result.rows_affected() == 1 => (
            StatusCode::CREATED,
            Json(serde_json::json!({"data":{"id":id,"trade_id":trade_id,"saved":true}})),
        )
            .into_response(),
        Ok(_) => match sqlx::query_as::<_, (String, String)>(
            "SELECT id,trade_id FROM journal_trade_attachments WHERE owner_id=$1 AND storage_provider=$2 AND storage_key=$3",
        )
        .bind(&claims.sub)
        .bind(&input.storage_provider)
        .bind(&input.storage_key)
        .fetch_optional(state.store.pool())
        .await
        {
            Ok(Some((existing_id, existing_trade))) if existing_trade == trade_id => (
                StatusCode::OK,
                Json(serde_json::json!({
                    "data":{"id":existing_id,"trade_id":trade_id,"saved":true,"already_imported":true}
                })),
            )
                .into_response(),
            Ok(Some(_)) => error(
                "attachment_conflict",
                "Attachment key already belongs to a different trade",
                StatusCode::CONFLICT,
            ),
            _ => error(
                "internal_error",
                "Unable to resolve attachment metadata",
                StatusCode::INTERNAL_SERVER_ERROR,
            ),
        },
        Err(_) => error(
            "internal_error",
            "Unable to save attachment metadata",
            StatusCode::INTERNAL_SERVER_ERROR,
        ),
    }
}

pub async fn delete(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((trade_id, attachment_id)): Path<(String, String)>,
) -> Response {
    let claims = match state.auth.authenticate(&headers) {
        Ok(claims) => claims,
        Err(error) => return error.into_response(),
    };
    let existing = sqlx::query(
        "SELECT storage_provider,storage_key FROM journal_trade_attachments WHERE owner_id=$1 AND trade_id=$2 AND id=$3",
    )
    .bind(&claims.sub)
    .bind(&trade_id)
    .bind(&attachment_id)
    .fetch_optional(state.store.pool())
    .await;
    let existing = match existing {
        Ok(Some(row)) => row,
        Ok(None) => return error("not_found", "Attachment not found", StatusCode::NOT_FOUND),
        Err(_) => {
            return error(
                "internal_error",
                "Unable to retrieve attachment",
                StatusCode::INTERNAL_SERVER_ERROR,
            )
        }
    };
    let provider: String = existing.get("storage_provider");
    let key: String = existing.get("storage_key");
    if provider == "r2" || provider == "s3" {
        let response = match signed_object_request("DELETE", &key).await {
            Ok(response) => response,
            Err(message) => return error("storage_error", &message, StatusCode::BAD_GATEWAY),
        };
        if !response.status().is_success() {
            return error(
                "storage_error",
                "Unable to delete screenshot from object storage",
                StatusCode::BAD_GATEWAY,
            );
        }
    }

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
            "Unable to delete attachment metadata",
            StatusCode::INTERNAL_SERVER_ERROR,
        ),
    }
}

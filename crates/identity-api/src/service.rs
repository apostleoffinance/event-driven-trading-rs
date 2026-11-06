use std::{time::{SystemTime, UNIX_EPOCH}, sync::Arc};

use argon2::{
    password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use chrono::{Duration, Utc};
use jsonwebtoken::{decode, encode, Algorithm, DecodingKey, EncodingKey, Header, Validation};
use rand_core::{OsRng, RngCore};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::PgPool;

use crate::AppState;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AccessClaims {
    pub sub: String,
    pub iss: String,
    pub aud: String,
    pub exp: usize,
    pub iat: usize,
    pub token_type: String,
}

pub struct JwtService {
    encoding: EncodingKey,
    decoding: DecodingKey,
    issuer: String,
    audience: String,
    validation: Validation,
}

impl JwtService {
    pub fn new(private_pem: &[u8], public_pem: &[u8], issuer: String, audience: String) -> Result<Self, String> {
        let encoding = EncodingKey::from_rsa_pem(private_pem).map_err(|e| format!("invalid private RSA PEM: {e}"))?;
        let decoding = DecodingKey::from_rsa_pem(public_pem).map_err(|e| format!("invalid public RSA PEM: {e}"))?;
        let mut validation = Validation::new(Algorithm::RS256);
        validation.set_issuer(&[issuer.as_str()]);
        validation.set_audience(&[audience.as_str()]);
        validation.validate_exp = true;
        validation.required_spec_claims.insert("sub".into());
        validation.required_spec_claims.insert("exp".into());
        Ok(Self { encoding, decoding, issuer, audience, validation })
    }

    pub fn issue_access(&self, user_id: &str, ttl_seconds: i64) -> Result<(String, usize), String> {
        let now = SystemTime::now().duration_since(UNIX_EPOCH).map_err(|e| e.to_string())?.as_secs() as usize;
        let exp = now.saturating_add(ttl_seconds.max(1) as usize);
        let claims = AccessClaims {
            sub: user_id.to_owned(), iss: self.issuer.clone(), aud: self.audience.clone(),
            exp, iat: now, token_type: "access".into(),
        };
        let token = encode(&Header::new(Algorithm::RS256), &claims, &self.encoding)
            .map_err(|e| format!("JWT signing failed: {e}"))?;
        Ok((token, exp))
    }

    pub fn authenticate(&self, headers: &HeaderMap) -> Result<AccessClaims, AuthError> {
        let bearer = headers.get(axum::http::header::AUTHORIZATION)
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.strip_prefix("Bearer "))
            .ok_or(AuthError)?;
        let claims = decode::<AccessClaims>(bearer, &self.decoding, &self.validation)
            .map_err(|_| AuthError)?.claims;
        if claims.token_type != "access" || claims.sub.trim().is_empty() { return Err(AuthError); }
        Ok(claims)
    }
}

#[derive(Debug)]
pub struct AuthError;
impl IntoResponse for AuthError {
    fn into_response(self) -> Response {
        api_error(StatusCode::UNAUTHORIZED, "unauthorized", "A valid EdgeNest access token is required")
    }
}

#[derive(Debug, Deserialize)]
pub struct RegisterRequest {
    pub email: String,
    pub display_name: String,
    pub password: String,
}
#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    pub email: String,
    pub password: String,
}
#[derive(Debug, Deserialize)]
pub struct RefreshRequest {
    pub refresh_token: String,
}
#[derive(Debug, Serialize)]
struct UserView {
    id: String,
    email: String,
    display_name: String,
}
#[derive(Debug, Serialize)]
struct TokenResponse {
    token_type: &'static str,
    access_token: String,
    expires_in: usize,
    refresh_token: String,
    user: UserView,
}

fn api_error(status: StatusCode, code: &str, message: &str) -> Response {
    (status, Json(serde_json::json!({"error":{"code":code,"message":message}}))).into_response()
}
fn invalid() -> Response { api_error(StatusCode::BAD_REQUEST, "invalid_request", "Invalid request fields") }
fn unauthorized() -> Response { api_error(StatusCode::UNAUTHORIZED, "invalid_credentials", "Invalid email or password") }
fn internal() -> Response { api_error(StatusCode::INTERNAL_SERVER_ERROR, "internal_error", "Internal server error") }
fn conflict() -> Response { api_error(StatusCode::CONFLICT, "email_exists", "An account with this email already exists") }

fn normalize_email(email: &str) -> String { email.trim().to_lowercase() }
fn valid_email(email: &str) -> bool {
    let mut parts = email.split('@');
    matches!((parts.next(), parts.next(), parts.next()), (Some(local), Some(domain), None) if !local.is_empty() && domain.contains('.') && !domain.starts_with('.') && !domain.ends_with('.'))
}
fn hash_password(password: &str) -> Result<String, String> {
    let salt = SaltString::generate(&mut OsRng);
    Argon2::default().hash_password(password.as_bytes(), &salt)
        .map(|hash| hash.to_string()).map_err(|e| e.to_string())
}
fn verify_password(password: &str, stored_hash: &str) -> bool {
    PasswordHash::new(stored_hash).ok().is_some_and(|hash| Argon2::default().verify_password(password.as_bytes(), &hash).is_ok())
}
fn new_refresh_token() -> String {
    let mut bytes = [0u8; 32];
    OsRng.fill_bytes(&mut bytes);
    hex::encode(bytes)
}
fn token_hash(raw: &str) -> String {
    hex::encode(Sha256::digest(raw.as_bytes()))
}

async fn issue_pair(state: &AppState, user: UserView) -> Result<TokenResponse, Response> {
    let (access_token, expires_at) = state.jwt.issue_access(&user.id, state.access_ttl_seconds).map_err(|_| internal())?;
    let refresh_token = new_refresh_token();
    let expires = Utc::now() + Duration::days(state.refresh_ttl_days);
    sqlx::query("INSERT INTO identity_refresh_tokens(id,user_id,token_hash,expires_at) VALUES($1,$2,$3,$4)")
        .bind(uuid::Uuid::new_v4().to_string()).bind(&user.id).bind(token_hash(&refresh_token)).bind(expires)
        .execute(&state.pool).await.map_err(|_| internal())?;
    Ok(TokenResponse { token_type: "Bearer", access_token, expires_in: expires_at, refresh_token, user })
}

pub async fn health() -> &'static str { "ok" }
pub async fn ready(State(state): State<AppState>) -> Response {
    match sqlx::query_scalar::<_, i32>("SELECT 1").fetch_one(&state.pool).await {
        Ok(_) => Json(serde_json::json!({"status":"ready"})).into_response(),
        Err(_) => api_error(StatusCode::SERVICE_UNAVAILABLE,"not_ready","Database unavailable"),
    }
}

pub async fn register(State(state): State<AppState>, Json(input): Json<RegisterRequest>) -> Response {
    let email = normalize_email(&input.email);
    let display_name = input.display_name.trim();
    if !valid_email(&email) || display_name.is_empty() || input.password.len() < 12 || input.password.len() > 1024 {
        return invalid();
    }
    let password_hash = match hash_password(&input.password) { Ok(hash) => hash, Err(_) => return internal() };
    let user_id = uuid::Uuid::new_v4().to_string();
    let inserted = sqlx::query_as::<_, (String,String,String)>(
        "INSERT INTO identity_users(id,email,display_name,password_hash) VALUES($1,$2,$3,$4) RETURNING id,email,display_name")
        .bind(&user_id).bind(&email).bind(display_name).bind(password_hash).fetch_optional(&state.pool).await;
    let user = match inserted {
        Ok(Some((id,email,display_name))) => UserView { id,email,display_name },
        Ok(None) => return internal(),
        Err(error) if error.as_database_error().is_some_and(|e| e.code().as_deref()==Some("23505")) => return conflict(),
        Err(_) => return internal(),
    };
    match issue_pair(&state, user).await {
        Ok(tokens) => (StatusCode::CREATED, Json(tokens)).into_response(),
        Err(response) => response,
    }
}

pub async fn login(State(state): State<AppState>, Json(input): Json<LoginRequest>) -> Response {
    let email = normalize_email(&input.email);
    let row = sqlx::query_as::<_, (String,String,String,String,bool)>(
        "SELECT id,email,display_name,password_hash,is_active FROM identity_users WHERE email=$1")
        .bind(email).fetch_optional(&state.pool).await;
    let (id,email,display_name,password_hash,is_active) = match row {
        Ok(Some(row)) => row,
        Ok(None) => return unauthorized(),
        Err(_) => return internal(),
    };
    if !is_active || !verify_password(&input.password, &password_hash) { return unauthorized(); }
    match issue_pair(&state, UserView { id,email,display_name }).await {
        Ok(tokens) => Json(tokens).into_response(),
        Err(response) => response,
    }
}

pub async fn refresh(State(state): State<AppState>, Json(input): Json<RefreshRequest>) -> Response {
    if input.refresh_token.len() < 32 || input.refresh_token.len() > 256 { return unauthorized(); }
    let mut tx = match state.pool.begin().await { Ok(tx) => tx, Err(_) => return internal() };
    let user_id = sqlx::query_scalar::<_, String>(
        "UPDATE identity_refresh_tokens SET revoked_at=NOW(),last_used_at=NOW() WHERE token_hash=$1 AND revoked_at IS NULL AND expires_at>NOW() RETURNING user_id")
        .bind(token_hash(&input.refresh_token)).fetch_optional(&mut *tx).await;
    let user_id = match user_id {
        Ok(Some(id)) => id,
        Ok(None) => { let _ = tx.rollback().await; return unauthorized(); },
        Err(_) => { let _ = tx.rollback().await; return internal(); },
    };
    let user = sqlx::query_as::<_, (String,String,String,bool)>(
        "SELECT id,email,display_name,is_active FROM identity_users WHERE id=$1")
        .bind(&user_id).fetch_optional(&mut *tx).await;
    let (id,email,display_name,is_active) = match user {
        Ok(Some(row)) => row,
        Ok(None) => { let _ = tx.rollback().await; return unauthorized(); },
        Err(_) => { let _ = tx.rollback().await; return internal(); },
    };
    if !is_active { let _ = tx.rollback().await; return unauthorized(); }
    let (access_token, expires_at) = match state.jwt.issue_access(&id, state.access_ttl_seconds) {
        Ok(token) => token, Err(_) => { let _ = tx.rollback().await; return internal(); },
    };
    let refresh_token = new_refresh_token();
    let expiry = Utc::now() + Duration::days(state.refresh_ttl_days);
    if sqlx::query("INSERT INTO identity_refresh_tokens(id,user_id,token_hash,expires_at) VALUES($1,$2,$3,$4)")
        .bind(uuid::Uuid::new_v4().to_string()).bind(&id).bind(token_hash(&refresh_token)).bind(expiry)
        .execute(&mut *tx).await.is_err() {
        let _ = tx.rollback().await; return internal();
    }
    if tx.commit().await.is_err() { return internal(); }
    Json(TokenResponse {
        token_type: "Bearer", access_token, expires_in: expires_at, refresh_token,
        user: UserView { id,email,display_name },
    }).into_response()
}

pub async fn logout(State(state): State<AppState>, headers: HeaderMap, Json(input): Json<RefreshRequest>) -> Response {
    let claims = match state.jwt.authenticate(&headers) { Ok(claims) => claims, Err(e) => return e.into_response() };
    match sqlx::query("UPDATE identity_refresh_tokens SET revoked_at=NOW() WHERE user_id=$1 AND token_hash=$2 AND revoked_at IS NULL")
        .bind(claims.sub).bind(token_hash(&input.refresh_token)).execute(&state.pool).await {
        Ok(_) => StatusCode::NO_CONTENT.into_response(),
        Err(_) => internal(),
    }
}

pub async fn me(State(state): State<AppState>, headers: HeaderMap) -> Response {
    let claims = match state.jwt.authenticate(&headers) { Ok(claims) => claims, Err(e) => return e.into_response() };
    match sqlx::query_as::<_, (String,String,String,bool)>("SELECT id,email,display_name,is_active FROM identity_users WHERE id=$1")
        .bind(claims.sub).fetch_optional(&state.pool).await {
        Ok(Some((id,email,display_name,true))) => Json(serde_json::json!({"data":{"id":id,"email":email,"display_name":display_name}})).into_response(),
        Ok(_) => unauthorized(),
        Err(_) => internal(),
    }
}

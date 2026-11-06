use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Nonce,
};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use hmac::{Hmac, Mac};
use rsa::{
    pkcs1::{DecodeRsaPrivateKey, DecodeRsaPublicKey},
    pkcs8::{DecodePrivateKey, DecodePublicKey},
    traits::PublicKeyParts,
    RsaPrivateKey, RsaPublicKey,
};
use sha1::Sha1;
use std::{
    collections::HashMap,
    time::{Duration as StdDuration, SystemTime, UNIX_EPOCH},
};

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
    public_keys: HashMap<String, DecodingKey>,
    active_kid: String,
    jwks: serde_json::Value,
    issuer: String,
    audience: String,
    validation: Validation,
}

impl JwtService {
    pub fn new(
        private_pem: &[u8],
        public_pem: &[u8],
        active_kid: String,
        public_keys_json: Option<&str>,
        issuer: String,
        audience: String,
    ) -> Result<Self, String> {
        let encoding = EncodingKey::from_rsa_pem(private_pem)
            .map_err(|e| format!("invalid private RSA PEM: {e}"))?;
        if active_kid.trim().is_empty() {
            return Err("EDGENEST_JWT_KEY_ID cannot be empty".into());
        }
        let decoding = DecodingKey::from_rsa_pem(public_pem)
            .map_err(|e| format!("invalid public RSA PEM: {e}"))?;
        let active_public_pem = std::str::from_utf8(public_pem).map_err(|e| e.to_string())?;
        let active_rsa = match RsaPublicKey::from_public_key_pem(active_public_pem) {
            Ok(key) => key,
            Err(_) => RsaPublicKey::from_pkcs1_pem(active_public_pem)
                .map_err(|e| format!("invalid active RSA public key: {e}"))?,
        };
        if active_rsa.n().bits() < 2048 {
            return Err("RSA signing key must be at least 2048 bits".into());
        }
        let private_pem = std::str::from_utf8(private_pem).map_err(|e| e.to_string())?;
        let private_rsa = match RsaPrivateKey::from_pkcs8_pem(private_pem) {
            Ok(key) => key,
            Err(_) => RsaPrivateKey::from_pkcs1_pem(private_pem)
                .map_err(|e| format!("invalid RSA private key: {e}"))?,
        };
        let derived_public = RsaPublicKey::from(&private_rsa);
        if derived_public.n() != active_rsa.n() || derived_public.e() != active_rsa.e() {
            return Err(
                "EDGENEST_JWT_PRIVATE_KEY_PEM does not match EDGENEST_JWT_PUBLIC_KEY_PEM".into(),
            );
        }
        #[derive(Deserialize)]
        struct KeyInput {
            kid: String,
            pem: String,
        }
        let mut public_keys: HashMap<String, DecodingKey> = HashMap::new();
        let mut jwks_keys = Vec::new();
        let mut inputs = match public_keys_json {
            Some(json) => serde_json::from_str::<Vec<KeyInput>>(json)
                .map_err(|e| format!("invalid EDGENEST_JWT_PUBLIC_KEYS_JSON: {e}"))?,
            None => Vec::new(),
        };
        if !inputs.iter().any(|key| key.kid == active_kid) {
            inputs.push(KeyInput {
                kid: active_kid.clone(),
                pem: String::from_utf8(public_pem.to_vec()).map_err(|e| e.to_string())?,
            });
        }
        for key in inputs {
            let pem = key.pem.replace("\\n", "\n");
            let decoder = DecodingKey::from_rsa_pem(pem.as_bytes())
                .map_err(|e| format!("invalid public PEM for kid {}: {e}", key.kid))?;
            let rsa_key = match RsaPublicKey::from_public_key_pem(&pem) {
                Ok(key) => key,
                Err(_) => RsaPublicKey::from_pkcs1_pem(&pem)
                    .map_err(|e| format!("invalid RSA public key {}: {e}", key.kid))?,
            };
            if rsa_key.n().bits() < 2048 {
                return Err(format!(
                    "RSA public key {} must be at least 2048 bits",
                    key.kid
                ));
            }
            if key.kid == active_kid
                && (rsa_key.n() != active_rsa.n() || rsa_key.e() != active_rsa.e())
            {
                return Err(
                    "active JWT KID public key does not match EDGENEST_JWT_PUBLIC_KEY_PEM".into(),
                );
            }
            if public_keys.contains_key(&key.kid) {
                return Err(format!("duplicate JWT KID: {}", key.kid));
            }
            jwks_keys.push(serde_json::json!({"kty":"RSA","use":"sig","alg":"RS256","kid":key.kid,"n":URL_SAFE_NO_PAD.encode(rsa_key.n().to_bytes_be()),"e":URL_SAFE_NO_PAD.encode(rsa_key.e().to_bytes_be())}));
            public_keys.insert(key.kid, decoder);
        }
        let jwks = serde_json::json!({"keys":jwks_keys});
        let mut validation = Validation::new(Algorithm::RS256);
        validation.set_issuer(&[issuer.as_str()]);
        validation.set_audience(&[audience.as_str()]);
        validation.validate_exp = true;
        validation.required_spec_claims.insert("sub".into());
        validation.required_spec_claims.insert("exp".into());
        Ok(Self {
            encoding,
            decoding,
            public_keys,
            active_kid,
            jwks,
            issuer,
            audience,
            validation,
        })
    }

    pub fn issue_access(&self, user_id: &str, ttl_seconds: i64) -> Result<(String, usize), String> {
        self.issue_typed(user_id, "access", ttl_seconds)
    }

    pub fn issue_typed(
        &self,
        user_id: &str,
        token_type: &str,
        ttl_seconds: i64,
    ) -> Result<(String, usize), String> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|e| e.to_string())?
            .as_secs() as usize;
        let exp = now.saturating_add(ttl_seconds.max(1) as usize);
        let claims = AccessClaims {
            sub: user_id.to_owned(),
            iss: self.issuer.clone(),
            aud: self.audience.clone(),
            exp,
            iat: now,
            token_type: token_type.into(),
        };
        let mut header = Header::new(Algorithm::RS256);
        header.kid = Some(self.active_kid.clone());
        let token = encode(&header, &claims, &self.encoding)
            .map_err(|e| format!("JWT signing failed: {e}"))?;
        Ok((token, exp))
    }

    pub fn authenticate(&self, headers: &HeaderMap) -> Result<AccessClaims, AuthError> {
        let bearer = headers
            .get(axum::http::header::AUTHORIZATION)
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.strip_prefix("Bearer "))
            .ok_or(AuthError)?;
        let claims = self.verify(bearer).map_err(|_| AuthError)?;
        if claims.token_type != "access" || claims.sub.trim().is_empty() {
            return Err(AuthError);
        }
        Ok(claims)
    }

    pub fn verify(&self, token: &str) -> Result<AccessClaims, String> {
        let header = jsonwebtoken::decode_header(token).map_err(|e| e.to_string())?;
        if let Some(kid) = header.kid.as_deref() {
            let key = self
                .public_keys
                .get(kid)
                .ok_or_else(|| "unknown JWT kid".to_string())?;
            decode::<AccessClaims>(token, key, &self.validation)
                .map(|data| data.claims)
                .map_err(|e| e.to_string())
        } else {
            for key in self.public_keys.values() {
                if let Ok(data) = decode::<AccessClaims>(token, key, &self.validation) {
                    return Ok(data.claims);
                }
            }
            decode::<AccessClaims>(token, &self.decoding, &self.validation)
                .map(|data| data.claims)
                .map_err(|e| e.to_string())
        }
    }
    pub fn jwks(&self) -> &serde_json::Value {
        &self.jwks
    }
}

#[derive(Debug)]
pub struct AuthError;
impl IntoResponse for AuthError {
    fn into_response(self) -> Response {
        api_error(
            StatusCode::UNAUTHORIZED,
            "unauthorized",
            "A valid EdgeNest access token is required",
        )
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
#[derive(Debug, Deserialize, Default)]
pub struct RefreshRequest {
    pub refresh_token: Option<String>,
}
#[derive(Debug, Deserialize)]
pub struct OneTimeTokenRequest {
    pub token: String,
}
#[derive(Debug, Deserialize)]
pub struct PasswordResetRequest {
    pub email: String,
}
#[derive(Debug, Deserialize)]
pub struct PasswordResetConfirmRequest {
    pub token: String,
    pub new_password: String,
}
#[derive(Debug, Deserialize)]
pub struct TotpRequest {
    pub code: String,
}
#[derive(Debug, Deserialize)]
pub struct MfaLoginRequest {
    pub challenge_token: String,
    pub code: String,
}
#[derive(Debug, Serialize)]
struct IssuedPair {
    response: TokenResponse,
    refresh_token: String,
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
    user: UserView,
}

fn api_error(status: StatusCode, code: &str, message: &str) -> Response {
    (
        status,
        Json(serde_json::json!({"error":{"code":code,"message":message}})),
    )
        .into_response()
}
fn invalid() -> Response {
    api_error(
        StatusCode::BAD_REQUEST,
        "invalid_request",
        "Invalid request fields",
    )
}
fn unauthorized() -> Response {
    api_error(
        StatusCode::UNAUTHORIZED,
        "invalid_credentials",
        "Invalid email or password",
    )
}
fn internal() -> Response {
    api_error(
        StatusCode::INTERNAL_SERVER_ERROR,
        "internal_error",
        "Internal server error",
    )
}
fn conflict() -> Response {
    api_error(
        StatusCode::CONFLICT,
        "email_exists",
        "An account with this email already exists",
    )
}

fn normalize_email(email: &str) -> String {
    email.trim().to_lowercase()
}
fn valid_email(email: &str) -> bool {
    if email.len() > 320 || email.chars().any(char::is_whitespace) {
        return false;
    }
    let mut parts = email.split('@');
    matches!((parts.next(), parts.next(), parts.next()), (Some(local), Some(domain), None) if !local.is_empty() && domain.contains('.') && !domain.starts_with('.') && !domain.ends_with('.'))
}
fn hash_password(password: &str) -> Result<String, String> {
    let salt = SaltString::generate(&mut OsRng);
    Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map(|hash| hash.to_string())
        .map_err(|e| e.to_string())
}
fn verify_password(password: &str, stored_hash: &str) -> bool {
    PasswordHash::new(stored_hash).ok().is_some_and(|hash| {
        Argon2::default()
            .verify_password(password.as_bytes(), &hash)
            .is_ok()
    })
}
fn new_refresh_token() -> String {
    let mut bytes = [0u8; 32];
    OsRng.fill_bytes(&mut bytes);
    hex::encode(bytes)
}
fn token_hash(raw: &str) -> String {
    hex::encode(Sha256::digest(raw.as_bytes()))
}

async fn issue_pair(state: &AppState, user: UserView) -> Result<IssuedPair, ()> {
    let (access_token, _expires_at) = state
        .jwt
        .issue_access(&user.id, state.access_ttl_seconds)
        .map_err(|_| ())?;
    let refresh_token = new_refresh_token();
    let expires = Utc::now() + Duration::days(state.refresh_ttl_days);
    sqlx::query(
        "INSERT INTO identity_refresh_tokens(id,user_id,token_hash,expires_at) VALUES($1,$2,$3,$4)",
    )
    .bind(uuid::Uuid::new_v4().to_string())
    .bind(&user.id)
    .bind(token_hash(&refresh_token))
    .bind(expires)
    .execute(&state.pool)
    .await
    .map_err(|_| ())?;
    Ok(IssuedPair {
        response: TokenResponse {
            token_type: "Bearer",
            access_token,
            expires_in: state.access_ttl_seconds as usize,
            user,
        },
        refresh_token,
    })
}

pub async fn health() -> &'static str {
    "ok"
}
pub async fn ready(State(state): State<AppState>) -> Response {
    if state.resend_api_key.is_none()
        || state.email_from.trim().is_empty()
        || state.public_app_url.trim().is_empty()
    {
        return api_error(
            StatusCode::SERVICE_UNAVAILABLE,
            "email_delivery_not_configured",
            "Email delivery provider is not configured",
        );
    }
    match sqlx::query_scalar::<_, i32>("SELECT 1")
        .fetch_one(&state.pool)
        .await
    {
        Ok(_) => Json(serde_json::json!({"status":"ready"})).into_response(),
        Err(_) => api_error(
            StatusCode::SERVICE_UNAVAILABLE,
            "not_ready",
            "Database unavailable",
        ),
    }
}

pub async fn register(
    State(state): State<AppState>,
    Json(input): Json<RegisterRequest>,
) -> Response {
    let email = normalize_email(&input.email);
    let display_name = input.display_name.trim();
    if !valid_email(&email)
        || display_name.is_empty()
        || display_name.len() > 120
        || input.password.len() < 12
        || input.password.len() > 1024
    {
        return invalid();
    }
    let password_hash = match hash_password(&input.password) {
        Ok(hash) => hash,
        Err(_) => return internal(),
    };
    let user_id = uuid::Uuid::new_v4().to_string();
    let inserted = sqlx::query_as::<_, (String,String,String)>(
        "INSERT INTO identity_users(id,email,display_name,password_hash) VALUES($1,$2,$3,$4) RETURNING id,email,display_name")
        .bind(&user_id).bind(&email).bind(display_name).bind(password_hash).fetch_optional(&state.pool).await;
    let user = match inserted {
        Ok(Some((id, email, display_name))) => UserView {
            id,
            email,
            display_name,
        },
        Ok(None) => return internal(),
        Err(error)
            if error
                .as_database_error()
                .is_some_and(|e| e.code().as_deref() == Some("23505")) =>
        {
            return conflict()
        }
        Err(_) => return internal(),
    };
    let token = match create_verification_token(&state, &user.id).await {
        Ok(v) => v,
        Err(()) => return internal(),
    };
    let verify_url = format!(
        "{}/verify-email?token={}",
        state.public_app_url.trim_end_matches('/'),
        token
    );
    if send_email(&state,&user.email,"Verify your EdgeNest email",&format!("<p>Welcome to EdgeNest.</p><p><a href=\"{verify_url}\">Verify your email address</a></p><p>This link expires in 24 hours.</p>")).await.is_err() {
        return api_error(StatusCode::SERVICE_UNAVAILABLE,"email_delivery_unavailable","Registration was created, but verification email delivery is temporarily unavailable. Use resend verification.");
    }
    (StatusCode::CREATED,Json(serde_json::json!({"status":"verification_required","message":"Check your email to verify your EdgeNest account."}))).into_response()
}

pub async fn login(State(state): State<AppState>, Json(input): Json<LoginRequest>) -> Response {
    let email = normalize_email(&input.email);
    let row = sqlx::query_as::<_, (String, String, String, String, bool)>(
        "SELECT id,email,display_name,password_hash,is_active FROM identity_users WHERE email=$1",
    )
    .bind(email)
    .fetch_optional(&state.pool)
    .await;
    let (id, email, display_name, password_hash, is_active) = match row {
        Ok(Some(row)) => row,
        Ok(None) => return unauthorized(),
        Err(_) => return internal(),
    };
    if !is_active || !verify_password(&input.password, &password_hash) {
        return unauthorized();
    }
    let verified = sqlx::query_scalar::<_, Option<chrono::DateTime<Utc>>>(
        "SELECT email_verified_at FROM identity_users WHERE id=$1",
    )
    .bind(&id)
    .fetch_one(&state.pool)
    .await;
    if !matches!(verified, Ok(Some(_))) {
        return api_error(
            StatusCode::FORBIDDEN,
            "email_not_verified",
            "Verify your email before signing in",
        );
    }
    let mfa_enabled =
        match sqlx::query_scalar::<_, bool>("SELECT mfa_enabled FROM identity_users WHERE id=$1")
            .bind(&id)
            .fetch_one(&state.pool)
            .await
        {
            Ok(value) => value,
            Err(_) => return internal(),
        };
    if mfa_enabled {
        let (challenge_token, expires_at) = match state.jwt.issue_typed(&id, "mfa_challenge", 300) {
            Ok(v) => v,
            Err(_) => return internal(),
        };
        let now = Utc::now().timestamp() as usize;
        return (StatusCode::ACCEPTED, Json(serde_json::json!({"mfa_required":true,"challenge_token":challenge_token,"expires_in":expires_at.saturating_sub(now)}))).into_response();
    }
    match issue_pair(
        &state,
        UserView {
            id,
            email,
            display_name,
        },
    )
    .await
    {
        Ok(tokens) => set_refresh_cookie(
            Json(tokens.response).into_response(),
            &tokens.refresh_token,
            state.refresh_ttl_days,
        ),
        Err(()) => internal(),
    }
}

pub async fn refresh(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<RefreshRequest>,
) -> Response {
    let refresh_token = input
        .refresh_token
        .or_else(|| cookie_value(&headers, "__Host-edgenest_refresh"));
    let Some(refresh_token) = refresh_token else {
        return unauthorized();
    };
    if refresh_token.len() < 32 || refresh_token.len() > 256 {
        return unauthorized();
    }
    let mut tx = match state.pool.begin().await {
        Ok(tx) => tx,
        Err(_) => return internal(),
    };
    let user_id = sqlx::query_scalar::<_, String>(
        "UPDATE identity_refresh_tokens SET revoked_at=NOW(),last_used_at=NOW() WHERE token_hash=$1 AND revoked_at IS NULL AND expires_at>NOW() RETURNING user_id")
         .bind(token_hash(&refresh_token)).fetch_optional(&mut *tx).await;
    let user_id = match user_id {
        Ok(Some(id)) => id,
        Ok(None) => {
            let _ = tx.rollback().await;
            return unauthorized();
        }
        Err(_) => {
            let _ = tx.rollback().await;
            return internal();
        }
    };
    let user = sqlx::query_as::<_, (String, String, String, bool)>(
        "SELECT id,email,display_name,is_active FROM identity_users WHERE id=$1",
    )
    .bind(&user_id)
    .fetch_optional(&mut *tx)
    .await;
    let (id, email, display_name, is_active) = match user {
        Ok(Some(row)) => row,
        Ok(None) => {
            let _ = tx.rollback().await;
            return unauthorized();
        }
        Err(_) => {
            let _ = tx.rollback().await;
            return internal();
        }
    };
    if !is_active {
        let _ = tx.rollback().await;
        return unauthorized();
    }
    let (access_token, _expires_at) = match state.jwt.issue_access(&id, state.access_ttl_seconds) {
        Ok(token) => token,
        Err(_) => {
            let _ = tx.rollback().await;
            return internal();
        }
    };
    let refresh_token = new_refresh_token();
    let expiry = Utc::now() + Duration::days(state.refresh_ttl_days);
    if sqlx::query(
        "INSERT INTO identity_refresh_tokens(id,user_id,token_hash,expires_at) VALUES($1,$2,$3,$4)",
    )
    .bind(uuid::Uuid::new_v4().to_string())
    .bind(&id)
    .bind(token_hash(&refresh_token))
    .bind(expiry)
    .execute(&mut *tx)
    .await
    .is_err()
    {
        let _ = tx.rollback().await;
        return internal();
    }
    if tx.commit().await.is_err() {
        return internal();
    }
    set_refresh_cookie(
        Json(TokenResponse {
            token_type: "Bearer",
            access_token,
            expires_in: state.access_ttl_seconds as usize,
            user: UserView {
                id,
                email,
                display_name,
            },
        })
        .into_response(),
        &refresh_token,
        state.refresh_ttl_days,
    )
}

pub async fn logout(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<RefreshRequest>,
) -> Response {
    let claims = match state.jwt.authenticate(&headers) {
        Ok(claims) => claims,
        Err(e) => return e.into_response(),
    };
    let refresh_token = input
        .refresh_token
        .or_else(|| cookie_value(&headers, "__Host-edgenest_refresh"));
    if let Some(refresh_token) = refresh_token {
        let _ = sqlx::query("UPDATE identity_refresh_tokens SET revoked_at=NOW() WHERE user_id=$1 AND token_hash=$2 AND revoked_at IS NULL")
            .bind(claims.sub).bind(token_hash(&refresh_token)).execute(&state.pool).await;
    }
    match Ok::<(), sqlx::Error>(()) {
        Ok(_) => clear_refresh_cookie(StatusCode::NO_CONTENT.into_response()),
        Err(_) => internal(),
    }
}

pub async fn me(State(state): State<AppState>, headers: HeaderMap) -> Response {
    let claims = match state.jwt.authenticate(&headers) {
        Ok(claims) => claims,
        Err(e) => return e.into_response(),
    };
    match sqlx::query_as::<_, (String, String, String, bool)>(
        "SELECT id,email,display_name,is_active FROM identity_users WHERE id=$1",
    )
    .bind(claims.sub)
    .fetch_optional(&state.pool)
    .await
    {
        Ok(Some((id, email, display_name, true))) => {
            Json(serde_json::json!({"data":{"id":id,"email":email,"display_name":display_name}}))
                .into_response()
        }
        Ok(_) => unauthorized(),
        Err(_) => internal(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn email_is_normalized_and_validated() {
        assert_eq!(normalize_email("  Test@Example.com "), "test@example.com");
        assert!(valid_email("test@example.com"));
        assert!(!valid_email("missing-at.example.com"));
        assert!(!valid_email("test@localhost"));
        assert!(!valid_email("user name@example.com"));
    }

    #[test]
    fn passwords_are_argon2_hashed_and_verified() {
        let password = "correct horse battery staple";
        let hash = hash_password(password).unwrap_or_default();
        assert!(!hash.is_empty());
        assert_ne!(hash, password);
        assert!(verify_password(password, &hash));
        assert!(!verify_password("incorrect password", &hash));
    }
}

fn set_refresh_cookie(mut response: Response, token: &str, ttl_days: i64) -> Response {
    let cookie = format!(
        "__Host-edgenest_refresh={token}; Path=/; Max-Age={}; HttpOnly; Secure; SameSite=None",
        ttl_days.max(1) * 86400
    );
    if let Ok(value) = cookie.parse() {
        response
            .headers_mut()
            .insert(axum::http::header::SET_COOKIE, value);
    }
    response
}
fn clear_refresh_cookie(mut response: Response) -> Response {
    if let Ok(value) =
        "__Host-edgenest_refresh=; Path=/; Max-Age=0; HttpOnly; Secure; SameSite=None".parse()
    {
        response
            .headers_mut()
            .insert(axum::http::header::SET_COOKIE, value);
    }
    response
}
fn cookie_value(headers: &HeaderMap, name: &str) -> Option<String> {
    headers
        .get(axum::http::header::COOKIE)?
        .to_str()
        .ok()?
        .split(';')
        .map(str::trim)
        .find_map(|part| part.strip_prefix(&format!("{name}=")).map(str::to_owned))
}
fn random_token() -> String {
    new_refresh_token()
}

async fn send_email(state: &AppState, to: &str, subject: &str, html: &str) -> Result<(), ()> {
    let Some(api_key) = state.resend_api_key.as_deref() else {
        return Err(());
    };
    if state.email_from.trim().is_empty() || state.public_app_url.trim().is_empty() {
        return Err(());
    }
    let client = reqwest::Client::builder()
        .timeout(StdDuration::from_secs(10))
        .build()
        .map_err(|_| ())?;
    let response = client
        .post("https://api.resend.com/emails")
        .bearer_auth(api_key)
        .json(&serde_json::json!({"from":state.email_from,"to":[to],"subject":subject,"html":html}))
        .send()
        .await
        .map_err(|_| ())?;
    if response.status().is_success() {
        Ok(())
    } else {
        Err(())
    }
}
async fn create_verification_token(state: &AppState, user_id: &str) -> Result<String, ()> {
    let raw = random_token();
    sqlx::query("INSERT INTO identity_email_verification_tokens(id,user_id,token_hash,expires_at) VALUES($1,$2,$3,$4)")
        .bind(uuid::Uuid::new_v4().to_string()).bind(user_id).bind(token_hash(&raw)).bind(Utc::now()+Duration::hours(24))
        .execute(&state.pool).await.map_err(|_| ())?;
    Ok(raw)
}
async fn create_password_reset_token(state: &AppState, user_id: &str) -> Result<String, ()> {
    let raw = random_token();
    sqlx::query("INSERT INTO identity_password_reset_tokens(id,user_id,token_hash,expires_at) VALUES($1,$2,$3,$4)")
        .bind(uuid::Uuid::new_v4().to_string()).bind(user_id).bind(token_hash(&raw)).bind(Utc::now()+Duration::minutes(30))
        .execute(&state.pool).await.map_err(|_| ())?;
    Ok(raw)
}
fn base32_encode(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 32] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ234567";
    let mut out = String::new();
    let mut buffer: u32 = 0;
    let mut bits = 0;
    for byte in bytes {
        buffer = (buffer << 8) | (*byte as u32);
        bits += 8;
        while bits >= 5 {
            bits -= 5;
            out.push(ALPHABET[((buffer >> bits) & 31) as usize] as char);
        }
    }
    if bits > 0 {
        out.push(ALPHABET[((buffer << (5 - bits)) & 31) as usize] as char);
    }
    out
}
fn totp_code(secret: &[u8], timestamp: u64) -> String {
    let counter = (timestamp / 30).to_be_bytes();
    let mut mac = <Hmac<Sha1> as Mac>::new_from_slice(secret).expect("HMAC accepts all key sizes");
    mac.update(&counter);
    let digest = mac.finalize().into_bytes();
    let offset = (digest[19] & 0x0f) as usize;
    let binary = ((digest[offset] & 0x7f) as u32) << 24
        | (digest[offset + 1] as u32) << 16
        | (digest[offset + 2] as u32) << 8
        | digest[offset + 3] as u32;
    format!("{:06}", binary % 1_000_000)
}
fn verify_totp(secret: &[u8], code: &str) -> bool {
    if code.len() != 6 || !code.bytes().all(|b| b.is_ascii_digit()) {
        return false;
    }
    let now = Utc::now().timestamp().max(0) as u64;
    (-1i64..=1).any(|offset| totp_code(secret, (now as i64 + offset * 30).max(0) as u64) == code)
}
fn encrypt_mfa_secret(state: &AppState, secret: &[u8]) -> Result<Vec<u8>, ()> {
    let key = Sha256::digest(state.mfa_encryption_key.as_bytes());
    let cipher = Aes256Gcm::new_from_slice(&key).map_err(|_| ())?;
    let mut nonce = [0u8; 12];
    OsRng.fill_bytes(&mut nonce);
    let encrypted = cipher
        .encrypt(Nonce::from_slice(&nonce), secret)
        .map_err(|_| ())?;
    Ok([nonce.as_slice(), encrypted.as_slice()].concat())
}
fn decrypt_mfa_secret(state: &AppState, data: &[u8]) -> Result<Vec<u8>, ()> {
    if data.len() < 13 {
        return Err(());
    }
    let key = Sha256::digest(state.mfa_encryption_key.as_bytes());
    let cipher = Aes256Gcm::new_from_slice(&key).map_err(|_| ())?;
    cipher
        .decrypt(Nonce::from_slice(&data[..12]), &data[12..])
        .map_err(|_| ())
}

pub async fn verify_email(
    State(state): State<AppState>,
    Json(input): Json<OneTimeTokenRequest>,
) -> Response {
    if input.token.len() < 32 || input.token.len() > 256 {
        return invalid();
    }
    let mut tx = match state.pool.begin().await {
        Ok(tx) => tx,
        Err(_) => return internal(),
    };
    let user_id=sqlx::query_scalar::<_,String>("UPDATE identity_email_verification_tokens SET consumed_at=NOW() WHERE token_hash=$1 AND consumed_at IS NULL AND expires_at>NOW() RETURNING user_id")
        .bind(token_hash(&input.token)).fetch_optional(&mut *tx).await;
    let user_id = match user_id {
        Ok(Some(id)) => id,
        _ => {
            let _ = tx.rollback().await;
            return api_error(
                StatusCode::BAD_REQUEST,
                "invalid_token",
                "Verification link is invalid or expired",
            );
        }
    };
    if sqlx::query("UPDATE identity_users SET email_verified_at=NOW(),updated_at=NOW() WHERE id=$1")
        .bind(user_id)
        .execute(&mut *tx)
        .await
        .is_err()
    {
        let _ = tx.rollback().await;
        return internal();
    }
    if tx.commit().await.is_err() {
        return internal();
    }
    Json(serde_json::json!({"status":"email_verified","message":"Email verified. You can now sign in."})).into_response()
}
pub async fn resend_verification(
    State(state): State<AppState>,
    Json(input): Json<PasswordResetRequest>,
) -> Response {
    if state.resend_api_key.is_none()
        || state.email_from.trim().is_empty()
        || state.public_app_url.trim().is_empty()
    {
        return api_error(
            StatusCode::SERVICE_UNAVAILABLE,
            "email_delivery_unavailable",
            "Email delivery is not configured",
        );
    }
    let email = normalize_email(&input.email);
    if let Ok(Some((id, verified))) = sqlx::query_as::<_, (String, bool)>(
        "SELECT id,email_verified_at IS NOT NULL FROM identity_users WHERE email=$1",
    )
    .bind(email.clone())
    .fetch_optional(&state.pool)
    .await
    {
        if !verified {
            if let Ok(token) = create_verification_token(&state, &id).await {
                let url = format!(
                    "{}/verify-email?token={}",
                    state.public_app_url.trim_end_matches('/'),
                    token
                );
                let _=send_email(&state,&email,"Verify your EdgeNest email",&format!("<p>Verify your EdgeNest email:</p><p><a href=\"{url}\">Verify email</a></p><p>This link expires in 24 hours.</p>")).await;
            }
        }
    }
    (StatusCode::ACCEPTED,Json(serde_json::json!({"message":"If this account needs verification, instructions have been sent."}))).into_response()
}
pub async fn request_password_reset(
    State(state): State<AppState>,
    Json(input): Json<PasswordResetRequest>,
) -> Response {
    if state.resend_api_key.is_none()
        || state.email_from.trim().is_empty()
        || state.public_app_url.trim().is_empty()
    {
        return api_error(
            StatusCode::SERVICE_UNAVAILABLE,
            "email_delivery_unavailable",
            "Email delivery is not configured",
        );
    }
    let email = normalize_email(&input.email);
    if let Ok(Some((id, active))) = sqlx::query_as::<_, (String, bool)>(
        "SELECT id,is_active FROM identity_users WHERE email=$1",
    )
    .bind(email.clone())
    .fetch_optional(&state.pool)
    .await
    {
        if active {
            if let Ok(token) = create_password_reset_token(&state, &id).await {
                let url = format!(
                    "{}/reset-password?token={}",
                    state.public_app_url.trim_end_matches('/'),
                    token
                );
                let _=send_email(&state,&email,"Reset your EdgeNest password",&format!("<p>Reset your password:</p><p><a href=\"{url}\">Reset password</a></p><p>This link expires in 30 minutes. Ignore this message if you did not request it.</p>")).await;
            }
        }
    }
    (StatusCode::ACCEPTED,Json(serde_json::json!({"message":"If an account exists for this email, password reset instructions have been sent."}))).into_response()
}
pub async fn confirm_password_reset(
    State(state): State<AppState>,
    Json(input): Json<PasswordResetConfirmRequest>,
) -> Response {
    if input.token.len() < 32 || input.new_password.len() < 12 || input.new_password.len() > 1024 {
        return invalid();
    }
    let hash = match hash_password(&input.new_password) {
        Ok(v) => v,
        Err(_) => return internal(),
    };
    let mut tx = match state.pool.begin().await {
        Ok(tx) => tx,
        Err(_) => return internal(),
    };
    let user_id=sqlx::query_scalar::<_,String>("UPDATE identity_password_reset_tokens SET consumed_at=NOW() WHERE token_hash=$1 AND consumed_at IS NULL AND expires_at>NOW() RETURNING user_id")
        .bind(token_hash(&input.token)).fetch_optional(&mut *tx).await;
    let user_id = match user_id {
        Ok(Some(id)) => id,
        _ => {
            let _ = tx.rollback().await;
            return api_error(
                StatusCode::BAD_REQUEST,
                "invalid_token",
                "Password reset link is invalid or expired",
            );
        }
    };
    if sqlx::query("UPDATE identity_users SET password_hash=$2,updated_at=NOW() WHERE id=$1").bind(&user_id).bind(hash).execute(&mut *tx).await.is_err()
        || sqlx::query("UPDATE identity_refresh_tokens SET revoked_at=NOW() WHERE user_id=$1 AND revoked_at IS NULL").bind(user_id).execute(&mut *tx).await.is_err() {
        let _=tx.rollback().await;return internal();
    }
    if tx.commit().await.is_err() {
        return internal();
    }
    Json(serde_json::json!({"status":"password_reset","message":"Password reset successful. Sign in again."})).into_response()
}
pub async fn mfa_setup(State(state): State<AppState>, headers: HeaderMap) -> Response {
    let claims = match state.jwt.authenticate(&headers) {
        Ok(v) => v,
        Err(e) => return e.into_response(),
    };
    let enabled =
        match sqlx::query_scalar::<_, bool>("SELECT mfa_enabled FROM identity_users WHERE id=$1")
            .bind(&claims.sub)
            .fetch_optional(&state.pool)
            .await
        {
            Ok(Some(value)) => value,
            Ok(None) => return unauthorized(),
            Err(_) => return internal(),
        };
    if enabled {
        return api_error(
            StatusCode::CONFLICT,
            "mfa_enabled",
            "MFA is already enabled",
        );
    }
    let mut secret = [0u8; 20];
    OsRng.fill_bytes(&mut secret);
    let encrypted = match encrypt_mfa_secret(&state, &secret) {
        Ok(v) => v,
        Err(_) => return internal(),
    };
    if sqlx::query("UPDATE identity_users SET mfa_secret_encrypted=$2 WHERE id=$1")
        .bind(&claims.sub)
        .bind(encrypted)
        .execute(&state.pool)
        .await
        .is_err()
    {
        return internal();
    }
    let encoded = base32_encode(&secret);
    let email = sqlx::query_scalar::<_, String>("SELECT email FROM identity_users WHERE id=$1")
        .bind(&claims.sub)
        .fetch_one(&state.pool)
        .await
        .unwrap_or_default();
    let uri = format!(
        "otpauth://totp/EdgeNest:{}?secret={}&issuer=EdgeNest&algorithm=SHA1&digits=6&period=30",
        email, encoded
    );
    Json(serde_json::json!({"secret":encoded,"otpauth_uri":uri,"message":"Add this secret to your authenticator app, then confirm with a current code."})).into_response()
}
pub async fn mfa_confirm(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<TotpRequest>,
) -> Response {
    let claims = match state.jwt.authenticate(&headers) {
        Ok(v) => v,
        Err(e) => return e.into_response(),
    };
    let encrypted = sqlx::query_scalar::<_, Option<Vec<u8>>>(
        "SELECT mfa_secret_encrypted FROM identity_users WHERE id=$1",
    )
    .bind(&claims.sub)
    .fetch_optional(&state.pool)
    .await
    .unwrap_or(None)
    .flatten();
    let Some(encrypted) = encrypted else {
        return api_error(
            StatusCode::BAD_REQUEST,
            "mfa_not_setup",
            "Start MFA setup first",
        );
    };
    let secret = match decrypt_mfa_secret(&state, &encrypted) {
        Ok(v) => v,
        Err(_) => return internal(),
    };
    if !verify_totp(&secret, &input.code) {
        return api_error(
            StatusCode::BAD_REQUEST,
            "invalid_code",
            "Authenticator code is invalid",
        );
    }
    let mut codes = Vec::new();
    for _ in 0..10 {
        let code = random_token()[..32].to_owned();
        codes.push(code);
    }
    let mut tx = match state.pool.begin().await {
        Ok(tx) => tx,
        Err(_) => return internal(),
    };
    if sqlx::query("UPDATE identity_users SET mfa_enabled=TRUE WHERE id=$1")
        .bind(&claims.sub)
        .execute(&mut *tx)
        .await
        .is_err()
    {
        let _ = tx.rollback().await;
        return internal();
    }
    for code in &codes {
        if sqlx::query(
            "INSERT INTO identity_mfa_recovery_codes(id,user_id,code_hash) VALUES($1,$2,$3)",
        )
        .bind(uuid::Uuid::new_v4().to_string())
        .bind(&claims.sub)
        .bind(token_hash(code))
        .execute(&mut *tx)
        .await
        .is_err()
        {
            let _ = tx.rollback().await;
            return internal();
        }
    }
    if tx.commit().await.is_err() {
        return internal();
    }
    Json(serde_json::json!({"status":"mfa_enabled","recovery_codes":codes,"warning":"Save these recovery codes now. They are shown only once."})).into_response()
}
pub async fn mfa_disable(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<TotpRequest>,
) -> Response {
    let claims = match state.jwt.authenticate(&headers) {
        Ok(v) => v,
        Err(e) => return e.into_response(),
    };
    let encrypted = sqlx::query_scalar::<_, Option<Vec<u8>>>(
        "SELECT mfa_secret_encrypted FROM identity_users WHERE id=$1",
    )
    .bind(&claims.sub)
    .fetch_optional(&state.pool)
    .await
    .unwrap_or(None)
    .flatten();
    let Some(encrypted) = encrypted else {
        return unauthorized();
    };
    let secret = match decrypt_mfa_secret(&state, &encrypted) {
        Ok(v) => v,
        Err(_) => return internal(),
    };
    if !verify_totp(&secret, &input.code) {
        return api_error(
            StatusCode::UNAUTHORIZED,
            "invalid_code",
            "Authenticator code is invalid",
        );
    }
    if sqlx::query(
        "UPDATE identity_users SET mfa_enabled=FALSE,mfa_secret_encrypted=NULL WHERE id=$1",
    )
    .bind(&claims.sub)
    .execute(&state.pool)
    .await
    .is_err()
    {
        return internal();
    }
    let _ = sqlx::query("DELETE FROM identity_mfa_recovery_codes WHERE user_id=$1")
        .bind(&claims.sub)
        .execute(&state.pool)
        .await;
    Json(serde_json::json!({"status":"mfa_disabled"})).into_response()
}
pub async fn mfa_verify_login(
    State(state): State<AppState>,
    Json(input): Json<MfaLoginRequest>,
) -> Response {
    let claims = match state.jwt.verify(&input.challenge_token) {
        Ok(v) => v,
        Err(_) => return unauthorized(),
    };
    if claims.token_type != "mfa_challenge" || claims.sub.trim().is_empty() {
        return unauthorized();
    }
    let encrypted=sqlx::query_scalar::<_,Option<Vec<u8>>>("SELECT mfa_secret_encrypted FROM identity_users WHERE id=$1 AND mfa_enabled=TRUE AND is_active=TRUE AND email_verified_at IS NOT NULL").bind(&claims.sub).fetch_optional(&state.pool).await.unwrap_or(None).flatten();
    let Some(encrypted) = encrypted else {
        return unauthorized();
    };
    let secret = match decrypt_mfa_secret(&state, &encrypted) {
        Ok(v) => v,
        Err(_) => return internal(),
    };
    let valid_totp = verify_totp(&secret, &input.code);
    let mut valid = valid_totp;
    if !valid_totp {
        let hash = token_hash(&input.code);
        let consumed=sqlx::query("UPDATE identity_mfa_recovery_codes SET used_at=NOW() WHERE user_id=$1 AND code_hash=$2 AND used_at IS NULL").bind(&claims.sub).bind(hash).execute(&state.pool).await;
        valid = consumed.is_ok_and(|r| r.rows_affected() == 1);
    }
    if !valid {
        return unauthorized();
    }
    let user = sqlx::query_as::<_, (String, String, String)>(
        "SELECT id,email,display_name FROM identity_users WHERE id=$1",
    )
    .bind(&claims.sub)
    .fetch_one(&state.pool)
    .await;
    let (id, email, display_name) = match user {
        Ok(v) => v,
        Err(_) => return internal(),
    };
    match issue_pair(
        &state,
        UserView {
            id,
            email,
            display_name,
        },
    )
    .await
    {
        Ok(pair) => set_refresh_cookie(
            Json(pair.response).into_response(),
            &pair.refresh_token,
            state.refresh_ttl_days,
        ),
        Err(()) => internal(),
    }
}
pub async fn jwks(State(state): State<AppState>) -> Response {
    Json(state.jwt.jwks().clone()).into_response()
}

#[cfg(test)]
mod security_tests {
    use super::*;

    #[test]
    fn totp_matches_current_rfc6238_window() {
        let secret = b"edge-nest-test-secret";
        let timestamp = Utc::now().timestamp().max(0) as u64;
        let code = totp_code(secret, timestamp);
        assert!(verify_totp(secret, &code));
        assert!(!verify_totp(secret, "abcdef"));
        assert!(!verify_totp(secret, "12345"));
    }

    #[test]
    fn base32_secret_has_no_padding_and_valid_alphabet() {
        let encoded = base32_encode(&[0xFF, 0x00, 0x10, 0x20]);
        assert!(!encoded.contains('='));
        assert!(encoded
            .chars()
            .all(|c| c.is_ascii_uppercase() || ('2'..='7').contains(&c)));
    }

    #[test]
    fn one_time_token_hash_is_not_raw_token() {
        let token = random_token();
        let digest = token_hash(&token);
        assert_ne!(digest, token);
        assert_eq!(digest.len(), 64);
    }
}

#[cfg(test)]
mod jwks_rotation_tests {
    use super::*;
    use rsa::{
        pkcs8::{EncodePrivateKey, EncodePublicKey, LineEnding},
        RsaPrivateKey,
    };

    fn key_pair() -> (String, String) {
        let private = RsaPrivateKey::new(&mut OsRng, 2048).expect("generate RSA key");
        let private_pem = private
            .to_pkcs8_pem(LineEnding::LF)
            .expect("encode private PEM")
            .to_string();
        let public_pem = private
            .to_public_key()
            .to_public_key_pem(LineEnding::LF)
            .expect("encode public PEM");
        (private_pem, public_pem)
    }

    #[test]
    fn overlapping_jwks_keyring_verifies_old_tokens_and_publishes_both_keys() {
        let (old_private, old_public) = key_pair();
        let (active_private, active_public) = key_pair();
        let issuer = "https://identity.edgenest.test".to_string();
        let audience = "edgenest-api".to_string();

        let old_issuer = JwtService::new(
            old_private.as_bytes(),
            old_public.as_bytes(),
            "old-key".into(),
            None,
            issuer.clone(),
            audience.clone(),
        )
        .expect("old key issuer");
        let (old_token, _) = old_issuer.issue_access("user-123", 900).expect("old JWT");

        let keyring = serde_json::json!([
            {"kid":"old-key","pem":old_public},
            {"kid":"active-key","pem":active_public}
        ])
        .to_string();
        let active_issuer = JwtService::new(
            active_private.as_bytes(),
            active_public.as_bytes(),
            "active-key".into(),
            Some(&keyring),
            issuer,
            audience,
        )
        .expect("active key issuer");

        assert_eq!(
            active_issuer
                .verify(&old_token)
                .expect("verify old token")
                .sub,
            "user-123"
        );
        let (new_token, _) = active_issuer
            .issue_access("user-456", 900)
            .expect("new JWT");
        assert_eq!(
            jsonwebtoken::decode_header(&new_token)
                .expect("header")
                .kid
                .as_deref(),
            Some("active-key")
        );
        assert_eq!(
            active_issuer.jwks()["keys"]
                .as_array()
                .expect("JWKS keys")
                .len(),
            2
        );
    }
}

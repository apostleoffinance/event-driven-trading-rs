use std::time::{SystemTime, UNIX_EPOCH};

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
use chrono::{DateTime, Duration, Utc};
use identity_token::AccessClaims;
use rand_core::{OsRng, RngCore};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::AppState;

fn access_claims(
    jwt: &identity_token::JwtKeyRing,
    user_id: &str,
    session_id: &str,
    session_revision: i64,
    ttl_seconds: i64,
) -> Result<AccessClaims, ()> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| ())?
        .as_secs() as usize;
    Ok(AccessClaims {
        sub: user_id.to_owned(),
        iss: jwt.issuer().to_owned(),
        aud: jwt.audience().to_owned(),
        exp: now.saturating_add(ttl_seconds.max(1) as usize),
        iat: now,
        token_type: "access".into(),
        jti: uuid::Uuid::new_v4().to_string(),
        session_id: session_id.to_owned(),
        session_revision,
    })
}

fn bearer_claims(state: &AppState, headers: &HeaderMap) -> Result<AccessClaims, AuthError> {
    let bearer = headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .ok_or(AuthError)?;
    state.jwt.verify(bearer).map_err(|_| AuthError)
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

fn new_session_id() -> String {
    let mut bytes = [0_u8; 32];
    OsRng.fill_bytes(&mut bytes);
    hex::encode(bytes)
}

async fn issue_access_in_tx(
    state: &AppState,
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    user_id: &str,
    session_id: &str,
    session_revision: i64,
) -> Result<(String, DateTime<Utc>), ()> {
    let claims = access_claims(
        &state.jwt,
        user_id,
        session_id,
        session_revision,
        state.access_ttl_seconds,
    )?;
    let token = state.jwt.issue(&claims).map_err(|_| ())?;
    let expiry = DateTime::<Utc>::from_timestamp(claims.exp as i64, 0).ok_or(())?;
    sqlx::query("INSERT INTO identity_access_tokens(jti,session_id,user_id,session_revision,issued_at,expires_at) VALUES($1,$2,$3,$4,to_timestamp($5),$6)")
        .bind(&claims.jti).bind(session_id).bind(user_id).bind(session_revision)
        .bind(claims.iat as i64).bind(expiry).execute(&mut **tx).await.map_err(|_| ())?;
    Ok((token, expiry))
}

async fn issue_pair(state: &AppState, user: UserView) -> Result<TokenResponse, ()> {
    let mut tx = state.pool.begin().await.map_err(|_| ())?;
    let active = sqlx::query_scalar::<_, bool>(
        "SELECT is_active FROM identity_users WHERE id=$1 FOR UPDATE",
    )
    .bind(&user.id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(|_| ())?;
    if active != Some(true) {
        return Err(());
    }
    let now = Utc::now();
    let session_id = new_session_id();
    let session_expiry = now + Duration::days(state.refresh_ttl_days);
    let family_id = uuid::Uuid::new_v4().to_string();
    sqlx::query("INSERT INTO identity_sessions(id,user_id,revision,status,issued_at,expires_at,last_used_at) VALUES($1,$2,1,'ACTIVE',$3,$4,$3)")
        .bind(&session_id).bind(&user.id).bind(now).bind(session_expiry).execute(&mut *tx).await.map_err(|_| ())?;
    let refresh_token = new_refresh_token();
    sqlx::query("INSERT INTO identity_refresh_tokens(id,user_id,token_hash,expires_at,session_id,token_family_id) VALUES($1,$2,$3,$4,$5,$6)")
        .bind(uuid::Uuid::new_v4().to_string()).bind(&user.id).bind(token_hash(&refresh_token))
        .bind(session_expiry).bind(&session_id).bind(&family_id).execute(&mut *tx).await.map_err(|_| ())?;
    let (access_token, _) = issue_access_in_tx(state, &mut tx, &user.id, &session_id, 1).await?;
    sqlx::query("INSERT INTO identity_session_events(session_id,user_id,actor_user_id,event_type,session_revision,token_family_id,reason) VALUES($1,$2,$2,'SESSION_ISSUED',1,$3,'primary authentication succeeded')")
        .bind(&session_id).bind(&user.id).bind(&family_id).execute(&mut *tx).await.map_err(|_| ())?;
    tx.commit().await.map_err(|_| ())?;
    Ok(TokenResponse {
        token_type: "Bearer",
        access_token,
        expires_in: state.access_ttl_seconds as usize,
        refresh_token,
        user,
    })
}

pub async fn health() -> &'static str {
    "ok"
}
pub async fn ready(State(state): State<AppState>) -> Response {
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
    match issue_pair(&state, user).await {
        Ok(tokens) => (StatusCode::CREATED, Json(tokens)).into_response(),
        Err(()) => internal(),
    }
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
        Ok(tokens) => Json(tokens).into_response(),
        Err(()) => internal(),
    }
}

pub async fn refresh(State(state): State<AppState>, Json(input): Json<RefreshRequest>) -> Response {
    if input.refresh_token.len() < 32 || input.refresh_token.len() > 256 {
        return unauthorized();
    }
    let mut tx = match state.pool.begin().await {
        Ok(tx) => tx,
        Err(_) => return internal(),
    };
    // Discover the lock keys without locking, then take locks in the global
    // order used by suspension and access-token validation: user, session,
    // refresh token. The token row is re-read after the locks are held.
    let refresh_key = sqlx::query_as::<_, (String, Option<String>)>(
        "SELECT user_id,session_id FROM identity_refresh_tokens WHERE token_hash=$1",
    )
    .bind(token_hash(&input.refresh_token))
    .fetch_optional(&mut *tx)
    .await;
    let (user_id, session_id) = match refresh_key {
        Ok(Some((user_id, Some(session_id)))) => (user_id, session_id),
        Ok(Some((_, None))) => {
            let _ = tx.rollback().await;
            return unauthorized();
        }
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
        "SELECT id,email,display_name,is_active FROM identity_users WHERE id=$1 FOR UPDATE",
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
    let session = sqlx::query_as::<_, (i64, String, DateTime<Utc>)>("SELECT revision,status,expires_at FROM identity_sessions WHERE id=$1 AND user_id=$2 FOR UPDATE")
        .bind(&session_id).bind(&user_id).fetch_optional(&mut *tx).await;
    let (session_revision, session_status, session_expiry) = match session {
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
    let refresh_row = sqlx::query_as::<_, (String, String, DateTime<Utc>, Option<DateTime<Utc>>)>(
        "SELECT id,token_family_id,expires_at,revoked_at FROM identity_refresh_tokens WHERE token_hash=$1 AND user_id=$2 AND session_id=$3 FOR UPDATE",
    )
    .bind(token_hash(&input.refresh_token))
    .bind(&user_id)
    .bind(&session_id)
    .fetch_optional(&mut *tx)
    .await;
    let (token_id, family_id, refresh_expires, refresh_revoked) = match refresh_row {
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
    if refresh_revoked.is_some() {
        let session = sqlx::query_scalar::<_, String>(
            "SELECT status FROM identity_sessions WHERE id=$1 AND user_id=$2 FOR UPDATE",
        )
        .bind(&session_id)
        .bind(&user_id)
        .fetch_optional(&mut *tx)
        .await;
        match session {
            Ok(Some(status)) if status == "ACTIVE" => {
                if revoke_session_tx(
                    &mut tx,
                    &session_id,
                    &user_id,
                    "refresh token reuse detected",
                    "REFRESH_TOKEN_REUSE_REVOKED_SESSION",
                )
                .await
                .is_err()
                {
                    let _ = tx.rollback().await;
                    return internal();
                }
            }
            Err(_) => {
                let _ = tx.rollback().await;
                return internal();
            }
            _ => {}
        }
        return match tx.commit().await {
            Ok(()) => unauthorized(),
            Err(_) => internal(),
        };
    }
    if refresh_expires <= Utc::now() {
        if sqlx::query(
            "UPDATE identity_refresh_tokens SET revoked_at=NOW(),last_used_at=NOW() WHERE id=$1",
        )
        .bind(&token_id)
        .execute(&mut *tx)
        .await
        .is_err()
        {
            let _ = tx.rollback().await;
            return internal();
        }
        return match tx.commit().await {
            Ok(()) => unauthorized(),
            Err(_) => internal(),
        };
    }
    if session_status != "ACTIVE" || session_expiry <= Utc::now() {
        let _ = tx.rollback().await;
        return unauthorized();
    }
    if !is_active {
        if revoke_session_tx(
            &mut tx,
            &session_id,
            &id,
            "identity account inactive",
            "SESSION_REVOKED",
        )
        .await
        .is_err()
        {
            let _ = tx.rollback().await;
            return internal();
        }
        return match tx.commit().await {
            Ok(()) => unauthorized(),
            Err(_) => internal(),
        };
    }
    if !matches!(sqlx::query("UPDATE identity_refresh_tokens SET revoked_at=NOW(),last_used_at=NOW() WHERE id=$1 AND revoked_at IS NULL")
        .bind(&token_id).execute(&mut *tx).await.map(|result| result.rows_affected() == 1), Ok(true)) {
        let _ = tx.rollback().await;
        return unauthorized();
    }
    let refresh_token = new_refresh_token();
    let expiry = Utc::now() + Duration::days(state.refresh_ttl_days);
    let (access_token, _) =
        match issue_access_in_tx(&state, &mut tx, &id, &session_id, session_revision).await {
            Ok(tokens) => tokens,
            Err(_) => {
                let _ = tx.rollback().await;
                return internal();
            }
        };
    if !matches!(sqlx::query("UPDATE identity_sessions SET expires_at=$2,last_used_at=NOW(),updated_at=NOW() WHERE id=$1 AND revision=$3 AND status='ACTIVE'")
        .bind(&session_id).bind(expiry).bind(session_revision).execute(&mut *tx).await.map(|result| result.rows_affected() == 1), Ok(true)) {
        let _ = tx.rollback().await;
        return unauthorized();
    }
    if sqlx::query("INSERT INTO identity_refresh_tokens(id,user_id,token_hash,expires_at,session_id,token_family_id) VALUES($1,$2,$3,$4,$5,$6)")
        .bind(uuid::Uuid::new_v4().to_string()).bind(&id).bind(token_hash(&refresh_token)).bind(expiry).bind(&session_id).bind(&family_id)
        .execute(&mut *tx).await.is_err() {
        let _ = tx.rollback().await;
        return internal();
    }
    if sqlx::query("INSERT INTO identity_session_events(session_id,user_id,actor_user_id,event_type,session_revision,token_family_id,reason) VALUES($1,$2,$2,'SESSION_REFRESHED',$3,$4,'refresh token rotated')")
        .bind(&session_id).bind(&id).bind(session_revision).bind(&family_id).execute(&mut *tx).await.is_err() {
        let _ = tx.rollback().await;
        return internal();
    }
    if tx.commit().await.is_err() {
        return internal();
    }
    Json(TokenResponse {
        token_type: "Bearer",
        access_token,
        expires_in: state.access_ttl_seconds as usize,
        refresh_token,
        user: UserView {
            id,
            email,
            display_name,
        },
    })
    .into_response()
}

pub async fn logout(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<RefreshRequest>,
) -> Response {
    let claims = match bearer_claims(&state, &headers) {
        Ok(claims) => claims,
        Err(e) => return e.into_response(),
    };
    let _ = input;
    if !session_token_is_current(&state, &claims).await {
        return unauthorized();
    }
    let mut tx = match state.pool.begin().await {
        Ok(tx) => tx,
        Err(_) => return internal(),
    };
    if !lock_current_user_session(&mut tx, &claims).await {
        let _ = tx.rollback().await;
        return unauthorized();
    }
    match revoke_session_tx(
        &mut tx,
        &claims.session_id,
        &claims.sub,
        "user logout",
        "SESSION_REVOKED",
    )
    .await
    {
        Ok(_) => match tx.commit().await {
            Ok(_) => StatusCode::NO_CONTENT.into_response(),
            Err(_) => internal(),
        },
        Err(_) => internal(),
    }
}

pub async fn revoke_access_token(State(state): State<AppState>, headers: HeaderMap) -> Response {
    let claims = match bearer_claims(&state, &headers) {
        Ok(claims) => claims,
        Err(error) => return error.into_response(),
    };
    if !session_token_is_current(&state, &claims).await {
        return unauthorized();
    }
    let mut tx = match state.pool.begin().await {
        Ok(tx) => tx,
        Err(_) => return internal(),
    };
    if !lock_current_user_session(&mut tx, &claims).await {
        let _ = tx.rollback().await;
        return unauthorized();
    }
    let result = sqlx::query("UPDATE identity_access_tokens SET revoked_at=NOW(),revocation_reason='explicit access-token revocation' WHERE jti=$1 AND session_id=$2 AND user_id=$3 AND session_revision=$4 AND revoked_at IS NULL AND EXISTS (SELECT 1 FROM identity_sessions s WHERE s.id=$2 AND s.user_id=$3 AND s.status='ACTIVE' AND s.revision=$4)")
        .bind(&claims.jti).bind(&claims.session_id).bind(&claims.sub).bind(claims.session_revision).execute(&mut *tx).await;
    match result {
        Ok(result) if result.rows_affected() == 1 => {
            let event = sqlx::query("INSERT INTO identity_session_events(session_id,user_id,actor_user_id,event_type,session_revision,token_jti,reason) VALUES($1,$2,$2,'ACCESS_TOKEN_REVOKED',$3,$4,'explicit access-token revocation')")
                .bind(&claims.session_id).bind(&claims.sub).bind(claims.session_revision).bind(&claims.jti).execute(&mut *tx).await;
            if event.is_err() || tx.commit().await.is_err() {
                return internal();
            }
            StatusCode::NO_CONTENT.into_response()
        }
        Ok(_) => {
            let _ = tx.rollback().await;
            unauthorized()
        }
        Err(_) => internal(),
    }
}

pub async fn me(State(state): State<AppState>, headers: HeaderMap) -> Response {
    let claims = match bearer_claims(&state, &headers) {
        Ok(claims) => claims,
        Err(e) => return e.into_response(),
    };
    if !session_token_is_current(&state, &claims).await {
        return unauthorized();
    }
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

async fn session_token_is_current(state: &AppState, claims: &AccessClaims) -> bool {
    persistence::TradingStore::new(state.pool.clone())
        .validate_access_token(claims)
        .await
        .unwrap_or(false)
}

async fn lock_current_user_session(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    claims: &AccessClaims,
) -> bool {
    let user_active = sqlx::query_scalar::<_, bool>(
        "SELECT is_active FROM identity_users WHERE id=$1 FOR UPDATE",
    )
    .bind(&claims.sub)
    .fetch_optional(&mut **tx)
    .await;
    if !matches!(user_active, Ok(Some(true))) {
        return false;
    }
    let session = sqlx::query_as::<_, (i64, String)>(
        "SELECT revision,status FROM identity_sessions WHERE id=$1 AND user_id=$2 FOR UPDATE",
    )
    .bind(&claims.session_id)
    .bind(&claims.sub)
    .fetch_optional(&mut **tx)
    .await;
    matches!(session, Ok(Some((revision, status))) if revision == claims.session_revision && status == "ACTIVE")
}

async fn revoke_session_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    session_id: &str,
    user_id: &str,
    reason: &str,
    event_type: &str,
) -> Result<i64, sqlx::Error> {
    let revision = sqlx::query_scalar::<_, i64>("UPDATE identity_sessions SET status='REVOKED',revision=revision+1,revoked_at=NOW(),revocation_reason=$3,updated_at=NOW() WHERE id=$1 AND user_id=$2 AND status='ACTIVE' RETURNING revision")
        .bind(session_id).bind(user_id).bind(reason).fetch_optional(&mut **tx).await?;
    let Some(revision) = revision else {
        return Ok(0);
    };
    sqlx::query("UPDATE identity_refresh_tokens SET revoked_at=COALESCE(revoked_at,NOW()),last_used_at=COALESCE(last_used_at,NOW()) WHERE session_id=$1 AND revoked_at IS NULL")
        .bind(session_id).execute(&mut **tx).await?;
    sqlx::query("UPDATE identity_access_tokens SET revoked_at=COALESCE(revoked_at,NOW()),revocation_reason=COALESCE(revocation_reason,$2) WHERE session_id=$1 AND revoked_at IS NULL")
        .bind(session_id).bind(reason).execute(&mut **tx).await?;
    sqlx::query("INSERT INTO identity_session_events(session_id,user_id,actor_user_id,event_type,session_revision,reason) VALUES($1,$2,$2,$3,$4,$5)")
        .bind(session_id).bind(user_id).bind(event_type).bind(revision).bind(reason).execute(&mut **tx).await?;
    Ok(revision)
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

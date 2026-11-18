#![allow(clippy::unwrap_used)]

use chrono::{Duration, Utc};
use identity_token::AccessClaims;
use persistence::{migrate, TradingStore};
use sqlx::PgPool;

async fn seed_user(pool: &PgPool, id: &str, active: bool) {
    sqlx::query("INSERT INTO identity_users(id,email,display_name,password_hash,is_active) VALUES($1,$2,$1,'test-only',$3)")
        .bind(id).bind(format!("{id}@example.test")).bind(active).execute(pool).await.unwrap();
}

async fn seed_session(
    pool: &PgPool,
    user: &str,
    session: &str,
    revision: i64,
    status: &str,
    expires: chrono::DateTime<Utc>,
) {
    let now = Utc::now();
    let revoked_at = (status == "REVOKED").then_some(now);
    let reason = (status == "REVOKED").then_some("test revocation");
    sqlx::query("INSERT INTO identity_sessions(id,user_id,revision,status,issued_at,expires_at,last_used_at,revoked_at,revocation_reason) VALUES($1,$2,$3,$4,$5,$6,$5,$7,$8)")
        .bind(session).bind(user).bind(revision).bind(status).bind(now - Duration::minutes(1)).bind(expires).bind(revoked_at).bind(reason).execute(pool).await.unwrap();
}

async fn seed_access(pool: &PgPool, claims: &AccessClaims, expires: chrono::DateTime<Utc>) {
    sqlx::query("INSERT INTO identity_access_tokens(jti,session_id,user_id,session_revision,issued_at,expires_at) VALUES($1,$2,$3,$4,$5 - INTERVAL '1 second',$5)")
        .bind(&claims.jti).bind(&claims.session_id).bind(&claims.sub).bind(claims.session_revision).bind(expires).execute(pool).await.unwrap();
}

fn claims(user: &str, session: &str, revision: i64, jti: &str) -> AccessClaims {
    let now = Utc::now();
    AccessClaims {
        sub: user.into(),
        iss: "test-issuer".into(),
        aud: "test-audience".into(),
        exp: (now + Duration::minutes(5)).timestamp() as usize,
        iat: now.timestamp() as usize,
        token_type: "access".into(),
        jti: jti.into(),
        session_id: session.into(),
        session_revision: revision,
    }
}

#[sqlx::test]
async fn access_token_requires_active_user_session_revision_and_unrevoked_allowlist_row(
    pool: PgPool,
) {
    migrate(&pool).await.unwrap();
    let store = TradingStore::new(pool.clone());
    let future = Utc::now() + Duration::days(1);

    seed_user(&pool, "i1-active-user", true).await;
    seed_session(
        &pool,
        "i1-active-user",
        &"a".repeat(64),
        2,
        "ACTIVE",
        future,
    )
    .await;
    let current = claims("i1-active-user", &"a".repeat(64), 2, "i1-current");
    seed_access(&pool, &current, Utc::now() + Duration::minutes(5)).await;
    assert!(store.validate_access_token(&current).await.unwrap());

    let stale = claims("i1-active-user", &"a".repeat(64), 1, "i1-stale");
    seed_access(&pool, &stale, Utc::now() + Duration::minutes(5)).await;
    assert!(!store.validate_access_token(&stale).await.unwrap());

    sqlx::query("UPDATE identity_access_tokens SET revoked_at=NOW(),revocation_reason='test revocation' WHERE jti='i1-current'")
        .execute(&pool).await.unwrap();
    assert!(!store.validate_access_token(&current).await.unwrap());

    seed_user(&pool, "i1-inactive-user", false).await;
    seed_session(
        &pool,
        "i1-inactive-user",
        &"b".repeat(64),
        1,
        "ACTIVE",
        future,
    )
    .await;
    let inactive = claims("i1-inactive-user", &"b".repeat(64), 1, "i1-inactive");
    seed_access(&pool, &inactive, Utc::now() + Duration::minutes(5)).await;
    assert!(!store.validate_access_token(&inactive).await.unwrap());
    let status: String = sqlx::query_scalar("SELECT status FROM identity_sessions WHERE id=$1")
        .bind(&inactive.session_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(status, "REVOKED");

    seed_user(&pool, "i1-expired-user", true).await;
    seed_session(
        &pool,
        "i1-expired-user",
        &"c".repeat(64),
        4,
        "ACTIVE",
        Utc::now() - Duration::seconds(1),
    )
    .await;
    let expired = claims("i1-expired-user", &"c".repeat(64), 4, "i1-expired");
    seed_access(&pool, &expired, Utc::now() + Duration::minutes(5)).await;
    assert!(!store.validate_access_token(&expired).await.unwrap());
    let status: String = sqlx::query_scalar("SELECT status FROM identity_sessions WHERE id=$1")
        .bind(&expired.session_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(status, "EXPIRED");
}

#[sqlx::test]
async fn expired_access_token_allowlist_and_revoked_session_are_rejected(pool: PgPool) {
    migrate(&pool).await.unwrap();
    let store = TradingStore::new(pool.clone());
    seed_user(&pool, "i1-revoked-user", true).await;
    seed_session(
        &pool,
        "i1-revoked-user",
        &"d".repeat(64),
        3,
        "REVOKED",
        Utc::now() + Duration::days(1),
    )
    .await;
    let revoked = claims("i1-revoked-user", &"d".repeat(64), 3, "i1-revoked");
    seed_access(&pool, &revoked, Utc::now() + Duration::minutes(5)).await;
    assert!(!store.validate_access_token(&revoked).await.unwrap());

    seed_user(&pool, "i1-token-exp-user", true).await;
    seed_session(
        &pool,
        "i1-token-exp-user",
        &"e".repeat(64),
        1,
        "ACTIVE",
        Utc::now() + Duration::days(1),
    )
    .await;
    let token_expired = claims("i1-token-exp-user", &"e".repeat(64), 1, "i1-token-expired");
    seed_access(&pool, &token_expired, Utc::now() - Duration::seconds(1)).await;
    assert!(!store.validate_access_token(&token_expired).await.unwrap());
}

#[sqlx::test]
async fn runtime_role_guard_rejects_the_migration_or_table_owner(pool: PgPool) {
    migrate(&pool).await.unwrap();
    let current_user_owns_event_table: bool = sqlx::query_scalar(
        "SELECT c.relowner=r.oid FROM pg_class c JOIN pg_roles r ON r.rolname=current_user WHERE c.oid='public.identity_session_events'::regclass",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(current_user_owns_event_table);
    let result = persistence::validate_identity_runtime_role(&pool).await;
    assert!(result.is_err());
}

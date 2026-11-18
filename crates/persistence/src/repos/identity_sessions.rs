use chrono::{DateTime, Utc};
use identity_token::AccessClaims;
use sqlx::PgPool;

use crate::error::PersistenceResult;

pub async fn validate_access_token(
    pool: &PgPool,
    claims: &AccessClaims,
) -> PersistenceResult<bool> {
    let mut tx = pool.begin().await?;
    // Use the same lock order as refresh and suspension: identity user, then
    // session, then the specific access-token row. This prevents a validator
    // from racing suspension while holding session/token locks first.
    let user_active = sqlx::query_scalar::<_, bool>(
        "SELECT is_active FROM identity_users WHERE id=$1 FOR UPDATE",
    )
    .bind(&claims.sub)
    .fetch_optional(&mut *tx)
    .await?;
    let Some(user_active) = user_active else {
        tx.commit().await?;
        return Ok(false);
    };
    let session = sqlx::query_as::<_, (i64, String, DateTime<Utc>)>(
        "SELECT revision,status,expires_at FROM identity_sessions WHERE id=$1 AND user_id=$2 FOR UPDATE",
    )
    .bind(&claims.session_id)
    .bind(&claims.sub)
    .fetch_optional(&mut *tx)
    .await?;
    let Some((revision, status, session_expires)) = session else {
        tx.commit().await?;
        return Ok(false);
    };
    let token = sqlx::query_as::<_, (i64, DateTime<Utc>, Option<DateTime<Utc>>)>(
        "SELECT session_revision,expires_at,revoked_at FROM identity_access_tokens WHERE jti=$1 AND session_id=$2 AND user_id=$3 FOR UPDATE",
    )
    .bind(&claims.jti)
    .bind(&claims.session_id)
    .bind(&claims.sub)
    .fetch_optional(&mut *tx)
    .await?;
    let Some((token_revision, token_expires, revoked_at)) = token else {
        tx.commit().await?;
        return Ok(false);
    };
    let now = Utc::now();
    if !user_active && status == "ACTIVE" {
        revoke_session_in_tx(
            &mut tx,
            &claims.session_id,
            &claims.sub,
            "identity account inactive",
            "SESSION_REVOKED",
        )
        .await?;
        tx.commit().await?;
        return Ok(false);
    }
    if status == "ACTIVE" && session_expires <= now {
        sqlx::query("UPDATE identity_sessions SET status='EXPIRED',revision=revision+1,updated_at=NOW() WHERE id=$1 AND status='ACTIVE' AND expires_at<=NOW()")
            .bind(&claims.session_id).execute(&mut *tx).await?;
        sqlx::query("UPDATE identity_refresh_tokens SET revoked_at=COALESCE(revoked_at,NOW()),last_used_at=COALESCE(last_used_at,NOW()) WHERE session_id=$1 AND revoked_at IS NULL")
            .bind(&claims.session_id).execute(&mut *tx).await?;
        sqlx::query("UPDATE identity_access_tokens SET revoked_at=COALESCE(revoked_at,NOW()),revocation_reason=COALESCE(revocation_reason,'session expired') WHERE session_id=$1 AND revoked_at IS NULL")
            .bind(&claims.session_id).execute(&mut *tx).await?;
        sqlx::query("INSERT INTO identity_session_events(session_id,user_id,event_type,session_revision,reason) VALUES($1,$2,'SESSION_EXPIRED',$3,'session reached expiry')")
            .bind(&claims.session_id).bind(&claims.sub).bind(revision + 1).execute(&mut *tx).await?;
        tx.commit().await?;
        return Ok(false);
    }
    let valid = user_active
        && status == "ACTIVE"
        && revision == claims.session_revision
        && token_revision == claims.session_revision
        && token_expires > now
        && revoked_at.is_none();
    if valid {
        sqlx::query("UPDATE identity_sessions SET last_used_at=NOW(),updated_at=NOW() WHERE id=$1 AND revision=$2 AND status='ACTIVE'")
            .bind(&claims.session_id).bind(claims.session_revision).execute(&mut *tx).await?;
    }
    tx.commit().await?;
    Ok(valid)
}

pub async fn revoke_session_in_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    session_id: &str,
    user_id: &str,
    reason: &str,
    event_type: &str,
) -> PersistenceResult<i64> {
    let revision = sqlx::query_scalar::<_, i64>(
        "UPDATE identity_sessions SET status='REVOKED',revision=revision+1,revoked_at=NOW(),revocation_reason=$3,updated_at=NOW() WHERE id=$1 AND user_id=$2 AND status='ACTIVE' RETURNING revision",
    )
    .bind(session_id)
    .bind(user_id)
    .bind(reason)
    .fetch_optional(&mut **tx)
    .await?;
    let Some(revision) = revision else {
        return Ok(0);
    };
    sqlx::query("UPDATE identity_refresh_tokens SET revoked_at=COALESCE(revoked_at,NOW()),last_used_at=COALESCE(last_used_at,NOW()) WHERE session_id=$1 AND revoked_at IS NULL")
        .bind(session_id).execute(&mut **tx).await?;
    sqlx::query("UPDATE identity_access_tokens SET revoked_at=COALESCE(revoked_at,NOW()),revocation_reason=COALESCE(revocation_reason,$2) WHERE session_id=$1 AND revoked_at IS NULL")
        .bind(session_id).bind(reason).execute(&mut **tx).await?;
    sqlx::query("INSERT INTO identity_session_events(session_id,user_id,event_type,session_revision,reason) VALUES($1,$2,$3,$4,$5)")
        .bind(session_id).bind(user_id).bind(event_type).bind(revision).bind(reason).execute(&mut **tx).await?;
    Ok(revision)
}

//! Durable, fenced single-publisher leases for MT5 account/terminal streams.

use chrono::{DateTime, Utc};
use sha2::{Digest, Sha256};
use sqlx::PgPool;

use crate::error::{PersistenceError, PersistenceResult};

const MIN_LEASE_SECONDS: i64 = 5;
const MAX_LEASE_SECONDS: i64 = 300;

/// Process-local lease capability. The token is deliberately redacted from Debug.
#[derive(Clone)]
pub struct Mt5WorkerLease {
    pub quant_account_id: String,
    pub terminal_id: String,
    pub assignment_id: String,
    pub worker_ref: String,
    token: String,
    pub fencing_token: i64,
    pub lease_until: DateTime<Utc>,
}

impl std::fmt::Debug for Mt5WorkerLease {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Mt5WorkerLease")
            .field("quant_account_id", &self.quant_account_id)
            .field("terminal_id", &self.terminal_id)
            .field("assignment_id", &self.assignment_id)
            .field("worker_ref", &self.worker_ref)
            .field("token", &"[REDACTED]")
            .field("fencing_token", &self.fencing_token)
            .field("lease_until", &self.lease_until)
            .finish()
    }
}

impl Mt5WorkerLease {
    fn new(
        quant_account_id: String,
        terminal_id: String,
        assignment_id: String,
        worker_ref: String,
        token: String,
        fencing_token: i64,
        lease_until: DateTime<Utc>,
    ) -> Self {
        Self {
            quant_account_id,
            terminal_id,
            assignment_id,
            worker_ref,
            token,
            fencing_token,
            lease_until,
        }
    }

    fn token_hash(&self) -> Vec<u8> {
        Sha256::digest(self.token.as_bytes()).to_vec()
    }
}

fn validate_scope(
    quant_account_id: &str,
    terminal_id: &str,
    binding_id: &str,
    worker_ref: &str,
    token: &str,
    lease_seconds: i64,
) -> PersistenceResult<()> {
    if [quant_account_id, terminal_id, binding_id, worker_ref]
        .iter()
        .any(|value| value.trim().is_empty() || value.trim() != *value)
        || token.len() < 32
        || !(MIN_LEASE_SECONDS..=MAX_LEASE_SECONDS).contains(&lease_seconds)
    {
        return Err(PersistenceError::Other(
            "invalid MT5 worker lease scope, token, or duration".into(),
        ));
    }
    Ok(())
}

/// Atomically acquire the only publisher lease for an account/terminal pair.
/// An unexpired lease always blocks acquisition, including attempts by the same
/// worker identity. Expiry/release takeover allocates a new fencing token.
pub async fn acquire(
    pool: &PgPool,
    quant_account_id: &str,
    terminal_id: &str,
    binding_id: &str,
    worker_ref: &str,
    token: &str,
    lease_seconds: i64,
) -> PersistenceResult<Mt5WorkerLease> {
    validate_scope(
        quant_account_id,
        terminal_id,
        binding_id,
        worker_ref,
        token,
        lease_seconds,
    )?;
    let token_hash = Sha256::digest(token.as_bytes()).to_vec();
    let row = sqlx::query_as::<_, (String, i64, DateTime<Utc>)>(
        r#"
        WITH assignment AS (
            SELECT id
              FROM broker_worker_assignments
             WHERE binding_id = $1
               AND quant_account_id = $2
               AND worker_ref = $3
               AND status = 'ACTIVE'
               AND valid_from <= NOW()
               AND valid_until > NOW()
             FOR SHARE
        )
        INSERT INTO broker_worker_leases
            (quant_account_id, terminal_id, assignment_id, worker_ref,
             lease_token_hash, fencing_token, lease_until, acquired_at, renewed_at, released_at)
        SELECT $2, $4, assignment.id, $3, $5,
               1,
               NOW() + make_interval(secs => $6), NOW(), NOW(), NULL
          FROM assignment
        ON CONFLICT (quant_account_id, terminal_id) DO UPDATE SET
            assignment_id = EXCLUDED.assignment_id,
            worker_ref = EXCLUDED.worker_ref,
            lease_token_hash = EXCLUDED.lease_token_hash,
            fencing_token = broker_worker_leases.fencing_token + 1,
            lease_until = EXCLUDED.lease_until,
            acquired_at = NOW(),
            renewed_at = NOW(),
            released_at = NULL
        WHERE broker_worker_leases.lease_until <= NOW()
           OR broker_worker_leases.released_at IS NOT NULL
        RETURNING assignment_id, fencing_token, lease_until
        "#,
    )
    .bind(binding_id)
    .bind(quant_account_id)
    .bind(worker_ref)
    .bind(terminal_id)
    .bind(token_hash)
    .bind(lease_seconds as f64)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| {
        PersistenceError::Other(
            "MT5 publisher lease unavailable: active lease or worker assignment missing".into(),
        )
    })?;

    Ok(Mt5WorkerLease::new(
        quant_account_id.to_owned(),
        terminal_id.to_owned(),
        row.0,
        worker_ref.to_owned(),
        token.to_owned(),
        row.1,
        row.2,
    ))
}

/// Renew only a live lease held by the exact process token and fencing generation.
/// An expired lease cannot be revived; it must be reacquired with a new fence.
pub async fn renew(
    pool: &PgPool,
    lease: &Mt5WorkerLease,
    lease_seconds: i64,
) -> PersistenceResult<DateTime<Utc>> {
    if !(MIN_LEASE_SECONDS..=MAX_LEASE_SECONDS).contains(&lease_seconds) {
        return Err(PersistenceError::Other("invalid MT5 lease duration".into()));
    }
    let renewed = sqlx::query_scalar::<_, DateTime<Utc>>(
        r#"
        UPDATE broker_worker_leases
           SET lease_until = NOW() + make_interval(secs => $5),
               renewed_at = NOW()
         WHERE quant_account_id = $1 AND terminal_id = $2
           AND assignment_id = $3 AND worker_ref = $4
           AND lease_token_hash = $6 AND fencing_token = $7
           AND released_at IS NULL AND lease_until > NOW()
        RETURNING lease_until
        "#,
    )
    .bind(&lease.quant_account_id)
    .bind(&lease.terminal_id)
    .bind(&lease.assignment_id)
    .bind(&lease.worker_ref)
    .bind(lease_seconds as f64)
    .bind(lease.token_hash())
    .bind(lease.fencing_token)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| {
        PersistenceError::Other("MT5 publisher lease lost; refusing to continue".into())
    })?;
    Ok(renewed)
}

/// Verify a lease is still current. Database errors propagate and must be treated
/// as loss of permission by callers.
pub async fn validate(pool: &PgPool, lease: &Mt5WorkerLease) -> PersistenceResult<()> {
    let current: bool = sqlx::query_scalar(
        r#"
        SELECT EXISTS (
            SELECT 1 FROM broker_worker_leases
             WHERE quant_account_id = $1 AND terminal_id = $2
               AND assignment_id = $3 AND worker_ref = $4
               AND lease_token_hash = $5 AND fencing_token = $6
               AND released_at IS NULL AND lease_until > NOW()
        )
        "#,
    )
    .bind(&lease.quant_account_id)
    .bind(&lease.terminal_id)
    .bind(&lease.assignment_id)
    .bind(&lease.worker_ref)
    .bind(lease.token_hash())
    .bind(lease.fencing_token)
    .fetch_one(pool)
    .await?;
    if current {
        Ok(())
    } else {
        Err(PersistenceError::Other(
            "MT5 publisher lease is no longer current".into(),
        ))
    }
}

/// Release is compare-and-set: a stale process cannot release a successor's lease.
pub async fn release(pool: &PgPool, lease: &Mt5WorkerLease) -> PersistenceResult<bool> {
    let result = sqlx::query(
        r#"
        UPDATE broker_worker_leases
           SET released_at = NOW(), lease_until = NOW(), renewed_at = NOW()
         WHERE quant_account_id = $1 AND terminal_id = $2
           AND assignment_id = $3 AND worker_ref = $4
           AND lease_token_hash = $5 AND fencing_token = $6
           AND released_at IS NULL
        "#,
    )
    .bind(&lease.quant_account_id)
    .bind(&lease.terminal_id)
    .bind(&lease.assignment_id)
    .bind(&lease.worker_ref)
    .bind(lease.token_hash())
    .bind(lease.fencing_token)
    .execute(pool)
    .await?;
    Ok(result.rows_affected() == 1)
}

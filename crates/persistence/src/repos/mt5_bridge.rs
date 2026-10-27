use chrono::{DateTime, Utc};
use serde_json::Value;
use sqlx::{types::Json, PgPool};

use crate::error::{PersistenceError, PersistenceResult};

/// Durable result of claiming an MT5 client-order key before any network send.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Mt5OrderClaim {
    /// This process created and committed the key. Only this result authorizes
    /// the caller to perform the first send attempt.
    New,
    /// A key already existed. Even "prepared" can mean a prior process crashed
    /// after sending, so the caller must reconcile rather than resubmit.
    Existing {
        state: Mt5DurableOrderState,
        venue_order_id: Option<String>,
        reason_code: Option<String>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mt5DurableOrderState {
    Prepared,
    Unknown,
    Accepted,
    Rejected,
}

impl Mt5DurableOrderState {
    fn as_str(self) -> &'static str {
        match self {
            Self::Prepared => "prepared",
            Self::Unknown => "unknown",
            Self::Accepted => "accepted",
            Self::Rejected => "rejected",
        }
    }
}

fn parse_state(value: &str) -> PersistenceResult<Mt5DurableOrderState> {
    match value {
        "prepared" => Ok(Mt5DurableOrderState::Prepared),
        "unknown" => Ok(Mt5DurableOrderState::Unknown),
        "accepted" => Ok(Mt5DurableOrderState::Accepted),
        "rejected" => Ok(Mt5DurableOrderState::Rejected),
        other => Err(PersistenceError::Other(format!(
            "unknown durable MT5 order state: {other}"
        ))),
    }
}

/// Claim an account-scoped client order ID. The canonical request is stored as
/// JSONB and compared structurally. This is transactional across processes.
pub async fn claim_order(
    pool: &PgPool,
    account_id: &str,
    client_order_id: &str,
    request_payload: Value,
) -> PersistenceResult<Mt5OrderClaim> {
    if account_id.trim().is_empty() || client_order_id.trim().is_empty() {
        return Err(PersistenceError::Other(
            "MT5 account ID and client order ID must be non-empty".into(),
        ));
    }

    let mut tx = pool.begin().await?;
    let inserted = sqlx::query(
        r#"
        INSERT INTO mt5_bridge_order_keys
            (account_id, client_order_id, request_payload, state)
        VALUES ($1, $2, $3, 'prepared')
        ON CONFLICT (account_id, client_order_id) DO NOTHING
        RETURNING client_order_id
        "#,
    )
    .bind(account_id)
    .bind(client_order_id)
    .bind(Json(request_payload.clone()))
    .fetch_optional(&mut *tx)
    .await?;

    if inserted.is_some() {
        tx.commit().await?;
        return Ok(Mt5OrderClaim::New);
    }

    let existing = sqlx::query_as::<_, (Json<Value>, String, Option<String>, Option<String>)>(
        r#"
        SELECT request_payload, state, venue_order_id, reason_code
        FROM mt5_bridge_order_keys
        WHERE account_id = $1 AND client_order_id = $2
        FOR UPDATE
        "#,
    )
    .bind(account_id)
    .bind(client_order_id)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or_else(|| {
        PersistenceError::Other("MT5 idempotency key disappeared during claim".into())
    })?;

    let (Json(existing_payload), existing_state, venue_order_id, reason_code) = existing;
    if existing_payload != request_payload {
        return Err(PersistenceError::Other(format!(
            "MT5 client order ID conflict for account {account_id}: canonical payload differs"
        )));
    }

    let claim = Mt5OrderClaim::Existing {
        state: parse_state(&existing_state)?,
        venue_order_id,
        reason_code,
    };
    tx.commit().await?;
    Ok(claim)
}

/// Persist a definitive or ambiguous transport outcome. The state can move
/// from prepared/unknown to an observed outcome, but accepted/rejected records
/// are immutable to prevent late responses rewriting terminal knowledge.
pub async fn record_outcome(
    pool: &PgPool,
    account_id: &str,
    client_order_id: &str,
    state: Mt5DurableOrderState,
    venue_order_id: Option<&str>,
    reason_code: Option<&str>,
) -> PersistenceResult<()> {
    if state == Mt5DurableOrderState::Prepared {
        return Err(PersistenceError::Other(
            "outcome cannot transition back to prepared".into(),
        ));
    }

    let updated = sqlx::query(
        r#"
        UPDATE mt5_bridge_order_keys
        SET state = $3,
            venue_order_id = COALESCE($4, venue_order_id),
            reason_code = COALESCE($5, reason_code),
            updated_at = NOW()
        WHERE account_id = $1 AND client_order_id = $2
          AND state IN ('prepared', 'unknown')
        "#,
    )
    .bind(account_id)
    .bind(client_order_id)
    .bind(state.as_str())
    .bind(venue_order_id)
    .bind(reason_code)
    .execute(pool)
    .await?;

    if updated.rows_affected() == 1 {
        return Ok(());
    }

    let current = sqlx::query_as::<_, (String, Option<String>)>(
        "SELECT state, venue_order_id FROM mt5_bridge_order_keys WHERE account_id = $1 AND client_order_id = $2",
    )
    .bind(account_id)
    .bind(client_order_id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| PersistenceError::NotFound(format!(
        "MT5 order key {account_id}/{client_order_id}"
    )))?;

    if current.0 == state.as_str()
        && venue_order_id.map_or(true, |id| current.1.as_deref() == Some(id))
    {
        return Ok(());
    }

    Err(PersistenceError::Other(format!(
        "refusing conflicting MT5 outcome transition from {} to {}",
        current.0,
        state.as_str()
    )))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mt5SequenceObservation {
    /// Sequence was the next expected value on a previously reconciled stream.
    Accepted,
    /// New stream, gap, duplicate, or out-of-order sequence: exposure must stay blocked.
    ReconciliationRequired,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mt5StreamReadiness {
    Ready,
    Degraded,
}

/// Persist an incoming sequence. A new stream is degraded until explicitly
/// reconciled. Only an exact next sequence preserves Ready status.
pub async fn observe_sequence(
    pool: &PgPool,
    account_id: &str,
    terminal_id: &str,
    sequence: u64,
) -> PersistenceResult<Mt5SequenceObservation> {
    let sequence = i64::try_from(sequence).map_err(|_| {
        PersistenceError::Other("MT5 sequence exceeds PostgreSQL BIGINT range".into())
    })?;
    if account_id.trim().is_empty() || terminal_id.trim().is_empty() || sequence < 0 {
        return Err(PersistenceError::Other(
            "MT5 account ID, terminal ID and non-negative sequence are required".into(),
        ));
    }

    let mut tx = pool.begin().await?;
    let current = sqlx::query_as::<_, (i64, String)>(
        "SELECT last_sequence, readiness FROM mt5_bridge_streams WHERE account_id = $1 AND terminal_id = $2 FOR UPDATE",
    )
    .bind(account_id)
    .bind(terminal_id)
    .fetch_optional(&mut *tx)
    .await?;

    let (observation, next_sequence, readiness) = match current {
        None => (
            Mt5SequenceObservation::ReconciliationRequired,
            sequence,
            "degraded",
        ),
        Some((last, readiness))
            if readiness == "ready" && last.checked_add(1) == Some(sequence) =>
        {
            (Mt5SequenceObservation::Accepted, sequence, "ready")
        }
        Some((last, _)) => (
            Mt5SequenceObservation::ReconciliationRequired,
            last,
            "degraded",
        ),
    };

    sqlx::query(
        r#"
        INSERT INTO mt5_bridge_streams (account_id, terminal_id, last_sequence, readiness)
        VALUES ($1, $2, $3, $4)
        ON CONFLICT (account_id, terminal_id) DO UPDATE SET
            last_sequence = EXCLUDED.last_sequence,
            readiness = EXCLUDED.readiness,
            updated_at = NOW()
        "#,
    )
    .bind(account_id)
    .bind(terminal_id)
    .bind(next_sequence)
    .bind(readiness)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(observation)
}

/// Reconciliation must explicitly establish a new checkpoint before the
/// stream can become ready after startup, reconnect, or a sequence gap.
pub async fn mark_stream_reconciled(
    pool: &PgPool,
    account_id: &str,
    terminal_id: &str,
    reconciled_sequence: u64,
) -> PersistenceResult<()> {
    let sequence = i64::try_from(reconciled_sequence).map_err(|_| {
        PersistenceError::Other("MT5 sequence exceeds PostgreSQL BIGINT range".into())
    })?;
    if account_id.trim().is_empty() || terminal_id.trim().is_empty() || sequence < 0 {
        return Err(PersistenceError::Other(
            "MT5 account ID, terminal ID and non-negative sequence are required".into(),
        ));
    }
    let snapshot_sequence = sqlx::query_scalar::<_, i64>(
        "SELECT MAX(reconciled_sequence) FROM mt5_bridge_reconciliation_snapshots WHERE account_id = $1 AND terminal_id = $2",
    )
    .bind(account_id)
    .bind(terminal_id)
    .fetch_one(pool)
    .await?;
    if snapshot_sequence.is_none_or(|snapshot_sequence| snapshot_sequence < sequence) {
        return Err(PersistenceError::Other(
            "MT5 stream cannot become ready without a durable external-state reconciliation snapshot at or beyond the requested sequence".into(),
        ));
    }
    sqlx::query(
        r#"
        INSERT INTO mt5_bridge_streams (account_id, terminal_id, last_sequence, readiness)
        VALUES ($1, $2, $3, 'ready')
        ON CONFLICT (account_id, terminal_id) DO UPDATE SET
            last_sequence = EXCLUDED.last_sequence,
            readiness = 'ready',
            updated_at = NOW()
        "#,
    )
    .bind(account_id)
    .bind(terminal_id)
    .bind(sequence)
    .execute(pool)
    .await?;
    Ok(())
}

/// Mark a stream degraded after a disconnected heartbeat or invalid/stale frame.
/// Existing sequence checkpoints are retained so only explicit reconciliation can
/// advance the checkpoint and restore readiness.
pub async fn mark_stream_degraded(
    pool: &PgPool,
    account_id: &str,
    terminal_id: &str,
    observed_sequence: u64,
) -> PersistenceResult<()> {
    let sequence = i64::try_from(observed_sequence).map_err(|_| {
        PersistenceError::Other("MT5 sequence exceeds PostgreSQL BIGINT range".into())
    })?;
    if account_id.trim().is_empty() || terminal_id.trim().is_empty() || sequence < 0 {
        return Err(PersistenceError::Other(
            "MT5 account ID, terminal ID and non-negative sequence are required".into(),
        ));
    }
    sqlx::query(
        r#"
        INSERT INTO mt5_bridge_streams (account_id, terminal_id, last_sequence, readiness)
        VALUES ($1, $2, $3, 'degraded')
        ON CONFLICT (account_id, terminal_id) DO UPDATE SET
            readiness = 'degraded',
            updated_at = NOW()
        "#,
    )
    .bind(account_id)
    .bind(terminal_id)
    .bind(sequence)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn stream_readiness(
    pool: &PgPool,
    account_id: &str,
    terminal_id: &str,
) -> PersistenceResult<Mt5StreamReadiness> {
    let readiness = sqlx::query_scalar::<_, String>(
        "SELECT readiness FROM mt5_bridge_streams WHERE account_id = $1 AND terminal_id = $2",
    )
    .bind(account_id)
    .bind(terminal_id)
    .fetch_optional(pool)
    .await?;
    match readiness.as_deref() {
        Some("ready") => Ok(Mt5StreamReadiness::Ready),
        Some("degraded") | None => Ok(Mt5StreamReadiness::Degraded),
        Some(other) => Err(PersistenceError::Other(format!(
            "unknown MT5 stream readiness: {other}"
        ))),
    }
}

/// Persist a complete authoritative external-state snapshot and advance the stream
/// checkpoint in one transaction. A snapshot must contain account state, open orders,
/// and positions; a lower sequence than the durable checkpoint is rejected.
pub async fn persist_reconciliation_snapshot(
    pool: &PgPool,
    account_id: &str,
    terminal_id: &str,
    sequence: u64,
    source: &str,
    captured_at: DateTime<Utc>,
    snapshot: Value,
) -> PersistenceResult<()> {
    let sequence = i64::try_from(sequence).map_err(|_| {
        PersistenceError::Other(
            "MT5 reconciliation sequence exceeds PostgreSQL BIGINT range".into(),
        )
    })?;
    if account_id.trim().is_empty()
        || terminal_id.trim().is_empty()
        || source.trim().is_empty()
        || sequence < 0
        || snapshot.get("account_id").and_then(Value::as_str) != Some(account_id)
        || snapshot.get("terminal_id").and_then(Value::as_str) != Some(terminal_id)
        || snapshot.get("source").and_then(Value::as_str) != Some(source)
        || !snapshot.get("account_state").is_some_and(Value::is_object)
        || !snapshot.get("open_orders").is_some_and(Value::is_array)
        || !snapshot.get("positions").is_some_and(Value::is_array)
    {
        return Err(PersistenceError::Other(
            "MT5 reconciliation requires matching identities and complete account/order/position state".into(),
        ));
    }

    let mut tx = pool.begin().await?;
    // Serialize even the first reconciliation, when no stream row exists to lock.
    sqlx::query("SELECT pg_advisory_xact_lock(hashtext($1), hashtext($2))")
        .bind(account_id)
        .bind(terminal_id)
        .execute(&mut *tx)
        .await?;
    let current = sqlx::query_scalar::<_, i64>(
        "SELECT last_sequence FROM mt5_bridge_streams WHERE account_id = $1 AND terminal_id = $2 FOR UPDATE",
    )
    .bind(account_id)
    .bind(terminal_id)
    .fetch_optional(&mut *tx)
    .await?;
    if current.is_some_and(|last| sequence < last) {
        return Err(PersistenceError::Other(
            "MT5 reconciliation sequence cannot move behind the durable checkpoint".into(),
        ));
    }

    sqlx::query(
        r#"
        INSERT INTO mt5_bridge_reconciliation_snapshots
            (account_id, terminal_id, reconciled_sequence, source, captured_at, snapshot)
        VALUES ($1, $2, $3, $4, $5, $6)
        "#,
    )
    .bind(account_id)
    .bind(terminal_id)
    .bind(sequence)
    .bind(source)
    .bind(captured_at)
    .bind(Json(snapshot))
    .execute(&mut *tx)
    .await?;

    sqlx::query(
        r#"
        INSERT INTO mt5_bridge_streams (account_id, terminal_id, last_sequence, readiness)
        VALUES ($1, $2, $3, 'ready')
        ON CONFLICT (account_id, terminal_id) DO UPDATE SET
            last_sequence = EXCLUDED.last_sequence,
            readiness = 'ready',
            updated_at = NOW()
        "#,
    )
    .bind(account_id)
    .bind(terminal_id)
    .bind(sequence)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(())
}

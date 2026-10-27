use chrono::{DateTime, Utc};
use domain::{AccountId, InstrumentId, TradeIntentId};
use serde_json::Value;
use sqlx::{types::Json, PgPool};

use crate::error::PersistenceResult;

/// Append a complete sizing-decision audit record. Rows are insert-only so
/// later reconciliation never rewrites the historical decision inputs.
pub async fn insert(
    pool: &PgPool,
    trade_intent_id: &TradeIntentId,
    account_id: &AccountId,
    instrument_id: &InstrumentId,
    source_id: &str,
    snapshot_captured_at: DateTime<Utc>,
    evaluated_at: DateTime<Utc>,
    payload: Value,
) -> PersistenceResult<()> {
    sqlx::query(
        r#"
        INSERT INTO sizing_decision_audits (
            trade_intent_id, account_id, instrument_id, source_id,
            snapshot_captured_at, evaluated_at, payload_json
        ) VALUES ($1,$2,$3,$4,$5,$6,$7)
        "#,
    )
    .bind(trade_intent_id.as_str())
    .bind(account_id.as_str())
    .bind(instrument_id.as_str())
    .bind(source_id)
    .bind(snapshot_captured_at)
    .bind(evaluated_at)
    .bind(Json(payload))
    .execute(pool)
    .await?;
    Ok(())
}

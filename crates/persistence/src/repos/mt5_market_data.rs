//! Read access to the latest persisted MT5 market-data snapshot.

use chrono::{DateTime, Utc};
use serde_json::Value;
use sqlx::{types::Json, PgPool};

use crate::error::{PersistenceError, PersistenceResult};

#[derive(Debug, Clone, PartialEq)]
pub struct Mt5MarketDataRecord {
    pub account_id: String,
    pub terminal_id: String,
    pub server: String,
    pub sequence: u64,
    pub source: String,
    pub account_currency: String,
    pub captured_at: DateTime<Utc>,
    pub snapshot: Value,
}

/// Load only the latest snapshot for the exact account/terminal pair.
/// Freshness remains a caller-side risk decision and must be checked before sizing.
pub async fn load_latest(
    pool: &PgPool,
    account_id: &str,
    terminal_id: &str,
) -> PersistenceResult<Option<Mt5MarketDataRecord>> {
    if account_id.trim().is_empty() || terminal_id.trim().is_empty() {
        return Err(PersistenceError::Other(
            "MT5 account and terminal identities are required".into(),
        ));
    }
    let row = sqlx::query_as::<
        _,
        (
            String,
            String,
            String,
            i64,
            String,
            String,
            DateTime<Utc>,
            Json<Value>,
        ),
    >(
        r#"SELECT account_id, terminal_id, server, sequence, source, account_currency, captured_at, snapshot
           FROM mt5_bridge_market_data_snapshots
           WHERE account_id = $1 AND terminal_id = $2"#,
    )
    .bind(account_id)
    .bind(terminal_id)
    .fetch_optional(pool)
    .await?;

    row.map(
        |(
            account_id,
            terminal_id,
            server,
            sequence,
            source,
            account_currency,
            captured_at,
            Json(snapshot),
        )| {
            let sequence = u64::try_from(sequence).map_err(|_| {
                PersistenceError::Other("persisted MT5 sequence is negative".into())
            })?;
            if snapshot.get("terminal_id").and_then(Value::as_str) != Some(terminal_id.as_str())
                || snapshot.get("server").and_then(Value::as_str) != Some(server.as_str())
                || snapshot.get("source").and_then(Value::as_str) != Some(source.as_str())
                || snapshot.get("account_currency").and_then(Value::as_str)
                    != Some(account_currency.as_str())
            {
                return Err(PersistenceError::Other(
                    "persisted MT5 market-data identity does not match its database key".into(),
                ));
            }
            Ok(Mt5MarketDataRecord {
                account_id,
                terminal_id,
                server,
                sequence,
                source,
                account_currency,
                captured_at,
                snapshot,
            })
        },
    )
    .transpose()
}

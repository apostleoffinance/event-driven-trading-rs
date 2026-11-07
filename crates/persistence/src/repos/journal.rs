//! Owner-scoped journal persistence.
//!
//! All methods require an owner ID from the authenticated application layer.
//! Never accept owner_id from an untrusted client without binding it to the
//! authenticated principal. These methods do not write OMS records.

use chrono::{DateTime, Utc};
use domain::{JournalDirection, JournalTrade, JournalTradeSource, JournalTradeStatus};
use rust_decimal::Decimal;
use sqlx::{FromRow, PgPool};

use crate::error::{PersistenceError, PersistenceResult};

#[derive(Debug, Clone, FromRow)]
struct JournalTradeRow {
    id: String,
    owner_id: String,
    journal_account_id: String,
    setup_id: Option<String>,
    symbol: String,
    direction: String,
    status: String,
    source: String,
    source_system: Option<String>,
    source_trade_id: Option<String>,
    opened_at: DateTime<Utc>,
    closed_at: Option<DateTime<Utc>>,
    quantity: Decimal,
    entry_price: Decimal,
    exit_price: Option<Decimal>,
    stop_loss: Option<Decimal>,
    take_profit: Option<Decimal>,
    gross_pnl: Option<Decimal>,
    commission: Decimal,
    swap: Decimal,
    other_fees: Decimal,
    net_pnl: Option<Decimal>,
    pnl_currency: String,
    risk_amount: Option<Decimal>,
    r_multiple: Option<Decimal>,
    session: Option<String>,
    timeframe: Option<String>,
    timezone: Option<String>,
}

impl TryFrom<JournalTradeRow> for JournalTrade {
    type Error = PersistenceError;

    fn try_from(row: JournalTradeRow) -> Result<Self, Self::Error> {
        let direction = match row.direction.as_str() {
            "long" => JournalDirection::Long,
            "short" => JournalDirection::Short,
            other => {
                return Err(PersistenceError::Other(format!(
                    "invalid journal direction: {other}"
                )))
            }
        };
        let status = match row.status.as_str() {
            "open" => JournalTradeStatus::Open,
            "closed" => JournalTradeStatus::Closed,
            "cancelled" => JournalTradeStatus::Cancelled,
            other => {
                return Err(PersistenceError::Other(format!(
                    "invalid journal status: {other}"
                )))
            }
        };
        let source = match row.source.as_str() {
            "manual" => JournalTradeSource::Manual,
            "mt5" => JournalTradeSource::Mt5,
            "legacy_import" => JournalTradeSource::LegacyImport,
            "other" => JournalTradeSource::Other,
            other => {
                return Err(PersistenceError::Other(format!(
                    "invalid journal source: {other}"
                )))
            }
        };
        Ok(JournalTrade {
            id: row.id,
            owner_id: row.owner_id,
            journal_account_id: row.journal_account_id,
            setup_id: row.setup_id,
            symbol: row.symbol,
            direction,
            status,
            source,
            source_system: row.source_system,
            source_trade_id: row.source_trade_id,
            opened_at: row.opened_at,
            closed_at: row.closed_at,
            quantity: row.quantity,
            entry_price: row.entry_price,
            exit_price: row.exit_price,
            stop_loss: row.stop_loss,
            take_profit: row.take_profit,
            gross_pnl: row.gross_pnl,
            commission: row.commission,
            swap: row.swap,
            other_fees: row.other_fees,
            net_pnl: row.net_pnl,
            pnl_currency: row.pnl_currency,
            risk_amount: row.risk_amount,
            r_multiple: row.r_multiple,
            session: row.session,
            timeframe: row.timeframe,
            timezone: row.timezone,
        })
    }
}

fn direction(value: JournalDirection) -> &'static str {
    match value {
        JournalDirection::Long => "long",
        JournalDirection::Short => "short",
    }
}
fn status(value: JournalTradeStatus) -> &'static str {
    match value {
        JournalTradeStatus::Open => "open",
        JournalTradeStatus::Closed => "closed",
        JournalTradeStatus::Cancelled => "cancelled",
    }
}
fn source(value: JournalTradeSource) -> &'static str {
    match value {
        JournalTradeSource::Manual => "manual",
        JournalTradeSource::Mt5 => "mt5",
        JournalTradeSource::LegacyImport => "legacy_import",
        JournalTradeSource::Other => "other",
    }
}

pub async fn insert(pool: &PgPool, trade: &JournalTrade) -> PersistenceResult<()> {
    trade.validate()?;
    sqlx::query(
        r#"INSERT INTO journal_trades (
            id, owner_id, journal_account_id, setup_id, symbol, direction, status, source,
            source_system, source_trade_id, opened_at, closed_at, quantity, entry_price,
            exit_price, stop_loss, take_profit, gross_pnl, commission, swap, other_fees,
            net_pnl, pnl_currency, risk_amount, r_multiple, session, timeframe, timezone
        ) VALUES (
            $1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16,$17,$18,$19,$20,$21,
            $22,$23,$24,$25,$26,$27,$28
        ) ON CONFLICT (journal_account_id, source_system, source_trade_id)
        WHERE source_system IS NOT NULL AND source_trade_id IS NOT NULL DO NOTHING"#,
    )
    .bind(&trade.id)
    .bind(&trade.owner_id)
    .bind(&trade.journal_account_id)
    .bind(&trade.setup_id)
    .bind(&trade.symbol)
    .bind(direction(trade.direction))
    .bind(status(trade.status))
    .bind(source(trade.source))
    .bind(&trade.source_system)
    .bind(&trade.source_trade_id)
    .bind(trade.opened_at)
    .bind(trade.closed_at)
    .bind(trade.quantity)
    .bind(trade.entry_price)
    .bind(trade.exit_price)
    .bind(trade.stop_loss)
    .bind(trade.take_profit)
    .bind(trade.gross_pnl)
    .bind(trade.commission)
    .bind(trade.swap)
    .bind(trade.other_fees)
    .bind(trade.net_pnl)
    .bind(&trade.pnl_currency)
    .bind(trade.risk_amount)
    .bind(trade.r_multiple)
    .bind(&trade.session)
    .bind(&trade.timeframe)
    .bind(&trade.timezone)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn list_for_owner(
    pool: &PgPool,
    owner_id: &str,
    account_id: Option<&str>,
    limit: i64,
    offset: i64,
) -> PersistenceResult<Vec<JournalTrade>> {
    if owner_id.trim().is_empty() {
        return Err(PersistenceError::Other("owner_id must be non-empty".into()));
    }
    let limit = limit.clamp(1, 500);
    let offset = offset.max(0);
    let rows = sqlx::query_as::<_, JournalTradeRow>(
        r#"SELECT id, owner_id, journal_account_id, setup_id, symbol, direction, status, source,
            source_system, source_trade_id, opened_at, closed_at, quantity, entry_price,
            exit_price, stop_loss, take_profit, gross_pnl, commission, swap, other_fees,
            net_pnl, pnl_currency, risk_amount, r_multiple, session, timeframe, timezone
          FROM journal_trades
          WHERE owner_id = $1 AND ($2::TEXT IS NULL OR journal_account_id = $2)
          ORDER BY opened_at DESC, id DESC LIMIT $3 OFFSET $4"#,
    )
    .bind(owner_id)
    .bind(account_id)
    .bind(limit)
    .bind(offset)
    .fetch_all(pool)
    .await?;
    rows.into_iter().map(JournalTrade::try_from).collect()
}

pub async fn get_for_owner(
    pool: &PgPool,
    owner_id: &str,
    trade_id: &str,
) -> PersistenceResult<JournalTrade> {
    let row = sqlx::query_as::<_, JournalTradeRow>(
        r#"SELECT id, owner_id, journal_account_id, setup_id, symbol, direction, status, source,
            source_system, source_trade_id, opened_at, closed_at, quantity, entry_price,
            exit_price, stop_loss, take_profit, gross_pnl, commission, swap, other_fees,
            net_pnl, pnl_currency, risk_amount, r_multiple
          FROM journal_trades WHERE owner_id = $1 AND id = $2"#,
    )
    .bind(owner_id)
    .bind(trade_id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| PersistenceError::NotFound(format!("journal trade {trade_id}")))?;
    JournalTrade::try_from(row)
}

pub async fn delete_for_owner(
    pool: &PgPool,
    owner_id: &str,
    trade_id: &str,
) -> PersistenceResult<bool> {
    let result = sqlx::query("DELETE FROM journal_trades WHERE owner_id = $1 AND id = $2")
        .bind(owner_id)
        .bind(trade_id)
        .execute(pool)
        .await?;
    Ok(result.rows_affected() == 1)
}

use domain::{AccountId, Position, PositionSide};
use rust_decimal::Decimal;
use sqlx::PgPool;

use crate::error::{PersistenceError, PersistenceResult};

fn side_str(s: PositionSide) -> &'static str {
    match s {
        PositionSide::Long => "long",
        PositionSide::Short => "short",
    }
}

fn parse_side(s: &str) -> PersistenceResult<PositionSide> {
    match s {
        "long" => Ok(PositionSide::Long),
        "short" => Ok(PositionSide::Short),
        o => Err(PersistenceError::Other(format!(
            "unknown position side {o}"
        ))),
    }
}

pub async fn upsert(pool: &PgPool, position: &Position) -> PersistenceResult<()> {
    sqlx::query(
        r#"
        INSERT INTO positions (
            id, account_id, instrument_id, side, quantity, avg_entry_price,
            stop_loss, mark_price, realized_pnl, opened_at, updated_at
        ) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11)
        ON CONFLICT (account_id, instrument_id) DO UPDATE SET
            id = EXCLUDED.id,
            side = EXCLUDED.side,
            quantity = EXCLUDED.quantity,
            avg_entry_price = EXCLUDED.avg_entry_price,
            stop_loss = EXCLUDED.stop_loss,
            mark_price = EXCLUDED.mark_price,
            realized_pnl = EXCLUDED.realized_pnl,
            updated_at = EXCLUDED.updated_at
        "#,
    )
    .bind(position.id.as_str())
    .bind(position.account_id.as_str())
    .bind(position.instrument_id.as_str())
    .bind(side_str(position.side))
    .bind(position.quantity)
    .bind(position.avg_entry_price)
    .bind(position.stop_loss)
    .bind(position.mark_price)
    .bind(position.realized_pnl)
    .bind(position.opened_at)
    .bind(position.updated_at)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn load_for_account(
    pool: &PgPool,
    account_id: &AccountId,
) -> PersistenceResult<Vec<Position>> {
    let rows = sqlx::query_as::<
        _,
        (
            String,
            String,
            String,
            String,
            Decimal,
            Decimal,
            Option<Decimal>,
            Decimal,
            Decimal,
            chrono::DateTime<chrono::Utc>,
            chrono::DateTime<chrono::Utc>,
        ),
    >(
        r#"
        SELECT id, account_id, instrument_id, side, quantity, avg_entry_price,
               stop_loss, mark_price, realized_pnl, opened_at, updated_at
        FROM positions WHERE account_id = $1
        "#,
    )
    .bind(account_id.as_str())
    .fetch_all(pool)
    .await?;

    let mut out = Vec::with_capacity(rows.len());
    for row in rows {
        out.push(Position {
            id: domain::PositionId::new(row.0)?,
            account_id: domain::AccountId::new(row.1)?,
            instrument_id: domain::InstrumentId::new(row.2)?,
            side: parse_side(&row.3)?,
            quantity: row.4,
            avg_entry_price: row.5,
            stop_loss: row.6,
            mark_price: row.7,
            realized_pnl: row.8,
            opened_at: row.9,
            updated_at: row.10,
        });
    }
    Ok(out)
}

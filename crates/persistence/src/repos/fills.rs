use domain::{Fill, OrderId};
use rust_decimal::Decimal;
use sqlx::PgPool;

use crate::error::PersistenceResult;

pub async fn insert(pool: &PgPool, fill: &Fill) -> PersistenceResult<()> {
    sqlx::query(
        r#"
        INSERT INTO fills (id, order_id, instrument_id, price, quantity, fee, filled_at)
        VALUES ($1,$2,$3,$4,$5,$6,$7)
        ON CONFLICT (id) DO NOTHING
        "#,
    )
    .bind(fill.id.as_str())
    .bind(fill.order_id.as_str())
    .bind(fill.instrument_id.as_str())
    .bind(fill.price)
    .bind(fill.quantity)
    .bind(fill.fee)
    .bind(fill.timestamp)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn load_for_order(pool: &PgPool, order_id: &OrderId) -> PersistenceResult<Vec<Fill>> {
    let rows = sqlx::query_as::<
        _,
        (
            String,
            String,
            String,
            Decimal,
            Decimal,
            Decimal,
            chrono::DateTime<chrono::Utc>,
        ),
    >(
        r#"
        SELECT id, order_id, instrument_id, price, quantity, fee, filled_at
        FROM fills WHERE order_id = $1 ORDER BY filled_at ASC
        "#,
    )
    .bind(order_id.as_str())
    .fetch_all(pool)
    .await?;

    let mut fills = Vec::with_capacity(rows.len());
    for row in rows {
        fills.push(Fill {
            id: domain::FillId::new(row.0)?,
            order_id: domain::OrderId::new(row.1)?,
            instrument_id: domain::InstrumentId::new(row.2)?,
            price: row.3,
            quantity: row.4,
            fee: row.5,
            timestamp: row.6,
        });
    }
    Ok(fills)
}

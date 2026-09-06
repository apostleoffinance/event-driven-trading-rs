use domain::{OrderSide, TradeIntent, TradeIntentId};
use rust_decimal::Decimal;
use sqlx::PgPool;

use crate::error::{PersistenceError, PersistenceResult};

fn side_str(side: OrderSide) -> &'static str {
    match side {
        OrderSide::Buy => "buy",
        OrderSide::Sell => "sell",
    }
}

fn parse_side(s: &str) -> PersistenceResult<OrderSide> {
    match s {
        "buy" => Ok(OrderSide::Buy),
        "sell" => Ok(OrderSide::Sell),
        other => Err(PersistenceError::Other(format!("unknown side: {other}"))),
    }
}

pub async fn insert(pool: &PgPool, intent: &TradeIntent) -> PersistenceResult<()> {
    sqlx::query(
        r#"
        INSERT INTO trade_intents (
            id, strategy_id, strategy_version, deployment_id, instrument_id, side,
            target_quantity, target_notional, entry_price, stop_loss, take_profit,
            confidence, created_at
        ) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13)
        ON CONFLICT (id) DO NOTHING
        "#,
    )
    .bind(intent.id.as_str())
    .bind(intent.strategy_id.as_str())
    .bind(intent.strategy_version.as_str())
    .bind(intent.deployment_id.as_str())
    .bind(intent.instrument_id.as_str())
    .bind(side_str(intent.side))
    .bind(intent.target_quantity)
    .bind(intent.target_notional)
    .bind(intent.entry_price)
    .bind(intent.stop_loss)
    .bind(intent.take_profit)
    .bind(intent.confidence)
    .bind(intent.timestamp)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn load(pool: &PgPool, id: &TradeIntentId) -> PersistenceResult<TradeIntent> {
    let row = sqlx::query_as::<
        _,
        (
            String,
            String,
            String,
            String,
            String,
            String,
            Option<Decimal>,
            Option<Decimal>,
            Option<Decimal>,
            Option<Decimal>,
            Option<Decimal>,
            Option<Decimal>,
            chrono::DateTime<chrono::Utc>,
        ),
    >(
        r#"
        SELECT id, strategy_id, strategy_version, deployment_id, instrument_id, side,
               target_quantity, target_notional, entry_price, stop_loss, take_profit,
               confidence, created_at
        FROM trade_intents WHERE id = $1
        "#,
    )
    .bind(id.as_str())
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| PersistenceError::NotFound(id.as_str().to_string()))?;

    Ok(TradeIntent {
        id: domain::TradeIntentId::new(row.0)?,
        strategy_id: domain::StrategyId::new(row.1)?,
        strategy_version: domain::StrategyVersion::new(row.2)?,
        deployment_id: domain::DeploymentId::new(row.3)?,
        instrument_id: domain::InstrumentId::new(row.4)?,
        side: parse_side(&row.5)?,
        target_quantity: row.6,
        target_notional: row.7,
        entry_price: row.8,
        stop_loss: row.9,
        take_profit: row.10,
        confidence: row.11,
        timestamp: row.12,
    })
}

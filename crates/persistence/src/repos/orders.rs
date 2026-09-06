use domain::{ClientOrderId, Order, OrderId, OrderSide, OrderStatus, OrderType, TimeInForce};
use rust_decimal::Decimal;
use sqlx::{FromRow, PgPool};

use crate::error::{PersistenceError, PersistenceResult};

#[derive(Debug, FromRow)]
struct OrderRow {
    id: String,
    client_order_id: String,
    account_id: String,
    venue_id: String,
    deployment_id: String,
    strategy_id: String,
    trade_intent_id: Option<String>,
    instrument_id: String,
    side: String,
    order_type: String,
    time_in_force: String,
    quantity: Decimal,
    price: Option<Decimal>,
    filled_quantity: Decimal,
    status: String,
    created_at: chrono::DateTime<chrono::Utc>,
    updated_at: chrono::DateTime<chrono::Utc>,
    submitted_at: Option<chrono::DateTime<chrono::Utc>>,
    filled_at: Option<chrono::DateTime<chrono::Utc>>,
}

fn side_str(s: OrderSide) -> &'static str {
    match s {
        OrderSide::Buy => "buy",
        OrderSide::Sell => "sell",
    }
}
fn order_type_str(t: OrderType) -> &'static str {
    match t {
        OrderType::Market => "market",
        OrderType::Limit => "limit",
    }
}
fn tif_str(t: TimeInForce) -> &'static str {
    match t {
        TimeInForce::Day => "day",
        TimeInForce::Gtc => "gtc",
        TimeInForce::Ioc => "ioc",
        TimeInForce::Fok => "fok",
    }
}

fn parse_side(s: &str) -> PersistenceResult<OrderSide> {
    match s {
        "buy" => Ok(OrderSide::Buy),
        "sell" => Ok(OrderSide::Sell),
        o => Err(PersistenceError::Other(format!("unknown side {o}"))),
    }
}
fn parse_order_type(s: &str) -> PersistenceResult<OrderType> {
    match s {
        "market" => Ok(OrderType::Market),
        "limit" => Ok(OrderType::Limit),
        o => Err(PersistenceError::Other(format!("unknown order type {o}"))),
    }
}
fn parse_tif(s: &str) -> PersistenceResult<TimeInForce> {
    match s {
        "day" => Ok(TimeInForce::Day),
        "gtc" => Ok(TimeInForce::Gtc),
        "ioc" => Ok(TimeInForce::Ioc),
        "fok" => Ok(TimeInForce::Fok),
        o => Err(PersistenceError::Other(format!("unknown tif {o}"))),
    }
}
fn parse_status(s: &str) -> PersistenceResult<OrderStatus> {
    match s {
        "CREATED" => Ok(OrderStatus::Created),
        "PENDING_RISK" => Ok(OrderStatus::PendingRisk),
        "APPROVED" => Ok(OrderStatus::Approved),
        "REJECTED" => Ok(OrderStatus::Rejected),
        "SUBMITTED" => Ok(OrderStatus::Submitted),
        "ACCEPTED" => Ok(OrderStatus::Accepted),
        "PARTIALLY_FILLED" => Ok(OrderStatus::PartiallyFilled),
        "FILLED" => Ok(OrderStatus::Filled),
        "CANCEL_PENDING" => Ok(OrderStatus::CancelPending),
        "CANCELLED" => Ok(OrderStatus::Cancelled),
        "FAILED" => Ok(OrderStatus::Failed),
        "UNKNOWN" => Ok(OrderStatus::Unknown),
        o => Err(PersistenceError::Other(format!("unknown status {o}"))),
    }
}

pub async fn upsert(pool: &PgPool, order: &Order) -> PersistenceResult<()> {
    sqlx::query(
        r#"
        INSERT INTO orders (
            id, client_order_id, account_id, venue_id, deployment_id, strategy_id,
            trade_intent_id, instrument_id, side, order_type, time_in_force,
            quantity, price, filled_quantity, status, created_at, updated_at,
            submitted_at, filled_at
        ) VALUES (
            $1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16,$17,$18,$19
        )
        ON CONFLICT (id) DO UPDATE SET
            filled_quantity = EXCLUDED.filled_quantity,
            status = EXCLUDED.status,
            updated_at = EXCLUDED.updated_at,
            submitted_at = EXCLUDED.submitted_at,
            filled_at = EXCLUDED.filled_at,
            price = EXCLUDED.price
        "#,
    )
    .bind(order.id.as_str())
    .bind(order.client_order_id.as_str())
    .bind(order.account_id.as_str())
    .bind(order.venue_id.as_str())
    .bind(order.deployment_id.as_str())
    .bind(order.strategy_id.as_str())
    .bind(order.trade_intent_id.as_ref().map(|id| id.as_str()))
    .bind(order.instrument_id.as_str())
    .bind(side_str(order.side))
    .bind(order_type_str(order.order_type))
    .bind(tif_str(order.time_in_force))
    .bind(order.quantity)
    .bind(order.price)
    .bind(order.filled_quantity)
    .bind(order.status.as_str())
    .bind(order.created_at)
    .bind(order.updated_at)
    .bind(order.submitted_at)
    .bind(order.filled_at)
    .execute(pool)
    .await?;
    Ok(())
}

fn map_order(row: OrderRow) -> PersistenceResult<Order> {
    Ok(Order {
        id: OrderId::new(row.id)?,
        client_order_id: ClientOrderId::new(row.client_order_id)?,
        account_id: domain::AccountId::new(row.account_id)?,
        venue_id: domain::VenueId::new(row.venue_id)?,
        deployment_id: domain::DeploymentId::new(row.deployment_id)?,
        strategy_id: domain::StrategyId::new(row.strategy_id)?,
        trade_intent_id: match row.trade_intent_id {
            Some(id) => Some(domain::TradeIntentId::new(id)?),
            None => None,
        },
        instrument_id: domain::InstrumentId::new(row.instrument_id)?,
        side: parse_side(&row.side)?,
        order_type: parse_order_type(&row.order_type)?,
        time_in_force: parse_tif(&row.time_in_force)?,
        quantity: row.quantity,
        price: row.price,
        filled_quantity: row.filled_quantity,
        status: parse_status(&row.status)?,
        created_at: row.created_at,
        updated_at: row.updated_at,
        submitted_at: row.submitted_at,
        filled_at: row.filled_at,
    })
}

const ORDER_SELECT: &str = r#"
    SELECT id, client_order_id, account_id, venue_id, deployment_id, strategy_id,
           trade_intent_id, instrument_id, side, order_type, time_in_force,
           quantity, price, filled_quantity, status, created_at, updated_at,
           submitted_at, filled_at
    FROM orders
"#;

pub async fn load(pool: &PgPool, order_id: &OrderId) -> PersistenceResult<Order> {
    let row = sqlx::query_as::<_, OrderRow>(&format!("{ORDER_SELECT} WHERE id = $1"))
        .bind(order_id.as_str())
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| PersistenceError::NotFound(order_id.as_str().to_string()))?;
    map_order(row)
}

pub async fn load_by_client_id(
    pool: &PgPool,
    client_order_id: &ClientOrderId,
) -> PersistenceResult<Order> {
    let row = sqlx::query_as::<_, OrderRow>(&format!("{ORDER_SELECT} WHERE client_order_id = $1"))
        .bind(client_order_id.as_str())
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| PersistenceError::NotFound(client_order_id.as_str().to_string()))?;
    map_order(row)
}

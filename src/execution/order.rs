use rust_decimal::Decimal;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrderSide {
    Buy,
    Sell,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrderType {
    Market,
    Limit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimeInForce {
    Day,
    Gtc,
    Ioc,
    Fok,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrderStatus {
    New,
    PartiallyFilled,
    Filled,
    Cancelled,
    Rejected,
}

#[derive(Debug, Clone)]
pub struct Order {
    pub id: u64,
    pub symbol: String,
    pub side: OrderSide,
    pub order_type: OrderType,
    pub tif: TimeInForce,
    pub quantity: Decimal,
    pub price: Option<Decimal>,
    pub filled_quantity: Decimal,
    pub status: OrderStatus,
    pub created_at: u64,
    pub updated_at: u64,
}
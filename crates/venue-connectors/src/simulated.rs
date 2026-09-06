//! Simulated venue: paper execution behind [`VenueAdapter`].

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};

use async_trait::async_trait;
use chrono::Utc;
use domain::{
    AccountId, AccountState, Fill, FillId, Order, OrderId, OrderSide, OrderStatus, Position,
    PositionId, PositionSide, VenueId,
};
use rust_decimal::Decimal;
use tokio::sync::Mutex;

use crate::adapter::VenueAdapter;
use crate::error::{VenueError, VenueResult};
use crate::types::{CancelRequest, OrderAck, OrderRequest};

static FILL_SEQ: AtomicU64 = AtomicU64::new(1);
static POS_SEQ: AtomicU64 = AtomicU64::new(1);

fn next_fill_id() -> VenueResult<FillId> {
    let seq = FILL_SEQ.fetch_add(1, Ordering::Relaxed);
    Ok(FillId::new(format!("sv-fill-{seq}"))?)
}

fn next_position_id() -> VenueResult<PositionId> {
    let seq = POS_SEQ.fetch_add(1, Ordering::Relaxed);
    Ok(PositionId::new(format!("sv-pos-{seq}"))?)
}

#[derive(Debug)]
struct Inner {
    accounts: HashMap<AccountId, AccountState>,
    orders: HashMap<OrderId, Order>,
    positions: HashMap<(String, String), Position>,
    fills: Vec<Fill>,
}

/// Paper-trading venue. OMS talks only through [`VenueAdapter`].
#[derive(Debug)]
pub struct SimulatedVenue {
    venue_id: VenueId,
    fee_rate: Decimal,
    inner: Mutex<Inner>,
}

impl SimulatedVenue {
    pub fn new(venue_id: VenueId) -> Self {
        Self {
            venue_id,
            fee_rate: Decimal::new(5, 4),
            inner: Mutex::new(Inner {
                accounts: HashMap::new(),
                orders: HashMap::new(),
                positions: HashMap::new(),
                fills: Vec::new(),
            }),
        }
    }

    pub fn with_fee_rate(mut self, fee_rate: Decimal) -> Self {
        self.fee_rate = fee_rate;
        self
    }

    pub fn venue_id(&self) -> &VenueId {
        &self.venue_id
    }

    pub async fn register_account(&self, state: AccountState) -> VenueResult<()> {
        let mut inner = self.inner.lock().await;
        inner.accounts.insert(state.account_id.clone(), state);
        Ok(())
    }

    fn simulate_fills(&self, order: &Order) -> VenueResult<Vec<Fill>> {
        let price = order.price.ok_or_else(|| {
            VenueError::Rejected("simulated market orders require a price".to_string())
        })?;
        let now = Utc::now();
        let (first_qty, second_qty) = if order.quantity > Decimal::ONE {
            let half = (order.quantity / Decimal::from(2)).round_dp(8);
            (half, order.quantity - half)
        } else {
            (order.quantity, Decimal::ZERO)
        };

        let mut fills = Vec::new();
        for qty in [first_qty, second_qty] {
            if qty <= Decimal::ZERO {
                continue;
            }
            let fee = (price * qty * self.fee_rate).round_dp(8);
            fills.push(Fill::new(
                next_fill_id()?,
                order.id.clone(),
                order.instrument_id.clone(),
                price,
                qty,
                fee,
                now,
            )?);
        }
        Ok(fills)
    }

    fn apply_position_fill(
        positions: &mut HashMap<(String, String), Position>,
        order: &Order,
        fill: &Fill,
    ) -> VenueResult<()> {
        let key = (
            order.account_id.as_str().to_string(),
            order.instrument_id.as_str().to_string(),
        );
        let side = match order.side {
            OrderSide::Buy => PositionSide::Long,
            OrderSide::Sell => PositionSide::Short,
        };
        let now = fill.timestamp;

        if let Some(pos) = positions.get_mut(&key) {
            if pos.side == side {
                let total_qty = pos.quantity + fill.quantity;
                let notional = pos.avg_entry_price * pos.quantity + fill.price * fill.quantity;
                pos.avg_entry_price = (notional / total_qty).round_dp(8);
                pos.quantity = total_qty;
                pos.mark_price = fill.price;
                pos.updated_at = now;
            } else if fill.quantity < pos.quantity {
                let _ = pos.reduce(fill.quantity, fill.price, now)?;
            } else if fill.quantity == pos.quantity {
                let _ = pos.reduce(fill.quantity, fill.price, now)?;
                positions.remove(&key);
            } else {
                let remaining = fill.quantity - pos.quantity;
                let close_qty = pos.quantity;
                let _ = pos.reduce(close_qty, fill.price, now)?;
                positions.remove(&key);
                let opened = Position::open(
                    next_position_id()?,
                    order.account_id.clone(),
                    order.instrument_id.clone(),
                    side,
                    remaining,
                    fill.price,
                    None,
                    now,
                )?;
                positions.insert(key, opened);
            }
        } else {
            let opened = Position::open(
                next_position_id()?,
                order.account_id.clone(),
                order.instrument_id.clone(),
                side,
                fill.quantity,
                fill.price,
                None,
                now,
            )?;
            positions.insert(key, opened);
        }
        Ok(())
    }
}

#[async_trait]
impl VenueAdapter for SimulatedVenue {
    fn venue_name(&self) -> &str {
        self.venue_id.as_str()
    }

    async fn account_state(&self, account_id: &AccountId) -> VenueResult<AccountState> {
        let inner = self.inner.lock().await;
        inner
            .accounts
            .get(account_id)
            .cloned()
            .ok_or_else(|| VenueError::AccountNotFound(account_id.as_str().to_string()))
    }

    async fn submit_order(&self, request: OrderRequest) -> VenueResult<OrderAck> {
        if request.venue_id != self.venue_id {
            return Err(VenueError::Rejected(format!(
                "venue mismatch: expected {}, got {}",
                self.venue_id, request.venue_id
            )));
        }

        // Idempotent replay check
        {
            let inner = self.inner.lock().await;
            if let Some(existing) = inner
                .orders
                .values()
                .find(|o| o.client_order_id == request.order.client_order_id)
            {
                let fills: Vec<Fill> = inner
                    .fills
                    .iter()
                    .filter(|f| f.order_id == existing.id)
                    .cloned()
                    .collect();
                return Ok(OrderAck {
                    client_order_id: existing.client_order_id.clone(),
                    order_id: existing.id.clone(),
                    venue_order_id: format!("sv-{}", existing.id.as_str()),
                    accepted: true,
                    fills,
                    message: Some("idempotent venue replay".into()),
                });
            }
            if !inner.accounts.contains_key(&request.order.account_id) {
                return Err(VenueError::AccountNotFound(
                    request.order.account_id.as_str().to_string(),
                ));
            }
        }

        let fills = self.simulate_fills(&request.order)?;
        let filled_qty: Decimal = fills.iter().map(|f| f.quantity).sum();
        let now = Utc::now();

        let mut order = request.order;
        order.submitted_at = Some(now);
        order.filled_quantity = filled_qty;
        order.updated_at = now;
        order.status = if filled_qty >= order.quantity {
            order.filled_at = Some(now);
            OrderStatus::Filled
        } else if filled_qty > Decimal::ZERO {
            OrderStatus::PartiallyFilled
        } else {
            OrderStatus::Accepted
        };

        let mut inner = self.inner.lock().await;
        for fill in &fills {
            Self::apply_position_fill(&mut inner.positions, &order, fill)?;
            inner.fills.push(fill.clone());
        }

        let open_count = inner
            .positions
            .keys()
            .filter(|(a, _)| a == order.account_id.as_str())
            .count() as u32;
        let fees: Decimal = fills.iter().map(|f| f.fee).sum();
        if let Some(account) = inner.accounts.get_mut(&order.account_id) {
            account.open_position_count = open_count;
            account.balance -= fees;
            account.equity -= fees;
            account.updated_at = now;
        }

        let ack = OrderAck {
            client_order_id: order.client_order_id.clone(),
            order_id: order.id.clone(),
            venue_order_id: format!("sv-{}", order.id.as_str()),
            accepted: true,
            fills: fills.clone(),
            message: None,
        };
        inner.orders.insert(order.id.clone(), order);

        tracing::info!(
            venue = %self.venue_id,
            order_id = %ack.order_id,
            fills = ack.fills.len(),
            "simulated venue accepted order"
        );
        Ok(ack)
    }

    async fn cancel_order(&self, request: CancelRequest) -> VenueResult<()> {
        let mut inner = self.inner.lock().await;
        let order = inner
            .orders
            .get_mut(&request.order_id)
            .ok_or_else(|| VenueError::OrderNotFound(request.order_id.as_str().to_string()))?;
        if order.account_id != request.account_id {
            return Err(VenueError::Rejected("account mismatch on cancel".into()));
        }
        if order.status.is_terminal() {
            return Err(VenueError::Rejected(format!(
                "cannot cancel terminal order in state {}",
                order.status.as_str()
            )));
        }
        order.status = OrderStatus::Cancelled;
        order.updated_at = Utc::now();
        Ok(())
    }

    async fn open_orders(&self, account_id: &AccountId) -> VenueResult<Vec<Order>> {
        let inner = self.inner.lock().await;
        Ok(inner
            .orders
            .values()
            .filter(|o| &o.account_id == account_id && !o.status.is_terminal())
            .cloned()
            .collect())
    }

    async fn positions(&self, account_id: &AccountId) -> VenueResult<Vec<Position>> {
        let inner = self.inner.lock().await;
        Ok(inner
            .positions
            .iter()
            .filter(|((a, _), _)| a == account_id.as_str())
            .map(|(_, p)| p.clone())
            .collect())
    }

    async fn get_order(&self, order_id: &OrderId) -> VenueResult<Order> {
        let inner = self.inner.lock().await;
        inner
            .orders
            .get(order_id)
            .cloned()
            .ok_or_else(|| VenueError::OrderNotFound(order_id.as_str().to_string()))
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;
    use chrono::Utc;
    use domain::{
        AccountId, ClientOrderId, DeploymentId, InstrumentId, OrderSide, OrderType, StrategyId,
        TimeInForce, TradeIntentId, VenueId,
    };

    #[tokio::test]
    async fn submit_produces_fills_and_position() {
        let venue_id = VenueId::new("simulated").unwrap();
        let venue = SimulatedVenue::new(venue_id.clone());
        let account_id = AccountId::new("sim-1").unwrap();
        venue
            .register_account(
                AccountState::new(account_id.clone(), Decimal::from(100_000), Utc::now()).unwrap(),
            )
            .await
            .unwrap();

        let order = Order::create(
            OrderId::new("ord-1").unwrap(),
            ClientOrderId::new("clid-1").unwrap(),
            account_id.clone(),
            venue_id.clone(),
            DeploymentId::new("dep-1").unwrap(),
            StrategyId::new("strat").unwrap(),
            Some(TradeIntentId::new("ti-1").unwrap()),
            InstrumentId::new("BTCUSDT").unwrap(),
            OrderSide::Buy,
            OrderType::Market,
            TimeInForce::Ioc,
            Decimal::from(2),
            Some(Decimal::from(100)),
            Utc::now(),
        )
        .unwrap();

        let ack = venue
            .submit_order(OrderRequest { venue_id, order })
            .await
            .unwrap();
        assert!(ack.accepted);
        assert_eq!(ack.fills.len(), 2);
        let positions = venue.positions(&account_id).await.unwrap();
        assert_eq!(positions.len(), 1);
        assert_eq!(positions[0].quantity, Decimal::from(2));
    }
}

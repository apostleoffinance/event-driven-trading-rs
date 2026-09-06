//! First prop venue connector (`VenueType::Prop`).
//!
//! Paper mode provides a deterministic prop-style fill model for OMS / runtime
//! tests. Live mode is intentionally gated with [`VenueError::NotImplemented`]
//! until a concrete firm REST/WS API is selected.

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
    Ok(FillId::new(format!("prop-fill-{seq}"))?)
}

fn next_position_id() -> VenueResult<PositionId> {
    let seq = POS_SEQ.fetch_add(1, Ordering::Relaxed);
    Ok(PositionId::new(format!("prop-pos-{seq}"))?)
}

/// Operating mode for the prop connector.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PropVenueMode {
    /// In-process prop simulator (deterministic fills).
    Paper,
    /// Live firm API — blocked until a vendor transport is wired.
    Live { base_url: String },
}

/// Configuration for [`PropVenue`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PropVenueConfig {
    pub venue_id: VenueId,
    /// Vendor-agnostic label (e.g. `"generic-prop"`) until a firm is chosen.
    pub firm_label: String,
    pub mode: PropVenueMode,
    /// Flat commission charged per filled contract/unit.
    pub commission_per_unit: Decimal,
}

impl PropVenueConfig {
    /// Default paper prop venue (`venue_id = "prop"`).
    pub fn paper() -> VenueResult<Self> {
        Ok(Self {
            venue_id: VenueId::new("prop")?,
            firm_label: "generic-prop".into(),
            mode: PropVenueMode::Paper,
            commission_per_unit: Decimal::new(25, 2), // $0.25 / unit
        })
    }

    pub fn with_firm_label(mut self, label: impl Into<String>) -> Self {
        self.firm_label = label.into();
        self
    }

    pub fn with_commission_per_unit(mut self, commission: Decimal) -> Self {
        self.commission_per_unit = commission;
        self
    }

    /// Mark as live. Adapter methods return [`VenueError::NotImplemented`].
    pub fn live(mut self, base_url: impl Into<String>) -> Self {
        self.mode = PropVenueMode::Live {
            base_url: base_url.into(),
        };
        self
    }
}

#[derive(Debug)]
struct Inner {
    accounts: HashMap<AccountId, AccountState>,
    orders: HashMap<OrderId, Order>,
    positions: HashMap<(String, String), Position>,
    fills: Vec<Fill>,
}

/// Prop firm venue behind [`VenueAdapter`].
///
/// Distinct from [`crate::SimulatedVenue`]: single full fill, flat per-unit
/// commission, venue id `prop`, and an explicit live-mode gate.
#[derive(Debug)]
pub struct PropVenue {
    config: PropVenueConfig,
    inner: Mutex<Inner>,
}

impl PropVenue {
    pub fn new(config: PropVenueConfig) -> Self {
        Self {
            config,
            inner: Mutex::new(Inner {
                accounts: HashMap::new(),
                orders: HashMap::new(),
                positions: HashMap::new(),
                fills: Vec::new(),
            }),
        }
    }

    /// Convenience: paper prop venue with defaults.
    pub fn paper() -> VenueResult<Self> {
        Ok(Self::new(PropVenueConfig::paper()?))
    }

    pub fn config(&self) -> &PropVenueConfig {
        &self.config
    }

    pub fn venue_id(&self) -> &VenueId {
        &self.config.venue_id
    }

    pub fn is_paper(&self) -> bool {
        matches!(self.config.mode, PropVenueMode::Paper)
    }

    pub async fn register_account(&self, state: AccountState) -> VenueResult<()> {
        self.ensure_paper()?;
        let mut inner = self.inner.lock().await;
        inner.accounts.insert(state.account_id.clone(), state);
        Ok(())
    }

    fn ensure_paper(&self) -> VenueResult<()> {
        match &self.config.mode {
            PropVenueMode::Paper => Ok(()),
            PropVenueMode::Live { base_url } => Err(VenueError::NotImplemented(format!(
                "live prop connector not wired for firm={} base_url={base_url}; use PropVenue::paper()",
                self.config.firm_label
            ))),
        }
    }

    fn simulate_fill(&self, order: &Order) -> VenueResult<Fill> {
        let price = order.price.ok_or_else(|| {
            VenueError::Rejected("prop market orders require a price".to_string())
        })?;
        let fee = (self.config.commission_per_unit * order.quantity).round_dp(8);
        Fill::new(
            next_fill_id()?,
            order.id.clone(),
            order.instrument_id.clone(),
            price,
            order.quantity,
            fee,
            Utc::now(),
        )
        .map_err(VenueError::from)
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
impl VenueAdapter for PropVenue {
    fn venue_name(&self) -> &str {
        self.config.venue_id.as_str()
    }

    async fn account_state(&self, account_id: &AccountId) -> VenueResult<AccountState> {
        self.ensure_paper()?;
        let inner = self.inner.lock().await;
        inner
            .accounts
            .get(account_id)
            .cloned()
            .ok_or_else(|| VenueError::AccountNotFound(account_id.as_str().to_string()))
    }

    async fn submit_order(&self, request: OrderRequest) -> VenueResult<OrderAck> {
        self.ensure_paper()?;
        if request.venue_id != self.config.venue_id {
            return Err(VenueError::Rejected(format!(
                "venue mismatch: expected {}, got {}",
                self.config.venue_id, request.venue_id
            )));
        }

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
                    venue_order_id: format!("prop-{}", existing.id.as_str()),
                    accepted: true,
                    fills,
                    message: Some("idempotent prop venue replay".into()),
                });
            }
            if !inner.accounts.contains_key(&request.order.account_id) {
                return Err(VenueError::AccountNotFound(
                    request.order.account_id.as_str().to_string(),
                ));
            }
        }

        let fill = self.simulate_fill(&request.order)?;
        let now = Utc::now();
        let mut order = request.order;
        order.submitted_at = Some(now);
        order.filled_quantity = fill.quantity;
        order.filled_at = Some(now);
        order.updated_at = now;
        order.status = OrderStatus::Filled;

        let mut inner = self.inner.lock().await;
        Self::apply_position_fill(&mut inner.positions, &order, &fill)?;
        inner.fills.push(fill.clone());

        let open_count = inner
            .positions
            .keys()
            .filter(|(a, _)| a == order.account_id.as_str())
            .count() as u32;
        if let Some(account) = inner.accounts.get_mut(&order.account_id) {
            account.open_position_count = open_count;
            account.balance -= fill.fee;
            account.equity -= fill.fee;
            account.updated_at = now;
        }

        let ack = OrderAck {
            client_order_id: order.client_order_id.clone(),
            order_id: order.id.clone(),
            venue_order_id: format!("prop-{}", order.id.as_str()),
            accepted: true,
            fills: vec![fill],
            message: Some(format!("prop paper fill firm={}", self.config.firm_label)),
        };
        inner.orders.insert(order.id.clone(), order);

        tracing::info!(
            venue = %self.config.venue_id,
            firm = %self.config.firm_label,
            order_id = %ack.order_id,
            "prop venue accepted order"
        );
        Ok(ack)
    }

    async fn cancel_order(&self, request: CancelRequest) -> VenueResult<()> {
        self.ensure_paper()?;
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
        self.ensure_paper()?;
        let inner = self.inner.lock().await;
        Ok(inner
            .orders
            .values()
            .filter(|o| &o.account_id == account_id && !o.status.is_terminal())
            .cloned()
            .collect())
    }

    async fn positions(&self, account_id: &AccountId) -> VenueResult<Vec<Position>> {
        self.ensure_paper()?;
        let inner = self.inner.lock().await;
        Ok(inner
            .positions
            .iter()
            .filter(|((a, _), _)| a == account_id.as_str())
            .map(|(_, p)| p.clone())
            .collect())
    }

    async fn get_order(&self, order_id: &OrderId) -> VenueResult<Order> {
        self.ensure_paper()?;
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
    use domain::{
        AccountId, ClientOrderId, DeploymentId, InstrumentId, OrderSide, OrderType, StrategyId,
        TimeInForce, TradeIntentId,
    };

    #[tokio::test]
    async fn paper_submit_single_fill() {
        let venue = PropVenue::paper().unwrap();
        let account_id = AccountId::new("prop-1").unwrap();
        venue
            .register_account(
                AccountState::new(account_id.clone(), Decimal::from(50_000), Utc::now()).unwrap(),
            )
            .await
            .unwrap();

        let order = Order::create(
            OrderId::new("ord-prop-1").unwrap(),
            ClientOrderId::new("clid-prop-1").unwrap(),
            account_id.clone(),
            venue.venue_id().clone(),
            DeploymentId::new("dep-1").unwrap(),
            StrategyId::new("strat").unwrap(),
            Some(TradeIntentId::new("ti-1").unwrap()),
            InstrumentId::new("ES").unwrap(),
            OrderSide::Buy,
            OrderType::Market,
            TimeInForce::Ioc,
            Decimal::from(2),
            Some(Decimal::from(5000)),
            Utc::now(),
        )
        .unwrap();

        let ack = venue
            .submit_order(OrderRequest {
                venue_id: venue.venue_id().clone(),
                order,
            })
            .await
            .unwrap();
        assert!(ack.accepted);
        assert_eq!(ack.fills.len(), 1);
        assert_eq!(ack.fills[0].quantity, Decimal::from(2));
        // $0.25 * 2
        assert_eq!(ack.fills[0].fee, Decimal::new(50, 2));
        assert!(ack.venue_order_id.starts_with("prop-"));
        let positions = venue.positions(&account_id).await.unwrap();
        assert_eq!(positions.len(), 1);
    }

    #[tokio::test]
    async fn live_mode_is_gated() {
        let config = PropVenueConfig::paper()
            .unwrap()
            .live("https://prop-firm.example/api");
        let venue = PropVenue::new(config);
        let err = venue
            .account_state(&AccountId::new("x").unwrap())
            .await
            .unwrap_err();
        assert!(matches!(err, VenueError::NotImplemented(_)));
    }
}

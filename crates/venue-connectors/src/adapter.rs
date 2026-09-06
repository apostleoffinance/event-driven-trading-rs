//! Venue adapter trait — execution boundary to prop / CEX / simulated venues.

use async_trait::async_trait;
use domain::{AccountId, AccountState, Order, OrderId, Position};

use crate::error::VenueResult;
use crate::types::{CancelRequest, OrderAck, OrderRequest};

#[async_trait]
pub trait VenueAdapter: Send + Sync {
    fn venue_name(&self) -> &str;

    async fn account_state(&self, account_id: &AccountId) -> VenueResult<AccountState>;

    async fn submit_order(&self, request: OrderRequest) -> VenueResult<OrderAck>;

    async fn cancel_order(&self, request: CancelRequest) -> VenueResult<()>;

    async fn open_orders(&self, account_id: &AccountId) -> VenueResult<Vec<Order>>;

    async fn positions(&self, account_id: &AccountId) -> VenueResult<Vec<Position>>;

    async fn get_order(&self, order_id: &OrderId) -> VenueResult<Order>;
}

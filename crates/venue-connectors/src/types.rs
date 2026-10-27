//! Venue request/response types.

use domain::{AccountId, ClientOrderId, Fill, Order, OrderId, VenueId};
use serde::{Deserialize, Serialize};

/// Submission payload sent to a venue.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OrderRequest {
    pub venue_id: VenueId,
    pub order: Order,
}

/// Acknowledgement from a venue after submit.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OrderAck {
    pub client_order_id: ClientOrderId,
    pub order_id: OrderId,
    pub venue_order_id: String,
    pub accepted: bool,
    /// Immediate fills (e.g. simulated market orders).
    pub fills: Vec<Fill>,
    pub message: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CancelRequest {
    pub account_id: AccountId,
    pub order_id: OrderId,
    pub client_order_id: ClientOrderId,
}

/// Compare immutable submit fields for venue-side client-ID idempotency.
/// Lifecycle fields (status, timestamps, and filled quantity) are intentionally ignored.
pub fn same_submission_payload(existing: &Order, incoming: &Order) -> bool {
    existing.client_order_id == incoming.client_order_id
        && existing.account_id == incoming.account_id
        && existing.venue_id == incoming.venue_id
        && existing.deployment_id == incoming.deployment_id
        && existing.strategy_id == incoming.strategy_id
        && existing.trade_intent_id == incoming.trade_intent_id
        && existing.instrument_id == incoming.instrument_id
        && existing.side == incoming.side
        && existing.order_type == incoming.order_type
        && existing.time_in_force == incoming.time_in_force
        && existing.quantity == incoming.quantity
        && existing.price == incoming.price
}

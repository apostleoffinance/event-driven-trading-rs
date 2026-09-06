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

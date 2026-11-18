//! Read-only venue data interface and explicit simulation execution capability.

use async_trait::async_trait;
use domain::{AccountId, AccountState, Order, OrderId, Position};

use crate::error::VenueResult;
use crate::types::{CancelRequest, OrderAck, OrderRequest};

pub(crate) mod private {
    pub trait Sealed {}
}

/// General public interface for venue metadata and observed account state.
/// It deliberately has no order-submit or order-cancel operation.
///
/// ```compile_fail
/// use venue_connectors::{types::OrderRequest, VenueAdapter};
/// fn ordinary_caller_cannot_submit(venue: &dyn VenueAdapter, request: OrderRequest) {
///     let _submission = venue.submit_order(request);
/// }
/// ```
///
/// ```compile_fail
/// use execution_engine::{ExecutionEngine, NewOrderRequest};
/// use venue_connectors::{execute_simulated_order, VenueAdapter};
/// fn general_venue_cannot_reach_execution(
///     oms: &mut ExecutionEngine,
///     venue: &dyn VenueAdapter,
///     request: NewOrderRequest,
/// ) {
///     let _execution = execute_simulated_order(oms, venue, request);
/// }
/// ```
///
/// ```compile_fail
/// use venue_connectors::adapter::private::Sealed;
/// ```
#[allow(clippy::double_must_use)]
#[async_trait]
pub trait VenueAdapter: Send + Sync {
    fn venue_name(&self) -> &str;

    async fn account_state(&self, account_id: &AccountId) -> VenueResult<AccountState>;

    async fn open_orders(&self, account_id: &AccountId) -> VenueResult<Vec<Order>>;

    async fn positions(&self, account_id: &AccountId) -> VenueResult<Vec<Position>>;

    async fn get_order(&self, order_id: &OrderId) -> VenueResult<Order>;
}

/// Execution capability for in-process simulation and paper-fill adapters only.
///
/// This trait is deliberately separate from [`VenueAdapter`]. It must never be
/// implemented by a broker-backed adapter. Future broker submission belongs
/// behind the private execution authorization coordinator capability.
#[allow(clippy::double_must_use)]
#[allow(private_bounds)]
#[async_trait]
pub trait SimulationExecutionAdapter: VenueAdapter + private::Sealed {
    async fn submit_simulated_order(&self, request: OrderRequest) -> VenueResult<OrderAck>;

    async fn cancel_simulated_order(&self, request: CancelRequest) -> VenueResult<()>;
}

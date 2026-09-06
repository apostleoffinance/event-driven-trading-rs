//! Bridge OMS ↔ VenueAdapter without coupling OMS to a concrete venue.

use chrono::Utc;
use execution_engine::{CreateOrderOutcome, ExecutionEngine, ExecutionError, NewOrderRequest};

use crate::adapter::VenueAdapter;
use crate::error::{VenueError, VenueResult};
use crate::types::{CancelRequest, OrderRequest};

/// Submit a risk-approved order through OMS then the venue adapter.
///
/// Flow: create (idempotent) → mark submitted → venue.submit → apply fills → accept/fill states.
pub async fn submit_approved_order(
    oms: &mut ExecutionEngine,
    venue: &dyn VenueAdapter,
    request: NewOrderRequest,
) -> VenueResult<CreateOrderOutcome> {
    let at = request.created_at;
    let outcome = oms.create_order(request).map_err(map_exec)?;

    if !outcome.created {
        return Ok(outcome);
    }

    let order_id = outcome.order.id.clone();
    oms.mark_submitted(&order_id, at).map_err(map_exec)?;

    let order = oms.get(&order_id).map_err(map_exec)?.clone();
    let ack = venue
        .submit_order(OrderRequest {
            venue_id: order.venue_id.clone(),
            order,
        })
        .await?;

    if !ack.accepted {
        let reason = ack
            .message
            .unwrap_or_else(|| "venue rejected order".to_string());
        oms.mark_rejected(&order_id, reason, Utc::now())
            .map_err(map_exec)?;
        return Err(VenueError::Rejected("venue did not accept order".into()));
    }

    oms.mark_accepted(&order_id, Utc::now()).map_err(map_exec)?;

    for fill in ack.fills {
        oms.apply_fill(
            &order_id,
            fill.price,
            fill.quantity,
            fill.fee,
            fill.timestamp,
        )
        .map_err(map_exec)?;
    }

    let order = oms.get(&order_id).map_err(map_exec)?.clone();
    Ok(CreateOrderOutcome {
        order,
        created: true,
    })
}

/// Request cancel at OMS then at the venue.
pub async fn cancel_order(
    oms: &mut ExecutionEngine,
    venue: &dyn VenueAdapter,
    order_id: &domain::OrderId,
) -> VenueResult<()> {
    let at = Utc::now();
    let order = oms.request_cancel(order_id, at).map_err(map_exec)?;
    venue
        .cancel_order(CancelRequest {
            account_id: order.account_id.clone(),
            order_id: order.id.clone(),
            client_order_id: order.client_order_id.clone(),
        })
        .await?;
    oms.mark_cancelled(order_id, at).map_err(map_exec)?;
    Ok(())
}

fn map_exec(err: ExecutionError) -> VenueError {
    VenueError::Failed(err.to_string())
}

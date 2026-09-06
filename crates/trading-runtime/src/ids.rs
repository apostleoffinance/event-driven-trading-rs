//! Client order ID allocation derived from trade intents.

use domain::{ClientOrderId, TradeIntentId};

use crate::error::{RuntimeError, RuntimeResult};

/// Stable client order id for a trade intent (retry-safe / idempotent).
pub fn client_order_id_for_intent(intent_id: &TradeIntentId) -> RuntimeResult<ClientOrderId> {
    ClientOrderId::new(format!("clid-{}", intent_id.as_str())).map_err(RuntimeError::from)
}

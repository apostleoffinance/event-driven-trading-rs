//! Trade intent ID allocation for strategy outputs.

use std::sync::atomic::{AtomicU64, Ordering};

use domain::TradeIntentId;

use crate::error::{StrategyError, StrategyResult};

static INTENT_SEQ: AtomicU64 = AtomicU64::new(1);

/// Allocate a unique in-process [`TradeIntentId`].
pub fn next_trade_intent_id() -> StrategyResult<TradeIntentId> {
    let seq = INTENT_SEQ.fetch_add(1, Ordering::Relaxed);
    let millis = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|e| StrategyError::Invariant(format!("clock error: {e}")))?
        .as_millis();
    TradeIntentId::new(format!("ti-{millis}-{seq}")).map_err(StrategyError::from)
}

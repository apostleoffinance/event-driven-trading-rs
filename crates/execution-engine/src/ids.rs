//! In-process ID allocation for orders and fills.

use std::sync::atomic::{AtomicU64, Ordering};

use domain::{FillId, OrderId};

use crate::error::{ExecutionError, ExecutionResult};

static ORDER_SEQ: AtomicU64 = AtomicU64::new(1);
static FILL_SEQ: AtomicU64 = AtomicU64::new(1);

pub fn next_order_id() -> ExecutionResult<OrderId> {
    let seq = ORDER_SEQ.fetch_add(1, Ordering::Relaxed);
    let millis = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|e| ExecutionError::Failed(format!("clock error: {e}")))?
        .as_millis();
    OrderId::new(format!("ord-{millis}-{seq}")).map_err(ExecutionError::from)
}

pub fn next_fill_id() -> ExecutionResult<FillId> {
    let seq = FILL_SEQ.fetch_add(1, Ordering::Relaxed);
    let millis = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|e| ExecutionError::Failed(format!("clock error: {e}")))?
        .as_millis();
    FillId::new(format!("fill-{millis}-{seq}")).map_err(ExecutionError::from)
}

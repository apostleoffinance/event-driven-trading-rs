//! Audit trail for OMS decisions and lifecycle transitions.

use chrono::{DateTime, Utc};
use domain::{ClientOrderId, OrderId};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AuditKind {
    OrderCreated,
    OrderIdempotentReplay,
    PendingRisk,
    RiskApproved,
    RiskRejected,
    Submitted,
    Accepted,
    Rejected,
    PartiallyFilled,
    Filled,
    CancelPending,
    Cancelled,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuditEvent {
    pub at: DateTime<Utc>,
    pub order_id: OrderId,
    pub client_order_id: ClientOrderId,
    pub kind: AuditKind,
    pub detail: String,
}

impl AuditEvent {
    pub fn new(
        at: DateTime<Utc>,
        order_id: OrderId,
        client_order_id: ClientOrderId,
        kind: AuditKind,
        detail: impl Into<String>,
    ) -> Self {
        Self {
            at,
            order_id,
            client_order_id,
            kind,
            detail: detail.into(),
        }
    }
}

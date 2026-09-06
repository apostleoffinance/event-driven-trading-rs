//! Execution / OMS engine.
//!
//! Owns order lifecycle, client-order idempotency, fills, and audit trail.
//! Does not run strategies or evaluate risk rules — requires a prior
//! executable [`RiskDecision`] before creating an order.

#![forbid(unsafe_code)]
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

pub mod audit;
pub mod error;
pub mod ids;
pub mod oms;
pub mod request;

pub use audit::{AuditEvent, AuditKind};
pub use error::{ExecutionError, ExecutionResult};
pub use oms::{CreateOrderOutcome, ExecutionEngine};
pub use request::NewOrderRequest;

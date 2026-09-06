//! Execution-engine errors.

use thiserror::Error;

pub type ExecutionResult<T> = Result<T, ExecutionError>;

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum ExecutionError {
    #[error("order not found: {0}")]
    OrderNotFound(String),

    #[error("missing account id")]
    MissingAccount,

    #[error("missing venue id")]
    MissingVenue,

    #[error("missing client order id")]
    MissingClientOrderId,

    #[error("order requires an executable risk decision before creation")]
    MissingRiskDecision,

    #[error("risk decision is not executable: {0}")]
    RiskNotExecutable(String),

    #[error("idempotency conflict for client_order_id {client_order_id}: {reason}")]
    IdempotencyConflict {
        client_order_id: String,
        reason: String,
    },

    #[error("illegal order state: {0}")]
    IllegalState(String),

    #[error("execution failure: {0}")]
    Failed(String),

    #[error(transparent)]
    Domain(#[from] domain::DomainError),
}

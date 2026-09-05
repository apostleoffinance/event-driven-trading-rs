//! Domain-layer errors for quantitative trading primitives.
//!
//! Subsystem crates should convert these at their boundaries rather than
//! reusing a single global application error type.

use thiserror::Error;

/// Fallible domain operations return this result alias.
pub type DomainResult<T> = Result<T, DomainError>;

/// Errors arising from invalid domain state or illegal transitions.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum DomainError {
    #[error("invalid identifier `{id}`: {reason}")]
    InvalidId { id: String, reason: String },

    #[error("invalid money/quantity value: {0}")]
    InvalidMoney(String),

    #[error("validation failed: {0}")]
    Validation(String),

    #[error("illegal order transition from {from:?} to {to:?}")]
    IllegalOrderTransition { from: String, to: String },

    #[error("invariant violated: {0}")]
    Invariant(String),
}

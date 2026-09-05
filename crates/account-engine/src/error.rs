//! Account-engine errors.

use thiserror::Error;

pub type AccountEngineResult<T> = Result<T, AccountEngineError>;

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum AccountEngineError {
    #[error("account not found: {0}")]
    NotFound(String),

    #[error("account already exists: {0}")]
    AlreadyExists(String),

    #[error("invalid account status transition: {0}")]
    InvalidStatus(String),

    #[error("account operation not allowed: {0}")]
    NotAllowed(String),

    #[error(transparent)]
    Domain(#[from] domain::DomainError),
}

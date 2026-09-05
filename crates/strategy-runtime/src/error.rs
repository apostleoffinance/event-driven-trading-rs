//! Strategy-runtime errors.

use thiserror::Error;

pub type StrategyResult<T> = Result<T, StrategyError>;

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum StrategyError {
    #[error("strategy validation failed: {0}")]
    Validation(String),

    #[error("strategy invariant violated: {0}")]
    Invariant(String),

    #[error(transparent)]
    Domain(#[from] domain::DomainError),

    #[error(transparent)]
    Events(#[from] events::EventsError),
}

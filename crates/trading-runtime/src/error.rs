//! Trading-runtime errors.

use thiserror::Error;

/// Errors raised while orchestrating the continuous trading pipeline.
#[derive(Debug, Error)]
pub enum RuntimeError {
    #[error("domain error: {0}")]
    Domain(#[from] domain::DomainError),

    #[error("events error: {0}")]
    Events(#[from] events::EventsError),

    #[error("strategy error: {0}")]
    Strategy(#[from] strategy_runtime::StrategyError),

    #[error("risk error: {0}")]
    Risk(#[from] risk_engine::RiskEngineError),

    #[error("account error: {0}")]
    Account(#[from] account_engine::AccountEngineError),

    #[error("execution error: {0}")]
    Execution(#[from] execution_engine::ExecutionError),

    #[error("venue error: {0}")]
    Venue(#[from] venue_connectors::VenueError),

    #[error("persistence error: {0}")]
    Persistence(#[from] persistence::PersistenceError),

    #[error("runtime invariant: {0}")]
    Invariant(String),
}

pub type RuntimeResult<T> = Result<T, RuntimeError>;

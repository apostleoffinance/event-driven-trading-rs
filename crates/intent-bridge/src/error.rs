//! Intent-bridge errors.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum BridgeError {
    #[error("domain error: {0}")]
    Domain(#[from] domain::DomainError),

    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("invalid intent payload: {0}")]
    Invalid(String),

    #[error("runtime error: {0}")]
    Runtime(#[from] trading_runtime::RuntimeError),

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("events error: {0}")]
    Events(#[from] events::EventsError),

    #[error("venue error: {0}")]
    Venue(#[from] venue_connectors::VenueError),
}

pub type BridgeResult<T> = Result<T, BridgeError>;

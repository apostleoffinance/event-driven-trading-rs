//! Reconciliation errors.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum ReconciliationError {
    #[error("domain error: {0}")]
    Domain(#[from] domain::DomainError),

    #[error("events error: {0}")]
    Events(#[from] events::EventsError),

    #[error("venue error: {0}")]
    Venue(#[from] venue_connectors::VenueError),

    #[error("reconciliation failed: {0}")]
    Failed(String),
}

pub type ReconciliationResult<T> = Result<T, ReconciliationError>;

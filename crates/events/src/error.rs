//! Errors for the events crate.

use thiserror::Error;

pub type EventsResult<T> = Result<T, EventsError>;

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum EventsError {
    #[error("event channel closed")]
    ChannelClosed,

    #[error("failed to publish event: {0}")]
    Publish(String),

    #[error("invalid event: {0}")]
    Invalid(String),

    #[error(transparent)]
    Domain(#[from] domain::DomainError),
}

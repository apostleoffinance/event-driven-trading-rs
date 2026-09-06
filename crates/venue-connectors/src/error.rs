//! Venue-connector errors.

use thiserror::Error;

pub type VenueResult<T> = Result<T, VenueError>;

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum VenueError {
    #[error("venue rejected order: {0}")]
    Rejected(String),

    #[error("order not found at venue: {0}")]
    OrderNotFound(String),

    #[error("account not found at venue: {0}")]
    AccountNotFound(String),

    #[error("venue failure: {0}")]
    Failed(String),

    #[error(transparent)]
    Domain(#[from] domain::DomainError),
}

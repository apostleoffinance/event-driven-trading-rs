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

    /// Timeout / disconnect after submit — outcome unknown; do not assume Failed.
    #[error("ambiguous venue outcome: {0}")]
    Ambiguous(String),

    #[error("not implemented: {0}")]
    NotImplemented(String),

    #[error(transparent)]
    Domain(#[from] domain::DomainError),
}

impl VenueError {
    pub fn is_ambiguous(&self) -> bool {
        matches!(self, Self::Ambiguous(_))
    }
}

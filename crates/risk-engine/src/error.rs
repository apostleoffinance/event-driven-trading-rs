//! Risk-engine errors.

use thiserror::Error;

pub type RiskEngineResult<T> = Result<T, RiskEngineError>;

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum RiskEngineError {
    #[error("risk evaluation failed: {0}")]
    Evaluation(String),

    #[error(transparent)]
    Domain(#[from] domain::DomainError),
}

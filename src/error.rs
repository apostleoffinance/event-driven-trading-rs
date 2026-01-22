use thiserror::Error;

#[derive(Debug, Error)]
#[allow(dead_code)]
pub enum TradingError {
    #[error("Market data error: {0}")]
    MarketData(String),

    #[error("Network error: {0}")]
    Network(#[from] reqwest::Error),

    #[error("Validation error: {0}")]
    Validation(String),

    #[error("Execution error: {0}")]
    Execution(String),

    #[error("Invalid decimal: {0}")]
    Decimal(#[from] rust_decimal::Error),

    #[error("Time error: {0}")]
    Time(String),
}

pub type Result<T> = std::result::Result<T, TradingError>;

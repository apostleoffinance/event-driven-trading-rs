use thiserror::Error;

/// Centralized error type for the trading engine
/// Uses thiserror for automatic Display and Error trait implementations
#[derive(Debug, Error)]
pub enum TradingError {
    #[error("Market data error: {0}")]
    MarketData(String),

    #[error("Network error: {0}")]
    Network(#[from] reqwest::Error),

    #[error("Validation error: {0}")]
    Validation(String),

    #[error("Execution error: {0}")]
    Execution(String),

    #[error("Decimal conversion error: {0}")]
    Decimal(#[from] rust_decimal::Error),

    #[error("Decimal parse error: {0}")]
    DecimalParse(String),

    #[error("Time error: {0}")]
    Time(String),

    #[error("Configuration error: {0}")]
    Config(String),

    #[error("Strategy error: {0}")]
    Strategy(String),

    #[error("Risk management error: {0}")]
    Risk(String),

    #[error("Event bus error: {0}")]
    EventBus(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Environment variable error: {0}")]
    Env(#[from] std::env::VarError),
}

/// Result type alias for convenience
pub type Result<T> = std::result::Result<T, TradingError>;

/// Convert from serde_json errors
impl From<serde_json::Error> for TradingError {
    fn from(err: serde_json::Error) -> Self {
        TradingError::MarketData(format!("JSON parse error: {}", err))
    }
}

/// Convert from SystemTimeError
impl From<std::time::SystemTimeError> for TradingError {
    fn from(err: std::time::SystemTimeError) -> Self {
        TradingError::Time(err.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display() {
        let error = TradingError::MarketData("API timeout".to_string());
        assert_eq!(error.to_string(), "Market data error: API timeout");
    }

    #[test]
    fn test_validation_error() {
        let error = TradingError::Validation("Invalid price".to_string());
        assert_eq!(error.to_string(), "Validation error: Invalid price");
    }

    #[test]
    fn test_config_error() {
        let error = TradingError::Config("Missing API key".to_string());
        assert_eq!(error.to_string(), "Configuration error: Missing API key");
    }

    #[test]
    fn test_risk_error() {
        let error = TradingError::Risk("Position too large".to_string());
        assert_eq!(error.to_string(), "Risk management error: Position too large");
    }

    #[test]
    fn test_error_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<TradingError>();
    }
}

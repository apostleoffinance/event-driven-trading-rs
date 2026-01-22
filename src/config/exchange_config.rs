use serde::{Deserialize, Serialize};
use crate::error::Result;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExchangeConfig {
    pub exchange_type: ExchangeType,
    pub api_key: Option<String>,
    pub api_secret: Option<String>,
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ExchangeType {
    /// Binance - Largest crypto exchange
    /// Great for: Learning, paper trading, most altcoins
    Binance,
    /// Bybit - Derivatives and spot trading
    /// Great for: Leveraged trading, futures
    Bybit,
}

impl ExchangeType {
    pub fn description(&self) -> &str {
        match self {
            ExchangeType::Binance => "Binance - Crypto spot trading",
            ExchangeType::Bybit => "Bybit - Crypto derivatives & spot",

        }
    }
}

impl ExchangeConfig {
    pub fn validate(&self) -> Result<()> {
        // For live trading, require API credentials
        // For paper trading, they're optional
        Ok(())
    }
}

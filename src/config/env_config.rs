use crate::error::Result;

pub struct EnvConfig;

impl EnvConfig {
    pub fn load() -> Result<()> {
        dotenv::dotenv().ok();
        Ok(())
    }

    pub fn get_binance_api_key() -> Option<String> {
        std::env::var("BINANCE_API_KEY").ok()
    }

    pub fn get_binance_secret_key() -> Option<String> {
        std::env::var("BINANCE_SECRET_KEY").ok()
    }

    pub fn get_bybit_api_key() -> Option<String> {
        std::env::var("BYBIT_API_KEY").ok()
    }

    pub fn get_bybit_secret_key() -> Option<String> {
        std::env::var("BYBIT_SECRET_KEY").ok()
    }
}

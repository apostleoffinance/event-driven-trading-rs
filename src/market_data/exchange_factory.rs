use crate::config::exchange_config::{ExchangeConfig, ExchangeType};
use crate::error::Result;
use crate::engine::EventBus;
use super::fetcher_trait::MarketDataFetcher;
use super::binance_fetcher::BinanceFetcher;
use super::bybit_fetcher::BybitFetcher;

pub struct ExchangeFactory;

impl ExchangeFactory {
    pub fn create_fetcher(
        config: &ExchangeConfig,
        event_bus: EventBus,
    ) -> Result<Box<dyn MarketDataFetcher>> {
        match &config.exchange_type {
            ExchangeType::Binance => {
                Ok(Box::new(BinanceFetcher::new(event_bus)))
            }
            ExchangeType::Bybit => {
                Ok(Box::new(BybitFetcher::new(event_bus)))
            }
        }
    }
}

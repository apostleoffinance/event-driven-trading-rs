use crate::config::exchange_config::{ExchangeConfig, ExchangeType};
use crate::error::Result;
use crate::engine::EventBus;
use super::fetcher_trait::MarketDataFetcher;
use super::binance_fetcher::BinanceFetcher;
use super::bybit_fetcher::BybitFetcher;
use super::resilient_fetcher::ResilientFetcher;

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

    pub fn create_resilient_fetcher(
        primary: ExchangeType,
        secondary: ExchangeType,
        event_bus: EventBus,
    ) -> Result<Box<dyn MarketDataFetcher>> {
        let primary_fetcher: Box<dyn MarketDataFetcher> = match primary {
            ExchangeType::Binance => Box::new(BinanceFetcher::new(event_bus.clone())),
            ExchangeType::Bybit => Box::new(BybitFetcher::new(event_bus.clone())),
        };

        let secondary_fetcher: Box<dyn MarketDataFetcher> = match secondary {
            ExchangeType::Binance => Box::new(BinanceFetcher::new(event_bus.clone())),
            ExchangeType::Bybit => Box::new(BybitFetcher::new(event_bus.clone())),
        };

        Ok(Box::new(ResilientFetcher::new(
            primary_fetcher,
            secondary_fetcher,
            event_bus,
        )))
    }
}

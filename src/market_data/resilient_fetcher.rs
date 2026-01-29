use async_trait::async_trait;
use crate::error::{Result, TradingError};
use crate::engine::{EventBus, Event};
use super::fetcher_trait::MarketDataFetcher;
use super::event::PriceEvent;

pub struct ResilientFetcher {
    primary: Box<dyn MarketDataFetcher>,
    secondary: Box<dyn MarketDataFetcher>,
    event_bus: EventBus,
}

impl ResilientFetcher {
    pub fn new(
        primary: Box<dyn MarketDataFetcher>,
        secondary: Box<dyn MarketDataFetcher>,
        event_bus: EventBus,
    ) -> Self {
        Self {
            primary,
            secondary,
            event_bus,
        }
    }
}

#[async_trait]
impl MarketDataFetcher for ResilientFetcher {
    async fn fetch_price(&self, symbol: &str) -> Result<PriceEvent> {
        match self.primary.fetch_price(symbol).await {
            Ok(event) => Ok(event),
            Err(primary_err) => {
                let msg = format!("Primary feed failed: {}", primary_err);
                let _ = self.event_bus.publish(Event::Error(msg));

                self.secondary.fetch_price(symbol).await.map_err(|secondary_err| {
                    TradingError::MarketData(format!(
                        "Secondary feed failed: {}", secondary_err
                    ))
                })
            }
        }
    }

    fn exchange_name(&self) -> &str {
        "ResilientFetcher"
    }
}
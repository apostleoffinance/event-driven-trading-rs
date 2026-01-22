use reqwest::Client;
use serde::{Deserialize, Serialize};
use rust_decimal::Decimal;
use async_trait::async_trait;

use crate::error::{TradingError, Result};
use crate::engine::EventBus;
use super::event::PriceEvent;
use super::fetcher_trait::MarketDataFetcher;

#[derive(Debug, Deserialize, Serialize)]
pub struct BinanceTickerResponse {
    pub symbol: String,
    #[serde(rename = "lastPrice")]
    pub last_price: String,
    pub volume: String,
}

pub struct BinanceFetcher {
    client: Client,
    base_url: String,
    event_bus: EventBus,
}

impl BinanceFetcher {
    pub fn new(event_bus: EventBus) -> Self {
        Self {
            client: Client::new(),
            base_url: "https://api.binance.com/api/v3".to_string(),
            event_bus,
        }
    }
}

#[async_trait]
impl MarketDataFetcher for BinanceFetcher {
    async fn fetch_price(&self, symbol: &str) -> Result<PriceEvent> {
        let url = format!("{}/ticker/24hr?symbol={}", self.base_url, symbol);

        let response = self.client
            .get(&url)
            .send()
            .await?
            .json::<BinanceTickerResponse>()
            .await?;

        let price = Decimal::from_str_exact(&response.last_price)
            .map_err(|e| TradingError::Decimal(e))?;
        
        let volume = Decimal::from_str_exact(&response.volume)
            .map_err(|e| TradingError::Decimal(e))?;

        let price_event = PriceEvent::new(response.symbol, price, volume)?;

        self.event_bus.publish(crate::engine::Event::PriceUpdated(price_event.clone()))?;

        Ok(price_event)
    }

    fn exchange_name(&self) -> &str {
        "Binance"
    }
}

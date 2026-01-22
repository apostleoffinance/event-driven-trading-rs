use reqwest::Client;
use serde::{Deserialize, Serialize};
use rust_decimal::Decimal;
use async_trait::async_trait;

use crate::error::{TradingError, Result};
use crate::engine::EventBus;
use super::event::PriceEvent;
use super::fetcher_trait::MarketDataFetcher;

/// Bybit API response structures
#[derive(Debug, Deserialize, Serialize)]
pub struct BybitResponse<T> {
    pub result: BybitResult<T>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct BybitResult<T> {
    pub list: Vec<T>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct BybitTickerData {
    pub symbol: String,
    #[serde(rename = "lastPrice")]
    pub last_price: String,
    pub volume24h: String,
}

pub struct BybitFetcher {
    client: Client,
    base_url: String,
    event_bus: EventBus,
}

impl BybitFetcher {
    pub fn new(event_bus: EventBus) -> Self {
        Self {
            client: Client::new(),
            base_url: "https://api.bybit.com/v5/market".to_string(),
            event_bus,
        }
    }
}

#[async_trait]
impl MarketDataFetcher for BybitFetcher {
    async fn fetch_price(&self, symbol: &str) -> Result<PriceEvent> {
        // Bybit uses USDT suffix for spot trading
        // If symbol is BTCUSDT, it stays BTCUSDT
        // If symbol is BTC, convert to BTCUSDT
        let bybit_symbol = if symbol.contains("USDT") {
            symbol.to_string()
        } else {
            format!("{}USDT", symbol)
        };

        let url = format!(
            "{}/tickers?category=spot&symbol={}",
            self.base_url, bybit_symbol
        );

        let response = self.client
            .get(&url)
            .send()
            .await?
            .json::<BybitResponse<BybitTickerData>>()
            .await?;

        let ticker = response
            .result
            .list
            .into_iter()
            .next()
            .ok_or_else(|| TradingError::MarketData(
                "No ticker data from Bybit".to_string(),
            ))?;

        let price = Decimal::from_str_exact(&ticker.last_price)
            .map_err(|e| TradingError::Decimal(e))?;
        
        let volume = Decimal::from_str_exact(&ticker.volume24h)
            .map_err(|e| TradingError::Decimal(e))?;

        let price_event = PriceEvent::new(symbol.to_string(), price, volume)?;

        self.event_bus.publish(crate::engine::Event::PriceUpdated(price_event.clone()))?;

        Ok(price_event)
    }

    fn exchange_name(&self) -> &str {
        "Bybit"
    }
}

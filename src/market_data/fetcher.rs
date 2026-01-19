use reqwest::Client;
use serde::{Deserialize, Serialize};
use rust_decimal::Decimal;

use crate::error::{TradingError, Result};
use super::event::PriceEvent;

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
}

impl BinanceFetcher {
    pub fn new() -> Self {
        Self {
            client: Client::new(),
            base_url: "https://api.binance.com/api/v3".to_string(),
        }
    }

    pub async fn fetch_price(&self, symbol: &str) -> Result<PriceEvent> {
        let url = format!("{}/ticker/24hr?symbol={}", self.base_url, symbol);

        let response = self.client
            .get(&url)
            .send()
            .await?
            .json::<BinanceTickerResponse>()
            .await?;

        let price = Decimal::from_str_exact(&response.last_price)
            .map_err(|e| TradingError::Decimal(e.to_string()))?;
        
        let volume = Decimal::from_str_exact(&response.volume)
            .map_err(|e| TradingError::Decimal(e.to_string()))?;

        Ok(PriceEvent::new(response.symbol, price, volume))
    }
}

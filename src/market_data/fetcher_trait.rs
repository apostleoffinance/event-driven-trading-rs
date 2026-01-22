use async_trait::async_trait;
use crate::error::Result;
use super::event::PriceEvent;

#[async_trait]
pub trait MarketDataFetcher: Send + Sync {
    async fn fetch_price(&self, symbol: &str) -> Result<PriceEvent>;
    fn exchange_name(&self) -> &str;
}

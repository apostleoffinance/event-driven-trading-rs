use super::event::PriceEvent;
use crate::error::Result;
use async_trait::async_trait;

#[allow(clippy::double_must_use)]
#[async_trait]
pub trait MarketDataFetcher: Send + Sync {
    async fn fetch_price(&self, symbol: &str) -> Result<PriceEvent>;
    fn exchange_name(&self) -> &str;
}

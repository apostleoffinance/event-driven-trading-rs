mod engine;
mod market_data;
mod strategy;
mod execution;
mod portfolio;
mod instrument;
mod utils;
mod error;

use market_data::{BinanceFetcher, PriceValidator};
use error::Result;

#[tokio::main]
async fn main() -> Result<()> {
    let fetcher = BinanceFetcher::new();

    println!("📊 Fetching BTC price from Binance...");
    let event = fetcher.fetch_price("BTCUSDT").await?;
    println!("✅ Fetched: {:?}", event);

    println!("\n🔍 Validating and normalizing price...");
    let normalized = PriceValidator::normalize(event)?;
    println!("✅ Normalized: {:?}", normalized);

    Ok(())
}

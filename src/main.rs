#![allow(dead_code)]
#![allow(unused_imports)]


mod engine;
mod market_data;
mod strategy;
mod execution;
mod portfolio;
mod instrument;
mod utils;
mod error;
mod risk;
mod config;

use market_data::{PriceValidator, ExchangeFactory};
use strategy::StrategyFactory;
use engine::{EventBus, Event};
use config::strategy_config::{StrategyConfig, StrategyType};
use config::exchange_config::{ExchangeConfig, ExchangeType};
use config::EnvConfig;
use rust_decimal::Decimal;
use error::Result;

#[tokio::main]
async fn main() -> Result<()> {
    // Load environment variables from .env file
    EnvConfig::load()?;
    // ==========================================
    // USER CONFIGURATION - CUSTOMIZE HERE
    // ==========================================
    
    // Choose which exchange to use:
    // ExchangeType::Binance  - Crypto spot trading
    // ExchangeType::Bybit    - Crypto derivatives & spot
    // ExchangeType::Alpaca   - Crypto trading (requires ALPACA_API_KEY in .env)
    let exchange_config = ExchangeConfig {
        exchange_type: ExchangeType::Binance,  // ← Change to Bybit or Alpaca for testing
        api_key: None,
        api_secret: None,
        enabled: true,
    };

    // Choose which strategy and parameters
    // NOTE: Risk is managed via profiles, not raw percentages
    let strategy_config = StrategyConfig {
        strategy_type: StrategyType::MeanReversion {
            threshold: Decimal::from_str_exact("0.02")?,
            window_size: 10,
        },
        symbol: "BTCUSDT".to_string(),
        risk_profile: config::strategy_config::RiskProfile::Balanced,  // User chooses profile
        enabled: true,
    };

    // ==========================================
    // VALIDATION
    // ==========================================
    exchange_config.validate()?;
    strategy_config.validate()?;

    // ==========================================
    // INITIALIZE EVENT BUS
    // ==========================================
    let event_bus = EventBus::new();

    println!("🔌 Setting up Event Bus subscribers...\n");

    // Subscribe to price updates
    event_bus.subscribe("PriceUpdated", |event| {
        if let Event::PriceUpdated(price_event) = event {
            println!("  📊 [EventBus] Price Updated: {} @ {}", 
                price_event.symbol, price_event.price);
        }
    })?;

    // Subscribe to signals
    event_bus.subscribe("SignalGenerated", |event| {
        if let Event::SignalGenerated { strategy_name, symbol, signal, price } = event {
            println!("  📈 [EventBus] Signal from {}: {:?} on {} @ {}", 
                strategy_name, signal, symbol, price);
        }
    })?;

    // Subscribe to trade execution
    event_bus.subscribe("TradeExecuted", |event| {
        if let Event::TradeExecuted { symbol, signal, entry_price, position_size, stop_loss } = event {
            println!("  🚀 [EventBus] Trade Executed: {:?} {} @ {} (Size: {}, SL: {})", 
                signal, symbol, entry_price, position_size, stop_loss);
        }
    })?;

    // Subscribe to errors
    event_bus.subscribe("Error", |event| {
        if let Event::Error(msg) = event {
            println!("  ❌ [EventBus] Error: {}", msg);
        }
    })?;

    println!("✅ Event Bus ready with subscribers\n");

    // ==========================================
    // CREATE EXCHANGE FETCHER (USER'S CHOICE)
    // ==========================================
    println!("📍 Creating market data fetcher...");
    let fetcher = ExchangeFactory::create_fetcher(&exchange_config, event_bus.clone())?;
    println!("✅ Using exchange: {}\n", fetcher.exchange_name());

    // ==========================================
    // CREATE STRATEGY (USER'S CHOICE)
    // ==========================================
    println!("🎯 Creating strategy...");
    let strategy = StrategyFactory::create_strategy(&strategy_config)?;
    println!("✅ Using strategy: {}\n", strategy.name());

    // ==========================================
    // FETCH MARKET DATA
    // ==========================================
    println!("📊 Fetching {} price from {}...", 
        strategy_config.symbol, fetcher.exchange_name());
    let price_event = fetcher.fetch_price(&strategy_config.symbol).await?;
    println!("✅ Fetched: {:?}\n", price_event);

    // ==========================================
    // VALIDATE & NORMALIZE
    // ==========================================
    println!("🔍 Validating and normalizing price...");
    let normalized = PriceValidator::normalize(price_event)?;
    println!("✅ Normalized: {:?}\n", normalized);

    // ==========================================
    // GENERATE SIGNAL
    // ==========================================
    println!("📈 Generating trading signal...");
    let signal = strategy.signal(&normalized)?;
    println!("✅ Signal: {:?}\n", signal);

    // Publish signal to event bus
    event_bus.publish(Event::SignalGenerated {
        strategy_name: strategy.name().to_string(),
        symbol: normalized.symbol.clone(),
        signal,
        price: normalized.price,
    })?;

    // ==========================================
    // SUMMARY
    // ==========================================
    let risk_params = strategy_config.get_risk_params();
    
    println!("📊 System Summary:");
    println!("  Exchange: {}", fetcher.exchange_name());
    println!("  Strategy: {}", strategy.name());
    println!("  Symbol: {}", strategy_config.symbol);
    println!("  Signal: {:?}", signal);
    println!("  Price: {}", normalized.price);
    println!("\n💼 Risk Profile: {}", strategy_config.risk_profile.description());
    println!("  Max Risk Per Trade: {}%", risk_params.max_risk_per_trade);
    println!("  Max Daily Loss: {}%", risk_params.max_daily_loss);
    println!("  Max Drawdown: {}%", risk_params.max_drawdown);
    println!("  Max Position Size: {}%", risk_params.max_position_size);
    println!("  Max Open Positions: {}", risk_params.max_open_positions);
    println!("  Max Leverage: {}x", risk_params.max_leverage);
    Ok(())
}

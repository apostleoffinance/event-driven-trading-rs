#![allow(dead_code)]
#![allow(unused_imports)]

mod config;
mod engine;
mod error;
mod execution;
mod instrument;
mod market_data;
mod portfolio;
mod risk;
mod strategy;
mod utils;

use config::exchange_config::{ExchangeConfig, ExchangeType};
use config::strategy_config::{StrategyConfig, StrategyType};
use config::EnvConfig;
use engine::{Event, EventBus};
use error::Result;
use execution::ExecutionEngine;
use market_data::{ExchangeFactory, PriceMonitor, PriceValidator};
use risk::PortfolioLimits;
use rust_decimal::Decimal;
use std::sync::{Arc, Mutex};
use strategy::StrategyFactory;

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
        exchange_type: ExchangeType::Binance, // ← Change to Bybit or Alpaca for testing
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
        risk_profile: config::strategy_config::RiskProfile::Balanced, // User chooses profile
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
    let execution_engine_ref: Arc<Mutex<Option<ExecutionEngine>>> = Arc::new(Mutex::new(None));
    let execution_engine_ref_clone = Arc::clone(&execution_engine_ref);
    event_bus.subscribe("PriceUpdated", move |event| {
        if let Event::PriceUpdated(price_event) = event {
            println!(
                "  📊 [EventBus] Price Updated: {} @ {}",
                price_event.symbol, price_event.price
            );

            if let Ok(mut guard) = execution_engine_ref_clone.lock() {
                if let Some(engine) = guard.as_mut() {
                    if let Err(err) = engine.update_price(&price_event.symbol, price_event.price) {
                        let _ = engine.is_kill_switch_active();
                        eprintln!("  ⚠️ [Risk] Price update error: {}", err);
                    }
                }
            }
        }
    })?;

    // Subscribe to signals
    event_bus.subscribe("SignalGenerated", |event| {
        if let Event::SignalGenerated {
            strategy_name,
            symbol,
            signal,
            price,
        } = event
        {
            println!(
                "  📈 [EventBus] Signal from {}: {:?} on {} @ {}",
                strategy_name, signal, symbol, price
            );
        }
    })?;

    // Subscribe to trade execution
    event_bus.subscribe("TradeExecuted", |event| {
        if let Event::TradeExecuted {
            symbol,
            signal,
            entry_price,
            position_size,
            stop_loss,
        } = event
        {
            println!(
                "  🚀 [EventBus] Trade Executed: {:?} {} @ {} (Size: {}, SL: {})",
                signal, symbol, entry_price, position_size, stop_loss
            );
        }
    })?;

    // Subscribe to risk halt
    event_bus.subscribe("RiskHalt", |event| {
        if let Event::RiskHalt { reason } = event {
            println!("  🛑 [Risk] Kill-switch activated: {}", reason);
        }
    })?;

    // Subscribe to order lifecycle events
    event_bus.subscribe("OrderSubmitted", |event| {
        if let Event::OrderSubmitted {
            order_id,
            symbol,
            side,
            quantity,
            price,
        } = event
        {
            println!(
                "  📝 [OMS] Order {} {:?} {} qty={} price={:?}",
                order_id, side, symbol, quantity, price
            );
        }
    })?;

    event_bus.subscribe("OrderFilled", |event| {
        if let Event::OrderFilled {
            order_id,
            symbol,
            filled_qty,
            price,
        } = event
        {
            println!(
                "  ✅ [OMS] Fill {} {} qty={} price={}",
                order_id, symbol, filled_qty, price
            );
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
    let fallback_exchange = match exchange_config.exchange_type {
        ExchangeType::Binance => ExchangeType::Bybit,
        ExchangeType::Bybit => ExchangeType::Binance,
    };
    let fetcher = ExchangeFactory::create_resilient_fetcher(
        exchange_config.exchange_type.clone(),
        fallback_exchange,
        event_bus.clone(),
    )?;
    println!("✅ Using exchange: {}\n", fetcher.exchange_name());

    // ==========================================
    // CREATE STRATEGY (USER'S CHOICE)
    // ==========================================
    println!("🎯 Creating strategy...");
    let strategy = StrategyFactory::create_strategy(&strategy_config)?;
    println!("✅ Using strategy: {}\n", strategy.name());

    // ==========================================
    // INITIALIZE RISK ENGINE + EXECUTION ENGINE
    // ==========================================
    let initial_balance = Decimal::from_str_exact("10000")?; // Example starting balance
    let risk_params = strategy_config.get_risk_params();
    let portfolio_limits = PortfolioLimits::from_risk_params(initial_balance, risk_params)?;
    let execution_engine =
        ExecutionEngine::new(initial_balance, portfolio_limits, event_bus.clone())?;
    if let Ok(mut guard) = execution_engine_ref.lock() {
        *guard = Some(execution_engine);
    }

    // ==========================================
    // FETCH MARKET DATA
    // ==========================================
    println!(
        "📊 Fetching {} price from {}...",
        strategy_config.symbol,
        fetcher.exchange_name()
    );
    let price_event = fetcher.fetch_price(&strategy_config.symbol).await?;
    println!("✅ Fetched: {:?}\n", price_event);

    // ==========================================
    // VALIDATE & NORMALIZE
    // ==========================================
    println!("🔍 Validating and normalizing price...");
    let normalized = PriceValidator::normalize(price_event)?;
    let mut monitor = PriceMonitor::new(60_000);
    let normalized = match monitor.process(normalized)? {
        Some(event) => event,
        None => {
            println!("⚠️ Duplicate market data ignored");
            return Ok(());
        }
    };
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
    // RISK-AWARE EXECUTION
    // ==========================================
    let (entry_price, stop_loss_distance, _strategy_position_size) =
        strategy.get_risk_params(normalized.price)?;

    let _ = execution_engine_ref
        .lock()
        .ok()
        .and_then(|mut guard| {
            guard.as_mut().map(|engine| {
                engine.execute(
                    normalized.symbol.clone(),
                    signal,
                    entry_price,
                    stop_loss_distance,
                )
            })
        })
        .unwrap_or_else(|| Ok(None))?;

    // ==========================================
    // SUMMARY
    // ==========================================

    println!("📊 System Summary:");
    println!("  Exchange: {}", fetcher.exchange_name());
    println!("  Strategy: {}", strategy.name());
    println!("  Symbol: {}", strategy_config.symbol);
    println!("  Signal: {:?}", signal);
    println!("  Price: {}", normalized.price);
    println!(
        "\n💼 Risk Profile: {}",
        strategy_config.risk_profile.description()
    );
    println!("  Max Risk Per Trade: {}%", risk_params.max_risk_per_trade);
    println!("  Max Daily Loss: {}%", risk_params.max_daily_loss);
    println!("  Max Drawdown: {}%", risk_params.max_drawdown);
    println!("  Max Position Size: {}%", risk_params.max_position_size);
    println!("  Max Open Positions: {}", risk_params.max_open_positions);
    println!("  Max Leverage: {}x", risk_params.max_leverage);

    let metrics = event_bus.metrics_snapshot();
    println!("\n📈 Event Metrics: {:?}", metrics);
    Ok(())
}

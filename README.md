# Event Trading Engine

A high-performance, event-driven cryptocurrency trading system built in Rust with institutional-grade risk management.

## 🎯 Project Goals

- Fetch live market data from multiple exchanges
- Validate and normalize price data with decimal precision
- Execute strategies with configurable risk profiles
- Track positions and manage portfolio risk
- Process events through a pub/sub event bus

## 📦 Project Structure

```
src/
├── main.rs                 # Application entry point
├── lib.rs                  # Library exports
├── error.rs                # Centralized error handling
├── engine/                 # Event bus & event types
├── market_data/            # Multi-exchange price ingestion
├── strategy/               # Strategy interface & implementations
├── execution/              # Paper trading engine
├── portfolio/              # Position tracking
├── risk/                   # Position sizing, stop loss, limits
├── config/                 # Strategy, exchange & env configuration
├── instrument/             # Asset definitions
└── utils/                  # Utilities (clock, etc.)
```

## ✅ Completed Features

### Multi-Exchange Support
| Exchange | Status | API Key Required |
|----------|--------|------------------|
| Binance  | ✅ Live | No (public data) |
| Bybit    | ✅ Live | No (public data) |

### Event Bus Architecture
- Decoupled pub/sub messaging system
- Event types: `PriceUpdated`, `SignalGenerated`, `TradeExecuted`, `TradeClosed`, `Error`
- Multiple strategies can subscribe independently

### Risk Profiles (Institutional-Grade)
```
Conservative:  1% risk/trade, 5% daily limit, 1.0x leverage
Balanced:      2% risk/trade, 10% daily limit, 1.5x leverage
Aggressive:    3% risk/trade, 15% daily limit, 2.0x leverage
```

### Financial Data Integrity
- `rust_decimal::Decimal` for all prices and volumes
- Zero floating-point errors
- 8 decimal place precision

### Configuration System
- Environment variables via `.env` file
- Strategy configuration with validation
- Exchange configuration with factory pattern

## 🚀 Getting Started

### Prerequisites
- Rust 1.70+

### Run
```bash
cargo run
```

### Test All Exchanges
```bash
cargo run --bin test_all_exchanges
```

### Run Tests
```bash
cargo test
```

## 📋 Dependencies

- `tokio` - Async runtime
- `reqwest` - HTTP client
- `serde` - JSON serialization
- `rust_decimal` - Financial precision
- `thiserror` - Error handling
- `async_trait` - Async traits
- `dotenv` - Environment variables
- `rust_decimal` - High-precision decimal arithmetic
- `thiserror` - Error handling macros

## 📝 License

MIT
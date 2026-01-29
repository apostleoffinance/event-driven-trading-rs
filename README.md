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
├── execution/              # Paper trading engine + OMS/EMS
├── portfolio/              # Position tracking + PnL
├── risk/                   # Risk engine & portfolio limits
├── config/                 # Strategy, exchange & env configuration
├── instrument/             # Asset definitions
└── utils/                  # Utilities (clock, etc.)
```

## ✅ Completed Features (v1)

### Multi-Exchange Support
| Exchange | Status | API Key Required |
|----------|--------|------------------|
| Binance  | ✅ Live | No (public data) |
| Bybit    | ✅ Live | No (public data) |

### Event Bus Architecture
- Decoupled pub/sub messaging system
- Event types: `PriceUpdated`, `SignalGenerated`, `TradeExecuted`, `TradeClosed`, `RiskHalt`, `OrderSubmitted`, `OrderFilled`, `OrderCancelled`, `OrderRejected`, `Error`

### Risk Engine (Institutional-Grade)
- Pre-trade validation: limits, exposure, leverage
- Margin checks and daily loss limits
- Live monitoring with kill-switch
- Automatic liquidation on kill-switch activation

### OMS/EMS (Paper Trading)
- Order lifecycle with submit/cancel/replace
- Partial fills and fill simulation
- Rejections and state tracking

### Portfolio & PnL
- Position tracking with realized/unrealized PnL
- Reconciliation hooks

### Market Data Reliability
- Price normalization with Decimal precision
- Dedupe and gap detection
- Resilient fetcher with primary/secondary failover

### Observability (Basic)
- Event counters in the event bus

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

## 📝 License

MIT
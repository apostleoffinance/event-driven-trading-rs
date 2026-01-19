# Event Trading Engine

A high-performance, event-driven cryptocurrency trading system built in Rust. This project demonstrates real-world Rust patterns including ownership, lifetimes, traits, and error handling through a minimal end-to-end trading system.

## 🎯 Project Goals

Build a trading system that can:
- Fetch live market data from Binance
- Validate and normalize price data with decimal precision
- Execute strategies with paper trading (no real money)
- Track positions and manage a portfolio
- Process events through a deterministic event loop

**Target Skills:** Ownership & lifetimes, traits for financial behavior, error handling under data failures, and building production-grade systems in Rust.

## 📦 Project Structure

```
src/
├── main.rs                 # Application entry point
├── error.rs               # Centralized error handling
├── engine/                # Event loop & bus
├── market_data/           # Price ingestion & normalization
├── strategy/              # Strategy interface & signals
├── execution/             # Paper trading engine
├── portfolio/             # Position tracking
├── instrument/            # Asset definitions
└── utils/                 # Utilities (clock, etc.)
```

## ✅ Completed: Market Data Module

### Features
- **REST Price Ingestion:** Fetches real-time BTC/USDT prices from Binance API
- **Unified Event Format:** All market data standardized to `PriceEvent` struct
- **Validation:** Ensures prices are positive, volumes are non-negative, symbols exist
- **Normalization:** Rounds all prices to 8 decimal places for consistency
- **Precision:** Uses `rust_decimal::Decimal` instead of `f64` for financial accuracy
- **Error Handling:** Zero `.unwrap()` calls—proper error propagation with `?` operator

### Data Flow
```
Binance API
    ↓
BinanceFetcher (REST ingestion)
    ↓
PriceEvent (Unified format with Decimal precision)
    ↓
PriceValidator (Validate + Normalize)
    ↓
Clean, standardized event ready to use
```

### Example Output
```
📊 Fetching BTC price from Binance...
✅ Fetched: PriceEvent { symbol: "BTCUSDT", price: 93237.50000000, timestamp: 1768841156417, volume: 15082.81988000 }

🔍 Validating and normalizing price...
✅ Normalized: PriceEvent { symbol: "BTCUSDT", price: 93237.50000000, timestamp: 1768841156417, volume: 15082.81988000 }
```

## 🔄 Next Steps

- [ ] Implement Strategy Interface
- [ ] Build Execution Engine (Paper Trading)
- [ ] Create Event Loop
- [ ] Implement Portfolio Tracking
- [ ] Add Integration Tests

## 🚀 Getting Started

### Prerequisites
- Rust 1.70+
- Cargo

### Run
```bash
cargo run
```

### Test
```bash
cargo test
```

## 📚 Key Concepts Used

- **Ownership & Lifetimes:** Borrowing in fetcher and validator functions
- **Traits:** `PriceValidator` for extensible validation
- **Error Handling:** `Result<T>` type with `thiserror` crate
- **Decimal Precision:** `rust_decimal` for accurate financial calculations
- **Async/Await:** Tokio runtime for non-blocking API calls
- **Serialization:** Serde for JSON handling

## 📋 Dependencies

- `tokio` - Async runtime
- `reqwest` - HTTP client for REST API
- `serde` - Serialization framework
- `rust_decimal` - High-precision decimal arithmetic
- `thiserror` - Error handling macros

## 📝 License

MIT
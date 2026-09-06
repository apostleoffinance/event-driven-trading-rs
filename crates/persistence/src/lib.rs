//! PostgreSQL persistence for quantitative trading entities.
//!
//! Financial values are stored as `NUMERIC` and mapped to `rust_decimal::Decimal`.
//! This crate must not contain strategy logic or venue HTTP clients.

#![forbid(unsafe_code)]
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

pub mod error;
pub mod pool;
pub mod repos;

pub use error::{PersistenceError, PersistenceResult};
pub use pool::{connect, migrate};
pub use repos::TradingStore;

//! Strategy runtime: interface for stateful strategies that emit [`TradeIntent`]s.
//!
//! Strategies must not submit orders, size account risk, or call venues.

#![forbid(unsafe_code)]
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

pub mod context;
pub mod error;
pub mod intent_id;
pub mod strategy;
pub mod worker;

pub use context::StrategyContext;
pub use error::{StrategyError, StrategyResult};
pub use intent_id::next_trade_intent_id;
pub use strategy::Strategy;
pub use worker::process_market_envelope;

//! Continuous trading runtime: orchestrates the modular pipeline.
//!
//! ```text
//! MarketData → Strategy → TradeIntent → Risk → OMS → Venue → Fills → Positions
//! ```
//!
//! This crate owns orchestration only — not risk rules, venue HTTP, or SQL.

#![forbid(unsafe_code)]
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

pub mod config;
pub mod error;
pub mod ids;
pub mod pipeline;
pub mod runtime;

pub use config::RuntimeConfig;
pub use error::{RuntimeError, RuntimeResult};
pub use ids::client_order_id_for_intent;
pub use pipeline::IntentOutcome;
pub use runtime::{TickOutcome, TradingRuntime};

pub use reconciliation::{InternalSnapshot, Reconciler, ReconciliationBreak, ReconciliationReport};

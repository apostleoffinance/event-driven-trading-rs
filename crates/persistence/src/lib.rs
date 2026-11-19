//! PostgreSQL persistence for quantitative trading entities.
//!
//! Financial values are stored as `NUMERIC` and mapped to `rust_decimal::Decimal`.
//! This crate must not contain strategy logic or venue HTTP clients.

#![forbid(unsafe_code)]
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

pub mod broker_admission;
pub mod broker_worker_lease;
pub mod error;
pub mod pool;
pub mod mt5_terminal_slot_admission;
pub mod repos;

pub use broker_admission::evaluate_persisted_broker_admission;
pub use broker_worker_lease::Mt5WorkerLease;
pub use mt5_terminal_slot_admission::validate_ready_mt5_terminal_slot;
pub use error::{PersistenceError, PersistenceResult};
pub use pool::{check_database_health, connect, migrate, validate_identity_runtime_role};
pub use repos::{
    BrokerBindingTransition, BrokerBindingView, BrokerCatalogEntry, Mt5DurableOrderState,
    Mt5OrderClaim, Mt5SequenceObservation, Mt5StreamReadiness, TradingStore,
};

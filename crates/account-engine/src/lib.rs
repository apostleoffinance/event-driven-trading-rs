//! Account engine: registry, live state, snapshots, and account-level controls.
//!
//! Account (capital / risk ownership) is distinct from Venue (execution location).
//! This crate must not contain venue HTTP or broker SDK logic.

#![forbid(unsafe_code)]
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

pub mod config;
pub mod error;
pub mod service;
pub mod simulated;

pub use config::AccountConfig;
pub use error::{AccountEngineError, AccountEngineResult};
pub use service::{
    default_simulated_venue_id, AccountEngine, AccountRecord, InMemoryAccountEngine,
};
pub use simulated::SimulatedAccountSpec;

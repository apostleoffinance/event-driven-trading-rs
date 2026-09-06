//! Reconciliation: compare internal engine state to venue state.
//!
//! Mismatches emit [`TradingEvent::ReconciliationAlert`] only.
//! This crate must **never** silently overwrite account, OMS, or position books.

#![forbid(unsafe_code)]
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

pub mod compare;
pub mod engine;
pub mod error;
pub mod report;

pub use compare::{compare_snapshots, CompareTolerances};
pub use engine::Reconciler;
pub use error::{ReconciliationError, ReconciliationResult};
pub use report::{InternalSnapshot, ReconciliationBreak, ReconciliationReport};

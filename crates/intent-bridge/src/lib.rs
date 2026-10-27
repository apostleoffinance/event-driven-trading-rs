//! Python strategy → Rust execution bridge.
//!
//! Strategies run in Python and emit JSON [`TradeIntentWire`] lines.
//! This crate parses them and runs risk → OMS → venue in Rust.

#![forbid(unsafe_code)]
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

pub mod error;
pub mod ingest;
pub mod mt5_session;
pub mod mt5_v1;
pub mod wire;

pub use error::{BridgeError, BridgeResult};
pub use ingest::{BridgeVenueKind, IntentIngestSession};
pub use mt5_session::{Mt5SessionGate, Mt5SessionPolicy, Mt5SessionState};
pub use wire::{parse_intent_json, TradeIntentWire};

//! Python strategy → Rust execution bridge.
//!
//! Strategies run in Python and emit JSON [`TradeIntentWire`] lines.
//! This crate parses them and runs risk → OMS → venue in Rust.

#![forbid(unsafe_code)]
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

pub mod error;
pub mod ingest;
pub mod mt5_dispatch;
pub mod mt5_reconciliation;
pub mod mt5_runtime;
pub mod mt5_session;
pub mod mt5_transport;
pub mod mt5_v1;
pub mod wire;

pub use error::{BridgeError, BridgeResult};
pub use ingest::{BridgeVenueKind, IntentIngestSession};
pub use mt5_dispatch::{prepare_submit, Mt5DispatchDecision, Mt5DispatchPermit};
pub use mt5_reconciliation::{
    persist_and_confirm_reconciliation, Mt5AccountStateSnapshot, Mt5OpenOrderSnapshot,
    Mt5PositionSnapshot, Mt5ReconciliationSnapshot, Mt5ReconciliationSource,
};
pub use mt5_runtime::{Mt5BridgeRuntime, Mt5RuntimeDispatchResult};
pub use mt5_session::{Mt5SessionGate, Mt5SessionPolicy, Mt5SessionState};
pub use mt5_transport::{
    spawn_heartbeat_watchdog, AuthenticatedMt5Connection, Mt5TransportConfig, Mt5TransportLimits,
    Mt5TransportPrincipal,
};
pub use wire::{parse_intent_json, TradeIntentWire};

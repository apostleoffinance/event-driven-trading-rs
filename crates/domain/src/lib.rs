//! Shared quantitative trading domain primitives.
//!
//! This crate owns business concepts only. It must not depend on:
//! databases, HTTP clients, exchange SDKs, strategy implementations,
//! or runtime orchestration.
//!
//! Money, prices, quantities, and risk ratios use [`rust_decimal::Decimal`].
//! All fallible construction returns [`DomainError`] via [`DomainResult`].

#![forbid(unsafe_code)]
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

pub mod account;
pub mod error;
pub mod fill;
pub mod ids;
pub mod instrument;
pub mod money;
pub mod order;
pub mod position;
pub mod risk;
pub mod strategy;
pub mod trade_intent;
pub mod venue;

pub use account::{Account, AccountSnapshot, AccountState, AccountStatus, AccountType};
pub use error::{DomainError, DomainResult};
pub use fill::Fill;
pub use ids::*;
pub use instrument::{Instrument, InstrumentSpec, InstrumentType};
pub use order::{Order, OrderSide, OrderStatus, OrderType, TimeInForce};
pub use position::{Position, PositionSide};
pub use risk::{RiskDecision, RiskPolicy, RiskProfile, RiskRejectReason, RiskRequest};
pub use strategy::{Strategy, StrategyDeployment, StrategyMetadata};
pub use trade_intent::TradeIntent;
pub use venue::{Venue, VenueType};

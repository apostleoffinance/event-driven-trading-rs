//! Deterministic risk evaluation for trade intents.
//!
//! Given the same [`RiskRequest`], the evaluator always returns the same
//! [`RiskDecision`]. This crate does not submit orders, call venues, or
//! depend on strategy implementations.

#![forbid(unsafe_code)]
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

pub mod error;
pub mod evaluator;
pub mod instrument_sizing;
pub mod rules;
pub mod sizing;

#[cfg(test)]
mod test_support;

pub use error::{RiskEngineError, RiskEngineResult};
pub use evaluator::{DefaultRiskEvaluator, RiskEvaluator};
pub use instrument_sizing::{
    size_order_instrument_aware, InstrumentSizingContext, InstrumentSizingOutcome, LossModel,
    SizingLimitReason, SizingRejectReason,
};
pub use rules::RuleOutcome;

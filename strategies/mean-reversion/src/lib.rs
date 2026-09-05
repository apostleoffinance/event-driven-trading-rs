//! Mean-reversion strategy: buys below the rolling mean, sells above it.
//!
//! Emits [`TradeIntent`] only. Does not size account risk or submit orders.

#![forbid(unsafe_code)]
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod strategy;

pub use strategy::{MeanReversionConfig, MeanReversionStrategy};

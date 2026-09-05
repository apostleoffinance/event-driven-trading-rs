// Library interface for event-trading
// Exposes all public modules for use in binaries and tests
#![allow(dead_code)]
#![allow(unused_imports)]

pub mod config;
pub mod engine;
pub mod error;
pub mod execution;
pub mod instrument;
pub mod market_data;
pub mod portfolio;
pub mod risk;
pub mod strategy;
pub mod utils;

pub use error::{Result, TradingError};

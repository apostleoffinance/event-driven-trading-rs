//! EdgeNest market-data service: read-only candle history behind a provider
//! abstraction, plus one optional read-only MT5 latest-quote route. No trading
//! and no writes; the only database access is the scoped, read-only MT5 snapshot
//! read in `mt5_latest`.

pub mod config;
pub mod error;
pub mod fx_conversion;
pub mod http;
pub mod instruments;
pub mod model;
pub mod mt5_latest;
pub mod provider;
pub mod providers;
pub mod service;
pub mod validation;

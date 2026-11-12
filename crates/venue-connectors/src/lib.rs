//! Venue connectors.
//!
//! [`VenueAdapter`] exposes venue/account information. Simulation and paper
//! execution use [`SimulationExecutionAdapter`]; broker execution has no public
//! capability in this crate.

#![forbid(unsafe_code)]
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

pub mod adapter;
pub mod bridge;
pub mod error;
pub mod prop;
pub mod simulated;
pub mod types;

pub use adapter::{SimulationExecutionAdapter, VenueAdapter};
pub use bridge::{cancel_simulated_order, execute_simulated_order};
pub use error::{VenueError, VenueResult};
pub use prop::{PropVenue, PropVenueConfig, PropVenueMode};
pub use simulated::SimulatedVenue;
pub use types::{CancelRequest, OrderAck, OrderRequest};

#[cfg(test)]
extern crate self as venue_connectors;

#[cfg(test)]
#[path = "tests/ambiguous_unknown.rs"]
mod ambiguous_unknown_tests;
#[cfg(test)]
#[path = "tests/cancel_confirmation.rs"]
mod cancel_confirmation_tests;
#[cfg(test)]
#[path = "tests/failure_injection.rs"]
mod failure_injection_tests;

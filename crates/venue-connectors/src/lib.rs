//! Venue connectors.
//!
//! [`VenueAdapter`] abstracts execution venues. The execution engine must not
//! know whether it talks to [`SimulatedVenue`], [`PropVenue`], or a future CEX.

#![forbid(unsafe_code)]
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

pub mod adapter;
pub mod bridge;
pub mod error;
pub mod prop;
pub mod simulated;
pub mod types;

pub use adapter::VenueAdapter;
pub use bridge::{cancel_order, submit_approved_order};
pub use error::{VenueError, VenueResult};
pub use prop::{PropVenue, PropVenueConfig, PropVenueMode};
pub use simulated::SimulatedVenue;
pub use types::{CancelRequest, OrderAck, OrderRequest};

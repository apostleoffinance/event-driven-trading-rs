//! Typed in-process async trading events.
//!
//! Events are immutable facts published over Tokio `mpsc` channels.
//! This crate must not own strategy logic, risk evaluation, HTTP, or persistence.

#![forbid(unsafe_code)]
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

pub mod bus;
pub mod error;
pub mod ids;
pub mod market_data;
pub mod trading_event;

pub use bus::{event_channel, EventPublisher, EventSubscriber, DEFAULT_EVENT_CHANNEL_CAPACITY};
pub use error::{EventsError, EventsResult};
pub use ids::EventId;
pub use market_data::MarketDataEvent;
pub use trading_event::{EventEnvelope, TradingEvent};

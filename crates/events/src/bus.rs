//! Tokio `mpsc`-backed in-process event transport.
//!
//! No Kafka/NATS/Redis — typed channels only for this phase.

use std::sync::atomic::{AtomicU64, Ordering};

use chrono::Utc;
use tokio::sync::mpsc;

use crate::error::{EventsError, EventsResult};
use crate::ids::{CorrelationId, EventId};
use crate::trading_event::{EventEnvelope, TradingEvent};

/// Default bounded capacity for the trading event channel.
pub const DEFAULT_EVENT_CHANNEL_CAPACITY: usize = 1024;

static EVENT_SEQ: AtomicU64 = AtomicU64::new(1);

fn next_event_id() -> EventsResult<EventId> {
    let seq = EVENT_SEQ.fetch_add(1, Ordering::Relaxed);
    let millis = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|e| EventsError::Invalid(format!("clock error: {e}")))?
        .as_millis();
    EventId::new(format!("evt-{millis}-{seq}"))
}

/// Cloneable publisher for immutable [`EventEnvelope`] values.
#[derive(Debug, Clone)]
pub struct EventPublisher {
    tx: mpsc::Sender<EventEnvelope>,
}

impl EventPublisher {
    /// Publish a pre-built envelope.
    pub async fn publish_envelope(&self, envelope: EventEnvelope) -> EventsResult<()> {
        let name = envelope.event.name();
        tracing::debug!(event_id = %envelope.id, event = name, "publishing trading event");
        self.tx
            .send(envelope)
            .await
            .map_err(|_| EventsError::ChannelClosed)
    }

    /// Wrap `event` in a new envelope and publish.
    pub async fn publish(&self, event: TradingEvent) -> EventsResult<EventEnvelope> {
        let envelope = EventEnvelope {
            id: next_event_id()?,
            correlation_id: None,
            causation_id: None,
            occurred_at: Utc::now(),
            event,
        };
        self.publish_envelope(envelope.clone()).await?;
        Ok(envelope)
    }

    /// Publish with correlation / causation linkage for audit reconstruction.
    pub async fn publish_correlated(
        &self,
        event: TradingEvent,
        correlation_id: CorrelationId,
        causation_id: Option<EventId>,
    ) -> EventsResult<EventEnvelope> {
        let envelope = EventEnvelope {
            id: next_event_id()?,
            correlation_id: Some(correlation_id),
            causation_id,
            occurred_at: Utc::now(),
            event,
        };
        self.publish_envelope(envelope.clone()).await?;
        Ok(envelope)
    }

    /// Non-blocking publish; returns an error if the channel is full or closed.
    pub fn try_publish(&self, event: TradingEvent) -> EventsResult<EventEnvelope> {
        let envelope = EventEnvelope {
            id: next_event_id()?,
            correlation_id: None,
            causation_id: None,
            occurred_at: Utc::now(),
            event,
        };
        let name = envelope.event.name();
        tracing::debug!(event_id = %envelope.id, event = name, "try_publish trading event");
        self.tx
            .try_send(envelope.clone())
            .map_err(|err| match err {
                mpsc::error::TrySendError::Full(_) => {
                    EventsError::Publish("event channel is full".to_string())
                }
                mpsc::error::TrySendError::Closed(_) => EventsError::ChannelClosed,
            })?;
        Ok(envelope)
    }

    pub fn is_closed(&self) -> bool {
        self.tx.is_closed()
    }
}

/// Single-consumer subscriber for trading events.
#[derive(Debug)]
pub struct EventSubscriber {
    rx: mpsc::Receiver<EventEnvelope>,
}

impl EventSubscriber {
    /// Receive the next event, or `None` if the channel is closed and empty.
    pub async fn recv(&mut self) -> Option<EventEnvelope> {
        self.rx.recv().await
    }

    /// Non-blocking receive.
    pub fn try_recv(&mut self) -> EventsResult<EventEnvelope> {
        self.rx.try_recv().map_err(|err| match err {
            mpsc::error::TryRecvError::Empty => {
                EventsError::Publish("event channel is empty".to_string())
            }
            mpsc::error::TryRecvError::Disconnected => EventsError::ChannelClosed,
        })
    }
}

/// Create a bounded publisher/subscriber pair.
pub fn event_channel(capacity: usize) -> (EventPublisher, EventSubscriber) {
    let capacity = capacity.max(1);
    let (tx, rx) = mpsc::channel(capacity);
    (EventPublisher { tx }, EventSubscriber { rx })
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;
    use crate::market_data::MarketDataEvent;
    use chrono::Utc;
    use domain::InstrumentId;
    use rust_decimal::Decimal;

    #[tokio::test]
    async fn publish_and_receive_market_data() {
        let (pub_, mut sub) = event_channel(8);
        let md = MarketDataEvent::new(
            InstrumentId::new("BTCUSDT").unwrap(),
            Decimal::from(90_000),
            "binance",
            Utc::now(),
            Utc::now(),
        )
        .unwrap();

        pub_.publish(TradingEvent::MarketDataReceived { market_data: md })
            .await
            .unwrap();

        let env = sub.recv().await.expect("event");
        assert_eq!(env.event.name(), "MarketDataReceived");
    }

    #[tokio::test]
    async fn channel_closed_on_drop_subscriber() {
        let (pub_, sub) = event_channel(1);
        drop(sub);
        let err = pub_
            .publish(TradingEvent::TradingHalted {
                account_id: None,
                strategy_id: None,
                reason: "test".into(),
            })
            .await
            .unwrap_err();
        assert_eq!(err, EventsError::ChannelClosed);
    }
}

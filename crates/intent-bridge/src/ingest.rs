//! Ingest NDJSON TradeIntents into the Rust trading runtime.

use std::io::{BufRead, BufReader, Read};

use domain::{AccountId, RiskPolicy, VenueId};
use events::{event_channel, EventPublisher, EventSubscriber};
use rust_decimal::Decimal;
use trading_runtime::{IntentOutcome, RuntimeConfig, TradingRuntime};
use venue_connectors::{PropVenue, SimulatedVenue};

use crate::error::{BridgeError, BridgeResult};
use crate::wire::parse_intent_json;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BridgeVenueKind {
    Simulated,
    Prop,
}

/// Session that accepts Python-emitted intents and runs the Rust pipeline.
pub struct IntentIngestSession {
    runtime: TradingRuntime,
    events_out: EventPublisher,
    /// Kept alive so publishes do not fail with ChannelClosed.
    _events_sub: EventSubscriber,
    venue_kind: BridgeVenueKind,
    simulated: Option<SimulatedVenue>,
    prop: Option<PropVenue>,
}

impl IntentIngestSession {
    pub async fn bootstrap(
        account_id: AccountId,
        venue_kind: BridgeVenueKind,
        capital: Decimal,
        policy: RiskPolicy,
    ) -> BridgeResult<Self> {
        let (events_out, events_sub) = event_channel(1024);

        match venue_kind {
            BridgeVenueKind::Simulated => {
                let venue_id = VenueId::new("simulated")?;
                let config = RuntimeConfig::new(account_id, venue_id, policy);
                let mut runtime = TradingRuntime::new(config);
                let venue = SimulatedVenue::new(VenueId::new("simulated")?);
                runtime.bootstrap_prop_on_simulated(&venue, capital).await?;
                Ok(Self {
                    runtime,
                    events_out,
                    _events_sub: events_sub,
                    venue_kind,
                    simulated: Some(venue),
                    prop: None,
                })
            }
            BridgeVenueKind::Prop => {
                let venue_id = VenueId::new("prop")?;
                let config = RuntimeConfig::new(account_id, venue_id, policy);
                let mut runtime = TradingRuntime::new(config);
                let venue = PropVenue::paper()?;
                runtime.bootstrap_prop(&venue, capital).await?;
                Ok(Self {
                    runtime,
                    events_out,
                    _events_sub: events_sub,
                    venue_kind,
                    simulated: None,
                    prop: Some(venue),
                })
            }
        }
    }

    pub fn runtime(&self) -> &TradingRuntime {
        &self.runtime
    }

    /// Execute one domain intent through risk → OMS → venue.
    pub async fn ingest_intent(
        &mut self,
        intent: &domain::TradeIntent,
    ) -> BridgeResult<IntentOutcome> {
        self.events_out
            .publish(events::TradingEvent::TradeIntentCreated {
                intent: intent.clone(),
            })
            .await?;

        match self.venue_kind {
            BridgeVenueKind::Simulated => {
                let venue = self
                    .simulated
                    .as_ref()
                    .ok_or_else(|| BridgeError::Invalid("simulated venue missing".into()))?;
                Ok(self
                    .runtime
                    .execute_intent(intent, venue, &self.events_out)
                    .await?)
            }
            BridgeVenueKind::Prop => {
                let venue = self
                    .prop
                    .as_ref()
                    .ok_or_else(|| BridgeError::Invalid("prop venue missing".into()))?;
                Ok(self
                    .runtime
                    .execute_intent(intent, venue, &self.events_out)
                    .await?)
            }
        }
    }

    /// Parse JSON and execute.
    pub async fn ingest_json(&mut self, json: &str) -> BridgeResult<IntentOutcome> {
        let intent = parse_intent_json(json)?;
        self.ingest_intent(&intent).await
    }

    /// Read NDJSON from `reader` (one TradeIntent JSON object per line).
    pub async fn ingest_ndjson<R: Read>(&mut self, reader: R) -> BridgeResult<Vec<IntentOutcome>> {
        let buffered = BufReader::new(reader);
        let mut outcomes = Vec::new();
        for line in buffered.lines() {
            let line = line?;
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }
            outcomes.push(self.ingest_json(trimmed).await?);
        }
        Ok(outcomes)
    }
}

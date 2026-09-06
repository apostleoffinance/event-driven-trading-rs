//! Helpers to drive a strategy from async market-data events.

use chrono::Utc;
use domain::TradeIntent;
use events::{
    EventEnvelope, EventPublisher, MarketDataHealthTracker, TradingEvent,
};

use crate::context::StrategyContext;
use crate::error::StrategyResult;
use crate::strategy::Strategy;

/// Process a single market-data envelope through `strategy` and publish intents.
///
/// When `health` reports non-tradable (stale / gap), intents are **not** emitted
/// (fail closed). Returns the created intents (also published as events).
pub async fn process_market_envelope(
    strategy: &mut dyn Strategy,
    ctx: &StrategyContext,
    envelope: &EventEnvelope,
    intent_publisher: &EventPublisher,
) -> StrategyResult<Vec<TradeIntent>> {
    process_market_envelope_with_health(strategy, ctx, envelope, intent_publisher, None).await
}

/// Same as [`process_market_envelope`] with optional health tracking.
pub async fn process_market_envelope_with_health(
    strategy: &mut dyn Strategy,
    ctx: &StrategyContext,
    envelope: &EventEnvelope,
    intent_publisher: &EventPublisher,
    health_tracker: Option<&mut MarketDataHealthTracker>,
) -> StrategyResult<Vec<TradeIntent>> {
    let TradingEvent::MarketDataReceived { market_data } = &envelope.event else {
        return Ok(Vec::new());
    };

    if let Some(tracker) = health_tracker {
        let health = tracker.observe(market_data, Utc::now());
        if !health.is_tradable() {
            intent_publisher
                .publish(TradingEvent::MarketDataUnhealthy { health })
                .await?;
            return Ok(Vec::new());
        }
    }

    let intents = strategy.on_market_data(ctx, market_data)?;

    for intent in &intents {
        intent_publisher
            .publish(TradingEvent::StrategySignalGenerated {
                strategy_id: intent.strategy_id.clone(),
                strategy_version: intent.strategy_version.clone(),
                deployment_id: intent.deployment_id.clone(),
                instrument_id: intent.instrument_id.clone(),
                side: intent.side,
                price: intent.entry_price.unwrap_or(market_data.price),
            })
            .await?;

        intent_publisher
            .publish(TradingEvent::TradeIntentCreated {
                intent: intent.clone(),
            })
            .await?;
    }

    Ok(intents)
}

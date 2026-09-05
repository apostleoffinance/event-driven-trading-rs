//! Helpers to drive a strategy from async market-data events.

use domain::TradeIntent;
use events::{EventEnvelope, EventPublisher, TradingEvent};

use crate::context::StrategyContext;
use crate::error::StrategyResult;
use crate::strategy::Strategy;

/// Process a single market-data envelope through `strategy` and publish intents.
///
/// Returns the created intents (also published as `TradeIntentCreated` events).
pub async fn process_market_envelope(
    strategy: &mut dyn Strategy,
    ctx: &StrategyContext,
    envelope: &EventEnvelope,
    intent_publisher: &EventPublisher,
) -> StrategyResult<Vec<TradeIntent>> {
    let TradingEvent::MarketDataReceived { market_data } = &envelope.event else {
        return Ok(Vec::new());
    };

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

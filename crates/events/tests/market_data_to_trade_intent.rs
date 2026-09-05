//! Phase 2 acceptance: MarketData → Strategy → TradeIntent over async channels.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use chrono::Utc;
use domain::{
    DeploymentId, InstrumentId, OrderSide, StrategyId, StrategyVersion, TradeIntent, TradeIntentId,
};
use events::{event_channel, EventPublisher, MarketDataEvent, TradingEvent};
use rust_decimal::Decimal;
use tokio::sync::oneshot;

/// Minimal stand-in for the future strategy-runtime (Phase 3).
async fn strategy_task(
    mut market_rx: events::EventSubscriber,
    intent_tx: EventPublisher,
    ready: oneshot::Sender<()>,
) {
    let _ = ready.send(());
    while let Some(envelope) = market_rx.recv().await {
        let TradingEvent::MarketDataReceived { market_data } = envelope.event else {
            continue;
        };

        // Stateful strategy arrives in Phase 3; here we only prove event flow.
        let intent = TradeIntent::builder(
            TradeIntentId::new("ti-async-001").unwrap(),
            StrategyId::new("btc-mean-reversion").unwrap(),
            StrategyVersion::new("v1").unwrap(),
            DeploymentId::new("deployment-001").unwrap(),
            market_data.instrument_id.clone(),
            OrderSide::Buy,
            Utc::now(),
        )
        .entry_price(market_data.price)
        .unwrap()
        .build()
        .unwrap();

        intent_tx
            .publish(TradingEvent::StrategySignalGenerated {
                strategy_id: intent.strategy_id.clone(),
                strategy_version: intent.strategy_version.clone(),
                deployment_id: intent.deployment_id.clone(),
                instrument_id: intent.instrument_id.clone(),
                side: intent.side,
                price: market_data.price,
            })
            .await
            .unwrap();

        intent_tx
            .publish(TradingEvent::TradeIntentCreated { intent })
            .await
            .unwrap();
        break;
    }
}

#[tokio::test]
async fn market_data_flows_to_trade_intent() {
    let (md_pub, md_sub) = event_channel(16);
    let (intent_pub, mut intent_sub) = event_channel(16);
    let (ready_tx, ready_rx) = oneshot::channel();

    tokio::spawn(strategy_task(md_sub, intent_pub, ready_tx));
    ready_rx.await.unwrap();

    let md = MarketDataEvent::new(
        InstrumentId::new("BTCUSDT").unwrap(),
        Decimal::new(90123, 0),
        "binance",
        Utc::now(),
        Utc::now(),
    )
    .unwrap();

    md_pub
        .publish(TradingEvent::MarketDataReceived { market_data: md })
        .await
        .unwrap();

    let signal = intent_sub.recv().await.expect("signal event");
    assert_eq!(signal.event.name(), "StrategySignalGenerated");

    let intent_env = intent_sub.recv().await.expect("intent event");
    assert_eq!(intent_env.event.name(), "TradeIntentCreated");
    assert_eq!(
        intent_env.correlation_trade_intent_id().unwrap().as_str(),
        "ti-async-001"
    );

    match intent_env.event {
        TradingEvent::TradeIntentCreated { intent } => {
            assert_eq!(intent.instrument_id.as_str(), "BTCUSDT");
            assert_eq!(intent.side, OrderSide::Buy);
            assert_eq!(intent.entry_price, Some(Decimal::new(90123, 0)));
            assert!(intent.target_quantity.is_none());
        }
        other => panic!("unexpected event: {}", other.name()),
    }
}

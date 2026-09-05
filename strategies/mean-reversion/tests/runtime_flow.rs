//! End-to-end: market data events → mean reversion → TradeIntent events.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use chrono::Utc;
use domain::{DeploymentId, InstrumentId, OrderSide, StrategyId, StrategyVersion};
use events::{event_channel, MarketDataEvent, TradingEvent};
use mean_reversion::{MeanReversionConfig, MeanReversionStrategy};
use rust_decimal::Decimal;
use strategy_runtime::{process_market_envelope, StrategyContext};

#[tokio::test]
async fn market_data_to_trade_intent_via_runtime() {
    let (md_pub, mut md_sub) = event_channel(32);
    let (intent_pub, mut intent_sub) = event_channel(32);

    let config = MeanReversionConfig {
        strategy_id: StrategyId::new("btc-mean-reversion").unwrap(),
        strategy_version: StrategyVersion::new("v1").unwrap(),
        instrument_id: InstrumentId::new("BTCUSDT").unwrap(),
        threshold: Decimal::new(5, 2),
        window_size: 3,
        stop_distance_pct: Some(Decimal::new(2, 2)),
    };
    let mut strategy = MeanReversionStrategy::new(config).unwrap();
    let ctx = StrategyContext::new(
        StrategyId::new("btc-mean-reversion").unwrap(),
        StrategyVersion::new("v1").unwrap(),
        DeploymentId::new("deployment-001").unwrap(),
    );

    let prices = [100_i64, 100, 100, 90];
    for price in prices {
        let md = MarketDataEvent::new(
            InstrumentId::new("BTCUSDT").unwrap(),
            Decimal::from(price),
            "binance",
            Utc::now(),
            Utc::now(),
        )
        .unwrap();
        md_pub
            .publish(TradingEvent::MarketDataReceived { market_data: md })
            .await
            .unwrap();
    }

    let mut saw_intent = false;
    for _ in 0..4 {
        let env = md_sub.recv().await.expect("md");
        let intents = process_market_envelope(&mut strategy, &ctx, &env, &intent_pub)
            .await
            .unwrap();
        if !intents.is_empty() {
            saw_intent = true;
            assert_eq!(intents[0].side, OrderSide::Buy);
        }
    }
    assert!(saw_intent);

    let signal = intent_sub.recv().await.expect("signal");
    assert_eq!(signal.event.name(), "StrategySignalGenerated");
    let intent_env = intent_sub.recv().await.expect("intent");
    assert_eq!(intent_env.event.name(), "TradeIntentCreated");
}

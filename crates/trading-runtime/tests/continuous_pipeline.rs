//! Phase 9 acceptance: continuous MarketData → … → Position over the event bus.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use account_engine::AccountEngine;
use chrono::Utc;
use domain::{
    AccountId, DeploymentId, InstrumentId, OrderSide, OrderStatus, OrderType, RiskDecision,
    RiskPolicy, StrategyId, StrategyVersion, TradeIntent, TradeIntentId, VenueId,
};
use events::{event_channel, MarketDataEvent, TradingEvent};
use mean_reversion::{MeanReversionConfig, MeanReversionStrategy};
use risk_engine::{InstrumentSizingContext, LossModel};
use rust_decimal::Decimal;
use strategy_runtime::StrategyContext;
use trading_runtime::{RuntimeConfig, RuntimeInstrumentSizingContext, TradingRuntime};
use venue_connectors::{SimulatedVenue, VenueAdapter};

#[tokio::test]
async fn continuous_market_data_to_position() {
    let venue_id = VenueId::new("simulated").unwrap();
    let venue = SimulatedVenue::new(venue_id.clone());
    let account_id = AccountId::new("prop-account-001").unwrap();

    let policy = RiskPolicy::try_new(
        Decimal::from(2),
        Decimal::from(10),
        Decimal::from(50),
        Decimal::from(2),
        Decimal::from(5),
        Decimal::from(10),
        5,
    )
    .unwrap();

    let config =
        RuntimeConfig::new(account_id.clone(), venue_id, policy).with_order_type(OrderType::Market);

    let mut runtime = TradingRuntime::new(config);
    runtime
        .bootstrap_prop_on_simulated(&venue, Decimal::from(100_000))
        .await
        .unwrap();

    let strategy_cfg = MeanReversionConfig {
        strategy_id: StrategyId::new("btc-mean-reversion").unwrap(),
        strategy_version: StrategyVersion::new("v1").unwrap(),
        instrument_id: InstrumentId::new("BTCUSDT").unwrap(),
        threshold: Decimal::new(5, 2),
        window_size: 3,
        stop_distance_pct: Some(Decimal::new(2, 2)),
    };
    let mut strategy = MeanReversionStrategy::new(strategy_cfg).unwrap();
    let ctx = StrategyContext::new(
        StrategyId::new("btc-mean-reversion").unwrap(),
        StrategyVersion::new("v1").unwrap(),
        DeploymentId::new("deployment-001").unwrap(),
    );

    let (md_pub, mut md_sub) = event_channel(64);
    let (events_out, mut events_sub) = event_channel(256);

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
    drop(md_pub);

    let ticks = runtime
        .run_n(&mut md_sub, &mut strategy, &ctx, &venue, &events_out, 4)
        .await
        .unwrap();

    let saw_execution = ticks.iter().any(|t| {
        t.executions
            .iter()
            .any(|e| e.order_status == Some(OrderStatus::Filled))
    });
    assert!(
        saw_execution,
        "expected at least one filled order from the loop"
    );

    let positions = venue.positions(&account_id).await.unwrap();
    assert_eq!(positions.len(), 1);
    assert!(positions[0].quantity > Decimal::ZERO);

    assert_eq!(
        runtime
            .accounts()
            .get(&account_id)
            .unwrap()
            .state
            .open_position_count,
        1
    );

    // Drain outbound pipeline events and assert vocabulary coverage.
    drop(events_out);
    let mut names = Vec::new();
    while let Some(env) = events_sub.recv().await {
        names.push(env.event.name().to_string());
    }

    for required in [
        "StrategySignalGenerated",
        "TradeIntentCreated",
        "RiskCheckRequested",
        "RiskApproved",
        "OrderCreated",
        "OrderFilled",
        "PositionOpened",
        "AccountUpdated",
    ] {
        assert!(
            names.iter().any(|n| n == required),
            "missing outbound event {required}; saw {names:?}"
        );
    }
}

#[tokio::test]
async fn run_until_stops_after_fill() {
    let venue_id = VenueId::new("simulated").unwrap();
    let venue = SimulatedVenue::new(venue_id.clone());
    let account_id = AccountId::new("prop-account-002").unwrap();
    let policy = RiskPolicy::try_new(
        Decimal::from(2),
        Decimal::from(10),
        Decimal::from(50),
        Decimal::from(2),
        Decimal::from(5),
        Decimal::from(10),
        5,
    )
    .unwrap();

    let mut runtime = TradingRuntime::new(RuntimeConfig::new(account_id.clone(), venue_id, policy));
    runtime
        .bootstrap_prop_on_simulated(&venue, Decimal::from(100_000))
        .await
        .unwrap();

    let mut strategy = MeanReversionStrategy::new(MeanReversionConfig {
        strategy_id: StrategyId::new("btc-mean-reversion").unwrap(),
        strategy_version: StrategyVersion::new("v1").unwrap(),
        instrument_id: InstrumentId::new("BTCUSDT").unwrap(),
        threshold: Decimal::new(5, 2),
        window_size: 3,
        stop_distance_pct: None,
    })
    .unwrap();
    let ctx = StrategyContext::new(
        StrategyId::new("btc-mean-reversion").unwrap(),
        StrategyVersion::new("v1").unwrap(),
        DeploymentId::new("deployment-002").unwrap(),
    );

    let (md_pub, mut md_sub) = event_channel(32);
    let (events_out, _events_sub) = event_channel(128);

    for price in [100_i64, 100, 100, 90, 90] {
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

    let ticks = runtime
        .run_until(
            &mut md_sub,
            &mut strategy,
            &ctx,
            &venue,
            &events_out,
            |tick| {
                tick.executions
                    .iter()
                    .any(|e| e.order_status == Some(OrderStatus::Filled))
            },
        )
        .await
        .unwrap();

    assert!(ticks.len() <= 5);
    assert!(ticks
        .last()
        .unwrap()
        .executions
        .iter()
        .any(|e| { e.order_status == Some(OrderStatus::Filled) }));
}

#[tokio::test]
async fn instrument_aware_runtime_rejects_stale_sizing_before_oms() {
    let venue_id = VenueId::new("simulated").unwrap();
    let venue = SimulatedVenue::new(venue_id.clone());
    let account_id = AccountId::new("sizing-stale-account").unwrap();
    let policy = RiskPolicy::try_new(
        Decimal::from(2),
        Decimal::from(10),
        Decimal::from(50),
        Decimal::from(2),
        Decimal::from(5),
        Decimal::from(10),
        5,
    )
    .unwrap();
    let mut runtime = TradingRuntime::new(RuntimeConfig::new(account_id.clone(), venue_id, policy));
    runtime
        .bootstrap_prop_on_simulated(&venue, Decimal::from(100_000))
        .await
        .unwrap();

    let intent = TradeIntent::builder(
        TradeIntentId::new("ti-stale-sizing").unwrap(),
        StrategyId::new("test-strategy").unwrap(),
        StrategyVersion::new("v1").unwrap(),
        DeploymentId::new("deployment-stale-sizing").unwrap(),
        InstrumentId::new("BTCUSDT").unwrap(),
        OrderSide::Buy,
        Utc::now(),
    )
    .entry_price(Decimal::from(100))
    .unwrap()
    .stop_loss(Decimal::from(90))
    .unwrap()
    .build()
    .unwrap();
    let context = RuntimeInstrumentSizingContext {
        account_id: account_id.clone(),
        instrument_id: intent.instrument_id.clone(),
        source_id: "test-fixture".to_string(),
        captured_at: Utc::now(),
        max_age_seconds: 30,
        sizing: InstrumentSizingContext {
            loss_model: LossModel::Linear {
                contract_multiplier: Decimal::ONE,
                pnl_to_account_rate: Decimal::ONE,
            },
            inputs_fresh: false,
            notional_to_account_rate: Decimal::ONE,
            quantity_step: Decimal::ONE,
            min_quantity: Decimal::ONE,
            max_quantity: Some(Decimal::from(100)),
            min_notional_account_currency: None,
            estimated_cost_per_quantity: Decimal::ZERO,
            slippage_gap_buffer_per_quantity: Decimal::ZERO,
            pending_order_exposure: Decimal::ZERO,
            available_margin: None,
            pending_order_margin: Decimal::ZERO,
            margin_per_quantity: None,
        },
    };
    let (events_out, _events_in) = event_channel(32);

    let outcome = runtime
        .execute_intent_with_instrument_sizing(&intent, &venue, &events_out, &context)
        .await
        .unwrap();

    assert_eq!(
        outcome.decision,
        RiskDecision::Rejected {
            reason: domain::RiskRejectReason::DependencyUnavailable,
        }
    );
    assert_eq!(outcome.order_status, None);
    assert!(runtime.oms().open_orders_for(&account_id).is_empty());

    let mut mismatched = context.clone();
    mismatched.instrument_id = InstrumentId::new("ETHUSDT").unwrap();
    let mismatch_outcome = runtime
        .execute_intent_with_instrument_sizing(&intent, &venue, &events_out, &mismatched)
        .await
        .unwrap();
    assert_eq!(
        mismatch_outcome.decision,
        RiskDecision::Rejected {
            reason: domain::RiskRejectReason::DependencyUnavailable,
        }
    );
    assert_eq!(mismatch_outcome.order_status, None);
    assert!(runtime.oms().open_orders_for(&account_id).is_empty());
}

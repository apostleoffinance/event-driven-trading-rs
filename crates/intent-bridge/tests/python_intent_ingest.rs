//! Wire format + ingest tests (Python JSON → Rust pipeline).

#![allow(clippy::unwrap_used, clippy::expect_used)]

use domain::{AccountId, OrderStatus};
use intent_bridge::{parse_intent_json, BridgeVenueKind, IntentIngestSession, TradeIntentWire};
use rust_decimal::Decimal;

#[test]
fn python_shaped_json_parses_to_domain() {
    let json = r#"{
      "id": "ti-py-001",
      "strategy_id": "btc-mean-reversion",
      "strategy_version": "v1",
      "deployment_id": "deployment-001",
      "instrument_id": "BTCUSDT",
      "side": "Buy",
      "entry_price": "90.00",
      "stop_loss": "88.20",
      "confidence": "0.5",
      "timestamp": "2026-03-06T12:00:00Z"
    }"#;

    let intent = parse_intent_json(json).unwrap();
    assert_eq!(intent.id.as_str(), "ti-py-001");
    assert_eq!(intent.side, domain::OrderSide::Buy);
    assert_eq!(intent.entry_price.unwrap().to_string(), "90.00");
    assert!(intent.target_quantity.is_none());
}

#[test]
fn wire_round_trip() {
    let json = r#"{"id":"ti-1","strategy_id":"s","strategy_version":"v1","deployment_id":"d","instrument_id":"ES","side":"Sell","entry_price":"5000","timestamp":"2026-03-06T12:00:00+00:00"}"#;
    let intent = parse_intent_json(json).unwrap();
    let wire = TradeIntentWire::from_domain(&intent);
    let back = wire.into_domain().unwrap();
    assert_eq!(intent.id, back.id);
    assert_eq!(intent.side, back.side);
}

#[tokio::test]
async fn python_intent_executes_on_prop_venue() {
    let policy = domain::RiskPolicy::try_new(
        Decimal::from(2),
        Decimal::from(10),
        Decimal::from(50),
        Decimal::from(2),
        Decimal::from(5),
        Decimal::from(10),
        5,
    )
    .unwrap();

    let mut session = IntentIngestSession::bootstrap(
        AccountId::new("prop-account-py").unwrap(),
        BridgeVenueKind::Prop,
        Decimal::from(100_000),
        policy,
    )
    .await
    .unwrap();

    let json = r#"{
      "id": "ti-py-exec-1",
      "strategy_id": "es-mean-reversion",
      "strategy_version": "v1",
      "deployment_id": "deployment-py",
      "instrument_id": "ES",
      "side": "Buy",
      "entry_price": "5000",
      "timestamp": "2026-03-06T12:00:00Z"
    }"#;

    let outcome = session.ingest_json(json).await.unwrap();
    assert!(outcome.decision.is_executable());
    assert_eq!(outcome.order_status, Some(OrderStatus::Filled));
}

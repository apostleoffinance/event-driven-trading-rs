//! Phase 11: TradeIntent → Risk → OMS → PropVenue → Fill → Position.

#![allow(clippy::unwrap_used)]

use account_engine::{AccountEngine, InMemoryAccountEngine};
use chrono::Utc;
use domain::{
    AccountId, ClientOrderId, DeploymentId, InstrumentId, OrderSide, OrderStatus, OrderType,
    StrategyId, StrategyVersion, TimeInForce, TradeIntent, TradeIntentId, VenueId,
};
use execution_engine::{ExecutionEngine, NewOrderRequest};
use risk_engine::{DefaultRiskEvaluator, RiskEvaluator};
use rust_decimal::Decimal;
use venue_connectors::{submit_approved_order, PropVenue, VenueAdapter};

#[tokio::test]
async fn intent_risk_oms_prop_venue_fill_position() {
    let venue = PropVenue::paper().unwrap();
    assert_eq!(VenueAdapter::venue_name(&venue), "prop");
    assert!(venue.is_paper());

    let mut accounts = InMemoryAccountEngine::new();
    let account_id = AccountId::new("prop-account-001").unwrap();
    accounts
        .open_prop(account_id.clone(), Decimal::from(100_000), Utc::now())
        .unwrap();
    assert_eq!(
        accounts.get(&account_id).unwrap().account.venue_id.as_str(),
        "prop"
    );

    let account_state = accounts.get(&account_id).unwrap().state.clone();
    venue.register_account(account_state.clone()).await.unwrap();

    let intent = TradeIntent::builder(
        TradeIntentId::new("ti-phase11").unwrap(),
        StrategyId::new("es-mean-reversion").unwrap(),
        StrategyVersion::new("v1").unwrap(),
        DeploymentId::new("deployment-prop").unwrap(),
        InstrumentId::new("ES").unwrap(),
        OrderSide::Buy,
        Utc::now(),
    )
    .entry_price(Decimal::from(5000))
    .unwrap()
    .target_quantity(Decimal::from(2))
    .unwrap()
    .build()
    .unwrap();

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

    let risk_req = domain::RiskRequest::from_intent(
        &intent,
        account_id.clone(),
        account_state,
        policy,
        Utc::now(),
    );
    let decision = DefaultRiskEvaluator::new().evaluate(&risk_req).unwrap();
    assert!(decision.is_executable());
    let quantity = decision.approved_quantity().unwrap();

    let mut oms = ExecutionEngine::new();
    let venue_id = VenueId::new("prop").unwrap();
    let outcome = submit_approved_order(
        &mut oms,
        &venue,
        NewOrderRequest {
            client_order_id: ClientOrderId::new("clid-phase11").unwrap(),
            account_id: account_id.clone(),
            venue_id,
            deployment_id: intent.deployment_id.clone(),
            strategy_id: intent.strategy_id.clone(),
            trade_intent_id: intent.id.clone(),
            instrument_id: intent.instrument_id.clone(),
            side: intent.side,
            order_type: OrderType::Market,
            time_in_force: TimeInForce::Ioc,
            price: intent.entry_price,
            risk_decision: decision,
            instrument_spec: Some(domain::InstrumentSpec::crypto_spot_default().unwrap()),
            created_at: Utc::now(),
        },
    )
    .await
    .unwrap();

    assert_eq!(outcome.order.status, OrderStatus::Filled);
    assert_eq!(outcome.order.filled_quantity, quantity);
    // Prop venue uses a single full fill (vs SimulatedVenue's split fills).
    assert_eq!(oms.fills_for(&outcome.order.id).len(), 1);

    let positions = venue.positions(&account_id).await.unwrap();
    assert_eq!(positions.len(), 1);
    assert_eq!(positions[0].quantity, quantity);

    accounts
        .set_open_position_count(&account_id, positions.len() as u32, Utc::now())
        .unwrap();
    assert_eq!(
        accounts.get(&account_id).unwrap().state.open_position_count,
        1
    );
}

#[tokio::test]
async fn live_prop_venue_rejects_until_wired() {
    let config = venue_connectors::PropVenueConfig::paper()
        .unwrap()
        .live("https://example-prop.invalid/v1");
    let venue = PropVenue::new(config);
    let err = venue
        .positions(&AccountId::new("any").unwrap())
        .await
        .unwrap_err();
    assert!(matches!(
        err,
        venue_connectors::VenueError::NotImplemented(_)
    ));
}

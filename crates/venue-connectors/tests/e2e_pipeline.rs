//! Phase 7 acceptance: TradeIntent → Risk → OMS → SimulatedVenue → Fill → Position.

#![allow(clippy::unwrap_used)]

use account_engine::{AccountEngine, InMemoryAccountEngine, SimulatedAccountSpec};
use chrono::Utc;
use domain::{
    AccountId, ClientOrderId, DeploymentId, InstrumentId, OrderSide, OrderStatus, OrderType,
    StrategyId, StrategyVersion, TimeInForce, TradeIntent, TradeIntentId, VenueId,
};
use execution_engine::{ExecutionEngine, NewOrderRequest};
use risk_engine::{DefaultRiskEvaluator, RiskEvaluator};
use rust_decimal::Decimal;
use venue_connectors::{submit_approved_order, SimulatedVenue, VenueAdapter};

#[tokio::test]
async fn intent_risk_oms_venue_fill_position() {
    let venue_id = VenueId::new("simulated").unwrap();
    let venue = SimulatedVenue::new(venue_id.clone());

    let mut accounts = InMemoryAccountEngine::new();
    let account_id = AccountId::new("prop-account-001").unwrap();
    accounts
        .open_prop_on_simulated(account_id.clone(), Decimal::from(100_000), Utc::now())
        .unwrap();
    let account_state = accounts.get(&account_id).unwrap().state.clone();
    venue.register_account(account_state.clone()).await.unwrap();

    let intent = TradeIntent::builder(
        TradeIntentId::new("ti-phase7").unwrap(),
        StrategyId::new("btc-mean-reversion").unwrap(),
        StrategyVersion::new("v1").unwrap(),
        DeploymentId::new("deployment-001").unwrap(),
        InstrumentId::new("BTCUSDT").unwrap(),
        OrderSide::Buy,
        Utc::now(),
    )
    .entry_price(Decimal::from(100))
    .unwrap()
    .stop_loss(Decimal::from(98))
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
    let outcome = submit_approved_order(
        &mut oms,
        &venue,
        NewOrderRequest {
            client_order_id: ClientOrderId::new("clid-phase7").unwrap(),
            account_id: account_id.clone(),
            venue_id: venue_id.clone(),
            deployment_id: intent.deployment_id.clone(),
            strategy_id: intent.strategy_id.clone(),
            trade_intent_id: intent.id.clone(),
            instrument_id: intent.instrument_id.clone(),
            side: intent.side,
            order_type: OrderType::Market,
            time_in_force: TimeInForce::Ioc,
            price: intent.entry_price,
            risk_decision: decision,
            created_at: Utc::now(),
        },
    )
    .await
    .unwrap();

    assert_eq!(outcome.order.status, OrderStatus::Filled);
    assert_eq!(outcome.order.filled_quantity, quantity);
    assert!(!oms.fills_for(&outcome.order.id).is_empty());
    assert!(!oms.audit_for_order(&outcome.order.id).is_empty());

    let positions = venue.positions(&account_id).await.unwrap();
    assert_eq!(positions.len(), 1);
    assert_eq!(positions[0].quantity, quantity);
    assert!(positions[0].unrealized_pnl().abs() >= Decimal::ZERO);

    // Execution path is venue-agnostic at the call site (trait object).
    let _name = VenueAdapter::venue_name(&venue);
    assert_eq!(_name, "simulated");

    // Sync account open positions from venue state.
    accounts
        .set_open_position_count(&account_id, positions.len() as u32, Utc::now())
        .unwrap();
    assert_eq!(
        accounts.get(&account_id).unwrap().state.open_position_count,
        1
    );
}

#[tokio::test]
async fn simulated_account_helper_wires_venue() {
    let mut accounts = InMemoryAccountEngine::new();
    let id = AccountId::new("sim-p7").unwrap();
    accounts
        .open_simulated(SimulatedAccountSpec {
            account_id: id.clone(),
            starting_capital: Decimal::from(25_000),
            label: Some("paper".into()),
            created_at: Utc::now(),
        })
        .unwrap();
    assert_eq!(
        accounts.get(&id).unwrap().account.venue_id.as_str(),
        "simulated"
    );
}

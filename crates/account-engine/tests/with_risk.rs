//! Simulated account state feeds risk evaluation.

#![allow(clippy::unwrap_used)]

use account_engine::{AccountEngine, InMemoryAccountEngine, SimulatedAccountSpec};
use chrono::Utc;
use domain::{
    AccountId, DeploymentId, InstrumentId, OrderSide, RiskPolicy, StrategyId, TradeIntentId,
};
use risk_engine::{DefaultRiskEvaluator, RiskEvaluator};
use rust_decimal::Decimal;

#[test]
fn simulated_account_state_used_by_risk_engine() {
    let mut accounts = InMemoryAccountEngine::new();
    let account_id = AccountId::new("prop-account-001").unwrap();
    accounts
        .open_prop_on_simulated(account_id.clone(), Decimal::from(100_000), Utc::now())
        .unwrap();

    let record = accounts.get(&account_id).unwrap();
    assert_eq!(record.account.venue_id.as_str(), "simulated");
    accounts.ensure_tradable(&account_id).unwrap();

    let policy = RiskPolicy::try_new(
        Decimal::from(2),
        Decimal::from(5),
        Decimal::from(50),
        Decimal::new(15, 1),
        Decimal::from(5),
        Decimal::from(10),
        5,
    )
    .unwrap();

    let request = domain::RiskRequest {
        trade_intent_id: TradeIntentId::new("ti-1").unwrap(),
        strategy_id: StrategyId::new("btc-mean-reversion").unwrap(),
        deployment_id: DeploymentId::new("deployment-001").unwrap(),
        account_id: account_id.clone(),
        instrument_id: InstrumentId::new("BTCUSDT").unwrap(),
        side: OrderSide::Buy,
        requested_quantity: Some(Decimal::ONE),
        requested_notional: None,
        entry_price: Some(Decimal::from(100)),
        stop_loss: Some(Decimal::from(98)),
        account_state: record.state.clone(),
        policy,
        current_exposure: Decimal::ZERO,
        session_allowed: true,
        requested_at: Utc::now(),
    };

    let decision = DefaultRiskEvaluator::new().evaluate(&request).unwrap();
    assert!(decision.is_executable());
}

#[test]
fn kill_switch_from_account_engine_halts_tradability() {
    let mut accounts = InMemoryAccountEngine::new();
    let account_id = AccountId::new("sim-ks").unwrap();
    accounts
        .open_simulated(SimulatedAccountSpec {
            account_id: account_id.clone(),
            starting_capital: Decimal::from(25_000),
            label: Some("paper".into()),
            created_at: Utc::now(),
        })
        .unwrap();
    accounts
        .activate_kill_switch(&account_id, Utc::now())
        .unwrap();
    assert!(accounts.ensure_tradable(&account_id).is_err());
}

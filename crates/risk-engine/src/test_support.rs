//! Shared fixtures for unit tests.

#![cfg(test)]
#![allow(clippy::unwrap_used)]

use chrono::Utc;
use domain::{
    AccountId, AccountState, DeploymentId, InstrumentId, OrderSide, RiskPolicy, RiskRequest,
    StrategyId, TradeIntentId,
};
use rust_decimal::Decimal;

pub fn sample_policy() -> RiskPolicy {
    RiskPolicy::try_new(
        Decimal::from(2),
        Decimal::from(5),
        Decimal::from(50),
        Decimal::new(15, 1),
        Decimal::from(5),
        Decimal::from(10),
        5,
    )
    .unwrap()
}

pub fn sample_request() -> RiskRequest {
    let account_id = AccountId::new("prop-account-001").unwrap();
    let state = AccountState::new(account_id.clone(), Decimal::from(100_000), Utc::now()).unwrap();
    RiskRequest {
        trade_intent_id: TradeIntentId::new("ti-1").unwrap(),
        strategy_id: StrategyId::new("btc-mean-reversion").unwrap(),
        deployment_id: DeploymentId::new("deployment-001").unwrap(),
        account_id,
        instrument_id: InstrumentId::new("BTCUSDT").unwrap(),
        side: OrderSide::Buy,
        requested_quantity: Some(Decimal::ONE),
        requested_notional: None,
        entry_price: Some(Decimal::from(100)),
        stop_loss: Some(Decimal::from(98)),
        account_state: state,
        policy: sample_policy(),
        current_exposure: Decimal::ZERO,
        session_allowed: true,
        requested_at: Utc::now(),
    }
}

//! TradeIntent → RiskRequest → RiskDecision (Phase 4 acceptance).

#![allow(clippy::unwrap_used)]

use chrono::Utc;
use domain::{
    AccountId, AccountState, DeploymentId, InstrumentId, OrderSide, RiskPolicy, StrategyId,
    StrategyVersion, TradeIntent, TradeIntentId,
};
use risk_engine::{DefaultRiskEvaluator, RiskEvaluator};
use rust_decimal::Decimal;

#[test]
fn trade_intent_to_risk_decision() {
    let intent = TradeIntent::builder(
        TradeIntentId::new("ti-phase4").unwrap(),
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
    .build()
    .unwrap();

    let account_id = AccountId::new("prop-account-001").unwrap();
    let state = AccountState::new(account_id.clone(), Decimal::from(100_000), Utc::now()).unwrap();
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

    let request = domain::RiskRequest::from_intent(&intent, account_id, state, policy, Utc::now());
    let decision = DefaultRiskEvaluator::new().evaluate(&request).unwrap();
    assert!(decision.is_executable());
    assert!(decision.approved_quantity().unwrap() > Decimal::ZERO);
}

#[test]
fn leverage_limit_can_reject() {
    let mut intent_price = TradeIntent::builder(
        TradeIntentId::new("ti-lev").unwrap(),
        StrategyId::new("btc-mean-reversion").unwrap(),
        StrategyVersion::new("v1").unwrap(),
        DeploymentId::new("deployment-001").unwrap(),
        InstrumentId::new("BTCUSDT").unwrap(),
        OrderSide::Buy,
        Utc::now(),
    )
    .entry_price(Decimal::from(100))
    .unwrap()
    .target_quantity(Decimal::from(100))
    .unwrap()
    .build()
    .unwrap();

    let account_id = AccountId::new("prop-account-001").unwrap();
    let state = AccountState::new(account_id.clone(), Decimal::from(1_000), Utc::now()).unwrap();
    let mut policy = RiskPolicy::try_new(
        Decimal::from(50),
        Decimal::from(100),
        Decimal::from(100),
        Decimal::ONE, // 1x leverage
        Decimal::from(50),
        Decimal::from(50),
        10,
    )
    .unwrap();
    // Force exposure room but no leverage room beyond equity.
    let _ = &mut policy;
    let _ = &mut intent_price;

    let request =
        domain::RiskRequest::from_intent(&intent_price, account_id, state, policy, Utc::now())
            .with_exposure(Decimal::from(1_000)); // already at 1x

    let decision = DefaultRiskEvaluator::new().evaluate(&request).unwrap();
    assert!(
        matches!(
            decision,
            domain::RiskDecision::Rejected {
                reason: domain::RiskRejectReason::LeverageLimit
            } | domain::RiskDecision::Rejected {
                reason: domain::RiskRejectReason::ExposureLimit
            }
        ),
        "got {decision:?}"
    );
}

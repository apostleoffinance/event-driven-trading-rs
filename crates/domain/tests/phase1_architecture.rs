//! Phase 1 architectural acceptance: domain can represent the target flow.

#![allow(clippy::unwrap_used)]

use chrono::Utc;
use domain::{
    Account, AccountState, AccountType, Instrument, OrderSide, RiskDecision, RiskPolicy,
    RiskProfile, RiskRejectReason, RiskRequest, StrategyDeployment, StrategyMetadata, TradeIntent,
    Venue, VenueType,
};
use domain::{
    AccountId, DeploymentId, InstrumentId, RiskProfileId, StrategyId, StrategyVersion,
    TradeIntentId, VenueId,
};
use rust_decimal::Decimal;

#[test]
fn phase1_architecture_can_represent_prop_deployment_flow() {
    let now = Utc::now();

    let strategy = StrategyMetadata::new(
        StrategyId::new("btc-mean-reversion").unwrap(),
        StrategyVersion::new("v1").unwrap(),
        "BTC Mean Reversion v1",
    )
    .unwrap();

    let venue = Venue::new(
        VenueId::new("simulated").unwrap(),
        VenueType::Simulated,
        "Simulated Venue",
        now,
    );

    let account = Account::new(
        AccountId::new("prop-account-001").unwrap(),
        AccountType::Prop,
        venue.id.clone(),
        now,
    );

    let policy = RiskPolicy::try_new(
        Decimal::from(2),    // max trade risk %
        Decimal::from(5),    // max position size %
        Decimal::from(50),   // max total exposure %
        Decimal::new(15, 1), // 1.5x leverage
        Decimal::from(5),    // max daily loss %
        Decimal::from(10),   // max drawdown %
        3,
    )
    .unwrap();

    let risk_profile = RiskProfile {
        id: RiskProfileId::new("prop-conservative").unwrap(),
        name: "Prop Conservative".to_string(),
        policy: policy.clone(),
    };

    let deployment = StrategyDeployment::new(
        DeploymentId::new("deployment-001").unwrap(),
        strategy.id.clone(),
        strategy.version.clone(),
        account.id.clone(),
        risk_profile.id.clone(),
        now,
    );

    let instrument = Instrument::crypto_spot(
        InstrumentId::new("BTCUSDT").unwrap(),
        "BTCUSDT",
        "BTC",
        "USDT",
    )
    .unwrap();

    let intent = TradeIntent::builder(
        TradeIntentId::new("ti-001").unwrap(),
        strategy.id.clone(),
        strategy.version.clone(),
        deployment.id.clone(),
        instrument.id.clone(),
        OrderSide::Buy,
        now,
    )
    .entry_price(Decimal::new(90000, 0))
    .unwrap()
    .stop_loss(Decimal::new(88200, 0))
    .unwrap()
    .confidence(Decimal::new(75, 2))
    .unwrap()
    .build()
    .unwrap();

    let account_state = AccountState::new(account.id.clone(), Decimal::from(100_000), now).unwrap();

    let request = RiskRequest::from_intent(
        &intent,
        account.id.clone(),
        account_state,
        risk_profile.policy,
        now,
    );

    // Domain can express decisions; evaluation engine arrives in Phase 4.
    let approved = RiskDecision::Approved {
        quantity: Decimal::from(1),
    };
    let rejected = RiskDecision::Rejected {
        reason: RiskRejectReason::TradeRiskLimit,
    };

    assert_eq!(deployment.account_id.as_str(), "prop-account-001");
    assert_eq!(account.venue_id.as_str(), "simulated");
    assert_eq!(request.deployment_id.as_str(), "deployment-001");
    assert_eq!(request.trade_intent_id, intent.id);
    assert!(approved.is_executable());
    assert!(!rejected.is_executable());
    assert!(intent.target_quantity.is_none()); // strategy did not size the account
}

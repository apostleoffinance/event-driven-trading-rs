//! Invariant tests for OMS / execution engine.

#![allow(clippy::unwrap_used)]

use chrono::Utc;
use domain::{
    AccountId, ClientOrderId, DeploymentId, InstrumentId, OrderSide, OrderStatus, OrderType,
    RiskDecision, RiskRejectReason, StrategyId, TimeInForce, TradeIntentId, VenueId,
};
use execution_engine::{AuditKind, ExecutionEngine, ExecutionError, NewOrderRequest};
use rust_decimal::Decimal;

fn req(client: &str, decision: RiskDecision) -> NewOrderRequest {
    NewOrderRequest {
        client_order_id: ClientOrderId::new(client).unwrap(),
        account_id: AccountId::new("prop-account-001").unwrap(),
        venue_id: VenueId::new("simulated").unwrap(),
        deployment_id: DeploymentId::new("deployment-001").unwrap(),
        strategy_id: StrategyId::new("btc-mean-reversion").unwrap(),
        trade_intent_id: TradeIntentId::new("ti-inv").unwrap(),
        instrument_id: InstrumentId::new("BTCUSDT").unwrap(),
        side: OrderSide::Buy,
        order_type: OrderType::Market,
        time_in_force: TimeInForce::Gtc,
        price: Some(Decimal::from(100)),
        risk_decision: decision,
        created_at: Utc::now(),
    }
}

#[test]
fn invariant_no_order_without_risk_decision() {
    let mut oms = ExecutionEngine::new();
    let err = oms
        .create_order(req(
            "no-risk",
            RiskDecision::Rejected {
                reason: RiskRejectReason::TradeRiskLimit,
            },
        ))
        .unwrap_err();
    assert!(matches!(
        err,
        ExecutionError::MissingRiskDecision | ExecutionError::RiskNotExecutable(_)
    ));
}

#[test]
fn invariant_retry_same_client_id_same_order() {
    let mut oms = ExecutionEngine::new();
    let decision = RiskDecision::Approved {
        quantity: Decimal::from(3),
    };
    let a = oms.create_order(req("idem-1", decision.clone())).unwrap();
    let b = oms.create_order(req("idem-1", decision)).unwrap();
    assert_eq!(a.order.id, b.order.id);
    assert!(a.created);
    assert!(!b.created);
}

#[test]
fn invariant_idempotency_conflict_on_payload_mismatch() {
    let mut oms = ExecutionEngine::new();
    oms.create_order(req(
        "idem-2",
        RiskDecision::Approved {
            quantity: Decimal::ONE,
        },
    ))
    .unwrap();
    let err = oms
        .create_order(req(
            "idem-2",
            RiskDecision::Approved {
                quantity: Decimal::from(9),
            },
        ))
        .unwrap_err();
    assert!(matches!(err, ExecutionError::IdempotencyConflict { .. }));
}

#[test]
fn invariant_every_order_has_audit_trail() {
    let mut oms = ExecutionEngine::new();
    let outcome = oms
        .create_submit_accept(req(
            "audit-1",
            RiskDecision::Resized {
                quantity: Decimal::ONE,
                reason: "capped".into(),
            },
        ))
        .unwrap();
    let trail = oms.audit_for_order(&outcome.order.id);
    assert!(trail.iter().any(|e| e.kind == AuditKind::OrderCreated));
    assert!(trail.iter().any(|e| e.kind == AuditKind::RiskApproved));
    assert!(trail.iter().any(|e| e.kind == AuditKind::Submitted));
    assert!(trail.iter().any(|e| e.kind == AuditKind::Accepted));
    assert_eq!(outcome.order.status, OrderStatus::Accepted);
}

#[test]
fn invariant_order_requires_account_and_venue() {
    let mut oms = ExecutionEngine::new();
    let order = oms
        .create_order(req(
            "acct-venue",
            RiskDecision::Approved {
                quantity: Decimal::ONE,
            },
        ))
        .unwrap()
        .order;
    assert_eq!(order.account_id.as_str(), "prop-account-001");
    assert_eq!(order.venue_id.as_str(), "simulated");
    assert!(!order.client_order_id.as_str().is_empty());
}

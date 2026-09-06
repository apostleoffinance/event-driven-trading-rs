//! Ambiguous venue submit → OrderStatus::Unknown (never Failed).

use async_trait::async_trait;
use chrono::Utc;
use domain::{
    AccountId, AccountState, ClientOrderId, DeploymentId, InstrumentId, Order, OrderId, OrderSide,
    OrderStatus, OrderType, Position, RiskDecision, StrategyId, TimeInForce, TradeIntentId, VenueId,
};
use execution_engine::{ExecutionEngine, NewOrderRequest};
use rust_decimal::Decimal;
use venue_connectors::types::{CancelRequest, OrderAck, OrderRequest};
use venue_connectors::{submit_approved_order, VenueAdapter, VenueError, VenueResult};

struct AmbiguousVenue;

#[async_trait]
impl VenueAdapter for AmbiguousVenue {
    fn venue_name(&self) -> &str {
        "ambiguous-test"
    }

    async fn account_state(&self, _account_id: &AccountId) -> VenueResult<AccountState> {
        Err(VenueError::NotImplemented("n/a".into()))
    }

    async fn submit_order(&self, _request: OrderRequest) -> VenueResult<OrderAck> {
        Err(VenueError::Ambiguous(
            "timeout after submit; outcome unknown".into(),
        ))
    }

    async fn cancel_order(&self, _request: CancelRequest) -> VenueResult<()> {
        Err(VenueError::NotImplemented("n/a".into()))
    }

    async fn open_orders(&self, _account_id: &AccountId) -> VenueResult<Vec<Order>> {
        Ok(vec![])
    }

    async fn positions(&self, _account_id: &AccountId) -> VenueResult<Vec<Position>> {
        Ok(vec![])
    }

    async fn get_order(&self, _order_id: &OrderId) -> VenueResult<Order> {
        Err(VenueError::OrderNotFound("n/a".into()))
    }
}

#[tokio::test]
async fn ambiguous_submit_marks_unknown_not_failed() {
    let mut oms = ExecutionEngine::new();
    let venue = AmbiguousVenue;

    let outcome = submit_approved_order(
        &mut oms,
        &venue,
        NewOrderRequest {
            client_order_id: ClientOrderId::new("clid-unknown").unwrap(),
            account_id: AccountId::new("acct-1").unwrap(),
            venue_id: VenueId::new("ambiguous-test").unwrap(),
            deployment_id: DeploymentId::new("dep-1").unwrap(),
            strategy_id: StrategyId::new("s1").unwrap(),
            trade_intent_id: TradeIntentId::new("ti-unk").unwrap(),
            instrument_id: InstrumentId::new("BTCUSDT").unwrap(),
            side: OrderSide::Buy,
            order_type: OrderType::Market,
            time_in_force: TimeInForce::Ioc,
            price: None,
            risk_decision: RiskDecision::Approved {
                quantity: Decimal::ONE,
            },
            instrument_spec: None,
            created_at: Utc::now(),
        },
    )
    .await
    .unwrap();

    assert_eq!(outcome.order.status, OrderStatus::Unknown);
    assert!(!outcome.order.status.is_terminal());
}

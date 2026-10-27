//! Cancel request acknowledgement must not be treated as terminal cancellation.

#![allow(clippy::unwrap_used)]

use async_trait::async_trait;
use chrono::Utc;
use domain::{
    AccountId, AccountState, ClientOrderId, DeploymentId, InstrumentId, Order, OrderId, OrderSide,
    OrderStatus, OrderType, Position, RiskDecision, StrategyId, TimeInForce, TradeIntentId,
    VenueId,
};
use execution_engine::{ExecutionEngine, NewOrderRequest};
use rust_decimal::Decimal;
use venue_connectors::types::{CancelRequest, OrderAck, OrderRequest};
use venue_connectors::{cancel_order, VenueAdapter, VenueError, VenueResult};

struct CancelAckButStillOpenVenue {
    order: Order,
}

#[async_trait]
impl VenueAdapter for CancelAckButStillOpenVenue {
    fn venue_name(&self) -> &str {
        "cancel-ack-open-test"
    }

    async fn account_state(&self, _account_id: &AccountId) -> VenueResult<AccountState> {
        Err(VenueError::NotImplemented("not used by this test".into()))
    }

    async fn submit_order(&self, _request: OrderRequest) -> VenueResult<OrderAck> {
        Err(VenueError::NotImplemented("not used by this test".into()))
    }

    async fn cancel_order(&self, _request: CancelRequest) -> VenueResult<()> {
        // Simulate a venue acknowledging the cancel request while the order remains open.
        Ok(())
    }

    async fn open_orders(&self, _account_id: &AccountId) -> VenueResult<Vec<Order>> {
        Ok(vec![self.order.clone()])
    }

    async fn positions(&self, _account_id: &AccountId) -> VenueResult<Vec<Position>> {
        Ok(vec![])
    }

    async fn get_order(&self, _order_id: &OrderId) -> VenueResult<Order> {
        Ok(self.order.clone())
    }
}

#[tokio::test]
async fn cancel_ack_does_not_mark_order_cancelled_without_confirmation() {
    let now = Utc::now();
    let mut oms = ExecutionEngine::new();
    let outcome = oms
        .create_order(NewOrderRequest {
            client_order_id: ClientOrderId::new("clid-cancel-confirm").unwrap(),
            account_id: AccountId::new("acct-cancel-confirm").unwrap(),
            venue_id: VenueId::new("cancel-ack-open-test").unwrap(),
            deployment_id: DeploymentId::new("dep-cancel-confirm").unwrap(),
            strategy_id: StrategyId::new("strategy-cancel-confirm").unwrap(),
            trade_intent_id: TradeIntentId::new("intent-cancel-confirm").unwrap(),
            instrument_id: InstrumentId::new("EURUSD").unwrap(),
            side: OrderSide::Buy,
            order_type: OrderType::Limit,
            time_in_force: TimeInForce::Gtc,
            price: Some(Decimal::new(11000, 4)),
            risk_decision: RiskDecision::Approved {
                quantity: Decimal::ONE,
            },
            instrument_spec: None,
            created_at: now,
        })
        .unwrap();

    let order_id = outcome.order.id.clone();
    oms.mark_submitted(&order_id, now).unwrap();
    oms.mark_accepted(&order_id, now).unwrap();
    let venue_order = oms.get(&order_id).unwrap().clone();
    let venue = CancelAckButStillOpenVenue { order: venue_order };

    let result = cancel_order(&mut oms, &venue, &order_id).await;

    assert!(matches!(result, Err(VenueError::Ambiguous(_))));
    assert_eq!(
        oms.get(&order_id).unwrap().status,
        OrderStatus::CancelPending
    );
}

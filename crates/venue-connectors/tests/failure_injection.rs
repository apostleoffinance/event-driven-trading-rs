//! QOS-016 deterministic venue failure-injection tests.
//!
//! These tests exercise the OMS/venue boundary only. They do not enable live execution.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::time::Duration;

use async_trait::async_trait;
use chrono::Utc;
use domain::{
    AccountId, AccountState, ClientOrderId, DeploymentId, InstrumentId, Order, OrderId, OrderSide,
    OrderStatus, OrderType, Position, RiskDecision, StrategyId, TimeInForce, TradeIntentId,
    VenueId,
};
use execution_engine::{ExecutionEngine, NewOrderRequest};
use rust_decimal::Decimal;
use tokio::time::sleep;
use venue_connectors::{
    submit_approved_order, CancelRequest, OrderAck, OrderRequest, SimulatedVenue, VenueAdapter,
    VenueError, VenueResult,
};

#[derive(Debug, Clone, Copy)]
enum FailureMode {
    DisconnectBeforeSubmit,
    AmbiguousAfterVenueCommit,
    DelayBeforeSubmit,
    TruncateFillAcknowledgement,
}

struct InjectedVenue {
    inner: SimulatedVenue,
    mode: FailureMode,
}

impl InjectedVenue {
    fn new(inner: SimulatedVenue, mode: FailureMode) -> Self {
        Self { inner, mode }
    }
}

#[async_trait]
impl VenueAdapter for InjectedVenue {
    fn venue_name(&self) -> &str {
        self.inner.venue_name()
    }

    async fn account_state(&self, account_id: &AccountId) -> VenueResult<AccountState> {
        self.inner.account_state(account_id).await
    }

    async fn submit_order(&self, request: OrderRequest) -> VenueResult<OrderAck> {
        match self.mode {
            FailureMode::DisconnectBeforeSubmit => Err(VenueError::Failed(
                "injected disconnect before venue submission".into(),
            )),
            FailureMode::AmbiguousAfterVenueCommit => {
                let _ack = self.inner.submit_order(request).await?;
                Err(VenueError::Ambiguous(
                    "injected response loss after venue commit".into(),
                ))
            }
            FailureMode::DelayBeforeSubmit => {
                sleep(Duration::from_millis(25)).await;
                self.inner.submit_order(request).await
            }
            FailureMode::TruncateFillAcknowledgement => {
                let mut ack = self.inner.submit_order(request).await?;
                ack.fills.truncate(1);
                Ok(ack)
            }
        }
    }

    async fn cancel_order(&self, request: CancelRequest) -> VenueResult<()> {
        self.inner.cancel_order(request).await
    }

    async fn open_orders(&self, account_id: &AccountId) -> VenueResult<Vec<Order>> {
        self.inner.open_orders(account_id).await
    }

    async fn positions(&self, account_id: &AccountId) -> VenueResult<Vec<Position>> {
        self.inner.positions(account_id).await
    }

    async fn get_order(&self, order_id: &OrderId) -> VenueResult<Order> {
        self.inner.get_order(order_id).await
    }
}

async fn seeded_venue(account_text: &str) -> (SimulatedVenue, AccountId, VenueId) {
    let venue_id = VenueId::new("simulated").unwrap();
    let venue = SimulatedVenue::new(venue_id.clone());
    let account_id = AccountId::new(account_text).unwrap();
    let state = AccountState::new(account_id.clone(), Decimal::from(100_000), Utc::now()).unwrap();
    venue.register_account(state).await.unwrap();
    (venue, account_id, venue_id)
}

fn approved_request(
    account_id: AccountId,
    venue_id: VenueId,
    client_id: &str,
    quantity: Decimal,
) -> NewOrderRequest {
    NewOrderRequest {
        client_order_id: ClientOrderId::new(client_id).unwrap(),
        account_id,
        venue_id,
        deployment_id: DeploymentId::new("qos016-deployment").unwrap(),
        strategy_id: StrategyId::new("qos016-test").unwrap(),
        trade_intent_id: TradeIntentId::new(format!("intent-{client_id}")).unwrap(),
        instrument_id: InstrumentId::new("BTCUSDT").unwrap(),
        side: OrderSide::Buy,
        order_type: OrderType::Market,
        time_in_force: TimeInForce::Ioc,
        price: Some(Decimal::from(100)),
        risk_decision: RiskDecision::Approved { quantity },
        instrument_spec: None,
        created_at: Utc::now(),
    }
}

#[tokio::test]
async fn disconnect_before_submit_is_failed_not_unknown() {
    let (inner, account_id, venue_id) = seeded_venue("qos016-disconnect").await;
    let venue = InjectedVenue::new(inner, FailureMode::DisconnectBeforeSubmit);
    let mut oms = ExecutionEngine::new();

    let result = submit_approved_order(
        &mut oms,
        &venue,
        approved_request(
            account_id,
            venue_id,
            "qos016-disconnect-order",
            Decimal::ONE,
        ),
    )
    .await;

    assert!(matches!(result, Err(VenueError::Failed(_))));
    let order = oms.orders().into_iter().next().unwrap();
    assert_eq!(order.status, OrderStatus::Failed);
    assert!(venue
        .open_orders(&order.account_id)
        .await
        .unwrap()
        .is_empty());
}

#[tokio::test]
async fn response_loss_after_venue_commit_keeps_oms_unknown_without_blind_retry() {
    let (inner, account_id, venue_id) = seeded_venue("qos016-ambiguous").await;
    let venue = InjectedVenue::new(inner, FailureMode::AmbiguousAfterVenueCommit);
    let mut oms = ExecutionEngine::new();
    let request = approved_request(
        account_id.clone(),
        venue_id,
        "qos016-ambiguous-order",
        Decimal::ONE,
    );

    let first = submit_approved_order(&mut oms, &venue, request.clone())
        .await
        .unwrap();
    assert_eq!(first.order.status, OrderStatus::Unknown);
    assert_eq!(venue.open_orders(&account_id).await.unwrap().len(), 0);
    // The simulated market order is terminal at the venue despite the lost response.
    let venue_order = venue.get_order(&first.order.id).await.unwrap();
    assert_eq!(venue_order.status, OrderStatus::Filled);

    // A same-client-ID replay is handled by OMS and does not submit a second order.
    let replay = submit_approved_order(&mut oms, &venue, request)
        .await
        .unwrap();
    assert!(!replay.created);
    assert_eq!(replay.order.id, first.order.id);
    assert_eq!(
        venue.positions(&account_id).await.unwrap()[0].quantity,
        Decimal::ONE
    );
}

#[tokio::test]
async fn delayed_ack_is_handled_without_changing_order_identity() {
    let (inner, account_id, venue_id) = seeded_venue("qos016-delayed").await;
    let venue = InjectedVenue::new(inner, FailureMode::DelayBeforeSubmit);
    let mut oms = ExecutionEngine::new();

    let outcome = submit_approved_order(
        &mut oms,
        &venue,
        approved_request(
            account_id.clone(),
            venue_id,
            "qos016-delayed-order",
            Decimal::ONE,
        ),
    )
    .await
    .unwrap();

    assert_eq!(outcome.order.status, OrderStatus::Filled);
    assert_eq!(
        venue
            .get_order(&outcome.order.id)
            .await
            .unwrap()
            .client_order_id,
        outcome.order.client_order_id
    );
}

#[tokio::test]
async fn truncated_fill_ack_exposes_reconciliation_break_instead_of_matching_venue_truth() {
    let (inner, account_id, venue_id) = seeded_venue("qos016-partial-ack").await;
    let venue = InjectedVenue::new(inner, FailureMode::TruncateFillAcknowledgement);
    let mut oms = ExecutionEngine::new();

    let outcome = submit_approved_order(
        &mut oms,
        &venue,
        approved_request(
            account_id,
            venue_id,
            "qos016-partial-ack-order",
            Decimal::from(2),
        ),
    )
    .await
    .unwrap();

    // The adapter acknowledges one of two fills even though the venue committed both.
    assert_eq!(outcome.order.status, OrderStatus::PartiallyFilled);
    assert_eq!(outcome.order.filled_quantity, Decimal::ONE);
    let venue_order = venue.get_order(&outcome.order.id).await.unwrap();
    assert_eq!(venue_order.status, OrderStatus::Filled);
    assert_eq!(venue_order.filled_quantity, Decimal::from(2));
    assert_ne!(outcome.order.filled_quantity, venue_order.filled_quantity);
}

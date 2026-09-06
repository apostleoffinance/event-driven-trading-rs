//! Persistence survives reconnect — Phase 8 acceptance.

#![allow(clippy::unwrap_used)]

use chrono::Utc;
use domain::{
    Account, AccountId, AccountState, AccountType, ClientOrderId, DeploymentId, Fill, FillId,
    Instrument, InstrumentId, Order, OrderId, OrderSide, OrderStatus, OrderType, Position,
    PositionId, PositionSide, RiskDecision, RiskPolicy, RiskProfile, RiskProfileId,
    StrategyDeployment, StrategyId, StrategyVersion, TimeInForce, TradeIntent, TradeIntentId,
    Venue, VenueId, VenueType,
};
use execution_engine::{AuditEvent, AuditKind};
use persistence::TradingStore;
use rust_decimal::Decimal;
use sqlx::PgPool;

async fn seed_graph(store: &TradingStore) {
    let now = Utc::now();
    let venue = Venue::new(
        VenueId::new("simulated").unwrap(),
        VenueType::Simulated,
        "Simulated",
        now,
    );
    store.upsert_venue(&venue).await.unwrap();

    let instrument = Instrument::crypto_spot(
        InstrumentId::new("BTCUSDT").unwrap(),
        "BTCUSDT",
        "BTC",
        "USDT",
    )
    .unwrap();
    store.upsert_instrument(&instrument).await.unwrap();

    let account_id = AccountId::new("prop-account-001").unwrap();
    let account = Account::new(account_id.clone(), AccountType::Prop, venue.id.clone(), now);
    let state = AccountState::new(account_id.clone(), Decimal::from(100_000), now).unwrap();
    store
        .upsert_account(&account, Decimal::from(100_000), Some("prop"), &state)
        .await
        .unwrap();

    let strategy_id = StrategyId::new("btc-mean-reversion").unwrap();
    let version = StrategyVersion::new("v1").unwrap();
    let risk_profile = RiskProfile {
        id: RiskProfileId::new("balanced").unwrap(),
        name: "Balanced".into(),
        policy: RiskPolicy::try_new(
            Decimal::from(2),
            Decimal::from(5),
            Decimal::from(50),
            Decimal::new(15, 1),
            Decimal::from(5),
            Decimal::from(10),
            5,
        )
        .unwrap(),
    };
    let deployment = StrategyDeployment::new(
        DeploymentId::new("deployment-001").unwrap(),
        strategy_id.clone(),
        version.clone(),
        account_id.clone(),
        risk_profile.id.clone(),
        now,
    );
    store
        .ensure_strategy_graph(
            &strategy_id,
            &version,
            "BTC Mean Reversion",
            &deployment,
            &risk_profile,
        )
        .await
        .unwrap();
}

#[sqlx::test(migrations = "../../migrations")]
async fn critical_entities_survive_reload(pool: PgPool) {
    let store = TradingStore::new(pool.clone());
    seed_graph(&store).await;
    let now = Utc::now();

    let intent = TradeIntent::builder(
        TradeIntentId::new("ti-persist-1").unwrap(),
        StrategyId::new("btc-mean-reversion").unwrap(),
        StrategyVersion::new("v1").unwrap(),
        DeploymentId::new("deployment-001").unwrap(),
        InstrumentId::new("BTCUSDT").unwrap(),
        OrderSide::Buy,
        now,
    )
    .entry_price(Decimal::from(100))
    .unwrap()
    .target_quantity(Decimal::from(2))
    .unwrap()
    .build()
    .unwrap();
    store.save_trade_intent(&intent).await.unwrap();

    let decision = RiskDecision::Approved {
        quantity: Decimal::from(2),
    };
    store
        .save_risk_decision(
            &intent.id,
            &AccountId::new("prop-account-001").unwrap(),
            &decision,
        )
        .await
        .unwrap();

    let order = Order::create(
        OrderId::new("ord-persist-1").unwrap(),
        ClientOrderId::new("clid-persist-1").unwrap(),
        AccountId::new("prop-account-001").unwrap(),
        VenueId::new("simulated").unwrap(),
        DeploymentId::new("deployment-001").unwrap(),
        StrategyId::new("btc-mean-reversion").unwrap(),
        Some(intent.id.clone()),
        InstrumentId::new("BTCUSDT").unwrap(),
        OrderSide::Buy,
        OrderType::Market,
        TimeInForce::Ioc,
        Decimal::from(2),
        Some(Decimal::from(100)),
        now,
    )
    .unwrap();
    let mut order = order;
    order.status = OrderStatus::Filled;
    order.filled_quantity = Decimal::from(2);
    order.filled_at = Some(now);
    store.save_order(&order).await.unwrap();

    let fill = Fill::new(
        FillId::new("fill-persist-1").unwrap(),
        order.id.clone(),
        InstrumentId::new("BTCUSDT").unwrap(),
        Decimal::from(100),
        Decimal::from(2),
        Decimal::new(1, 1),
        now,
    )
    .unwrap();
    store.save_fill(&fill).await.unwrap();

    let position = Position::open(
        PositionId::new("pos-persist-1").unwrap(),
        AccountId::new("prop-account-001").unwrap(),
        InstrumentId::new("BTCUSDT").unwrap(),
        PositionSide::Long,
        Decimal::from(2),
        Decimal::from(100),
        None,
        now,
    )
    .unwrap();
    store.upsert_position(&position).await.unwrap();

    store
        .save_audit_event(
            &AuditEvent::new(
                now,
                order.id.clone(),
                order.client_order_id.clone(),
                AuditKind::Filled,
                "persisted",
            ),
            Some(&order.account_id),
            order.trade_intent_id.as_ref(),
        )
        .await
        .unwrap();

    // Simulate restart: new store handle on same pool (fresh reads).
    let reloaded = TradingStore::new(pool);
    let loaded_intent = reloaded.load_trade_intent(&intent.id).await.unwrap();
    assert_eq!(loaded_intent.target_quantity, Some(Decimal::from(2)));

    let loaded_order = reloaded
        .load_order_by_client_id(&order.client_order_id)
        .await
        .unwrap();
    assert_eq!(loaded_order.status, OrderStatus::Filled);
    assert_eq!(loaded_order.filled_quantity, Decimal::from(2));

    let fills = reloaded.load_fills_for_order(&order.id).await.unwrap();
    assert_eq!(fills.len(), 1);
    assert_eq!(fills[0].quantity, Decimal::from(2));

    let positions = reloaded
        .load_positions(&AccountId::new("prop-account-001").unwrap())
        .await
        .unwrap();
    assert_eq!(positions.len(), 1);
    assert_eq!(positions[0].quantity, Decimal::from(2));

    let state = reloaded
        .load_account_state(&AccountId::new("prop-account-001").unwrap())
        .await
        .unwrap();
    assert_eq!(state.balance, Decimal::from(100_000));
}

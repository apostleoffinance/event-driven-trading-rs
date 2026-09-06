//! Repository façade for critical trading persistence.

mod accounts;
mod audit;
mod fills;
mod instruments;
mod orders;
mod positions;
mod risk;
mod trade_intents;
mod venues;

use sqlx::PgPool;

use crate::error::PersistenceResult;

/// Aggregated store used by the trading runtime.
#[derive(Clone)]
pub struct TradingStore {
    pool: PgPool,
}

impl TradingStore {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    pub async fn upsert_venue(&self, venue: &domain::Venue) -> PersistenceResult<()> {
        venues::upsert(&self.pool, venue).await
    }

    pub async fn upsert_instrument(
        &self,
        instrument: &domain::Instrument,
    ) -> PersistenceResult<()> {
        instruments::upsert(&self.pool, instrument).await
    }

    pub async fn upsert_account(
        &self,
        account: &domain::Account,
        starting_capital: rust_decimal::Decimal,
        label: Option<&str>,
        state: &domain::AccountState,
    ) -> PersistenceResult<()> {
        accounts::upsert_account(&self.pool, account, starting_capital, label, state).await
    }

    pub async fn load_account_state(
        &self,
        account_id: &domain::AccountId,
    ) -> PersistenceResult<domain::AccountState> {
        accounts::load_state(&self.pool, account_id).await
    }

    pub async fn save_trade_intent(&self, intent: &domain::TradeIntent) -> PersistenceResult<()> {
        trade_intents::insert(&self.pool, intent).await
    }

    pub async fn load_trade_intent(
        &self,
        id: &domain::TradeIntentId,
    ) -> PersistenceResult<domain::TradeIntent> {
        trade_intents::load(&self.pool, id).await
    }

    pub async fn save_risk_decision(
        &self,
        trade_intent_id: &domain::TradeIntentId,
        account_id: &domain::AccountId,
        decision: &domain::RiskDecision,
    ) -> PersistenceResult<()> {
        risk::insert_decision(&self.pool, trade_intent_id, account_id, decision).await
    }

    pub async fn save_order(&self, order: &domain::Order) -> PersistenceResult<()> {
        orders::upsert(&self.pool, order).await
    }

    pub async fn load_order(&self, order_id: &domain::OrderId) -> PersistenceResult<domain::Order> {
        orders::load(&self.pool, order_id).await
    }

    pub async fn load_order_by_client_id(
        &self,
        client_order_id: &domain::ClientOrderId,
    ) -> PersistenceResult<domain::Order> {
        orders::load_by_client_id(&self.pool, client_order_id).await
    }

    pub async fn save_fill(&self, fill: &domain::Fill) -> PersistenceResult<()> {
        fills::insert(&self.pool, fill).await
    }

    pub async fn load_fills_for_order(
        &self,
        order_id: &domain::OrderId,
    ) -> PersistenceResult<Vec<domain::Fill>> {
        fills::load_for_order(&self.pool, order_id).await
    }

    pub async fn upsert_position(&self, position: &domain::Position) -> PersistenceResult<()> {
        positions::upsert(&self.pool, position).await
    }

    pub async fn load_positions(
        &self,
        account_id: &domain::AccountId,
    ) -> PersistenceResult<Vec<domain::Position>> {
        positions::load_for_account(&self.pool, account_id).await
    }

    pub async fn save_audit_event(
        &self,
        event: &execution_engine::AuditEvent,
        account_id: Option<&domain::AccountId>,
        trade_intent_id: Option<&domain::TradeIntentId>,
    ) -> PersistenceResult<()> {
        audit::insert(&self.pool, event, account_id, trade_intent_id).await
    }

    pub async fn ensure_strategy_graph(
        &self,
        strategy_id: &domain::StrategyId,
        strategy_version: &domain::StrategyVersion,
        name: &str,
        deployment: &domain::StrategyDeployment,
        risk_profile: &domain::RiskProfile,
    ) -> PersistenceResult<()> {
        risk::ensure_strategy_graph(
            &self.pool,
            strategy_id,
            strategy_version,
            name,
            deployment,
            risk_profile,
        )
        .await
    }
}

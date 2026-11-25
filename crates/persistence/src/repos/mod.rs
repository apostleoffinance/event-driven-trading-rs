//! Repository façade for critical trading persistence.

mod accounts;
mod audit;
mod broker_registry;
mod fills;
mod identity_sessions;
mod instruments;
mod journal;
mod mt5_bridge;
pub mod mt5_market_data;
mod orders;
mod organizations;
mod positions;
mod risk;
mod sizing_audits;
mod trade_intents;
mod venues;
mod workspaces;

use sqlx::PgPool;

use crate::error::PersistenceResult;

pub use broker_registry::{BrokerBindingTransition, BrokerBindingView, BrokerCatalogEntry};

pub use mt5_bridge::{
    Mt5DurableOrderState, Mt5OrderClaim, Mt5SequenceObservation, Mt5StreamReadiness,
};

/// Bounded, owner-scoped filters for Journal trade listing.
#[derive(Debug, Clone, Default)]
pub struct JournalTradeListQuery {
    pub account_id: Option<String>,
    pub symbol: Option<String>,
    pub setup_id: Option<String>,
    pub closed_from: Option<chrono::DateTime<chrono::Utc>>,
    pub closed_to: Option<chrono::DateTime<chrono::Utc>>,
    pub limit: i64,
    pub offset: i64,
}

/// Aggregated store used by the trading runtime.
#[derive(Clone)]
pub struct TradingStore {
    pool: PgPool,
}

impl TradingStore {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Insert a journal trade without creating or mutating OMS state.
    pub async fn insert_journal_trade(
        &self,
        trade: &domain::JournalTrade,
    ) -> PersistenceResult<()> {
        journal::insert(&self.pool, trade).await
    }

    /// Owner-scoped journal listing; caller must pass the authenticated principal ID.
    pub async fn list_journal_trades(
        &self,
        owner_id: &str,
        query: JournalTradeListQuery,
    ) -> PersistenceResult<Vec<domain::JournalTrade>> {
        journal::list_for_owner(&self.pool, owner_id, &query).await
    }

    pub async fn get_journal_trade(
        &self,
        owner_id: &str,
        trade_id: &str,
    ) -> PersistenceResult<domain::JournalTrade> {
        journal::get_for_owner(&self.pool, owner_id, trade_id).await
    }

    pub async fn delete_journal_trade(
        &self,
        owner_id: &str,
        trade_id: &str,
    ) -> PersistenceResult<bool> {
        journal::delete_for_owner(&self.pool, owner_id, trade_id).await
    }

    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    /// Resolve the canonical server identifier from the trusted broker catalog.
    /// The identifier is immutable; callers still re-evaluate catalog status via
    /// the database-backed admission check before accepting each MT5 frame.
    pub async fn broker_server_canonical_identifier(
        &self,
        broker_ref: &str,
        server_ref: &str,
    ) -> PersistenceResult<Option<String>> {
        sqlx::query_scalar(
            "SELECT canonical_identifier FROM broker_servers WHERE id=$1 AND broker_id=$2 AND status='ACTIVE'",
        )
        .bind(server_ref)
        .bind(broker_ref)
        .fetch_optional(&self.pool)
        .await
        .map_err(Into::into)
    }

    pub async fn validate_access_token(
        &self,
        claims: &identity_token::AccessClaims,
    ) -> PersistenceResult<bool> {
        identity_sessions::validate_access_token(&self.pool, claims).await
    }

    pub async fn broker_catalog(&self) -> PersistenceResult<Vec<BrokerCatalogEntry>> {
        broker_registry::list_catalog(&self.pool).await
    }

    pub async fn broker_bindings(
        &self,
        organization: &str,
    ) -> PersistenceResult<Vec<BrokerBindingView>> {
        broker_registry::list_bindings(&self.pool, organization).await
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn register_pending_broker_binding(
        &self,
        organization: &str,
        actor: &str,
        quant_account: &str,
        broker: &str,
        server: &str,
        account_id: &str,
        binding_id: &str,
        normalization_version: i16,
        key_version: &str,
        lookup_hmacs: &[(String, [u8; 32])],
        reason: &str,
    ) -> PersistenceResult<()> {
        broker_registry::register_pending_binding(
            &self.pool,
            organization,
            actor,
            quant_account,
            broker,
            server,
            account_id,
            binding_id,
            normalization_version,
            key_version,
            lookup_hmacs,
            reason,
        )
        .await
    }

    pub async fn transition_broker_binding(
        &self,
        organization: &str,
        actor: &str,
        binding_id: &str,
        expected_revision: i64,
        transition: BrokerBindingTransition,
        reason: &str,
    ) -> PersistenceResult<i64> {
        broker_registry::transition_binding(
            &self.pool,
            organization,
            actor,
            binding_id,
            expected_revision,
            transition,
            reason,
        )
        .await
    }

    pub async fn create_organization(
        &self,
        org: &domain::Organization,
        actor: &str,
    ) -> PersistenceResult<domain::Workspace> {
        organizations::create(&self.pool, org, actor).await
    }

    /// Create a workspace and append its initial audit event in one transaction.
    /// The creator must be an active organization OWNER or ADMIN.
    pub async fn create_workspace(
        &self,
        workspace: &domain::Workspace,
        reason: &str,
    ) -> PersistenceResult<()> {
        workspaces::create(&self.pool, workspace, reason).await
    }

    pub async fn workspaces_for_organization(
        &self,
        organization: &str,
    ) -> PersistenceResult<Vec<domain::Workspace>> {
        workspaces::list_for_organization(&self.pool, organization).await
    }

    pub async fn workspace(
        &self,
        organization: &str,
        workspace_id: &str,
    ) -> PersistenceResult<Option<domain::Workspace>> {
        workspaces::get_for_organization(&self.pool, organization, workspace_id).await
    }

    pub async fn default_workspace(
        &self,
        organization: &str,
    ) -> PersistenceResult<Option<domain::Workspace>> {
        workspaces::default_for_organization(&self.pool, organization).await
    }

    /// Resolve the workspace assigned to a Quant account without bypassing
    /// organization ownership; callers must still validate active entitlement.
    pub async fn workspace_for_account(
        &self,
        organization: &str,
        account_id: &str,
    ) -> PersistenceResult<Option<domain::Workspace>> {
        workspaces::for_account(&self.pool, organization, account_id).await
    }

    pub async fn revise_organization(
        &self,
        organization: &str,
        actor: &str,
        expected_revision: i64,
        name: Option<&str>,
        status: Option<domain::OrganizationStatus>,
        reason: &str,
    ) -> PersistenceResult<i64> {
        organizations::revise_organization(
            &self.pool,
            organization,
            actor,
            expected_revision,
            name,
            status,
            reason,
        )
        .await
    }

    pub async fn organizations_for_user(
        &self,
        user: &str,
    ) -> PersistenceResult<Vec<(domain::Organization, domain::OrganizationRole, u64)>> {
        organizations::list_for_user(&self.pool, user).await
    }

    pub async fn organization_membership(
        &self,
        organization: &str,
        user: &str,
    ) -> PersistenceResult<Option<(domain::OrganizationRole, domain::MembershipStatus, i64)>> {
        organizations::membership(&self.pool, organization, user).await
    }

    pub async fn organization_members(
        &self,
        organization: &str,
    ) -> PersistenceResult<Vec<domain::OrganizationMembership>> {
        organizations::list_members(&self.pool, organization).await
    }

    pub async fn create_organization_membership(
        &self,
        organization: &str,
        user: &str,
        actor: &str,
        role: domain::OrganizationRole,
        reason: &str,
    ) -> PersistenceResult<()> {
        organizations::create_membership(&self.pool, organization, user, actor, role, reason).await
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn revise_organization_membership(
        &self,
        organization: &str,
        user: &str,
        actor: &str,
        expected_revision: i64,
        role: domain::OrganizationRole,
        status: domain::MembershipStatus,
        reason: &str,
    ) -> PersistenceResult<i64> {
        organizations::revise_membership(
            &self.pool,
            organization,
            user,
            actor,
            expected_revision,
            role,
            status,
            reason,
        )
        .await
    }

    pub async fn transfer_organization_owner(
        &self,
        organization: &str,
        actor: &str,
        target: &str,
        actor_revision: i64,
        target_revision: i64,
        reason: &str,
    ) -> PersistenceResult<()> {
        organizations::transfer_owner(
            &self.pool,
            organization,
            actor,
            target,
            actor_revision,
            target_revision,
            reason,
        )
        .await
    }

    pub async fn organization_entitlements(
        &self,
        organization: &str,
    ) -> PersistenceResult<Vec<domain::OrganizationAccountEntitlement>> {
        organizations::list_entitlements(&self.pool, organization).await
    }

    pub async fn assign_account_entitlement(
        &self,
        organization: &str,
        account: &str,
        actor: &str,
        account_revision: i64,
        entitlement_revision: i64,
        reason: &str,
    ) -> PersistenceResult<i64> {
        organizations::assign_account(
            &self.pool,
            organization,
            account,
            actor,
            account_revision,
            entitlement_revision,
            reason,
        )
        .await
    }

    pub async fn create_paper_account_for_owner(
        &self,
        organization: &str,
        actor: &str,
        account: &domain::Account,
        starting_capital: rust_decimal::Decimal,
        label: &str,
        state: &domain::AccountState,
    ) -> PersistenceResult<i64> {
        organizations::create_paper_account_for_owner(
            &self.pool,
            organization,
            actor,
            account,
            starting_capital,
            label,
            state,
        )
        .await
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn set_account_entitlement_status(
        &self,
        organization: &str,
        account: &str,
        actor: &str,
        account_revision: i64,
        entitlement_revision: i64,
        status: domain::EntitlementStatus,
        reason: &str,
    ) -> PersistenceResult<i64> {
        organizations::set_entitlement_status(
            &self.pool,
            organization,
            account,
            actor,
            account_revision,
            entitlement_revision,
            status,
            reason,
        )
        .await
    }

    /// Acquire the unique durable publisher lease for one account/terminal pair.
    pub async fn acquire_mt5_worker_lease(
        &self,
        quant_account_id: &str,
        terminal_id: &str,
        binding_id: &str,
        worker_ref: &str,
        process_token: &str,
        lease_seconds: i64,
    ) -> PersistenceResult<crate::Mt5WorkerLease> {
        crate::broker_worker_lease::acquire(
            &self.pool,
            quant_account_id,
            terminal_id,
            binding_id,
            worker_ref,
            process_token,
            lease_seconds,
        )
        .await
    }

    /// Renew the exact live lease; expiry or takeover permanently fences this process.
    pub async fn renew_mt5_worker_lease(
        &self,
        lease: &crate::Mt5WorkerLease,
        lease_seconds: i64,
    ) -> PersistenceResult<chrono::DateTime<chrono::Utc>> {
        crate::broker_worker_lease::renew(&self.pool, lease, lease_seconds).await
    }

    pub async fn validate_mt5_worker_lease(
        &self,
        lease: &crate::Mt5WorkerLease,
    ) -> PersistenceResult<()> {
        crate::broker_worker_lease::validate(&self.pool, lease).await
    }

    /// Compare-and-set release; stale owners cannot release a successor's lease.
    pub async fn release_mt5_worker_lease(
        &self,
        lease: &crate::Mt5WorkerLease,
    ) -> PersistenceResult<bool> {
        crate::broker_worker_lease::release(&self.pool, lease).await
    }

    /// Atomically reserve a durable MT5 client-order ID before transport send.
    pub async fn claim_mt5_order(
        &self,
        account_id: &str,
        client_order_id: &str,
        request_payload: serde_json::Value,
    ) -> PersistenceResult<Mt5OrderClaim> {
        mt5_bridge::claim_order(&self.pool, account_id, client_order_id, request_payload).await
    }

    /// Persist an MT5 accepted/rejected/unknown outcome without rewriting terminal state.
    pub async fn record_mt5_order_outcome(
        &self,
        account_id: &str,
        client_order_id: &str,
        state: Mt5DurableOrderState,
        venue_order_id: Option<&str>,
        reason_code: Option<&str>,
    ) -> PersistenceResult<()> {
        mt5_bridge::record_outcome(
            &self.pool,
            account_id,
            client_order_id,
            state,
            venue_order_id,
            reason_code,
        )
        .await
    }

    /// Record a sequence checkpoint; gaps and uninitialized streams require reconciliation.
    pub async fn observe_mt5_sequence(
        &self,
        account_id: &str,
        terminal_id: &str,
        sequence: u64,
    ) -> PersistenceResult<Mt5SequenceObservation> {
        mt5_bridge::observe_sequence(&self.pool, account_id, terminal_id, sequence).await
    }

    /// Mark the stream ready only after the caller has reconciled external orders and positions.
    pub async fn mark_mt5_stream_reconciled(
        &self,
        account_id: &str,
        terminal_id: &str,
        reconciled_sequence: u64,
    ) -> PersistenceResult<()> {
        mt5_bridge::mark_stream_reconciled(&self.pool, account_id, terminal_id, reconciled_sequence)
            .await
    }

    /// Persist degraded state without advancing a known-good sequence checkpoint.
    pub async fn mark_mt5_stream_degraded(
        &self,
        account_id: &str,
        terminal_id: &str,
        observed_sequence: u64,
    ) -> PersistenceResult<()> {
        mt5_bridge::mark_stream_degraded(&self.pool, account_id, terminal_id, observed_sequence)
            .await
    }

    pub async fn mt5_stream_readiness(
        &self,
        account_id: &str,
        terminal_id: &str,
    ) -> PersistenceResult<Mt5StreamReadiness> {
        mt5_bridge::stream_readiness(&self.pool, account_id, terminal_id).await
    }

    /// Persist a complete external MT5 state snapshot and atomically restore the
    /// durable stream checkpoint only when the snapshot sequence is not stale.
    pub async fn persist_mt5_reconciliation_snapshot(
        &self,
        account_id: &str,
        terminal_id: &str,
        sequence: u64,
        source: &str,
        captured_at: chrono::DateTime<chrono::Utc>,
        snapshot: serde_json::Value,
    ) -> PersistenceResult<()> {
        mt5_bridge::persist_reconciliation_snapshot(
            &self.pool,
            account_id,
            terminal_id,
            sequence,
            source,
            captured_at,
            snapshot,
        )
        .await
    }

    /// Persist an MT5 instrument/quote frame without changing stream readiness.
    #[allow(clippy::too_many_arguments)]
    pub async fn persist_mt5_market_data_snapshot(
        &self,
        account_id: &str,
        terminal_id: &str,
        server: &str,
        sequence: u64,
        source: &str,
        account_currency: &str,
        captured_at: chrono::DateTime<chrono::Utc>,
        snapshot: serde_json::Value,
    ) -> PersistenceResult<()> {
        mt5_bridge::persist_market_data_snapshot(
            &self.pool,
            account_id,
            terminal_id,
            server,
            sequence,
            source,
            account_currency,
            captured_at,
            snapshot,
        )
        .await
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

    pub async fn load_account(
        &self,
        account_id: &domain::AccountId,
    ) -> PersistenceResult<domain::Account> {
        accounts::load_account(&self.pool, account_id).await
    }

    /// Record an attributable environment verification using optimistic revision checking.
    pub async fn verify_account_environment(
        &self,
        account_id: &domain::AccountId,
        environment: domain::AccountEnvironment,
        verifier_id: &str,
        expected_revision: u64,
        reason: Option<&str>,
    ) -> PersistenceResult<u64> {
        accounts::verify_environment(
            &self.pool,
            account_id,
            environment,
            verifier_id,
            expected_revision,
            reason,
        )
        .await
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

    /// Append an instrument-aware sizing calculation for immutable audit.
    #[allow(clippy::too_many_arguments)]
    pub async fn save_sizing_audit(
        &self,
        trade_intent_id: &domain::TradeIntentId,
        account_id: &domain::AccountId,
        instrument_id: &domain::InstrumentId,
        source_id: &str,
        snapshot_captured_at: chrono::DateTime<chrono::Utc>,
        evaluated_at: chrono::DateTime<chrono::Utc>,
        payload: serde_json::Value,
    ) -> PersistenceResult<()> {
        sizing_audits::insert(
            &self.pool,
            trade_intent_id,
            account_id,
            instrument_id,
            source_id,
            snapshot_captured_at,
            evaluated_at,
            payload,
        )
        .await
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

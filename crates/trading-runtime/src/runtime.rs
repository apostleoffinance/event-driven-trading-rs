//! Continuous market-data loop and runtime session state.

use account_engine::{AccountEngine, InMemoryAccountEngine};
use chrono::Utc;
use domain::{Position, TradeIntent};
use events::{EventEnvelope, EventPublisher, EventSubscriber, TradingEvent};
use execution_engine::ExecutionEngine;
use persistence::TradingStore;
use reconciliation::{InternalSnapshot, Reconciler, ReconciliationReport};
use risk_engine::DefaultRiskEvaluator;
use rust_decimal::Decimal;
use strategy_runtime::{process_market_envelope, Strategy, StrategyContext};
use venue_connectors::{SimulatedVenue, VenueAdapter};

use crate::config::RuntimeConfig;
use crate::error::{RuntimeError, RuntimeResult};
use crate::pipeline::IntentOutcome;

/// Orchestrates MarketData → Strategy → Risk → OMS → Venue → Positions.
///
/// Owns account/OMS/risk state. Strategy and venue are supplied per run so
/// callers can swap adapters without rebuilding the session.
pub struct TradingRuntime {
    pub(crate) config: RuntimeConfig,
    pub(crate) accounts: InMemoryAccountEngine,
    pub(crate) oms: ExecutionEngine,
    pub(crate) risk: DefaultRiskEvaluator,
    pub(crate) store: Option<TradingStore>,
}

/// Summary of handling one inbound market-data envelope.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct TickOutcome {
    pub intents: Vec<TradeIntent>,
    pub executions: Vec<IntentOutcome>,
}

impl TradingRuntime {
    /// Create an in-memory runtime (no PostgreSQL).
    pub fn new(config: RuntimeConfig) -> Self {
        Self {
            config,
            accounts: InMemoryAccountEngine::new(),
            oms: ExecutionEngine::new(),
            risk: DefaultRiskEvaluator::new(),
            store: None,
        }
    }

    /// Attach a persistence store (optional — tests may omit).
    pub fn with_store(mut self, store: TradingStore) -> Self {
        self.store = Some(store);
        self
    }

    pub fn config(&self) -> &RuntimeConfig {
        &self.config
    }

    pub fn accounts(&self) -> &InMemoryAccountEngine {
        &self.accounts
    }

    pub fn accounts_mut(&mut self) -> &mut InMemoryAccountEngine {
        &mut self.accounts
    }

    pub fn oms(&self) -> &ExecutionEngine {
        &self.oms
    }

    /// Open a prop account on the simulated venue and register venue-local state.
    pub async fn bootstrap_prop_on_simulated(
        &mut self,
        venue: &SimulatedVenue,
        starting_capital: Decimal,
    ) -> RuntimeResult<()> {
        self.accounts.open_prop_on_simulated(
            self.config.account_id.clone(),
            starting_capital,
            Utc::now(),
        )?;
        let state = self.accounts.get(&self.config.account_id)?.state.clone();
        venue.register_account(state).await?;
        Ok(())
    }

    /// Process a single inbound envelope (market data only drives the strategy).
    pub async fn handle_envelope(
        &mut self,
        strategy: &mut dyn Strategy,
        ctx: &StrategyContext,
        venue: &dyn VenueAdapter,
        envelope: &EventEnvelope,
        events_out: &EventPublisher,
    ) -> RuntimeResult<TickOutcome> {
        match &envelope.event {
            TradingEvent::MarketDataReceived { .. } => {
                self.handle_market_data(strategy, ctx, venue, envelope, events_out)
                    .await
            }
            TradingEvent::TradeIntentCreated { intent } => {
                let execution = self.execute_intent(intent, venue, events_out).await?;
                Ok(TickOutcome {
                    intents: vec![intent.clone()],
                    executions: vec![execution],
                })
            }
            other => {
                tracing::debug!(event = other.name(), "runtime ignoring non-pipeline event");
                Ok(TickOutcome::default())
            }
        }
    }

    /// Drive strategy from market data, publish intents, execute each immediately.
    ///
    /// Intents are executed from the strategy return value (not re-consumed from
    /// the outbound bus) to avoid channel capacity deadlocks in a single task.
    pub async fn handle_market_data(
        &mut self,
        strategy: &mut dyn Strategy,
        ctx: &StrategyContext,
        venue: &dyn VenueAdapter,
        envelope: &EventEnvelope,
        events_out: &EventPublisher,
    ) -> RuntimeResult<TickOutcome> {
        let intents = process_market_envelope(strategy, ctx, envelope, events_out).await?;
        let mut executions = Vec::with_capacity(intents.len());
        for intent in &intents {
            executions.push(self.execute_intent(intent, venue, events_out).await?);
        }
        Ok(TickOutcome {
            intents,
            executions,
        })
    }

    /// Continuous loop: receive market (or intent) events until the channel closes.
    pub async fn run(
        &mut self,
        market_in: &mut EventSubscriber,
        strategy: &mut dyn Strategy,
        ctx: &StrategyContext,
        venue: &dyn VenueAdapter,
        events_out: &EventPublisher,
    ) -> RuntimeResult<()> {
        while let Some(envelope) = market_in.recv().await {
            self.handle_envelope(strategy, ctx, venue, &envelope, events_out)
                .await?;
        }
        Ok(())
    }

    /// Process up to `max_ticks` inbound envelopes then return.
    pub async fn run_n(
        &mut self,
        market_in: &mut EventSubscriber,
        strategy: &mut dyn Strategy,
        ctx: &StrategyContext,
        venue: &dyn VenueAdapter,
        events_out: &EventPublisher,
        max_ticks: usize,
    ) -> RuntimeResult<Vec<TickOutcome>> {
        let mut outcomes = Vec::new();
        for _ in 0..max_ticks {
            let Some(envelope) = market_in.recv().await else {
                break;
            };
            outcomes.push(
                self.handle_envelope(strategy, ctx, venue, &envelope, events_out)
                    .await?,
            );
        }
        Ok(outcomes)
    }

    /// Run until `predicate` returns true on a tick outcome, or the channel closes.
    pub async fn run_until<F>(
        &mut self,
        market_in: &mut EventSubscriber,
        strategy: &mut dyn Strategy,
        ctx: &StrategyContext,
        venue: &dyn VenueAdapter,
        events_out: &EventPublisher,
        mut predicate: F,
    ) -> RuntimeResult<Vec<TickOutcome>>
    where
        F: FnMut(&TickOutcome) -> bool,
    {
        let mut outcomes = Vec::new();
        while let Some(envelope) = market_in.recv().await {
            let tick = self
                .handle_envelope(strategy, ctx, venue, &envelope, events_out)
                .await?;
            let done = predicate(&tick);
            outcomes.push(tick);
            if done {
                return Ok(outcomes);
            }
        }
        Err(RuntimeError::Invariant(
            "event channel closed before run_until predicate matched".into(),
        ))
    }

    /// Compare internal account/OMS/position view to the venue.
    ///
    /// Alert-only: never overwrites account or OMS state on mismatch.
    /// Pass the position book the runtime/store believes is internal truth.
    pub async fn reconcile(
        &self,
        venue: &dyn VenueAdapter,
        internal_positions: Vec<Position>,
        events_out: Option<&EventPublisher>,
    ) -> RuntimeResult<ReconciliationReport> {
        let account_state = self.accounts.get(&self.config.account_id)?.state.clone();
        let open_orders: Vec<_> = self
            .oms
            .open_orders_for(&self.config.account_id)
            .into_iter()
            .cloned()
            .collect();
        let internal = InternalSnapshot::new(
            self.config.account_id.clone(),
            self.config.venue_id.clone(),
            account_state,
        )
        .with_positions(internal_positions)
        .with_open_orders(open_orders);

        Ok(Reconciler::new()
            .reconcile(venue, &internal, events_out)
            .await?)
    }
}

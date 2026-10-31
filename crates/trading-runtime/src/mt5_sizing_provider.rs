//! Runtime provider joining persisted MT5 quotes to instrument-aware risk sizing.
//!
//! The bridge is read-only. Margin, pending-order reserves, and estimated costs must
//! come from a separate trusted provider; this module never fabricates those inputs.

use std::collections::HashMap;

use async_trait::async_trait;
use chrono::{DateTime, Duration, Utc};
use domain::{AccountId, InstrumentId, OrderSide};
use persistence::{repos::mt5_market_data, TradingStore};
use risk_engine::mt5_market_data::{prepare_mt5_sizing, Mt5SizingSupplement};

use crate::{
    InstrumentSizingSnapshotProvider, RuntimeError, RuntimeInstrumentSizingContext, RuntimeResult,
};

/// Dynamic account/instrument inputs that are not part of an MT5 market-data frame.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Mt5SizingSupplementSnapshot {
    pub account_id: AccountId,
    pub instrument_id: InstrumentId,
    pub inputs: Mt5SizingSupplement,
    pub captured_at: DateTime<Utc>,
    pub max_age_seconds: i64,
}

#[async_trait]
pub trait Mt5SizingSupplementProvider: Send + Sync {
    async fn load_supplement(
        &self,
        account_id: &AccountId,
        instrument_id: &InstrumentId,
    ) -> RuntimeResult<Mt5SizingSupplementSnapshot>;
}

fn timestamps_match(left: &DateTime<Utc>, right: &DateTime<Utc>) -> bool {
    (*left)
        .signed_duration_since(*right)
        .num_microseconds()
        .is_some_and(|delta| delta.unsigned_abs() <= 1)
}

/// Explicit mapping from a Quant OS instrument ID to the broker's exact MT5 symbol.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Mt5InstrumentBinding {
    pub symbol: String,
    pub pnl_conversion_path: String,
    pub notional_conversion_path: String,
}

/// Reads persisted snapshots and composes them with fresh, trusted account/margin inputs.
pub struct Mt5PersistedSizingProvider {
    store: TradingStore,
    terminal_id: String,
    expected_server: String,
    expected_account_currency: String,
    max_age_seconds: i64,
    venue_capabilities_validated: bool,
    account_position_semantics_known: bool,
    bindings: HashMap<String, Mt5InstrumentBinding>,
    supplements: std::sync::Arc<dyn Mt5SizingSupplementProvider>,
}

impl Mt5PersistedSizingProvider {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        store: TradingStore,
        terminal_id: String,
        expected_server: String,
        expected_account_currency: String,
        max_age_seconds: i64,
        venue_capabilities_validated: bool,
        account_position_semantics_known: bool,
        bindings: HashMap<String, Mt5InstrumentBinding>,
        supplements: std::sync::Arc<dyn Mt5SizingSupplementProvider>,
    ) -> RuntimeResult<Self> {
        if terminal_id.trim().is_empty()
            || expected_server.trim().is_empty()
            || expected_account_currency.trim().is_empty()
            || max_age_seconds <= 0
            || bindings.is_empty()
            || bindings.iter().any(|(id, binding)| {
                id.trim().is_empty()
                    || binding.symbol.trim().is_empty()
                    || binding.pnl_conversion_path.trim().is_empty()
                    || binding.notional_conversion_path.trim().is_empty()
            })
        {
            return Err(RuntimeError::Invariant(
                "invalid MT5 sizing provider configuration".into(),
            ));
        }
        Ok(Self {
            store,
            terminal_id,
            expected_server,
            expected_account_currency,
            max_age_seconds,
            venue_capabilities_validated,
            account_position_semantics_known,
            bindings,
            supplements,
        })
    }

    async fn load_for_side(
        &self,
        account_id: &AccountId,
        instrument_id: &InstrumentId,
        side: OrderSide,
    ) -> RuntimeResult<RuntimeInstrumentSizingContext> {
        if !self.venue_capabilities_validated || !self.account_position_semantics_known {
            return Err(RuntimeError::Invariant(
                "MT5 sizing blocked until venue capabilities and account position semantics are validated".into(),
            ));
        }
        let binding = self.bindings.get(instrument_id.as_str()).ok_or_else(|| {
            RuntimeError::Invariant("no explicit MT5 symbol mapping for instrument".into())
        })?;
        let record =
            mt5_market_data::load_latest(self.store.pool(), account_id.as_str(), &self.terminal_id)
                .await?
                .ok_or_else(|| {
                    RuntimeError::Invariant("no persisted MT5 market snapshot".into())
                })?;
        if record.server != self.expected_server
            || record.account_currency != self.expected_account_currency
            || record
                .snapshot
                .get("sequence")
                .and_then(serde_json::Value::as_u64)
                != Some(record.sequence)
            || record
                .snapshot
                .get("captured_at")
                .and_then(serde_json::Value::as_str)
                .and_then(|value| chrono::DateTime::parse_from_rfc3339(value).ok())
                .is_none_or(|captured| {
                    captured
                        .with_timezone(&Utc)
                        .signed_duration_since(record.captured_at)
                        .num_microseconds()
                        .is_none_or(|delta| delta.unsigned_abs() > 1)
                })
        {
            return Err(RuntimeError::Invariant(
                "persisted MT5 snapshot identity or indexed metadata mismatch".into(),
            ));
        }
        let supplement = self
            .supplements
            .load_supplement(account_id, instrument_id)
            .await?;
        let now = Utc::now();
        if &supplement.account_id != account_id || &supplement.instrument_id != instrument_id {
            return Err(RuntimeError::Invariant(
                "trusted sizing supplement account/instrument identity mismatch".into(),
            ));
        }
        if supplement.max_age_seconds <= 0 || supplement.captured_at > now {
            return Err(RuntimeError::Invariant(
                "trusted MT5 sizing supplement has invalid freshness metadata".into(),
            ));
        }
        let max_age_seconds = self.max_age_seconds.min(supplement.max_age_seconds);
        if now.signed_duration_since(supplement.captured_at) > Duration::seconds(max_age_seconds) {
            return Err(RuntimeError::Invariant(
                "trusted MT5 sizing supplement is stale".into(),
            ));
        }
        let prepared = prepare_mt5_sizing(
            &record.snapshot,
            &binding.symbol,
            &self.expected_account_currency,
            side == OrderSide::Buy,
            now,
            Duration::seconds(max_age_seconds),
            &supplement.inputs,
        )
        .map_err(|error| RuntimeError::Invariant(format!("MT5 sizing data rejected: {error}")))?;

        if prepared.sequence != record.sequence
            || !timestamps_match(&prepared.snapshot_captured_at, &record.captured_at)
        {
            return Err(RuntimeError::Invariant(
                "prepared MT5 sizing metadata does not match persisted record".into(),
            ));
        }
        let captured_at = prepared
            .snapshot_captured_at
            .min(prepared.oldest_quote_time)
            .min(supplement.captured_at);

        Ok(RuntimeInstrumentSizingContext {
            account_id: account_id.clone(),
            instrument_id: instrument_id.clone(),
            source_id: format!("{}:{}:{}", self.terminal_id, record.source, record.sequence),
            pnl_conversion_path: binding.pnl_conversion_path.clone(),
            notional_conversion_path: binding.notional_conversion_path.clone(),
            venue_capabilities_validated: self.venue_capabilities_validated,
            account_position_semantics_known: self.account_position_semantics_known,
            captured_at,
            max_age_seconds,
            sizing: prepared.context,
            entry_price: Some(prepared.entry_price),
        })
    }
}

#[async_trait]
impl InstrumentSizingSnapshotProvider for Mt5PersistedSizingProvider {
    async fn load_snapshot(
        &self,
        account_id: &AccountId,
        instrument_id: &InstrumentId,
    ) -> RuntimeResult<RuntimeInstrumentSizingContext> {
        self.load_for_side(account_id, instrument_id, OrderSide::Buy)
            .await
    }

    async fn load_snapshot_for_side(
        &self,
        account_id: &AccountId,
        instrument_id: &InstrumentId,
        side: OrderSide,
    ) -> RuntimeResult<RuntimeInstrumentSizingContext> {
        self.load_for_side(account_id, instrument_id, side).await
    }
}

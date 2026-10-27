//! End-to-end contract tests for the persisted MT5 sizing provider.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::{collections::HashMap, sync::Arc};

use async_trait::async_trait;
use chrono::{Duration, Utc};
use domain::{AccountId, InstrumentId, OrderSide};
use persistence::TradingStore;
use risk_engine::mt5_market_data::Mt5SizingSupplement;
use rust_decimal::Decimal;
use serde_json::json;
use sqlx::PgPool;
use trading_runtime::{
    InstrumentSizingSnapshotProvider, Mt5InstrumentBinding, Mt5PersistedSizingProvider,
    Mt5SizingSupplementProvider, Mt5SizingSupplementSnapshot, RuntimeResult,
};

struct FixedSupplement(Mt5SizingSupplementSnapshot);

#[async_trait]
impl Mt5SizingSupplementProvider for FixedSupplement {
    async fn load_supplement(
        &self,
        _account_id: &AccountId,
        _instrument_id: &InstrumentId,
    ) -> RuntimeResult<Mt5SizingSupplementSnapshot> {
        Ok(self.0.clone())
    }
}

fn supplement(
    captured_at: chrono::DateTime<chrono::Utc>,
    account_id: &str,
    instrument_id: &str,
) -> Mt5SizingSupplementSnapshot {
    Mt5SizingSupplementSnapshot {
        account_id: AccountId::new(account_id).unwrap(),
        instrument_id: InstrumentId::new(instrument_id).unwrap(),
        inputs: Mt5SizingSupplement {
            estimated_cost_per_quantity: Decimal::new(2, 1),
            slippage_gap_buffer_per_quantity: Decimal::new(1, 1),
            pending_order_exposure: Decimal::from(100),
            available_margin: Decimal::from(1_000),
            pending_order_margin: Decimal::from(100),
            margin_per_quantity: Decimal::from(50),
            min_notional_account_currency: None,
        },
        captured_at,
        max_age_seconds: 30,
    }
}

async fn seed_snapshot(store: &TradingStore, captured_at: chrono::DateTime<chrono::Utc>) {
    let snapshot = json!({
        "terminal_id": "terminal-provider-01",
        "server": "broker-demo",
        "source": "mt5_python_readonly_v1",
        "sequence": 12,
        "captured_at": captured_at.to_rfc3339(),
        "account_currency": "USD",
        "instruments": [{
            "symbol": "EURUSD.a",
            "base_currency": "EUR",
            "quote_currency": "USD",
            "contract_size": "100000",
            "tick_size": "0.00001",
            "tick_value_account_currency": "1.00",
            "volume_min": "0.01",
            "volume_max": "100.00",
            "volume_step": "0.01"
        }],
        "quotes": [{
            "symbol": "EURUSD.a",
            "bid": "1.10000",
            "ask": "1.10010",
            "tick_time": captured_at.to_rfc3339()
        }]
    });
    store
        .persist_mt5_market_data_snapshot(
            "sizing-provider-account",
            "terminal-provider-01",
            "broker-demo",
            12,
            "mt5_python_readonly_v1",
            "USD",
            captured_at,
            snapshot,
        )
        .await
        .unwrap();
}

fn provider(
    store: TradingStore,
    supplement: Mt5SizingSupplementSnapshot,
) -> Mt5PersistedSizingProvider {
    let bindings = HashMap::from([(
        "EURUSD".to_string(),
        Mt5InstrumentBinding {
            symbol: "EURUSD.a".to_string(),
            pnl_conversion_path: "mt5_tick_value_account_currency".to_string(),
            notional_conversion_path: "EURUSD:USD".to_string(),
        },
    )]);
    Mt5PersistedSizingProvider::new(
        store,
        "terminal-provider-01".to_string(),
        "broker-demo".to_string(),
        "USD".to_string(),
        30,
        true,
        true,
        bindings,
        Arc::new(FixedSupplement(supplement)),
    )
    .unwrap()
}

#[sqlx::test(migrations = "../../migrations")]
async fn provider_uses_ask_for_buys_and_bid_for_sells(pool: PgPool) {
    let store = TradingStore::new(pool);
    let now = Utc::now();
    seed_snapshot(&store, now).await;
    let provider = provider(store, supplement(now, "sizing-provider-account", "EURUSD"));
    let account = AccountId::new("sizing-provider-account").unwrap();
    let instrument = InstrumentId::new("EURUSD").unwrap();

    let buy = provider
        .load_snapshot_for_side(&account, &instrument, OrderSide::Buy)
        .await
        .unwrap();
    let sell = provider
        .load_snapshot_for_side(&account, &instrument, OrderSide::Sell)
        .await
        .unwrap();

    assert_eq!(buy.entry_price, Some(Decimal::new(11001, 4)));
    assert_eq!(sell.entry_price, Some(Decimal::new(11000, 4)));
    assert_eq!(buy.sizing.quantity_step, Decimal::new(1, 2));
    assert_eq!(buy.source_id, "terminal-provider-01:mt5_python_readonly_v1:12");
}

#[sqlx::test(migrations = "../../migrations")]
async fn provider_fails_closed_on_stale_margin_and_exposure_supplement(pool: PgPool) {
    let store = TradingStore::new(pool);
    let now = Utc::now();
    seed_snapshot(&store, now).await;
    let provider = provider(
        store,
        supplement(
            now - Duration::seconds(60),
            "sizing-provider-account",
            "EURUSD",
        ),
    );
    let account = AccountId::new("sizing-provider-account").unwrap();
    let instrument = InstrumentId::new("EURUSD").unwrap();

    assert!(provider
        .load_snapshot_for_side(&account, &instrument, OrderSide::Buy)
        .await
        .is_err());
}

#[sqlx::test(migrations = "../../migrations")]
async fn provider_rejects_cross_instrument_supplement(pool: PgPool) {
    let store = TradingStore::new(pool);
    let now = Utc::now();
    seed_snapshot(&store, now).await;
    let provider = provider(
        store,
        supplement(now, "sizing-provider-account", "GBPUSD"),
    );
    let account = AccountId::new("sizing-provider-account").unwrap();
    let instrument = InstrumentId::new("EURUSD").unwrap();

    assert!(provider
        .load_snapshot_for_side(&account, &instrument, OrderSide::Buy)
        .await
        .is_err());
}

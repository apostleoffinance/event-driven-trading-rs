//! Persistence contract for the QOS-013 MT5 sizing-data consumer.
#![allow(clippy::unwrap_used)]
use persistence::{repos::mt5_market_data, TradingStore};
use serde_json::json;
use sqlx::PgPool;

#[sqlx::test(migrations = "../../migrations")]
async fn latest_market_snapshot_is_loaded_for_exact_account_and_terminal(pool: PgPool) {
    let store=TradingStore::new(pool.clone());
    let now=chrono::Utc::now();
    let snapshot=json!({
        "terminal_id":"terminal-sizing-01","server":"broker-demo","source":"mt5_python_readonly_v1",
        "sequence":9,"captured_at":now.to_rfc3339(),"account_currency":"USD",
        "instruments":[{"symbol":"EURUSD.a","base_currency":"EUR","quote_currency":"USD","contract_size":"100000","tick_size":"0.00001","tick_value_account_currency":"1.00","volume_min":"0.01","volume_max":"100.00","volume_step":"0.01"}],
        "quotes":[{"symbol":"EURUSD.a","bid":"1.10000","ask":"1.10010","tick_time":now.to_rfc3339()}]
    });
    store.persist_mt5_market_data_snapshot("account-sizing-01","terminal-sizing-01","broker-demo",9,"mt5_python_readonly_v1","USD",now,snapshot.clone()).await.unwrap();
    let record=mt5_market_data::load_latest(&pool,"account-sizing-01","terminal-sizing-01").await.unwrap().unwrap();
    assert_eq!(record.account_id,"account-sizing-01");
    assert_eq!(record.terminal_id,"terminal-sizing-01");
    assert_eq!(record.sequence,9);
    assert_eq!(record.account_currency,"USD");
    assert_eq!(record.snapshot,snapshot);
    assert!(mt5_market_data::load_latest(&pool,"other-account","terminal-sizing-01").await.unwrap().is_none());
}

#[sqlx::test(migrations = "../../migrations")]
async fn market_snapshot_loader_rejects_blank_identity(pool: PgPool) {
    assert!(mt5_market_data::load_latest(&pool," ","terminal-sizing-01").await.is_err());
}

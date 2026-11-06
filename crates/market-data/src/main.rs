use std::sync::Arc;

use market_data::config::Config;
use market_data::http::router_with_mt5;
use market_data::mt5_latest::{Mt5LatestService, Mt5Scope, Mt5SnapshotSource, PgMt5SnapshotSource};
use market_data::providers::binance::{BinanceConfig, BinanceProvider, ReqwestHttp};
use market_data::providers::demo::DemoProvider;
use market_data::providers::mt5::Mt5Provider;
use market_data::service::{Clock, MarketDataService, ServiceConfig, SystemClock};

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "market_data=info".into()),
        )
        .init();

    let config = match Config::from_env() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("invalid configuration: {e}");
            std::process::exit(2);
        }
    };

    let http = ReqwestHttp::new().expect("failed to build HTTP client");
    let binance = BinanceProvider::new(
        BinanceConfig {
            enabled: config.binance_enabled,
            base_url: config.binance_base_url.clone(),
            timeout: config.request_timeout,
        },
        http,
    );
    let clock: Arc<dyn Clock> = Arc::new(SystemClock);
    let service = Arc::new(MarketDataService::new(
        vec![
            Arc::new(DemoProvider),
            Arc::new(Mt5Provider),
            Arc::new(binance),
        ],
        ServiceConfig::default(),
        clock.clone(),
    ));

    // The pool connects lazily on the first request, so startup never touches the DB.
    let mt5 = match (
        &config.mt5.database_url,
        &config.mt5.account_id,
        &config.mt5.terminal_id,
    ) {
        (Some(url), Some(account_id), Some(terminal_id)) => {
            let source: Arc<dyn Mt5SnapshotSource> = match PgMt5SnapshotSource::new(url) {
                Ok(s) => Arc::new(s),
                Err(e) => {
                    eprintln!("invalid configuration: MARKET_DATA_DATABASE_URL: {e}");
                    std::process::exit(2);
                }
            };
            Mt5LatestService::new(
                Some(source),
                Some(Mt5Scope {
                    account_id: account_id.clone(),
                    terminal_id: terminal_id.clone(),
                }),
                clock,
                config.mt5.stale_after_seconds,
                config.request_timeout,
            )
        }
        _ => Mt5LatestService::unconfigured(clock),
    };
    let mt5_configured = config.mt5.is_fully_configured();

    let listener = tokio::net::TcpListener::bind(config.bind)
        .await
        .expect("failed to bind");
    tracing::info!(addr = %config.bind, binance_enabled = config.binance_enabled, mt5_configured, "market-data API listening (read-only)");
    axum::serve(
        listener,
        router_with_mt5(service, Arc::new(mt5), &config.cors_origins),
    )
    .with_graceful_shutdown(async {
        let _ = tokio::signal::ctrl_c().await;
    })
    .await
    .expect("server error");
}

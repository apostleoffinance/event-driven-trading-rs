#![recursion_limit = "256"]

use std::{env, net::SocketAddr};

use axum::{
    http::{header, HeaderValue, Method},
    routing::get,
    Router,
};
use persistence::{connect, migrate, TradingStore};
use tower_http::cors::{AllowOrigin, CorsLayer};

mod analytics;
mod attachments;
mod auth;
mod calculations;
mod dashboard;
mod excursions;
mod http;
mod imports;
mod intelligence;
mod psychology;
mod resources;
mod risk_policy;

#[derive(Clone)]
pub struct AppState {
    pub store: TradingStore,
    pub auth: auth::JwtConfig,
}

fn env_required(name: &str) -> String {
    env::var(name).unwrap_or_else(|_| {
        eprintln!("missing required environment variable: {name}");
        std::process::exit(2);
    })
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "journal_api=info,tower_http=warn".into()),
        )
        .init();

    let database_url = env_required("DATABASE_URL");
    let issuer = env_required("EDGENEST_JWT_ISSUER");
    let audience = env_required("EDGENEST_JWT_AUDIENCE");
    let public_key = env_required("EDGENEST_JWT_PUBLIC_KEY_PEM").replace("\\n", "\n");
    let bind: SocketAddr = env::var("JOURNAL_API_BIND")
        .unwrap_or_else(|_| "0.0.0.0:8080".into())
        .parse()
        .unwrap_or_else(|e| {
            eprintln!("invalid JOURNAL_API_BIND: {e}");
            std::process::exit(2);
        });
    let origins = env::var("JOURNAL_CORS_ORIGINS")
        .unwrap_or_else(|_| "https://edgenest-frontend.edgenest.workers.dev,http://localhost:3000,http://localhost:3107,http://127.0.0.1:8787".into())
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .filter_map(|s| s.parse::<HeaderValue>().ok())
        .collect::<Vec<_>>();

    let pool = connect(&database_url).await.unwrap_or_else(|e| {
        eprintln!("database connection failed: {e}");
        std::process::exit(2);
    });
    migrate(&pool).await.unwrap_or_else(|e| {
        eprintln!("database migrations failed: {e}");
        std::process::exit(2);
    });

    let state = AppState {
        store: TradingStore::new(pool),
        auth: auth::JwtConfig::new(public_key.as_bytes(), issuer, audience).unwrap_or_else(|e| {
            eprintln!("invalid JWT configuration: {e}");
            std::process::exit(2);
        }),
    };

    let cors = CorsLayer::new()
        .allow_origin(AllowOrigin::list(origins))
        .allow_methods([
            Method::GET,
            Method::POST,
            Method::PUT,
            Method::PATCH,
            Method::DELETE,
            Method::OPTIONS,
        ])
        .allow_headers([header::AUTHORIZATION, header::CONTENT_TYPE, header::ACCEPT]);

    let app = Router::new()
        .route("/healthz", get(http::health))
        .route("/readyz", get(http::ready))
        .route(
            "/api/v1/trading/accounts",
            get(dashboard::accounts).post(dashboard::create_paper_account),
        )
        .route("/api/v1/trading/accounts/{id}", get(dashboard::account))
        .route("/api/v1/trading/portfolio", get(dashboard::portfolio))
        .route("/api/v1/trading/strategies", get(dashboard::strategies))
        .route("/api/v1/trading/positions", get(dashboard::positions))
        .route("/api/v1/trading/orders", get(dashboard::orders))
        .route("/api/v1/trading/risk", get(dashboard::risk))
        .route(
            "/api/v1/trading/risk/decisions",
            get(dashboard::risk_decisions),
        )
        .route("/api/v1/trading/events", get(dashboard::events))
        .route(
            "/api/v1/trading/reconciliation",
            get(dashboard::reconciliation),
        )
        .route(
            "/api/v1/trading/system/components",
            get(dashboard::system_components),
        )
        .route(
            "/api/v1/trading/system/pipeline",
            get(dashboard::system_pipeline),
        )
        .route(
            "/api/v1/journal/accounts",
            get(resources::list_accounts).post(resources::create_account),
        )
        .route(
            "/api/v1/journal/accounts/{id}",
            get(resources::get_account)
                .put(resources::update_account)
                .patch(resources::update_account)
                .delete(resources::archive_account),
        )
        .route(
            "/api/v1/journal/accounts/{id}/risk-policy",
            get(risk_policy::get).put(risk_policy::put),
        )
        .route(
            "/api/v1/journal/accounts/{id}/risk-policy/evaluate",
            axum::routing::post(risk_policy::evaluate),
        )
        .route(
            "/api/v1/journal/accounts/{id}/migration-metadata",
            axum::routing::put(resources::update_account_migration_metadata),
        )
        .route(
            "/api/v1/journal/setups",
            get(resources::list_setups).post(resources::create_setup),
        )
        .route(
            "/api/v1/journal/setups/{id}",
            get(resources::get_setup)
                .put(resources::update_setup)
                .patch(resources::update_setup)
                .delete(resources::archive_setup),
        )
        .route("/api/v1/journal/analytics/summary", get(analytics::summary))
        .route(
            "/api/v1/journal/analytics/intelligence",
            get(intelligence::summary),
        )
        .route(
            "/api/v1/journal/import",
            axum::routing::post(imports::import_trades),
        )
        .route("/api/v1/journal/trades", get(http::list).post(http::create))
        .route(
            "/api/v1/journal/trades/{trade_id}/excursions",
            axum::routing::put(excursions::save),
        )
        .route(
            "/api/v1/journal/trades/{trade_id}/attachments",
            get(attachments::list).post(attachments::create),
        )
        .route(
            "/api/v1/journal/trades/{trade_id}/attachments/upload-url",
            axum::routing::post(attachments::create_upload_url),
        )
        .route(
            "/api/v1/journal/trades/{trade_id}/attachments/{attachment_id}/download-url",
            get(attachments::download_url),
        )
        .route(
            "/api/v1/journal/trades/{trade_id}/attachments/{attachment_id}",
            axum::routing::delete(attachments::delete),
        )
        .route(
            "/api/v1/journal/trades/{trade_id}/psychology",
            get(psychology::get).put(psychology::upsert),
        )
        .route(
            "/api/v1/journal/trades/{trade_id}",
            get(http::get_one)
                .put(http::replace)
                .patch(http::replace)
                .delete(http::delete),
        )
        .with_state(state)
        .layer(cors);

    let listener = tokio::net::TcpListener::bind(bind)
        .await
        .unwrap_or_else(|e| {
            eprintln!("failed to bind {bind}: {e}");
            std::process::exit(2);
        });
    tracing::info!(%bind, "EdgeNest Journal API listening");
    axum::serve(listener, app)
        .with_graceful_shutdown(async {
            let _ = tokio::signal::ctrl_c().await;
        })
        .await
        .unwrap_or_else(|e| {
            eprintln!("server error: {e}");
            std::process::exit(1);
        });
}

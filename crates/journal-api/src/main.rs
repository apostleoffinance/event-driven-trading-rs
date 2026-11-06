use std::{env, net::SocketAddr};

use axum::{
    http::{header, HeaderValue, Method},
    routing::get,
    Router,
};
use persistence::{connect, migrate, TradingStore};
use tower_http::cors::{AllowOrigin, CorsLayer};

mod auth;
mod http;
mod resources;

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
    dotenvy::dotenv().ok();
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
    let active_key_id =
        env::var("EDGENEST_JWT_KEY_ID").unwrap_or_else(|_| "edgenest-rsa-2026-01".into());
    let public_keys_json = env::var("EDGENEST_JWT_PUBLIC_KEYS_JSON").ok();
    let bind: SocketAddr = env::var("JOURNAL_API_BIND")
        .unwrap_or_else(|_| "0.0.0.0:8080".into())
        .parse()
        .unwrap_or_else(|e| {
            eprintln!("invalid JOURNAL_API_BIND: {e}");
            std::process::exit(2);
        });
    let origins = env::var("JOURNAL_CORS_ORIGINS")
        .unwrap_or_default()
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
        auth: auth::JwtConfig::new(
            public_key.as_bytes(),
            active_key_id,
            public_keys_json.as_deref(),
            issuer,
            audience,
        )
        .unwrap_or_else(|e| {
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
        .allow_headers([header::AUTHORIZATION, header::CONTENT_TYPE, header::ACCEPT])
        .allow_credentials(true);

    let app = Router::new()
        .route("/healthz", get(http::health))
        .route("/readyz", get(http::ready))
        .route(
            "/api/v1/journal/accounts",
            get(resources::list_accounts).post(resources::create_account),
        )
        .route(
            "/api/v1/journal/accounts/:id",
            get(resources::get_account)
                .put(resources::update_account)
                .patch(resources::update_account)
                .delete(resources::archive_account),
        )
        .route(
            "/api/v1/journal/setups",
            get(resources::list_setups).post(resources::create_setup),
        )
        .route(
            "/api/v1/journal/setups/:id",
            get(resources::get_setup)
                .put(resources::update_setup)
                .patch(resources::update_setup)
                .delete(resources::archive_setup),
        )
        .route("/api/v1/journal/trades", get(http::list).post(http::create))
        .route(
            "/api/v1/journal/trades/:trade_id",
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

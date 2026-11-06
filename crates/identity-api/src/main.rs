use std::{env, net::SocketAddr, sync::Arc};

use axum::{
    http::{header, HeaderValue, Method},
    routing::{get, post},
    Router,
};
use sqlx::PgPool;
use tower_http::cors::{AllowOrigin, CorsLayer};

mod service;

#[derive(Clone)]
pub struct AppState {
    pub pool: PgPool,
    pub jwt: Arc<service::JwtService>,
    pub access_ttl_seconds: i64,
    pub refresh_ttl_days: i64,
}

fn required(name: &str) -> String {
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
                .unwrap_or_else(|_| "identity_api=info,tower_http=warn".into()),
        )
        .init();

    let database_url = required("DATABASE_URL");
    let issuer = required("EDGENEST_JWT_ISSUER");
    let audience = required("EDGENEST_JWT_AUDIENCE");
    let private_key = required("EDGENEST_JWT_PRIVATE_KEY_PEM").replace("\\n", "\n");
    let public_key = required("EDGENEST_JWT_PUBLIC_KEY_PEM").replace("\\n", "\n");
    let access_ttl_seconds = env::var("EDGENEST_ACCESS_TOKEN_TTL_SECONDS")
        .unwrap_or_else(|_| "900".into())
        .parse::<i64>()
        .unwrap_or(900)
        .clamp(60, 3600);
    let refresh_ttl_days = env::var("EDGENEST_REFRESH_TOKEN_TTL_DAYS")
        .unwrap_or_else(|_| "30".into())
        .parse::<i64>()
        .unwrap_or(30)
        .clamp(1, 90);
    let bind: SocketAddr = env::var("IDENTITY_API_BIND")
        .unwrap_or_else(|_| "0.0.0.0:8081".into())
        .parse()
        .unwrap_or_else(|e| {
            eprintln!("invalid IDENTITY_API_BIND: {e}");
            std::process::exit(2);
        });
    let origins = env::var("IDENTITY_CORS_ORIGINS")
        .unwrap_or_default()
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .filter_map(|s| s.parse::<HeaderValue>().ok())
        .collect::<Vec<_>>();

    let pool = persistence::connect(&database_url).await.unwrap_or_else(|e| {
        eprintln!("database connection failed: {e}");
        std::process::exit(2);
    });
    persistence::migrate(&pool).await.unwrap_or_else(|e| {
        eprintln!("database migrations failed: {e}");
        std::process::exit(2);
    });
    let jwt = service::JwtService::new(private_key.as_bytes(), public_key.as_bytes(), issuer, audience)
        .unwrap_or_else(|e| {
            eprintln!("invalid EdgeNest JWT key configuration: {e}");
            std::process::exit(2);
        });

    let state = AppState {
        pool,
        jwt: Arc::new(jwt),
        access_ttl_seconds,
        refresh_ttl_days,
    };
    let cors = CorsLayer::new()
        .allow_origin(AllowOrigin::list(origins))
        .allow_methods([Method::GET, Method::POST, Method::OPTIONS])
        .allow_headers([header::AUTHORIZATION, header::CONTENT_TYPE, header::ACCEPT]);

    let app = Router::new()
        .route("/healthz", get(service::health))
        .route("/readyz", get(service::ready))
        .route("/api/v1/auth/register", post(service::register))
        .route("/api/v1/auth/login", post(service::login))
        .route("/api/v1/auth/refresh", post(service::refresh))
        .route("/api/v1/auth/logout", post(service::logout))
        .route("/api/v1/auth/me", get(service::me))
        .with_state(state)
        .layer(cors);

    let listener = tokio::net::TcpListener::bind(bind).await.unwrap_or_else(|e| {
        eprintln!("failed to bind {bind}: {e}");
        std::process::exit(2);
    });
    tracing::info!(%bind, "EdgeNest Identity API listening");
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

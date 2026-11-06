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
    pub resend_api_key: Option<String>,
    pub email_from: String,
    pub public_app_url: String,
    pub mfa_encryption_key: String,
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
    let resend_api_key = env::var("RESEND_API_KEY").ok().filter(|v| !v.trim().is_empty());
    let email_from = env::var("IDENTITY_EMAIL_FROM").unwrap_or_else(|_| "EdgeNest <no-reply@edgenest.app>".into());
    let public_app_url = env::var("EDGENEST_PUBLIC_APP_URL").unwrap_or_else(|_| "https://app.edgenest.app".into());
    let mfa_encryption_key = required("IDENTITY_MFA_ENCRYPTION_KEY");
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

    let pool = persistence::connect(&database_url)
        .await
        .unwrap_or_else(|e| {
            eprintln!("database connection failed: {e}");
            std::process::exit(2);
        });
    persistence::migrate(&pool).await.unwrap_or_else(|e| {
        eprintln!("database migrations failed: {e}");
        std::process::exit(2);
    });
    let jwt = service::JwtService::new(
        private_key.as_bytes(),
        public_key.as_bytes(),
        issuer,
        audience,
    )
    .unwrap_or_else(|e| {
        eprintln!("invalid EdgeNest JWT key configuration: {e}");
        std::process::exit(2);
    });

    let state = AppState {
        pool,
        jwt: Arc::new(jwt),
        access_ttl_seconds,
        refresh_ttl_days,
        resend_api_key,
        email_from,
        public_app_url,
        mfa_encryption_key,
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
        .route("/api/v1/auth/verify-email", post(service::verify_email))
        .route("/api/v1/auth/resend-verification", post(service::resend_verification))
        .route("/api/v1/auth/password-reset/request", post(service::request_password_reset))
        .route("/api/v1/auth/password-reset/confirm", post(service::confirm_password_reset))
        .route("/api/v1/auth/mfa/setup", post(service::mfa_setup))
        .route("/api/v1/auth/mfa/confirm", post(service::mfa_confirm))
        .route("/api/v1/auth/mfa/disable", post(service::mfa_disable))
        .route("/api/v1/auth/mfa/verify-login", post(service::mfa_verify_login))
        .route("/.well-known/jwks.json", get(service::jwks))
        .with_state(state)
        .layer(cors);

    let listener = tokio::net::TcpListener::bind(bind)
        .await
        .unwrap_or_else(|e| {
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

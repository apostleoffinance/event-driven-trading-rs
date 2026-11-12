use std::{env, process::Stdio, time::Duration};

use axum::{
    body::Body,
    extract::State,
    http::{header, HeaderValue, Request, Response, StatusCode},
    middleware::{self, Next},
    response::IntoResponse,
    routing::get,
    Router,
};
use reqwest::Client;
use tokio::{
    process::{Child, Command},
    time::sleep,
};

#[derive(Clone)]
struct GatewayState {
    client: Client,
}

fn start_child(name: &str, executable: &str, bind_key: &str, bind: &str) -> Child {
    let mut command = Command::new(executable);
    command
        .env(bind_key, bind)
        .stdin(Stdio::null())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit());
    let cors = env::var("EDGENEST_CORS_ORIGINS")
        .unwrap_or_else(|_| "https://edgenest-frontend.edgenest.workers.dev,http://localhost:3000,http://localhost:3107,http://127.0.0.1:8787".into());
    if !cors.trim().is_empty() {
        command
            .env("IDENTITY_CORS_ORIGINS", &cors)
            .env("JOURNAL_CORS_ORIGINS", &cors)
            .env("MARKET_DATA_CORS_ORIGINS", &cors);
    }
    let child = command.spawn().unwrap_or_else(|e| {
        eprintln!("failed to start {name}: {e}");
        std::process::exit(1);
    });
    tracing::info!(service = name, "child API process started");
    child
}

async fn health() -> &'static str {
    "ok"
}

/// The gateway owns /healthz and /readyz rather than proxying them to a child
/// API. Apply the configured origin allow-list so browser health checks are
/// not incorrectly reported as unavailable due to a CORS failure.
async fn cors_headers(request: Request<Body>, next: Next) -> Response<Body> {
    let origin = request.headers().get(header::ORIGIN).cloned();
    let mut response = next.run(request).await;
    if let Some(origin) = origin {
        let origin_text = origin.to_str().unwrap_or_default();
        let allowed_origins = env::var("EDGENEST_CORS_ORIGINS")
            .unwrap_or_else(|_| "https://edgenest-frontend.edgenest.workers.dev,http://localhost:3000,http://localhost:3107,http://127.0.0.1:8787".into());
        let allowed = allowed_origins.split(',').any(|allowed| allowed.trim() == origin_text);
        if allowed {
            response.headers_mut().insert(header::ACCESS_CONTROL_ALLOW_ORIGIN, origin);
            response.headers_mut().append(header::VARY, HeaderValue::from_static("Origin"));
        }
    }
    response
}

/// Report readiness only after the Identity, Journal, and Market Data services
/// are accepting requests. Identity and Journal expose DB-backed /readyz;
/// Market Data currently exposes /healthz only.
async fn readiness(State(state): State<GatewayState>) -> Response<Body> {
    let checks = [
        ("identity-api", "http://127.0.0.1:8081/readyz"),
        ("journal-api", "http://127.0.0.1:8080/readyz"),
        ("market-data-api", "http://127.0.0.1:8787/healthz"),
    ];

    for (service, url) in checks {
        match state.client.get(url).send().await {
            Ok(response) if response.status().is_success() => {}
            Ok(response) => {
                tracing::warn!(service, status = %response.status(), "upstream readiness check failed");
                return (StatusCode::SERVICE_UNAVAILABLE, "EdgeNest API not ready").into_response();
            }
            Err(error) => {
                tracing::warn!(service, %error, "upstream readiness request failed");
                return (StatusCode::SERVICE_UNAVAILABLE, "EdgeNest API not ready").into_response();
            }
        }
    }

    (StatusCode::OK, "ready").into_response()
}

async fn proxy(State(state): State<GatewayState>, request: Request<Body>) -> Response<Body> {
    let path = request.uri().path().to_owned();
    let target = if path.starts_with("/api/v1/auth/") {
        "http://127.0.0.1:8081"
    } else if path.starts_with("/api/v1/journal/") || path.starts_with("/api/v1/trading/") {
        "http://127.0.0.1:8080"
    } else if path.starts_with("/api/v1/market-data/") {
        "http://127.0.0.1:8787"
    } else {
        return (StatusCode::NOT_FOUND, "unknown EdgeNest API route").into_response();
    };

    let (parts, body) = request.into_parts();
    let uri = format!(
        "{target}{}{}",
        parts.uri.path(),
        parts
            .uri
            .query()
            .map(|q| format!("?{q}"))
            .unwrap_or_default()
    );
    let mut upstream = state.client.request(parts.method, uri);
    for (name, value) in parts.headers.iter() {
        if name != header::HOST && name != header::CONNECTION && name != header::TRANSFER_ENCODING {
            upstream = upstream.header(name, value);
        }
    }

    let body = match axum::body::to_bytes(body, 10 * 1024 * 1024).await {
        Ok(body) => body,
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                "request body too large or unreadable",
            )
                .into_response()
        }
    };
    let upstream = match upstream.body(body).send().await {
        Ok(response) => response,
        Err(error) => {
            tracing::error!(%error, path, "upstream API request failed");
            return (StatusCode::BAD_GATEWAY, "upstream API unavailable").into_response();
        }
    };

    let status = upstream.status();
    let headers = upstream.headers().clone();
    let bytes = match upstream.bytes().await {
        Ok(bytes) => bytes,
        Err(_) => {
            return (StatusCode::BAD_GATEWAY, "failed to read upstream response").into_response()
        }
    };
    let mut response = Response::builder().status(status);
    for (name, value) in headers.iter() {
        if name != header::CONNECTION && name != header::TRANSFER_ENCODING {
            response = response.header(name, value);
        }
    }
    response.body(Body::from(bytes)).unwrap_or_else(|_| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "failed to construct response",
        )
            .into_response()
    })
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "api_gateway=info".into()),
        )
        .init();

    // Keep API processes on loopback; only this gateway is internet-facing.
    let mut identity = start_child(
        "identity-api",
        "./target/release/identity-api",
        "IDENTITY_API_BIND",
        "127.0.0.1:8081",
    );
    let mut journal = start_child(
        "journal-api",
        "./target/release/journal-api",
        "JOURNAL_API_BIND",
        "127.0.0.1:8080",
    );
    let mut market = start_child(
        "market-data-api",
        "./target/release/market-data-api",
        "MARKET_DATA_BIND",
        "127.0.0.1:8787",
    );

    // Give services a brief opportunity to start; readiness endpoints are exposed via the gateway.
    sleep(Duration::from_millis(500)).await;

    let client = Client::builder()
        .timeout(Duration::from_secs(30))
        .build()
        .unwrap_or_else(|e| {
            eprintln!("failed to create gateway HTTP client: {e}");
            std::process::exit(1);
        });
    let app = Router::new()
        .route("/healthz", get(health))
        .route("/readyz", get(readiness))
        .fallback(proxy)
        .with_state(GatewayState { client })
        .layer(middleware::from_fn(cors_headers));

    let port = env::var("PORT").unwrap_or_else(|_| "10000".into());
    let bind = format!("0.0.0.0:{port}");
    let listener = tokio::net::TcpListener::bind(&bind)
        .await
        .unwrap_or_else(|e| {
            eprintln!("gateway failed to bind {bind}: {e}");
            std::process::exit(1);
        });
    tracing::info!(%bind, "EdgeNest combined API gateway listening");

    let server = axum::serve(listener, app).with_graceful_shutdown(async {
        let _ = tokio::signal::ctrl_c().await;
    });
    tokio::select! {
        result = server => {
            if let Err(error) = result {
                eprintln!("gateway server error: {error}");
            }
        }
        status = identity.wait() => eprintln!("identity-api exited unexpectedly: {status:?}"),
        status = journal.wait() => eprintln!("journal-api exited unexpectedly: {status:?}"),
        status = market.wait() => eprintln!("market-data-api exited unexpectedly: {status:?}"),
    }
    let _ = identity.kill().await;
    let _ = journal.kill().await;
    let _ = market.kill().await;
}

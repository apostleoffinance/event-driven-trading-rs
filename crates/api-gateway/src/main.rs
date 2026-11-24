use std::{env, process::Stdio, time::Duration};

use axum::{
    body::Body,
    extract::State,
    http::{header, HeaderMap, HeaderValue, Method, Request, Response, StatusCode},
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

const DEFAULT_CORS_ORIGINS: &str = "https://edgenest-frontend.edgenest.workers.dev,http://localhost:3000,http://localhost:3107,http://127.0.0.1:8787";
const CORS_ALLOWED_METHODS: &str = "GET, POST, PUT, PATCH, DELETE, OPTIONS";
const CORS_ALLOWED_HEADERS: &str = "authorization, content-type, accept, x-organization-id";

fn configured_cors_origins() -> Vec<String> {
    env::var("EDGENEST_CORS_ORIGINS")
        .unwrap_or_else(|_| DEFAULT_CORS_ORIGINS.to_owned())
        .split(',')
        .map(str::trim)
        .filter(|origin| !origin.is_empty())
        .map(str::to_owned)
        .collect()
}

fn origin_is_allowed(origin: &HeaderValue, allowed_origins: &[String]) -> bool {
    origin
        .to_str()
        .map(|value| allowed_origins.iter().any(|allowed| allowed == value))
        .unwrap_or(false)
}

fn preflight_is_allowed(headers: &HeaderMap) -> bool {
    let Some(method) = headers
        .get(header::ACCESS_CONTROL_REQUEST_METHOD)
        .and_then(|value| value.to_str().ok())
    else {
        return false;
    };
    let method_allowed = matches!(
        method,
        "GET" | "POST" | "PUT" | "PATCH" | "DELETE" | "OPTIONS"
    );
    if !method_allowed {
        return false;
    }

    let Some(requested_headers) = headers
        .get(header::ACCESS_CONTROL_REQUEST_HEADERS)
        .and_then(|value| value.to_str().ok())
    else {
        return true;
    };
    requested_headers.split(',').all(|name| {
        matches!(
            name.trim().to_ascii_lowercase().as_str(),
            "authorization" | "content-type" | "accept" | "x-organization-id"
        )
    })
}

fn preflight_response(
    origin: Option<HeaderValue>,
    request_headers: &HeaderMap,
    allowed_origins: &[String],
) -> Response<Body> {
    let allowed_origin = origin
        .as_ref()
        .is_some_and(|value| origin_is_allowed(value, allowed_origins));
    if !allowed_origin || !preflight_is_allowed(request_headers) {
        return StatusCode::FORBIDDEN.into_response();
    }

    let mut response = Response::builder()
        .status(StatusCode::NO_CONTENT)
        .body(Body::empty())
        .expect("static CORS preflight response is valid");
    let headers = response.headers_mut();
    headers.insert(
        header::ACCESS_CONTROL_ALLOW_ORIGIN,
        origin.expect("allowed origin was checked"),
    );
    headers.insert(
        header::ACCESS_CONTROL_ALLOW_METHODS,
        HeaderValue::from_static(CORS_ALLOWED_METHODS),
    );
    headers.insert(
        header::ACCESS_CONTROL_ALLOW_HEADERS,
        HeaderValue::from_static(CORS_ALLOWED_HEADERS),
    );
    headers.insert(
        header::ACCESS_CONTROL_MAX_AGE,
        HeaderValue::from_static("600"),
    );
    headers.insert(
        header::VARY,
        HeaderValue::from_static(
            "Origin, Access-Control-Request-Method, Access-Control-Request-Headers",
        ),
    );
    response
}

/// The gateway terminates browser CORS for every route, including its own
/// health endpoints. Preflight requests are answered here instead of being
/// forwarded to an upstream service (which may not expose OPTIONS routes).
async fn cors_headers(request: Request<Body>, next: Next) -> Response<Body> {
    let allowed_origins = configured_cors_origins();
    let origin = request.headers().get(header::ORIGIN).cloned();

    if request.method() == Method::OPTIONS
        && request
            .headers()
            .contains_key(header::ACCESS_CONTROL_REQUEST_METHOD)
    {
        return preflight_response(origin, request.headers(), &allowed_origins);
    }

    let mut response = next.run(request).await;
    if let Some(origin) = origin {
        if origin_is_allowed(&origin, &allowed_origins) {
            response
                .headers_mut()
                .insert(header::ACCESS_CONTROL_ALLOW_ORIGIN, origin);
            response
                .headers_mut()
                .append(header::VARY, HeaderValue::from_static("Origin"));
        }
    }
    response
}

/// Resolve the owning service for each public API route. Organization
/// resources live in Journal API alongside tenant membership and entitlements.
fn upstream_for_path(path: &str) -> Option<&'static str> {
    if path.starts_with("/api/v1/auth/") {
        Some("http://127.0.0.1:8081")
    } else if path == "/api/v1/organizations"
        || path.starts_with("/api/v1/organizations/")
        || path.starts_with("/api/v1/journal/")
        || path.starts_with("/api/v1/trading/")
    {
        Some("http://127.0.0.1:8080")
    } else if path.starts_with("/api/v1/market-data/") {
        Some("http://127.0.0.1:8787")
    } else {
        None
    }
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
    let Some(target) = upstream_for_path(&path) else {
        tracing::warn!(path, "no upstream route registered");
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


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn organization_collection_and_nested_routes_are_forwarded_to_journal() {
        assert_eq!(
            upstream_for_path("/api/v1/organizations"),
            Some("http://127.0.0.1:8080")
        );
        assert_eq!(
            upstream_for_path("/api/v1/organizations/org-123/members"),
            Some("http://127.0.0.1:8080")
        );
        assert_eq!(
            upstream_for_path("/api/v1/organizations/org-123/account-entitlements"),
            Some("http://127.0.0.1:8080")
        );
    }

    #[test]
    fn existing_service_routes_keep_their_upstream() {
        assert_eq!(
            upstream_for_path("/api/v1/auth/me"),
            Some("http://127.0.0.1:8081")
        );
        assert_eq!(
            upstream_for_path("/api/v1/journal/accounts"),
            Some("http://127.0.0.1:8080")
        );
        assert_eq!(
            upstream_for_path("/api/v1/trading/accounts"),
            Some("http://127.0.0.1:8080")
        );
        assert_eq!(
            upstream_for_path("/api/v1/market-data/quotes"),
            Some("http://127.0.0.1:8787")
        );
        assert_eq!(upstream_for_path("/api/v1/not-registered"), None);
    }

    #[test]
    fn cors_origin_matching_is_exact() {
        let allowed = vec!["https://dashboard.example.com".to_owned()];
        let valid = HeaderValue::from_static("https://dashboard.example.com");
        let lookalike = HeaderValue::from_static("https://dashboard.example.com.attacker.test");
        assert!(origin_is_allowed(&valid, &allowed));
        assert!(!origin_is_allowed(&lookalike, &allowed));
    }

    #[test]
    fn preflight_allows_authorization_and_tenant_header() {
        let mut headers = HeaderMap::new();
        headers.insert(
            header::ACCESS_CONTROL_REQUEST_METHOD,
            HeaderValue::from_static("GET"),
        );
        headers.insert(
            header::ACCESS_CONTROL_REQUEST_HEADERS,
            HeaderValue::from_static("authorization, x-organization-id"),
        );
        assert!(preflight_is_allowed(&headers));
    }

    #[test]
    fn preflight_response_returns_required_browser_cors_headers() {
        let mut request_headers = HeaderMap::new();
        request_headers.insert(
            header::ACCESS_CONTROL_REQUEST_METHOD,
            HeaderValue::from_static("GET"),
        );
        request_headers.insert(
            header::ACCESS_CONTROL_REQUEST_HEADERS,
            HeaderValue::from_static("authorization, x-organization-id"),
        );
        let allowed = vec!["https://dashboard.example.com".to_owned()];
        let response = preflight_response(
            Some(HeaderValue::from_static("https://dashboard.example.com")),
            &request_headers,
            &allowed,
        );

        assert_eq!(response.status(), StatusCode::NO_CONTENT);
        assert_eq!(
            response.headers()[header::ACCESS_CONTROL_ALLOW_ORIGIN],
            "https://dashboard.example.com"
        );
        assert_eq!(
            response.headers()[header::ACCESS_CONTROL_ALLOW_METHODS],
            CORS_ALLOWED_METHODS
        );
        assert_eq!(
            response.headers()[header::ACCESS_CONTROL_ALLOW_HEADERS],
            CORS_ALLOWED_HEADERS
        );
        assert_eq!(response.headers()[header::ACCESS_CONTROL_MAX_AGE], "600");
    }

    #[test]
    fn preflight_response_rejects_untrusted_origin() {
        let mut request_headers = HeaderMap::new();
        request_headers.insert(
            header::ACCESS_CONTROL_REQUEST_METHOD,
            HeaderValue::from_static("GET"),
        );
        let allowed = vec!["https://dashboard.example.com".to_owned()];
        let response = preflight_response(
            Some(HeaderValue::from_static("https://attacker.example")),
            &request_headers,
            &allowed,
        );
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
    }

    #[test]
    fn preflight_rejects_unapproved_methods_and_headers() {
        let mut headers = HeaderMap::new();
        headers.insert(
            header::ACCESS_CONTROL_REQUEST_METHOD,
            HeaderValue::from_static("TRACE"),
        );
        assert!(!preflight_is_allowed(&headers));

        headers.insert(
            header::ACCESS_CONTROL_REQUEST_METHOD,
            HeaderValue::from_static("GET"),
        );
        headers.insert(
            header::ACCESS_CONTROL_REQUEST_HEADERS,
            HeaderValue::from_static("authorization, x-secret-token"),
        );
        assert!(!preflight_is_allowed(&headers));
    }
}

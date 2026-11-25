#![recursion_limit = "256"]

use std::{env, net::SocketAddr};

use axum::{
    extract::State,
    http::{header, HeaderValue, Method, Request, StatusCode},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::get,
    Router,
};
use persistence::{connect, migrate, TradingStore};
use tower_http::cors::{AllowOrigin, CorsLayer};

mod analytics;
mod attachments;
mod auth;
mod broker_registry;
mod calculations;
mod dashboard;
mod excursions;
mod http;
mod imports;
mod intelligence;
mod organizations_api;
mod psychology;
mod resources;
mod risk_policy;
mod saved_views;
mod workspace_api;

#[derive(Clone)]
pub struct AppState {
    pub store: TradingStore,
    pub auth: auth::JwtConfig,
    pub broker_identity_hmac: Option<broker_registry::BrokerIdentityHmac>,
}

fn env_required(name: &str) -> String {
    env::var(name).unwrap_or_else(|_| {
        eprintln!("missing required environment variable: {name}");
        std::process::exit(2);
    })
}

fn build_router(state: AppState, origins: Vec<HeaderValue>) -> Router {
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
        .allow_headers([
            header::AUTHORIZATION,
            header::CONTENT_TYPE,
            header::ACCEPT,
            axum::http::HeaderName::from_static("x-organization-id"),
            axum::http::HeaderName::from_static("x-workspace-id"),
        ]);

    Router::new()
        .route(
            "/api/v1/organizations",
            get(organizations_api::list).post(organizations_api::create),
        )
        .route(
            "/api/v1/organizations/{org}/workspaces",
            get(workspace_api::list).post(workspace_api::create),
        )
        .route(
            "/api/v1/organizations/{org}/workspaces/{workspace}",
            get(workspace_api::get).patch(workspace_api::update),
        )
        .route(
            "/api/v1/organizations/{org}/workspaces/{workspace}/archive",
            axum::routing::post(workspace_api::archive),
        )
        .route(
            "/api/v1/organizations/{org}/workspaces/{workspace}/default",
            axum::routing::post(workspace_api::set_default),
        )
        .route(
            "/api/v1/organizations/{org}/workspaces/{workspace}/members",
            get(workspace_api::list_grants).post(workspace_api::create_grant),
        )
        .route(
            "/api/v1/organizations/{org}/workspaces/{workspace}/members/{user}",
            axum::routing::patch(workspace_api::update_grant),
        )
        .route(
            "/api/v1/organizations/{org}/workspaces/{workspace}/accounts/{account}/assignment",
            axum::routing::put(workspace_api::assign_account),
        )
        .route(
            "/api/v1/organizations/{org}/members",
            get(organizations_api::members).post(organizations_api::add_member),
        )
        .route(
            "/api/v1/organizations/{org}/members/{user}",
            axum::routing::patch(organizations_api::update_member),
        )
        .route(
            "/api/v1/organizations/{org}",
            axum::routing::patch(organizations_api::update),
        )
        .route(
            "/api/v1/organizations/{org}/owner-transfer",
            axum::routing::post(organizations_api::transfer_owner),
        )
        .route(
            "/api/v1/organizations/{org}/account-entitlements",
            get(organizations_api::entitlements),
        )
        .route(
            "/api/v1/organizations/{org}/account-entitlements/{account}",
            axum::routing::put(organizations_api::assign_entitlement)
                .patch(organizations_api::update_entitlement),
        )
        .route(
            "/api/v1/broker-registry/catalog",
            get(broker_registry::catalog),
        )
        .route(
            "/api/v1/organizations/{org}/broker-account-bindings",
            get(broker_registry::bindings).post(broker_registry::register),
        )
        .route(
            "/api/v1/organizations/{org}/broker-account-bindings/{binding}",
            axum::routing::patch(broker_registry::transition),
        )
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
            "/api/v1/journal/accounts/{id}/trading-link",
            axum::routing::put(resources::link_trading_account),
        )
        .route(
            "/api/v1/journal/portfolio",
            get(resources::journal_portfolio),
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
            "/api/v1/journal/analytics/saved-views",
            get(saved_views::list).post(saved_views::upsert),
        )
        .route(
            "/api/v1/journal/analytics/saved-views/{id}",
            axum::routing::delete(saved_views::delete),
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
            "/api/v1/journal/trades/{trade_id}/attachments/upload",
            axum::routing::post(attachments::upload)
                .layer(axum::extract::DefaultBodyLimit::max(15 * 1024 * 1024)),
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
        .route_layer(middleware::from_fn_with_state(state.clone(), session_guard))
        .with_state(state)
        .layer(cors)
}

async fn session_guard(
    State(state): State<AppState>,
    request: Request<axum::body::Body>,
    next: Next,
) -> Response {
    if !request.uri().path().starts_with("/api/") {
        return next.run(request).await;
    }
    let claims = match state.auth.authenticate(request.headers()) {
        Ok(claims) => claims,
        Err(error) => return error.into_response(),
    };
    match state.store.validate_access_token(&claims).await {
        Ok(true) => next.run(request).await,
        Ok(false) => (StatusCode::UNAUTHORIZED, axum::Json(serde_json::json!({
            "error": {"code":"unauthorized", "message":"The identity session is not active"}
        }))).into_response(),
        Err(_) => (StatusCode::SERVICE_UNAVAILABLE, axum::Json(serde_json::json!({
            "error": {"code":"identity_unavailable", "message":"Session state could not be verified"}
        }))).into_response(),
    }
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
    let migrator_database_url = env_required("MIGRATOR_DATABASE_URL");
    let issuer = env_required("EDGENEST_JWT_ISSUER");
    let audience = env_required("EDGENEST_JWT_AUDIENCE");
    let verification_keys = env_required("EDGENEST_JWT_VERIFICATION_KEY_FILES");
    let auth =
        auth::JwtConfig::from_files(&verification_keys, issuer, audience).unwrap_or_else(|error| {
            eprintln!("invalid JWT verification key-ring configuration: {error}");
            std::process::exit(2);
        });
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

    let migrator_pool = connect(&migrator_database_url).await.unwrap_or_else(|e| {
        eprintln!("migration database connection failed: {e}");
        std::process::exit(2);
    });
    migrate(&migrator_pool).await.unwrap_or_else(|e| {
        eprintln!("database migrations failed: {e}");
        std::process::exit(2);
    });
    migrator_pool.close().await;
    let pool = connect(&database_url).await.unwrap_or_else(|e| {
        eprintln!("runtime database connection failed: {e}");
        std::process::exit(2);
    });
    persistence::validate_identity_runtime_role(&pool)
        .await
        .unwrap_or_else(|e| {
            eprintln!("database runtime role is not sufficiently restricted: {e}");
            std::process::exit(2);
        });

    let state = AppState {
        store: TradingStore::new(pool),
        auth,
        broker_identity_hmac: broker_registry::BrokerIdentityHmac::from_environment(),
    };

    let app = build_router(state, origins);

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

#[cfg(test)]
mod qos021a_e3a_r2_api_tests {
    #![allow(clippy::unwrap_used)]

    use crate::{auth, build_router, AppState};
    use axum::{
        body::Body,
        http::{Method, Request, StatusCode},
        Router,
    };
    use chrono::{Duration, Utc};
    use http_body_util::BodyExt;
    use identity_token::AccessClaims;
    use jsonwebtoken::{encode, Algorithm, EncodingKey, Header};
    use persistence::{migrate, TradingStore};
    use serde_json::{json, Value};
    use sha2::{Digest, Sha256};
    use sqlx::PgPool;
    use tower::ServiceExt;

    const PRIVATE_KEY: &str = include_str!("../tests/fixtures/r2_test_private.pem");
    const PUBLIC_KEY: &str = include_str!("../tests/fixtures/r2_test_public.pem");

    fn token(subject: &str, expires: chrono::DateTime<Utc>) -> (String, AccessClaims) {
        let claims = AccessClaims {
            sub: subject.to_owned(),
            iss: "qos021a-r2-test".into(),
            aud: "qos021a-r2-api".into(),
            exp: expires.timestamp() as usize,
            iat: Utc::now().timestamp() as usize,
            token_type: "access".into(),
            jti: uuid::Uuid::new_v4().to_string(),
            session_id: hex::encode(Sha256::digest(subject.as_bytes())),
            session_revision: 1,
        };
        let mut header = Header::new(Algorithm::RS256);
        header.kid = Some("test-key".into());
        let token = encode(
            &header,
            &claims,
            &EncodingKey::from_rsa_pem(PRIVATE_KEY.as_bytes()).unwrap(),
        )
        .unwrap();
        (token, claims)
    }

    async fn persist_test_token(pool: &PgPool, claims: &AccessClaims) {
        let now = Utc::now();
        let session_expires = now + Duration::days(30);
        sqlx::query("INSERT INTO identity_sessions(id,user_id,revision,status,issued_at,expires_at,last_used_at) VALUES($1,$2,1,'ACTIVE',$3,$4,$3) ON CONFLICT(id) DO NOTHING")
            .bind(&claims.session_id).bind(&claims.sub).bind(now - Duration::minutes(1)).bind(session_expires).execute(pool).await.unwrap();
        let expires = chrono::DateTime::<Utc>::from_timestamp(claims.exp as i64, 0).unwrap();
        if expires > now {
            sqlx::query("INSERT INTO identity_access_tokens(jti,session_id,user_id,session_revision,issued_at,expires_at) VALUES($1,$2,$3,1,$4,$5)")
                .bind(&claims.jti).bind(&claims.session_id).bind(&claims.sub).bind(now).bind(expires).execute(pool).await.unwrap();
        }
    }

    async fn insert_user(pool: &PgPool, id: &str) {
        sqlx::query("INSERT INTO identity_users(id,email,display_name,password_hash) VALUES($1,$2,$1,'test-only')")
            .bind(id).bind(format!("{id}@example.test")).execute(pool).await.unwrap();
    }

    async fn insert_quant_account(
        pool: &PgPool,
        store: &TradingStore,
        id: &str,
        org: &str,
        owner: &str,
    ) {
        sqlx::query("INSERT INTO venues(id,venue_type,name,created_at) VALUES('simulated','simulated','Paper Simulator',NOW()) ON CONFLICT(id) DO NOTHING")
            .execute(pool).await.unwrap();
        sqlx::query("INSERT INTO accounts(id,account_type,status,venue_id,starting_capital,created_at,environment) VALUES($1,'simulated','active','simulated',100,NOW(),'PAPER')")
            .bind(id).execute(pool).await.unwrap();
        sqlx::query("INSERT INTO account_states(account_id,balance,equity,used_margin,open_position_count,daily_realized_pnl,peak_equity,kill_switch,updated_at) VALUES($1,100,100,0,0,0,100,FALSE,NOW())")
            .bind(id).execute(pool).await.unwrap();
        store
            .assign_account_entitlement(org, id, owner, 1, 0, "API test fixture")
            .await
            .unwrap();
    }

    #[allow(clippy::too_many_arguments)]
    async fn request_response(
        pool: &PgPool,
        app: &Router,
        method: Method,
        path: &str,
        subject: Option<&str>,
        organization: Option<&str>,
        body: Option<Value>,
        expired: bool,
    ) -> axum::response::Response {
        let mut builder = Request::builder().method(method).uri(path);
        if let Some(subject) = subject {
            let expiration = if expired {
                Utc::now() - Duration::minutes(5)
            } else {
                Utc::now() + Duration::minutes(5)
            };
            let (raw_token, claims) = token(subject, expiration);
            persist_test_token(pool, &claims).await;
            builder = builder.header("authorization", format!("Bearer {raw_token}"));
        }
        if let Some(org) = organization {
            builder = builder.header("x-organization-id", org);
            if let Some(workspace) = sqlx::query_scalar::<_, String>(
                "SELECT id FROM workspaces WHERE organization_id=$1 AND is_default=TRUE AND status='ACTIVE'",
            ).bind(org).fetch_optional(pool).await.unwrap() {
                builder = builder.header("x-workspace-id", workspace);
            }
        }
        let request = if let Some(value) = body {
            builder
                .header("content-type", "application/json")
                .body(Body::from(value.to_string()))
                .unwrap()
        } else {
            builder.body(Body::empty()).unwrap()
        };
        app.clone().oneshot(request).await.unwrap()
    }

    #[allow(clippy::too_many_arguments)]
    async fn request(
        pool: &PgPool,
        app: &Router,
        method: Method,
        path: &str,
        subject: Option<&str>,
        organization: Option<&str>,
        body: Option<Value>,
        expired: bool,
    ) -> StatusCode {
        request_response(
            pool,
            app,
            method,
            path,
            subject,
            organization,
            body,
            expired,
        )
        .await
        .status()
    }

    #[sqlx::test]
    async fn authenticated_http_routes_enforce_tenant_role_and_revalidation(pool: PgPool) {
        migrate(&pool).await.unwrap();
        for id in [
            "api-owner-a",
            "api-owner-b",
            "api-admin",
            "api-trader",
            "api-viewer",
            "api-risk",
        ] {
            insert_user(&pool, id).await;
        }
        let store = TradingStore::new(pool.clone());
        store
            .create_organization(
                &domain::Organization::new(
                    domain::OrganizationId::new("api-org-a").unwrap(),
                    "A",
                    Utc::now(),
                )
                .unwrap(),
                "api-owner-a",
            )
            .await
            .unwrap();
        store
            .create_organization(
                &domain::Organization::new(
                    domain::OrganizationId::new("api-org-b").unwrap(),
                    "B",
                    Utc::now(),
                )
                .unwrap(),
                "api-owner-b",
            )
            .await
            .unwrap();
        for (user, role) in [
            ("api-admin", domain::OrganizationRole::Admin),
            ("api-trader", domain::OrganizationRole::Trader),
            ("api-viewer", domain::OrganizationRole::Viewer),
            ("api-risk", domain::OrganizationRole::RiskManager),
        ] {
            store
                .create_organization_membership(
                    "api-org-a",
                    user,
                    "api-owner-a",
                    role,
                    "API fixture",
                )
                .await
                .unwrap();
        }
        for (user, role) in [
            ("api-trader", "TRADER"),
            ("api-viewer", "VIEWER"),
            ("api-risk", "RISK_MANAGER"),
        ] {
            sqlx::query("INSERT INTO workspace_access_grants(organization_id,workspace_id,user_id,role,status,revision,granted_by,updated_by,reason) SELECT 'api-org-a',id,$1,$2,'ACTIVE',1,'api-owner-a','api-owner-a','API workspace grant fixture' FROM workspaces WHERE organization_id='api-org-a' AND is_default=TRUE")
                .bind(user).bind(role).execute(&pool).await.unwrap();
        }
        insert_quant_account(&pool, &store, "api-account-a", "api-org-a", "api-owner-a").await;
        insert_quant_account(&pool, &store, "api-account-b", "api-org-b", "api-owner-b").await;
        sqlx::query("INSERT INTO dashboard_account_access(owner_id,account_id) VALUES('api-owner-a','api-account-b')").execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO accounts(id,account_type,status,venue_id,starting_capital,created_at,environment) VALUES('api-legacy-account','simulated','active','simulated',100,NOW(),'PAPER')")
            .execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO account_states(account_id,balance,equity,used_margin,open_position_count,daily_realized_pnl,peak_equity,kill_switch,updated_at) VALUES('api-legacy-account',100,100,0,0,0,100,FALSE,NOW())")
            .execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO dashboard_account_access(owner_id,account_id) VALUES('api-owner-a','api-legacy-account')")
            .execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO journal_accounts(id,owner_id,name) VALUES('journal-owned-a','api-owner-a','Private A'),('journal-owned-b','api-owner-b','Private B')")
            .execute(&pool).await.unwrap();

        let auth = auth::JwtConfig::new(
            PUBLIC_KEY.as_bytes(),
            "qos021a-r2-test".into(),
            "qos021a-r2-api".into(),
        )
        .unwrap();
        let app = build_router(
            AppState {
                store: store.clone(),
                auth,
                broker_identity_hmac: Some(crate::broker_registry::BrokerIdentityHmac::test_key()),
            },
            Vec::new(),
        );
        assert_eq!(
            request(
                &pool,
                &app,
                Method::GET,
                "/api/v1/trading/accounts/api-account-a",
                Some("api-owner-a"),
                Some("api-org-a"),
                None,
                false
            )
            .await,
            StatusCode::OK
        );
        assert_eq!(
            request(
                &pool,
                &app,
                Method::GET,
                "/api/v1/trading/accounts/api-account-b",
                Some("api-owner-a"),
                Some("api-org-a"),
                None,
                false
            )
            .await,
            StatusCode::NOT_FOUND
        );
        assert_eq!(
            request(
                &pool,
                &app,
                Method::GET,
                "/api/v1/trading/accounts/api-legacy-account",
                Some("api-owner-a"),
                Some("api-org-a"),
                None,
                false
            )
            .await,
            StatusCode::NOT_FOUND
        );
        assert_eq!(
            request(
                &pool,
                &app,
                Method::GET,
                "/api/v1/journal/accounts/journal-owned-a",
                Some("api-owner-a"),
                None,
                None,
                false
            )
            .await,
            StatusCode::OK
        );
        assert_eq!(
            request(
                &pool,
                &app,
                Method::GET,
                "/api/v1/journal/accounts/journal-owned-a",
                Some("api-owner-b"),
                Some("api-org-a"),
                None,
                false
            )
            .await,
            StatusCode::NOT_FOUND
        );
        assert_eq!(
            request(
                &pool,
                &app,
                Method::GET,
                "/api/v1/trading/accounts/api-account-b",
                Some("api-owner-a"),
                Some("api-org-b"),
                None,
                false
            )
            .await,
            StatusCode::NOT_FOUND
        );
        assert_eq!(
            request(
                &pool,
                &app,
                Method::GET,
                "/api/v1/trading/accounts",
                Some("api-owner-a"),
                None,
                None,
                false
            )
            .await,
            StatusCode::NOT_FOUND
        );
        assert_eq!(
            request(
                &pool,
                &app,
                Method::GET,
                "/api/v1/trading/accounts",
                None,
                Some("api-org-a"),
                None,
                false
            )
            .await,
            StatusCode::UNAUTHORIZED
        );
        let invalid_token = Request::builder()
            .method(Method::GET)
            .uri("/api/v1/trading/accounts")
            .header("authorization", "Bearer invalid.jwt.token")
            .header("x-organization-id", "api-org-a")
            .body(Body::empty())
            .unwrap();
        assert_eq!(
            app.clone().oneshot(invalid_token).await.unwrap().status(),
            StatusCode::UNAUTHORIZED
        );
        assert_eq!(
            request(
                &pool,
                &app,
                Method::GET,
                "/api/v1/trading/accounts",
                Some("api-owner-a"),
                Some("api-org-a"),
                None,
                true
            )
            .await,
            StatusCode::UNAUTHORIZED
        );
        assert_eq!(
            request(
                &pool,
                &app,
                Method::GET,
                "/api/v1/organizations/api-org-b/account-entitlements",
                Some("api-owner-a"),
                Some("api-org-a"),
                None,
                false
            )
            .await,
            StatusCode::NOT_FOUND
        );

        let assignment = json!({"expected_account_revision":1,"expected_entitlement_revision":0,"reason":"attempt"});
        assert_eq!(
            request(
                &pool,
                &app,
                Method::PUT,
                "/api/v1/organizations/api-org-a/account-entitlements/api-account-b",
                Some("api-admin"),
                Some("api-org-a"),
                Some(assignment.clone()),
                false
            )
            .await,
            StatusCode::FORBIDDEN
        );
        assert_eq!(
            request(
                &pool,
                &app,
                Method::POST,
                "/api/v1/trading/accounts",
                Some("api-owner-a"),
                Some("api-org-a"),
                Some(json!({"name":"API paper","initial_capital":"250","environment":"PAPER"})),
                false
            )
            .await,
            StatusCode::CREATED
        );
        let owned_count:i64=sqlx::query_scalar("SELECT count(*) FROM organization_account_entitlements WHERE organization_id='api-org-a' AND status='ACTIVE'")
            .fetch_one(&pool).await.unwrap();
        assert_eq!(owned_count, 2);
        assert_eq!(
            request(
                &pool,
                &app,
                Method::POST,
                "/api/v1/trading/accounts",
                Some("api-viewer"),
                Some("api-org-a"),
                Some(
                    json!({"name":"Viewer account","initial_capital":"100","environment":"PAPER"})
                ),
                false
            )
            .await,
            StatusCode::FORBIDDEN
        );
        assert_eq!(
            request(
                &pool,
                &app,
                Method::POST,
                "/api/v1/organizations/api-org-a/owner-transfer",
                Some("api-admin"),
                Some("api-org-a"),
                Some(json!({"target_user_id":"api-trader","actor_expected_revision":1,"target_expected_revision":1,"reason":"attempt"})),
                false
            ).await,
            StatusCode::FORBIDDEN
        );
        assert_eq!(
            request(
                &pool,
                &app,
                Method::POST,
                "/api/v1/organizations/api-org-a/members",
                Some("api-trader"),
                Some("api-org-a"),
                Some(json!({"user_id":"api-viewer","role":"VIEWER","reason":"attempt"})),
                false
            )
            .await,
            StatusCode::FORBIDDEN
        );
        assert_eq!(
            request(
                &pool,
                &app,
                Method::POST,
                "/api/v1/trading/risk/decisions",
                Some("api-risk"),
                Some("api-org-a"),
                Some(json!({})),
                false
            )
            .await,
            StatusCode::METHOD_NOT_ALLOWED
        );
        assert_eq!(
            request(
                &pool,
                &app,
                Method::POST,
                "/api/v1/trading/execution-authorizations",
                Some("api-risk"),
                Some("api-org-a"),
                Some(json!({})),
                false
            )
            .await,
            StatusCode::NOT_FOUND
        );
        store
            .revise_organization(
                "api-org-a",
                "api-owner-a",
                1,
                None,
                Some(domain::OrganizationStatus::Suspended),
                "test suspension",
            )
            .await
            .unwrap();
        assert_eq!(
            request(
                &pool,
                &app,
                Method::GET,
                "/api/v1/trading/accounts/api-account-a",
                Some("api-owner-a"),
                Some("api-org-a"),
                None,
                false
            )
            .await,
            StatusCode::NOT_FOUND
        );
        assert_eq!(
            request(
                &pool,
                &app,
                Method::PATCH,
                "/api/v1/organizations/api-org-a",
                Some("api-owner-a"),
                Some("api-org-a"),
                Some(json!({"expected_revision":2,"status":"ACTIVE","reason":"restore test"})),
                false
            )
            .await,
            StatusCode::OK
        );

        assert_eq!(
            request(
                &pool,
                &app,
                Method::PATCH,
                "/api/v1/organizations/api-org-a/account-entitlements/api-account-a",
                Some("api-owner-a"),
                Some("api-org-a"),
                Some(json!({"expected_account_revision":1,"expected_entitlement_revision":1,"status":"SUSPENDED","reason":"suspend test"})),
                false
            ).await,
            StatusCode::OK
        );
        assert_eq!(
            request(
                &pool,
                &app,
                Method::GET,
                "/api/v1/trading/accounts/api-account-a",
                Some("api-owner-a"),
                Some("api-org-a"),
                None,
                false
            )
            .await,
            StatusCode::NOT_FOUND
        );
        assert_eq!(
            request(
                &pool,
                &app,
                Method::PATCH,
                "/api/v1/organizations/api-org-a/account-entitlements/api-account-a",
                Some("api-owner-a"),
                Some("api-org-a"),
                Some(json!({"expected_account_revision":1,"expected_entitlement_revision":1,"status":"ACTIVE","reason":"stale"})),
                false
            ).await,
            StatusCode::CONFLICT
        );
        assert_eq!(
            request(
                &pool,
                &app,
                Method::PATCH,
                "/api/v1/organizations/api-org-a/account-entitlements/api-account-a",
                Some("api-owner-a"),
                Some("api-org-a"),
                Some(json!({"expected_account_revision":1,"expected_entitlement_revision":2,"status":"REVOKED","reason":"revoke test"})),
                false
            ).await,
            StatusCode::OK
        );
        assert_eq!(
            request(
                &pool,
                &app,
                Method::GET,
                "/api/v1/trading/accounts/api-account-a",
                Some("api-owner-a"),
                Some("api-org-a"),
                None,
                false
            )
            .await,
            StatusCode::NOT_FOUND
        );

        assert_eq!(
            request(
                &pool,
                &app,
                Method::PATCH,
                "/api/v1/organizations/api-org-a/members/api-admin",
                Some("api-owner-a"),
                Some("api-org-a"),
                Some(json!({"expected_revision":1,"role":"ADMIN","status":"SUSPENDED","reason":"suspend member"})),
                false
            ).await,
            StatusCode::OK
        );
        assert_eq!(
            request(
                &pool,
                &app,
                Method::GET,
                "/api/v1/trading/accounts/api-account-a",
                Some("api-admin"),
                Some("api-org-a"),
                None,
                false
            )
            .await,
            StatusCode::NOT_FOUND
        );
        assert_eq!(
            request(
                &pool,
                &app,
                Method::PATCH,
                "/api/v1/organizations/api-org-a/members/api-admin",
                Some("api-owner-a"),
                Some("api-org-a"),
                Some(
                    json!({"expected_revision":1,"role":"ADMIN","status":"ACTIVE","reason":"stale"})
                ),
                false
            )
            .await,
            StatusCode::CONFLICT
        );
        assert_eq!(
            request(
                &pool,
                &app,
                Method::PATCH,
                "/api/v1/organizations/api-org-a/members/api-admin",
                Some("api-owner-a"),
                Some("api-org-a"),
                Some(json!({"expected_revision":2,"role":"ADMIN","status":"ACTIVE","reason":"reinstate"})),
                false
            ).await,
            StatusCode::OK
        );
        assert_eq!(
            request(
                &pool,
                &app,
                Method::PATCH,
                "/api/v1/organizations/api-org-a/members/api-admin",
                Some("api-owner-a"),
                Some("api-org-a"),
                Some(json!({"expected_revision":3,"role":"ADMIN","status":"REVOKED","reason":"revoke"})),
                false
            ).await,
            StatusCode::OK
        );
        assert_eq!(
            request(
                &pool,
                &app,
                Method::GET,
                "/api/v1/trading/accounts/api-account-a",
                Some("api-admin"),
                Some("api-org-a"),
                None,
                false
            )
            .await,
            StatusCode::NOT_FOUND
        );

        let create_response = request_response(
            &pool,
            &app,
            Method::POST,
            "/api/v1/organizations",
            Some("api-owner-a"),
            None,
            Some(json!({"name":"Created through authenticated API"})),
            false,
        )
        .await;
        assert_eq!(create_response.status(), StatusCode::CREATED);
        let create_body = axum::body::to_bytes(create_response.into_body(), usize::MAX)
            .await
            .unwrap();
        let create_payload: Value = serde_json::from_slice(&create_body).unwrap();
        assert_eq!(
            create_payload["data"]["defaultWorkspace"]["name"],
            "Default"
        );
        assert_eq!(create_payload["data"]["membershipRole"], "OWNER");
        // Organization, OWNER membership, default workspace and both audit streams
        // are committed together. The API response contract is additive.
        let created_org: String = sqlx::query_scalar(
            "SELECT id FROM organizations WHERE name='Created through authenticated API' AND created_by='api-owner-a'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        let provisioned: (String, String, bool, String) = sqlx::query_as(
            "SELECT id,name,is_default,status FROM workspaces WHERE organization_id=$1",
        )
        .bind(&created_org)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(provisioned.1, "Default");
        assert!(provisioned.2);
        assert_eq!(provisioned.3, "ACTIVE");
        let owner_membership: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM organization_memberships WHERE organization_id=$1 AND user_id='api-owner-a' AND role='OWNER' AND status='ACTIVE')",
        )
        .bind(&created_org)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert!(owner_membership);
        let workspace_events: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM workspace_events WHERE workspace_id=$1 AND event_type='CREATED'",
        )
        .bind(&provisioned.0)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(workspace_events, 1);

        // Fail after the organization INSERT and M1 workspace trigger have
        // both executed. The transaction must roll back the organization,
        // both audit records, the OWNER membership, and the default workspace.
        let rollback_id = "api-org-provision-rollback";
        sqlx::raw_sql(
            "CREATE FUNCTION qos021a_fail_owner_membership_insert() RETURNS trigger
             LANGUAGE plpgsql AS $fail$
             BEGIN
                 RAISE EXCEPTION 'injected owner membership failure';
             END
             $fail$;
             CREATE TRIGGER qos021a_fail_owner_membership_insert
             BEFORE INSERT ON organization_memberships
             FOR EACH ROW EXECUTE FUNCTION qos021a_fail_owner_membership_insert();",
        )
        .execute(&pool)
        .await
        .unwrap();
        let rollback_org = domain::Organization::new(
            domain::OrganizationId::new(rollback_id).unwrap(),
            "Must roll back",
            Utc::now(),
        )
        .unwrap();
        assert!(store
            .create_organization(&rollback_org, "api-owner-a")
            .await
            .is_err());
        sqlx::raw_sql(
            "DROP TRIGGER qos021a_fail_owner_membership_insert ON organization_memberships;
             DROP FUNCTION qos021a_fail_owner_membership_insert();",
        )
        .execute(&pool)
        .await
        .unwrap();
        let rollback_counts: (i64, i64, i64, i64, i64) = sqlx::query_as(
            "SELECT
                (SELECT count(*) FROM organizations WHERE id=$1),
                (SELECT count(*) FROM organization_events WHERE organization_id=$1),
                (SELECT count(*) FROM organization_memberships WHERE organization_id=$1),
                (SELECT count(*) FROM workspaces WHERE organization_id=$1),
                (SELECT count(*) FROM workspace_events WHERE organization_id=$1)",
        )
        .bind(rollback_id)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(rollback_counts, (0, 0, 0, 0, 0));

        let owner_session = hex::encode(Sha256::digest("api-owner-a".as_bytes()));
        sqlx::query("UPDATE identity_sessions SET status='REVOKED',revision=revision+1,revoked_at=NOW(),revocation_reason='test logout',updated_at=NOW() WHERE id=$1")
            .bind(owner_session).execute(&pool).await.unwrap();
        assert_eq!(
            request(
                &pool,
                &app,
                Method::GET,
                "/api/v1/trading/accounts/api-account-a",
                Some("api-owner-a"),
                Some("api-org-a"),
                None,
                false
            )
            .await,
            StatusCode::UNAUTHORIZED
        );
    }

    #[sqlx::test]
    async fn broker_registry_http_flow_is_private_tenant_scoped_and_pending_only(pool: PgPool) {
        migrate(&pool).await.unwrap();
        let store = TradingStore::new(pool.clone());
        for id in [
            "r3-api-owner-a",
            "r3-api-owner-b",
            "r3-api-admin",
            "r3-api-trader",
            "r3-api-viewer",
            "r3-api-risk-manager",
        ] {
            insert_user(&pool, id).await;
        }
        for (org, owner) in [
            ("r3-api-org-a", "r3-api-owner-a"),
            ("r3-api-org-b", "r3-api-owner-b"),
        ] {
            store
                .create_organization(
                    &domain::Organization::new(
                        domain::OrganizationId::new(org).unwrap(),
                        "Broker registry API",
                        Utc::now(),
                    )
                    .unwrap(),
                    owner,
                )
                .await
                .unwrap();
        }
        for (member, role) in [
            ("r3-api-admin", domain::OrganizationRole::Admin),
            ("r3-api-trader", domain::OrganizationRole::Trader),
            ("r3-api-viewer", domain::OrganizationRole::Viewer),
            ("r3-api-risk-manager", domain::OrganizationRole::RiskManager),
        ] {
            store
                .create_organization_membership(
                    "r3-api-org-a",
                    member,
                    "r3-api-owner-a",
                    role,
                    "R3 test",
                )
                .await
                .unwrap();
        }
        insert_quant_account(
            &pool,
            &store,
            "r3-api-account-a",
            "r3-api-org-a",
            "r3-api-owner-a",
        )
        .await;
        insert_quant_account(
            &pool,
            &store,
            "r3-api-account-b",
            "r3-api-org-b",
            "r3-api-owner-b",
        )
        .await;
        sqlx::query("INSERT INTO brokers(id,canonical_name,display_name,normalization_version) VALUES('example-broker','example-broker','Example Broker',1)")
            .execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO broker_servers(id,broker_id,canonical_identifier,display_name,provider_type,environment,normalization_version,source_reference) VALUES('r3-api-server','example-broker','demo.example','Demo','MT5_PYTHON','DEMO',1,'controlled-test-seed')")
            .execute(&pool).await.unwrap();

        let app = build_router(
            AppState {
                store: store.clone(),
                auth: auth::JwtConfig::new(
                    PUBLIC_KEY.as_bytes(),
                    "qos021a-r2-test".into(),
                    "qos021a-r2-api".into(),
                )
                .unwrap(),
                broker_identity_hmac: Some(crate::broker_registry::BrokerIdentityHmac::test_key()),
            },
            Vec::new(),
        );
        let raw_identifier = "R3-ACCT-001234";
        let registration = json!({
            "quant_account_id":"r3-api-account-a",
            "broker_id":"example-broker",
            "broker_server_id":"r3-api-server",
            "external_account_identifier":raw_identifier,
            "reason":"Owner initiated registration"
        });
        assert_eq!(
            request(
                &pool,
                &app,
                Method::POST,
                "/api/v1/organizations/r3-api-org-a/broker-account-bindings",
                Some("r3-api-owner-a"),
                Some("r3-api-org-a"),
                Some(registration.clone()),
                false
            )
            .await,
            StatusCode::ACCEPTED
        );
        for member in [
            "r3-api-admin",
            "r3-api-trader",
            "r3-api-viewer",
            "r3-api-risk-manager",
        ] {
            assert_eq!(
                request(
                    &pool,
                    &app,
                    Method::POST,
                    "/api/v1/organizations/r3-api-org-a/broker-account-bindings",
                    Some(member),
                    Some("r3-api-org-a"),
                    Some(registration.clone()),
                    false
                )
                .await,
                StatusCode::FORBIDDEN
            );
        }
        assert_eq!(
            request(
                &pool,
                &app,
                Method::POST,
                "/api/v1/organizations/r3-api-org-a/broker-account-bindings",
                Some("r3-api-owner-a"),
                None,
                Some(registration.clone()),
                false
            )
            .await,
            StatusCode::NOT_FOUND
        );
        let duplicate = json!({
            "quant_account_id":"r3-api-account-b",
            "broker_id":"example-broker",
            "broker_server_id":"r3-api-server",
            "external_account_identifier":raw_identifier,
            "reason":"Competing organization claim"
        });
        assert_eq!(
            request(
                &pool,
                &app,
                Method::POST,
                "/api/v1/organizations/r3-api-org-b/broker-account-bindings",
                Some("r3-api-owner-b"),
                Some("r3-api-org-b"),
                Some(duplicate),
                false
            )
            .await,
            StatusCode::CONFLICT
        );
        let binding_id: String = sqlx::query_scalar(
            "SELECT id FROM trading_account_broker_bindings WHERE organization_id='r3-api-org-a'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(
            request(
                &pool,
                &app,
                Method::POST,
                "/api/v1/organizations/r3-api-org-b/broker-account-bindings",
                Some("r3-api-owner-a"),
                Some("r3-api-org-a"),
                Some(json!({"quant_account_id":"r3-api-account-a","broker_id":"example-broker","broker_server_id":"r3-api-server","external_account_identifier":"another-id","reason":"Cross-tenant path"})),
                false
            )
            .await,
            StatusCode::NOT_FOUND
        );
        assert_eq!(
            request(
                &pool,
                &app,
                Method::PATCH,
                &format!("/api/v1/organizations/r3-api-org-a/broker-account-bindings/{binding_id}"),
                Some("r3-api-owner-a"),
                Some("r3-api-org-a"),
                Some(json!({"expected_revision":1,"status":"VERIFIED","reason":"self approval"})),
                false
            )
            .await,
            StatusCode::FORBIDDEN
        );
        assert_eq!(
            request(
                &pool,
                &app,
                Method::PATCH,
                &format!("/api/v1/organizations/r3-api-org-a/broker-account-bindings/{binding_id}"),
                Some("r3-api-owner-a"),
                Some("r3-api-org-a"),
                Some(json!({"expected_revision":1,"status":"SUSPENDED","reason":"owner suspend"})),
                false
            )
            .await,
            StatusCode::OK
        );
        let lookup_hex: String = sqlx::query_scalar(
            "SELECT encode(identifier_lookup_hmac,'hex') FROM external_broker_accounts WHERE id=(SELECT external_broker_account_id FROM trading_account_broker_bindings WHERE id=$1)",
        )
        .bind(&binding_id)
        .fetch_one(&pool)
        .await
        .unwrap();
        let (list_token, list_claims) = token("r3-api-owner-a", Utc::now() + Duration::minutes(5));
        persist_test_token(&pool, &list_claims).await;
        let list_request = Request::builder()
            .method(Method::GET)
            .uri("/api/v1/organizations/r3-api-org-a/broker-account-bindings")
            .header("authorization", format!("Bearer {list_token}"))
            .header("x-organization-id", "r3-api-org-a")
            .body(Body::empty())
            .unwrap();
        let response = app.clone().oneshot(list_request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = response.into_body().collect().await.unwrap().to_bytes();
        let body_text = String::from_utf8_lossy(&body);
        assert!(!body_text.contains(raw_identifier));
        assert!(!body_text.contains(&lookup_hex));
        assert!(body_text.contains("SUSPENDED"));
        assert_eq!(
            request(
                &pool,
                &app,
                Method::POST,
                "/api/v1/broker-registry/catalog",
                Some("r3-api-owner-a"),
                Some("r3-api-org-a"),
                Some(json!({})),
                false
            )
            .await,
            StatusCode::METHOD_NOT_ALLOWED
        );
        let no_key_app = build_router(
            AppState {
                store,
                auth: auth::JwtConfig::new(
                    PUBLIC_KEY.as_bytes(),
                    "qos021a-r2-test".into(),
                    "qos021a-r2-api".into(),
                )
                .unwrap(),
                broker_identity_hmac: None,
            },
            Vec::new(),
        );
        assert_eq!(
            request(
                &pool,
                &no_key_app,
                Method::POST,
                "/api/v1/organizations/r3-api-org-a/broker-account-bindings",
                Some("r3-api-owner-a"),
                Some("r3-api-org-a"),
                Some(registration),
                false
            )
            .await,
            StatusCode::SERVICE_UNAVAILABLE
        );
    }
    #[sqlx::test]
    async fn workspace_authorization_and_dashboard_queries_are_isolated_by_assignment(
        pool: PgPool,
    ) {
        migrate(&pool).await.unwrap();
        insert_user(&pool, "ws-owner").await;
        insert_user(&pool, "ws-trader").await;
        let store = TradingStore::new(pool.clone());
        store
            .create_organization(
                &domain::Organization::new(
                    domain::OrganizationId::new("ws-org").unwrap(),
                    "Workspace Org",
                    Utc::now(),
                )
                .unwrap(),
                "ws-owner",
            )
            .await
            .unwrap();
        store
            .create_organization_membership(
                "ws-org",
                "ws-trader",
                "ws-owner",
                domain::OrganizationRole::Trader,
                "workspace test",
            )
            .await
            .unwrap();
        let default = store.default_workspace("ws-org").await.unwrap().unwrap();
        let second = domain::Workspace::new(
            domain::WorkspaceId::new("ws-research").unwrap(),
            domain::OrganizationId::new("ws-org").unwrap(),
            "Research",
            "research",
            "ws-owner",
            Utc::now(),
        )
        .unwrap();
        store
            .create_workspace(&second, "workspace isolation test")
            .await
            .unwrap();
        sqlx::query("INSERT INTO workspace_access_grants(organization_id,workspace_id,user_id,role,status,revision,granted_by,updated_by,reason) VALUES('ws-org',$1,'ws-trader','TRADER','ACTIVE',1,'ws-owner','ws-owner','default workspace only')")
            .bind(default.id.as_str()).execute(&pool).await.unwrap();
        insert_quant_account(&pool, &store, "ws-account-a", "ws-org", "ws-owner").await;
        insert_quant_account(&pool, &store, "ws-account-b", "ws-org", "ws-owner").await;

        let auth = auth::JwtConfig::new(
            PUBLIC_KEY.as_bytes(),
            "qos021a-r2-test".into(),
            "qos021a-r2-api".into(),
        )
        .unwrap();
        let app = build_router(
            AppState {
                store: store.clone(),
                auth,
                broker_identity_hmac: Some(crate::broker_registry::BrokerIdentityHmac::test_key()),
            },
            Vec::new(),
        );

        assert_eq!(
            request(
                &pool,
                &app,
                Method::PUT,
                &format!(
                    "/api/v1/organizations/ws-org/workspaces/{}/accounts/ws-account-b/assignment",
                    second.id.as_str()
                ),
                Some("ws-owner"),
                Some("ws-org"),
                Some(json!({"expected_revision":1,"reason":"move into research workspace"})),
                false
            )
            .await,
            StatusCode::OK
        );

        let default_response = request_response(
            &pool,
            &app,
            Method::GET,
            "/api/v1/trading/accounts",
            Some("ws-owner"),
            Some("ws-org"),
            None,
            false,
        )
        .await;
        assert_eq!(default_response.status(), StatusCode::OK);
        let default_body = default_response
            .into_body()
            .collect()
            .await
            .unwrap()
            .to_bytes();
        let default_json: Value = serde_json::from_slice(&default_body).unwrap();
        assert_eq!(default_json["data"].as_array().unwrap().len(), 1);
        assert_eq!(default_json["data"][0]["id"], "ws-account-a");

        let (owner_token, owner_claims) = token("ws-owner", Utc::now() + Duration::minutes(5));
        persist_test_token(&pool, &owner_claims).await;
        let second_request = Request::builder()
            .method(Method::GET)
            .uri("/api/v1/trading/accounts")
            .header("authorization", format!("Bearer {owner_token}"))
            .header("x-organization-id", "ws-org")
            .header("x-workspace-id", second.id.as_str())
            .body(Body::empty())
            .unwrap();
        let second_response = app.clone().oneshot(second_request).await.unwrap();
        assert_eq!(second_response.status(), StatusCode::OK);
        let second_body = second_response
            .into_body()
            .collect()
            .await
            .unwrap()
            .to_bytes();
        let second_json: Value = serde_json::from_slice(&second_body).unwrap();
        assert_eq!(second_json["data"].as_array().unwrap().len(), 1);
        assert_eq!(second_json["data"][0]["id"], "ws-account-b");

        let (trader_token, trader_claims) = token("ws-trader", Utc::now() + Duration::minutes(5));
        persist_test_token(&pool, &trader_claims).await;
        let denied = Request::builder()
            .method(Method::GET)
            .uri("/api/v1/trading/accounts")
            .header("authorization", format!("Bearer {trader_token}"))
            .header("x-organization-id", "ws-org")
            .header("x-workspace-id", second.id.as_str())
            .body(Body::empty())
            .unwrap();
        assert_eq!(
            app.clone().oneshot(denied).await.unwrap().status(),
            StatusCode::NOT_FOUND
        );

        assert_eq!(request(&pool, &app, Method::POST,
            &format!("/api/v1/organizations/ws-org/workspaces/{}/members", second.id.as_str()),
            Some("ws-owner"), Some("ws-org"),
            Some(json!({"user_id":"ws-trader","role":"TRADER","reason":"grant research workspace access"})), false).await,
            StatusCode::CREATED);

        let (trader_token_after_grant, trader_claims_after_grant) =
            token("ws-trader", Utc::now() + Duration::minutes(5));
        persist_test_token(&pool, &trader_claims_after_grant).await;
        let granted = Request::builder()
            .method(Method::GET)
            .uri("/api/v1/trading/accounts")
            .header(
                "authorization",
                format!("Bearer {trader_token_after_grant}"),
            )
            .header("x-organization-id", "ws-org")
            .header("x-workspace-id", second.id.as_str())
            .body(Body::empty())
            .unwrap();
        let granted_response = app.clone().oneshot(granted).await.unwrap();
        assert_eq!(granted_response.status(), StatusCode::OK);
        let granted_body = granted_response
            .into_body()
            .collect()
            .await
            .unwrap()
            .to_bytes();
        let granted_json: Value = serde_json::from_slice(&granted_body).unwrap();
        assert_eq!(granted_json["data"].as_array().unwrap().len(), 1);
        assert_eq!(granted_json["data"][0]["id"], "ws-account-b");

        assert_eq!(request(&pool, &app, Method::PATCH,
            &format!("/api/v1/organizations/ws-org/workspaces/{}/members/ws-trader", second.id.as_str()),
            Some("ws-owner"), Some("ws-org"),
            Some(json!({"expected_revision":1,"role":"TRADER","status":"REVOKED","reason":"remove research workspace access"})), false).await,
            StatusCode::OK);

        let (trader_token_after_revoke, trader_claims_after_revoke) =
            token("ws-trader", Utc::now() + Duration::minutes(5));
        persist_test_token(&pool, &trader_claims_after_revoke).await;
        let revoked = Request::builder()
            .method(Method::GET)
            .uri("/api/v1/trading/accounts")
            .header(
                "authorization",
                format!("Bearer {trader_token_after_revoke}"),
            )
            .header("x-organization-id", "ws-org")
            .header("x-workspace-id", second.id.as_str())
            .body(Body::empty())
            .unwrap();
        assert_eq!(
            app.clone().oneshot(revoked).await.unwrap().status(),
            StatusCode::NOT_FOUND
        );

        let (missing_context_token, missing_context_claims) =
            token("ws-owner", Utc::now() + Duration::minutes(5));
        persist_test_token(&pool, &missing_context_claims).await;
        let missing_context = Request::builder()
            .method(Method::GET)
            .uri("/api/v1/trading/accounts")
            .header("authorization", format!("Bearer {missing_context_token}"))
            .header("x-organization-id", "ws-org")
            .body(Body::empty())
            .unwrap();
        assert_eq!(
            app.clone().oneshot(missing_context).await.unwrap().status(),
            StatusCode::BAD_REQUEST
        );

        let (owner_token_for_creation, owner_claims_for_creation) =
            token("ws-owner", Utc::now() + Duration::minutes(5));
        persist_test_token(&pool, &owner_claims_for_creation).await;
        let create_in_selected_workspace = Request::builder()
            .method(Method::POST)
            .uri("/api/v1/trading/accounts")
            .header(
                "authorization",
                format!("Bearer {owner_token_for_creation}"),
            )
            .header("x-organization-id", "ws-org")
            .header("x-workspace-id", second.id.as_str())
            .header("content-type", "application/json")
            .body(Body::from(
                json!({"name":"Research Paper","initial_capital":"250","environment":"PAPER"})
                    .to_string(),
            ))
            .unwrap();
        let created = app
            .clone()
            .oneshot(create_in_selected_workspace)
            .await
            .unwrap();
        assert_eq!(created.status(), StatusCode::CREATED);
        let created_body = created.into_body().collect().await.unwrap().to_bytes();
        let created_json: Value = serde_json::from_slice(&created_body).unwrap();
        let created_account = created_json["data"]["id"].as_str().unwrap();
        let assigned_workspace: String = sqlx::query_scalar(
            "SELECT workspace_id FROM workspace_account_assignments WHERE account_id=$1 AND organization_id='ws-org'",
        )
        .bind(created_account)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(assigned_workspace, second.id.as_str());
    }
}

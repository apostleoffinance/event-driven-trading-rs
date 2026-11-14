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
        ]);

    Router::new()
        .route(
            "/api/v1/organizations",
            get(organizations_api::list).post(organizations_api::create),
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
        auth::JwtConfig::from_files(&verification_keys, issuer, audience).unwrap_or_else(|_| {
            eprintln!("invalid JWT verification key-ring configuration");
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
        }
        let request = if let Some(value) = body {
            builder
                .header("content-type", "application/json")
                .body(Body::from(value.to_string()))
                .unwrap()
        } else {
            builder.body(Body::empty()).unwrap()
        };
        app.clone().oneshot(request).await.unwrap().status()
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

        assert_eq!(
            request(
                &pool,
                &app,
                Method::POST,
                "/api/v1/organizations",
                Some("api-owner-a"),
                None,
                Some(json!({"name":"Created through authenticated API"})),
                false
            )
            .await,
            StatusCode::CREATED
        );
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
}

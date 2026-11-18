use std::{env, net::SocketAddr, sync::Arc};

use axum::{
    http::{header, HeaderValue, Method},
    routing::{get, post},
    Router,
};
use identity_token::JwtKeyRing;
use sqlx::PgPool;
use tower_http::cors::{AllowOrigin, CorsLayer};

mod service;

#[derive(Clone)]
pub struct AppState {
    pub pool: PgPool,
    pub jwt: Arc<JwtKeyRing>,
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
    let migrator_database_url = required("MIGRATOR_DATABASE_URL");
    let issuer = required("EDGENEST_JWT_ISSUER");
    let audience = required("EDGENEST_JWT_AUDIENCE");
    let active_kid = required("EDGENEST_JWT_ACTIVE_KID");
    let signing_keys = required("EDGENEST_JWT_SIGNING_KEY_FILES");
    let verification_keys = required("EDGENEST_JWT_VERIFICATION_KEY_FILES");
    let jwt = JwtKeyRing::from_files(
        &active_kid,
        &signing_keys,
        &verification_keys,
        issuer,
        audience,
    )
    .unwrap_or_else(|_| {
        eprintln!("invalid EdgeNest JWT key-ring configuration");
        std::process::exit(2);
    });
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
        .unwrap_or_else(|_| "https://edgenest-frontend.edgenest.workers.dev,http://localhost:3000,http://localhost:3107,http://127.0.0.1:8787".into())
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .filter_map(|s| s.parse::<HeaderValue>().ok())
        .collect::<Vec<_>>();

    let migrator_pool = persistence::connect(&migrator_database_url)
        .await
        .unwrap_or_else(|e| {
            eprintln!("migration database connection failed: {e}");
            std::process::exit(2);
        });
    persistence::migrate(&migrator_pool)
        .await
        .unwrap_or_else(|e| {
            eprintln!("database migrations failed: {e}");
            std::process::exit(2);
        });
    migrator_pool.close().await;
    let pool = persistence::connect(&database_url)
        .await
        .unwrap_or_else(|e| {
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
        .route(
            "/api/v1/auth/access-token/revoke",
            post(service::revoke_access_token),
        )
        .route("/api/v1/auth/me", get(service::me))
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

#[cfg(test)]
mod i1_tests {
    #![allow(clippy::unwrap_used)]

    use super::*;
    use axum::{
        extract::State,
        http::{HeaderMap, HeaderValue, StatusCode},
        response::Response,
        Json,
    };
    use http_body_util::BodyExt;
    use identity_token::AccessClaims;
    use serde_json::Value;
    use std::collections::BTreeMap;

    const PRIVATE: &[u8] = include_bytes!("../../journal-api/tests/fixtures/r2_test_private.pem");
    const PUBLIC: &[u8] = include_bytes!("../../journal-api/tests/fixtures/r2_test_public.pem");

    fn state(pool: PgPool) -> AppState {
        let mut signing = BTreeMap::new();
        signing.insert("i1-test".into(), PRIVATE.to_vec());
        let mut verification = BTreeMap::new();
        verification.insert("i1-test".into(), PUBLIC.to_vec());
        AppState {
            pool,
            jwt: Arc::new(
                JwtKeyRing::from_pem_maps(
                    "i1-test",
                    signing,
                    verification,
                    "i1-test-issuer".into(),
                    "i1-test-audience".into(),
                )
                .unwrap(),
            ),
            access_ttl_seconds: 900,
            refresh_ttl_days: 30,
        }
    }

    async fn response_json(response: Response) -> (StatusCode, Value) {
        let status = response.status();
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        (
            status,
            serde_json::from_slice(&bytes).unwrap_or(Value::Null),
        )
    }

    async fn register(state: &AppState, email: &str) -> Value {
        let response = service::register(
            State(state.clone()),
            Json(service::RegisterRequest {
                email: email.into(),
                display_name: "I1 integration test".into(),
                password: "correct horse battery staple".into(),
            }),
        )
        .await;
        let (status, value) = response_json(response).await;
        assert_eq!(status, StatusCode::CREATED);
        value
    }

    fn bearer(token: &str) -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert(
            "authorization",
            HeaderValue::from_str(&format!("Bearer {token}")).unwrap(),
        );
        headers
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn session_issue_refresh_replay_revokes_and_logout_invalidates_access(pool: PgPool) {
        let app = state(pool.clone());
        let issued = register(&app, "i1-flow@example.test").await;
        let issued_access = issued["access_token"].as_str().unwrap();
        let issued_refresh = issued["refresh_token"].as_str().unwrap().to_owned();
        let claims: AccessClaims = app.jwt.verify(issued_access).unwrap();
        assert!(persistence::TradingStore::new(pool.clone())
            .validate_access_token(&claims)
            .await
            .unwrap());

        let rotated = service::refresh(
            State(app.clone()),
            Json(service::RefreshRequest {
                refresh_token: issued_refresh.clone(),
            }),
        )
        .await;
        let (status, value) = response_json(rotated).await;
        assert_eq!(status, StatusCode::OK);
        let rotated_access = value["access_token"].as_str().unwrap();
        let rotated_claims = app.jwt.verify(rotated_access).unwrap();
        assert!(persistence::TradingStore::new(pool.clone())
            .validate_access_token(&rotated_claims)
            .await
            .unwrap());

        let replay = service::refresh(
            State(app.clone()),
            Json(service::RefreshRequest {
                refresh_token: issued_refresh,
            }),
        )
        .await;
        assert_eq!(replay.status(), StatusCode::UNAUTHORIZED);
        assert!(!persistence::TradingStore::new(pool.clone())
            .validate_access_token(&rotated_claims)
            .await
            .unwrap());
        let event: String = sqlx::query_scalar("SELECT event_type FROM identity_session_events WHERE session_id=$1 ORDER BY id DESC LIMIT 1")
            .bind(&claims.session_id).fetch_one(&pool).await.unwrap();
        assert_eq!(event, "REFRESH_TOKEN_REUSE_REVOKED_SESSION");
        let rewrite_event = sqlx::query(
            "UPDATE identity_session_events SET reason='rewritten' WHERE session_id=$1",
        )
        .bind(&claims.session_id)
        .execute(&pool)
        .await;
        assert!(rewrite_event.is_err());

        let second = register(&app, "i1-logout@example.test").await;
        let token = second["access_token"].as_str().unwrap();
        let second_claims = app.jwt.verify(token).unwrap();
        assert_eq!(
            service::me(State(app.clone()), bearer(token))
                .await
                .status(),
            StatusCode::OK
        );
        let logout = service::logout(
            State(app.clone()),
            bearer(token),
            Json(service::RefreshRequest {
                refresh_token: second["refresh_token"].as_str().unwrap().into(),
            }),
        )
        .await;
        assert_eq!(logout.status(), StatusCode::NO_CONTENT);
        assert!(!persistence::TradingStore::new(pool)
            .validate_access_token(&second_claims)
            .await
            .unwrap());

        let third = register(&app, "i1-revoke@example.test").await;
        let third_token = third["access_token"].as_str().unwrap();
        let third_claims = app.jwt.verify(third_token).unwrap();
        let revoked = service::revoke_access_token(State(app.clone()), bearer(third_token)).await;
        assert_eq!(revoked.status(), StatusCode::NO_CONTENT);
        assert!(!persistence::TradingStore::new(app.pool.clone())
            .validate_access_token(&third_claims)
            .await
            .unwrap());
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn concurrent_refresh_replay_has_one_rotation_and_revokes_the_family(pool: PgPool) {
        let app = state(pool.clone());
        let issued = register(&app, "i1-concurrent@example.test").await;
        let refresh_token = issued["refresh_token"].as_str().unwrap().to_owned();
        let left = service::refresh(
            State(app.clone()),
            Json(service::RefreshRequest {
                refresh_token: refresh_token.clone(),
            }),
        );
        let right = service::refresh(
            State(app.clone()),
            Json(service::RefreshRequest { refresh_token }),
        );
        let (left, right) = tokio::join!(left, right);
        let statuses = [left.status(), right.status()];
        assert!(statuses.contains(&StatusCode::OK));
        assert!(statuses.contains(&StatusCode::UNAUTHORIZED));
        let session_status: String = sqlx::query_scalar("SELECT status FROM identity_sessions WHERE user_id=(SELECT id FROM identity_users WHERE email='i1-concurrent@example.test')")
            .fetch_one(&pool).await.unwrap();
        assert_eq!(session_status, "REVOKED");
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn suspension_revokes_tokens_and_reactivation_does_not_restore_session(pool: PgPool) {
        let app = state(pool.clone());
        let issued = register(&app, "i1-suspension@example.test").await;
        let access = issued["access_token"].as_str().unwrap();
        let refresh = issued["refresh_token"].as_str().unwrap().to_owned();
        let claims = app.jwt.verify(access).unwrap();
        sqlx::query("UPDATE identity_users SET is_active=FALSE WHERE id=$1")
            .bind(&claims.sub)
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("UPDATE identity_users SET is_active=TRUE WHERE id=$1")
            .bind(&claims.sub)
            .execute(&pool)
            .await
            .unwrap();

        assert!(!persistence::TradingStore::new(pool.clone())
            .validate_access_token(&claims)
            .await
            .unwrap());
        assert_eq!(
            service::refresh(
                State(app),
                Json(service::RefreshRequest {
                    refresh_token: refresh,
                }),
            )
            .await
            .status(),
            StatusCode::UNAUTHORIZED
        );
        let (status, revision): (String, i64) =
            sqlx::query_as("SELECT status,revision FROM identity_sessions WHERE id=$1")
                .bind(claims.session_id)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(status, "REVOKED");
        assert_eq!(revision, claims.session_revision + 1);
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn suspension_audit_failure_rolls_back_user_and_session_state(pool: PgPool) {
        let app = state(pool.clone());
        let issued = register(&app, "i1-suspend-fault@example.test").await;
        let access = issued["access_token"].as_str().unwrap();
        let claims = app.jwt.verify(access).unwrap();
        sqlx::query(
            "CREATE FUNCTION qos021a_fail_suspension_event() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN IF NEW.event_type = 'SESSION_REVOKED' AND NEW.reason = 'identity account suspended' THEN RAISE EXCEPTION 'injected suspension audit failure'; END IF; RETURN NEW; END $$",
        )
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query("CREATE TRIGGER qos021a_fail_suspension_event BEFORE INSERT ON identity_session_events FOR EACH ROW EXECUTE FUNCTION qos021a_fail_suspension_event()")
            .execute(&pool)
            .await
            .unwrap();

        let result = sqlx::query("UPDATE identity_users SET is_active=FALSE WHERE id=$1")
            .bind(&claims.sub)
            .execute(&pool)
            .await;
        assert!(result.is_err());
        let is_active: bool =
            sqlx::query_scalar("SELECT is_active FROM identity_users WHERE id=$1")
                .bind(&claims.sub)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert!(is_active);
        assert!(persistence::TradingStore::new(pool.clone())
            .validate_access_token(&claims)
            .await
            .unwrap());
        let trigger_drop =
            sqlx::query("DROP TRIGGER qos021a_fail_suspension_event ON identity_session_events")
                .execute(&pool)
                .await;
        assert!(trigger_drop.is_ok());
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn session_event_truncate_and_cross_user_attribution_are_rejected(pool: PgPool) {
        let app = state(pool.clone());
        let first = register(&app, "i1-event-owner@example.test").await;
        let second = register(&app, "i1-event-other@example.test").await;
        let claims = app
            .jwt
            .verify(first["access_token"].as_str().unwrap())
            .unwrap();
        let other_user = second["user"]["id"].as_str().unwrap();
        let forged_attribution = sqlx::query("INSERT INTO identity_session_events(session_id,user_id,actor_user_id,event_type,session_revision,reason) VALUES($1,$2,$2,'SESSION_ISSUED',1,'cross-user attribution test')")
            .bind(&claims.session_id).bind(other_user).execute(&pool).await;
        assert!(forged_attribution.is_err());
        assert!(sqlx::query("TRUNCATE identity_session_events")
            .execute(&pool)
            .await
            .is_err());
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn concurrent_refresh_and_logout_finish_with_revoked_session(pool: PgPool) {
        let app = state(pool.clone());
        let issued = register(&app, "i1-refresh-logout@example.test").await;
        let access = issued["access_token"].as_str().unwrap().to_owned();
        let refresh_token = issued["refresh_token"].as_str().unwrap().to_owned();
        let refresh = service::refresh(
            State(app.clone()),
            Json(service::RefreshRequest {
                refresh_token: refresh_token.clone(),
            }),
        );
        let logout = service::logout(
            State(app.clone()),
            bearer(&access),
            Json(service::RefreshRequest { refresh_token }),
        );
        let (refresh, logout) = tokio::join!(refresh, logout);
        assert!(matches!(
            refresh.status(),
            StatusCode::OK | StatusCode::UNAUTHORIZED
        ));
        assert!(matches!(
            logout.status(),
            StatusCode::NO_CONTENT | StatusCode::UNAUTHORIZED
        ));
        let claims = app.jwt.verify(&access).unwrap();
        assert!(!persistence::TradingStore::new(pool.clone())
            .validate_access_token(&claims)
            .await
            .unwrap());
        let status: String = sqlx::query_scalar("SELECT status FROM identity_sessions WHERE id=$1")
            .bind(claims.session_id)
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(status, "REVOKED");
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn refresh_event_failure_rolls_back_token_rotation(pool: PgPool) {
        let app = state(pool.clone());
        let issued = register(&app, "i1-refresh-fault@example.test").await;
        let old_refresh = issued["refresh_token"].as_str().unwrap().to_owned();
        sqlx::query(
            "CREATE FUNCTION qos021a_fail_refresh_event() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN IF NEW.event_type = 'SESSION_REFRESHED' THEN RAISE EXCEPTION 'injected refresh audit failure'; END IF; RETURN NEW; END $$",
        )
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query("CREATE TRIGGER qos021a_fail_refresh_event BEFORE INSERT ON identity_session_events FOR EACH ROW EXECUTE FUNCTION qos021a_fail_refresh_event()")
            .execute(&pool)
            .await
            .unwrap();
        let failed = service::refresh(
            State(app.clone()),
            Json(service::RefreshRequest {
                refresh_token: old_refresh.clone(),
            }),
        )
        .await;
        assert_eq!(failed.status(), StatusCode::INTERNAL_SERVER_ERROR);
        sqlx::query("DROP TRIGGER qos021a_fail_refresh_event ON identity_session_events")
            .execute(&pool)
            .await
            .unwrap();
        let retry = service::refresh(
            State(app),
            Json(service::RefreshRequest {
                refresh_token: old_refresh,
            }),
        )
        .await;
        assert_eq!(retry.status(), StatusCode::OK);
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn concurrent_refresh_and_access_token_revocation_preserve_both_intents(pool: PgPool) {
        let app = state(pool.clone());
        let issued = register(&app, "i1-refresh-access-revoke@example.test").await;
        let access = issued["access_token"].as_str().unwrap().to_owned();
        let old_claims = app.jwt.verify(&access).unwrap();
        let refresh = service::refresh(
            State(app.clone()),
            Json(service::RefreshRequest {
                refresh_token: issued["refresh_token"].as_str().unwrap().into(),
            }),
        );
        let revoke = service::revoke_access_token(State(app.clone()), bearer(&access));
        let (refresh, revoke) = tokio::join!(refresh, revoke);
        assert!(matches!(
            refresh.status(),
            StatusCode::OK | StatusCode::UNAUTHORIZED
        ));
        assert!(matches!(
            revoke.status(),
            StatusCode::NO_CONTENT | StatusCode::UNAUTHORIZED
        ));
        assert!(!persistence::TradingStore::new(pool.clone())
            .validate_access_token(&old_claims)
            .await
            .unwrap());
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn concurrent_refresh_and_suspension_leave_user_inactive_and_session_revoked(
        pool: PgPool,
    ) {
        let app = state(pool.clone());
        let issued = register(&app, "i1-refresh-suspension@example.test").await;
        let access = issued["access_token"].as_str().unwrap();
        let claims = app.jwt.verify(access).unwrap();
        let refresh = service::refresh(
            State(app.clone()),
            Json(service::RefreshRequest {
                refresh_token: issued["refresh_token"].as_str().unwrap().into(),
            }),
        );
        let suspension = sqlx::query("UPDATE identity_users SET is_active=FALSE WHERE id=$1")
            .bind(&claims.sub)
            .execute(&pool);
        let (refresh, suspension) = tokio::join!(refresh, suspension);
        assert!(matches!(
            refresh.status(),
            StatusCode::OK | StatusCode::UNAUTHORIZED
        ));
        assert!(suspension.is_ok());
        let (active, status): (bool, String) = sqlx::query_as(
            "SELECT u.is_active,s.status FROM identity_users u JOIN identity_sessions s ON s.user_id=u.id WHERE u.id=$1 AND s.id=$2",
        )
        .bind(&claims.sub)
        .bind(&claims.session_id)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert!(!active);
        assert_eq!(status, "REVOKED");
        assert!(!persistence::TradingStore::new(pool)
            .validate_access_token(&claims)
            .await
            .unwrap());
    }
}

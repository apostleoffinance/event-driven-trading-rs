#![allow(clippy::unwrap_used)]

use std::{borrow::Cow, collections::BTreeMap, path::PathBuf};

use chrono::Utc;
use domain::{
    Account, AccountEnvironment, AccountId, AccountState, AccountType, EnvironmentVerification,
    VenueId,
};
use persistence::{migrate, TradingStore};
use rust_decimal::Decimal;
use sqlx::{migrate::Migrator, PgPool};

fn migration_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../migrations")
}

#[tokio::test]
async fn migration_versions_are_unique() {
    let migrator = Migrator::new(migration_path()).await.unwrap();
    let mut versions = BTreeMap::new();
    for migration in migrator.iter() {
        assert!(
            versions.insert(migration.version, ()).is_none(),
            "duplicate SQLx migration version {}",
            migration.version
        );
    }
}

async fn selected_migrator(pool: &PgPool, predicate: impl Fn(i64) -> bool) {
    let all = Migrator::new(migration_path()).await.unwrap();
    let selected = all
        .iter()
        .filter(|migration| predicate(migration.version))
        .cloned()
        .collect::<Vec<_>>();
    Migrator {
        migrations: Cow::Owned(selected),
        ..Migrator::DEFAULT
    }
    .run(pool)
    .await
    .unwrap();
}

async fn old_full_migrator(pool: &PgPool) {
    selected_migrator(pool, |version| version < 20261111000001).await;
}

#[sqlx::test]
async fn fresh_bootstrap_uses_original_core_migrations_and_is_repeatable(pool: PgPool) {
    migrate(&pool).await.unwrap();
    migrate(&pool).await.unwrap();

    let source = Migrator::new(migration_path()).await.unwrap();
    let expected = source
        .iter()
        .map(|migration| (migration.version, migration.checksum.to_vec()))
        .collect::<BTreeMap<_, _>>();
    let rows = sqlx::query_as::<_, (i64, Vec<u8>, bool)>(
        "SELECT version, checksum, success FROM _sqlx_migrations ORDER BY version",
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(rows.len(), expected.len());
    for (version, checksum, success) in rows {
        assert!(success);
        assert_eq!(expected.get(&version), Some(&checksum));
    }
    let core_count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM _sqlx_migrations WHERE version IN (20260306000001,20260906000001,20261027000001,20261027000002,20261027000003,20261027000004)",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(core_count, 6);
    // Do not infer execution order from SQLx migration versions. The
    // historical journal migrations use 12-digit versions while the core
    // migrations use 14-digit versions, so numeric ordering is not the same
    // as chronological or bootstrap ordering. The core-first behavior is
    // enforced by migrate()'s filtered core migrator above; this test verifies
    // that the complete, checksum-matching migration ledger is repeatable.
}

#[sqlx::test]
async fn concurrent_fresh_bootstraps_serialize_history_classification_and_migration(pool: PgPool) {
    let (first, second) = tokio::join!(migrate(&pool), migrate(&pool));
    assert!(first.is_ok(), "first bootstrap result: {first:?}");
    assert!(second.is_ok(), "second bootstrap result: {second:?}");
    let applied: i64 = sqlx::query_scalar("SELECT count(*) FROM _sqlx_migrations")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(
        applied,
        Migrator::new(migration_path())
            .await
            .unwrap()
            .iter()
            .count() as i64
    );
}

#[sqlx::test]
async fn exact_six_core_history_upgrades_through_full_migrator(pool: PgPool) {
    selected_migrator(&pool, |version| {
        [
            20260306000001,
            20260906000001,
            20261027000001,
            20261027000002,
            20261027000003,
            20261027000004,
        ]
        .contains(&version)
    })
    .await;
    migrate(&pool).await.unwrap();
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM _sqlx_migrations")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(
        count,
        Migrator::new(migration_path())
            .await
            .unwrap()
            .iter()
            .count() as i64
    );
}

#[sqlx::test]
async fn complete_journal_first_history_receives_forward_corrections(pool: PgPool) {
    old_full_migrator(&pool).await;
    let legacy_versions: Vec<i64> =
        sqlx::query_scalar("SELECT version FROM _sqlx_migrations ORDER BY version")
            .fetch_all(&pool)
            .await
            .unwrap();

    let full_migrator = Migrator::new(migration_path()).await.unwrap();
    let expected_forward_versions = full_migrator
        .iter()
        .filter(|migration| migration.version >= 20261111000001)
        .map(|migration| migration.version)
        .collect::<Vec<_>>();
    // Forward migrations are intentionally append-only. Derive the expected
    // count from the migration inventory instead of freezing a count here;
    // adding a valid migration must not make this historical-upgrade test stale.
    assert!(
        expected_forward_versions.len() >= 10,
        "the journal-first fixture should include the current forward migration set",
    );

    migrate(&pool).await.unwrap();

    let applied_versions: Vec<i64> =
        sqlx::query_scalar("SELECT version FROM _sqlx_migrations ORDER BY version")
            .fetch_all(&pool)
            .await
            .unwrap();
    assert_eq!(
        &applied_versions[..legacy_versions.len()],
        legacy_versions.as_slice(),
        "forward correction must preserve the complete historical migration prefix",
    );
    assert_eq!(
        &applied_versions[legacy_versions.len()..],
        expected_forward_versions.as_slice(),
        "journal-first history must receive every currently defined forward migration",
    );
}

#[sqlx::test]
async fn i1_upgrade_revokes_legacy_refresh_tokens_without_promoting_sessions(pool: PgPool) {
    old_full_migrator(&pool).await;
    sqlx::query("INSERT INTO identity_users(id,email,display_name,password_hash) VALUES('i1-legacy-user','i1-legacy@example.test','Legacy','test-only')")
        .execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO identity_refresh_tokens(id,user_id,token_hash,expires_at) VALUES('i1-legacy-refresh','i1-legacy-user',repeat('a',64),NOW()+INTERVAL '1 day')")
        .execute(&pool).await.unwrap();

    migrate(&pool).await.unwrap();

    let (revoked, session_id): (Option<chrono::DateTime<Utc>>, Option<String>) = sqlx::query_as(
        "SELECT revoked_at,session_id FROM identity_refresh_tokens WHERE id='i1-legacy-refresh'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(revoked.is_some());
    assert!(session_id.is_none());
    let sessions: i64 =
        sqlx::query_scalar("SELECT count(*) FROM identity_sessions WHERE user_id='i1-legacy-user'")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(sessions, 0);
}

#[sqlx::test]
async fn r2_upgrade_preserves_legacy_accounts_and_does_not_infer_tenants(pool: PgPool) {
    old_full_migrator(&pool).await;
    sqlx::query("INSERT INTO identity_users(id,email,display_name,password_hash) VALUES('legacy-user','legacy@example.test','Legacy user','test-only')")
        .execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO venues(id,venue_type,name,created_at) VALUES('legacy-venue','simulated','Legacy venue',NOW())")
        .execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO accounts(id,account_type,status,venue_id,starting_capital,created_at,environment) VALUES('legacy-tenant-account','simulated','active','legacy-venue',100,NOW(),'PAPER')")
        .execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO dashboard_account_access(owner_id,account_id) VALUES('legacy-user','legacy-tenant-account')")
        .execute(&pool).await.unwrap();
    migrate(&pool).await.unwrap();
    let (org_count, membership_count, entitlement_count, grant_count): (i64,i64,i64,i64) = sqlx::query_as("SELECT (SELECT count(*) FROM organizations),(SELECT count(*) FROM organization_memberships),(SELECT count(*) FROM organization_account_entitlements),(SELECT count(*) FROM dashboard_account_access WHERE owner_id='legacy-user' AND account_id='legacy-tenant-account')")
        .fetch_one(&pool).await.unwrap();
    assert_eq!(
        (org_count, membership_count, entitlement_count, grant_count),
        (0, 0, 0, 1)
    );
    let (environment,verification,account_revision): (String,String,i64) = sqlx::query_as("SELECT environment,environment_verification,account_revision FROM accounts WHERE id='legacy-tenant-account'")
        .fetch_one(&pool).await.unwrap();
    assert_eq!(environment, "PAPER");
    assert_eq!(verification, "UNVERIFIED_LEGACY");
    assert_eq!(account_revision, 1);
}

#[sqlx::test]
async fn exact_e2_to_r2_upgrade_preserves_verified_account_grants_and_journal(pool: PgPool) {
    selected_migrator(&pool, |version| version < 20261112000001).await;
    sqlx::query("INSERT INTO identity_users(id,email,display_name,password_hash) VALUES('e2-owner','e2@example.test','E2 owner','test-only')")
        .execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO venues(id,venue_type,name,created_at) VALUES('e2-venue','simulated','E2 venue',NOW())")
        .execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO accounts(id,account_type,status,venue_id,starting_capital,created_at,environment,environment_verification,environment_revision,environment_verified_at,environment_verified_by,environment_verification_reason) VALUES('e2-account','simulated','active','e2-venue',100,NOW(),'PAPER','VERIFIED',4,NOW(),'e2-owner','verified fixture')")
        .execute(&pool).await.unwrap();
    sqlx::query(
        "INSERT INTO dashboard_account_access(owner_id,account_id) VALUES('e2-owner','e2-account')",
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO journal_accounts(id,owner_id,name,linked_trading_account_id) VALUES('e2-journal','e2-owner','E2 Journal','e2-account')")
        .execute(&pool).await.unwrap();
    let old_history = sqlx::query_as::<_, (i64, Vec<u8>)>(
        "SELECT version,checksum FROM _sqlx_migrations ORDER BY version",
    )
    .fetch_all(&pool)
    .await
    .unwrap();

    migrate(&pool).await.unwrap();

    let (organizations,memberships,entitlements,grants,journal):(i64,i64,i64,i64,i64)=sqlx::query_as("SELECT (SELECT count(*) FROM organizations),(SELECT count(*) FROM organization_memberships),(SELECT count(*) FROM organization_account_entitlements),(SELECT count(*) FROM dashboard_account_access WHERE owner_id='e2-owner' AND account_id='e2-account'),(SELECT count(*) FROM journal_accounts WHERE id='e2-journal' AND linked_trading_account_id='e2-account')")
        .fetch_one(&pool).await.unwrap();
    assert_eq!(
        (organizations, memberships, entitlements, grants, journal),
        (0, 0, 0, 1, 1)
    );
    let (environment,verification,revision,verified_by,reason):(String,String,i64,Option<String>,Option<String>)=sqlx::query_as("SELECT environment,environment_verification,environment_revision,environment_verified_by,environment_verification_reason FROM accounts WHERE id='e2-account'")
        .fetch_one(&pool).await.unwrap();
    assert_eq!(
        (
            environment.as_str(),
            verification.as_str(),
            revision,
            verified_by.as_deref(),
            reason.as_deref()
        ),
        (
            "PAPER",
            "VERIFIED",
            4,
            Some("e2-owner"),
            Some("verified fixture")
        )
    );
    let new_history=sqlx::query_as::<_,(i64,Vec<u8>)>("SELECT version,checksum FROM _sqlx_migrations WHERE version < 20261112000001 ORDER BY version")
        .fetch_all(&pool).await.unwrap();
    assert_eq!(new_history, old_history);
    let r2_count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM _sqlx_migrations WHERE version=20261112000002 AND success",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(r2_count, 1);
}

#[sqlx::test]
async fn partial_history_fails_closed_without_applying_more_migrations(pool: PgPool) {
    selected_migrator(&pool, |version| version == 20260306000001).await;
    assert!(migrate(&pool).await.is_err());
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM _sqlx_migrations")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 1);
}

#[sqlx::test]
async fn empty_migration_history_with_application_tables_is_not_fresh(pool: PgPool) {
    sqlx::query("CREATE TABLE untracked_application_table(id TEXT PRIMARY KEY)")
        .execute(&pool)
        .await
        .unwrap();
    assert!(migrate(&pool).await.is_err());
    let history_exists: bool =
        sqlx::query_scalar("SELECT to_regclass('public._sqlx_migrations') IS NOT NULL")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(!history_exists);
}

#[sqlx::test]
async fn recognized_core_history_with_missing_schema_fails_closed(pool: PgPool) {
    selected_migrator(&pool, |version| {
        [
            20260306000001,
            20260906000001,
            20261027000001,
            20261027000002,
            20261027000003,
            20261027000004,
        ]
        .contains(&version)
    })
    .await;
    sqlx::query("DROP TABLE orders CASCADE")
        .execute(&pool)
        .await
        .unwrap();
    assert!(migrate(&pool).await.is_err());
}

#[sqlx::test]
async fn failed_dirty_history_fails_closed(pool: PgPool) {
    selected_migrator(&pool, |version| version == 20260306000001).await;
    sqlx::query("UPDATE _sqlx_migrations SET success=FALSE WHERE version=20260306000001")
        .execute(&pool)
        .await
        .unwrap();
    assert!(migrate(&pool).await.is_err());
    let success: bool =
        sqlx::query_scalar("SELECT success FROM _sqlx_migrations WHERE version=20260306000001")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(!success);
}

#[sqlx::test]
async fn legacy_environments_are_preserved_unverified_and_new_accounts_are_explicit(pool: PgPool) {
    old_full_migrator(&pool).await;
    sqlx::query("INSERT INTO venues(id,venue_type,name,created_at) VALUES('simulated','simulated','Simulator',NOW())")
        .execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO accounts(id,account_type,status,venue_id,starting_capital,created_at,environment) VALUES ('legacy-paper','simulated','active','simulated',100,NOW(),'PAPER'),('legacy-shadow','personal','active','simulated',100,NOW(),'SHADOW'),('legacy-live','personal','active','simulated',100,NOW(),'LIVE')")
        .execute(&pool).await.unwrap();
    migrate(&pool).await.unwrap();
    let rows = sqlx::query_as::<_, (String, String, i64)>(
        "SELECT id, environment, environment_revision FROM accounts WHERE id LIKE 'legacy-%' ORDER BY id",
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(
        rows,
        vec![
            ("legacy-live".into(), "LIVE".into(), 0),
            ("legacy-paper".into(), "PAPER".into(), 0),
            ("legacy-shadow".into(), "SHADOW".into(), 0)
        ]
    );
    let state: String =
        sqlx::query_scalar("SELECT environment_verification FROM accounts WHERE id='legacy-paper'")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(state, "UNVERIFIED_LEGACY");

    let omitted = sqlx::query("INSERT INTO accounts(id,account_type,status,venue_id,starting_capital,created_at) VALUES('missing-env','simulated','active','simulated',100,NOW())")
        .execute(&pool).await;
    assert!(omitted.is_err());
    sqlx::query("INSERT INTO accounts(id,account_type,status,venue_id,starting_capital,created_at,environment) VALUES('new-paper','simulated','active','simulated',100,NOW(),'PAPER')")
        .execute(&pool).await.unwrap();
    let (verification, revision): (String, i64) = sqlx::query_as(
        "SELECT environment_verification,environment_revision FROM accounts WHERE id='new-paper'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!((verification.as_str(), revision), ("UNVERIFIED", 1));
}

#[sqlx::test]
async fn account_environment_round_trip_verification_and_revision_guard(pool: PgPool) {
    migrate(&pool).await.unwrap();
    sqlx::query("INSERT INTO venues(id,venue_type,name,created_at) VALUES('simulated','simulated','Simulator',NOW())")
        .execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO identity_users(id,email,display_name,password_hash) VALUES('operator-1','operator@example.test','Operator','not-a-real-hash')")
        .execute(&pool).await.unwrap();
    let id = AccountId::new("account-roundtrip").unwrap();
    let now = Utc::now();
    let account = Account::new(
        id.clone(),
        AccountType::Simulated,
        VenueId::new("simulated").unwrap(),
        AccountEnvironment::Paper,
        now,
    );
    let state = AccountState::new(id.clone(), Decimal::from(100), now).unwrap();
    let store = TradingStore::new(pool.clone());
    store
        .upsert_account(&account, Decimal::from(100), None, &state)
        .await
        .unwrap();
    let saved = store.load_account(&id).await.unwrap();
    assert_eq!(saved.environment, Some(AccountEnvironment::Paper));
    assert_eq!(
        saved.environment_verification,
        EnvironmentVerification::Unverified
    );
    assert_eq!(saved.environment_revision, 1);
    assert_eq!(
        store
            .verify_account_environment(
                &id,
                AccountEnvironment::Live,
                "operator-1",
                1,
                Some("verified test fixture")
            )
            .await
            .unwrap(),
        2
    );
    let verified = store.load_account(&id).await.unwrap();
    assert_eq!(verified.environment, Some(AccountEnvironment::Live));
    assert!(matches!(
        verified.environment_verification,
        EnvironmentVerification::Verified { revision: 2, .. }
    ));
    assert_eq!(
        verified.environment_verified_by.as_deref(),
        Some("operator-1")
    );
    assert_eq!(
        domain::require_account_broker_eligibility(
            verified.environment,
            verified.environment_verification,
            verified.environment_revision,
            verified.environment_verified_by.as_deref(),
            Utc::now(),
        ),
        Ok(2)
    );
    assert!(store
        .verify_account_environment(&id, AccountEnvironment::Paper, "operator-1", 1, None)
        .await
        .is_err());
    let (left, right) = tokio::join!(
        store.verify_account_environment(&id, AccountEnvironment::Live, "operator-1", 2, None),
        store.verify_account_environment(&id, AccountEnvironment::Shadow, "operator-1", 2, None),
    );
    assert_ne!(left.is_ok(), right.is_ok());
    assert_eq!(
        store.load_account(&id).await.unwrap().environment_revision,
        3
    );
}

#[sqlx::test]
async fn journal_account_link_enforces_fk_and_repair_sql_is_repeatable(pool: PgPool) {
    migrate(&pool).await.unwrap();
    sqlx::query("INSERT INTO venues(id,venue_type,name,created_at) VALUES('simulated','simulated','Simulator',NOW())")
        .execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO accounts(id,account_type,status,venue_id,starting_capital,created_at,environment) VALUES('trade-1','simulated','active','simulated',100,NOW(),'PAPER')")
        .execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO journal_accounts(id,owner_id,name,linked_trading_account_id) VALUES('journal-1','owner-1','Journal','trade-1')")
        .execute(&pool).await.unwrap();
    let orphan = sqlx::query("INSERT INTO journal_accounts(id,owner_id,name,linked_trading_account_id) VALUES('journal-orphan','owner-1','Orphan','missing-trade')")
        .execute(&pool).await;
    assert!(orphan.is_err());
    sqlx::raw_sql(include_str!(
        "../../../migrations/20261111000001_journal_trading_account_fk_repair.sql"
    ))
    .execute(&pool)
    .await
    .unwrap();
    sqlx::raw_sql(include_str!(
        "../../../migrations/20261111000001_journal_trading_account_fk_repair.sql"
    ))
    .execute(&pool)
    .await
    .unwrap();
}

#[sqlx::test]
async fn orphaned_journal_reference_aborts_fk_migration(pool: PgPool) {
    old_full_migrator(&pool).await;
    sqlx::query("INSERT INTO journal_accounts(id,owner_id,name,linked_trading_account_id) VALUES('orphan-journal','owner','Orphan','absent-account')")
        .execute(&pool).await.unwrap();
    assert!(migrate(&pool).await.is_err());
    let constraint_exists: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM pg_constraint WHERE conname='fk_journal_accounts_trading_account')")
        .fetch_one(&pool).await.unwrap();
    assert!(!constraint_exists);
}

#[sqlx::test]
async fn conflicting_journal_fk_aborts_without_replacing_it(pool: PgPool) {
    old_full_migrator(&pool).await;
    sqlx::query("ALTER TABLE journal_accounts ADD CONSTRAINT fk_journal_accounts_trading_account FOREIGN KEY(linked_trading_account_id) REFERENCES venues(id)")
        .execute(&pool).await.unwrap();
    assert!(migrate(&pool).await.is_err());
    let references_venues: bool = sqlx::query_scalar("SELECT confrelid='venues'::regclass FROM pg_constraint WHERE conname='fk_journal_accounts_trading_account'")
        .fetch_one(&pool).await.unwrap();
    assert!(references_venues);
}

#[sqlx::test]
async fn correct_preexisting_journal_fk_is_recognized(pool: PgPool) {
    old_full_migrator(&pool).await;
    sqlx::query("ALTER TABLE journal_accounts ADD CONSTRAINT legacy_correct_trading_account_fk FOREIGN KEY(linked_trading_account_id) REFERENCES accounts(id) ON UPDATE NO ACTION ON DELETE NO ACTION")
        .execute(&pool).await.unwrap();
    migrate(&pool).await.unwrap();

    // A subsequent service startup must validate the completed E2 schema
    // structurally, accepting an equivalent validated FK regardless of its
    // historical constraint name.
    migrate(&pool).await.unwrap();

    let validated: bool = sqlx::query_scalar(
        "SELECT convalidated FROM pg_constraint WHERE conname='legacy_correct_trading_account_fk'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(validated);
    let duplicate: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM pg_constraint WHERE conname='fk_journal_accounts_trading_account')")
        .fetch_one(&pool).await.unwrap();
    assert!(!duplicate);
}

#[sqlx::test]
async fn full_core_history_with_divergent_checksum_fails_closed(pool: PgPool) {
    selected_migrator(&pool, |version| {
        [
            20260306000001,
            20260906000001,
            20261027000001,
            20261027000002,
            20261027000003,
            20261027000004,
        ]
        .contains(&version)
    })
    .await;
    sqlx::query("UPDATE _sqlx_migrations SET checksum=decode(repeat('00',48),'hex') WHERE version=20260306000001")
        .execute(&pool).await.unwrap();
    assert!(migrate(&pool).await.is_err());
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM _sqlx_migrations")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 6);
}

#[sqlx::test]
async fn durable_authorization_claim_schema_enforces_identity_and_replay_keys(pool: PgPool) {
    migrate(&pool).await.unwrap();
    sqlx::query("INSERT INTO identity_users(id,email,display_name,password_hash) VALUES('auth-owner','auth@example.test','Auth owner','not-a-real-hash')")
        .execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO venues(id,venue_type,name,created_at) VALUES('auth-venue','simulated','Auth venue',NOW())")
        .execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO accounts(id,account_type,status,venue_id,starting_capital,created_at,environment) VALUES('auth-account','simulated','active','auth-venue',100,NOW(),'PAPER')")
        .execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO dashboard_account_access(owner_id,account_id) VALUES('auth-owner','auth-account')")
        .execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO instruments(id,instrument_type,symbol) VALUES('auth-instrument','forex','EURUSD')")
        .execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO strategies(id,name) VALUES('auth-strategy','Auth strategy')")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO strategy_versions(strategy_id,version) VALUES('auth-strategy','1')")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO risk_profiles(id,name,policy_json) VALUES('auth-risk','Auth risk','{}')",
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO strategy_deployments(id,strategy_id,strategy_version,account_id,risk_profile_id,created_at) VALUES('auth-deployment','auth-strategy','1','auth-account','auth-risk',NOW())")
        .execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO trade_intents(id,strategy_id,strategy_version,deployment_id,instrument_id,side,created_at) VALUES('auth-intent','auth-strategy','1','auth-deployment','auth-instrument','buy',NOW())")
        .execute(&pool).await.unwrap();
    let risk_id: i64 = sqlx::query_scalar("INSERT INTO risk_decisions(trade_intent_id,account_id,decision_type,decision_json,created_at) VALUES('auth-intent','auth-account','approved','{}',NOW()) RETURNING id")
        .fetch_one(&pool).await.unwrap();
    sqlx::query("INSERT INTO orders(id,client_order_id,account_id,venue_id,deployment_id,strategy_id,trade_intent_id,instrument_id,side,order_type,time_in_force,quantity,filled_quantity,status,created_at,updated_at) VALUES('auth-order','auth-client','auth-account','auth-venue','auth-deployment','auth-strategy','auth-intent','auth-instrument','buy','market','gtc',1,0,'created',NOW(),NOW())")
        .execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO execution_authorizations(id,owner_id,account_id,broker_id,broker_server_id,environment,environment_revision,order_id,canonical_order_sha256,client_order_id,risk_decision_id,nonce,state,issued_at,expires_at) VALUES('authz-1','auth-owner','auth-account','broker-x','server-a','PAPER',1,'auth-order',repeat('a',64),'auth-client',$1,'nonce-1','ISSUED',NOW(),NOW()+INTERVAL '1 minute')")
        .bind(risk_id).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO execution_claims(id,account_id,client_order_id,authorization_id,order_id,lifecycle_state,submission_attempt_id) VALUES('claim-1','auth-account','auth-client','authz-1','auth-order','CLAIMED','attempt-1')")
        .execute(&pool).await.unwrap();
    let duplicate = sqlx::query("INSERT INTO execution_claims(id,account_id,client_order_id,authorization_id,order_id,lifecycle_state) VALUES('claim-2','auth-account','auth-client','authz-1','auth-order','CLAIMED')")
        .execute(&pool).await;
    assert!(duplicate.is_err());
    sqlx::query(
        "UPDATE execution_claims SET lifecycle_state='UNKNOWN',updated_at=NOW() WHERE id='claim-1'",
    )
    .execute(&pool)
    .await
    .unwrap();
    let state: String =
        sqlx::query_scalar("SELECT lifecycle_state FROM execution_claims WHERE id='claim-1'")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(state, "UNKNOWN");
    let expired = sqlx::query("INSERT INTO execution_authorizations(id,owner_id,account_id,broker_id,broker_server_id,environment,environment_revision,order_id,canonical_order_sha256,client_order_id,risk_decision_id,nonce,state,issued_at,expires_at) VALUES('authz-expired','auth-owner','auth-account','broker-x','server-a','PAPER',1,'auth-order',repeat('b',64),'auth-client',$1,'nonce-expired','ISSUED',NOW(),NOW())")
        .bind(risk_id).execute(&pool).await;
    assert!(expired.is_err());
}

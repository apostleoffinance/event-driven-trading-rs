#![allow(clippy::unwrap_used)]

use std::{borrow::Cow, path::PathBuf};

use chrono::Utc;
use domain::{Organization, OrganizationId};
use persistence::{migrate, BrokerBindingTransition, PersistenceError, TradingStore};
use sqlx::{migrate::Migrator, PgPool};

fn migration_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../migrations")
}

fn lookup(version: &str, token: [u8; 32]) -> Vec<(String, [u8; 32])> {
    vec![(version.to_owned(), token)]
}

async fn apply_before_r3(pool: &PgPool) {
    let all = Migrator::new(migration_path()).await.unwrap();
    let before_r3 = all
        .iter()
        .filter(|migration| migration.version < 20261113000001)
        .cloned()
        .collect::<Vec<_>>();
    assert!(
        before_r3.iter().any(|migration| migration.version == 202611070001),
        "pre-R3 history must include the Journal foundation migration"
    );

    // SQLx validates all registered migrations when running a selected set.
    // Use the selected history once, on an empty schema, to construct a true
    // R2-era database. Then apply the real R3 and M1 SQL as a controlled schema
    // upgrade fixture; do not manufacture _sqlx_migrations entries.
    Migrator {
        migrations: Cow::Owned(before_r3),
        ..Migrator::DEFAULT
    }
    .run(pool)
    .await
    .unwrap();

    let r3 = all
        .iter()
        .find(|migration| migration.version == 20261113000001)
        .expect("R3 migration must exist exactly once");
    sqlx::raw_sql(std::str::from_utf8(r3.sql.as_ref()).unwrap())
        .execute(pool)
        .await
        .unwrap();

    // R3 is the schema under test. Apply M1's source SQL directly because M1
    // is the known downstream consumer of R3's composite entitlement key.
    let m1 = all
        .iter()
        .find(|migration| migration.version == 20261125000001)
        .expect("M1 workspace migration must exist exactly once");
    sqlx::raw_sql(std::str::from_utf8(m1.sql.as_ref()).unwrap())
        .execute(pool)
        .await
        .unwrap();
}


async fn user(pool: &PgPool, id: &str) {
    sqlx::query("INSERT INTO identity_users(id,email,display_name,password_hash) VALUES($1,$2,$1,'test-only')")
        .bind(id)
        .bind(format!("{id}@example.test"))
        .execute(pool)
        .await
        .unwrap();
}

async fn organization(pool: &PgPool, id: &str, owner: &str) {
    user(pool, owner).await;
    TradingStore::new(pool.clone())
        .create_organization(
            &Organization::new(
                OrganizationId::new(id).unwrap(),
                "Registry test",
                Utc::now(),
            )
            .unwrap(),
            owner,
        )
        .await
        .unwrap();
}

async fn quant_account(pool: &PgPool, id: &str, org: &str, owner: &str) {
    sqlx::query("INSERT INTO venues(id,venue_type,name,created_at) VALUES('simulated','simulated','Paper',NOW()) ON CONFLICT(id) DO NOTHING")
        .execute(pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO accounts(id,account_type,status,venue_id,starting_capital,created_at,environment) VALUES($1,'simulated','active','simulated',100,NOW(),'PAPER')")
        .bind(id)
        .execute(pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO account_states(account_id,balance,equity,used_margin,open_position_count,daily_realized_pnl,peak_equity,kill_switch,updated_at) VALUES($1,100,100,0,0,0,100,FALSE,NOW())")
        .bind(id)
        .execute(pool)
        .await
        .unwrap();
    TradingStore::new(pool.clone())
        .assign_account_entitlement(org, id, owner, 1, 0, "R3 fixture")
        .await
        .unwrap();
}

async fn seed_catalog(pool: &PgPool) {
    sqlx::query("INSERT INTO brokers(id,canonical_name,display_name,normalization_version) VALUES('example-broker','example-broker','Example Broker',1)")
        .execute(pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO broker_servers(id,broker_id,canonical_identifier,display_name,provider_type,environment,normalization_version,source_reference) VALUES('server-demo','example-broker','demo.example','Demo Server','MT5_PYTHON','DEMO',1,'controlled-test-seed')")
        .execute(pool)
        .await
        .unwrap();
}

#[sqlx::test]
async fn fresh_install_applies_r3_schema_without_invented_catalogue_or_accounts(pool: PgPool) {
    migrate(&pool).await.unwrap();
    let (tables, migration_count, broker_count, account_count): (i64, i64, i64, i64) = sqlx::query_as(
        "SELECT (SELECT count(*) FROM information_schema.tables WHERE table_schema='public' AND table_name=ANY(ARRAY['brokers','broker_servers','external_broker_accounts','external_broker_account_identifier_tokens','trading_account_broker_bindings','broker_account_evidence','broker_account_verifications','trading_account_broker_binding_events'])),(SELECT count(*) FROM _sqlx_migrations WHERE version=20261113000001 AND success),(SELECT count(*) FROM brokers),(SELECT count(*) FROM external_broker_accounts)",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        (tables, migration_count, broker_count, account_count),
        (8, 1, 0, 0)
    );
    migrate(&pool).await.unwrap();
    let duplicate_migration: i64 =
        sqlx::query_scalar("SELECT count(*) FROM _sqlx_migrations WHERE version=20261113000001")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(duplicate_migration, 1);
}

#[sqlx::test]
async fn exact_r2_upgrade_preserves_tenant_and_environment_without_identity_backfill(pool: PgPool) {
    apply_before_r3(&pool).await;
    organization(&pool, "r3-upgrade-org", "r3-upgrade-owner").await;
    quant_account(
        &pool,
        "r3-upgrade-account",
        "r3-upgrade-org",
        "r3-upgrade-owner",
    )
    .await;
    let before: (Option<String>, String, i64, i64) = sqlx::query_as(
        "SELECT a.environment,a.environment_verification,a.environment_revision,e.revision FROM accounts a JOIN organization_account_entitlements e ON e.account_id=a.id WHERE a.id='r3-upgrade-account'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    let after: (Option<String>, String, i64, i64) = sqlx::query_as(
        "SELECT a.environment,a.environment_verification,a.environment_revision,e.revision FROM accounts a JOIN organization_account_entitlements e ON e.account_id=a.id WHERE a.id='r3-upgrade-account'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    let (organizations, bindings, external_accounts): (i64, i64, i64) = sqlx::query_as(
        "SELECT (SELECT count(*) FROM organizations WHERE id='r3-upgrade-org'),(SELECT count(*) FROM trading_account_broker_bindings),(SELECT count(*) FROM external_broker_accounts)",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(before, after);
    assert_eq!((organizations, bindings, external_accounts), (1, 0, 0));
}

#[sqlx::test]
async fn pending_registration_is_tenant_bound_and_duplicate_identity_is_private(pool: PgPool) {
    migrate(&pool).await.unwrap();
    organization(&pool, "r3-org-a", "r3-owner-a").await;
    organization(&pool, "r3-org-b", "r3-owner-b").await;
    quant_account(&pool, "r3-quant-a", "r3-org-a", "r3-owner-a").await;
    quant_account(&pool, "r3-quant-b", "r3-org-b", "r3-owner-b").await;
    seed_catalog(&pool).await;
    let store = TradingStore::new(pool.clone());
    let legacy_token = [0x5a_u8; 32];
    let active_token = [0x5b_u8; 32];
    let keyset = vec![
        ("legacy-v1".to_owned(), legacy_token),
        ("active-v2".to_owned(), active_token),
    ];
    store
        .register_pending_broker_binding(
            "r3-org-a",
            "r3-owner-a",
            "r3-quant-a",
            "example-broker",
            "server-demo",
            "external-a",
            "binding-a",
            1,
            "active-v2",
            &keyset,
            "Owner registration",
        )
        .await
        .unwrap();
    let (external_hash, raw_found, status, verification, token_aliases): (Vec<u8>, bool, String, String, i64) = sqlx::query_as(
        "SELECT e.identifier_lookup_hmac,position('001234' in encode(e.identifier_lookup_hmac,'hex'))>0,b.status,e.verification_status,(SELECT count(*) FROM external_broker_account_identifier_tokens t WHERE t.external_broker_account_id=e.id) FROM external_broker_accounts e JOIN trading_account_broker_bindings b ON b.external_broker_account_id=e.id WHERE e.id='external-a'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(external_hash, active_token);
    assert_eq!(token_aliases, 2);
    assert!(!raw_found);
    assert_eq!(
        (status.as_str(), verification.as_str()),
        ("PENDING_EVIDENCE", "PENDING_EVIDENCE")
    );
    assert_eq!(store.broker_bindings("r3-org-a").await.unwrap().len(), 1);
    assert!(store.broker_bindings("r3-org-b").await.unwrap().is_empty());

    let duplicate = store
        .register_pending_broker_binding(
            "r3-org-b",
            "r3-owner-b",
            "r3-quant-b",
            "example-broker",
            "server-demo",
            "external-b",
            "binding-b",
            1,
            "active-v3",
            &[
                ("legacy-v1".to_owned(), legacy_token),
                ("active-v3".to_owned(), [0x5c_u8; 32]),
            ],
            "Competing registration",
        )
        .await;
    assert!(matches!(duplicate, Err(PersistenceError::Other(_))));
    let (external_count, binding_count): (i64, i64) = sqlx::query_as(
        "SELECT (SELECT count(*) FROM external_broker_accounts),(SELECT count(*) FROM trading_account_broker_bindings)",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!((external_count, binding_count), (1, 1));
}

#[sqlx::test]
async fn binding_lifecycle_is_revisioned_and_history_is_append_only(pool: PgPool) {
    migrate(&pool).await.unwrap();
    organization(&pool, "r3-life-org", "r3-life-owner").await;
    quant_account(&pool, "r3-life-account", "r3-life-org", "r3-life-owner").await;
    seed_catalog(&pool).await;
    let store = TradingStore::new(pool.clone());
    store
        .register_pending_broker_binding(
            "r3-life-org",
            "r3-life-owner",
            "r3-life-account",
            "example-broker",
            "server-demo",
            "external-life",
            "binding-life",
            1,
            "test-key-v1",
            &lookup("test-key-v1", [7; 32]),
            "Pending registration",
        )
        .await
        .unwrap();
    let revision = store
        .transition_broker_binding(
            "r3-life-org",
            "r3-life-owner",
            "binding-life",
            1,
            BrokerBindingTransition::Suspended,
            "Owner requested suspension",
        )
        .await
        .unwrap();
    assert_eq!(revision, 2);
    let stale = store
        .transition_broker_binding(
            "r3-life-org",
            "r3-life-owner",
            "binding-life",
            1,
            BrokerBindingTransition::Revoked,
            "Stale transition",
        )
        .await;
    assert!(matches!(stale, Err(PersistenceError::Other(_))));
    let count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM trading_account_broker_binding_events WHERE binding_id='binding-life'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(count, 2);
    let mutate = sqlx::query("UPDATE trading_account_broker_binding_events SET reason='rewritten' WHERE binding_id='binding-life'")
        .execute(&pool).await;
    assert!(mutate.is_err());
}

#[sqlx::test]
async fn evidence_must_match_account_identity_and_verified_requires_authenticated_evidence(
    pool: PgPool,
) {
    migrate(&pool).await.unwrap();
    organization(&pool, "r3-evidence-org", "r3-evidence-owner").await;
    quant_account(
        &pool,
        "r3-evidence-account",
        "r3-evidence-org",
        "r3-evidence-owner",
    )
    .await;
    seed_catalog(&pool).await;
    let store = TradingStore::new(pool.clone());
    store
        .register_pending_broker_binding(
            "r3-evidence-org",
            "r3-evidence-owner",
            "r3-evidence-account",
            "example-broker",
            "server-demo",
            "external-evidence",
            "binding-evidence",
            1,
            "test-key-v1",
            &lookup("test-key-v1", [8; 32]),
            "Evidence test",
        )
        .await
        .unwrap();
    let mismatch = sqlx::query("INSERT INTO broker_account_evidence(id,external_broker_account_id,provider_type,evidence_type,observed_broker_id,observed_broker_server_id,observed_identifier_key_version,observed_identifier_normalization_version,observed_identifier_hmac,attestation_key_id,attestation_digest,observed_at,evidence_status,source_principal) VALUES('evidence-mismatch','external-evidence','MT5_PYTHON','account_metadata','example-broker','server-demo','test-key-v1',1,$1,'test-attestor',$2,NOW(),'AUTHENTICATED','test-provider')")
        .bind([9_u8; 32].as_slice()).bind([4_u8; 32].as_slice()).execute(&pool).await;
    assert!(mismatch.is_err());
    let no_evidence = sqlx::query("INSERT INTO broker_account_verifications(id,external_broker_account_id,decision,revision,approval_actor_id,approval_authority,approval_reference,reason) VALUES('forged-verified','external-evidence','VERIFIED',2,'r3-evidence-owner','ORG_OWNER','self-approved','must fail')")
        .execute(&pool).await;
    assert!(no_evidence.is_err());
    let status: String = sqlx::query_scalar(
        "SELECT verification_status FROM external_broker_accounts WHERE id='external-evidence'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(status, "PENDING_EVIDENCE");
}

#[sqlx::test]
async fn database_rejects_cross_tenant_binding_and_duplicate_catalog_identity(pool: PgPool) {
    migrate(&pool).await.unwrap();
    organization(&pool, "r3-constraint-org", "r3-constraint-owner").await;
    quant_account(
        &pool,
        "r3-constraint-account",
        "r3-constraint-org",
        "r3-constraint-owner",
    )
    .await;
    seed_catalog(&pool).await;
    let duplicate_server = sqlx::query("INSERT INTO broker_servers(id,broker_id,canonical_identifier,display_name,provider_type,environment,normalization_version,source_reference) VALUES('server-demo-duplicate','example-broker','demo.example','Duplicate','MT5_PYTHON','DEMO',1,'test')")
        .execute(&pool).await;
    assert!(duplicate_server.is_err());
    let store = TradingStore::new(pool.clone());
    let no_entitlement = store
        .register_pending_broker_binding(
            "r3-constraint-org",
            "r3-constraint-owner",
            "unentitled-account",
            "example-broker",
            "server-demo",
            "external-no-entitlement",
            "binding-no-entitlement",
            1,
            "test-key-v1",
            &lookup("test-key-v1", [6; 32]),
            "Must not bind",
        )
        .await;
    assert!(matches!(no_entitlement, Err(PersistenceError::NotFound(_))));
    let ext_count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM external_broker_accounts WHERE id='external-no-entitlement'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(ext_count, 0);
    let owners: i64 = sqlx::query_scalar("SELECT count(*) FROM organization_memberships WHERE organization_id='r3-constraint-org' AND role='OWNER' AND status='ACTIVE'")
        .fetch_one(&pool).await.unwrap();
    assert_eq!(owners, 1);
}

#[sqlx::test]
async fn concurrent_cross_tenant_registration_of_same_external_identity_has_one_winner(
    pool: PgPool,
) {
    migrate(&pool).await.unwrap();
    organization(&pool, "r3-race-org-a", "r3-race-owner-a").await;
    organization(&pool, "r3-race-org-b", "r3-race-owner-b").await;
    quant_account(
        &pool,
        "r3-race-account-a",
        "r3-race-org-a",
        "r3-race-owner-a",
    )
    .await;
    quant_account(
        &pool,
        "r3-race-account-b",
        "r3-race-org-b",
        "r3-race-owner-b",
    )
    .await;
    seed_catalog(&pool).await;
    let store = TradingStore::new(pool.clone());
    let token = [0x33_u8; 32];
    let first_tokens = lookup("test-key-v1", token);
    let second_tokens = lookup("test-key-v1", token);
    let (first, second) = tokio::join!(
        store.register_pending_broker_binding(
            "r3-race-org-a",
            "r3-race-owner-a",
            "r3-race-account-a",
            "example-broker",
            "server-demo",
            "external-race-a",
            "binding-race-a",
            1,
            "test-key-v1",
            &first_tokens,
            "race A",
        ),
        store.register_pending_broker_binding(
            "r3-race-org-b",
            "r3-race-owner-b",
            "r3-race-account-b",
            "example-broker",
            "server-demo",
            "external-race-b",
            "binding-race-b",
            1,
            "test-key-v1",
            &second_tokens,
            "race B",
        ),
    );
    assert_ne!(first.is_ok(), second.is_ok());
    let (accounts, bindings, events): (i64, i64, i64) = sqlx::query_as(
        "SELECT (SELECT count(*) FROM external_broker_accounts),(SELECT count(*) FROM trading_account_broker_bindings),(SELECT count(*) FROM trading_account_broker_binding_events)",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!((accounts, bindings, events), (1, 1, 1));
}

#[sqlx::test]
async fn concurrent_claims_for_one_quant_account_serialize_at_database_constraint(pool: PgPool) {
    migrate(&pool).await.unwrap();
    organization(&pool, "r3-quant-race-org", "r3-quant-race-owner").await;
    quant_account(
        &pool,
        "r3-quant-race-account",
        "r3-quant-race-org",
        "r3-quant-race-owner",
    )
    .await;
    seed_catalog(&pool).await;
    let store = TradingStore::new(pool.clone());
    let first_tokens = lookup("key-v1", [0xa1; 32]);
    let second_tokens = lookup("key-v1", [0xb2; 32]);
    let (first, second) = tokio::join!(
        store.register_pending_broker_binding(
            "r3-quant-race-org",
            "r3-quant-race-owner",
            "r3-quant-race-account",
            "example-broker",
            "server-demo",
            "quant-race-ext-a",
            "quant-race-binding-a",
            1,
            "key-v1",
            &first_tokens,
            "first quant claim",
        ),
        store.register_pending_broker_binding(
            "r3-quant-race-org",
            "r3-quant-race-owner",
            "r3-quant-race-account",
            "example-broker",
            "server-demo",
            "quant-race-ext-b",
            "quant-race-binding-b",
            1,
            "key-v1",
            &second_tokens,
            "second quant claim",
        ),
    );
    assert_ne!(first.is_ok(), second.is_ok());
    let (accounts, bindings): (i64, i64) = sqlx::query_as(
        "SELECT (SELECT count(*) FROM external_broker_accounts),(SELECT count(*) FROM trading_account_broker_bindings)",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!((accounts, bindings), (1, 1));
}

#[sqlx::test]
async fn concurrent_binding_lifecycle_updates_accept_one_revision(pool: PgPool) {
    migrate(&pool).await.unwrap();
    organization(&pool, "r3-state-race-org", "r3-state-race-owner").await;
    quant_account(
        &pool,
        "r3-state-race-account",
        "r3-state-race-org",
        "r3-state-race-owner",
    )
    .await;
    seed_catalog(&pool).await;
    let store = TradingStore::new(pool.clone());
    store
        .register_pending_broker_binding(
            "r3-state-race-org",
            "r3-state-race-owner",
            "r3-state-race-account",
            "example-broker",
            "server-demo",
            "state-race-external",
            "state-race-binding",
            1,
            "key-v1",
            &lookup("key-v1", [0xcc; 32]),
            "initial pending binding",
        )
        .await
        .unwrap();
    let (suspend, revoke) = tokio::join!(
        store.transition_broker_binding(
            "r3-state-race-org",
            "r3-state-race-owner",
            "state-race-binding",
            1,
            BrokerBindingTransition::Suspended,
            "suspend race",
        ),
        store.transition_broker_binding(
            "r3-state-race-org",
            "r3-state-race-owner",
            "state-race-binding",
            1,
            BrokerBindingTransition::Revoked,
            "revoke race",
        ),
    );
    assert_ne!(suspend.is_ok(), revoke.is_ok());
    let (revision, events): (i64, i64) = sqlx::query_as(
        "SELECT (SELECT revision FROM trading_account_broker_bindings WHERE id='state-race-binding'),(SELECT count(*) FROM trading_account_broker_binding_events WHERE binding_id='state-race-binding')",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!((revision, events), (2, 2));
}

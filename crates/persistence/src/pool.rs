use std::path::PathBuf;

use sqlx::postgres::{PgConnection, PgPoolOptions};
use sqlx::PgPool;
use std::borrow::Cow;
use std::collections::BTreeMap;

use crate::error::PersistenceResult;

// Exact, inspected legacy ledger found in the original Render EdgeNest database.
// This is deliberately not a general partial-history allowance: every applied
// version and checksum must match before the forward-only pending migrations run.
const KNOWN_RECOVERY_HISTORY: [i64; 14] = [
    // The three 12-digit legacy versions sort before the 14-digit versions
    // when SQLx stores them as integer migration IDs.
    202611070001,
    202611070002,
    202611120001,
    20260306000001,
    20260906000001,
    20261027000001,
    20261027000002,
    20261027000003,
    20261027000004,
    20261106000001,
    20261106000002,
    20261107000002,
    20261109000001,
    20261112000001,
];

fn is_known_recovery_history(versions: &[i64]) -> bool {
    versions == KNOWN_RECOVERY_HISTORY.as_slice()
}

/// Connect to PostgreSQL using `DATABASE_URL`.
pub async fn connect(database_url: &str) -> PersistenceResult<PgPool> {
    let pool = PgPoolOptions::new()
        .max_connections(10)
        .connect(database_url)
        .await?;
    Ok(pool)
}

fn migrations_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../migrations")
}

/// Run SQL migrations from the workspace `migrations/` directory.
pub async fn migrate(pool: &PgPool) -> PersistenceResult<()> {
    // Serialize history classification, bootstrap, and all migrations as one
    // transaction-scoped critical section. This also binds the lock lifetime to
    // the schema changes: commit or rollback releases it automatically.
    // Use a transaction-scoped advisory lock. PostgreSQL releases it reliably
    // on commit/rollback, even if migration execution returns early or the
    // connection is dropped. The transaction also pins all bootstrap reads and
    // migration DDL to the same physical session.
    let mut transaction = pool.begin().await?;
    sqlx::query("SELECT pg_advisory_xact_lock(20261111, 21)")
        .execute(&mut *transaction)
        .await?;
    match migrate_locked(&mut transaction).await {
        Ok(()) => {
            transaction.commit().await?;
            Ok(())
        }
        Err(error) => {
            // Roll back the transaction before returning the original
            // migration error. The xact advisory lock is released with it.
            let _ = transaction.rollback().await;
            Err(error)
        }
    }
}

async fn migrate_locked(connection: &mut PgConnection) -> PersistenceResult<()> {
    use sqlx::migrate::Migrator;

    let migrator = Migrator::new(migrations_path()).await?;
    let applied = read_migration_history(connection).await?;
    let current: BTreeMap<i64, Vec<u8>> = migrator
        .iter()
        .map(|migration| (migration.version, migration.checksum.to_vec()))
        .collect();
    let core_versions: [i64; 6] = [
        20260306000001,
        20260906000001,
        20261027000001,
        20261027000002,
        20261027000003,
        20261027000004,
    ];

    if applied.is_empty() {
        // A migration target with any pre-existing table but no recognized
        // SQLx history is not assumed to be fresh. Keep this fail-closed even
        // for unknown table names: the target may be misconfigured or contain
        // application data that must never be overwritten by bootstrap.
        // Include the database, schema, and table names to make diagnosis safe.
        let (database_name, schema_name, existing_tables): (String, String, Vec<String>) =
            sqlx::query_as(
                r#"
            SELECT
                current_database()::text,
                current_schema()::text,
                COALESCE(
                    array_agg(tablename ORDER BY tablename),
                    ARRAY[]::text[]
                )
            FROM pg_tables
            WHERE schemaname = current_schema()
              AND tablename <> '_sqlx_migrations'
            "#,
            )
            .fetch_one(&mut *connection)
            .await?;
        if !existing_tables.is_empty() {
            return Err(crate::error::PersistenceError::Other(format!(
                "refusing fresh-install migration bootstrap: database '{database_name}', schema '{schema_name}' contains table(s) without recognized SQLx history: {}. Verify MIGRATOR_DATABASE_URL and inspect the existing schema; do not delete tables or fabricate migration history.",
                existing_tables.join(", ")
            )));
        }
        let core = migrator
            .iter()
            .filter(|migration| core_versions.contains(&migration.version))
            .cloned()
            .collect::<Vec<_>>();
        if core.len() != core_versions.len() {
            return Err(crate::error::PersistenceError::Other(
                "core-first bootstrap migration set is incomplete".into(),
            ));
        }
        // Run one ordered migrator rather than invoking SQLx twice against
        // the same connection. SQLx snapshots the applied-version ledger at
        // the start of each run; splitting bootstrap into two runs can leave
        // the second run with a stale view and attempt duplicate inserts.
        let ordered = core
            .into_iter()
            .chain(
                migrator
                    .iter()
                    .filter(|migration| !core_versions.contains(&migration.version))
                    .cloned(),
            )
            .collect::<Vec<_>>();
        let ordered_migrator = Migrator {
            migrations: Cow::Owned(ordered),
            locking: false,
            ..Migrator::DEFAULT
        };
        ordered_migrator.run(&mut *connection).await?;
        harden_edgenest_runtime_audit_privileges(connection).await?;
        tracing::info!("fresh database migrations applied");
        return Ok(());
    } else {
        let applied_versions = applied.keys().copied().collect::<Vec<_>>();
        let core_history = core_versions.to_vec();
        let historical_complete = current
            .keys()
            .filter(|version| **version < 20261111000001)
            .copied()
            .collect::<Vec<_>>();
        let e2_complete = current
            .keys()
            .filter(|version| **version < 20261112000001)
            .copied()
            .collect::<Vec<_>>();
        let r2_complete = current
            .keys()
            .filter(|version| **version < 20261113000001)
            .copied()
            .collect::<Vec<_>>();
        let r3_complete = current
            .keys()
            .filter(|version| **version < 20261114000001)
            .copied()
            .collect::<Vec<_>>();
        let i1_complete = current
            .keys()
            .filter(|version| **version < 20261115000001)
            .copied()
            .collect::<Vec<_>>();
        let current_complete = current.keys().copied().collect::<Vec<_>>();
        let recovery_history = is_known_recovery_history(&applied_versions);
        let recognized = applied_versions == core_history
            || applied_versions == historical_complete
            || applied_versions == e2_complete
            || applied_versions == r2_complete
            || applied_versions == r3_complete
            || applied_versions == i1_complete
            || applied_versions == current_complete
            || recovery_history;
        if !recognized {
            return Err(crate::error::PersistenceError::Other(format!(
                "refusing migration on partial or unrecognized SQLx history: versions={applied_versions:?}"
            )));
        }
        for (version, checksum) in &applied {
            if current.get(version) != Some(checksum) {
                return Err(crate::error::PersistenceError::Other(format!(
                    "refusing migration: SQLx checksum mismatch for version {version}"
                )));
            }
        }
        if recovery_history {
            validate_recovery_schema(connection).await?;
        } else if applied_versions == core_history {
            validate_recognized_schema(connection, false, false, false, false, false).await?;
        } else {
            let has_e2 = applied_versions == e2_complete
                || applied_versions == r2_complete
                || applied_versions == current_complete;
            let has_r3 = applied_versions == r3_complete || applied_versions == current_complete;
            let has_i1 = applied_versions == i1_complete || applied_versions == current_complete;
            let has_i1_hardening = applied_versions == current_complete;
            validate_recognized_schema(connection, true, has_e2, has_r3, has_i1, has_i1_hardening)
                .await?;
        }
    }

    if !applied.is_empty() {
        let ordered = migrator
            .iter()
            .filter(|migration| core_versions.contains(&migration.version))
            .cloned()
            .chain(
                migrator
                    .iter()
                    .filter(|migration| !core_versions.contains(&migration.version))
                    .cloned(),
            )
            .collect::<Vec<_>>();
        let ordered_migrator = Migrator {
            migrations: Cow::Owned(ordered),
            locking: false,
            ..Migrator::DEFAULT
        };
        ordered_migrator.run(&mut *connection).await?;
        harden_edgenest_runtime_audit_privileges(connection).await?;
    }
    tracing::info!("database migrations applied");
    Ok(())
}

/// Keep the dedicated Render runtime principal from mutating the append-only
/// identity session audit stream. Other application tables retain their normal
/// CRUD grants; this narrow restriction is required by runtime-role validation.
async fn harden_edgenest_runtime_audit_privileges(
    connection: &mut PgConnection,
) -> PersistenceResult<()> {
    let runtime_role_exists: bool = sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM pg_roles WHERE rolname='edgenest_runtime')",
    )
    .fetch_one(&mut *connection)
    .await?;
    if !runtime_role_exists {
        return Ok(());
    }

    let migration_table_exists: bool =
        sqlx::query_scalar("SELECT to_regclass('public._sqlx_migrations') IS NOT NULL")
            .fetch_one(&mut *connection)
            .await?;
    if migration_table_exists {
        sqlx::query(
            "REVOKE INSERT, UPDATE, DELETE, TRUNCATE, REFERENCES, TRIGGER ON TABLE public._sqlx_migrations FROM edgenest_runtime",
        )
        .execute(&mut *connection)
        .await?;
    }

    let audit_table_exists: bool =
        sqlx::query_scalar("SELECT to_regclass('public.identity_session_events') IS NOT NULL")
            .fetch_one(&mut *connection)
            .await?;
    if audit_table_exists {
        sqlx::query(
            "REVOKE UPDATE, DELETE, TRUNCATE, REFERENCES, TRIGGER ON TABLE public.identity_session_events FROM edgenest_runtime",
        )
        .execute(&mut *connection)
        .await?;
        sqlx::query(
            "REVOKE UPDATE ON SEQUENCE public.identity_session_events_id_seq FROM edgenest_runtime",
        )
        .execute(&mut *connection)
        .await?;
    }
    Ok(())
}

/// Validate the one known, inspected Render EdgeNest ledger that was left
/// between migration releases. This gate checks the exact predecessor schema
/// and the data preconditions of the missing migrations before SQLx applies
/// them. It does not mark migrations as applied or bypass checksum validation.
async fn validate_recovery_schema(connection: &mut PgConnection) -> PersistenceResult<()> {
    let safe: bool = sqlx::query_scalar(
        r#"SELECT
            to_regclass('public.accounts') IS NOT NULL
            AND to_regclass('public.venues') IS NOT NULL
            AND to_regclass('public.account_states') IS NOT NULL
            AND to_regclass('public.orders') IS NOT NULL
            AND to_regclass('public.risk_decisions') IS NOT NULL
            AND to_regclass('public.trade_intents') IS NOT NULL
            AND to_regclass('public.journal_accounts') IS NOT NULL
            AND to_regclass('public.identity_users') IS NOT NULL
            AND to_regclass('public.dashboard_account_access') IS NOT NULL
            AND to_regclass('public.journal_analytics_saved_views') IS NOT NULL
            AND EXISTS (
                SELECT 1 FROM information_schema.columns
                 WHERE table_schema='public' AND table_name='journal_accounts'
                   AND column_name='linked_trading_account_id'
                   AND data_type='text' AND is_nullable='YES'
            )
            AND EXISTS (
                SELECT 1 FROM information_schema.columns
                 WHERE table_schema='public' AND table_name='accounts'
                   AND column_name='environment'
            )
            AND NOT EXISTS (
                SELECT 1 FROM accounts
                 WHERE environment IS NOT NULL
                   AND environment NOT IN ('PAPER','SHADOW','LIVE')
            )
            AND NOT EXISTS (
                SELECT 1 FROM journal_accounts j
                 LEFT JOIN accounts a ON a.id=j.linked_trading_account_id
                 WHERE j.linked_trading_account_id IS NOT NULL AND a.id IS NULL
            )
            AND EXISTS (
                SELECT 1 FROM pg_constraint c
                 WHERE c.conrelid='public.dashboard_account_access'::regclass
                   AND c.contype IN ('p','u')
                   AND c.conkey = ARRAY[
                       (SELECT attnum FROM pg_attribute WHERE attrelid='public.dashboard_account_access'::regclass AND attname='owner_id'),
                       (SELECT attnum FROM pg_attribute WHERE attrelid='public.dashboard_account_access'::regclass AND attname='account_id')
                   ]::smallint[]
            )
            AND NOT EXISTS (
                SELECT 1 FROM pg_constraint c
                 WHERE c.conrelid='public.journal_accounts'::regclass
                   AND c.contype='f'
                   AND (c.conname='fk_journal_accounts_trading_account'
                        OR c.conkey @> ARRAY[(
                            SELECT attnum FROM pg_attribute
                             WHERE attrelid='public.journal_accounts'::regclass
                               AND attname='linked_trading_account_id'
                        )]::smallint[])
                   AND NOT (
                        c.conkey = ARRAY[(
                            SELECT attnum FROM pg_attribute
                             WHERE attrelid='public.journal_accounts'::regclass
                               AND attname='linked_trading_account_id'
                        )]::smallint[]
                        AND c.confrelid='public.accounts'::regclass
                        AND c.confkey = ARRAY[(
                            SELECT attnum FROM pg_attribute
                             WHERE attrelid='public.accounts'::regclass
                               AND attname='id'
                        )]::smallint[]
                        AND c.confupdtype='a' AND c.confdeltype='a'
                        AND c.confmatchtype='s'
                   )
            )
            AND (
                SELECT count(*) FROM pg_constraint c
                 WHERE c.conrelid='public.journal_accounts'::regclass
                   AND c.contype='f'
                   AND c.conkey @> ARRAY[(
                       SELECT attnum FROM pg_attribute
                        WHERE attrelid='public.journal_accounts'::regclass
                          AND attname='linked_trading_account_id'
                   )]::smallint[]
            ) <= 1
            AND NOT EXISTS (
                SELECT 1 FROM pg_constraint
                 WHERE conrelid='public.orders'::regclass
                   AND conname='orders_id_account_client_identity'
            )
            AND NOT EXISTS (
                SELECT 1 FROM pg_constraint
                 WHERE conrelid='public.risk_decisions'::regclass
                   AND conname='risk_decisions_id_account_identity'
            )
            AND to_regclass('public.execution_authorizations') IS NULL
            AND to_regclass('public.execution_claims') IS NULL
            AND to_regclass('public.organizations') IS NULL
            AND to_regclass('public.organization_events') IS NULL
            AND to_regclass('public.organization_memberships') IS NULL
            AND to_regclass('public.organization_membership_events') IS NULL
            AND to_regclass('public.organization_account_entitlements') IS NULL
            AND to_regclass('public.organization_account_entitlement_events') IS NULL
            AND to_regclass('public.brokers') IS NULL
            AND to_regclass('public.broker_servers') IS NULL
            AND NOT EXISTS (
                SELECT 1 FROM information_schema.columns
                 WHERE table_schema='public' AND table_name='accounts'
                   AND column_name IN ('environment_verification','environment_revision',
                       'environment_verified_at','environment_verified_by',
                       'environment_verification_reason','account_revision')
            )
            AND NOT EXISTS (
                SELECT 1 FROM pg_proc
                 WHERE proname IN ('require_explicit_account_environment',
                     'protect_account_environment_revision','increment_account_revision',
                     'protect_last_organization_owner','prevent_tenant_history_change')
            )
            AND NOT EXISTS (
                SELECT 1 FROM pg_trigger
                 WHERE tgname IN ('accounts_require_explicit_environment',
                     'accounts_environment_revision_monotonic','accounts_revision_monotonic',
                     'organization_memberships_last_owner','organization_events_no_update_delete')
            )
            AND NOT EXISTS (
                SELECT 1 FROM pg_constraint
                 WHERE conname IN ('accounts_environment_valid',
                     'accounts_environment_verification_valid',
                     'accounts_environment_revision_valid',
                     'accounts_environment_provenance_consistent',
                     'accounts_environment_verifier_fk')
            )"#,
    )
    .fetch_one(&mut *connection)
    .await?;
    if !safe {
        return Err(crate::error::PersistenceError::Other(
            "known recovery migration history does not match the inspected predecessor schema or migration data preconditions; no migrations were applied".into(),
        ));
    }
    Ok(())
}

async fn validate_recognized_schema(
    connection: &mut PgConnection,
    journal_and_identity: bool,
    e2_schema: bool,
    r3_schema: bool,
    i1_schema: bool,
    i1_hardening_schema: bool,
) -> PersistenceResult<()> {
    let core_schema_valid: bool = sqlx::query_scalar(
        "SELECT to_regclass('public.accounts') IS NOT NULL AND to_regclass('public.venues') IS NOT NULL AND to_regclass('public.account_states') IS NOT NULL AND to_regclass('public.orders') IS NOT NULL AND to_regclass('public.risk_decisions') IS NOT NULL AND to_regclass('public.trade_intents') IS NOT NULL",
    )
    .fetch_one(&mut *connection)
    .await?;
    if !core_schema_valid {
        return Err(crate::error::PersistenceError::Other(
            "recognized migration history is missing required core trading schema objects".into(),
        ));
    }
    if journal_and_identity {
        let historical_schema_valid: bool = sqlx::query_scalar(
            "SELECT to_regclass('public.journal_accounts') IS NOT NULL AND to_regclass('public.identity_users') IS NOT NULL AND to_regclass('public.dashboard_account_access') IS NOT NULL AND EXISTS (SELECT 1 FROM information_schema.columns WHERE table_schema='public' AND table_name='accounts' AND column_name='environment') AND EXISTS (SELECT 1 FROM information_schema.columns WHERE table_schema='public' AND table_name='journal_accounts' AND column_name='linked_trading_account_id' AND data_type='text')",
        )
        .fetch_one(&mut *connection)
        .await?;
        if !historical_schema_valid {
            return Err(crate::error::PersistenceError::Other(
                "recognized historical migration set is missing Journal, identity, access, or environment schema objects".into(),
            ));
        }
    }
    if e2_schema {
        let e2_schema_valid: bool = sqlx::query_scalar(
            "SELECT EXISTS (SELECT 1 FROM information_schema.columns WHERE table_schema='public' AND table_name='accounts' AND column_name='environment_verification') AND EXISTS (SELECT 1 FROM information_schema.columns WHERE table_schema='public' AND table_name='accounts' AND column_name='environment_revision') AND to_regclass('public.execution_authorizations') IS NOT NULL AND to_regclass('public.execution_claims') IS NOT NULL AND EXISTS (SELECT 1 FROM pg_constraint WHERE conrelid='public.journal_accounts'::regclass AND conname='fk_journal_accounts_trading_account' AND contype='f' AND convalidated)",
        )
        .fetch_one(&mut *connection)
        .await?;
        if !e2_schema_valid {
            return Err(crate::error::PersistenceError::Other(
                "complete E2 migration history is missing required environment, authorization, claim, or validated Journal FK schema".into(),
            ));
        }
    }
    if e2_schema {
        let tenant_schema_valid: bool = sqlx::query_scalar(
            "SELECT to_regclass('public.organizations') IS NOT NULL AND to_regclass('public.organization_events') IS NOT NULL AND to_regclass('public.organization_memberships') IS NOT NULL AND to_regclass('public.organization_membership_events') IS NOT NULL AND to_regclass('public.organization_account_entitlements') IS NOT NULL AND to_regclass('public.organization_account_entitlement_events') IS NOT NULL AND EXISTS (SELECT 1 FROM information_schema.columns WHERE table_schema='public' AND table_name='accounts' AND column_name='account_revision')",
        )
        .fetch_one(&mut *connection)
        .await?;
        // E2-complete databases are expected to be missing R2 objects; validate
        // those objects only after the R2 migration is present in SQLx history.
        let applied_r2: bool = sqlx::query_scalar(
            "SELECT EXISTS (SELECT 1 FROM _sqlx_migrations WHERE version=20261112000002 AND success)",
        )
        .fetch_one(&mut *connection)
        .await?;
        if applied_r2 && !tenant_schema_valid {
            return Err(crate::error::PersistenceError::Other(
                "complete R2 migration history is missing organization tenancy schema objects"
                    .into(),
            ));
        }
    }
    if r3_schema {
        let registry_schema_valid: bool = sqlx::query_scalar(
            "SELECT to_regclass('public.brokers') IS NOT NULL AND to_regclass('public.broker_servers') IS NOT NULL AND to_regclass('public.external_broker_accounts') IS NOT NULL AND to_regclass('public.external_broker_account_identifier_tokens') IS NOT NULL AND to_regclass('public.trading_account_broker_bindings') IS NOT NULL AND to_regclass('public.broker_account_evidence') IS NOT NULL AND to_regclass('public.broker_account_verifications') IS NOT NULL AND to_regclass('public.trading_account_broker_binding_events') IS NOT NULL",
        )
        .fetch_one(&mut *connection)
        .await?;
        if !registry_schema_valid {
            return Err(crate::error::PersistenceError::Other(
                "complete R3 migration history is missing broker registry schema objects".into(),
            ));
        }
    }
    if i1_schema {
        let i1_schema_valid: bool = sqlx::query_scalar(
            "SELECT to_regclass('public.identity_sessions') IS NOT NULL AND to_regclass('public.identity_access_tokens') IS NOT NULL AND to_regclass('public.identity_session_events') IS NOT NULL AND EXISTS (SELECT 1 FROM information_schema.columns WHERE table_schema='public' AND table_name='identity_refresh_tokens' AND column_name='session_id')",
        )
        .fetch_one(&mut *connection)
        .await?;
        if !i1_schema_valid {
            return Err(crate::error::PersistenceError::Other(
                "complete I1 migration history is missing server-managed identity session schema objects".into(),
            ));
        }
    }
    if i1_hardening_schema {
        let hardened_schema_valid: bool = sqlx::query_scalar(
            "SELECT EXISTS (SELECT 1 FROM pg_constraint WHERE conrelid='public.identity_session_events'::regclass AND conname='identity_session_events_session_user_fk' AND contype='f' AND confdeltype='r') AND EXISTS (SELECT 1 FROM pg_trigger WHERE tgrelid='public.identity_session_events'::regclass AND tgname='identity_session_events_no_truncate' AND NOT tgisinternal) AND EXISTS (SELECT 1 FROM pg_trigger WHERE tgrelid='public.identity_users'::regclass AND tgname='identity_user_suspension_revokes_sessions' AND NOT tgisinternal)",
        )
        .fetch_one(&mut *connection)
        .await?;
        if !hardened_schema_valid {
            return Err(crate::error::PersistenceError::Other(
                "complete I1-R1 migration history is missing audit immutability, attribution, or suspension invalidation schema".into(),
            ));
        }
    }
    Ok(())
}

async fn read_migration_history(
    connection: &mut PgConnection,
) -> PersistenceResult<BTreeMap<i64, Vec<u8>>> {
    let exists: Option<String> =
        sqlx::query_scalar("SELECT to_regclass('public._sqlx_migrations')::text")
            .fetch_one(&mut *connection)
            .await?;
    let Some(_) = exists else {
        return Ok(BTreeMap::new());
    };
    let rows = sqlx::query_as::<_, (i64, Vec<u8>, bool)>(
        "SELECT version, checksum, success FROM _sqlx_migrations ORDER BY version",
    )
    .fetch_all(&mut *connection)
    .await?;
    let mut result = BTreeMap::new();
    for (version, checksum, success) in rows {
        if !success {
            return Err(crate::error::PersistenceError::Other(format!(
                "refusing migration with dirty/failed SQLx migration version {version}"
            )));
        }
        result.insert(version, checksum);
    }
    Ok(result)
}

/// Verify that the pool can execute a round-trip query against PostgreSQL.
pub async fn check_database_health(pool: &PgPool) -> PersistenceResult<()> {
    sqlx::query_scalar::<_, i32>("SELECT 1")
        .fetch_one(pool)
        .await?;
    Ok(())
}

/// Fail closed unless this pool uses a non-owner runtime role that can read
/// and append identity session audit events but cannot mutate their history or
/// create schema objects. A distinct migration principal must apply schema
/// changes before service startup.
pub async fn validate_identity_runtime_role(pool: &PgPool) -> PersistenceResult<()> {
    use sqlx::migrate::Migrator;

    let migrator = Migrator::new(migrations_path()).await?;
    let expected_checksum = migrator
        .iter()
        .find(|migration| migration.version == 20261115000001)
        .map(|migration| migration.checksum.to_vec())
        .ok_or_else(|| {
            crate::error::PersistenceError::Other(
                "I1-R1 migration is missing from this service build".into(),
            )
        })?;
    let applied = sqlx::query_as::<_, (Vec<u8>, bool)>(
        "SELECT checksum,success FROM public._sqlx_migrations WHERE version=20261115000001",
    )
    .fetch_optional(pool)
    .await?;
    if !matches!(applied, Some((checksum, true)) if checksum == expected_checksum) {
        return Err(crate::error::PersistenceError::Other(
            "runtime database is not at the exact successful I1-R1 migration checksum".into(),
        ));
    }
    let schema_safe: bool = sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM pg_constraint WHERE conrelid='public.identity_session_events'::regclass AND conname='identity_session_events_session_user_fk' AND contype='f' AND confdeltype='r') AND EXISTS (SELECT 1 FROM pg_trigger WHERE tgrelid='public.identity_session_events'::regclass AND tgname='identity_session_events_no_update_delete' AND NOT tgisinternal) AND EXISTS (SELECT 1 FROM pg_trigger WHERE tgrelid='public.identity_session_events'::regclass AND tgname='identity_session_events_no_truncate' AND NOT tgisinternal) AND EXISTS (SELECT 1 FROM pg_trigger WHERE tgrelid='public.identity_users'::regclass AND tgname='identity_user_suspension_revokes_sessions' AND NOT tgisinternal)",
    )
    .fetch_one(pool)
    .await?;
    if !schema_safe {
        return Err(crate::error::PersistenceError::Other(
            "runtime database is missing I1-R1 session invalidation or audit hardening schema"
                .into(),
        ));
    }
    let safe: bool = sqlx::query_scalar(
        r#"SELECT NOT r.rolsuper
                  AND NOT r.rolcreaterole
                  AND NOT r.rolcreatedb
                  AND c.relowner <> r.oid
                  AND NOT pg_has_role(r.oid,c.relowner,'MEMBER')
                  AND has_table_privilege(r.oid,c.oid,'SELECT')
                  AND has_table_privilege(r.oid,c.oid,'INSERT')
                  AND NOT has_table_privilege(r.oid,c.oid,'UPDATE')
                  AND NOT has_table_privilege(r.oid,c.oid,'DELETE')
                  AND NOT has_table_privilege(r.oid,c.oid,'TRUNCATE')
                  AND NOT has_table_privilege(r.oid,c.oid,'TRIGGER')
                  AND NOT has_schema_privilege(r.oid,c.relnamespace,'CREATE')
                  AND EXISTS (
                      SELECT 1 FROM pg_class seq
                       WHERE seq.oid=to_regclass('public.identity_session_events_id_seq')
                         AND has_sequence_privilege(r.oid,seq.oid,'USAGE')
                         AND NOT has_sequence_privilege(r.oid,seq.oid,'UPDATE')
                  )
             FROM pg_roles r
             JOIN pg_class c ON c.oid=to_regclass('public.identity_session_events')
            WHERE r.rolname=current_user"#,
    )
    .fetch_optional(pool)
    .await?
    .unwrap_or(false);
    if !safe {
        return Err(crate::error::PersistenceError::Other(
            "database runtime role must be distinct from the identity migration/table owner and restricted to session-event SELECT/INSERT".into(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod migration_history_tests {
    use super::{is_known_recovery_history, KNOWN_RECOVERY_HISTORY};

    #[test]
    fn accepts_only_the_exact_inspected_recovery_ledger() {
        assert!(is_known_recovery_history(&KNOWN_RECOVERY_HISTORY));
    }

    #[test]
    fn rejects_a_ledger_with_a_missing_version() {
        let mut versions = KNOWN_RECOVERY_HISTORY.to_vec();
        versions.remove(10);
        assert!(!is_known_recovery_history(&versions));
    }

    #[test]
    fn rejects_a_ledger_with_an_unrecognized_extra_version() {
        let mut versions = KNOWN_RECOVERY_HISTORY.to_vec();
        versions.push(20261121000001);
        assert!(!is_known_recovery_history(&versions));
    }
}

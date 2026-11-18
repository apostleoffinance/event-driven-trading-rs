use std::path::PathBuf;

use sqlx::postgres::{PgConnection, PgPoolOptions};
use sqlx::PgPool;
use std::borrow::Cow;
use std::collections::BTreeMap;

use crate::error::PersistenceResult;

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
        let app_table_count: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM pg_tables WHERE schemaname=current_schema() AND tablename <> '_sqlx_migrations'",
        )
        .fetch_one(&mut *connection)
        .await?;
        if app_table_count != 0 {
            return Err(crate::error::PersistenceError::Other(
                "refusing fresh-install migration bootstrap: application schema exists without recognized SQLx history".into(),
            ));
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
        let core_migrator = Migrator {
            migrations: Cow::Owned(core),
            ..Migrator::DEFAULT
        };
        core_migrator.run(&mut *connection).await?;
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
        let recognized = applied_versions == core_history
            || applied_versions == historical_complete
            || applied_versions == e2_complete
            || applied_versions == r2_complete
            || applied_versions == r3_complete
            || applied_versions == i1_complete
            || applied_versions == current_complete;
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
        if applied_versions == core_history {
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

    migrator.run(&mut *connection).await?;
    tracing::info!("database migrations applied");
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
            "SELECT EXISTS (SELECT 1 FROM _sqlx_migrations WHERE version=20261112000001 AND success)",
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

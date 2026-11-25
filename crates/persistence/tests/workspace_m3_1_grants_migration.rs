#![allow(clippy::unwrap_used)]

use persistence::migrate;
use sqlx::PgPool;

async fn seed(pool: &PgPool) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO identity_users(id,email,display_name,password_hash) VALUES \
         ('m31-owner','m31-owner@example.test','M3.1 Owner','test-only'),\
         ('m31-trader','m31-trader@example.test','M3.1 Trader','test-only'),\
         ('m31-viewer','m31-viewer@example.test','M3.1 Viewer','test-only')",
    )
    .execute(pool)
    .await?;
    sqlx::query(
        "INSERT INTO organizations(id,name,status,revision,created_by) VALUES ('m31-org','M3.1 Org','ACTIVE',1,'m31-owner')",
    )
    .execute(pool)
    .await?;
    sqlx::query(
        "INSERT INTO organization_memberships(organization_id,user_id,role,status,created_by,updated_by) VALUES \
         ('m31-org','m31-owner','OWNER','ACTIVE','m31-owner','m31-owner'),\
         ('m31-org','m31-trader','TRADER','ACTIVE','m31-owner','m31-owner'),\
         ('m31-org','m31-viewer','VIEWER','ACTIVE','m31-owner','m31-owner')",
    )
    .execute(pool)
    .await?;
    Ok(())
}

#[sqlx::test]
async fn grants_are_revisioned_audited_and_membership_revocation_is_immediate(
    pool: PgPool,
) -> Result<(), Box<dyn std::error::Error>> {
    migrate(&pool).await?;
    seed(&pool).await?;
    let workspace: String = sqlx::query_scalar(
        "SELECT id FROM workspaces WHERE organization_id='m31-org' AND is_default",
    )
    .fetch_one(&pool)
    .await?;

    sqlx::query(
        "INSERT INTO workspace_access_grants(organization_id,workspace_id,user_id,role,status,revision,granted_by,updated_by,reason) \
         VALUES('m31-org',$1,'m31-trader','TRADER','ACTIVE',1,'m31-owner','m31-owner','approved for trading')",
    )
    .bind(&workspace)
    .execute(&pool)
    .await?;

    let (role,status,revision):(String,String,i64)=sqlx::query_as(
        "SELECT role,status,revision FROM workspace_access_grants WHERE workspace_id=$1 AND user_id='m31-trader'",
    ).bind(&workspace).fetch_one(&pool).await?;
    assert_eq!(
        (role.as_str(), status.as_str(), revision),
        ("TRADER", "ACTIVE", 1)
    );

    sqlx::query(
        "UPDATE workspace_access_grants SET role='VIEWER',revision=2,updated_by='m31-owner',reason='reduced privileges' \
         WHERE workspace_id=$1 AND user_id='m31-trader'",
    )
    .bind(&workspace)
    .execute(&pool)
    .await?;

    sqlx::query(
        "UPDATE organization_memberships SET status='SUSPENDED',revision=revision+1,updated_by='m31-owner' \
         WHERE organization_id='m31-org' AND user_id='m31-trader'",
    )
    .execute(&pool)
    .await?;

    let (status,revision):(String,i64)=sqlx::query_as(
        "SELECT status,revision FROM workspace_access_grants WHERE workspace_id=$1 AND user_id='m31-trader'",
    ).bind(&workspace).fetch_one(&pool).await?;
    assert_eq!((status.as_str(), revision), ("REVOKED", 3));

    let (events,revoked_events):(i64,i64)=sqlx::query_as(
        "SELECT count(*),count(*) FILTER (WHERE event_type='REVOKED') FROM workspace_access_grant_events WHERE workspace_id=$1 AND user_id='m31-trader'",
    ).bind(&workspace).fetch_one(&pool).await?;
    assert_eq!((events, revoked_events), (3, 1));

    let history_mutation=sqlx::query(
        "UPDATE workspace_access_grant_events SET reason='tampered' WHERE workspace_id=$1 AND user_id='m31-trader'",
    ).bind(&workspace).execute(&pool).await;
    assert!(history_mutation.is_err());

    let grant_truncate = sqlx::query("TRUNCATE TABLE workspace_access_grants")
        .execute(&pool)
        .await;
    assert!(grant_truncate.is_err());
    let history_truncate = sqlx::query("TRUNCATE TABLE workspace_access_grant_events")
        .execute(&pool)
        .await;
    assert!(history_truncate.is_err());
    Ok(())
}

#[sqlx::test]
async fn grants_reject_admin_targets_and_non_admin_grantors_and_do_not_reactivate_after_promotion(
    pool: PgPool,
) -> Result<(), Box<dyn std::error::Error>> {
    migrate(&pool).await?;
    seed(&pool).await?;
    let workspace: String = sqlx::query_scalar(
        "SELECT id FROM workspaces WHERE organization_id='m31-org' AND is_default",
    )
    .fetch_one(&pool)
    .await?;

    let admin_target=sqlx::query(
        "INSERT INTO workspace_access_grants(organization_id,workspace_id,user_id,role,granted_by,updated_by,reason) \
         VALUES('m31-org',$1,'m31-owner','VIEWER','m31-owner','m31-owner','should not be needed')",
    ).bind(&workspace).execute(&pool).await;
    assert!(admin_target.is_err());

    let non_admin_actor=sqlx::query(
        "INSERT INTO workspace_access_grants(organization_id,workspace_id,user_id,role,granted_by,updated_by,reason) \
         VALUES('m31-org',$1,'m31-viewer','VIEWER','m31-owner','m31-trader','unauthorized actor')",
    ).bind(&workspace).execute(&pool).await;
    assert!(non_admin_actor.is_err());

    sqlx::query(
        "INSERT INTO workspace_access_grants(organization_id,workspace_id,user_id,role,granted_by,updated_by,reason) \
         VALUES('m31-org',$1,'m31-viewer','VIEWER','m31-owner','m31-owner','initial grant')",
    ).bind(&workspace).execute(&pool).await?;

    sqlx::query(
        "UPDATE organization_memberships SET role='ADMIN',revision=revision+1,updated_by='m31-owner' \
         WHERE organization_id='m31-org' AND user_id='m31-viewer'",
    ).execute(&pool).await?;

    let status: String = sqlx::query_scalar(
        "SELECT status FROM workspace_access_grants WHERE workspace_id=$1 AND user_id='m31-viewer'",
    )
    .bind(&workspace)
    .fetch_one(&pool)
    .await?;
    assert_eq!(status, "REVOKED");

    sqlx::query(
        "UPDATE organization_memberships SET role='VIEWER',revision=revision+1,updated_by='m31-owner' \
         WHERE organization_id='m31-org' AND user_id='m31-viewer'",
    ).execute(&pool).await?;

    let status: String = sqlx::query_scalar(
        "SELECT status FROM workspace_access_grants WHERE workspace_id=$1 AND user_id='m31-viewer'",
    )
    .bind(&workspace)
    .fetch_one(&pool)
    .await?;
    assert_eq!(
        status, "REVOKED",
        "downgrading an implicit admin must not resurrect a historical grant"
    );
    Ok(())
}

async fn migrate_pre_m3_1(pool: &PgPool) {
    use sqlx::migrate::Migrator;
    use std::{borrow::Cow, path::PathBuf};

    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../migrations");
    let all = Migrator::new(path).await.unwrap();
    let selected = all
        .iter()
        .filter(|migration| migration.version < 20261125000002)
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

#[sqlx::test]
async fn existing_m1_m2_database_is_recognized_and_upgraded_to_m3_1(
    pool: PgPool,
) -> Result<(), Box<dyn std::error::Error>> {
    migrate_pre_m3_1(&pool).await;
    seed(&pool).await?;

    // This exercises the real startup migration-history gate against a
    // database whose last successful migration is M1/M2, not a fresh install.
    migrate(&pool).await?;

    let (grant_table,event_table):(bool,bool)=sqlx::query_as(
        "SELECT to_regclass('public.workspace_access_grants') IS NOT NULL, to_regclass('public.workspace_access_grant_events') IS NOT NULL",
    ).fetch_one(&pool).await?;
    assert!(grant_table && event_table);

    let applied: bool = sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM _sqlx_migrations WHERE version=20261125000002 AND success)",
    )
    .fetch_one(&pool)
    .await?;
    assert!(applied);
    Ok(())
}

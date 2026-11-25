#![allow(clippy::unwrap_used)]

use std::{borrow::Cow, path::PathBuf};

use persistence::migrate;
use sqlx::{migrate::Migrator, PgPool};

fn migration_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../migrations")
}

async fn migrate_pre_workspace_history(pool: &PgPool) {
    let all = Migrator::new(migration_path()).await.unwrap();
    let selected = all
        .iter()
        .filter(|migration| migration.version < 20261125000001)
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
async fn m1_backfills_one_default_workspace_for_each_existing_organization(
    pool: PgPool,
) -> Result<(), Box<dyn std::error::Error>> {
    migrate_pre_workspace_history(&pool).await;
    sqlx::query(
        "INSERT INTO identity_users(id,email,display_name,password_hash) VALUES ('m1-owner','m1-owner@example.test','M1 Owner','test-only')",
    )
    .execute(&pool)
    .await?;
    sqlx::query(
        "INSERT INTO organizations(id,name,status,revision,created_by) VALUES ('m1-org-a','Alpha','ACTIVE',1,'m1-owner'),('m1-org-b','Beta','ACTIVE',1,'m1-owner')",
    )
    .execute(&pool)
    .await?;
    sqlx::query(
        "INSERT INTO organization_memberships(organization_id,user_id,role,status,created_by,updated_by) VALUES ('m1-org-a','m1-owner','OWNER','ACTIVE','m1-owner','m1-owner'),('m1-org-b','m1-owner','OWNER','ACTIVE','m1-owner','m1-owner')",
    )
    .execute(&pool)
    .await?;

    migrate(&pool).await?;

    let rows: Vec<(String, String, bool, String)> = sqlx::query_as(
        "SELECT organization_id,name,is_default,slug FROM workspaces ORDER BY organization_id",
    )
    .fetch_all(&pool)
    .await?;
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0], ("m1-org-a".into(), "Default".into(), true, "default".into()));
    assert_eq!(rows[1], ("m1-org-b".into(), "Default".into(), true, "default".into()));

    let events: i64 = sqlx::query_scalar("SELECT count(*) FROM workspace_events")
        .fetch_one(&pool)
        .await?;
    assert_eq!(events, 2);

    // A second migration run must be a no-op and retain the same workspace IDs.
    let before: Vec<String> =
        sqlx::query_scalar("SELECT id FROM workspaces ORDER BY organization_id")
            .fetch_all(&pool)
            .await?;
    migrate(&pool).await?;
    let after: Vec<String> =
        sqlx::query_scalar("SELECT id FROM workspaces ORDER BY organization_id")
            .fetch_all(&pool)
            .await?;
    assert_eq!(before, after);
    Ok(())
}

#[sqlx::test]
async fn m1_organization_creation_atomically_provisions_a_default_workspace(
    pool: PgPool,
) -> Result<(), Box<dyn std::error::Error>> {
    migrate(&pool).await?;
    sqlx::query(
        "INSERT INTO identity_users(id,email,display_name,password_hash) VALUES ('m1-no-workspace','m1-no-workspace@example.test','No Workspace','test-only')",
    )
    .execute(&pool)
    .await?;

    sqlx::query(
        "INSERT INTO organizations(id,name,status,revision,created_by) VALUES ('m1-new-org','New Organization','ACTIVE',1,'m1-no-workspace')",
    )
    .execute(&pool)
    .await?;

    let (workspace_count, default_count, event_count): (i64, i64, i64) = sqlx::query_as(
        "SELECT count(*),count(*) FILTER (WHERE is_default AND status='ACTIVE'),(SELECT count(*) FROM workspace_events WHERE organization_id='m1-new-org') FROM workspaces WHERE organization_id='m1-new-org'",
    )
    .fetch_one(&pool)
    .await?;
    assert_eq!((workspace_count, default_count, event_count), (1, 1, 1));
    Ok(())
}

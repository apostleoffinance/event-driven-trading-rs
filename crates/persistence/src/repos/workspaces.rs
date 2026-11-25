use chrono::{DateTime, Utc};
use domain::{OrganizationId, Workspace, WorkspaceId, WorkspaceStatus};
use sqlx::{PgPool, Postgres, Transaction};

use crate::error::{PersistenceError, PersistenceResult};

type WorkspaceRow = (
    String,
    String,
    String,
    String,
    String,
    bool,
    i64,
    String,
    DateTime<Utc>,
    DateTime<Utc>,
);

fn map_workspace(row: WorkspaceRow) -> PersistenceResult<Workspace> {
    let status = match row.4.as_str() {
        "ACTIVE" => WorkspaceStatus::Active,
        "ARCHIVED" => WorkspaceStatus::Archived,
        value => {
            return Err(PersistenceError::Other(format!(
                "invalid workspace status {value}"
            )))
        }
    };
    Ok(Workspace {
        id: WorkspaceId::new(row.0)?,
        organization_id: OrganizationId::new(row.1)?,
        name: row.2,
        slug: row.3,
        status,
        is_default: row.5,
        revision: u64::try_from(row.6)
            .map_err(|_| PersistenceError::Other("negative workspace revision".into()))?,
        created_by: row.7,
        created_at: row.8,
        updated_at: row.9,
    })
}

const SELECT_WORKSPACE: &str = r#"
    SELECT id,organization_id,name,slug,status,is_default,revision,created_by,created_at,updated_at
      FROM workspaces
"#;

pub async fn list_for_organization(
    pool: &PgPool,
    organization_id: &str,
) -> PersistenceResult<Vec<Workspace>> {
    let query = format!(
        "{SELECT_WORKSPACE} WHERE organization_id=$1 ORDER BY is_default DESC,created_at,id"
    );
    let rows = sqlx::query_as::<_, WorkspaceRow>(query.as_str())
        .bind(organization_id)
        .fetch_all(pool)
        .await?;
    rows.into_iter().map(map_workspace).collect()
}

pub async fn get_for_organization(
    pool: &PgPool,
    organization_id: &str,
    workspace_id: &str,
) -> PersistenceResult<Option<Workspace>> {
    let query = format!("{SELECT_WORKSPACE} WHERE organization_id=$1 AND id=$2");
    let row = sqlx::query_as::<_, WorkspaceRow>(query.as_str())
        .bind(organization_id)
        .bind(workspace_id)
        .fetch_optional(pool)
        .await?;
    row.map(map_workspace).transpose()
}

pub(crate) async fn default_for_organization_in_transaction(
    tx: &mut Transaction<'_, Postgres>,
    organization_id: &str,
) -> PersistenceResult<Workspace> {
    let query = format!(
        "{SELECT_WORKSPACE} WHERE organization_id=$1 AND is_default=TRUE AND status='ACTIVE'"
    );
    let row = sqlx::query_as::<_, WorkspaceRow>(query.as_str())
        .bind(organization_id)
        .fetch_optional(&mut **tx)
        .await?
        .ok_or_else(|| PersistenceError::Other(
            "organization default workspace was not provisioned".into(),
        ))?;
    map_workspace(row)
}

pub async fn default_for_organization(
    pool: &PgPool,
    organization_id: &str,
) -> PersistenceResult<Option<Workspace>> {
    let query = format!(
        "{SELECT_WORKSPACE} WHERE organization_id=$1 AND is_default=TRUE AND status='ACTIVE'"
    );
    let row = sqlx::query_as::<_, WorkspaceRow>(query.as_str())
        .bind(organization_id)
        .fetch_optional(pool)
        .await?;
    row.map(map_workspace).transpose()
}

pub async fn for_account(
    pool: &PgPool,
    organization_id: &str,
    account_id: &str,
) -> PersistenceResult<Option<Workspace>> {
    let query = r#"
        SELECT w.id,w.organization_id,w.name,w.slug,w.status,w.is_default,w.revision,
               w.created_by,w.created_at,w.updated_at
          FROM workspaces w
          JOIN workspace_account_assignments a
            ON a.workspace_id=w.id AND a.organization_id=w.organization_id
         WHERE a.organization_id=$1 AND a.account_id=$2
    "#;
    let row = sqlx::query_as::<_, WorkspaceRow>(query)
        .bind(organization_id)
        .bind(account_id)
        .fetch_optional(pool)
        .await?;
    row.map(map_workspace).transpose()
}

pub async fn create(pool: &PgPool, workspace: &Workspace, reason: &str) -> PersistenceResult<()> {
    if reason.trim().is_empty() || reason.len() > 1000 {
        return Err(PersistenceError::Other(
            "workspace creation reason must contain 1 to 1000 characters".into(),
        ));
    }
    let mut tx = pool.begin().await?;
    insert_in_transaction(&mut tx, workspace, reason).await?;
    tx.commit().await?;
    Ok(())
}

pub(crate) async fn insert_in_transaction(
    tx: &mut Transaction<'_, Postgres>,
    workspace: &Workspace,
    reason: &str,
) -> PersistenceResult<()> {
    if reason.trim().is_empty() || reason.len() > 1000 {
        return Err(PersistenceError::Other(
            "workspace creation reason must contain 1 to 1000 characters".into(),
        ));
    }
    let creator_is_authorized: bool = sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM organizations o JOIN organization_memberships m ON m.organization_id=o.id WHERE o.id=$1 AND o.status='ACTIVE' AND m.user_id=$2 AND m.status='ACTIVE' AND m.role IN ('OWNER','ADMIN'))",
    )
    .bind(workspace.organization_id.as_str())
    .bind(&workspace.created_by)
    .fetch_one(&mut **tx)
    .await?;
    if !creator_is_authorized {
        return Err(PersistenceError::Other(
            "workspace creator must be an active organization OWNER or ADMIN".into(),
        ));
    }

    sqlx::query(
        "INSERT INTO workspaces(id,organization_id,name,slug,status,is_default,revision,created_by,created_at,updated_at) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10)",
    )
    .bind(workspace.id.as_str())
    .bind(workspace.organization_id.as_str())
    .bind(&workspace.name)
    .bind(&workspace.slug)
    .bind(workspace.status.as_db())
    .bind(workspace.is_default)
    .bind(i64::try_from(workspace.revision).map_err(|_| PersistenceError::Other("workspace revision exceeds database range".into()))?)
    .bind(&workspace.created_by)
    .bind(workspace.created_at)
    .bind(workspace.updated_at)
    .execute(&mut **tx)
    .await?;

    sqlx::query(
        "INSERT INTO workspace_events(workspace_id,organization_id,actor_user_id,revision,event_type,previous_name,next_name,previous_status,next_status,previous_is_default,next_is_default,reason) VALUES($1,$2,$3,$4,'CREATED',NULL,$5,NULL,$6,NULL,$7,$8)",
    )
    .bind(workspace.id.as_str())
    .bind(workspace.organization_id.as_str())
    .bind(&workspace.created_by)
    .bind(i64::try_from(workspace.revision).map_err(|_| PersistenceError::Other("workspace revision exceeds database range".into()))?)
    .bind(&workspace.name)
    .bind(workspace.status.as_db())
    .bind(workspace.is_default)
    .bind(reason.trim())
    .execute(&mut **tx)
    .await?;
    Ok(())
}

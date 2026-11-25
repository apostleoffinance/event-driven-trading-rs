use chrono::{DateTime, Utc};
use domain::{
    Account, AccountEnvironment, AccountState, AccountType, EntitlementStatus, MembershipStatus,
    Organization, OrganizationAccountEntitlement, OrganizationMembership, OrganizationRole,
    OrganizationStatus,
};
use sqlx::{PgPool, Postgres, Transaction};

use crate::error::{PersistenceError, PersistenceResult};

fn role(value: &str) -> PersistenceResult<OrganizationRole> {
    match value {
        "OWNER" => Ok(OrganizationRole::Owner),
        "ADMIN" => Ok(OrganizationRole::Admin),
        "TRADER" => Ok(OrganizationRole::Trader),
        "RISK_MANAGER" => Ok(OrganizationRole::RiskManager),
        "VIEWER" => Ok(OrganizationRole::Viewer),
        _ => Err(PersistenceError::Other(format!(
            "invalid organization role {value}"
        ))),
    }
}

fn membership_status(value: &str) -> PersistenceResult<MembershipStatus> {
    match value {
        "ACTIVE" => Ok(MembershipStatus::Active),
        "SUSPENDED" => Ok(MembershipStatus::Suspended),
        "REVOKED" => Ok(MembershipStatus::Revoked),
        _ => Err(PersistenceError::Other(format!(
            "invalid membership status {value}"
        ))),
    }
}

fn entitlement_status(value: &str) -> PersistenceResult<EntitlementStatus> {
    match value {
        "ACTIVE" => Ok(EntitlementStatus::Active),
        "SUSPENDED" => Ok(EntitlementStatus::Suspended),
        "REVOKED" => Ok(EntitlementStatus::Revoked),
        _ => Err(PersistenceError::Other(format!(
            "invalid entitlement status {value}"
        ))),
    }
}

pub async fn create(
    pool: &PgPool,
    org: &Organization,
    actor: &str,
) -> PersistenceResult<domain::Workspace> {
    let mut tx = pool.begin().await?;
    sqlx::query("INSERT INTO organizations(id,name,status,revision,created_by,created_at,updated_at) VALUES($1,$2,'ACTIVE',1,$3,$4,$4)")
        .bind(org.id.as_str()).bind(&org.name).bind(actor).bind(org.created_at)
        .execute(&mut *tx).await?;
    sqlx::query("INSERT INTO organization_events(organization_id,actor_user_id,revision,next_name,next_status,reason) VALUES($1,$2,1,$3,'ACTIVE','organization created')")
        .bind(org.id.as_str()).bind(actor).bind(&org.name).execute(&mut *tx).await?;
    sqlx::query("INSERT INTO organization_memberships(organization_id,user_id,role,status,revision,created_by,updated_by,created_at,updated_at) VALUES($1,$2,'OWNER','ACTIVE',1,$2,$2,$3,$3)")
        .bind(org.id.as_str()).bind(actor).bind(org.created_at).execute(&mut *tx).await?;
    append_membership_event(
        &mut tx,
        org.id.as_str(),
        actor,
        actor,
        1,
        None,
        "OWNER",
        None,
        "ACTIVE",
        "organization creator becomes initial owner",
    )
    .await?;

    // M1's AFTER INSERT trigger provisions the default workspace in this
    // transaction. Read it before commit: a missing workspace must roll back
    // the organization and initial OWNER membership rather than create a partial tenant.
    let default_workspace =
        super::workspaces::default_for_organization_in_transaction(&mut tx, org.id.as_str())
            .await?;

    tx.commit().await?;
    Ok(default_workspace)
}

pub async fn revise_organization(
    pool: &PgPool,
    org: &str,
    actor: &str,
    expected: i64,
    name: Option<&str>,
    status: Option<OrganizationStatus>,
    reason: &str,
) -> PersistenceResult<i64> {
    let cleaned_name = name.map(str::trim);
    if cleaned_name.is_some_and(|n| n.is_empty() || n.len() > 120) || reason.trim().is_empty() {
        return Err(PersistenceError::Other(
            "invalid organization update".into(),
        ));
    }
    let mut tx = pool.begin().await?;
    let old = sqlx::query_as::<_, (String, String, i64)>(
        "SELECT name,status,revision FROM organizations WHERE id=$1 FOR UPDATE",
    )
    .bind(org)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or_else(|| PersistenceError::NotFound(org.into()))?;
    let owner: bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM organization_memberships WHERE organization_id=$1 AND user_id=$2 AND role='OWNER' AND status='ACTIVE')")
        .bind(org).bind(actor).fetch_one(&mut *tx).await?;
    if !owner || old.2 != expected {
        return Err(PersistenceError::Other(
            "organization owner or revision conflict".into(),
        ));
    }
    let next_name = cleaned_name.unwrap_or(&old.0);
    let next_status = status
        .map(|v| match v {
            OrganizationStatus::Active => "ACTIVE",
            OrganizationStatus::Suspended => "SUSPENDED",
            OrganizationStatus::Closed => "CLOSED",
        })
        .unwrap_or(&old.1);
    if next_name == old.0 && next_status == old.1 {
        tx.commit().await?;
        return Ok(expected);
    }
    let revision = expected
        .checked_add(1)
        .ok_or_else(|| PersistenceError::Other("organization revision overflow".into()))?;
    sqlx::query("UPDATE organizations SET name=$2,status=$3,revision=$4,updated_at=NOW() WHERE id=$1 AND revision=$5")
        .bind(org).bind(next_name).bind(next_status).bind(revision).bind(expected).execute(&mut *tx).await?;
    sqlx::query("INSERT INTO organization_events(organization_id,actor_user_id,revision,previous_name,next_name,previous_status,next_status,reason) VALUES($1,$2,$3,$4,$5,$6,$7,$8)")
        .bind(org).bind(actor).bind(revision).bind(&old.0).bind(next_name).bind(&old.1).bind(next_status).bind(reason.trim()).execute(&mut *tx).await?;
    tx.commit().await?;
    Ok(revision)
}

#[allow(clippy::too_many_arguments)]
async fn append_membership_event(
    tx: &mut Transaction<'_, Postgres>,
    org: &str,
    user: &str,
    actor: &str,
    revision: i64,
    old_role: Option<&str>,
    new_role: &str,
    old_status: Option<&str>,
    new_status: &str,
    reason: &str,
) -> PersistenceResult<()> {
    sqlx::query("INSERT INTO organization_membership_events(organization_id,user_id,actor_user_id,revision,previous_role,next_role,previous_status,next_status,reason) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9)")
        .bind(org).bind(user).bind(actor).bind(revision).bind(old_role).bind(new_role)
        .bind(old_status).bind(new_status).bind(reason.trim()).execute(&mut **tx).await?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn append_entitlement_event(
    tx: &mut Transaction<'_, Postgres>,
    account: &str,
    actor: &str,
    old_org: Option<&str>,
    new_org: &str,
    old_status: Option<&str>,
    new_status: &str,
    account_revision: i64,
    entitlement_revision: i64,
    reason: &str,
) -> PersistenceResult<()> {
    sqlx::query("INSERT INTO organization_account_entitlement_events(account_id,actor_user_id,previous_organization_id,next_organization_id,previous_status,next_status,account_revision,entitlement_revision,reason) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9)")
        .bind(account).bind(actor).bind(old_org).bind(new_org).bind(old_status).bind(new_status)
        .bind(account_revision).bind(entitlement_revision).bind(reason.trim()).execute(&mut **tx).await?;
    Ok(())
}

pub async fn list_for_user(
    pool: &PgPool,
    user_id: &str,
) -> PersistenceResult<Vec<(Organization, OrganizationRole, u64)>> {
    let rows = sqlx::query_as::<_, (String,String,String,i64,DateTime<Utc>,DateTime<Utc>,String,i64)>(
        "SELECT o.id,o.name,o.status,o.revision,o.created_at,o.updated_at,m.role,m.revision FROM organizations o JOIN organization_memberships m ON m.organization_id=o.id WHERE m.user_id=$1 AND m.status='ACTIVE' ORDER BY o.created_at")
        .bind(user_id).fetch_all(pool).await?;
    rows.into_iter()
        .map(|r| {
            let status = match r.2.as_str() {
                "ACTIVE" => OrganizationStatus::Active,
                "SUSPENDED" => OrganizationStatus::Suspended,
                "CLOSED" => OrganizationStatus::Closed,
                _ => {
                    return Err(PersistenceError::Other(
                        "invalid organization status".into(),
                    ))
                }
            };
            Ok((
                Organization {
                    id: domain::OrganizationId::new(r.0)?,
                    name: r.1,
                    status,
                    revision: u64::try_from(r.3).map_err(|_| {
                        PersistenceError::Other("negative organization revision".into())
                    })?,
                    created_at: r.4,
                    updated_at: r.5,
                },
                role(&r.6)?,
                u64::try_from(r.7)
                    .map_err(|_| PersistenceError::Other("negative membership revision".into()))?,
            ))
        })
        .collect()
}

pub async fn membership(
    pool: &PgPool,
    org: &str,
    user: &str,
) -> PersistenceResult<Option<(OrganizationRole, MembershipStatus, i64)>> {
    let row = sqlx::query_as::<_,(String,String,i64)>("SELECT m.role,m.status,m.revision FROM organization_memberships m JOIN organizations o ON o.id=m.organization_id WHERE m.organization_id=$1 AND m.user_id=$2 AND o.status='ACTIVE'")
        .bind(org).bind(user).fetch_optional(pool).await?;
    row.map(|(r, s, v)| Ok((role(&r)?, membership_status(&s)?, v)))
        .transpose()
}

pub async fn list_members(
    pool: &PgPool,
    org: &str,
) -> PersistenceResult<Vec<OrganizationMembership>> {
    let rows=sqlx::query_as::<_,(String,String,String,String,i64,DateTime<Utc>,DateTime<Utc>)>("SELECT organization_id,user_id,role,status,revision,created_at,updated_at FROM organization_memberships WHERE organization_id=$1 ORDER BY created_at,user_id")
        .bind(org).fetch_all(pool).await?;
    rows.into_iter()
        .map(|r| {
            Ok(OrganizationMembership {
                organization_id: domain::OrganizationId::new(r.0)?,
                user_id: r.1,
                role: role(&r.2)?,
                status: membership_status(&r.3)?,
                revision: u64::try_from(r.4)
                    .map_err(|_| PersistenceError::Other("negative membership revision".into()))?,
                created_at: r.5,
                updated_at: r.6,
            })
        })
        .collect()
}

pub async fn create_membership(
    pool: &PgPool,
    org: &str,
    user: &str,
    actor: &str,
    new_role: OrganizationRole,
    reason: &str,
) -> PersistenceResult<()> {
    if new_role == OrganizationRole::Owner {
        return Err(PersistenceError::Other(
            "ownership must be established with the explicit transfer operation".into(),
        ));
    }
    let mut tx = pool.begin().await?;
    sqlx::query("SELECT id FROM organizations WHERE id=$1 AND status='ACTIVE' FOR UPDATE")
        .bind(org)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or_else(|| PersistenceError::NotFound(org.into()))?;
    let actor_role: Option<String>=sqlx::query_scalar("SELECT role FROM organization_memberships WHERE organization_id=$1 AND user_id=$2 AND status='ACTIVE'").bind(org).bind(actor).fetch_optional(&mut *tx).await?;
    let actor_role =
        actor_role.ok_or_else(|| PersistenceError::NotFound("organization membership".into()))?;
    if !matches!(actor_role.as_str(), "OWNER" | "ADMIN")
        || (actor_role == "ADMIN"
            && matches!(new_role, OrganizationRole::Owner | OrganizationRole::Admin))
    {
        return Err(PersistenceError::Other(
            "membership management is not permitted".into(),
        ));
    }
    sqlx::query("INSERT INTO organization_memberships(organization_id,user_id,role,status,revision,created_by,updated_by) VALUES($1,$2,$3,'ACTIVE',1,$4,$4)")
        .bind(org).bind(user).bind(new_role.as_db()).bind(actor).execute(&mut *tx).await?;
    append_membership_event(
        &mut tx,
        org,
        user,
        actor,
        1,
        None,
        new_role.as_db(),
        None,
        "ACTIVE",
        reason,
    )
    .await?;
    tx.commit().await?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub async fn revise_membership(
    pool: &PgPool,
    org: &str,
    target: &str,
    actor: &str,
    expected: i64,
    new_role: OrganizationRole,
    new_status: MembershipStatus,
    reason: &str,
) -> PersistenceResult<i64> {
    let mut tx = pool.begin().await?;
    sqlx::query("SELECT id FROM organizations WHERE id=$1 AND status='ACTIVE' FOR UPDATE")
        .bind(org)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or_else(|| PersistenceError::NotFound(org.into()))?;
    let actor_role: Option<String>=sqlx::query_scalar("SELECT role FROM organization_memberships WHERE organization_id=$1 AND user_id=$2 AND status='ACTIVE'").bind(org).bind(actor).fetch_optional(&mut *tx).await?;
    let actor_role =
        actor_role.ok_or_else(|| PersistenceError::NotFound("organization membership".into()))?;
    let old=sqlx::query_as::<_,(String,String,i64)>("SELECT role,status,revision FROM organization_memberships WHERE organization_id=$1 AND user_id=$2 FOR UPDATE").bind(org).bind(target).fetch_optional(&mut *tx).await?.ok_or_else(||PersistenceError::NotFound("organization membership".into()))?;
    if old.2 != expected {
        return Err(PersistenceError::Other(
            "membership revision conflict".into(),
        ));
    }
    let owner_target = old.0 == "OWNER";
    if owner_target
        || new_role == OrganizationRole::Owner
        || (actor_role == "ADMIN" && (new_role == OrganizationRole::Admin || target == actor))
    {
        return Err(PersistenceError::Other("owner changes require explicit owner transfer; administrator cannot change owner/admin authority or self privileges".into()));
    }
    if actor_role != "OWNER" && actor_role != "ADMIN" {
        return Err(PersistenceError::Other(
            "membership management is not permitted".into(),
        ));
    }
    let next = expected
        .checked_add(1)
        .ok_or_else(|| PersistenceError::Other("membership revision overflow".into()))?;
    sqlx::query("UPDATE organization_memberships SET role=$4,status=$5,revision=$6,updated_by=$3,updated_at=NOW() WHERE organization_id=$1 AND user_id=$2 AND revision=$7")
        .bind(org).bind(target).bind(actor).bind(new_role.as_db()).bind(new_status.as_db()).bind(next).bind(expected).execute(&mut *tx).await?;
    append_membership_event(
        &mut tx,
        org,
        target,
        actor,
        next,
        Some(&old.0),
        new_role.as_db(),
        Some(&old.1),
        new_status.as_db(),
        reason,
    )
    .await?;
    tx.commit().await?;
    Ok(next)
}

pub async fn transfer_owner(
    pool: &PgPool,
    org: &str,
    actor: &str,
    target: &str,
    actor_expected: i64,
    target_expected: i64,
    reason: &str,
) -> PersistenceResult<()> {
    if actor == target {
        return Err(PersistenceError::Other(
            "owner transfer target must differ from actor".into(),
        ));
    }
    let mut tx = pool.begin().await?;
    sqlx::query("SELECT id FROM organizations WHERE id=$1 AND status='ACTIVE' FOR UPDATE")
        .bind(org)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or_else(|| PersistenceError::NotFound(org.into()))?;
    let old=sqlx::query_as::<_,(String,String,i64)>("SELECT role,status,revision FROM organization_memberships WHERE organization_id=$1 AND user_id=$2 FOR UPDATE").bind(org).bind(actor).fetch_optional(&mut *tx).await?.ok_or_else(||PersistenceError::NotFound("actor membership".into()))?;
    let new=sqlx::query_as::<_,(String,String,i64)>("SELECT role,status,revision FROM organization_memberships WHERE organization_id=$1 AND user_id=$2 FOR UPDATE").bind(org).bind(target).fetch_optional(&mut *tx).await?.ok_or_else(||PersistenceError::NotFound("target membership".into()))?;
    if old.0 != "OWNER"
        || old.1 != "ACTIVE"
        || new.1 != "ACTIVE"
        || old.2 != actor_expected
        || new.2 != target_expected
    {
        return Err(PersistenceError::Other(
            "owner transfer authority or revision conflict".into(),
        ));
    }
    let target_rev = target_expected
        .checked_add(1)
        .ok_or_else(|| PersistenceError::Other("membership revision overflow".into()))?;
    sqlx::query("UPDATE organization_memberships SET role='OWNER',revision=$3,updated_by=$4,updated_at=NOW() WHERE organization_id=$1 AND user_id=$2 AND revision=$5")
        .bind(org).bind(target).bind(target_rev).bind(actor).bind(target_expected).execute(&mut *tx).await?;
    append_membership_event(
        &mut tx,
        org,
        target,
        actor,
        target_rev,
        Some(&new.0),
        "OWNER",
        Some(&new.1),
        "ACTIVE",
        reason,
    )
    .await?;
    let actor_rev = actor_expected
        .checked_add(1)
        .ok_or_else(|| PersistenceError::Other("membership revision overflow".into()))?;
    sqlx::query("UPDATE organization_memberships SET role='ADMIN',revision=$3,updated_by=$4,updated_at=NOW() WHERE organization_id=$1 AND user_id=$2 AND revision=$5")
        .bind(org).bind(actor).bind(actor_rev).bind(actor).bind(actor_expected).execute(&mut *tx).await?;
    append_membership_event(
        &mut tx,
        org,
        actor,
        actor,
        actor_rev,
        Some(&old.0),
        "ADMIN",
        Some(&old.1),
        "ACTIVE",
        reason,
    )
    .await?;
    tx.commit().await?;
    Ok(())
}

pub async fn list_entitlements(
    pool: &PgPool,
    org: &str,
) -> PersistenceResult<Vec<OrganizationAccountEntitlement>> {
    let rows=sqlx::query_as::<_,(String,String,String,i64,String,String,DateTime<Utc>,DateTime<Utc>)>("SELECT organization_id,account_id,status,revision,assigned_by,reason,created_at,updated_at FROM organization_account_entitlements WHERE organization_id=$1 ORDER BY created_at")
        .bind(org).fetch_all(pool).await?;
    rows.into_iter()
        .map(|r| {
            Ok(OrganizationAccountEntitlement {
                organization_id: domain::OrganizationId::new(r.0)?,
                account_id: domain::AccountId::new(r.1)?,
                status: entitlement_status(&r.2)?,
                revision: u64::try_from(r.3)
                    .map_err(|_| PersistenceError::Other("negative entitlement revision".into()))?,
                assigned_by: r.4,
                reason: r.5,
                created_at: r.6,
                updated_at: r.7,
            })
        })
        .collect()
}

pub async fn assign_account(
    pool: &PgPool,
    org: &str,
    account: &str,
    actor: &str,
    expected_account_revision: i64,
    expected_entitlement_revision: i64,
    reason: &str,
) -> PersistenceResult<i64> {
    let mut tx = pool.begin().await?;
    sqlx::query("SELECT id FROM organizations WHERE id=$1 AND status='ACTIVE' FOR UPDATE")
        .bind(org)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or_else(|| PersistenceError::NotFound(org.into()))?;
    let allowed: bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM organization_memberships WHERE organization_id=$1 AND user_id=$2 AND role='OWNER' AND status='ACTIVE')").bind(org).bind(actor).fetch_one(&mut *tx).await?;
    if !allowed {
        return Err(PersistenceError::Other(
            "only an active organization OWNER may assign Quant accounts".into(),
        ));
    }
    let account_revision: Option<i64> =
        sqlx::query_scalar("SELECT account_revision FROM accounts WHERE id=$1 FOR UPDATE")
            .bind(account)
            .fetch_optional(&mut *tx)
            .await?;
    let account_revision =
        account_revision.ok_or_else(|| PersistenceError::NotFound("account".into()))?;
    if account_revision != expected_account_revision {
        return Err(PersistenceError::Other("account revision conflict".into()));
    }
    let old=sqlx::query_as::<_,(String,String,i64)>("SELECT organization_id,status,revision FROM organization_account_entitlements WHERE account_id=$1 FOR UPDATE").bind(account).fetch_optional(&mut *tx).await?;
    match old {
        None if expected_entitlement_revision == 0 => {
            sqlx::query("INSERT INTO organization_account_entitlements(account_id,organization_id,status,revision,assigned_by,updated_by,reason) VALUES($1,$2,'ACTIVE',1,$3,$3,$4)")
                .bind(account).bind(org).bind(actor).bind(reason.trim()).execute(&mut *tx).await?;
            append_entitlement_event(
                &mut tx,
                account,
                actor,
                None,
                org,
                None,
                "ACTIVE",
                account_revision,
                1,
                reason,
            )
            .await?;
            tx.commit().await?;
            Ok(1)
        }
        Some((old_org, old_status, old_revision))
            if old_revision == expected_entitlement_revision =>
        {
            if old_org != org {
                let source_owner: bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM organization_memberships WHERE organization_id=$1 AND user_id=$2 AND role='OWNER' AND status='ACTIVE')").bind(&old_org).bind(actor).fetch_one(&mut *tx).await?;
                if !source_owner {
                    return Err(PersistenceError::Other(
                        "reassignment requires OWNER approval in both organizations".into(),
                    ));
                }
            }
            let next = old_revision
                .checked_add(1)
                .ok_or_else(|| PersistenceError::Other("entitlement revision overflow".into()))?;
            sqlx::query("UPDATE organization_account_entitlements SET organization_id=$2,status='ACTIVE',revision=$3,assigned_by=$4,updated_by=$4,reason=$5,updated_at=NOW() WHERE account_id=$1 AND revision=$6")
                .bind(account).bind(org).bind(next).bind(actor).bind(reason.trim()).bind(old_revision).execute(&mut *tx).await?;
            append_entitlement_event(
                &mut tx,
                account,
                actor,
                Some(&old_org),
                org,
                Some(&old_status),
                "ACTIVE",
                account_revision,
                next,
                reason,
            )
            .await?;
            tx.commit().await?;
            Ok(next)
        }
        _ => Err(PersistenceError::Other(
            "entitlement revision conflict".into(),
        )),
    }
}

/// Create a paper Quant account and its first organization entitlement atomically.
/// This is intentionally narrower than account upsert and can only be used by an active OWNER.
pub async fn create_paper_account_for_owner(
    pool: &PgPool,
    organization: &str,
    actor: &str,
    account: &Account,
    starting_capital: rust_decimal::Decimal,
    label: &str,
    state: &AccountState,
) -> PersistenceResult<i64> {
    if account.account_type != AccountType::Simulated
        || account.environment != Some(AccountEnvironment::Paper)
        || account.id != state.account_id
        || starting_capital <= rust_decimal::Decimal::ZERO
        || label.trim().is_empty()
        || state.balance != starting_capital
        || state.equity != starting_capital
    {
        return Err(PersistenceError::Other(
            "invalid paper account creation request".into(),
        ));
    }
    let open_position_count = i32::try_from(state.open_position_count).map_err(|_| {
        PersistenceError::Other("open position count exceeds database range".into())
    })?;

    let mut tx = pool.begin().await?;
    let owner: Option<String> = sqlx::query_scalar("SELECT m.role FROM organizations o JOIN organization_memberships m ON m.organization_id=o.id WHERE o.id=$1 AND o.status='ACTIVE' AND m.user_id=$2 AND m.status='ACTIVE' FOR UPDATE OF o,m")
        .bind(organization).bind(actor).fetch_optional(&mut *tx).await?;
    if owner.as_deref() != Some("OWNER") {
        return Err(PersistenceError::Other(
            "active organization OWNER membership is required".into(),
        ));
    }

    sqlx::query("INSERT INTO venues(id,venue_type,name,enabled,created_at) VALUES('simulated','simulated','Paper Simulator',FALSE,$1) ON CONFLICT(id) DO NOTHING")
        .bind(account.created_at).execute(&mut *tx).await?;
    let venue_type: Option<String> =
        sqlx::query_scalar("SELECT venue_type FROM venues WHERE id='simulated' FOR SHARE")
            .fetch_optional(&mut *tx)
            .await?;
    if venue_type.as_deref() != Some("simulated") || account.venue_id.as_str() != "simulated" {
        return Err(PersistenceError::Other(
            "simulated venue identity is unavailable".into(),
        ));
    }

    sqlx::query("INSERT INTO accounts(id,account_type,status,venue_id,starting_capital,label,created_at,environment) VALUES($1,'simulated','active','simulated',$2,$3,$4,'PAPER')")
        .bind(account.id.as_str()).bind(starting_capital).bind(label.trim()).bind(account.created_at).execute(&mut *tx).await?;
    sqlx::query("INSERT INTO account_states(account_id,balance,equity,used_margin,open_position_count,daily_realized_pnl,peak_equity,kill_switch,updated_at) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9)")
        .bind(state.account_id.as_str()).bind(state.balance).bind(state.equity).bind(state.used_margin).bind(open_position_count).bind(state.daily_realized_pnl).bind(state.peak_equity).bind(state.kill_switch).bind(state.updated_at).execute(&mut *tx).await?;
    let account_revision: i64 =
        sqlx::query_scalar("SELECT account_revision FROM accounts WHERE id=$1")
            .bind(account.id.as_str())
            .fetch_one(&mut *tx)
            .await?;
    if account_revision != 1 {
        return Err(PersistenceError::Other(
            "new account revision must start at one".into(),
        ));
    }
    sqlx::query("INSERT INTO organization_account_entitlements(account_id,organization_id,status,revision,assigned_by,updated_by,reason) VALUES($1,$2,'ACTIVE',1,$3,$3,'created as an organization paper account')")
        .bind(account.id.as_str()).bind(organization).bind(actor).execute(&mut *tx).await?;
    append_entitlement_event(
        &mut tx,
        account.id.as_str(),
        actor,
        None,
        organization,
        None,
        "ACTIVE",
        account_revision,
        1,
        "created as an organization paper account",
    )
    .await?;
    tx.commit().await?;
    Ok(1)
}

#[allow(clippy::too_many_arguments)]
pub async fn set_entitlement_status(
    pool: &PgPool,
    org: &str,
    account: &str,
    actor: &str,
    expected_account_revision: i64,
    expected_entitlement_revision: i64,
    status: EntitlementStatus,
    reason: &str,
) -> PersistenceResult<i64> {
    let mut tx = pool.begin().await?;
    sqlx::query("SELECT id FROM organizations WHERE id=$1 AND status='ACTIVE' FOR UPDATE")
        .bind(org)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or_else(|| PersistenceError::NotFound(org.into()))?;
    let allowed: bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM organization_memberships WHERE organization_id=$1 AND user_id=$2 AND role='OWNER' AND status='ACTIVE')").bind(org).bind(actor).fetch_one(&mut *tx).await?;
    if !allowed {
        return Err(PersistenceError::Other(
            "only an active organization OWNER may change account entitlement".into(),
        ));
    }
    let account_revision: Option<i64> =
        sqlx::query_scalar("SELECT account_revision FROM accounts WHERE id=$1 FOR UPDATE")
            .bind(account)
            .fetch_optional(&mut *tx)
            .await?;
    let account_revision =
        account_revision.ok_or_else(|| PersistenceError::NotFound("account".into()))?;
    if account_revision != expected_account_revision {
        return Err(PersistenceError::Other("account revision conflict".into()));
    }
    let old=sqlx::query_as::<_,(String,String,i64)>("SELECT organization_id,status,revision FROM organization_account_entitlements WHERE account_id=$1 FOR UPDATE").bind(account).fetch_optional(&mut *tx).await?.ok_or_else(||PersistenceError::NotFound("entitlement".into()))?;
    if old.0 != org || old.2 != expected_entitlement_revision {
        return Err(PersistenceError::Other(
            "entitlement revision conflict".into(),
        ));
    }
    let next = old
        .2
        .checked_add(1)
        .ok_or_else(|| PersistenceError::Other("entitlement revision overflow".into()))?;
    sqlx::query("UPDATE organization_account_entitlements SET status=$3,revision=$4,updated_by=$5,reason=$6,updated_at=NOW() WHERE account_id=$1 AND organization_id=$2 AND revision=$7")
        .bind(account).bind(org).bind(status.as_db()).bind(next).bind(actor).bind(reason.trim()).bind(old.2).execute(&mut *tx).await?;
    append_entitlement_event(
        &mut tx,
        account,
        actor,
        Some(org),
        org,
        Some(&old.1),
        status.as_db(),
        account_revision,
        next,
        reason,
    )
    .await?;
    tx.commit().await?;
    Ok(next)
}

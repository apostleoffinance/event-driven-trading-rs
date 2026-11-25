//! Workspace grants and organization/workspace capability composition.
//!
//! Organization membership is always authoritative. A workspace grant can
//! narrow an organization's capabilities, but can never elevate them.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::{
    DomainError, DomainResult, MembershipStatus, OrganizationId, OrganizationRole,
    OrganizationStatus, WorkspaceId, WorkspaceStatus,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum WorkspaceRole {
    Admin,
    Trader,
    RiskManager,
    Viewer,
}

impl WorkspaceRole {
    pub fn as_db(self) -> &'static str {
        match self {
            Self::Admin => "WORKSPACE_ADMIN",
            Self::Trader => "TRADER",
            Self::RiskManager => "RISK_MANAGER",
            Self::Viewer => "VIEWER",
        }
    }

    fn capabilities(self) -> WorkspaceCapabilities {
        match self {
            Self::Admin => WorkspaceCapabilities::operational_admin(),
            Self::Trader => WorkspaceCapabilities {
                read: true,
                trade: true,
                manage_risk: false,
                manage_workspace: false,
            },
            Self::RiskManager => WorkspaceCapabilities {
                read: true,
                trade: false,
                manage_risk: true,
                manage_workspace: false,
            },
            Self::Viewer => WorkspaceCapabilities::read_only(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum WorkspaceGrantStatus {
    Active,
    Revoked,
}

impl WorkspaceGrantStatus {
    pub fn as_db(self) -> &'static str {
        match self {
            Self::Active => "ACTIVE",
            Self::Revoked => "REVOKED",
        }
    }
}

/// Persisted user-to-workspace grant. The composite organization/workspace/user
/// identity is stable across revocation and later reactivation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkspaceAccessGrant {
    pub organization_id: OrganizationId,
    pub workspace_id: WorkspaceId,
    pub user_id: String,
    pub role: WorkspaceRole,
    pub status: WorkspaceGrantStatus,
    pub revision: u64,
    pub granted_by: String,
    pub updated_by: String,
    pub reason: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl WorkspaceAccessGrant {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        organization_id: OrganizationId,
        workspace_id: WorkspaceId,
        user_id: impl Into<String>,
        member_role: OrganizationRole,
        member_status: MembershipStatus,
        role: WorkspaceRole,
        actor_role: OrganizationRole,
        actor_status: MembershipStatus,
        actor: impl Into<String>,
        reason: impl Into<String>,
        now: DateTime<Utc>,
    ) -> DomainResult<Self> {
        if member_role.may_administer() {
            return Err(DomainError::Invariant(
                "organization OWNER/ADMIN access is implicit; explicit workspace grants are not applicable".into(),
            ));
        }
        if member_status != MembershipStatus::Active {
            return Err(DomainError::Invariant(
                "workspace grants require an active organization member".into(),
            ));
        }
        if !actor_role.may_administer() || actor_status != MembershipStatus::Active {
            return Err(DomainError::Invariant(
                "workspace grants require an active organization OWNER or ADMIN actor".into(),
            ));
        }
        let user_id = validate_identity(user_id.into(), "workspace grant recipient")?;
        let actor = validate_identity(actor.into(), "workspace grant actor")?;
        let reason = validate_reason(reason.into())?;
        Ok(Self {
            organization_id,
            workspace_id,
            user_id,
            role,
            status: WorkspaceGrantStatus::Active,
            revision: 1,
            granted_by: actor.clone(),
            updated_by: actor,
            reason,
            created_at: now,
            updated_at: now,
        })
    }

    pub fn revise(
        &mut self,
        expected_revision: u64,
        role: WorkspaceRole,
        status: WorkspaceGrantStatus,
        member_role: OrganizationRole,
        member_status: MembershipStatus,
        actor_role: OrganizationRole,
        actor_status: MembershipStatus,
        actor: &str,
        reason: &str,
        now: DateTime<Utc>,
    ) -> DomainResult<u64> {
        if self.revision != expected_revision {
            return Err(DomainError::Invariant(
                "workspace grant revision conflict".into(),
            ));
        }
        if !actor_role.may_administer() || actor_status != MembershipStatus::Active {
            return Err(DomainError::Invariant(
                "workspace grant changes require an active organization OWNER or ADMIN actor"
                    .into(),
            ));
        }
        if status == WorkspaceGrantStatus::Active
            && (member_role.may_administer() || member_status != MembershipStatus::Active)
        {
            return Err(DomainError::Invariant(
                "active workspace grants require an active non-admin organization member".into(),
            ));
        }
        let actor = validate_identity(actor.to_owned(), "workspace grant actor")?;
        let reason = validate_reason(reason.to_owned())?;
        if self.role != role || self.status != status {
            self.role = role;
            self.status = status;
            self.updated_by = actor;
            self.reason = reason;
            self.revision = self.revision.checked_add(1).ok_or_else(|| {
                DomainError::Invariant("workspace grant revision overflow".into())
            })?;
            self.updated_at = now;
        }
        Ok(self.revision)
    }
}

/// Effective workspace permissions after intersecting organization membership
/// and an optional workspace grant. None means access is denied.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkspaceCapabilities {
    pub read: bool,
    pub trade: bool,
    pub manage_risk: bool,
    /// Does not grant authority to change organization membership or workspace grants.
    pub manage_workspace: bool,
}

impl WorkspaceCapabilities {
    fn read_only() -> Self {
        Self {
            read: true,
            trade: false,
            manage_risk: false,
            manage_workspace: false,
        }
    }

    fn operational_admin() -> Self {
        Self {
            read: true,
            trade: true,
            manage_risk: true,
            manage_workspace: true,
        }
    }
}

/// OWNER/ADMIN have implicit access to all active workspaces; all other
/// organization roles require an active explicit grant.
pub fn effective_workspace_capabilities(
    organization_status: OrganizationStatus,
    membership_status: MembershipStatus,
    organization_role: OrganizationRole,
    grant: Option<(WorkspaceRole, WorkspaceGrantStatus)>,
    workspace_status: WorkspaceStatus,
) -> Option<WorkspaceCapabilities> {
    if organization_status != OrganizationStatus::Active
        || membership_status != MembershipStatus::Active
        || workspace_status != WorkspaceStatus::Active
    {
        return None;
    }
    if organization_role.may_administer() {
        return Some(WorkspaceCapabilities::operational_admin());
    }
    let (workspace_role, grant_status) = grant?;
    if grant_status != WorkspaceGrantStatus::Active {
        return None;
    }
    let organization = match organization_role {
        OrganizationRole::Trader => WorkspaceCapabilities {
            read: true,
            trade: true,
            manage_risk: false,
            manage_workspace: false,
        },
        OrganizationRole::RiskManager => WorkspaceCapabilities {
            read: true,
            trade: false,
            manage_risk: true,
            manage_workspace: false,
        },
        OrganizationRole::Viewer => WorkspaceCapabilities::read_only(),
        OrganizationRole::Owner | OrganizationRole::Admin => {
            WorkspaceCapabilities::operational_admin()
        }
    };
    let workspace = workspace_role.capabilities();
    Some(WorkspaceCapabilities {
        read: organization.read && workspace.read,
        trade: organization.trade && workspace.trade,
        manage_risk: organization.manage_risk && workspace.manage_risk,
        manage_workspace: organization.manage_workspace && workspace.manage_workspace,
    })
}

fn validate_identity(value: String, label: &str) -> DomainResult<String> {
    let value = value.trim().to_owned();
    if value.is_empty() || value.len() > 128 {
        return Err(DomainError::Validation(format!(
            "{label} must contain 1 to 128 characters"
        )));
    }
    Ok(value)
}

fn validate_reason(value: String) -> DomainResult<String> {
    let value = value.trim().to_owned();
    if value.is_empty() || value.len() > 1000 {
        return Err(DomainError::Validation(
            "workspace grant reason must contain 1 to 1000 characters".into(),
        ));
    }
    Ok(value)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use super::*;

    fn org() -> OrganizationId {
        OrganizationId::new("org-1").unwrap()
    }
    fn workspace() -> WorkspaceId {
        WorkspaceId::new("ws-1").unwrap()
    }

    #[test]
    fn owner_and_admin_have_implicit_access_without_grant() {
        for role in [OrganizationRole::Owner, OrganizationRole::Admin] {
            let caps = effective_workspace_capabilities(
                OrganizationStatus::Active,
                MembershipStatus::Active,
                role,
                None,
                WorkspaceStatus::Active,
            )
            .unwrap();
            assert!(caps.read && caps.trade && caps.manage_risk && caps.manage_workspace);
        }
    }

    #[test]
    fn non_admin_members_require_active_grant() {
        assert!(effective_workspace_capabilities(
            OrganizationStatus::Active,
            MembershipStatus::Active,
            OrganizationRole::Trader,
            None,
            WorkspaceStatus::Active
        )
        .is_none());
        assert!(effective_workspace_capabilities(
            OrganizationStatus::Active,
            MembershipStatus::Active,
            OrganizationRole::Trader,
            Some((WorkspaceRole::Trader, WorkspaceGrantStatus::Revoked)),
            WorkspaceStatus::Active
        )
        .is_none());
        assert!(effective_workspace_capabilities(
            OrganizationStatus::Active,
            MembershipStatus::Active,
            OrganizationRole::Trader,
            Some((WorkspaceRole::Trader, WorkspaceGrantStatus::Active)),
            WorkspaceStatus::Archived
        )
        .is_none());
        assert!(effective_workspace_capabilities(
            OrganizationStatus::Suspended,
            MembershipStatus::Active,
            OrganizationRole::Owner,
            None,
            WorkspaceStatus::Active
        )
        .is_none());
        assert!(effective_workspace_capabilities(
            OrganizationStatus::Active,
            MembershipStatus::Suspended,
            OrganizationRole::Owner,
            None,
            WorkspaceStatus::Active
        )
        .is_none());
    }

    #[test]
    fn workspace_role_cannot_elevate_organization_role() {
        let viewer = effective_workspace_capabilities(
            OrganizationStatus::Active,
            MembershipStatus::Active,
            OrganizationRole::Viewer,
            Some((WorkspaceRole::Admin, WorkspaceGrantStatus::Active)),
            WorkspaceStatus::Active,
        )
        .unwrap();
        assert!(viewer.read);
        assert!(!viewer.trade && !viewer.manage_risk && !viewer.manage_workspace);
        let trader = effective_workspace_capabilities(
            OrganizationStatus::Active,
            MembershipStatus::Active,
            OrganizationRole::Trader,
            Some((WorkspaceRole::RiskManager, WorkspaceGrantStatus::Active)),
            WorkspaceStatus::Active,
        )
        .unwrap();
        assert!(trader.read);
        assert!(!trader.trade && !trader.manage_risk);
        let risk = effective_workspace_capabilities(
            OrganizationStatus::Active,
            MembershipStatus::Active,
            OrganizationRole::RiskManager,
            Some((WorkspaceRole::Trader, WorkspaceGrantStatus::Active)),
            WorkspaceStatus::Active,
        )
        .unwrap();
        assert!(risk.read);
        assert!(!risk.trade && !risk.manage_risk);
    }

    #[test]
    fn grant_creation_and_revision_validate_authority_and_reason() {
        let now = Utc::now();
        assert!(WorkspaceAccessGrant::new(
            org(),
            workspace(),
            "owner",
            OrganizationRole::Owner,
            MembershipStatus::Active,
            WorkspaceRole::Viewer,
            OrganizationRole::Admin,
            MembershipStatus::Active,
            "admin",
            "reason",
            now
        )
        .is_err());
        assert!(WorkspaceAccessGrant::new(
            org(),
            workspace(),
            "trader",
            OrganizationRole::Trader,
            MembershipStatus::Suspended,
            WorkspaceRole::Viewer,
            OrganizationRole::Admin,
            MembershipStatus::Active,
            "admin",
            "reason",
            now
        )
        .is_err());
        assert!(WorkspaceAccessGrant::new(
            org(),
            workspace(),
            "trader",
            OrganizationRole::Trader,
            MembershipStatus::Active,
            WorkspaceRole::Viewer,
            OrganizationRole::Trader,
            MembershipStatus::Active,
            "trader",
            "reason",
            now
        )
        .is_err());
        let mut grant = WorkspaceAccessGrant::new(
            org(),
            workspace(),
            "trader",
            OrganizationRole::Trader,
            MembershipStatus::Active,
            WorkspaceRole::Trader,
            OrganizationRole::Owner,
            MembershipStatus::Active,
            "owner",
            "approved",
            now,
        )
        .unwrap();
        assert_eq!(grant.revision, 1);
        assert_eq!(
            grant
                .revise(
                    1,
                    WorkspaceRole::Viewer,
                    WorkspaceGrantStatus::Active,
                    OrganizationRole::Trader,
                    MembershipStatus::Active,
                    OrganizationRole::Admin,
                    MembershipStatus::Active,
                    "admin",
                    "least privilege",
                    now
                )
                .unwrap(),
            2
        );
        assert!(grant
            .revise(
                1,
                WorkspaceRole::Trader,
                WorkspaceGrantStatus::Revoked,
                OrganizationRole::Trader,
                MembershipStatus::Active,
                OrganizationRole::Admin,
                MembershipStatus::Active,
                "admin",
                "stale",
                now
            )
            .is_err());
        assert!(grant
            .revise(
                2,
                WorkspaceRole::Viewer,
                WorkspaceGrantStatus::Revoked,
                OrganizationRole::Trader,
                MembershipStatus::Active,
                OrganizationRole::Admin,
                MembershipStatus::Active,
                " ",
                "reason",
                now
            )
            .is_err());
        assert!(grant
            .revise(
                2,
                WorkspaceRole::Viewer,
                WorkspaceGrantStatus::Active,
                OrganizationRole::Admin,
                MembershipStatus::Active,
                OrganizationRole::Admin,
                MembershipStatus::Active,
                "admin",
                "reactivation denied for implicit admin",
                now
            )
            .is_err());
    }
}

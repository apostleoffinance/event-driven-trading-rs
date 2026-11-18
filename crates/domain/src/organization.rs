//! Organization tenancy, membership and Quant-account entitlement primitives.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::{AccountId, DomainError, DomainResult, OrganizationId};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum OrganizationStatus {
    Active,
    Suspended,
    Closed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Organization {
    pub id: OrganizationId,
    pub name: String,
    pub status: OrganizationStatus,
    pub revision: u64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl Organization {
    pub fn new(
        id: OrganizationId,
        name: impl Into<String>,
        now: DateTime<Utc>,
    ) -> DomainResult<Self> {
        let name = name.into().trim().to_owned();
        if name.is_empty() || name.len() > 120 {
            return Err(DomainError::Validation(
                "organization name must contain 1 to 120 characters".into(),
            ));
        }
        Ok(Self {
            id,
            name,
            status: OrganizationStatus::Active,
            revision: 1,
            created_at: now,
            updated_at: now,
        })
    }

    pub fn revise(
        &mut self,
        expected_revision: u64,
        name: Option<&str>,
        status: Option<OrganizationStatus>,
        now: DateTime<Utc>,
    ) -> DomainResult<u64> {
        if self.revision != expected_revision {
            return Err(DomainError::Invariant(
                "organization revision conflict".into(),
            ));
        }
        let next_name = name.map(str::trim).unwrap_or(&self.name);
        if next_name.is_empty() || next_name.len() > 120 {
            return Err(DomainError::Validation(
                "organization name must contain 1 to 120 characters".into(),
            ));
        }
        if self.name != next_name || status.is_some_and(|next| next != self.status) {
            self.name = next_name.to_owned();
            if let Some(status) = status {
                self.status = status;
            }
            self.revision = self
                .revision
                .checked_add(1)
                .ok_or_else(|| DomainError::Invariant("organization revision overflow".into()))?;
            self.updated_at = now;
        }
        Ok(self.revision)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum OrganizationRole {
    Owner,
    Admin,
    Trader,
    RiskManager,
    Viewer,
}

impl OrganizationRole {
    pub fn may_administer(self) -> bool {
        matches!(self, Self::Owner | Self::Admin)
    }

    pub fn may_manage_owner(self) -> bool {
        matches!(self, Self::Owner)
    }

    pub fn may_write_trading_workspace(self) -> bool {
        matches!(self, Self::Owner | Self::Admin | Self::Trader)
    }

    pub fn may_manage_risk_configuration(self) -> bool {
        matches!(self, Self::Owner | Self::Admin | Self::RiskManager)
    }

    pub fn as_db(self) -> &'static str {
        match self {
            Self::Owner => "OWNER",
            Self::Admin => "ADMIN",
            Self::Trader => "TRADER",
            Self::RiskManager => "RISK_MANAGER",
            Self::Viewer => "VIEWER",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum MembershipStatus {
    Active,
    Suspended,
    Revoked,
}

impl MembershipStatus {
    pub fn as_db(self) -> &'static str {
        match self {
            Self::Active => "ACTIVE",
            Self::Suspended => "SUSPENDED",
            Self::Revoked => "REVOKED",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OrganizationMembership {
    pub organization_id: OrganizationId,
    pub user_id: String,
    pub role: OrganizationRole,
    pub status: MembershipStatus,
    pub revision: u64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl OrganizationMembership {
    pub fn revise(
        &mut self,
        expected_revision: u64,
        role: OrganizationRole,
        status: MembershipStatus,
        now: DateTime<Utc>,
    ) -> DomainResult<u64> {
        if self.revision != expected_revision {
            return Err(DomainError::Invariant(
                "membership revision conflict".into(),
            ));
        }
        if self.role != role || self.status != status {
            self.role = role;
            self.status = status;
            self.revision = self
                .revision
                .checked_add(1)
                .ok_or_else(|| DomainError::Invariant("membership revision overflow".into()))?;
            self.updated_at = now;
        }
        Ok(self.revision)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum EntitlementStatus {
    Active,
    Suspended,
    Revoked,
}

impl EntitlementStatus {
    pub fn as_db(self) -> &'static str {
        match self {
            Self::Active => "ACTIVE",
            Self::Suspended => "SUSPENDED",
            Self::Revoked => "REVOKED",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OrganizationAccountEntitlement {
    pub organization_id: OrganizationId,
    pub account_id: AccountId,
    pub status: EntitlementStatus,
    pub revision: u64,
    pub assigned_by: String,
    pub reason: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl OrganizationAccountEntitlement {
    pub fn revise(
        &mut self,
        expected_revision: u64,
        organization_id: OrganizationId,
        status: EntitlementStatus,
        actor: &str,
        reason: &str,
        now: DateTime<Utc>,
    ) -> DomainResult<u64> {
        if self.revision != expected_revision {
            return Err(DomainError::Invariant(
                "entitlement revision conflict".into(),
            ));
        }
        if actor.trim().is_empty() || reason.trim().is_empty() {
            return Err(DomainError::Validation(
                "actor and reason are required".into(),
            ));
        }
        if self.organization_id != organization_id || self.status != status {
            self.organization_id = organization_id;
            self.status = status;
            self.assigned_by = actor.trim().to_owned();
            self.reason = reason.trim().to_owned();
            self.revision = self
                .revision
                .checked_add(1)
                .ok_or_else(|| DomainError::Invariant("entitlement revision overflow".into()))?;
            self.updated_at = now;
        }
        Ok(self.revision)
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;

    #[test]
    fn revisions_are_optimistic_and_roles_are_narrow() {
        let now = Utc::now();
        let mut org =
            Organization::new(OrganizationId::new("org-1").unwrap(), "Alpha", now).unwrap();
        assert_eq!(org.revise(1, Some("Alpha Ltd"), None, now).unwrap(), 2);
        assert!(org.revise(1, Some("Stale"), None, now).is_err());
        assert!(!OrganizationRole::Viewer.may_manage_risk_configuration());
        assert!(OrganizationRole::RiskManager.may_manage_risk_configuration());
        assert!(!OrganizationRole::RiskManager.may_write_trading_workspace());
        assert!(!OrganizationRole::Trader.may_administer());
    }

    #[test]
    fn membership_and_entitlement_revisions_increment_on_change() {
        let now = Utc::now();
        let mut membership = OrganizationMembership {
            organization_id: OrganizationId::new("org-1").unwrap(),
            user_id: "user-1".into(),
            role: OrganizationRole::Viewer,
            status: MembershipStatus::Active,
            revision: 1,
            created_at: now,
            updated_at: now,
        };
        assert_eq!(
            membership
                .revise(1, OrganizationRole::Trader, MembershipStatus::Active, now)
                .unwrap(),
            2
        );
        assert!(membership
            .revise(1, OrganizationRole::Owner, MembershipStatus::Active, now)
            .is_err());

        let mut entitlement = OrganizationAccountEntitlement {
            organization_id: OrganizationId::new("org-1").unwrap(),
            account_id: AccountId::new("account-1").unwrap(),
            status: EntitlementStatus::Active,
            revision: 1,
            assigned_by: "user-1".into(),
            reason: "reviewed".into(),
            created_at: now,
            updated_at: now,
        };
        assert_eq!(
            entitlement
                .revise(
                    1,
                    OrganizationId::new("org-2").unwrap(),
                    EntitlementStatus::Active,
                    "user-2",
                    "approved transfer",
                    now
                )
                .unwrap(),
            2
        );
        assert!(entitlement
            .revise(
                1,
                OrganizationId::new("org-1").unwrap(),
                EntitlementStatus::Active,
                "user-1",
                "stale",
                now
            )
            .is_err());
    }
}

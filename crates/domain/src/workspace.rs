//! Organization-owned operational workspaces.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::{DomainError, DomainResult, OrganizationId, WorkspaceId};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum WorkspaceStatus {
    Active,
    Archived,
}

impl WorkspaceStatus {
    pub fn as_db(self) -> &'static str {
        match self {
            Self::Active => "ACTIVE",
            Self::Archived => "ARCHIVED",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Workspace {
    pub id: WorkspaceId,
    pub organization_id: OrganizationId,
    pub name: String,
    pub slug: String,
    pub status: WorkspaceStatus,
    pub is_default: bool,
    pub revision: u64,
    pub created_by: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl Workspace {
    pub fn new(
        id: WorkspaceId,
        organization_id: OrganizationId,
        name: impl Into<String>,
        slug: impl Into<String>,
        created_by: impl Into<String>,
        now: DateTime<Utc>,
    ) -> DomainResult<Self> {
        let name = name.into().trim().to_owned();
        let slug = slug.into().trim().to_owned();
        let created_by = created_by.into().trim().to_owned();
        validate_name(&name)?;
        validate_slug(&slug)?;
        if created_by.is_empty() || created_by.len() > 128 {
            return Err(DomainError::Validation(
                "workspace creator must contain 1 to 128 characters".into(),
            ));
        }
        Ok(Self {
            id,
            organization_id,
            name,
            slug,
            status: WorkspaceStatus::Active,
            is_default: false,
            revision: 1,
            created_by,
            created_at: now,
            updated_at: now,
        })
    }

    pub fn revise(
        &mut self,
        expected_revision: u64,
        name: Option<&str>,
        slug: Option<&str>,
        status: Option<WorkspaceStatus>,
        is_default: Option<bool>,
        now: DateTime<Utc>,
    ) -> DomainResult<u64> {
        if self.revision != expected_revision {
            return Err(DomainError::Invariant(
                "workspace revision conflict".into(),
            ));
        }

        let next_name = name.map(str::trim).unwrap_or(&self.name);
        let next_slug = slug.map(str::trim).unwrap_or(&self.slug);
        let next_status = status.unwrap_or(self.status);
        let next_default = is_default.unwrap_or(self.is_default);
        validate_name(next_name)?;
        validate_slug(next_slug)?;
        if next_default && next_status != WorkspaceStatus::Active {
            return Err(DomainError::Invariant(
                "an archived workspace cannot be the default workspace".into(),
            ));
        }

        if self.name != next_name
            || self.slug != next_slug
            || self.status != next_status
            || self.is_default != next_default
        {
            self.name = next_name.to_owned();
            self.slug = next_slug.to_owned();
            self.status = next_status;
            self.is_default = next_default;
            self.revision = self
                .revision
                .checked_add(1)
                .ok_or_else(|| DomainError::Invariant("workspace revision overflow".into()))?;
            self.updated_at = now;
        }
        Ok(self.revision)
    }
}

fn validate_name(name: &str) -> DomainResult<()> {
    if name.is_empty() || name.len() > 120 {
        return Err(DomainError::Validation(
            "workspace name must contain 1 to 120 characters".into(),
        ));
    }
    Ok(())
}

fn validate_slug(slug: &str) -> DomainResult<()> {
    let valid = !slug.is_empty()
        && slug.len() <= 63
        && slug.as_bytes()[0].is_ascii_lowercase()
            || (!slug.is_empty() && slug.len() <= 63 && slug.as_bytes()[0].is_ascii_digit());
    if !valid
        || !slug
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
        || slug.ends_with('-')
    {
        return Err(DomainError::Validation(
            "workspace slug must be 1 to 63 lowercase letters, digits or hyphens and start with a letter or digit".into(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;

    fn workspace() -> Workspace {
        let now = Utc::now();
        Workspace::new(
            WorkspaceId::new("ws-1").unwrap(),
            OrganizationId::new("org-1").unwrap(),
            "Research",
            "research",
            "user-1",
            now,
        )
        .unwrap()
    }

    #[test]
    fn creates_valid_workspace_with_revision_one() {
        let workspace = workspace();
        assert_eq!(workspace.name, "Research");
        assert_eq!(workspace.slug, "research");
        assert_eq!(workspace.status, WorkspaceStatus::Active);
        assert_eq!(workspace.revision, 1);
        assert!(!workspace.is_default);
    }

    #[test]
    fn rejects_invalid_names_slugs_and_creator() {
        let now = Utc::now();
        let org = OrganizationId::new("org-1").unwrap();
        assert!(Workspace::new(WorkspaceId::new("ws-1").unwrap(), org.clone(), " ", "ok", "u", now).is_err());
        assert!(Workspace::new(WorkspaceId::new("ws-2").unwrap(), org.clone(), "Name", "Not Valid", "u", now).is_err());
        assert!(Workspace::new(WorkspaceId::new("ws-3").unwrap(), org, "Name", "valid", " ", now).is_err());
    }

    #[test]
    fn revisions_are_optimistic_and_archived_workspace_cannot_be_default() {
        let now = Utc::now();
        let mut workspace = workspace();
        assert_eq!(workspace.revise(1, Some("Research Team"), None, None, None, now).unwrap(), 2);
        assert!(workspace.revise(1, Some("Stale"), None, None, None, now).is_err());
        assert!(workspace.revise(2, None, None, Some(WorkspaceStatus::Archived), Some(true), now).is_err());
    }
}

//! Trusted broker identity registry primitives.
//!
//! External identities are deliberately separate from execution venues and
//! Quant accounts. This module contains no credentials or provider SDK types.

use crate::{DomainError, DomainResult};
use chrono::{DateTime, Utc};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Broker {
    pub id: crate::BrokerId,
    pub canonical_name: String,
    pub display_name: String,
    pub status: BrokerRegistryStatus,
    pub revision: u64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BrokerServer {
    pub id: crate::BrokerServerId,
    pub broker_id: crate::BrokerId,
    pub canonical_identifier: String,
    pub display_name: String,
    pub provider_type: String,
    pub environment: String,
    pub status: BrokerRegistryStatus,
    pub normalization_version: i16,
    pub revision: u64,
}

/// Account identifiers are never retained here. The lookup value is keyed
/// HMAC-SHA-256, scoped to the broker and server, with explicit key version.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternalBrokerAccount {
    pub id: crate::ExternalBrokerAccountId,
    pub broker_id: crate::BrokerId,
    pub broker_server_id: crate::BrokerServerId,
    pub identifier_normalization_version: i16,
    pub identifier_key_version: String,
    pub identifier_lookup_hmac: [u8; 32],
    pub status: String,
    pub verification_status: BrokerAccountVerificationStatus,
    pub revision: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuantBrokerAccountBinding {
    pub id: crate::BrokerBindingId,
    pub organization_id: crate::OrganizationId,
    pub quant_account_id: crate::AccountId,
    pub external_broker_account_id: crate::ExternalBrokerAccountId,
    pub status: BrokerBindingStatus,
    pub revision: u64,
    pub created_by: String,
    pub approved_by: Option<String>,
    pub reassignment_reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BrokerAccountEvidence {
    pub id: crate::BrokerEvidenceId,
    pub external_broker_account_id: crate::ExternalBrokerAccountId,
    pub provider_type: String,
    pub evidence_type: String,
    pub observed_broker_id: crate::BrokerId,
    pub observed_broker_server_id: crate::BrokerServerId,
    pub observed_identifier_key_version: String,
    pub observed_identifier_normalization_version: i16,
    pub observed_identifier_hmac: [u8; 32],
    pub attestation_key_id: String,
    pub attestation_digest: [u8; 32],
    pub observed_at: DateTime<Utc>,
    pub source_principal: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BrokerAccountVerification {
    pub id: crate::BrokerVerificationId,
    pub external_broker_account_id: crate::ExternalBrokerAccountId,
    pub evidence_id: Option<crate::BrokerEvidenceId>,
    pub decision: BrokerAccountVerificationStatus,
    pub revision: u64,
    pub approval_actor_id: Option<String>,
    pub approval_authority: Option<String>,
    pub approval_reference: Option<String>,
    pub reason: String,
}

pub const BROKER_ID_NORMALIZATION_VERSION: i16 = 1;
pub const SERVER_ID_NORMALIZATION_VERSION: i16 = 1;
pub const EXTERNAL_ACCOUNT_IDENTIFIER_NORMALIZATION_VERSION: i16 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BrokerRegistryStatus {
    Active,
    Disabled,
}

impl BrokerRegistryStatus {
    pub fn as_db(self) -> &'static str {
        match self {
            Self::Active => "ACTIVE",
            Self::Disabled => "DISABLED",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BrokerAccountVerificationStatus {
    PendingEvidence,
    PendingApproval,
    Verified,
    Rejected,
    Suspended,
    Revoked,
}

impl BrokerAccountVerificationStatus {
    pub fn as_db(self) -> &'static str {
        match self {
            Self::PendingEvidence => "PENDING_EVIDENCE",
            Self::PendingApproval => "PENDING_APPROVAL",
            Self::Verified => "VERIFIED",
            Self::Rejected => "REJECTED",
            Self::Suspended => "SUSPENDED",
            Self::Revoked => "REVOKED",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BrokerBindingStatus {
    PendingEvidence,
    PendingApproval,
    Verified,
    Suspended,
    Revoked,
}

impl BrokerBindingStatus {
    pub fn as_db(self) -> &'static str {
        match self {
            Self::PendingEvidence => "PENDING_EVIDENCE",
            Self::PendingApproval => "PENDING_APPROVAL",
            Self::Verified => "VERIFIED",
            Self::Suspended => "SUSPENDED",
            Self::Revoked => "REVOKED",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NormalizedBrokerIdentity {
    pub canonical: String,
    pub normalization_version: i16,
}

/// Normalize registry IDs as lower-case ASCII slugs. Display names are never
/// accepted as IDs. Version 1 intentionally rejects Unicode and interior
/// whitespace so the lookup key is stable and unambiguous.
pub fn normalize_broker_id(value: &str) -> DomainResult<NormalizedBrokerIdentity> {
    normalize_slug(value, BROKER_ID_NORMALIZATION_VERSION, "broker")
}

/// Normalize an MT5 server identity according to the provider-specific v1
/// rule: trim ASCII outer whitespace, lowercase ASCII, preserve punctuation,
/// and reject interior whitespace/non-ASCII. Never normalize display labels.
pub fn normalize_mt5_server_id(value: &str) -> DomainResult<NormalizedBrokerIdentity> {
    normalize_slug(value, SERVER_ID_NORMALIZATION_VERSION, "MT5 server")
}

/// Normalize a provider-observed external account identifier without changing
/// its significant representation. Only surrounding ASCII whitespace is
/// removed; leading zeroes and case remain significant. Callers must not log
/// the input or include it in errors.
pub fn normalize_external_account_identifier(value: &str) -> DomainResult<String> {
    let normalized = value.trim_matches(|character: char| character.is_ascii_whitespace());
    if normalized.is_empty()
        || normalized.len() > 128
        || !normalized.is_ascii()
        || normalized.chars().any(char::is_control)
    {
        return Err(DomainError::InvalidId {
            id: "[redacted]".into(),
            reason: "external account identifier is invalid".into(),
        });
    }
    Ok(normalized.to_owned())
}

fn normalize_slug(
    value: &str,
    version: i16,
    label: &'static str,
) -> DomainResult<NormalizedBrokerIdentity> {
    let canonical = value.trim().to_ascii_lowercase();
    if canonical.is_empty()
        || canonical.len() > 128
        || !canonical.is_ascii()
        || !canonical.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || b"._-".contains(&byte)
        })
    {
        return Err(DomainError::InvalidId {
            id: "[redacted]".into(),
            reason: format!("{label} canonical identifier is invalid"),
        });
    }
    Ok(NormalizedBrokerIdentity {
        canonical,
        normalization_version: version,
    })
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;

    #[test]
    fn broker_and_server_ids_are_canonical_and_versioned() {
        let broker = normalize_broker_id("  Example.Broker ").unwrap();
        assert_eq!(broker.canonical, "example.broker");
        assert_eq!(broker.normalization_version, 1);
        let server = normalize_mt5_server_id("  Demo-Server  ").unwrap();
        assert_eq!(server.canonical, "demo-server");
        assert_eq!(server.normalization_version, 1);
    }

    #[test]
    fn normalization_rejects_ambiguous_and_non_ascii_values() {
        assert!(normalize_broker_id("two words").is_err());
        assert!(normalize_mt5_server_id("brökér").is_err());
        assert!(normalize_mt5_server_id("server\nname").is_err());
    }

    #[test]
    fn external_identifier_preserves_leading_zeroes_and_redacts_errors() {
        assert_eq!(
            normalize_external_account_identifier(" 001234 ").unwrap(),
            "001234"
        );
        let error = normalize_external_account_identifier("\n").unwrap_err();
        assert!(!error.to_string().contains("001234"));
        assert!(normalize_external_account_identifier("id\nwith-control").is_err());
    }

    #[test]
    fn identity_states_do_not_promote_without_workflow() {
        assert_eq!(
            BrokerAccountVerificationStatus::PendingEvidence.as_db(),
            "PENDING_EVIDENCE"
        );
        assert_eq!(
            BrokerBindingStatus::PendingApproval.as_db(),
            "PENDING_APPROVAL"
        );
    }
}

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



/// Environment assertion supplied by a trusted registry/admission fact source.
/// Broker-server environments are intentionally not mapped to Quant account
/// environments (for example, Demo does not imply Paper).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdmissionEnvironment {
    Demo,
    Live,
    Unknown,
    Unsupported,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegistryAvailability {
    Available,
    Unavailable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccountIdentityResolution {
    Unique,
    Missing,
    Ambiguous,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccountControlState {
    Verified,
    Unverified,
    Conflicting,
    Unavailable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdmissionEvidenceAuthentication {
    Authenticated,
    Unauthenticated,
    Unavailable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdmissionEvidenceValidity {
    Valid,
    Invalid,
    Expired,
    RevokedOrSuperseded,
    Conflict,
    Unavailable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VerificationAuthorityState {
    Authorized,
    Unauthorized,
    Unavailable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdmissionLifecycleStatus {
    Active,
    Inactive,
    Missing,
    Unavailable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkerAssignmentResolution {
    Unique,
    Missing,
    Ambiguous,
    Unavailable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdmissionSessionState {
    Ready,
    NotReady,
    Unavailable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdmissionOperation {
    ReadOnlyHistorical,
    ReadOnlyMarketData,
    Other,
}

/// A revision assertion is valid only when both values are present and equal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct AdmissionRevision {
    pub expected: Option<u64>,
    pub observed: Option<u64>,
}

impl AdmissionRevision {
    fn is_consistent(self) -> bool {
        matches!((self.expected, self.observed), (Some(expected), Some(observed)) if expected == observed)
    }
}

/// Opaque identifiers only. Never place broker logins, account numbers,
/// HMAC material, tokens, passwords, or certificate contents in these fields.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct BrokerCatalogAdmissionFacts {
    pub broker_ref: Option<String>,
    pub server_ref: Option<String>,
    pub broker_status: Option<BrokerRegistryStatus>,
    pub server_status: Option<BrokerRegistryStatus>,
    pub environment: Option<AdmissionEnvironment>,
    pub broker_revision: AdmissionRevision,
    pub server_revision: AdmissionRevision,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ExternalAccountAdmissionFacts {
    pub resolution: Option<AccountIdentityResolution>,
    pub account_ref: Option<String>,
    pub control: Option<AccountControlState>,
    pub binding_ref: Option<String>,
    pub binding_status: Option<BrokerBindingStatus>,
    pub account_revision: AdmissionRevision,
    pub binding_revision: AdmissionRevision,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct OrganizationAdmissionFacts {
    pub organization_ref: Option<String>,
    pub organization_status: Option<AdmissionLifecycleStatus>,
    pub membership_ref: Option<String>,
    pub membership_status: Option<AdmissionLifecycleStatus>,
    pub entitlement_ref: Option<String>,
    pub entitlement_status: Option<AdmissionLifecycleStatus>,
    pub organization_revision: AdmissionRevision,
    pub membership_revision: AdmissionRevision,
    pub entitlement_revision: AdmissionRevision,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct EvidenceAdmissionFacts {
    pub evidence_ref: Option<String>,
    pub external_account_ref: Option<String>,
    pub broker_ref: Option<String>,
    pub server_ref: Option<String>,
    pub authentication: Option<AdmissionEvidenceAuthentication>,
    pub validity: Option<AdmissionEvidenceValidity>,
    pub verification_authority: Option<VerificationAuthorityState>,
    pub revision: AdmissionRevision,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct WorkerAdmissionFacts {
    pub worker_ref: Option<String>,
    pub assignment_ref: Option<String>,
    pub assignment_resolution: Option<WorkerAssignmentResolution>,
    pub assigned_binding_ref: Option<String>,
    pub assigned_account_ref: Option<String>,
    pub assigned_broker_ref: Option<String>,
    pub assigned_server_ref: Option<String>,
    pub session_state: Option<AdmissionSessionState>,
    pub revision: AdmissionRevision,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BrokerAdmissionRequest {
    pub request_ref: String,
    pub operation: AdmissionOperation,
    pub organization_ref: String,
    pub account_ref: String,
    pub binding_ref: String,
    pub broker_ref: String,
    pub server_ref: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct BrokerAdmissionFacts {
    pub registry_availability: Option<RegistryAvailability>,
    pub broker: BrokerCatalogAdmissionFacts,
    pub external_account: ExternalAccountAdmissionFacts,
    pub organization: OrganizationAdmissionFacts,
    pub evidence: EvidenceAdmissionFacts,
    pub worker: WorkerAdmissionFacts,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BrokerAdmissionPermit {
    pub request_ref: String,
    pub organization_ref: String,
    pub account_ref: String,
    pub binding_ref: String,
    pub broker_ref: String,
    pub server_ref: String,
    pub worker_ref: String,
    pub assignment_ref: String,
    pub revisions: BrokerAdmissionPermitRevisions,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct BrokerAdmissionPermitRevisions {
    pub broker: u64,
    pub server: u64,
    pub external_account: u64,
    pub binding: u64,
    pub organization: u64,
    pub membership: u64,
    pub entitlement: u64,
    pub evidence: u64,
    pub worker_assignment: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum BrokerAdmissionDenialReason {
    RegistryUnavailable,
    UnknownBroker,
    UnknownServer,
    CatalogInactive,
    EnvironmentUnknownOrUnsupported,
    AccountBindingMissing,
    AccountIdentityAmbiguous,
    AccountControlUnverified,
    OrganizationInactive,
    MembershipMissingOrInactive,
    EntitlementMissingOrInactive,
    EvidenceMissing,
    EvidenceUnauthenticated,
    EvidenceInvalid,
    EvidenceExpired,
    EvidenceRevokedOrSuperseded,
    EvidenceConflict,
    VerificationAuthorityMissingOrUnauthorized,
    WorkerAssignmentMissingOrAmbiguous,
    SessionNotReady,
    SessionIdentityMismatch,
    AccountScopeMismatch,
    UnsupportedOperation,
    RevisionStaleOrInconsistent,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BrokerAdmissionDecision {
    Admitted(BrokerAdmissionPermit),
    Denied(Vec<BrokerAdmissionDenialReason>),
}

/// Evaluate an offline snapshot of admission facts. This function does not
/// authenticate the fact sources, query persistence, or establish broker
/// account ownership. Callers must obtain and authenticate facts separately.
/// Every missing, unavailable, stale, ambiguous, or conflicting assertion
/// fails closed.
pub fn evaluate_broker_admission(
    request: &BrokerAdmissionRequest,
    facts: &BrokerAdmissionFacts,
) -> BrokerAdmissionDecision {
    use BrokerAdmissionDenialReason as Deny;

    let mut denials = Vec::new();
    let mut deny = |reason| {
        if !denials.contains(&reason) {
            denials.push(reason);
        }
    };

    if facts.registry_availability != Some(RegistryAvailability::Available) {
        deny(Deny::RegistryUnavailable);
    }

    let broker = &facts.broker;
    if broker.broker_ref.as_deref().is_none_or(str::is_empty)
        || broker.broker_status.is_none()
    {
        deny(Deny::UnknownBroker);
    } else if broker.broker_status != Some(BrokerRegistryStatus::Active) {
        deny(Deny::CatalogInactive);
    }
    if broker.server_ref.as_deref().is_none_or(str::is_empty)
        || broker.server_status.is_none()
    {
        deny(Deny::UnknownServer);
    } else if broker.server_status != Some(BrokerRegistryStatus::Active) {
        deny(Deny::CatalogInactive);
    }
    if !matches!(broker.environment, Some(AdmissionEnvironment::Demo | AdmissionEnvironment::Live)) {
        deny(Deny::EnvironmentUnknownOrUnsupported);
    }

    let account = &facts.external_account;
    match account.resolution {
        Some(AccountIdentityResolution::Unique) => {
            if account.account_ref.as_deref().is_none_or(str::is_empty) {
                deny(Deny::AccountBindingMissing);
            }
        }
        Some(AccountIdentityResolution::Ambiguous) => deny(Deny::AccountIdentityAmbiguous),
        Some(AccountIdentityResolution::Missing) | None => deny(Deny::AccountBindingMissing),
    }
    if account.binding_ref.as_deref().is_none_or(str::is_empty)
        || account.binding_status != Some(BrokerBindingStatus::Verified)
    {
        deny(Deny::AccountBindingMissing);
    }
    if account.control != Some(AccountControlState::Verified) {
        deny(Deny::AccountControlUnverified);
    }

    let organization = &facts.organization;
    if organization.organization_ref.as_deref() != Some(request.organization_ref.as_str())
        || organization.organization_status != Some(AdmissionLifecycleStatus::Active)
    {
        deny(Deny::OrganizationInactive);
    }
    if organization.membership_ref.as_deref().is_none_or(str::is_empty)
        || organization.membership_status != Some(AdmissionLifecycleStatus::Active)
    {
        deny(Deny::MembershipMissingOrInactive);
    }
    if organization.entitlement_ref.as_deref().is_none_or(str::is_empty)
        || organization.entitlement_status != Some(AdmissionLifecycleStatus::Active)
    {
        deny(Deny::EntitlementMissingOrInactive);
    }

    let evidence = &facts.evidence;
    if evidence.evidence_ref.as_deref().is_none_or(str::is_empty) {
        deny(Deny::EvidenceMissing);
    }
    if evidence.external_account_ref.as_deref() != Some(request.account_ref.as_str())
        || evidence.broker_ref.as_deref() != Some(request.broker_ref.as_str())
        || evidence.server_ref.as_deref() != Some(request.server_ref.as_str())
    {
        deny(Deny::EvidenceConflict);
    }
    match evidence.authentication {
        Some(AdmissionEvidenceAuthentication::Authenticated) => {}
        Some(AdmissionEvidenceAuthentication::Unauthenticated) | None => deny(Deny::EvidenceUnauthenticated),
        Some(AdmissionEvidenceAuthentication::Unavailable) => deny(Deny::EvidenceUnauthenticated),
    }
    match evidence.validity {
        Some(AdmissionEvidenceValidity::Valid) => {}
        Some(AdmissionEvidenceValidity::Invalid) => deny(Deny::EvidenceInvalid),
        Some(AdmissionEvidenceValidity::Expired) => deny(Deny::EvidenceExpired),
        Some(AdmissionEvidenceValidity::RevokedOrSuperseded) => deny(Deny::EvidenceRevokedOrSuperseded),
        Some(AdmissionEvidenceValidity::Conflict) => deny(Deny::EvidenceConflict),
        Some(AdmissionEvidenceValidity::Unavailable) | None => deny(Deny::EvidenceInvalid),
    }
    if evidence.verification_authority != Some(VerificationAuthorityState::Authorized) {
        deny(Deny::VerificationAuthorityMissingOrUnauthorized);
    }

    let worker = &facts.worker;
    match worker.assignment_resolution {
        Some(WorkerAssignmentResolution::Unique) => {}
        Some(WorkerAssignmentResolution::Missing | WorkerAssignmentResolution::Ambiguous)
        | Some(WorkerAssignmentResolution::Unavailable)
        | None => deny(Deny::WorkerAssignmentMissingOrAmbiguous),
    }
    if worker.worker_ref.as_deref().is_none_or(str::is_empty)
        || worker.assignment_ref.as_deref().is_none_or(str::is_empty)
    {
        deny(Deny::WorkerAssignmentMissingOrAmbiguous);
    }
    if worker.session_state != Some(AdmissionSessionState::Ready) {
        deny(Deny::SessionNotReady);
    }

    if worker.assigned_binding_ref.as_deref() != Some(request.binding_ref.as_str())
        || worker.assigned_account_ref.as_deref() != Some(request.account_ref.as_str())
        || worker.assigned_broker_ref.as_deref() != Some(request.broker_ref.as_str())
        || worker.assigned_server_ref.as_deref() != Some(request.server_ref.as_str())
        || account.account_ref.as_deref() != Some(request.account_ref.as_str())
        || account.binding_ref.as_deref() != Some(request.binding_ref.as_str())
        || broker.broker_ref.as_deref() != Some(request.broker_ref.as_str())
        || broker.server_ref.as_deref() != Some(request.server_ref.as_str())
    {
        deny(Deny::SessionIdentityMismatch);
    }
    if organization.organization_ref.as_deref() != Some(request.organization_ref.as_str())
        || organization.entitlement_ref.as_deref().is_none()
    {
        deny(Deny::AccountScopeMismatch);
    }

    if !matches!(
        request.operation,
        AdmissionOperation::ReadOnlyHistorical | AdmissionOperation::ReadOnlyMarketData
    ) {
        deny(Deny::UnsupportedOperation);
    }

    let revisions = [
        broker.broker_revision,
        broker.server_revision,
        account.account_revision,
        account.binding_revision,
        organization.organization_revision,
        organization.membership_revision,
        organization.entitlement_revision,
        evidence.revision,
        worker.revision,
    ];
    if revisions.iter().any(|revision| !revision.is_consistent()) {
        deny(Deny::RevisionStaleOrInconsistent);
    }

    if !denials.is_empty() {
        denials.sort();
        return BrokerAdmissionDecision::Denied(denials);
    }

    // All required fields and revisions were checked above. Keep the permit
    // limited to opaque references and the exact revisions evaluated.
    let permit = BrokerAdmissionPermit {
        request_ref: request.request_ref.clone(),
        organization_ref: request.organization_ref.clone(),
        account_ref: request.account_ref.clone(),
        binding_ref: request.binding_ref.clone(),
        broker_ref: request.broker_ref.clone(),
        server_ref: request.server_ref.clone(),
        worker_ref: worker.worker_ref.clone().unwrap_or_default(),
        assignment_ref: worker.assignment_ref.clone().unwrap_or_default(),
        revisions: BrokerAdmissionPermitRevisions {
            broker: broker.broker_revision.observed.unwrap_or_default(),
            server: broker.server_revision.observed.unwrap_or_default(),
            external_account: account.account_revision.observed.unwrap_or_default(),
            binding: account.binding_revision.observed.unwrap_or_default(),
            organization: organization.organization_revision.observed.unwrap_or_default(),
            membership: organization.membership_revision.observed.unwrap_or_default(),
            entitlement: organization.entitlement_revision.observed.unwrap_or_default(),
            evidence: evidence.revision.observed.unwrap_or_default(),
            worker_assignment: worker.revision.observed.unwrap_or_default(),
        },
    };
    BrokerAdmissionDecision::Admitted(permit)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;


    fn revision(value: u64) -> AdmissionRevision {
        AdmissionRevision { expected: Some(value), observed: Some(value) }
    }

    fn valid_admission_fixture() -> (BrokerAdmissionRequest, BrokerAdmissionFacts) {
        let request = BrokerAdmissionRequest {
            request_ref: "request-opaque".into(),
            operation: AdmissionOperation::ReadOnlyHistorical,
            organization_ref: "org-opaque".into(),
            account_ref: "account-opaque".into(),
            binding_ref: "binding-opaque".into(),
            broker_ref: "broker-opaque".into(),
            server_ref: "server-opaque".into(),
        };
        let facts = BrokerAdmissionFacts {
            registry_availability: Some(RegistryAvailability::Available),
            broker: BrokerCatalogAdmissionFacts {
                broker_ref: Some("broker-opaque".into()),
                server_ref: Some("server-opaque".into()),
                broker_status: Some(BrokerRegistryStatus::Active),
                server_status: Some(BrokerRegistryStatus::Active),
                environment: Some(AdmissionEnvironment::Demo),
                broker_revision: revision(1),
                server_revision: revision(2),
            },
            external_account: ExternalAccountAdmissionFacts {
                resolution: Some(AccountIdentityResolution::Unique),
                account_ref: Some("account-opaque".into()),
                control: Some(AccountControlState::Verified),
                binding_ref: Some("binding-opaque".into()),
                binding_status: Some(BrokerBindingStatus::Verified),
                account_revision: revision(3),
                binding_revision: revision(4),
            },
            organization: OrganizationAdmissionFacts {
                organization_ref: Some("org-opaque".into()),
                organization_status: Some(AdmissionLifecycleStatus::Active),
                membership_ref: Some("membership-opaque".into()),
                membership_status: Some(AdmissionLifecycleStatus::Active),
                entitlement_ref: Some("entitlement-opaque".into()),
                entitlement_status: Some(AdmissionLifecycleStatus::Active),
                organization_revision: revision(5),
                membership_revision: revision(6),
                entitlement_revision: revision(7),
            },
            evidence: EvidenceAdmissionFacts {
                evidence_ref: Some("evidence-opaque".into()),
                external_account_ref: Some("account-opaque".into()),
                broker_ref: Some("broker-opaque".into()),
                server_ref: Some("server-opaque".into()),
                authentication: Some(AdmissionEvidenceAuthentication::Authenticated),
                validity: Some(AdmissionEvidenceValidity::Valid),
                verification_authority: Some(VerificationAuthorityState::Authorized),
                revision: revision(8),
            },
            worker: WorkerAdmissionFacts {
                worker_ref: Some("worker-opaque".into()),
                assignment_ref: Some("assignment-opaque".into()),
                assignment_resolution: Some(WorkerAssignmentResolution::Unique),
                assigned_binding_ref: Some("binding-opaque".into()),
                assigned_account_ref: Some("account-opaque".into()),
                assigned_broker_ref: Some("broker-opaque".into()),
                assigned_server_ref: Some("server-opaque".into()),
                session_state: Some(AdmissionSessionState::Ready),
                revision: revision(9),
            },
        };
        (request, facts)
    }

    fn denial_reasons(decision: BrokerAdmissionDecision) -> Vec<BrokerAdmissionDenialReason> {
        match decision {
            BrokerAdmissionDecision::Denied(reasons) => reasons,
            BrokerAdmissionDecision::Admitted(_) => Vec::new(),
        }
    }

    #[test]
    fn admission_evaluator_admits_only_matching_read_only_scope() {
        let (request, facts) = valid_admission_fixture();
        let decision = evaluate_broker_admission(&request, &facts);
        let permit = match decision {
            BrokerAdmissionDecision::Admitted(permit) => permit,
            BrokerAdmissionDecision::Denied(reasons) => {
                assert!(reasons.is_empty(), "unexpected denial reasons: {reasons:?}");
                return;
            }
        };
        assert_eq!(permit.request_ref, request.request_ref);
        assert_eq!(permit.account_ref, request.account_ref);
        assert_eq!(permit.revisions.worker_assignment, 9);
    }

    #[test]
    fn admission_evaluator_fails_closed_for_unavailable_registry_and_unknown_catalog() {
        let (request, mut facts) = valid_admission_fixture();
        facts.registry_availability = Some(RegistryAvailability::Unavailable);
        facts.broker.broker_ref = None;
        facts.broker.server_ref = None;
        let reasons = denial_reasons(evaluate_broker_admission(&request, &facts));
        assert!(reasons.contains(&BrokerAdmissionDenialReason::RegistryUnavailable));
        assert!(reasons.contains(&BrokerAdmissionDenialReason::UnknownBroker));
        assert!(reasons.contains(&BrokerAdmissionDenialReason::UnknownServer));
    }

    #[test]
    fn admission_evaluator_rejects_ambiguous_identity_and_unverified_control() {
        let (request, mut facts) = valid_admission_fixture();
        facts.external_account.resolution = Some(AccountIdentityResolution::Ambiguous);
        facts.external_account.control = Some(AccountControlState::Unverified);
        let reasons = denial_reasons(evaluate_broker_admission(&request, &facts));
        assert!(reasons.contains(&BrokerAdmissionDenialReason::AccountIdentityAmbiguous));
        assert!(reasons.contains(&BrokerAdmissionDenialReason::AccountControlUnverified));
    }

    #[test]
    fn admission_evaluator_rejects_missing_binding_membership_and_entitlement() {
        let (request, mut facts) = valid_admission_fixture();
        facts.external_account.binding_status = Some(BrokerBindingStatus::PendingApproval);
        facts.organization.membership_status = Some(AdmissionLifecycleStatus::Missing);
        facts.organization.entitlement_status = Some(AdmissionLifecycleStatus::Inactive);
        let reasons = denial_reasons(evaluate_broker_admission(&request, &facts));
        assert!(reasons.contains(&BrokerAdmissionDenialReason::AccountBindingMissing));
        assert!(reasons.contains(&BrokerAdmissionDenialReason::MembershipMissingOrInactive));
        assert!(reasons.contains(&BrokerAdmissionDenialReason::EntitlementMissingOrInactive));
    }

    #[test]
    fn admission_evaluator_rejects_untrusted_or_stale_evidence() {
        let (request, mut facts) = valid_admission_fixture();
        facts.evidence.authentication = Some(AdmissionEvidenceAuthentication::Unauthenticated);
        facts.evidence.validity = Some(AdmissionEvidenceValidity::Expired);
        facts.evidence.verification_authority = Some(VerificationAuthorityState::Unauthorized);
        let reasons = denial_reasons(evaluate_broker_admission(&request, &facts));
        assert!(reasons.contains(&BrokerAdmissionDenialReason::EvidenceUnauthenticated));
        assert!(reasons.contains(&BrokerAdmissionDenialReason::EvidenceExpired));
        assert!(reasons.contains(&BrokerAdmissionDenialReason::VerificationAuthorityMissingOrUnauthorized));
    }

    #[test]
    fn admission_evaluator_rejects_revoked_conflicting_and_invalid_evidence() {
        let (request, mut facts) = valid_admission_fixture();
        facts.evidence.validity = Some(AdmissionEvidenceValidity::RevokedOrSuperseded);
        let reasons = denial_reasons(evaluate_broker_admission(&request, &facts));
        assert!(reasons.contains(&BrokerAdmissionDenialReason::EvidenceRevokedOrSuperseded));
        facts.evidence.validity = Some(AdmissionEvidenceValidity::Conflict);
        let reasons = denial_reasons(evaluate_broker_admission(&request, &facts));
        assert!(reasons.contains(&BrokerAdmissionDenialReason::EvidenceConflict));
        facts.evidence.validity = Some(AdmissionEvidenceValidity::Invalid);
        let reasons = denial_reasons(evaluate_broker_admission(&request, &facts));
        assert!(reasons.contains(&BrokerAdmissionDenialReason::EvidenceInvalid));
    }

    #[test]
    fn admission_evaluator_rejects_missing_worker_assignment_and_identity_mismatch() {
        let (request, mut facts) = valid_admission_fixture();
        facts.worker.assignment_resolution = Some(WorkerAssignmentResolution::Missing);
        facts.worker.session_state = Some(AdmissionSessionState::NotReady);
        let reasons = denial_reasons(evaluate_broker_admission(&request, &facts));
        assert!(reasons.contains(&BrokerAdmissionDenialReason::WorkerAssignmentMissingOrAmbiguous));
        assert!(reasons.contains(&BrokerAdmissionDenialReason::SessionNotReady));

        facts.worker.assignment_resolution = Some(WorkerAssignmentResolution::Unique);
        facts.worker.session_state = Some(AdmissionSessionState::Ready);
        facts.worker.assigned_account_ref = Some("different-account".into());
        let reasons = denial_reasons(evaluate_broker_admission(&request, &facts));
        assert!(reasons.contains(&BrokerAdmissionDenialReason::SessionIdentityMismatch));
    }

    #[test]
    fn admission_evaluator_rejects_unknown_environment_unsupported_operation_and_stale_revision() {
        let (request, mut facts) = valid_admission_fixture();
        facts.broker.environment = Some(AdmissionEnvironment::Unknown);
        facts.evidence.revision.observed = Some(99);
        let reasons = denial_reasons(evaluate_broker_admission(&request, &facts));
        assert!(reasons.contains(&BrokerAdmissionDenialReason::EnvironmentUnknownOrUnsupported));
        assert!(reasons.contains(&BrokerAdmissionDenialReason::RevisionStaleOrInconsistent));

        let (mut request, facts) = valid_admission_fixture();
        request.operation = AdmissionOperation::Other;
        let reasons = denial_reasons(evaluate_broker_admission(&request, &facts));
        assert!(reasons.contains(&BrokerAdmissionDenialReason::UnsupportedOperation));
    }

    #[test]
    fn admission_denials_do_not_echo_raw_account_identifiers() {
        let (request, mut facts) = valid_admission_fixture();
        facts.external_account.account_ref = Some("raw-login-must-not-appear".into());
        let decision = evaluate_broker_admission(&request, &facts);
        let debug = format!("{decision:?}");
        assert!(!debug.contains("raw-login-must-not-appear"));
        assert!(matches!(decision, BrokerAdmissionDecision::Denied(_)));
    }

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

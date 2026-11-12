//! Execution environment and fail-closed broker authorization policy.
//!
//! These types intentionally do not mint execution grants. Trusted account
//! provenance and durable single-use grants require the QOS-021A-E2 schema.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Operating environment is independent of [`crate::AccountType`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AccountEnvironment {
    Paper,
    Shadow,
    Live,
}

/// Provenance attached to a persisted environment classification.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum EnvironmentVerification {
    /// No authoritative environment has been assigned.
    Unclassified,
    /// A legacy value exists but its origin has not been verified.
    UnverifiedLegacy,
    /// Explicit environment supplied for a new account, awaiting verification.
    Unverified,
    /// An operator or approved workflow verified this environment revision.
    Verified {
        revision: u64,
        verified_at: DateTime<Utc>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum BrokerAuthorizationError {
    #[error("account environment is unclassified")]
    Unclassified,
    #[error("legacy account environment provenance is unverified")]
    Unverified,
    #[error("broker submission requires a verified LIVE environment")]
    NotVerifiedLive,
    #[error("environment verification revision is invalid")]
    InvalidRevision,
    #[error("environment verification is from the future")]
    FutureVerification,
}

/// Check the environment portion of broker authorization. Passing this check
/// is necessary but never sufficient: risk and execution authorization remain
/// separate requirements.
pub fn require_verified_live_environment(
    environment: Option<AccountEnvironment>,
    verification: EnvironmentVerification,
    now: DateTime<Utc>,
) -> Result<u64, BrokerAuthorizationError> {
    let environment = environment.ok_or(BrokerAuthorizationError::Unclassified)?;
    let (revision, verified_at) = match verification {
        EnvironmentVerification::Unclassified => {
            return Err(BrokerAuthorizationError::Unclassified);
        }
        EnvironmentVerification::UnverifiedLegacy => {
            return Err(BrokerAuthorizationError::Unverified);
        }
        EnvironmentVerification::Unverified => {
            return Err(BrokerAuthorizationError::Unverified);
        }
        EnvironmentVerification::Verified {
            revision,
            verified_at,
        } => (revision, verified_at),
    };

    if revision == 0 {
        return Err(BrokerAuthorizationError::InvalidRevision);
    }
    if verified_at > now {
        return Err(BrokerAuthorizationError::FutureVerification);
    }
    if environment != AccountEnvironment::Live {
        return Err(BrokerAuthorizationError::NotVerifiedLive);
    }
    Ok(revision)
}

/// Validates persisted environment provenance against the account's current revision.
pub fn require_account_broker_eligibility(
    environment: Option<AccountEnvironment>,
    verification: EnvironmentVerification,
    current_revision: u64,
    verified_by: Option<&str>,
    now: DateTime<Utc>,
) -> Result<u64, BrokerAuthorizationError> {
    let revision = require_verified_live_environment(environment, verification, now)?;
    if revision != current_revision || verified_by.is_none_or(|actor| actor.trim().is_empty()) {
        return Err(BrokerAuthorizationError::InvalidRevision);
    }
    Ok(revision)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;

    #[test]
    fn account_environment_is_a_distinct_authorization_dimension() {
        let now = Utc::now();
        assert_eq!(
            require_verified_live_environment(
                Some(AccountEnvironment::Live),
                EnvironmentVerification::Verified {
                    revision: 4,
                    verified_at: now,
                },
                now,
            ),
            Ok(4)
        );
    }

    #[test]
    fn unclassified_legacy_paper_and_shadow_are_not_broker_authorized() {
        let now = Utc::now();
        assert_eq!(
            require_verified_live_environment(None, EnvironmentVerification::Unclassified, now),
            Err(BrokerAuthorizationError::Unclassified)
        );
        assert_eq!(
            require_verified_live_environment(
                Some(AccountEnvironment::Paper),
                EnvironmentVerification::UnverifiedLegacy,
                now
            ),
            Err(BrokerAuthorizationError::Unverified)
        );
        assert_eq!(
            require_verified_live_environment(
                Some(AccountEnvironment::Paper),
                EnvironmentVerification::Verified {
                    revision: 1,
                    verified_at: now,
                },
                now
            ),
            Err(BrokerAuthorizationError::NotVerifiedLive)
        );
        assert_eq!(
            require_verified_live_environment(
                Some(AccountEnvironment::Shadow),
                EnvironmentVerification::Verified {
                    revision: 1,
                    verified_at: now,
                },
                now
            ),
            Err(BrokerAuthorizationError::NotVerifiedLive)
        );
        assert_eq!(
            require_verified_live_environment(
                Some(AccountEnvironment::Live),
                EnvironmentVerification::UnverifiedLegacy,
                now
            ),
            Err(BrokerAuthorizationError::Unverified)
        );
    }

    #[test]
    fn verified_live_requires_valid_current_revision() {
        let now = Utc::now();
        assert_eq!(
            require_verified_live_environment(
                Some(AccountEnvironment::Live),
                EnvironmentVerification::Verified {
                    revision: 0,
                    verified_at: now,
                },
                now
            ),
            Err(BrokerAuthorizationError::InvalidRevision)
        );
        assert_eq!(
            require_verified_live_environment(
                Some(AccountEnvironment::Live),
                EnvironmentVerification::Verified {
                    revision: 1,
                    verified_at: now + Duration::seconds(1),
                },
                now
            ),
            Err(BrokerAuthorizationError::FutureVerification)
        );
        assert_eq!(
            require_verified_live_environment(
                Some(AccountEnvironment::Live),
                EnvironmentVerification::Verified {
                    revision: 3,
                    verified_at: now,
                },
                now
            ),
            Ok(3)
        );
    }
}

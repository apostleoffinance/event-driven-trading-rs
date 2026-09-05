//! Strongly-typed identifiers for trading entities.
//!
//! IDs are opaque validated strings. Empty / whitespace-only values are rejected.

use std::fmt;

use serde::{Deserialize, Serialize};

use crate::error::{DomainError, DomainResult};

macro_rules! define_id {
    ($(#[$meta:meta])* $name:ident) => {
        $(#[$meta])*
        #[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
        pub struct $name(String);

        impl $name {
            /// Construct a validated identifier.
            pub fn new(value: impl Into<String>) -> DomainResult<Self> {
                let value = value.into();
                let trimmed = value.trim();
                if trimmed.is_empty() {
                    return Err(DomainError::InvalidId {
                        id: value,
                        reason: "identifier must be non-empty".to_string(),
                    });
                }
                if trimmed.len() > 128 {
                    return Err(DomainError::InvalidId {
                        id: value,
                        reason: "identifier exceeds 128 characters".to_string(),
                    });
                }
                Ok(Self(trimmed.to_string()))
            }

            /// Borrow the underlying string.
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(&self.0)
            }
        }

        impl AsRef<str> for $name {
            fn as_ref(&self) -> &str {
                &self.0
            }
        }
    };
}

define_id!(
    /// Unique account identifier (capital / risk ownership).
    AccountId
);
define_id!(
    /// Unique venue identifier (where execution occurs).
    VenueId
);
define_id!(
    /// Unique instrument identifier.
    InstrumentId
);
define_id!(
    /// Logical strategy identifier (independent of account/venue).
    StrategyId
);
define_id!(
    /// Strategy version string (e.g. `1.0.0` or `v1`).
    StrategyVersion
);
define_id!(
    /// Binding of a strategy version to an account + risk profile.
    DeploymentId
);
define_id!(
    /// Risk profile / policy identifier.
    RiskProfileId
);
define_id!(
    /// Trade intent identifier emitted by a strategy deployment.
    TradeIntentId
);
define_id!(
    /// Internal order identifier.
    OrderId
);
define_id!(
    /// Client-supplied idempotency key for live-capable orders.
    ClientOrderId
);
define_id!(
    /// Fill / execution report identifier.
    FillId
);
define_id!(
    /// Position identifier.
    PositionId
);

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;

    #[test]
    fn rejects_empty_id() {
        assert!(AccountId::new("").is_err());
        assert!(AccountId::new("   ").is_err());
    }

    #[test]
    fn accepts_valid_id() {
        let id = AccountId::new("prop-account-001").unwrap();
        assert_eq!(id.as_str(), "prop-account-001");
    }
}

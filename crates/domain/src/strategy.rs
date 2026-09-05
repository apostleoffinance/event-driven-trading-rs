//! Strategy and deployment domain.
//!
//! A strategy is independent of account, venue, and capital allocation.
//! [`StrategyDeployment`] binds a strategy version to an account + risk profile.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::error::{DomainError, DomainResult};
use crate::ids::{AccountId, DeploymentId, RiskProfileId, StrategyId, StrategyVersion};

/// Strategy identity metadata (not the executable strategy logic).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StrategyMetadata {
    pub id: StrategyId,
    pub version: StrategyVersion,
    pub name: String,
}

impl StrategyMetadata {
    pub fn new(
        id: StrategyId,
        version: StrategyVersion,
        name: impl Into<String>,
    ) -> DomainResult<Self> {
        let name = name.into();
        if name.trim().is_empty() {
            return Err(DomainError::Validation(
                "strategy name must be non-empty".to_string(),
            ));
        }
        Ok(Self { id, version, name })
    }
}

/// Alias retained for readability in architecture docs.
pub type Strategy = StrategyMetadata;

/// Deployment binds one strategy version to one account under one risk profile.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StrategyDeployment {
    pub id: DeploymentId,
    pub strategy_id: StrategyId,
    pub strategy_version: StrategyVersion,
    pub account_id: AccountId,
    pub risk_profile_id: RiskProfileId,
    pub enabled: bool,
    pub created_at: DateTime<Utc>,
}

impl StrategyDeployment {
    pub fn new(
        id: DeploymentId,
        strategy_id: StrategyId,
        strategy_version: StrategyVersion,
        account_id: AccountId,
        risk_profile_id: RiskProfileId,
        created_at: DateTime<Utc>,
    ) -> Self {
        Self {
            id,
            strategy_id,
            strategy_version,
            account_id,
            risk_profile_id,
            enabled: true,
            created_at,
        }
    }

    pub fn disable(&mut self) {
        self.enabled = false;
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;

    #[test]
    fn deployment_binds_strategy_to_account() {
        let d = StrategyDeployment::new(
            DeploymentId::new("deployment-001").unwrap(),
            StrategyId::new("btc-mean-reversion").unwrap(),
            StrategyVersion::new("v1").unwrap(),
            AccountId::new("prop-account-001").unwrap(),
            RiskProfileId::new("balanced").unwrap(),
            Utc::now(),
        );
        assert!(d.enabled);
        assert_eq!(d.account_id.as_str(), "prop-account-001");
    }
}

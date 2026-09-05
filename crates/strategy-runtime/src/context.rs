//! Deployment-scoped context supplied by the runtime (not owned by strategy logic).

use domain::{DeploymentId, StrategyId, StrategyVersion};

/// Binding for a live strategy deployment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StrategyContext {
    pub strategy_id: StrategyId,
    pub strategy_version: StrategyVersion,
    pub deployment_id: DeploymentId,
}

impl StrategyContext {
    pub fn new(
        strategy_id: StrategyId,
        strategy_version: StrategyVersion,
        deployment_id: DeploymentId,
    ) -> Self {
        Self {
            strategy_id,
            strategy_version,
            deployment_id,
        }
    }
}

//! Independently testable pre-trade risk rules.
//!
//! Evaluation order is defined by [`super::evaluator::DefaultRiskEvaluator`].

mod daily_loss;
mod drawdown;
mod halt;
mod instrument;
mod position_limit;
mod session;

pub use daily_loss::check_daily_loss;
pub use drawdown::check_drawdown;
pub use halt::{check_account_halt, check_global_halt, check_strategy_halt};
pub use instrument::check_instrument;
pub use position_limit::check_position_limit;
pub use session::check_session;

use domain::RiskRejectReason;

/// Outcome of a single gate rule (before quantity sizing).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuleOutcome {
    Pass,
    Halt { reason: String },
    Reject { reason: RiskRejectReason },
}

impl RuleOutcome {
    pub fn is_pass(&self) -> bool {
        matches!(self, Self::Pass)
    }
}

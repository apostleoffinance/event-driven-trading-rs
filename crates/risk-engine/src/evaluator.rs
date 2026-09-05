//! Risk evaluator interface and default deterministic pipeline.

use domain::{RiskDecision, RiskRequest};

use crate::error::RiskEngineResult;
use crate::rules::{
    check_account_halt, check_daily_loss, check_drawdown, check_global_halt, check_instrument,
    check_position_limit, check_session, check_strategy_halt, RuleOutcome,
};
use crate::sizing::size_order;

/// Pure risk evaluation boundary.
pub trait RiskEvaluator {
    fn evaluate(&self, request: &RiskRequest) -> RiskEngineResult<RiskDecision>;
}

/// Default evaluator: ordered gate rules, then quantity sizing.
#[derive(Debug, Default, Clone, Copy)]
pub struct DefaultRiskEvaluator;

impl DefaultRiskEvaluator {
    pub fn new() -> Self {
        Self
    }

    /// Explicit evaluation order (independently testable gates first).
    fn run_gates(request: &RiskRequest) -> RuleOutcome {
        let checks = [
            check_global_halt,
            check_account_halt,
            check_strategy_halt,
            check_instrument,
            check_session,
            check_drawdown,
            check_daily_loss,
            check_position_limit,
        ];

        for check in checks {
            let outcome = check(request);
            if !outcome.is_pass() {
                return outcome;
            }
        }
        RuleOutcome::Pass
    }
}

impl RiskEvaluator for DefaultRiskEvaluator {
    fn evaluate(&self, request: &RiskRequest) -> RiskEngineResult<RiskDecision> {
        tracing::debug!(
            trade_intent_id = %request.trade_intent_id,
            account_id = %request.account_id,
            "evaluating risk request"
        );

        match Self::run_gates(request) {
            RuleOutcome::Pass => size_order(request),
            RuleOutcome::Halt { reason } => Ok(RiskDecision::Halted { reason }),
            RuleOutcome::Reject { reason } => Ok(RiskDecision::Rejected { reason }),
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;
    use crate::test_support::sample_request;
    use domain::RiskRejectReason;
    use rust_decimal::Decimal;

    #[test]
    fn determinism_same_input_same_output() {
        let evaluator = DefaultRiskEvaluator::new();
        let req = sample_request();
        let a = evaluator.evaluate(&req).unwrap();
        let b = evaluator.evaluate(&req).unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn kill_switch_prevents_sizing() {
        let evaluator = DefaultRiskEvaluator::new();
        let mut req = sample_request();
        req.policy.global_kill_switch = true;
        let decision = evaluator.evaluate(&req).unwrap();
        assert!(matches!(decision, RiskDecision::Halted { .. }));
        assert!(!decision.is_executable());
    }

    #[test]
    fn approves_valid_request() {
        let evaluator = DefaultRiskEvaluator::new();
        let mut req = sample_request();
        req.requested_quantity = Some(Decimal::ONE);
        req.entry_price = Some(Decimal::from(100));
        req.stop_loss = Some(Decimal::from(98));
        let decision = evaluator.evaluate(&req).unwrap();
        assert!(decision.is_executable());
    }

    #[test]
    fn rejects_drawdown_before_sizing() {
        let evaluator = DefaultRiskEvaluator::new();
        let mut req = sample_request();
        req.policy.max_drawdown_pct = Decimal::from(5);
        req.account_state.peak_equity = Decimal::from(100);
        req.account_state.equity = Decimal::from(90);
        let decision = evaluator.evaluate(&req).unwrap();
        assert_eq!(
            decision,
            RiskDecision::Rejected {
                reason: RiskRejectReason::DrawdownBreached
            }
        );
    }
}

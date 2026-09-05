use domain::RiskRequest;

use super::RuleOutcome;

/// 1. Global kill switch.
pub fn check_global_halt(request: &RiskRequest) -> RuleOutcome {
    if request.policy.global_kill_switch {
        RuleOutcome::Halt {
            reason: "global kill switch active".to_string(),
        }
    } else {
        RuleOutcome::Pass
    }
}

/// 2. Account kill switch (policy flag or live account state).
pub fn check_account_halt(request: &RiskRequest) -> RuleOutcome {
    if request.policy.account_kill_switch || request.account_state.kill_switch {
        RuleOutcome::Halt {
            reason: format!(
                "account kill switch active for {}",
                request.account_id.as_str()
            ),
        }
    } else {
        RuleOutcome::Pass
    }
}

/// 3. Strategy kill switch.
pub fn check_strategy_halt(request: &RiskRequest) -> RuleOutcome {
    if request.policy.strategy_kill_switch {
        RuleOutcome::Halt {
            reason: format!(
                "strategy kill switch active for {}",
                request.strategy_id.as_str()
            ),
        }
    } else {
        RuleOutcome::Pass
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;
    use crate::test_support::sample_request;

    #[test]
    fn global_halt() {
        let mut req = sample_request();
        req.policy.global_kill_switch = true;
        assert!(matches!(check_global_halt(&req), RuleOutcome::Halt { .. }));
    }

    #[test]
    fn account_halt_from_state() {
        let mut req = sample_request();
        req.account_state.kill_switch = true;
        assert!(matches!(check_account_halt(&req), RuleOutcome::Halt { .. }));
    }

    #[test]
    fn strategy_halt() {
        let mut req = sample_request();
        req.policy.strategy_kill_switch = true;
        assert!(matches!(
            check_strategy_halt(&req),
            RuleOutcome::Halt { .. }
        ));
    }
}

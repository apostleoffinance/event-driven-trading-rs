use domain::{RiskRejectReason, RiskRequest};

use super::RuleOutcome;

/// 9. Max open positions (checked before sizing a new position).
pub fn check_position_limit(request: &RiskRequest) -> RuleOutcome {
    if request.account_state.open_position_count >= request.policy.max_open_positions {
        RuleOutcome::Reject {
            reason: RiskRejectReason::PositionLimit,
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
    fn rejects_at_position_cap() {
        let mut req = sample_request();
        req.policy.max_open_positions = 2;
        req.account_state.open_position_count = 2;
        assert_eq!(
            check_position_limit(&req),
            RuleOutcome::Reject {
                reason: RiskRejectReason::PositionLimit
            }
        );
    }
}

use domain::{RiskRejectReason, RiskRequest};

use super::RuleOutcome;

/// 5. Trading session gate.
pub fn check_session(request: &RiskRequest) -> RuleOutcome {
    if request.session_allowed {
        RuleOutcome::Pass
    } else {
        RuleOutcome::Reject {
            reason: RiskRejectReason::SessionNotAllowed,
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;
    use crate::test_support::sample_request;

    #[test]
    fn rejects_closed_session() {
        let req = sample_request().with_session_allowed(false);
        assert_eq!(
            check_session(&req),
            RuleOutcome::Reject {
                reason: RiskRejectReason::SessionNotAllowed
            }
        );
    }
}

use domain::{RiskRejectReason, RiskRequest};

use super::RuleOutcome;

/// 6. Existing drawdown breach.
pub fn check_drawdown(request: &RiskRequest) -> RuleOutcome {
    match request.account_state.drawdown_pct() {
        Ok(dd) if dd >= request.policy.max_drawdown_pct => RuleOutcome::Reject {
            reason: RiskRejectReason::DrawdownBreached,
        },
        Ok(_) => RuleOutcome::Pass,
        Err(_) => RuleOutcome::Reject {
            reason: RiskRejectReason::InvalidRequest,
        },
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;
    use crate::test_support::sample_request;
    use rust_decimal::Decimal;

    #[test]
    fn rejects_when_drawdown_breached() {
        let mut req = sample_request();
        req.policy.max_drawdown_pct = Decimal::from(10);
        req.account_state.peak_equity = Decimal::from(100);
        req.account_state.equity = Decimal::from(85); // 15% DD
        assert_eq!(
            check_drawdown(&req),
            RuleOutcome::Reject {
                reason: RiskRejectReason::DrawdownBreached
            }
        );
    }

    #[test]
    fn passes_within_drawdown() {
        let mut req = sample_request();
        req.policy.max_drawdown_pct = Decimal::from(10);
        req.account_state.peak_equity = Decimal::from(100);
        req.account_state.equity = Decimal::from(95);
        assert_eq!(check_drawdown(&req), RuleOutcome::Pass);
    }
}

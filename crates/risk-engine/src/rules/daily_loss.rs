use domain::{RiskRejectReason, RiskRequest};
use rust_decimal::Decimal;

use super::RuleOutcome;

/// 7. Daily loss breach (realized daily PnL vs equity).
pub fn check_daily_loss(request: &RiskRequest) -> RuleOutcome {
    let equity = request.account_state.equity;
    if equity <= Decimal::ZERO {
        return RuleOutcome::Reject {
            reason: RiskRejectReason::InvalidRequest,
        };
    }

    if request.account_state.daily_realized_pnl >= Decimal::ZERO {
        return RuleOutcome::Pass;
    }

    let loss = -request.account_state.daily_realized_pnl;
    let loss_pct = (loss / equity * Decimal::from(100)).round_dp(8);
    if loss_pct >= request.policy.max_daily_loss_pct {
        RuleOutcome::Reject {
            reason: RiskRejectReason::DailyLossBreached,
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
    use rust_decimal::Decimal;

    #[test]
    fn rejects_daily_loss_breach() {
        let mut req = sample_request();
        req.policy.max_daily_loss_pct = Decimal::from(5);
        req.account_state.equity = Decimal::from(100);
        req.account_state.daily_realized_pnl = Decimal::from(-6);
        assert_eq!(
            check_daily_loss(&req),
            RuleOutcome::Reject {
                reason: RiskRejectReason::DailyLossBreached
            }
        );
    }
}

use domain::{RiskRejectReason, RiskRequest};

use super::RuleOutcome;

/// 4. Instrument allow/restrict lists.
pub fn check_instrument(request: &RiskRequest) -> RuleOutcome {
    if request
        .policy
        .restricted_instruments
        .iter()
        .any(|id| id == &request.instrument_id)
    {
        return RuleOutcome::Reject {
            reason: RiskRejectReason::InstrumentRestricted,
        };
    }

    if let Some(allowed) = &request.policy.allowed_instruments {
        if !allowed.iter().any(|id| id == &request.instrument_id) {
            return RuleOutcome::Reject {
                reason: RiskRejectReason::InstrumentRestricted,
            };
        }
    }

    RuleOutcome::Pass
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;
    use crate::test_support::sample_request;
    use domain::InstrumentId;

    #[test]
    fn rejects_restricted_instrument() {
        let mut req = sample_request();
        req.policy
            .restricted_instruments
            .push(InstrumentId::new("BTCUSDT").unwrap());
        assert_eq!(
            check_instrument(&req),
            RuleOutcome::Reject {
                reason: RiskRejectReason::InstrumentRestricted
            }
        );
    }

    #[test]
    fn rejects_outside_allow_list() {
        let mut req = sample_request();
        req.policy.allowed_instruments = Some(vec![InstrumentId::new("ETHUSDT").unwrap()]);
        assert_eq!(
            check_instrument(&req),
            RuleOutcome::Reject {
                reason: RiskRejectReason::InstrumentRestricted
            }
        );
    }
}

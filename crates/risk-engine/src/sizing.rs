//! Quantity sizing and exposure / leverage / trade-risk caps.
//!
//! Steps 8, 10, 11, 12 from the risk evaluation order.

use domain::{RiskDecision, RiskRejectReason, RiskRequest};
use rust_decimal::Decimal;

use crate::error::{RiskEngineError, RiskEngineResult};

#[derive(Debug, Clone, PartialEq, Eq)]
struct SizeCaps {
    desired: Decimal,
    max_by_position: Decimal,
    max_by_exposure: Decimal,
    max_by_leverage: Decimal,
    max_by_trade_risk: Option<Decimal>,
}

/// Compute an approved or resized quantity, or reject if nothing is allowed.
pub fn size_order(request: &RiskRequest) -> RiskEngineResult<RiskDecision> {
    let entry = match request.entry_price {
        Some(price) if price > Decimal::ZERO => price,
        _ => {
            return Ok(RiskDecision::Rejected {
                reason: RiskRejectReason::InvalidRequest,
            });
        }
    };

    let equity = request.account_state.equity;
    if equity <= Decimal::ZERO {
        return Ok(RiskDecision::Rejected {
            reason: RiskRejectReason::InvalidRequest,
        });
    }

    let desired = resolve_desired_quantity(request, entry, equity)?;
    if desired <= Decimal::ZERO {
        return Ok(RiskDecision::Rejected {
            reason: RiskRejectReason::TradeRiskLimit,
        });
    }

    let caps = compute_caps(request, entry, equity, desired)?;
    let mut allowed = caps
        .desired
        .min(caps.max_by_position)
        .min(caps.max_by_exposure)
        .min(caps.max_by_leverage);
    if let Some(risk_cap) = caps.max_by_trade_risk {
        allowed = allowed.min(risk_cap);
    }

    allowed = allowed.round_dp(8);

    if allowed <= Decimal::ZERO {
        let reason = if caps.max_by_exposure <= Decimal::ZERO {
            RiskRejectReason::ExposureLimit
        } else if caps.max_by_leverage <= Decimal::ZERO {
            RiskRejectReason::LeverageLimit
        } else {
            RiskRejectReason::TradeRiskLimit
        };
        return Ok(RiskDecision::Rejected { reason });
    }

    let explicit_request =
        request.requested_quantity.is_some() || request.requested_notional.is_some();

    if explicit_request && allowed < desired {
        let reason = resize_reason(&caps, allowed);
        return Ok(RiskDecision::Resized {
            quantity: allowed,
            reason,
        });
    }

    Ok(RiskDecision::Approved { quantity: allowed })
}

fn resolve_desired_quantity(
    request: &RiskRequest,
    entry: Decimal,
    equity: Decimal,
) -> RiskEngineResult<Decimal> {
    if let Some(qty) = request.requested_quantity {
        if qty <= Decimal::ZERO {
            return Err(RiskEngineError::Evaluation(
                "requested_quantity must be positive".to_string(),
            ));
        }
        return Ok(qty);
    }

    if let Some(notional) = request.requested_notional {
        if notional <= Decimal::ZERO {
            return Err(RiskEngineError::Evaluation(
                "requested_notional must be positive".to_string(),
            ));
        }
        return Ok((notional / entry).round_dp(8));
    }

    // Strategy did not size — use max position size as the starting desire.
    let max_notional = equity * request.policy.max_position_size_pct / Decimal::from(100);
    Ok((max_notional / entry).round_dp(8))
}

fn compute_caps(
    request: &RiskRequest,
    entry: Decimal,
    equity: Decimal,
    desired: Decimal,
) -> RiskEngineResult<SizeCaps> {
    let max_position_notional = equity * request.policy.max_position_size_pct / Decimal::from(100);
    let max_by_position = if entry > Decimal::ZERO {
        (max_position_notional / entry).round_dp(8)
    } else {
        Decimal::ZERO
    };

    let max_exposure_notional = equity * request.policy.max_total_exposure_pct / Decimal::from(100);
    let remaining_exposure = (max_exposure_notional - request.current_exposure).max(Decimal::ZERO);
    let max_by_exposure = (remaining_exposure / entry).round_dp(8);

    let max_levered_notional = equity * request.policy.max_leverage;
    let remaining_levered = (max_levered_notional - request.current_exposure).max(Decimal::ZERO);
    let max_by_leverage = (remaining_levered / entry).round_dp(8);

    let max_by_trade_risk = trade_risk_cap(request, entry, equity)?;

    Ok(SizeCaps {
        desired,
        max_by_position,
        max_by_exposure,
        max_by_leverage,
        max_by_trade_risk,
    })
}

fn trade_risk_cap(
    request: &RiskRequest,
    entry: Decimal,
    equity: Decimal,
) -> RiskEngineResult<Option<Decimal>> {
    let Some(stop) = request.stop_loss else {
        return Ok(None);
    };
    if stop <= Decimal::ZERO {
        return Ok(Some(Decimal::ZERO));
    }
    let stop_distance = (entry - stop).abs();
    if stop_distance <= Decimal::ZERO {
        return Ok(Some(Decimal::ZERO));
    }
    let max_loss = equity * request.policy.max_risk_per_trade_pct / Decimal::from(100);
    Ok(Some((max_loss / stop_distance).round_dp(8)))
}

fn resize_reason(caps: &SizeCaps, allowed: Decimal) -> String {
    if allowed == caps.max_by_exposure {
        "resized to remaining exposure capacity".to_string()
    } else if allowed == caps.max_by_leverage {
        "resized to leverage limit".to_string()
    } else if caps.max_by_trade_risk.is_some_and(|cap| allowed == cap) {
        "resized to max trade risk".to_string()
    } else if allowed == caps.max_by_position {
        "resized to max position size".to_string()
    } else {
        "resized by risk engine".to_string()
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::panic)]

    use super::*;
    use crate::test_support::sample_request;
    use rust_decimal::Decimal;

    #[test]
    fn approves_policy_sized_quantity_without_explicit_request() {
        let mut req = sample_request();
        req.requested_quantity = None;
        req.requested_notional = None;
        req.entry_price = Some(Decimal::from(100));
        req.stop_loss = None;
        let decision = size_order(&req).unwrap();
        assert!(matches!(decision, RiskDecision::Approved { .. }));
        assert!(decision.approved_quantity().unwrap() > Decimal::ZERO);
    }

    #[test]
    fn resizes_when_explicit_quantity_exceeds_position_cap() {
        let mut req = sample_request();
        req.account_state.equity = Decimal::from(10_000);
        req.policy.max_position_size_pct = Decimal::from(1); // 1% = 100 notional
        req.entry_price = Some(Decimal::from(100));
        req.requested_quantity = Some(Decimal::from(50)); // 5000 notional
        req.stop_loss = None;
        let decision = size_order(&req).unwrap();
        match decision {
            RiskDecision::Resized { quantity, .. } => {
                assert_eq!(quantity, Decimal::ONE); // 100/100
            }
            other => panic!("expected resized, got {other:?}"),
        }
    }

    #[test]
    fn rejects_when_exposure_exhausted() {
        let mut req = sample_request();
        req.account_state.equity = Decimal::from(10_000);
        req.policy.max_total_exposure_pct = Decimal::from(10); // 1000
        req.current_exposure = Decimal::from(1000);
        req.entry_price = Some(Decimal::from(100));
        req.requested_quantity = Some(Decimal::ONE);
        req.stop_loss = None;
        let decision = size_order(&req).unwrap();
        assert_eq!(
            decision,
            RiskDecision::Rejected {
                reason: RiskRejectReason::ExposureLimit
            }
        );
    }

    #[test]
    fn trade_risk_cap_resizes() {
        let mut req = sample_request();
        req.account_state.equity = Decimal::from(10_000);
        req.policy.max_risk_per_trade_pct = Decimal::from(1); // 100 risk budget
        req.policy.max_position_size_pct = Decimal::from(100);
        req.policy.max_total_exposure_pct = Decimal::from(100);
        req.policy.max_leverage = Decimal::from(10);
        req.entry_price = Some(Decimal::from(100));
        req.stop_loss = Some(Decimal::from(90)); // distance 10
        req.requested_quantity = Some(Decimal::from(50)); // risk 500
        let decision = size_order(&req).unwrap();
        match decision {
            RiskDecision::Resized { quantity, reason } => {
                assert_eq!(quantity, Decimal::from(10)); // 100/10
                assert!(reason.contains("trade risk"));
            }
            other => panic!("expected resized, got {other:?}"),
        }
    }
}

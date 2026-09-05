//! Decimal helpers for financial values.
//!
//! Never use floating-point types for money, price, quantity, risk, or P&L.

use rust_decimal::Decimal;

use crate::error::{DomainError, DomainResult};

/// Ensures a decimal is strictly positive (> 0).
pub fn require_positive(value: Decimal, field: &str) -> DomainResult<Decimal> {
    if value <= Decimal::ZERO {
        return Err(DomainError::InvalidMoney(format!(
            "{field} must be positive, got {value}"
        )));
    }
    Ok(value)
}

/// Ensures a decimal is non-negative (>= 0).
pub fn require_non_negative(value: Decimal, field: &str) -> DomainResult<Decimal> {
    if value < Decimal::ZERO {
        return Err(DomainError::InvalidMoney(format!(
            "{field} must be non-negative, got {value}"
        )));
    }
    Ok(value)
}

/// Ensures a percentage-style ratio is in (0, 100] when expressed as percent,
/// or returns the validated value as-is after positivity check.
pub fn require_percent(value: Decimal, field: &str) -> DomainResult<Decimal> {
    require_positive(value, field)?;
    if value > Decimal::from(100) {
        return Err(DomainError::InvalidMoney(format!(
            "{field} percent must be <= 100, got {value}"
        )));
    }
    Ok(value)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;
    use rust_decimal::Decimal;

    #[test]
    fn rejects_non_positive() {
        let err = require_positive(Decimal::ZERO, "qty").unwrap_err();
        assert!(matches!(err, DomainError::InvalidMoney(_)));
    }

    #[test]
    fn accepts_positive() {
        assert!(require_positive(Decimal::ONE, "qty").is_ok());
    }
}

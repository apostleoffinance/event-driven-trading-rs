//! Account domain: capital and risk ownership (distinct from Venue).

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use crate::error::{DomainError, DomainResult};
use crate::ids::{AccountId, VenueId};
use crate::money::{require_non_negative, require_positive};

/// Classification of an account. Vault is reserved for a future phase.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AccountType {
    Simulated,
    Prop,
    Personal,
    Cex,
    /// Reserved — do not implement vault product flows yet.
    Vault,
}

/// Operational status of an account.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AccountStatus {
    Active,
    Suspended,
    Halted,
    Closed,
}

/// Account registry record (identity + configuration linkage).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Account {
    pub id: AccountId,
    pub account_type: AccountType,
    pub status: AccountStatus,
    /// Default execution venue for this account (Account ≠ Venue).
    pub venue_id: VenueId,
    pub created_at: DateTime<Utc>,
}

impl Account {
    pub fn new(
        id: AccountId,
        account_type: AccountType,
        venue_id: VenueId,
        created_at: DateTime<Utc>,
    ) -> Self {
        Self {
            id,
            account_type,
            status: AccountStatus::Active,
            venue_id,
            created_at,
        }
    }

    pub fn is_tradable(&self) -> bool {
        matches!(self.status, AccountStatus::Active)
    }

    pub fn halt(&mut self) {
        self.status = AccountStatus::Halted;
    }
}

/// Live financial state of an account.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AccountState {
    pub account_id: AccountId,
    pub balance: Decimal,
    pub equity: Decimal,
    pub used_margin: Decimal,
    pub open_position_count: u32,
    pub daily_realized_pnl: Decimal,
    pub peak_equity: Decimal,
    pub kill_switch: bool,
    pub updated_at: DateTime<Utc>,
}

impl AccountState {
    pub fn new(
        account_id: AccountId,
        starting_capital: Decimal,
        updated_at: DateTime<Utc>,
    ) -> DomainResult<Self> {
        let capital = require_positive(starting_capital, "starting_capital")?;
        Ok(Self {
            account_id,
            balance: capital,
            equity: capital,
            used_margin: Decimal::ZERO,
            open_position_count: 0,
            daily_realized_pnl: Decimal::ZERO,
            peak_equity: capital,
            kill_switch: false,
            updated_at,
        })
    }

    pub fn drawdown_pct(&self) -> DomainResult<Decimal> {
        if self.peak_equity <= Decimal::ZERO {
            return Err(DomainError::Invariant(
                "peak_equity must be positive to compute drawdown".to_string(),
            ));
        }
        if self.equity >= self.peak_equity {
            return Ok(Decimal::ZERO);
        }
        let dd = (self.peak_equity - self.equity) / self.peak_equity * Decimal::from(100);
        Ok(require_non_negative(dd, "drawdown")?.round_dp(8))
    }

    pub fn activate_kill_switch(&mut self) {
        self.kill_switch = true;
    }
}

/// Point-in-time snapshot for audit / persistence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AccountSnapshot {
    pub account: Account,
    pub state: AccountState,
    pub captured_at: DateTime<Utc>,
}

impl AccountSnapshot {
    pub fn capture(
        account: Account,
        state: AccountState,
        captured_at: DateTime<Utc>,
    ) -> DomainResult<Self> {
        if account.id != state.account_id {
            return Err(DomainError::Invariant(
                "account snapshot id mismatch between Account and AccountState".to_string(),
            ));
        }
        Ok(Self {
            account,
            state,
            captured_at,
        })
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;
    use crate::ids::{AccountId, VenueId};
    use rust_decimal::Decimal;

    #[test]
    fn account_state_rejects_zero_capital() {
        let id = AccountId::new("a1").unwrap();
        let err = AccountState::new(id, Decimal::ZERO, Utc::now()).unwrap_err();
        assert!(matches!(err, DomainError::InvalidMoney(_)));
    }

    #[test]
    fn drawdown_computed_from_peak() {
        let id = AccountId::new("a1").unwrap();
        let mut state = AccountState::new(id, Decimal::from(100), Utc::now()).unwrap();
        state.equity = Decimal::from(90);
        let dd = state.drawdown_pct().unwrap();
        assert_eq!(dd, Decimal::from(10));
    }

    #[test]
    fn snapshot_rejects_id_mismatch() {
        let a = Account::new(
            AccountId::new("a1").unwrap(),
            AccountType::Prop,
            VenueId::new("simulated").unwrap(),
            Utc::now(),
        );
        let state = AccountState::new(
            AccountId::new("a2").unwrap(),
            Decimal::from(100),
            Utc::now(),
        )
        .unwrap();
        assert!(AccountSnapshot::capture(a, state, Utc::now()).is_err());
    }
}

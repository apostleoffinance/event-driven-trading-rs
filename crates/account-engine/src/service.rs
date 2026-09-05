//! In-memory account registry and controls.

use std::collections::HashMap;

use chrono::{DateTime, Utc};
use domain::{
    Account, AccountId, AccountSnapshot, AccountState, AccountStatus, AccountType, VenueId,
};
use rust_decimal::Decimal;

use crate::config::AccountConfig;
use crate::error::{AccountEngineError, AccountEngineResult};

/// Full account record held by the engine.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccountRecord {
    pub account: Account,
    pub state: AccountState,
    pub config: AccountConfig,
}

/// Account engine operations.
pub trait AccountEngine {
    fn register(&mut self, account: Account, config: AccountConfig) -> AccountEngineResult<()>;

    fn get(&self, account_id: &AccountId) -> AccountEngineResult<&AccountRecord>;

    fn list(&self) -> Vec<&AccountRecord>;

    fn snapshot(
        &self,
        account_id: &AccountId,
        at: DateTime<Utc>,
    ) -> AccountEngineResult<AccountSnapshot>;

    fn halt(&mut self, account_id: &AccountId, at: DateTime<Utc>) -> AccountEngineResult<()>;

    fn resume(&mut self, account_id: &AccountId, at: DateTime<Utc>) -> AccountEngineResult<()>;

    fn suspend(&mut self, account_id: &AccountId, at: DateTime<Utc>) -> AccountEngineResult<()>;

    fn close(&mut self, account_id: &AccountId, at: DateTime<Utc>) -> AccountEngineResult<()>;

    fn activate_kill_switch(
        &mut self,
        account_id: &AccountId,
        at: DateTime<Utc>,
    ) -> AccountEngineResult<()>;

    fn clear_kill_switch(
        &mut self,
        account_id: &AccountId,
        at: DateTime<Utc>,
    ) -> AccountEngineResult<()>;

    fn update_equity(
        &mut self,
        account_id: &AccountId,
        equity: Decimal,
        at: DateTime<Utc>,
    ) -> AccountEngineResult<()>;

    fn apply_realized_pnl(
        &mut self,
        account_id: &AccountId,
        pnl: Decimal,
        at: DateTime<Utc>,
    ) -> AccountEngineResult<()>;

    fn set_open_position_count(
        &mut self,
        account_id: &AccountId,
        count: u32,
        at: DateTime<Utc>,
    ) -> AccountEngineResult<()>;

    fn set_used_margin(
        &mut self,
        account_id: &AccountId,
        used_margin: Decimal,
        at: DateTime<Utc>,
    ) -> AccountEngineResult<()>;

    fn reset_daily_pnl(
        &mut self,
        account_id: &AccountId,
        at: DateTime<Utc>,
    ) -> AccountEngineResult<()>;

    fn ensure_tradable(&self, account_id: &AccountId) -> AccountEngineResult<()>;
}

/// In-process account store (persistence arrives in Phase 8).
#[derive(Debug, Default)]
pub struct InMemoryAccountEngine {
    accounts: HashMap<AccountId, AccountRecord>,
}

impl InMemoryAccountEngine {
    pub fn new() -> Self {
        Self {
            accounts: HashMap::new(),
        }
    }

    fn record_mut(&mut self, account_id: &AccountId) -> AccountEngineResult<&mut AccountRecord> {
        self.accounts
            .get_mut(account_id)
            .ok_or_else(|| AccountEngineError::NotFound(account_id.as_str().to_string()))
    }

    fn record(&self, account_id: &AccountId) -> AccountEngineResult<&AccountRecord> {
        self.accounts
            .get(account_id)
            .ok_or_else(|| AccountEngineError::NotFound(account_id.as_str().to_string()))
    }
}

impl AccountEngine for InMemoryAccountEngine {
    fn register(&mut self, account: Account, config: AccountConfig) -> AccountEngineResult<()> {
        if matches!(account.account_type, AccountType::Vault) {
            return Err(AccountEngineError::NotAllowed(
                "vault accounts are reserved for a future phase".to_string(),
            ));
        }
        if self.accounts.contains_key(&account.id) {
            return Err(AccountEngineError::AlreadyExists(
                account.id.as_str().to_string(),
            ));
        }

        let state = AccountState::new(
            account.id.clone(),
            config.starting_capital,
            account.created_at,
        )?;

        self.accounts.insert(
            account.id.clone(),
            AccountRecord {
                account,
                state,
                config,
            },
        );
        Ok(())
    }

    fn get(&self, account_id: &AccountId) -> AccountEngineResult<&AccountRecord> {
        self.record(account_id)
    }

    fn list(&self) -> Vec<&AccountRecord> {
        self.accounts.values().collect()
    }

    fn snapshot(
        &self,
        account_id: &AccountId,
        at: DateTime<Utc>,
    ) -> AccountEngineResult<AccountSnapshot> {
        let record = self.record(account_id)?;
        Ok(AccountSnapshot::capture(
            record.account.clone(),
            record.state.clone(),
            at,
        )?)
    }

    fn halt(&mut self, account_id: &AccountId, at: DateTime<Utc>) -> AccountEngineResult<()> {
        let record = self.record_mut(account_id)?;
        if matches!(record.account.status, AccountStatus::Closed) {
            return Err(AccountEngineError::InvalidStatus(
                "cannot halt a closed account".to_string(),
            ));
        }
        record.account.status = AccountStatus::Halted;
        record.state.kill_switch = true;
        record.state.updated_at = at;
        tracing::warn!(account_id = %account_id, "account halted");
        Ok(())
    }

    fn resume(&mut self, account_id: &AccountId, at: DateTime<Utc>) -> AccountEngineResult<()> {
        let record = self.record_mut(account_id)?;
        match record.account.status {
            AccountStatus::Halted | AccountStatus::Suspended => {
                record.account.status = AccountStatus::Active;
                record.state.kill_switch = false;
                record.state.updated_at = at;
                Ok(())
            }
            AccountStatus::Active => Ok(()),
            AccountStatus::Closed => Err(AccountEngineError::InvalidStatus(
                "cannot resume a closed account".to_string(),
            )),
        }
    }

    fn suspend(&mut self, account_id: &AccountId, at: DateTime<Utc>) -> AccountEngineResult<()> {
        let record = self.record_mut(account_id)?;
        if matches!(record.account.status, AccountStatus::Closed) {
            return Err(AccountEngineError::InvalidStatus(
                "cannot suspend a closed account".to_string(),
            ));
        }
        record.account.status = AccountStatus::Suspended;
        record.state.updated_at = at;
        Ok(())
    }

    fn close(&mut self, account_id: &AccountId, at: DateTime<Utc>) -> AccountEngineResult<()> {
        let record = self.record_mut(account_id)?;
        record.account.status = AccountStatus::Closed;
        record.state.kill_switch = true;
        record.state.updated_at = at;
        Ok(())
    }

    fn activate_kill_switch(
        &mut self,
        account_id: &AccountId,
        at: DateTime<Utc>,
    ) -> AccountEngineResult<()> {
        let record = self.record_mut(account_id)?;
        record.state.activate_kill_switch();
        record.state.updated_at = at;
        tracing::warn!(account_id = %account_id, "account kill switch activated");
        Ok(())
    }

    fn clear_kill_switch(
        &mut self,
        account_id: &AccountId,
        at: DateTime<Utc>,
    ) -> AccountEngineResult<()> {
        let record = self.record_mut(account_id)?;
        if matches!(
            record.account.status,
            AccountStatus::Halted | AccountStatus::Closed
        ) {
            return Err(AccountEngineError::NotAllowed(
                "clear kill switch only when account is active or suspended".to_string(),
            ));
        }
        record.state.kill_switch = false;
        record.state.updated_at = at;
        Ok(())
    }

    fn update_equity(
        &mut self,
        account_id: &AccountId,
        equity: Decimal,
        at: DateTime<Utc>,
    ) -> AccountEngineResult<()> {
        if equity < Decimal::ZERO {
            return Err(AccountEngineError::Domain(
                domain::DomainError::InvalidMoney("equity must be non-negative".to_string()),
            ));
        }
        let record = self.record_mut(account_id)?;
        record.state.equity = equity;
        if equity > record.state.peak_equity {
            record.state.peak_equity = equity;
        }
        record.state.updated_at = at;
        Ok(())
    }

    fn apply_realized_pnl(
        &mut self,
        account_id: &AccountId,
        pnl: Decimal,
        at: DateTime<Utc>,
    ) -> AccountEngineResult<()> {
        let record = self.record_mut(account_id)?;
        record.state.balance += pnl;
        record.state.equity += pnl;
        record.state.daily_realized_pnl += pnl;
        if record.state.equity > record.state.peak_equity {
            record.state.peak_equity = record.state.equity;
        }
        record.state.updated_at = at;
        Ok(())
    }

    fn set_open_position_count(
        &mut self,
        account_id: &AccountId,
        count: u32,
        at: DateTime<Utc>,
    ) -> AccountEngineResult<()> {
        let record = self.record_mut(account_id)?;
        record.state.open_position_count = count;
        record.state.updated_at = at;
        Ok(())
    }

    fn set_used_margin(
        &mut self,
        account_id: &AccountId,
        used_margin: Decimal,
        at: DateTime<Utc>,
    ) -> AccountEngineResult<()> {
        if used_margin < Decimal::ZERO {
            return Err(AccountEngineError::Domain(
                domain::DomainError::InvalidMoney("used_margin must be non-negative".to_string()),
            ));
        }
        let record = self.record_mut(account_id)?;
        record.state.used_margin = used_margin;
        record.state.updated_at = at;
        Ok(())
    }

    fn reset_daily_pnl(
        &mut self,
        account_id: &AccountId,
        at: DateTime<Utc>,
    ) -> AccountEngineResult<()> {
        let record = self.record_mut(account_id)?;
        record.state.daily_realized_pnl = Decimal::ZERO;
        record.state.updated_at = at;
        Ok(())
    }

    fn ensure_tradable(&self, account_id: &AccountId) -> AccountEngineResult<()> {
        let record = self.record(account_id)?;
        if !record.account.is_tradable() {
            return Err(AccountEngineError::NotAllowed(format!(
                "account {} is not tradable (status={:?})",
                account_id.as_str(),
                record.account.status
            )));
        }
        if record.state.kill_switch {
            return Err(AccountEngineError::NotAllowed(format!(
                "account {} kill switch is active",
                account_id.as_str()
            )));
        }
        Ok(())
    }
}

/// Convenience: venue id used by simulated paper accounts.
pub fn default_simulated_venue_id() -> AccountEngineResult<VenueId> {
    Ok(VenueId::new("simulated")?)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;
    use crate::simulated::SimulatedAccountSpec;
    use chrono::Utc;
    use domain::AccountType;
    use rust_decimal::Decimal;

    #[test]
    fn register_and_snapshot() {
        let mut engine = InMemoryAccountEngine::new();
        let id = AccountId::new("sim-1").unwrap();
        engine
            .open_simulated(SimulatedAccountSpec {
                account_id: id.clone(),
                starting_capital: Decimal::from(50_000),
                label: Some("paper".into()),
                created_at: Utc::now(),
            })
            .unwrap();

        let snap = engine.snapshot(&id, Utc::now()).unwrap();
        assert_eq!(snap.account.account_type, AccountType::Simulated);
        assert_eq!(snap.state.balance, Decimal::from(50_000));
        assert_eq!(snap.account.venue_id.as_str(), "simulated");
    }

    #[test]
    fn halt_blocks_tradable() {
        let mut engine = InMemoryAccountEngine::new();
        let id = AccountId::new("sim-2").unwrap();
        engine
            .open_simulated(SimulatedAccountSpec {
                account_id: id.clone(),
                starting_capital: Decimal::from(10_000),
                label: None,
                created_at: Utc::now(),
            })
            .unwrap();
        engine.ensure_tradable(&id).unwrap();
        engine.halt(&id, Utc::now()).unwrap();
        assert!(engine.ensure_tradable(&id).is_err());
    }

    #[test]
    fn rejects_duplicate_registration() {
        let mut engine = InMemoryAccountEngine::new();
        let id = AccountId::new("sim-3").unwrap();
        let spec = SimulatedAccountSpec {
            account_id: id,
            starting_capital: Decimal::from(10_000),
            label: None,
            created_at: Utc::now(),
        };
        engine.open_simulated(spec.clone()).unwrap();
        assert!(matches!(
            engine.open_simulated(spec),
            Err(AccountEngineError::AlreadyExists(_))
        ));
    }

    #[test]
    fn apply_pnl_updates_daily_and_equity() {
        let mut engine = InMemoryAccountEngine::new();
        let id = AccountId::new("sim-4").unwrap();
        engine
            .open_simulated(SimulatedAccountSpec {
                account_id: id.clone(),
                starting_capital: Decimal::from(10_000),
                label: None,
                created_at: Utc::now(),
            })
            .unwrap();
        engine
            .apply_realized_pnl(&id, Decimal::from(-100), Utc::now())
            .unwrap();
        let state = &engine.get(&id).unwrap().state;
        assert_eq!(state.balance, Decimal::from(9_900));
        assert_eq!(state.daily_realized_pnl, Decimal::from(-100));
    }
}

//! Simulated account helpers (paper / prop-prep accounts).

use chrono::{DateTime, Utc};
use domain::{Account, AccountId, AccountType};
use rust_decimal::Decimal;

use crate::config::AccountConfig;
use crate::error::AccountEngineResult;
use crate::service::{default_simulated_venue_id, AccountEngine, InMemoryAccountEngine};

/// Spec for opening a simulated account bound to the simulated venue.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SimulatedAccountSpec {
    pub account_id: AccountId,
    pub starting_capital: Decimal,
    pub label: Option<String>,
    pub created_at: DateTime<Utc>,
}

impl InMemoryAccountEngine {
    /// Open a simulated account (AccountType::Simulated on venue `simulated`).
    pub fn open_simulated(&mut self, spec: SimulatedAccountSpec) -> AccountEngineResult<AccountId> {
        let venue_id = default_simulated_venue_id()?;
        let account = Account::new(
            spec.account_id.clone(),
            AccountType::Simulated,
            venue_id,
            spec.created_at,
        );
        let mut config = AccountConfig::new(spec.starting_capital)?;
        if let Some(label) = spec.label {
            config = config.with_label(label);
        }
        self.register(account, config)?;
        tracing::info!(
            account_id = %spec.account_id,
            capital = %spec.starting_capital,
            "opened simulated account"
        );
        Ok(spec.account_id)
    }

    /// Open a prop-style account record still executing on the simulated venue
    /// (live prop connector arrives in Phase 11).
    pub fn open_prop_on_simulated(
        &mut self,
        account_id: AccountId,
        starting_capital: Decimal,
        created_at: DateTime<Utc>,
    ) -> AccountEngineResult<AccountId> {
        let venue_id = default_simulated_venue_id()?;
        let account = Account::new(account_id.clone(), AccountType::Prop, venue_id, created_at);
        let config = AccountConfig::new(starting_capital)?.with_label("prop-simulated");
        self.register(account, config)?;
        Ok(account_id)
    }
}

//! Account configuration (non-venue settings).

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use crate::error::AccountEngineResult;
use domain::money::require_positive;

/// Static configuration attached to an account at registration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AccountConfig {
    pub starting_capital: Decimal,
    pub label: Option<String>,
}

impl AccountConfig {
    pub fn new(starting_capital: Decimal) -> AccountEngineResult<Self> {
        Ok(Self {
            starting_capital: require_positive(starting_capital, "starting_capital")?,
            label: None,
        })
    }

    pub fn with_label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }
}

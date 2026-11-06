//! Journal domain primitives.
//!
//! Journal records are personal trading observations and imported trade history.
//! They are deliberately separate from OMS orders, fills and positions.
//! This module has no execution or risk-engine side effects.

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use crate::error::{DomainError, DomainResult};
use crate::money::{require_non_negative, require_positive};

/// Direction of a journaled trade.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JournalDirection {
    Long,
    Short,
}

/// Journal lifecycle. This does not represent an OMS order lifecycle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JournalTradeStatus {
    Open,
    Closed,
    Cancelled,
}

/// Where the journal record originated.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JournalTradeSource {
    Manual,
    Mt5,
    LegacyImport,
    Other,
}

/// Validated journal trade core. Optional annotations live in application-level
/// extensions and do not affect execution authority.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct JournalTrade {
    pub id: String,
    pub owner_id: String,
    pub journal_account_id: String,
    pub setup_id: Option<String>,
    pub symbol: String,
    pub direction: JournalDirection,
    pub status: JournalTradeStatus,
    pub source: JournalTradeSource,
    pub source_system: Option<String>,
    pub source_trade_id: Option<String>,
    pub opened_at: DateTime<Utc>,
    pub closed_at: Option<DateTime<Utc>>,
    pub quantity: Decimal,
    pub entry_price: Decimal,
    pub exit_price: Option<Decimal>,
    pub stop_loss: Option<Decimal>,
    pub take_profit: Option<Decimal>,
    pub gross_pnl: Option<Decimal>,
    pub commission: Decimal,
    pub swap: Decimal,
    pub other_fees: Decimal,
    pub net_pnl: Option<Decimal>,
    pub pnl_currency: String,
    pub risk_amount: Option<Decimal>,
    pub r_multiple: Option<Decimal>,
}

impl JournalTrade {
    /// Validate invariants that must hold before persistence.
    ///
    /// This intentionally does not calculate historical P&L: imported values
    /// are source facts and must not be rewritten using today's calculation rules.
    pub fn validate(&self) -> DomainResult<()> {
        for (value, field) in [
            (self.id.as_str(), "id"),
            (self.owner_id.as_str(), "owner_id"),
            (self.journal_account_id.as_str(), "journal_account_id"),
            (self.symbol.as_str(), "symbol"),
        ] {
            if value.trim().is_empty() {
                return Err(DomainError::Validation(format!(
                    "{field} must be non-empty"
                )));
            }
        }

        require_positive(self.quantity, "journal quantity")?;
        require_positive(self.entry_price, "journal entry price")?;
        if let Some(value) = self.exit_price {
            require_positive(value, "journal exit price")?;
        }
        if let Some(value) = self.stop_loss {
            require_positive(value, "journal stop loss")?;
        }
        if let Some(value) = self.take_profit {
            require_positive(value, "journal take profit")?;
        }
        if let Some(value) = self.risk_amount {
            require_non_negative(value, "journal risk amount")?;
        }

        if self.closed_at.is_some_and(|closed| closed < self.opened_at) {
            return Err(DomainError::Validation(
                "closed_at cannot be earlier than opened_at".to_string(),
            ));
        }
        if self.status == JournalTradeStatus::Closed
            && (self.closed_at.is_none() || self.exit_price.is_none())
        {
            return Err(DomainError::Validation(
                "closed journal trades require closed_at and exit_price".to_string(),
            ));
        }
        if self.source_system.is_some() != self.source_trade_id.is_some() {
            return Err(DomainError::Validation(
                "source_system and source_trade_id must be provided together".to_string(),
            ));
        }
        if self.pnl_currency.len() != 3
            || !self
                .pnl_currency
                .chars()
                .all(|character| character.is_ascii_uppercase())
        {
            return Err(DomainError::Validation(
                "pnl_currency must be a 3-character uppercase ISO-style code".to_string(),
            ));
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;
    use chrono::TimeZone;

    fn sample() -> JournalTrade {
        JournalTrade {
            id: "journal-1".into(),
            owner_id: "user-1".into(),
            journal_account_id: "journal-account-1".into(),
            setup_id: None,
            symbol: "EURUSD".into(),
            direction: JournalDirection::Long,
            status: JournalTradeStatus::Closed,
            source: JournalTradeSource::Manual,
            source_system: None,
            source_trade_id: None,
            opened_at: Utc.with_ymd_and_hms(2026, 1, 1, 10, 0, 0).unwrap(),
            closed_at: Some(Utc.with_ymd_and_hms(2026, 1, 1, 12, 0, 0).unwrap()),
            quantity: Decimal::new(1, 0),
            entry_price: Decimal::new(11000, 4),
            exit_price: Some(Decimal::new(11100, 4)),
            stop_loss: Some(Decimal::new(10900, 4)),
            take_profit: None,
            gross_pnl: Some(Decimal::new(100, 0)),
            commission: Decimal::ZERO,
            swap: Decimal::ZERO,
            other_fees: Decimal::ZERO,
            net_pnl: Some(Decimal::new(100, 0)),
            pnl_currency: "USD".into(),
            risk_amount: Some(Decimal::new(50, 0)),
            r_multiple: Some(Decimal::new(2, 0)),
        }
    }

    #[test]
    fn accepts_valid_closed_trade() {
        assert!(sample().validate().is_ok());
    }

    #[test]
    fn rejects_closed_trade_without_exit() {
        let mut trade = sample();
        trade.exit_price = None;
        assert!(trade.validate().is_err());
    }

    #[test]
    fn rejects_negative_quantity() {
        let mut trade = sample();
        trade.quantity = Decimal::NEGATIVE_ONE;
        assert!(trade.validate().is_err());
    }

    #[test]
    fn rejects_close_before_open() {
        let mut trade = sample();
        trade.closed_at = Some(trade.opened_at - chrono::Duration::seconds(1));
        assert!(trade.validate().is_err());
    }

    #[test]
    fn rejects_incomplete_source_identity() {
        let mut trade = sample();
        trade.source_system = Some("TraderOS".into());
        assert!(trade.validate().is_err());
    }
}

//! Instrument domain. Cross-asset variants exist for future use;
//! only crypto is required for the current prop/crypto path.

use serde::{Deserialize, Serialize};

use crate::error::{DomainError, DomainResult};
use crate::ids::InstrumentId;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum InstrumentType {
    Crypto,
    Fx,
    Equity,
    Future,
    Option,
    Rate,
    Credit,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Instrument {
    pub id: InstrumentId,
    pub instrument_type: InstrumentType,
    pub symbol: String,
    pub base: Option<String>,
    pub quote: Option<String>,
    pub enabled: bool,
}

impl Instrument {
    pub fn crypto_spot(
        id: InstrumentId,
        symbol: impl Into<String>,
        base: impl Into<String>,
        quote: impl Into<String>,
    ) -> DomainResult<Self> {
        let symbol = symbol.into();
        if symbol.trim().is_empty() {
            return Err(DomainError::Validation(
                "instrument symbol must be non-empty".to_string(),
            ));
        }
        Ok(Self {
            id,
            instrument_type: InstrumentType::Crypto,
            symbol,
            base: Some(base.into()),
            quote: Some(quote.into()),
            enabled: true,
        })
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;
    use crate::ids::InstrumentId;

    #[test]
    fn crypto_spot_btc() {
        let inst = Instrument::crypto_spot(
            InstrumentId::new("BTCUSDT").unwrap(),
            "BTCUSDT",
            "BTC",
            "USDT",
        )
        .unwrap();
        assert_eq!(inst.instrument_type, InstrumentType::Crypto);
    }
}

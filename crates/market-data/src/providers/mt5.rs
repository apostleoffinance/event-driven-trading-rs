//! MT5 candle history: **not implemented**, deliberately.
//!
//! The MT5 read-only worker and EA protocol expose only `PING`, `ACCOUNT`,
//! `ORDERS`, `POSITIONS`, `SELECT_SYMBOL`, `SYMBOL_INFO` and `TICK`. There is no
//! rates/candle command, so no historical data can be obtained. This adapter
//! never fabricates data and never falls back to demo; it reports the missing
//! capability. See `docs/MARKET_DATA_SERVICE.md` for the required extension.

use async_trait::async_trait;

use crate::error::MarketDataError;
use crate::instruments::Instrument;
use crate::model::{Candle, DataClass, ProviderId, ProviderStatus, Timeframe};
use crate::provider::{CandleProvider, ProviderRequest};

pub const MT5_REASON: &str = "The MT5 worker protocol has no candle/rates command (only PING, ACCOUNT, ORDERS, POSITIONS, SELECT_SYMBOL, SYMBOL_INFO, TICK). A read-only rates command must be added to the EA, the Python worker and the Rust daemon first.";

pub struct Mt5Provider;

#[async_trait]
impl CandleProvider for Mt5Provider {
    fn id(&self) -> ProviderId {
        ProviderId::Mt5Bridge
    }

    fn name(&self) -> &'static str {
        "MT5 bridge"
    }

    fn classification(&self) -> DataClass {
        DataClass::Live
    }

    fn status(&self) -> ProviderStatus {
        ProviderStatus::NotImplemented {
            reason: MT5_REASON.to_string(),
        }
    }

    fn supports(&self, _: &Instrument, _: Timeframe) -> Result<(), MarketDataError> {
        Err(self.not_implemented())
    }

    async fn fetch_candles(&self, _: &ProviderRequest) -> Result<Vec<Candle>, MarketDataError> {
        Err(self.not_implemented())
    }
}

impl Mt5Provider {
    fn not_implemented(&self) -> MarketDataError {
        MarketDataError::NotImplemented {
            provider: ProviderId::Mt5Bridge,
            reason: MT5_REASON.to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::instruments::find;

    #[tokio::test]
    async fn always_reports_missing_capability() {
        let p = Mt5Provider;
        let eur = find("EURUSD").unwrap();
        assert!(matches!(p.status(), ProviderStatus::NotImplemented { .. }));
        assert!(matches!(
            p.supports(eur, Timeframe::H1),
            Err(MarketDataError::NotImplemented { .. })
        ));
        let req = ProviderRequest {
            instrument: eur,
            timeframe: Timeframe::H1,
            from_ms: None,
            to_ms: None,
            limit: 10,
            now_ms: 0,
        };
        assert!(matches!(
            p.fetch_candles(&req).await,
            Err(MarketDataError::NotImplemented { .. })
        ));
    }
}

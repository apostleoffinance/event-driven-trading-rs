use async_trait::async_trait;

use crate::error::MarketDataError;
use crate::instruments::Instrument;
use crate::model::{Candle, DataClass, ProviderId, ProviderStatus, Timeframe};

/// A validated request handed to a provider. `now_ms` is supplied by the
/// service clock so providers stay deterministic under test.
#[derive(Debug, Clone, Copy)]
pub struct ProviderRequest {
    pub instrument: &'static Instrument,
    pub timeframe: Timeframe,
    pub from_ms: Option<i64>,
    pub to_ms: Option<i64>,
    pub limit: usize,
    pub now_ms: i64,
}

/// Adapter boundary. Implementations translate to and from a provider's own
/// symbols and wire format; nothing provider-specific may escape.
#[async_trait]
pub trait CandleProvider: Send + Sync {
    fn id(&self) -> ProviderId;
    fn name(&self) -> &'static str;
    /// What data this provider yields when it answers.
    fn classification(&self) -> DataClass;
    fn status(&self) -> ProviderStatus;
    /// Symbol and timeframe mapping check, without any I/O.
    fn supports(
        &self,
        instrument: &Instrument,
        timeframe: Timeframe,
    ) -> Result<(), MarketDataError>;
    /// The provider-specific symbol for an instrument, if any.
    fn symbol_for(&self, instrument: &Instrument) -> Option<&'static str> {
        instrument.alias(self.id())
    }
    async fn fetch_candles(
        &self,
        request: &ProviderRequest,
    ) -> Result<Vec<Candle>, MarketDataError>;
}

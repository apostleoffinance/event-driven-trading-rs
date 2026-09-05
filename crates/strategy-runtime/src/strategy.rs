//! Core strategy trait.

use domain::{StrategyId, StrategyVersion, TradeIntent};
use events::MarketDataEvent;

use crate::context::StrategyContext;
use crate::error::StrategyResult;

/// Stateful strategy interface.
///
/// Implementations may retain rolling windows and other internal state.
/// They emit [`TradeIntent`] values only — never orders or account risk decisions.
pub trait Strategy: Send {
    fn id(&self) -> &StrategyId;

    fn version(&self) -> &StrategyVersion;

    /// Handle one market data tick. Returns zero or more trade intents.
    fn on_market_data(
        &mut self,
        ctx: &StrategyContext,
        event: &MarketDataEvent,
    ) -> StrategyResult<Vec<TradeIntent>>;
}

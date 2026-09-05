//! Stateful mean-reversion implementation.

use chrono::Utc;
use domain::{InstrumentId, OrderSide, StrategyId, StrategyVersion, TradeIntent};
use events::MarketDataEvent;
use rust_decimal::Decimal;
use strategy_runtime::{
    next_trade_intent_id, Strategy, StrategyContext, StrategyError, StrategyResult,
};

/// Strategy parameters (signal design only — not account risk limits).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MeanReversionConfig {
    pub strategy_id: StrategyId,
    pub strategy_version: StrategyVersion,
    pub instrument_id: InstrumentId,
    /// Relative deviation from the rolling mean required to act, in (0, 1).
    pub threshold: Decimal,
    pub window_size: usize,
    /// Optional stop distance as a fraction of entry (e.g. 0.02 = 2%).
    /// This expresses a strategy stop preference, not account max risk.
    pub stop_distance_pct: Option<Decimal>,
}

impl MeanReversionConfig {
    pub fn validate(&self) -> StrategyResult<()> {
        if self.threshold <= Decimal::ZERO || self.threshold >= Decimal::ONE {
            return Err(StrategyError::Validation(
                "threshold must be in (0, 1)".to_string(),
            ));
        }
        if self.window_size == 0 {
            return Err(StrategyError::Validation(
                "window_size must be greater than 0".to_string(),
            ));
        }
        if let Some(stop) = self.stop_distance_pct {
            if stop <= Decimal::ZERO || stop >= Decimal::ONE {
                return Err(StrategyError::Validation(
                    "stop_distance_pct must be in (0, 1) when set".to_string(),
                ));
            }
        }
        Ok(())
    }
}

/// Rolling-window mean reversion strategy.
pub struct MeanReversionStrategy {
    config: MeanReversionConfig,
    prices: Vec<Decimal>,
}

impl MeanReversionStrategy {
    pub fn new(config: MeanReversionConfig) -> StrategyResult<Self> {
        config.validate()?;
        let capacity = config.window_size;
        Ok(Self {
            config,
            prices: Vec::with_capacity(capacity),
        })
    }

    pub fn window_len(&self) -> usize {
        self.prices.len()
    }

    pub fn is_warmed_up(&self) -> bool {
        self.prices.len() >= self.config.window_size
    }

    fn mean(&self) -> StrategyResult<Decimal> {
        if self.prices.is_empty() {
            return Err(StrategyError::Invariant(
                "cannot compute mean of empty window".to_string(),
            ));
        }
        let sum: Decimal = self.prices.iter().copied().sum();
        let n = Decimal::from(self.prices.len() as u64);
        Ok(sum / n)
    }

    fn deviation(price: Decimal, mean: Decimal) -> StrategyResult<Decimal> {
        if mean <= Decimal::ZERO {
            return Err(StrategyError::Invariant(
                "mean must be positive".to_string(),
            ));
        }
        Ok((price - mean).abs() / mean)
    }

    fn push_price(&mut self, price: Decimal) {
        self.prices.push(price);
        if self.prices.len() > self.config.window_size {
            self.prices.remove(0);
        }
    }

    fn stop_price(&self, entry: Decimal, side: OrderSide) -> StrategyResult<Option<Decimal>> {
        let Some(pct) = self.config.stop_distance_pct else {
            return Ok(None);
        };
        let distance = entry * pct;
        let stop = match side {
            OrderSide::Buy => entry - distance,
            OrderSide::Sell => entry + distance,
        };
        if stop <= Decimal::ZERO {
            return Err(StrategyError::Validation(
                "computed stop_loss must be positive".to_string(),
            ));
        }
        Ok(Some(stop))
    }

    fn confidence_for(&self, deviation: Decimal) -> Decimal {
        // Map excess deviation over threshold into [0, 1], capped.
        let excess = (deviation - self.config.threshold).max(Decimal::ZERO);
        let scaled = (excess / self.config.threshold).min(Decimal::ONE);
        scaled.round_dp(4)
    }

    fn build_intent(
        &self,
        ctx: &StrategyContext,
        event: &MarketDataEvent,
        side: OrderSide,
        deviation: Decimal,
    ) -> StrategyResult<TradeIntent> {
        let entry = event.price;
        let mut builder = TradeIntent::builder(
            next_trade_intent_id()?,
            ctx.strategy_id.clone(),
            ctx.strategy_version.clone(),
            ctx.deployment_id.clone(),
            event.instrument_id.clone(),
            side,
            Utc::now(),
        )
        .entry_price(entry)?;

        if let Some(stop) = self.stop_price(entry, side)? {
            builder = builder.stop_loss(stop)?;
        }

        let confidence = self.confidence_for(deviation);
        if confidence > Decimal::ZERO {
            builder = builder.confidence(confidence)?;
        }

        // Deliberately omit target_quantity / target_notional — Risk Engine sizes.
        Ok(builder.build()?)
    }
}

impl Strategy for MeanReversionStrategy {
    fn id(&self) -> &StrategyId {
        &self.config.strategy_id
    }

    fn version(&self) -> &StrategyVersion {
        &self.config.strategy_version
    }

    fn on_market_data(
        &mut self,
        ctx: &StrategyContext,
        event: &MarketDataEvent,
    ) -> StrategyResult<Vec<TradeIntent>> {
        if event.instrument_id != self.config.instrument_id {
            return Ok(Vec::new());
        }

        // Warm-up: fill the window without trading.
        if !self.is_warmed_up() {
            self.push_price(event.price);
            return Ok(Vec::new());
        }

        // Signal against the existing window, then slide in the new price.
        let mean = self.mean()?;
        let deviation = Self::deviation(event.price, mean)?;

        let mut intents = Vec::new();
        if deviation > self.config.threshold {
            let side = if event.price < mean {
                OrderSide::Buy
            } else if event.price > mean {
                OrderSide::Sell
            } else {
                // Exactly at mean with positive deviation is impossible; treat as hold.
                self.push_price(event.price);
                return Ok(Vec::new());
            };
            intents.push(self.build_intent(ctx, event, side, deviation)?);
        }

        self.push_price(event.price);
        Ok(intents)
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;
    use chrono::Utc;
    use domain::{DeploymentId, InstrumentId, StrategyId, StrategyVersion};
    use events::MarketDataEvent;
    use rust_decimal::Decimal;
    use strategy_runtime::StrategyContext;

    fn config(window: usize, threshold: Decimal) -> MeanReversionConfig {
        MeanReversionConfig {
            strategy_id: StrategyId::new("btc-mean-reversion").unwrap(),
            strategy_version: StrategyVersion::new("v1").unwrap(),
            instrument_id: InstrumentId::new("BTCUSDT").unwrap(),
            threshold,
            window_size: window,
            stop_distance_pct: Some(Decimal::new(2, 2)),
        }
    }

    fn ctx() -> StrategyContext {
        StrategyContext::new(
            StrategyId::new("btc-mean-reversion").unwrap(),
            StrategyVersion::new("v1").unwrap(),
            DeploymentId::new("deployment-001").unwrap(),
        )
    }

    fn md(price: i64) -> MarketDataEvent {
        MarketDataEvent::new(
            InstrumentId::new("BTCUSDT").unwrap(),
            Decimal::from(price),
            "test",
            Utc::now(),
            Utc::now(),
        )
        .unwrap()
    }

    #[test]
    fn rejects_invalid_threshold() {
        let mut cfg = config(3, Decimal::from(2));
        assert!(MeanReversionStrategy::new(cfg.clone()).is_err());
        cfg.threshold = Decimal::new(2, 2);
        assert!(MeanReversionStrategy::new(cfg).is_ok());
    }

    #[test]
    fn rolling_window_updates_during_warmup() {
        let mut strategy = MeanReversionStrategy::new(config(3, Decimal::new(2, 2))).unwrap();
        let ctx = ctx();
        assert!(strategy.on_market_data(&ctx, &md(100)).unwrap().is_empty());
        assert_eq!(strategy.window_len(), 1);
        assert!(strategy.on_market_data(&ctx, &md(101)).unwrap().is_empty());
        assert_eq!(strategy.window_len(), 2);
        assert!(strategy.on_market_data(&ctx, &md(102)).unwrap().is_empty());
        assert_eq!(strategy.window_len(), 3);
        assert!(strategy.is_warmed_up());
    }

    #[test]
    fn emits_buy_intent_when_price_below_mean() {
        let mut strategy = MeanReversionStrategy::new(config(3, Decimal::new(5, 2))).unwrap();
        let ctx = ctx();
        // Window mean will be 100
        strategy.on_market_data(&ctx, &md(100)).unwrap();
        strategy.on_market_data(&ctx, &md(100)).unwrap();
        strategy.on_market_data(&ctx, &md(100)).unwrap();

        // 10% below mean with 5% threshold → Buy
        let intents = strategy.on_market_data(&ctx, &md(90)).unwrap();
        assert_eq!(intents.len(), 1);
        let intent = &intents[0];
        assert_eq!(intent.side, OrderSide::Buy);
        assert_eq!(intent.entry_price, Some(Decimal::from(90)));
        assert!(intent.stop_loss.is_some());
        assert!(intent.target_quantity.is_none());
        assert!(intent.target_notional.is_none());
        assert_eq!(intent.deployment_id.as_str(), "deployment-001");
        assert_eq!(strategy.window_len(), 3);
    }

    #[test]
    fn emits_sell_intent_when_price_above_mean() {
        let mut strategy = MeanReversionStrategy::new(config(3, Decimal::new(5, 2))).unwrap();
        let ctx = ctx();
        strategy.on_market_data(&ctx, &md(100)).unwrap();
        strategy.on_market_data(&ctx, &md(100)).unwrap();
        strategy.on_market_data(&ctx, &md(100)).unwrap();

        let intents = strategy.on_market_data(&ctx, &md(110)).unwrap();
        assert_eq!(intents.len(), 1);
        assert_eq!(intents[0].side, OrderSide::Sell);
    }

    #[test]
    fn holds_when_inside_threshold() {
        let mut strategy = MeanReversionStrategy::new(config(3, Decimal::new(5, 2))).unwrap();
        let ctx = ctx();
        strategy.on_market_data(&ctx, &md(100)).unwrap();
        strategy.on_market_data(&ctx, &md(100)).unwrap();
        strategy.on_market_data(&ctx, &md(100)).unwrap();

        let intents = strategy.on_market_data(&ctx, &md(101)).unwrap();
        assert!(intents.is_empty());
        assert_eq!(strategy.window_len(), 3);
    }

    #[test]
    fn ignores_other_instruments() {
        let mut strategy = MeanReversionStrategy::new(config(3, Decimal::new(2, 2))).unwrap();
        let ctx = ctx();
        let other = MarketDataEvent::new(
            InstrumentId::new("ETHUSDT").unwrap(),
            Decimal::from(100),
            "test",
            Utc::now(),
            Utc::now(),
        )
        .unwrap();
        assert!(strategy.on_market_data(&ctx, &other).unwrap().is_empty());
        assert_eq!(strategy.window_len(), 0);
    }
}

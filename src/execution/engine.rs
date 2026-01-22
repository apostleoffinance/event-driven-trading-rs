use rust_decimal::Decimal;
use crate::error::{Result, TradingError};
use crate::engine::{EventBus, Event};
use crate::risk::{PositionSizer, StopLossManager, PortfolioLimits};
use crate::strategy::Signal;

/// Trade execution record
#[derive(Debug, Clone)]
pub struct Trade {
    pub symbol: String,
    pub signal: Signal,
    pub entry_price: Decimal,
    pub position_size: Decimal,
    pub stop_loss: Decimal,
    pub timestamp: u64,
}

/// Paper trading execution engine with risk management
pub struct ExecutionEngine {
    account_balance: Decimal,
    portfolio_limits: PortfolioLimits,
    trades: Vec<Trade>,
    event_bus: EventBus,
}

impl ExecutionEngine {
    pub fn new(
        initial_balance: Decimal,
        portfolio_limits: PortfolioLimits,
        event_bus: EventBus,
    ) -> Result<Self> {
        if initial_balance <= Decimal::ZERO {
            return Err(TradingError::Validation(
                "Initial balance must be positive".to_string(),
            ));
        }

        Ok(Self {
            account_balance: initial_balance,
            portfolio_limits,
            trades: Vec::new(),
            event_bus,
        })
    }

    /// Execute trade with risk management checks
    pub fn execute(
        &mut self,
        symbol: String,
        signal: Signal,
        entry_price: Decimal,
        stop_loss_distance: Decimal,
    ) -> Result<Option<Trade>> {
        // Check portfolio limits
        if !self.portfolio_limits.can_open_new_position(self.trades.len())? {
            let err_msg = "Max open positions reached".to_string();
            self.event_bus.publish(Event::Error(err_msg.clone()))?;
            return Err(TradingError::Execution(err_msg));
        }

        // Calculate position size using risk management
        let position_size = PositionSizer::calculate(
            self.account_balance,
            Decimal::from(2), // 2% risk per trade
            stop_loss_distance,
        )?;

        // Check if position size exceeds limit
        if self.portfolio_limits.is_position_too_large(position_size)? {
            let err_msg = "Position size exceeds limit".to_string();
            self.event_bus.publish(Event::Error(err_msg.clone()))?;
            return Err(TradingError::Execution(err_msg));
        }

        // Calculate stop loss
        let is_long = matches!(signal, Signal::Buy);
        let stop_loss = StopLossManager::calculate_stop_loss(
            entry_price,
            stop_loss_distance,
            is_long,
        )?;

        // Execute trade only on Buy or Sell signals
        match signal {
            Signal::Buy | Signal::Sell => {
                let timestamp = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map_err(|e| TradingError::Time(e.to_string()))?
                    .as_millis() as u64;

                let trade = Trade {
                    symbol: symbol.clone(),
                    signal,
                    entry_price,
                    position_size,
                    stop_loss,
                    timestamp,
                };

                self.trades.push(trade.clone());

                // Publish trade execution event
                self.event_bus.publish(Event::TradeExecuted {
                    symbol,
                    signal,
                    entry_price,
                    position_size,
                    stop_loss,
                })?;

                Ok(Some(trade))
            }
            Signal::Hold => Ok(None),
        }
    }

    /// Check if trade hit stop loss
    pub fn check_stop_loss(&self, current_price: Decimal, trade: &Trade) -> Result<bool> {
        let is_long = matches!(trade.signal, Signal::Buy);
        StopLossManager::is_stop_hit(current_price, trade.stop_loss, is_long)
    }

    /// Get account balance
    pub fn balance(&self) -> Decimal {
        self.account_balance
    }

    /// Get open trades count
    pub fn open_positions(&self) -> usize {
        self.trades.len()
    }

    /// Get trade history
    pub fn trades(&self) -> &[Trade] {
        &self.trades
    }
}

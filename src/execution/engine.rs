use rust_decimal::Decimal;
use crate::error::{Result, TradingError};
use crate::engine::{EventBus, Event};
use crate::risk::{PositionSizer, StopLossManager, PortfolioLimits, RiskEngine};
use crate::portfolio::position::PositionSide;
use std::collections::HashMap;
use super::order::{Order, OrderSide, OrderType, OrderStatus, TimeInForce};
use super::fill::{Fill, FillSimulator};
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
    risk_engine: RiskEngine,
    trades: Vec<Trade>,
    event_bus: EventBus,
    orders: HashMap<u64, Order>,
    fills: Vec<Fill>,
    next_order_id: u64,
}

impl ExecutionEngine {
    pub fn new(
        initial_balance: Decimal,
        portfolio_limits: PortfolioLimits,
        event_bus: EventBus,
    ) -> Result<Self> {
        let risk_engine = RiskEngine::new(initial_balance, portfolio_limits)?;
        Ok(Self {
            risk_engine,
            trades: Vec::new(),
            event_bus,
            orders: HashMap::new(),
            fills: Vec::new(),
            next_order_id: 1,
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
        let side = match signal {
            Signal::Buy => PositionSide::Long,
            Signal::Sell => PositionSide::Short,
            Signal::Hold => return Ok(None),
        };
        let order_side = match signal {
            Signal::Buy => OrderSide::Buy,
            Signal::Sell => OrderSide::Sell,
            Signal::Hold => return Ok(None),
        };

        // Calculate position size using risk management
        let position_size = PositionSizer::calculate(
            self.risk_engine.account_balance(),
            Decimal::from(2), // 2% risk per trade
            stop_loss_distance,
        )?;

        // Pre-trade risk validation (limits, margin, daily loss, kill-switch)
        if let Err(err) = self.risk_engine.pre_trade_validate(
            &symbol,
            side,
            entry_price,
            position_size,
            stop_loss_distance,
        ) {
            let err_msg = err.to_string();
            if self.risk_engine.is_kill_switch_active() {
                if let Some(reason) = self.risk_engine.kill_switch_reason() {
                    let _ = self.event_bus.publish(Event::RiskHalt {
                        reason: reason.to_string(),
                    });
                }
            }
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

        let order_id = self.submit_order(
            symbol.clone(),
            order_side,
            OrderType::Market,
            TimeInForce::Ioc,
            position_size,
            Some(entry_price),
        )?;

        self.process_fills(order_id, entry_price, stop_loss, side, signal)
    }

    pub fn submit_order(
        &mut self,
        symbol: String,
        side: OrderSide,
        order_type: OrderType,
        tif: TimeInForce,
        quantity: Decimal,
        price: Option<Decimal>,
    ) -> Result<u64> {
        if quantity <= Decimal::ZERO {
            return Err(TradingError::Validation(
                "Order quantity must be positive".to_string(),
            ));
        }

        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|e| TradingError::Time(e.to_string()))?
            .as_millis() as u64;

        let order_id = self.next_order_id;
        self.next_order_id += 1;

        let order = Order {
            id: order_id,
            symbol: symbol.clone(),
            side,
            order_type,
            tif,
            quantity,
            price,
            filled_quantity: Decimal::ZERO,
            status: OrderStatus::New,
            created_at: timestamp,
            updated_at: timestamp,
        };

        self.orders.insert(order_id, order);

        let signal = match side {
            OrderSide::Buy => Signal::Buy,
            OrderSide::Sell => Signal::Sell,
        };

        self.event_bus.publish(Event::OrderSubmitted {
            order_id,
            symbol,
            side: signal,
            quantity,
            price,
        })?;

        Ok(order_id)
    }

    pub fn cancel_order(&mut self, order_id: u64) -> Result<()> {
        let order = self.orders.get_mut(&order_id).ok_or_else(|| {
            TradingError::Execution("Order not found".to_string())
        })?;

        if matches!(order.status, OrderStatus::Filled | OrderStatus::Cancelled) {
            return Err(TradingError::Execution(
                "Order already closed".to_string(),
            ));
        }

        order.status = OrderStatus::Cancelled;
        self.event_bus.publish(Event::OrderCancelled {
            order_id,
            symbol: order.symbol.clone(),
        })?;
        Ok(())
    }

    pub fn replace_order(&mut self, order_id: u64, new_qty: Decimal, new_price: Option<Decimal>) -> Result<()> {
        let order = self.orders.get_mut(&order_id).ok_or_else(|| {
            TradingError::Execution("Order not found".to_string())
        })?;

        if new_qty <= Decimal::ZERO {
            return Err(TradingError::Validation(
                "Order quantity must be positive".to_string(),
            ));
        }

        if matches!(order.status, OrderStatus::Filled | OrderStatus::Cancelled) {
            return Err(TradingError::Execution(
                "Order already closed".to_string(),
            ));
        }

        order.quantity = new_qty;
        order.price = new_price;
        order.updated_at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|e| TradingError::Time(e.to_string()))?
            .as_millis() as u64;
        Ok(())
    }

    fn process_fills(
        &mut self,
        order_id: u64,
        entry_price: Decimal,
        stop_loss: Decimal,
        side: PositionSide,
        signal: Signal,
    ) -> Result<Option<Trade>> {
        let order = self.orders.get(&order_id).ok_or_else(|| {
            TradingError::Execution("Order not found".to_string())
        })?;

        let fills = FillSimulator::simulate(
            order_id,
            &order.symbol,
            entry_price,
            order.quantity,
        );

        let mut filled_qty = Decimal::ZERO;
        for fill in &fills {
            filled_qty += fill.quantity;
            self.fills.push(fill.clone());
            self.event_bus.publish(Event::OrderFilled {
                order_id,
                symbol: fill.symbol.clone(),
                filled_qty: fill.quantity,
                price: fill.price,
            })?;
        }

        let order = self.orders.get_mut(&order_id).ok_or_else(|| {
            TradingError::Execution("Order not found".to_string())
        })?;
        order.filled_quantity += filled_qty;
        order.status = if order.filled_quantity >= order.quantity {
            OrderStatus::Filled
        } else {
            OrderStatus::PartiallyFilled
        };

        if filled_qty > Decimal::ZERO {
            let timestamp = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_err(|e| TradingError::Time(e.to_string()))?
                .as_millis() as u64;

            let trade = Trade {
                symbol: order.symbol.clone(),
                signal,
                entry_price,
                position_size: filled_qty,
                stop_loss,
                timestamp,
            };

            self.trades.push(trade.clone());
            self.risk_engine.record_trade_open(
                order.symbol.clone(),
                side,
                entry_price,
                filled_qty,
                stop_loss,
                timestamp,
            )?;

            self.event_bus.publish(Event::TradeExecuted {
                symbol: order.symbol.clone(),
                signal,
                entry_price,
                position_size: filled_qty,
                stop_loss,
            })?;

            return Ok(Some(trade));
        }

        Ok(None)
    }

    /// Check if trade hit stop loss
    pub fn check_stop_loss(&self, current_price: Decimal, trade: &Trade) -> Result<bool> {
        let is_long = matches!(trade.signal, Signal::Buy);
        StopLossManager::is_stop_hit(current_price, trade.stop_loss, is_long)
    }

    /// Update market price for risk monitoring
    pub fn update_price(&mut self, symbol: &str, price: Decimal) -> Result<()> {
        self.risk_engine.update_price(symbol, price)?;
        if self.risk_engine.is_kill_switch_active() {
            if let Some(reason) = self.risk_engine.kill_switch_reason() {
                self.event_bus.publish(Event::RiskHalt {
                    reason: reason.to_string(),
                })?;
            }
            self.liquidate_all()?;
        }
        Ok(())
    }

    /// Check if kill-switch is active
    pub fn is_kill_switch_active(&self) -> bool {
        self.risk_engine.is_kill_switch_active()
    }

    pub fn kill_switch_reason(&self) -> Option<&str> {
        self.risk_engine.kill_switch_reason()
    }

    fn liquidate_all(&mut self) -> Result<()> {
        let results = self.risk_engine.liquidate_all();
        for (symbol, exit_price, pnl) in results {
            self.event_bus.publish(Event::TradeClosed {
                symbol,
                exit_price,
                pnl,
            })?;
        }
        Ok(())
    }

    /// Get account balance
    pub fn balance(&self) -> Decimal {
        self.risk_engine.account_balance()
    }

    /// Get open trades count
    pub fn open_positions(&self) -> usize {
        self.risk_engine.open_positions()
    }

    /// Get trade history
    pub fn trades(&self) -> &[Trade] {
        &self.trades
    }

    pub fn orders(&self) -> &HashMap<u64, Order> {
        &self.orders
    }

    pub fn fills(&self) -> &[Fill] {
        &self.fills
    }
}

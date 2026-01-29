use rust_decimal::Decimal;
use crate::market_data::event::PriceEvent;
use crate::strategy::Signal;

/// All events in the trading system
#[derive(Debug, Clone)]
pub enum Event {
    /// Market data event
    PriceUpdated(PriceEvent),
    
    /// Strategy generated a signal
    SignalGenerated {
        strategy_name: String,
        symbol: String,
        signal: Signal,
        price: Decimal,
    },
    
    /// Trade execution event
    TradeExecuted {
        symbol: String,
        signal: Signal,
        entry_price: Decimal,
        position_size: Decimal,
        stop_loss: Decimal,
    },
    
    /// Trade closed event
    TradeClosed {
        symbol: String,
        exit_price: Decimal,
        pnl: Decimal,
    },

    /// Order submitted event
    OrderSubmitted {
        order_id: u64,
        symbol: String,
        side: Signal,
        quantity: Decimal,
        price: Option<Decimal>,
    },

    /// Order filled event
    OrderFilled {
        order_id: u64,
        symbol: String,
        filled_qty: Decimal,
        price: Decimal,
    },

    /// Order cancelled event
    OrderCancelled {
        order_id: u64,
        symbol: String,
    },

    /// Order rejected event
    OrderRejected {
        order_id: u64,
        symbol: String,
        reason: String,
    },

    /// Risk kill-switch event
    RiskHalt {
        reason: String,
    },
    
    /// Error event
    Error(String),
}

impl Event {
    pub fn event_type(&self) -> &str {
        match self {
            Event::PriceUpdated(_) => "PriceUpdated",
            Event::SignalGenerated { .. } => "SignalGenerated",
            Event::TradeExecuted { .. } => "TradeExecuted",
            Event::TradeClosed { .. } => "TradeClosed",
            Event::OrderSubmitted { .. } => "OrderSubmitted",
            Event::OrderFilled { .. } => "OrderFilled",
            Event::OrderCancelled { .. } => "OrderCancelled",
            Event::OrderRejected { .. } => "OrderRejected",
            Event::RiskHalt { .. } => "RiskHalt",
            Event::Error(_) => "Error",
        }
    }
}
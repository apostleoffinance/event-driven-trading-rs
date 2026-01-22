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
            Event::Error(_) => "Error",
        }
    }
}
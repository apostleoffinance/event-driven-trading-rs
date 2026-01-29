use rust_decimal::Decimal;
use crate::error::{Result, TradingError};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PositionSide {
	Long,
	Short,
}

#[derive(Debug, Clone)]
pub struct Position {
	pub symbol: String,
	pub side: PositionSide,
	pub entry_price: Decimal,
	pub size: Decimal,
	pub stop_loss: Decimal,
	pub opened_at: u64,
	pub last_price: Decimal,
}

impl Position {
	pub fn new(
		symbol: String,
		side: PositionSide,
		entry_price: Decimal,
		size: Decimal,
		stop_loss: Decimal,
		opened_at: u64,
	) -> Result<Self> {
		if entry_price <= Decimal::ZERO || size <= Decimal::ZERO || stop_loss <= Decimal::ZERO {
			return Err(TradingError::Validation(
				"Entry price, size, and stop loss must be positive".to_string(),
			));
		}

		Ok(Self {
			symbol,
			side,
			entry_price,
			size,
			stop_loss,
			opened_at,
			last_price: entry_price,
		})
	}

	pub fn update_price(&mut self, price: Decimal) -> Result<()> {
		if price <= Decimal::ZERO {
			return Err(TradingError::Validation(
				"Price must be positive".to_string(),
			));
		}
		self.last_price = price;
		Ok(())
	}

	pub fn notional_value(&self) -> Decimal {
		self.entry_price * self.size
	}

	pub fn unrealized_pnl(&self) -> Decimal {
		let diff = match self.side {
			PositionSide::Long => self.last_price - self.entry_price,
			PositionSide::Short => self.entry_price - self.last_price,
		};
		(diff * self.size).round_dp(8)
	}
}

use std::collections::HashMap;
use rust_decimal::Decimal;
use crate::error::{Result, TradingError};
use super::position::{Position, PositionSide};

#[derive(Debug, Default)]
pub struct Portfolio {
	positions: HashMap<String, Position>,
	realized_pnl: Decimal,
}

impl Portfolio {
	pub fn new() -> Self {
		Self {
			positions: HashMap::new(),
			realized_pnl: Decimal::ZERO,
		}
	}

	pub fn open_position(
		&mut self,
		symbol: String,
		side: PositionSide,
		entry_price: Decimal,
		size: Decimal,
		stop_loss: Decimal,
		opened_at: u64,
	) -> Result<()> {
		if self.positions.contains_key(&symbol) {
			return Err(TradingError::Risk(
				format!("Position already open for {symbol}"),
			));
		}

		let position = Position::new(symbol.clone(), side, entry_price, size, stop_loss, opened_at)?;
		self.positions.insert(symbol, position);
		Ok(())
	}

	pub fn close_position(&mut self, symbol: &str, exit_price: Decimal) -> Result<Decimal> {
		let mut position = self.positions.remove(symbol).ok_or_else(|| {
			TradingError::Risk(format!("No open position for {symbol}"))
		})?;

		position.update_price(exit_price)?;
		let pnl = position.unrealized_pnl();
		self.realized_pnl += pnl;
		Ok(pnl)
	}

	pub fn update_price(&mut self, symbol: &str, price: Decimal) -> Result<()> {
		if let Some(position) = self.positions.get_mut(symbol) {
			position.update_price(price)?;
		}
		Ok(())
	}

	pub fn open_positions(&self) -> usize {
		self.positions.len()
	}

	pub fn exposure(&self) -> Decimal {
		self.positions.values().map(|p| p.notional_value()).sum()
	}

	pub fn unrealized_pnl(&self) -> Decimal {
		self.positions.values().map(|p| p.unrealized_pnl()).sum()
	}

	pub fn realized_pnl(&self) -> Decimal {
		self.realized_pnl
	}

	/// Close all positions at their last known prices
	pub fn close_all_at_last(&mut self) -> Vec<(String, Decimal, Decimal)> {
		let mut results = Vec::new();
		let symbols: Vec<String> = self.positions.keys().cloned().collect();

		for symbol in symbols {
			if let Some(position) = self.positions.remove(&symbol) {
				let exit_price = position.last_price;
				let pnl = position.unrealized_pnl();
				self.realized_pnl += pnl;
				results.push((symbol, exit_price, pnl));
			}
		}

		results
	}

	/// Reconcile internal positions against external snapshot
	pub fn reconcile(&self, external_positions: &HashMap<String, Decimal>) -> Vec<String> {
		let mut breaks = Vec::new();

		for (symbol, position) in &self.positions {
			let internal_qty = position.size;
			let external_qty = external_positions.get(symbol).cloned().unwrap_or(Decimal::ZERO);
			if (internal_qty - external_qty).abs() > Decimal::from_str_exact("0.0001").unwrap_or(Decimal::ZERO) {
				breaks.push(format!(
					"Position break for {}: internal={}, external={}",
					symbol, internal_qty, external_qty
				));
			}
		}

		for (symbol, external_qty) in external_positions {
			if !self.positions.contains_key(symbol) && *external_qty != Decimal::ZERO {
				breaks.push(format!(
					"External position not in portfolio: {} qty={}",
					symbol, external_qty
				));
			}
		}

		breaks
	}
}

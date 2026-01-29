use rust_decimal::Decimal;

#[derive(Debug, Clone)]
pub struct Fill {
	pub order_id: u64,
	pub symbol: String,
	pub price: Decimal,
	pub quantity: Decimal,
	pub fee: Decimal,
	pub timestamp: u64,
}

pub struct FillSimulator;

impl FillSimulator {
	/// Simulate fills with basic partial fill handling
	pub fn simulate(
		order_id: u64,
		symbol: &str,
		price: Decimal,
		quantity: Decimal,
	) -> Vec<Fill> {
		let timestamp = std::time::SystemTime::now()
			.duration_since(std::time::UNIX_EPOCH)
			.map(|d| d.as_millis() as u64)
			.unwrap_or(0);

		let mut fills = Vec::new();

		let (first_qty, second_qty) = if quantity > Decimal::from(1) {
			let half = (quantity / Decimal::from(2)).round_dp(8);
			(half, quantity - half)
		} else {
			(quantity, Decimal::ZERO)
		};

		if first_qty > Decimal::ZERO {
			fills.push(Fill {
				order_id,
				symbol: symbol.to_string(),
				price,
				quantity: first_qty,
				fee: (price * first_qty * Decimal::from_str_exact("0.0005").unwrap_or(Decimal::ZERO)).round_dp(8),
				timestamp,
			});
		}

		if second_qty > Decimal::ZERO {
			fills.push(Fill {
				order_id,
				symbol: symbol.to_string(),
				price,
				quantity: second_qty,
				fee: (price * second_qty * Decimal::from_str_exact("0.0005").unwrap_or(Decimal::ZERO)).round_dp(8),
				timestamp,
			});
		}

		fills
	}
}

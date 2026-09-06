//! Cross-language TradeIntent wire format (JSON / NDJSON).
//!
//! Decimals and IDs are strings so Python and Rust agree without float risk.

use chrono::{DateTime, Utc};
use domain::{
    DeploymentId, InstrumentId, OrderSide, StrategyId, StrategyVersion, TradeIntent, TradeIntentId,
};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use crate::error::{BridgeError, BridgeResult};

/// JSON shape emitted by Python strategies and ingested by Rust.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TradeIntentWire {
    pub id: String,
    pub strategy_id: String,
    pub strategy_version: String,
    pub deployment_id: String,
    pub instrument_id: String,
    /// `"Buy"` or `"Sell"`.
    pub side: String,
    #[serde(default)]
    pub target_quantity: Option<String>,
    #[serde(default)]
    pub target_notional: Option<String>,
    #[serde(default)]
    pub entry_price: Option<String>,
    #[serde(default)]
    pub stop_loss: Option<String>,
    #[serde(default)]
    pub take_profit: Option<String>,
    #[serde(default)]
    pub confidence: Option<String>,
    /// RFC3339 timestamp.
    pub timestamp: String,
}

impl TradeIntentWire {
    pub fn from_domain(intent: &TradeIntent) -> Self {
        Self {
            id: intent.id.as_str().to_string(),
            strategy_id: intent.strategy_id.as_str().to_string(),
            strategy_version: intent.strategy_version.as_str().to_string(),
            deployment_id: intent.deployment_id.as_str().to_string(),
            instrument_id: intent.instrument_id.as_str().to_string(),
            side: match intent.side {
                OrderSide::Buy => "Buy".into(),
                OrderSide::Sell => "Sell".into(),
            },
            target_quantity: intent.target_quantity.map(|d| d.to_string()),
            target_notional: intent.target_notional.map(|d| d.to_string()),
            entry_price: intent.entry_price.map(|d| d.to_string()),
            stop_loss: intent.stop_loss.map(|d| d.to_string()),
            take_profit: intent.take_profit.map(|d| d.to_string()),
            confidence: intent.confidence.map(|d| d.to_string()),
            timestamp: intent.timestamp.to_rfc3339(),
        }
    }

    pub fn into_domain(self) -> BridgeResult<TradeIntent> {
        let side = match self.side.as_str() {
            "Buy" | "buy" | "BUY" => OrderSide::Buy,
            "Sell" | "sell" | "SELL" => OrderSide::Sell,
            other => {
                return Err(BridgeError::Invalid(format!(
                    "side must be Buy or Sell, got {other}"
                )));
            }
        };

        let timestamp: DateTime<Utc> = DateTime::parse_from_rfc3339(&self.timestamp)
            .map_err(|e| BridgeError::Invalid(format!("timestamp: {e}")))?
            .with_timezone(&Utc);

        let mut builder = TradeIntent::builder(
            TradeIntentId::new(self.id)?,
            StrategyId::new(self.strategy_id)?,
            StrategyVersion::new(self.strategy_version)?,
            DeploymentId::new(self.deployment_id)?,
            InstrumentId::new(self.instrument_id)?,
            side,
            timestamp,
        );

        if let Some(v) = self.target_quantity {
            builder = builder.target_quantity(parse_decimal(&v, "target_quantity")?)?;
        }
        if let Some(v) = self.target_notional {
            builder = builder.target_notional(parse_decimal(&v, "target_notional")?)?;
        }
        if let Some(v) = self.entry_price {
            builder = builder.entry_price(parse_decimal(&v, "entry_price")?)?;
        }
        if let Some(v) = self.stop_loss {
            builder = builder.stop_loss(parse_decimal(&v, "stop_loss")?)?;
        }
        if let Some(v) = self.take_profit {
            builder = builder.take_profit(parse_decimal(&v, "take_profit")?)?;
        }
        if let Some(v) = self.confidence {
            builder = builder.confidence(parse_decimal(&v, "confidence")?)?;
        }

        Ok(builder.build()?)
    }
}

fn parse_decimal(raw: &str, field: &str) -> BridgeResult<Decimal> {
    Decimal::from_str_exact(raw.trim())
        .map_err(|e| BridgeError::Invalid(format!("{field}: invalid decimal '{raw}': {e}")))
}

/// Parse one JSON object (single line or blob) into a domain intent.
pub fn parse_intent_json(json: &str) -> BridgeResult<TradeIntent> {
    let wire: TradeIntentWire = serde_json::from_str(json)?;
    wire.into_domain()
}

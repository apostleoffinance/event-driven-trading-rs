//! Event identifiers (distinct from domain trading entity IDs).

use std::fmt;

use serde::{Deserialize, Serialize};

use crate::error::{EventsError, EventsResult};

/// Unique identifier for a published event envelope.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct EventId(String);

impl EventId {
    pub fn new(value: impl Into<String>) -> EventsResult<Self> {
        let value = value.into();
        let trimmed = value.trim();
        if trimmed.is_empty() {
            return Err(EventsError::Invalid(
                "event id must be non-empty".to_string(),
            ));
        }
        if trimmed.len() > 128 {
            return Err(EventsError::Invalid(
                "event id exceeds 128 characters".to_string(),
            ));
        }
        Ok(Self(trimmed.to_string()))
    }

    /// Generate a time-based unique id without requiring external RNG crates.
    pub fn generate(prefix: &str) -> EventsResult<Self> {
        let millis = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|e| EventsError::Invalid(format!("clock error: {e}")))?
            .as_millis();
        Self::new(format!("{prefix}-{millis}"))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for EventId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// End-to-end correlation id spanning intent → risk → order → fill.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CorrelationId(String);

impl CorrelationId {
    pub fn new(value: impl Into<String>) -> EventsResult<Self> {
        let value = value.into();
        let trimmed = value.trim();
        if trimmed.is_empty() {
            return Err(EventsError::Invalid(
                "correlation id must be non-empty".to_string(),
            ));
        }
        if trimmed.len() > 128 {
            return Err(EventsError::Invalid(
                "correlation id exceeds 128 characters".to_string(),
            ));
        }
        Ok(Self(trimmed.to_string()))
    }

    /// Derive a stable correlation id from a trade intent id.
    pub fn from_trade_intent(trade_intent_id: &str) -> EventsResult<Self> {
        Self::new(format!("corr-{trade_intent_id}"))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for CorrelationId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;

    #[test]
    fn rejects_empty() {
        assert!(EventId::new("").is_err());
        assert!(CorrelationId::new("").is_err());
    }
}

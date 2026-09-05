//! Venue domain: where execution occurs (distinct from Account).

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::ids::VenueId;

/// High-level venue category. Connectors are implemented later.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum VenueType {
    Simulated,
    Prop,
    Cex,
    Onchain,
}

/// Execution venue registry record.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Venue {
    pub id: VenueId,
    pub venue_type: VenueType,
    pub name: String,
    pub enabled: bool,
    pub created_at: DateTime<Utc>,
}

impl Venue {
    pub fn new(
        id: VenueId,
        venue_type: VenueType,
        name: impl Into<String>,
        created_at: DateTime<Utc>,
    ) -> Self {
        Self {
            id,
            venue_type,
            name: name.into(),
            enabled: true,
            created_at,
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;
    use crate::ids::VenueId;

    #[test]
    fn simulated_venue_construction() {
        let v = Venue::new(
            VenueId::new("simulated").unwrap(),
            VenueType::Simulated,
            "Paper Simulator",
            Utc::now(),
        );
        assert!(v.enabled);
        assert_eq!(v.venue_type, VenueType::Simulated);
    }
}

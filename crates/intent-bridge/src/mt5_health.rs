use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HealthSignal {
    #[default]
    Unknown,
    Healthy,
    Degraded,
    Unhealthy,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct Mt5HealthSnapshot {
    pub transport: HealthSignal,
    pub authentication: HealthSignal,
    pub terminal_connectivity: HealthSignal,
    pub account_identity: HealthSignal,
    pub market_data_freshness: HealthSignal,
    pub database_persistence: HealthSignal,
    pub reconciliation: HealthSignal,
    pub recovery: HealthSignal,
}

impl Mt5HealthSnapshot {
    pub fn overall(&self) -> HealthSignal {
        let signals = [
            self.transport,
            self.authentication,
            self.terminal_connectivity,
            self.account_identity,
            self.market_data_freshness,
            self.database_persistence,
            self.reconciliation,
            self.recovery,
        ];

        if signals.contains(&HealthSignal::Unhealthy) {
            HealthSignal::Unhealthy
        } else if signals
            .iter()
            .all(|signal| *signal == HealthSignal::Healthy)
        {
            HealthSignal::Healthy
        } else {
            HealthSignal::Degraded
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{HealthSignal, Mt5HealthSnapshot};

    #[test]
    fn unknown_signals_never_report_healthy() {
        assert_eq!(
            Mt5HealthSnapshot::default().overall(),
            HealthSignal::Degraded
        );
    }

    #[test]
    fn all_signals_must_be_healthy_for_overall_health() {
        let healthy = Mt5HealthSnapshot {
            transport: HealthSignal::Healthy,
            authentication: HealthSignal::Healthy,
            terminal_connectivity: HealthSignal::Healthy,
            account_identity: HealthSignal::Healthy,
            market_data_freshness: HealthSignal::Healthy,
            database_persistence: HealthSignal::Healthy,
            reconciliation: HealthSignal::Healthy,
            recovery: HealthSignal::Healthy,
        };
        assert_eq!(healthy.overall(), HealthSignal::Healthy);
    }

    #[test]
    fn stale_market_data_degrades_health_without_changing_transport_signal() {
        let snapshot = Mt5HealthSnapshot {
            transport: HealthSignal::Healthy,
            authentication: HealthSignal::Healthy,
            terminal_connectivity: HealthSignal::Healthy,
            account_identity: HealthSignal::Healthy,
            market_data_freshness: HealthSignal::Degraded,
            database_persistence: HealthSignal::Healthy,
            reconciliation: HealthSignal::Healthy,
            recovery: HealthSignal::Healthy,
        };
        assert_eq!(snapshot.transport, HealthSignal::Healthy);
        assert_eq!(snapshot.overall(), HealthSignal::Degraded);
    }

    #[test]
    fn any_unhealthy_signal_fails_overall_health() {
        let snapshot = Mt5HealthSnapshot {
            database_persistence: HealthSignal::Unhealthy,
            ..Mt5HealthSnapshot::default()
        };
        assert_eq!(snapshot.overall(), HealthSignal::Unhealthy);
    }
}

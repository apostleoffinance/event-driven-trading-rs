//! Reconciler: fetch venue state, compare, emit alert-only events.

use chrono::Utc;
use events::{EventPublisher, TradingEvent};
use venue_connectors::VenueAdapter;

use crate::compare::{compare_snapshots, CompareTolerances};
use crate::error::{ReconciliationError, ReconciliationResult};
use crate::report::{InternalSnapshot, ReconciliationReport};

/// Stateless reconciler. Never mutates account, OMS, or venue books.
#[derive(Debug, Clone)]
pub struct Reconciler {
    tolerances: CompareTolerances,
}

impl Default for Reconciler {
    fn default() -> Self {
        Self::new()
    }
}

impl Reconciler {
    pub fn new() -> Self {
        Self {
            tolerances: CompareTolerances::default(),
        }
    }

    pub fn with_tolerances(tolerances: CompareTolerances) -> Self {
        Self { tolerances }
    }

    pub fn tolerances(&self) -> &CompareTolerances {
        &self.tolerances
    }

    /// Compare a prepared internal snapshot against already-fetched venue state.
    pub fn compare(
        &self,
        internal: &InternalSnapshot,
        external_state: &domain::AccountState,
        external_positions: &[domain::Position],
        external_orders: &[domain::Order],
    ) -> ReconciliationReport {
        let started_at = Utc::now();
        let breaks = compare_snapshots(
            &internal.account_state,
            &internal.positions,
            &internal.open_orders,
            external_state,
            external_positions,
            external_orders,
            &self.tolerances,
        );
        ReconciliationReport {
            account_id: internal.account_id.clone(),
            venue_id: internal.venue_id.clone(),
            breaks,
            started_at,
            completed_at: Utc::now(),
        }
    }

    /// Fetch venue state, compare, optionally publish Started / Alert* / Completed|Failed.
    ///
    /// On venue fetch failure: publishes `ReconciliationFailed` (if `events_out` set)
    /// and returns `Err`. Does **not** overwrite internal state on mismatches.
    pub async fn reconcile(
        &self,
        venue: &dyn VenueAdapter,
        internal: &InternalSnapshot,
        events_out: Option<&EventPublisher>,
    ) -> ReconciliationResult<ReconciliationReport> {
        if let Some(pub_) = events_out {
            pub_.publish(TradingEvent::ReconciliationStarted {
                account_id: internal.account_id.clone(),
                venue_id: internal.venue_id.clone(),
            })
            .await?;
        }

        let external_state = match venue.account_state(&internal.account_id).await {
            Ok(state) => state,
            Err(err) => {
                self.publish_failed(internal, events_out, err.to_string())
                    .await?;
                return Err(ReconciliationError::Venue(err));
            }
        };

        let external_positions = match venue.positions(&internal.account_id).await {
            Ok(positions) => positions,
            Err(err) => {
                self.publish_failed(internal, events_out, err.to_string())
                    .await?;
                return Err(ReconciliationError::Venue(err));
            }
        };

        let external_orders = match venue.open_orders(&internal.account_id).await {
            Ok(orders) => orders,
            Err(err) => {
                self.publish_failed(internal, events_out, err.to_string())
                    .await?;
                return Err(ReconciliationError::Venue(err));
            }
        };

        let report = self.compare(
            internal,
            &external_state,
            &external_positions,
            &external_orders,
        );

        if let Some(pub_) = events_out {
            for brk in &report.breaks {
                tracing::warn!(
                    account_id = %report.account_id,
                    venue_id = %report.venue_id,
                    detail = %brk.detail(),
                    "reconciliation alert (no silent overwrite)"
                );
                pub_.publish(TradingEvent::ReconciliationAlert {
                    account_id: report.account_id.clone(),
                    venue_id: report.venue_id.clone(),
                    detail: brk.detail(),
                })
                .await?;
            }

            pub_.publish(TradingEvent::ReconciliationCompleted {
                account_id: report.account_id.clone(),
                venue_id: report.venue_id.clone(),
            })
            .await?;
        }

        Ok(report)
    }

    async fn publish_failed(
        &self,
        internal: &InternalSnapshot,
        events_out: Option<&EventPublisher>,
        reason: String,
    ) -> ReconciliationResult<()> {
        if let Some(pub_) = events_out {
            pub_.publish(TradingEvent::ReconciliationFailed {
                account_id: internal.account_id.clone(),
                venue_id: internal.venue_id.clone(),
                reason,
            })
            .await?;
        }
        Ok(())
    }
}

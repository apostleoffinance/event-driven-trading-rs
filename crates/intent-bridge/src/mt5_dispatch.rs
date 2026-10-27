//! Durable MT5 dispatch coordinator, intentionally independent of sockets/EA transport.
//!
//! Safe pre-send path: session authorization -> durable claim -> one first-send
//! permit. Existing claims never produce a permit and must reconcile. A permit
//! is consumed when recording an outcome so transport errors cannot be retried
//! by reusing the same capability.

use chrono::{DateTime, Duration, Utc};
use persistence::{Mt5DurableOrderState, Mt5OrderClaim, TradingStore};

use crate::mt5_session::Mt5SessionGate;
use crate::mt5_v1::{BridgeFrame, BridgePayload, SubmitState};
use crate::{BridgeError, BridgeResult};

#[derive(Debug)]
pub enum Mt5DispatchDecision {
    /// New durable claim: this request may be sent once by the future transport.
    SendOnce(Mt5DispatchPermit),
    /// The durable key already existed; query/reconcile instead of resending.
    ReconcileRequired {
        client_order_id: String,
        prior_state: Mt5DurableOrderState,
        venue_order_id: Option<String>,
        reason_code: Option<String>,
    },
}

/// A single-send capability produced only after session and durable-claim checks.
/// Fields are private so callers cannot fabricate a permit from a raw frame.
#[derive(Debug)]
pub struct Mt5DispatchPermit {
    account_id: String,
    client_order_id: String,
    correlation_id: String,
    frame: BridgeFrame,
}

impl Mt5DispatchPermit {
    /// Frame to hand to the transport for the one authorized initial send.
    pub fn frame(&self) -> &BridgeFrame {
        &self.frame
    }

    pub fn account_id(&self) -> &str {
        &self.account_id
    }

    pub fn client_order_id(&self) -> &str {
        &self.client_order_id
    }

    /// Execute exactly one send through an authenticated MT5 connection, then persist
    /// its correlated outcome. Any transport ambiguity is durably recorded as Unknown,
    /// degrades the session, and consumes this permit; this method never retries.
    pub async fn execute_over_transport(
        self,
        connection: &mut crate::mt5_transport::AuthenticatedMt5Connection,
        store: &TradingStore,
        session: &mut Mt5SessionGate,
        allowed_clock_skew: Duration,
    ) -> BridgeResult<()> {
        // Re-check the session immediately before the first byte is sent. A watchdog
        // may have degraded the session after this permit was prepared.
        session.authorize_submit(self.frame())?;
        self.frame().validate_at(Utc::now(), allowed_clock_skew)?;
        if let Err(send_error) = connection.write_frame(self.frame()).await {
            self.record_ambiguous_transport_failure(store, session, "transport_write_failed")
                .await?;
            return Err(send_error);
        }

        let response = match connection
            .read_submit_outcome_with_session_checkpoint(store, session, allowed_clock_skew)
            .await
        {
            Ok(response) => response,
            Err(read_error) => {
                self.record_ambiguous_transport_failure(
                    store,
                    session,
                    "transport_response_ambiguous",
                )
                .await?;
                return Err(read_error);
            }
        };
        self.record_outcome(store, session, &response, Utc::now(), allowed_clock_skew)
            .await
    }

    /// Persist a response only if it is for this permit's account, correlation
    /// and stable client-order key. Any mismatch degrades the session.
    pub async fn record_outcome(
        self,
        store: &TradingStore,
        session: &mut Mt5SessionGate,
        outcome_frame: &BridgeFrame,
        now: DateTime<Utc>,
        allowed_clock_skew: Duration,
    ) -> BridgeResult<()> {
        if let Err(error) = validate_outcome_binding(
            &self.account_id,
            &self.client_order_id,
            &self.correlation_id,
            outcome_frame,
            now,
            allowed_clock_skew,
        ) {
            session.degrade();
            return Err(error);
        }

        let outcome = match &outcome_frame.payload {
            BridgePayload::SubmitOutcome(value) => value,
            _ => {
                session.degrade();
                return Err(BridgeError::Invalid(
                    "MT5 dispatch response must be a submit_outcome".into(),
                ));
            }
        };
        let state = durable_state(outcome.outcome);
        match store
            .record_mt5_order_outcome(
                &self.account_id,
                &self.client_order_id,
                state,
                outcome.venue_order_id.as_deref(),
                outcome.reason_code.as_deref(),
            )
            .await
        {
            Ok(()) => Ok(()),
            Err(error) => {
                // The transport may have committed the order; if we cannot
                // durably record the response, exposure must remain blocked.
                session.degrade();
                Err(BridgeError::from(error))
            }
        }
    }

    /// Persist a lost/malformed/ambiguous transport response as Unknown.
    /// This consumes the permit and deliberately provides no retry permission.
    pub async fn record_ambiguous_transport_failure(
        self,
        store: &TradingStore,
        session: &mut Mt5SessionGate,
        reason_code: &str,
    ) -> BridgeResult<()> {
        if reason_code.trim().is_empty() {
            session.degrade();
            return Err(BridgeError::Invalid(
                "ambiguous MT5 transport failure requires a reason code".into(),
            ));
        }
        match store
            .record_mt5_order_outcome(
                &self.account_id,
                &self.client_order_id,
                Mt5DurableOrderState::Unknown,
                None,
                Some(reason_code),
            )
            .await
        {
            Ok(()) => {
                session.degrade();
                Ok(())
            }
            Err(error) => {
                session.degrade();
                Err(BridgeError::from(error))
            }
        }
    }
}

/// Authorize and claim a submit before the transport is allowed to send it.
/// Existing claims, even Prepared, never produce a send permit.
pub async fn prepare_submit(
    store: &TradingStore,
    session: &Mt5SessionGate,
    frame: &BridgeFrame,
    now: DateTime<Utc>,
    allowed_clock_skew: Duration,
) -> BridgeResult<Mt5DispatchDecision> {
    let order = session.authorize_submit(frame)?;
    frame.validate_at(now, allowed_clock_skew)?;
    let client_order_id = order.client_order_id.clone();
    let account_id = frame.account_id.clone();
    let correlation_id = frame.correlation_id.clone();
    match crate::mt5_v1::claim_submit_order(store, frame, now, allowed_clock_skew).await? {
        Mt5OrderClaim::New => Ok(Mt5DispatchDecision::SendOnce(Mt5DispatchPermit {
            account_id,
            client_order_id,
            correlation_id,
            frame: frame.clone(),
        })),
        Mt5OrderClaim::Existing {
            state,
            venue_order_id,
            reason_code,
        } => Ok(Mt5DispatchDecision::ReconcileRequired {
            client_order_id,
            prior_state: state,
            venue_order_id,
            reason_code,
        }),
    }
}

fn durable_state(state: SubmitState) -> Mt5DurableOrderState {
    match state {
        SubmitState::Accepted => Mt5DurableOrderState::Accepted,
        SubmitState::Rejected => Mt5DurableOrderState::Rejected,
        SubmitState::Unknown => Mt5DurableOrderState::Unknown,
    }
}

fn validate_outcome_binding(
    account_id: &str,
    client_order_id: &str,
    correlation_id: &str,
    frame: &BridgeFrame,
    now: DateTime<Utc>,
    allowed_clock_skew: Duration,
) -> BridgeResult<()> {
    frame.validate_at(now, allowed_clock_skew)?;
    if frame.account_id != account_id || frame.correlation_id != correlation_id {
        return Err(BridgeError::Invalid(
            "MT5 submit outcome account/correlation does not match dispatch permit".into(),
        ));
    }
    let outcome = match &frame.payload {
        BridgePayload::SubmitOutcome(value) => value,
        _ => {
            return Err(BridgeError::Invalid(
                "MT5 dispatch response must be a submit_outcome".into(),
            ));
        }
    };
    if outcome.client_order_id != client_order_id {
        return Err(BridgeError::Invalid(
            "MT5 submit outcome client_order_id does not match dispatch permit".into(),
        ));
    }
    if outcome.observed_at > now + allowed_clock_skew {
        return Err(BridgeError::Invalid(
            "MT5 submit outcome observed_at is in the future".into(),
        ));
    }
    if outcome.outcome == SubmitState::Unknown
        && outcome.reason_code.as_deref().is_none_or(str::is_empty)
    {
        return Err(BridgeError::Invalid(
            "unknown MT5 submit outcome requires a reason code".into(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use super::*;
    use chrono::TimeZone;

    use crate::mt5_v1::{BridgeFrame, BridgePayload, SubmitOutcome, MT5_BRIDGE_SCHEMA_VERSION};

    fn now() -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 10, 27, 12, 0, 0).unwrap()
    }

    fn outcome_frame(
        account: &str,
        correlation: &str,
        client_order_id: &str,
        outcome: SubmitState,
    ) -> BridgeFrame {
        BridgeFrame {
            schema_version: MT5_BRIDGE_SCHEMA_VERSION,
            message_id: "outcome-msg".into(),
            correlation_id: correlation.into(),
            account_id: account.into(),
            sent_at: now(),
            payload: BridgePayload::SubmitOutcome(SubmitOutcome {
                client_order_id: client_order_id.into(),
                outcome,
                venue_order_id: Some("broker-ticket-7".into()),
                reason_code: (outcome == SubmitState::Unknown).then(|| "response_lost".into()),
                observed_at: now(),
            }),
        }
    }

    #[test]
    fn outcome_binding_rejects_wrong_account_correlation_or_order_id() {
        for (account, correlation, order_id) in [
            ("other-account", "submit-correlation", "stable-order-1"),
            ("account-1", "other-correlation", "stable-order-1"),
            ("account-1", "submit-correlation", "different-order"),
        ] {
            let frame = outcome_frame(account, correlation, order_id, SubmitState::Accepted);
            assert!(validate_outcome_binding(
                "account-1",
                "stable-order-1",
                "submit-correlation",
                &frame,
                now(),
                Duration::zero(),
            )
            .is_err());
        }
    }

    #[test]
    fn valid_outcome_must_be_timestamped_and_bound_to_the_permit() {
        let frame = outcome_frame(
            "account-1",
            "submit-correlation",
            "stable-order-1",
            SubmitState::Accepted,
        );
        assert!(validate_outcome_binding(
            "account-1",
            "stable-order-1",
            "submit-correlation",
            &frame,
            now(),
            Duration::zero(),
        )
        .is_ok());
    }

    #[test]
    fn durable_outcome_mapping_preserves_ambiguity() {
        assert_eq!(
            durable_state(SubmitState::Unknown),
            Mt5DurableOrderState::Unknown
        );
        assert_eq!(
            durable_state(SubmitState::Accepted),
            Mt5DurableOrderState::Accepted
        );
        assert_eq!(
            durable_state(SubmitState::Rejected),
            Mt5DurableOrderState::Rejected
        );
    }
}

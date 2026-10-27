//! Versioned, transport-neutral MT5 terminal bridge protocol.
//!
//! This module defines and validates messages only. It does not connect to MT5,
//! authorize risk, submit live orders, or interpret an acknowledgement as a fill.

use chrono::{DateTime, Duration, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use crate::{BridgeError, BridgeResult};

pub const MT5_BRIDGE_SCHEMA_VERSION: u16 = 1;
pub const MAX_MT5_BRIDGE_FRAME_BYTES: usize = 64 * 1024;

/// Incremental NDJSON decoder for arbitrarily chunked byte streams.
///
/// The decoder bounds memory before a newline arrives and validates each complete
/// frame before returning it. A malformed/oversized line poisons the decoder:
/// callers must close and recreate the session instead of attempting unsafe resync.
#[derive(Debug, Default)]
pub struct NdjsonFrameDecoder {
    buffer: Vec<u8>,
    poisoned: bool,
}

impl NdjsonFrameDecoder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(
        &mut self,
        chunk: &[u8],
        now: DateTime<Utc>,
        allowed_clock_skew: Duration,
    ) -> BridgeResult<Vec<BridgeFrame>> {
        if self.poisoned {
            return Err(BridgeError::Invalid(
                "MT5 NDJSON decoder is poisoned; reset the connection".into(),
            ));
        }

        let mut frames = Vec::new();
        for byte in chunk {
            if *byte == b'\n' {
                if self.buffer.last() == Some(&b'\r') {
                    self.buffer.pop();
                }
                if self.buffer.is_empty() {
                    self.poisoned = true;
                    return Err(BridgeError::Invalid(
                        "blank MT5 NDJSON frames are not allowed".into(),
                    ));
                }
                let line = match std::str::from_utf8(&self.buffer) {
                    Ok(line) => line,
                    Err(error) => {
                        self.poisoned = true;
                        return Err(BridgeError::Invalid(format!(
                            "MT5 NDJSON frame is not UTF-8: {error}"
                        )));
                    }
                };
                match parse_frame_json(line, now, allowed_clock_skew) {
                    Ok(frame) => frames.push(frame),
                    Err(error) => {
                        self.poisoned = true;
                        return Err(error);
                    }
                }
                self.buffer.clear();
            } else {
                self.buffer.push(*byte);
                if self.buffer.len() > MAX_MT5_BRIDGE_FRAME_BYTES {
                    self.poisoned = true;
                    return Err(BridgeError::Invalid(
                        "MT5 NDJSON frame exceeds 64 KiB limit".into(),
                    ));
                }
            }
        }
        Ok(frames)
    }

    /// Require a clean line boundary at EOF; partial frames are never executed.
    pub fn finish(&mut self) -> BridgeResult<()> {
        if self.poisoned {
            return Err(BridgeError::Invalid(
                "MT5 NDJSON decoder is poisoned; reset the connection".into(),
            ));
        }
        if !self.buffer.is_empty() {
            self.poisoned = true;
            return Err(BridgeError::Invalid(
                "connection ended with an incomplete MT5 NDJSON frame".into(),
            ));
        }
        Ok(())
    }
}

/// Decode and validate one UTF-8 JSON frame. Callers must frame NDJSON before invoking this.
pub fn parse_frame_json(
    json: &str,
    now: DateTime<Utc>,
    allowed_clock_skew: Duration,
) -> BridgeResult<BridgeFrame> {
    if json.len() > MAX_MT5_BRIDGE_FRAME_BYTES {
        return Err(BridgeError::Invalid(
            "MT5 bridge frame exceeds 64 KiB limit".into(),
        ));
    }
    let frame: BridgeFrame = serde_json::from_str(json)?;
    frame.validate_at(now, allowed_clock_skew)?;
    Ok(frame)
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BridgeFrame {
    pub schema_version: u16,
    /// Unique ID for this transport message; retries may resend the same ID.
    pub message_id: String,
    /// Links responses/events to the initiating request where applicable.
    pub correlation_id: String,
    /// Explicit account identity, not a terminal/server identity.
    pub account_id: String,
    pub sent_at: DateTime<Utc>,
    pub payload: BridgePayload,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum BridgePayload {
    Heartbeat(Heartbeat),
    SubmitOrder(SubmitOrder),
    SubmitOutcome(SubmitOutcome),
    CancelOrder(CancelOrder),
    CancelOutcome(CancelOutcome),
    OrderSnapshot(OrderSnapshot),
    ReconcileRequest(ReconcileRequest),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Heartbeat {
    pub sequence: u64,
    pub terminal_id: String,
    pub server: String,
    pub terminal_time: DateTime<Utc>,
    pub connection: ConnectionState,
    pub capabilities: BridgeCapabilities,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConnectionState {
    Connected,
    Degraded,
    Disconnected,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BridgeCapabilities {
    pub market_orders: bool,
    pub limit_orders: bool,
    pub stop_orders: bool,
    pub cancel_requests: bool,
    pub order_lookup: bool,
    pub idempotent_client_order_id: bool,
    pub sequence_numbers: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SubmitOrder {
    /// Stable across retries; identifies the order, not the transport message.
    pub client_order_id: String,
    pub instrument_id: String,
    pub side: OrderSide,
    pub order_type: OrderType,
    pub quantity: String,
    pub price: Option<String>,
    pub stop_loss: Option<String>,
    pub take_profit: Option<String>,
    pub time_in_force: TimeInForce,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OrderSide {
    Buy,
    Sell,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OrderType {
    Market,
    Limit,
    Stop,
    StopLimit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TimeInForce {
    Day,
    Gtc,
    Ioc,
    Fok,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SubmitOutcome {
    pub client_order_id: String,
    pub outcome: SubmitState,
    pub venue_order_id: Option<String>,
    pub reason_code: Option<String>,
    pub observed_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SubmitState {
    Accepted,
    Rejected,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CancelOrder {
    pub client_order_id: String,
    pub venue_order_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CancelOutcome {
    pub client_order_id: String,
    pub request_state: CancelRequestState,
    /// Only set when a venue query/event confirms a terminal state.
    pub terminal_state: Option<TerminalOrderState>,
    pub observed_at: DateTime<Utc>,
    pub reason_code: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CancelRequestState {
    Requested,
    Rejected,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TerminalOrderState {
    Filled,
    Cancelled,
    Expired,
    Rejected,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OrderSnapshot {
    pub client_order_id: String,
    pub venue_order_id: Option<String>,
    pub sequence: Option<u64>,
    pub status: OrderSnapshotState,
    pub filled_quantity: String,
    pub average_fill_price: Option<String>,
    pub source_time: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OrderSnapshotState {
    Pending,
    Accepted,
    PartiallyFilled,
    Filled,
    CancelPending,
    Cancelled,
    Rejected,
    Expired,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReconcileRequest {
    pub client_order_id: String,
}

impl BridgeFrame {
    /// Validate protocol invariants using an injected clock and allowed skew.
    /// This keeps tests deterministic and prevents a wall clock from being
    /// hidden inside protocol validation.
    pub fn validate_at(
        &self,
        now: DateTime<Utc>,
        allowed_clock_skew: Duration,
    ) -> BridgeResult<()> {
        if self.schema_version != MT5_BRIDGE_SCHEMA_VERSION {
            return Err(BridgeError::Invalid(format!(
                "unsupported MT5 bridge schema version {}",
                self.schema_version
            )));
        }
        if self.message_id.trim().is_empty()
            || self.correlation_id.trim().is_empty()
            || self.account_id.trim().is_empty()
        {
            return Err(BridgeError::Invalid(
                "message_id, correlation_id and account_id are required".into(),
            ));
        }
        if allowed_clock_skew < Duration::zero() || self.sent_at > now + allowed_clock_skew {
            return Err(BridgeError::Invalid(
                "message timestamp is in the future".into(),
            ));
        }
        match &self.payload {
            BridgePayload::Heartbeat(heartbeat) => {
                require_text(&heartbeat.terminal_id, "terminal_id")?;
                require_text(&heartbeat.server, "server")?;
            }
            BridgePayload::SubmitOrder(order) => {
                require_text(&order.client_order_id, "client_order_id")?;
                require_text(&order.instrument_id, "instrument_id")?;
                let quantity = decimal(&order.quantity, "quantity")?;
                if quantity <= Decimal::ZERO {
                    return Err(BridgeError::Invalid("quantity must be positive".into()));
                }
                for (field, value) in [
                    ("price", order.price.as_deref()),
                    ("stop_loss", order.stop_loss.as_deref()),
                    ("take_profit", order.take_profit.as_deref()),
                ] {
                    if let Some(value) = value {
                        if decimal(value, field)? <= Decimal::ZERO {
                            return Err(BridgeError::Invalid(format!("{field} must be positive")));
                        }
                    }
                }
                match order.order_type {
                    OrderType::Market if order.price.is_some() => {
                        return Err(BridgeError::Invalid(
                            "market order must not specify price".into(),
                        ));
                    }
                    OrderType::Limit | OrderType::Stop if order.price.is_none() => {
                        return Err(BridgeError::Invalid(
                            "limit/stop order requires price".into(),
                        ));
                    }
                    OrderType::StopLimit if order.price.is_none() => {
                        return Err(BridgeError::Invalid(
                            "stop-limit order requires price".into(),
                        ));
                    }
                    _ => {}
                }
            }
            BridgePayload::SubmitOutcome(outcome) => {
                require_text(&outcome.client_order_id, "client_order_id")?;
                if outcome.outcome == SubmitState::Unknown && outcome.reason_code.is_none() {
                    return Err(BridgeError::Invalid(
                        "unknown submit outcome requires reason_code".into(),
                    ));
                }
            }
            BridgePayload::CancelOrder(cancel) => {
                require_text(&cancel.client_order_id, "client_order_id")?
            }
            BridgePayload::CancelOutcome(outcome) => {
                require_text(&outcome.client_order_id, "client_order_id")?;
                if outcome.request_state != CancelRequestState::Requested
                    && outcome.terminal_state.is_some()
                {
                    return Err(BridgeError::Invalid(
                        "terminal state requires an accepted cancel request or separate order snapshot".into(),
                    ));
                }
            }
            BridgePayload::OrderSnapshot(snapshot) => {
                require_text(&snapshot.client_order_id, "client_order_id")?;
                if decimal(&snapshot.filled_quantity, "filled_quantity")? < Decimal::ZERO {
                    return Err(BridgeError::Invalid(
                        "filled_quantity must be non-negative".into(),
                    ));
                }
                if let Some(price) = snapshot.average_fill_price.as_deref() {
                    if decimal(price, "average_fill_price")? <= Decimal::ZERO {
                        return Err(BridgeError::Invalid(
                            "average_fill_price must be positive".into(),
                        ));
                    }
                }
            }
            BridgePayload::ReconcileRequest(request) => {
                require_text(&request.client_order_id, "client_order_id")?
            }
        }
        Ok(())
    }
}

fn require_text(value: &str, field: &str) -> BridgeResult<()> {
    if value.trim().is_empty() {
        return Err(BridgeError::Invalid(format!("{field} must not be empty")));
    }
    Ok(())
}

fn decimal(raw: &str, field: &str) -> BridgeResult<Decimal> {
    let unsigned = raw.strip_prefix('-').unwrap_or(raw);
    let valid_integer = unsigned
        .split_once('.')
        .map(|(integer, fraction)| {
            !integer.is_empty()
                && !fraction.is_empty()
                && integer.chars().all(|ch| ch.is_ascii_digit())
                && fraction.chars().all(|ch| ch.is_ascii_digit())
                && (integer == "0" || !integer.starts_with('0'))
        })
        .unwrap_or_else(|| {
            unsigned.chars().all(|ch| ch.is_ascii_digit())
                && (unsigned == "0" || !unsigned.starts_with('0'))
        });
    if raw.trim() != raw
        || raw.is_empty()
        || raw.starts_with('+')
        || raw.starts_with('.')
        || raw.contains('e')
        || raw.contains('E')
        || !valid_integer
    {
        return Err(BridgeError::Invalid(format!(
            "{field} must be a canonical, unpadded base-10 decimal string"
        )));
    }
    Decimal::from_str_exact(raw)
        .map_err(|error| BridgeError::Invalid(format!("{field}: invalid decimal string: {error}")))
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use super::*;
    use chrono::TimeZone;

    fn frame(payload: BridgePayload) -> BridgeFrame {
        BridgeFrame {
            schema_version: MT5_BRIDGE_SCHEMA_VERSION,
            message_id: "msg-0001".into(),
            correlation_id: "corr-0001".into(),
            account_id: "account-001".into(),
            sent_at: Utc.with_ymd_and_hms(2026, 10, 27, 12, 0, 0).unwrap(),
            payload,
        }
    }

    fn valid_order() -> SubmitOrder {
        SubmitOrder {
            client_order_id: "co-001".into(),
            instrument_id: "EURUSD".into(),
            side: OrderSide::Buy,
            order_type: OrderType::Market,
            quantity: "0.01".into(),
            price: None,
            stop_loss: Some("1.08000".into()),
            take_profit: Some("1.09000".into()),
            time_in_force: TimeInForce::Day,
        }
    }

    #[test]
    fn ndjson_decoder_handles_split_and_multiple_frames() {
        let now = Utc.with_ymd_and_hms(2026, 10, 27, 12, 0, 1).unwrap();
        let one = serde_json::to_string(&frame(BridgePayload::SubmitOrder(valid_order()))).unwrap();
        let two = serde_json::to_string(&frame(BridgePayload::Heartbeat(Heartbeat {
            sequence: 1,
            terminal_id: "terminal-01".into(),
            server: "broker-demo".into(),
            terminal_time: now,
            connection: ConnectionState::Connected,
            capabilities: BridgeCapabilities {
                market_orders: true,
                limit_orders: true,
                stop_orders: true,
                cancel_requests: true,
                order_lookup: true,
                idempotent_client_order_id: true,
                sequence_numbers: true,
            },
        })))
        .unwrap();
        let mut decoder = NdjsonFrameDecoder::new();
        assert!(decoder
            .push(&one.as_bytes()[..10], now, Duration::zero())
            .unwrap()
            .is_empty());
        let chunk = format!("{}\n{}\r\n", &one[10..], two);
        let decoded = decoder
            .push(chunk.as_bytes(), now, Duration::seconds(2))
            .unwrap();
        assert_eq!(decoded.len(), 2);
        assert!(decoder.finish().is_ok());
    }

    #[test]
    fn ndjson_decoder_rejects_oversized_partial_and_poisoned_streams() {
        let now = Utc.with_ymd_and_hms(2026, 10, 27, 12, 0, 0).unwrap();
        let mut oversized = NdjsonFrameDecoder::new();
        assert!(oversized
            .push(
                &vec![b'x'; MAX_MT5_BRIDGE_FRAME_BYTES + 1],
                now,
                Duration::zero()
            )
            .is_err());
        assert!(oversized.push(b"{}\n", now, Duration::zero()).is_err());

        let mut partial = NdjsonFrameDecoder::new();
        assert!(partial
            .push(b"{", now, Duration::zero())
            .unwrap()
            .is_empty());
        assert!(partial.finish().is_err());
        assert!(partial.push(b"}\n", now, Duration::zero()).is_err());
        assert!(partial.push(b"{}\n", now, Duration::zero()).is_err());
    }

    #[test]
    fn ndjson_decoder_rejects_blank_and_non_utf8_lines() {
        let now = Utc.with_ymd_and_hms(2026, 10, 27, 12, 0, 0).unwrap();
        let mut blank = NdjsonFrameDecoder::new();
        assert!(blank.push(b"\n", now, Duration::zero()).is_err());
        let mut invalid_utf8 = NdjsonFrameDecoder::new();
        assert!(invalid_utf8
            .push(&[0xff, b'\n'], now, Duration::zero())
            .is_err());
    }

    #[test]
    fn accepts_decimal_string_order_and_round_trips_json() {
        let frame = frame(BridgePayload::SubmitOrder(valid_order()));
        let now = Utc.with_ymd_and_hms(2026, 10, 27, 12, 0, 1).unwrap();
        frame.validate_at(now, Duration::seconds(2)).unwrap();
        let json = serde_json::to_string(&frame).unwrap();
        let decoded: BridgeFrame = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded, frame);
    }

    #[test]
    fn rejects_unsupported_version_and_future_timestamp() {
        let now = Utc.with_ymd_and_hms(2026, 10, 27, 12, 0, 0).unwrap();
        let mut unsupported = frame(BridgePayload::SubmitOrder(valid_order()));
        unsupported.schema_version = 2;
        assert!(unsupported.validate_at(now, Duration::zero()).is_err());

        let mut future = frame(BridgePayload::SubmitOrder(valid_order()));
        future.sent_at = now + Duration::seconds(3);
        assert!(future.validate_at(now, Duration::seconds(2)).is_err());
    }

    #[test]
    fn rejects_noncanonical_decimal_lexemes() {
        let now = Utc.with_ymd_and_hms(2026, 10, 27, 12, 0, 0).unwrap();
        for quantity in ["1e-2", "01.0", ".1", "-.1", " 0.1", "0."] {
            let mut order = valid_order();
            order.quantity = quantity.to_string();
            assert!(
                frame(BridgePayload::SubmitOrder(order))
                    .validate_at(now, Duration::zero())
                    .is_err(),
                "accepted invalid decimal {quantity}"
            );
        }
    }

    #[test]
    fn parser_rejects_oversized_frames_before_deserialization() {
        let oversized = " ".repeat(MAX_MT5_BRIDGE_FRAME_BYTES + 1);
        let now = Utc.with_ymd_and_hms(2026, 10, 27, 12, 0, 0).unwrap();
        assert!(parse_frame_json(&oversized, now, Duration::zero()).is_err());
    }

    #[test]
    fn rejects_binary_float_style_numbers_and_invalid_quantity() {
        let now = Utc.with_ymd_and_hms(2026, 10, 27, 12, 0, 0).unwrap();
        let mut order = valid_order();
        order.quantity = "1e-2".into();
        assert!(frame(BridgePayload::SubmitOrder(order))
            .validate_at(now, Duration::zero())
            .is_err());

        let mut order = valid_order();
        order.quantity = "0".into();
        assert!(frame(BridgePayload::SubmitOrder(order))
            .validate_at(now, Duration::zero())
            .is_err());
    }

    #[test]
    fn unknown_submit_is_not_rejected_or_safe_to_retry() {
        let now = Utc.with_ymd_and_hms(2026, 10, 27, 12, 0, 0).unwrap();
        let outcome = SubmitOutcome {
            client_order_id: "co-001".into(),
            outcome: SubmitState::Unknown,
            venue_order_id: None,
            reason_code: Some("response_lost".into()),
            observed_at: now,
        };
        assert!(frame(BridgePayload::SubmitOutcome(outcome))
            .validate_at(now, Duration::zero())
            .is_ok());
    }

    #[test]
    fn cancel_request_ack_is_not_terminal_cancellation() {
        let now = Utc.with_ymd_and_hms(2026, 10, 27, 12, 0, 0).unwrap();
        let outcome = CancelOutcome {
            client_order_id: "co-001".into(),
            request_state: CancelRequestState::Requested,
            terminal_state: None,
            observed_at: now,
            reason_code: None,
        };
        assert!(frame(BridgePayload::CancelOutcome(outcome))
            .validate_at(now, Duration::zero())
            .is_ok());
    }
}

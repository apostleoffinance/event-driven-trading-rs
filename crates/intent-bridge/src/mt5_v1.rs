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

/// Persist a submit claim before any transport send. Only `New` authorizes
/// a first send; every existing key, including `prepared`, must reconcile.
pub async fn claim_submit_order(
    store: &persistence::TradingStore,
    frame: &BridgeFrame,
    now: DateTime<Utc>,
    allowed_clock_skew: Duration,
) -> BridgeResult<persistence::Mt5OrderClaim> {
    frame.validate_at(now, allowed_clock_skew)?;
    let order = match &frame.payload {
        BridgePayload::SubmitOrder(order) => order,
        _ => {
            return Err(BridgeError::Invalid(
                "durable submit claim requires a submit_order payload".into(),
            ));
        }
    };
    let canonical_payload = serde_json::to_value(&frame.payload)?;
    store
        .claim_mt5_order(&frame.account_id, &order.client_order_id, canonical_payload)
        .await
        .map_err(BridgeError::from)
}

/// Persist accepted/rejected/unknown response state. Unknown is deliberately
/// not treated as rejected and must be reconciled before any further submit.
pub async fn persist_submit_outcome(
    store: &persistence::TradingStore,
    frame: &BridgeFrame,
    now: DateTime<Utc>,
    allowed_clock_skew: Duration,
) -> BridgeResult<()> {
    frame.validate_at(now, allowed_clock_skew)?;
    let outcome = match &frame.payload {
        BridgePayload::SubmitOutcome(outcome) => outcome,
        _ => {
            return Err(BridgeError::Invalid(
                "persisted submit outcome requires a submit_outcome payload".into(),
            ));
        }
    };
    let state = match outcome.outcome {
        SubmitState::Accepted => persistence::Mt5DurableOrderState::Accepted,
        SubmitState::Rejected => persistence::Mt5DurableOrderState::Rejected,
        SubmitState::Unknown => persistence::Mt5DurableOrderState::Unknown,
    };
    store
        .record_mt5_order_outcome(
            &frame.account_id,
            &outcome.client_order_id,
            state,
            outcome.venue_order_id.as_deref(),
            outcome.reason_code.as_deref(),
        )
        .await
        .map_err(BridgeError::from)
}

/// Observe a heartbeat sequence in durable storage. Non-connected heartbeats
/// are never allowed to establish a ready stream; callers must reconcile first.
pub async fn observe_heartbeat_sequence(
    store: &persistence::TradingStore,
    frame: &BridgeFrame,
    now: DateTime<Utc>,
    allowed_clock_skew: Duration,
) -> BridgeResult<persistence::Mt5SequenceObservation> {
    let heartbeat = match &frame.payload {
        BridgePayload::Heartbeat(heartbeat) => heartbeat,
        _ => {
            return Err(BridgeError::Invalid(
                "sequence observation requires a heartbeat payload".into(),
            ));
        }
    };
    if let Err(validation_error) = frame.validate_at(now, allowed_clock_skew) {
        store
            .mark_mt5_stream_degraded(
                &frame.account_id,
                &heartbeat.terminal_id,
                heartbeat.sequence,
            )
            .await?;
        return Err(validation_error);
    }
    if heartbeat.connection != ConnectionState::Connected {
        store
            .mark_mt5_stream_degraded(
                &frame.account_id,
                &heartbeat.terminal_id,
                heartbeat.sequence,
            )
            .await?;
        return Err(BridgeError::Invalid(
            "non-connected MT5 heartbeat cannot authorize sequence readiness".into(),
        ));
    }
    store
        .observe_mt5_sequence(
            &frame.account_id,
            &heartbeat.terminal_id,
            heartbeat.sequence,
        )
        .await
        .map_err(BridgeError::from)
}

/// Mark readiness only after external orders, positions, and account state have
/// been reconciled for this account/terminal pair.
pub async fn mark_heartbeat_stream_reconciled(
    store: &persistence::TradingStore,
    account_id: &str,
    terminal_id: &str,
    reconciled_sequence: u64,
) -> BridgeResult<()> {
    store
        .mark_mt5_stream_reconciled(account_id, terminal_id, reconciled_sequence)
        .await
        .map_err(BridgeError::from)
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
    ReconciliationSnapshot(ReconciliationSnapshotWire),
    MarketDataSnapshot(MarketDataSnapshotWire),
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReconciliationSnapshotWire {
    pub terminal_id: String,
    pub server: String,
    pub source: String,
    pub sequence: u64,
    pub captured_at: DateTime<Utc>,
    pub account_state: ReconciliationAccountStateWire,
    pub open_orders: Vec<ReconciliationOpenOrderWire>,
    pub positions: Vec<ReconciliationPositionWire>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MarketDataSnapshotWire {
    pub terminal_id: String,
    pub server: String,
    pub source: String,
    pub sequence: u64,
    pub captured_at: DateTime<Utc>,
    pub account_currency: String,
    pub instruments: Vec<InstrumentMarketDataWire>,
    pub quotes: Vec<QuoteWire>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InstrumentMarketDataWire {
    pub symbol: String,
    pub base_currency: String,
    pub quote_currency: String,
    pub digits: u32,
    pub point: String,
    pub tick_size: String,
    pub tick_value_account_currency: String,
    pub contract_size: String,
    pub volume_min: String,
    pub volume_max: String,
    pub volume_step: String,
    pub trade_mode: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct QuoteWire {
    pub symbol: String,
    pub bid: String,
    pub ask: String,
    pub tick_time: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReconciliationAccountStateWire {
    pub currency: String,
    pub balance: String,
    pub equity: String,
    pub margin_used: String,
    pub free_margin: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReconciliationOpenOrderWire {
    pub venue_order_id: String,
    pub client_order_id: Option<String>,
    pub instrument_id: String,
    pub side: String,
    pub order_type: String,
    pub quantity: String,
    pub status: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReconciliationPositionWire {
    pub venue_position_id: String,
    pub instrument_id: String,
    pub side: String,
    pub quantity: String,
    pub average_price: String,
    pub unrealized_pnl: String,
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
            BridgePayload::MarketDataSnapshot(snapshot) => {
                require_text(&snapshot.terminal_id, "terminal_id")?;
                require_text(&snapshot.server, "server")?;
                require_text(&snapshot.source, "source")?;
                require_text(&snapshot.account_currency, "account currency")?;
                if snapshot.instruments.is_empty()
                    || snapshot.instruments.len() != snapshot.quotes.len()
                    || snapshot.instruments.len() > 64
                {
                    return Err(BridgeError::Invalid(
                        "market-data snapshot must contain 1–64 matched instrument/quote pairs".into(),
                    ));
                }
                let mut symbols = std::collections::BTreeSet::new();
                for instrument in &snapshot.instruments {
                    require_text(&instrument.symbol, "instrument symbol")?;
                    require_text(&instrument.base_currency, "base currency")?;
                    require_text(&instrument.quote_currency, "quote currency")?;
                    require_text(&instrument.trade_mode, "trade mode")?;
                    if !symbols.insert(instrument.symbol.as_str()) {
                        return Err(BridgeError::Invalid(
                            "market-data snapshot contains duplicate instrument symbols".into(),
                        ));
                    }
                    for (field, value) in [
                        ("point", instrument.point.as_str()),
                        ("tick_size", instrument.tick_size.as_str()),
                        ("tick_value_account_currency", instrument.tick_value_account_currency.as_str()),
                        ("contract_size", instrument.contract_size.as_str()),
                        ("volume_min", instrument.volume_min.as_str()),
                        ("volume_max", instrument.volume_max.as_str()),
                        ("volume_step", instrument.volume_step.as_str()),
                    ] {
                        if decimal(value, field)? <= Decimal::ZERO {
                            return Err(BridgeError::Invalid(format!("{field} must be positive")));
                        }
                    }
                    if instrument.digits > 16 {
                        return Err(BridgeError::Invalid("instrument digits exceed supported precision".into()));
                    }
                    let volume_min = decimal(&instrument.volume_min, "volume_min")?;
                    let volume_max = decimal(&instrument.volume_max, "volume_max")?;
                    let volume_step = decimal(&instrument.volume_step, "volume_step")?;
                    if volume_max < volume_min || volume_step > volume_max {
                        return Err(BridgeError::Invalid(
                            "instrument volume bounds or step are inconsistent".into(),
                        ));
                    }
                }
                let mut quote_symbols = std::collections::BTreeSet::new();
                for quote in &snapshot.quotes {
                    require_text(&quote.symbol, "quote symbol")?;
                    if !quote_symbols.insert(quote.symbol.as_str())
                        || !symbols.contains(quote.symbol.as_str())
                    {
                        return Err(BridgeError::Invalid(
                            "market-data quote symbols must uniquely match instrument metadata".into(),
                        ));
                    }
                    let bid = decimal(&quote.bid, "bid")?;
                    let ask = decimal(&quote.ask, "ask")?;
                    if bid <= Decimal::ZERO || ask < bid {
                        return Err(BridgeError::Invalid(
                            "quote bid must be positive and ask must be greater than or equal to bid".into(),
                        ));
                    }
                    if quote.tick_time > now + allowed_clock_skew
                        || now.signed_duration_since(quote.tick_time) > Duration::seconds(60)
                    {
                        return Err(BridgeError::Invalid(
                            "market-data quote is stale or timestamped in the future".into(),
                        ));
                    }
                }
            }
            BridgePayload::ReconciliationSnapshot(snapshot) => {
                require_text(&snapshot.terminal_id, "terminal_id")?;
                require_text(&snapshot.server, "server")?;
                require_text(&snapshot.source, "source")?;
                require_text(&snapshot.account_state.currency, "account currency")?;
                for (field, value) in [
                    ("balance", snapshot.account_state.balance.as_str()),
                    ("equity", snapshot.account_state.equity.as_str()),
                    ("margin_used", snapshot.account_state.margin_used.as_str()),
                    ("free_margin", snapshot.account_state.free_margin.as_str()),
                ] {
                    decimal(value, field)?;
                }
                let mut order_ids = std::collections::BTreeSet::new();
                for order in &snapshot.open_orders {
                    require_text(&order.venue_order_id, "venue_order_id")?;
                    require_text(&order.instrument_id, "instrument_id")?;
                    require_text(&order.side, "order side")?;
                    require_text(&order.order_type, "order type")?;
                    require_text(&order.status, "order status")?;
                    if !order_ids.insert(order.venue_order_id.as_str())
                        || decimal(&order.quantity, "order quantity")? <= Decimal::ZERO
                    {
                        return Err(BridgeError::Invalid(
                            "snapshot contains duplicate order IDs or invalid order quantity"
                                .into(),
                        ));
                    }
                    if order
                        .client_order_id
                        .as_deref()
                        .is_some_and(|id| id.trim().is_empty())
                    {
                        return Err(BridgeError::Invalid(
                            "snapshot contains empty client order ID".into(),
                        ));
                    }
                }
                let mut position_ids = std::collections::BTreeSet::new();
                for position in &snapshot.positions {
                    require_text(&position.venue_position_id, "venue_position_id")?;
                    require_text(&position.instrument_id, "instrument_id")?;
                    require_text(&position.side, "position side")?;
                    if !position_ids.insert(position.venue_position_id.as_str())
                        || decimal(&position.quantity, "position quantity")? <= Decimal::ZERO
                    {
                        return Err(BridgeError::Invalid(
                            "snapshot contains duplicate position IDs or invalid quantity".into(),
                        ));
                    }
                    decimal(&position.average_price, "average_price")?;
                    decimal(&position.unrealized_pnl, "unrealized_pnl")?;
                }
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

    #[test]
    fn market_data_snapshot_requires_matching_symbols_and_fresh_valid_quotes() {
        let now = Utc.with_ymd_and_hms(2026, 10, 27, 12, 0, 0).unwrap();
        let instrument = InstrumentMarketDataWire {
            symbol: "EURUSD.a".into(),
            base_currency: "EUR".into(),
            quote_currency: "USD".into(),
            digits: 5,
            point: "0.00001".into(),
            tick_size: "0.00001".into(),
            tick_value_account_currency: "1.0".into(),
            contract_size: "100000".into(),
            volume_min: "0.01".into(),
            volume_max: "100".into(),
            volume_step: "0.01".into(),
            trade_mode: "4".into(),
        };
        let quote = QuoteWire {
            symbol: "EURUSD.a".into(),
            bid: "1.0845".into(),
            ask: "1.08462".into(),
            tick_time: now.clone(),
        };
        let payload = MarketDataSnapshotWire {
            terminal_id: "terminal-1".into(),
            server: "Broker-Demo".into(),
            source: "mt5_python_readonly_v1".into(),
            sequence: 7,
            captured_at: now.clone(),
            account_currency: "USD".into(),
            instruments: vec![instrument.clone()],
            quotes: vec![quote.clone()],
        };
        assert!(frame(BridgePayload::MarketDataSnapshot(payload.clone()))
            .validate_at(now.clone(), Duration::zero()).is_ok());

        let mut stale = payload.clone();
        stale.quotes[0].tick_time = now - Duration::seconds(61);
        assert!(frame(BridgePayload::MarketDataSnapshot(stale))
            .validate_at(now.clone(), Duration::zero()).is_err());

        let mut crossed = payload.clone();
        crossed.quotes[0].ask = "1.0844".into();
        assert!(frame(BridgePayload::MarketDataSnapshot(crossed))
            .validate_at(now, Duration::zero()).is_err());

        let mut unmatched = payload;
        unmatched.quotes[0].symbol = "EURUSD".into();
        assert!(frame(BridgePayload::MarketDataSnapshot(unmatched))
            .validate_at(now, Duration::zero()).is_err());
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
    fn reconciliation_snapshot_round_trips_with_decimal_strings() {
        let now = Utc.with_ymd_and_hms(2026, 10, 27, 12, 0, 0).unwrap();
        let snapshot = ReconciliationSnapshotWire {
            terminal_id: "terminal-demo-1".into(),
            server: "Broker-Demo".into(),
            source: "mt5_python_readonly_v1".into(),
            sequence: 8,
            captured_at: now,
            account_state: ReconciliationAccountStateWire {
                currency: "USD".into(),
                balance: "10000.00".into(),
                equity: "9990.50".into(),
                margin_used: "100.00".into(),
                free_margin: "9890.50".into(),
            },
            open_orders: vec![ReconciliationOpenOrderWire {
                venue_order_id: "77".into(),
                client_order_id: None,
                instrument_id: "EURUSD.a".into(),
                side: "buy".into(),
                order_type: "buy_limit".into(),
                quantity: "0.10".into(),
                status: "1".into(),
            }],
            positions: vec![ReconciliationPositionWire {
                venue_position_id: "88".into(),
                instrument_id: "EURUSD.a".into(),
                side: "buy".into(),
                quantity: "0.10".into(),
                average_price: "1.08000".into(),
                unrealized_pnl: "-9.50".into(),
            }],
        };
        let frame = frame(BridgePayload::ReconciliationSnapshot(snapshot));
        frame.validate_at(now, Duration::zero()).unwrap();
        let json = serde_json::to_string(&frame).unwrap();
        let decoded: BridgeFrame = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded, frame);
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

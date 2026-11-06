//! Latest persisted MT5 EURUSD quote (read-only).
//!
//! Reads the single latest snapshot for one server-configured account/terminal
//! and exposes only a narrow quote DTO. Identity fields, raw snapshot JSON and
//! database details never leave this module. Freshness is derived from the
//! quote's own `tick_time`, never from the daemon's persistence activity.

use std::fmt;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use chrono::{DateTime, SecondsFormat, Utc};
use rust_decimal::Decimal;
use serde::Serialize;
use serde_json::Value;
use sqlx::postgres::{PgConnectOptions, PgPoolOptions, PgSslMode};

use crate::error::MarketDataError;
use crate::model::{DataClass, ProviderId};
use crate::service::Clock;

pub const MT5_SYMBOL: &str = "EURUSD";
pub const DEFAULT_STALE_AFTER_SECONDS: u64 = 60;
/// Same future tolerance the MT5 worker applies to quote timestamps.
const MAX_FUTURE_TOLERANCE_MS: i64 = 5_000;

const PROVIDER: ProviderId = ProviderId::Mt5Bridge;

/// Server-side scope. Never taken from a request.
#[derive(Clone, PartialEq, Eq)]
pub struct Mt5Scope {
    pub account_id: String,
    pub terminal_id: String,
}

impl fmt::Debug for Mt5Scope {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Mt5Scope(<redacted>)")
    }
}

/// Snapshot payload without any account/terminal/server identity.
#[derive(Clone)]
pub struct Mt5SnapshotRecord {
    pub sequence: u64,
    pub captured_at: DateTime<Utc>,
    pub snapshot: Value,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceError {
    Unavailable,
    Timeout,
    /// The stored record failed the repository's own integrity checks.
    Corrupt,
}

#[allow(clippy::double_must_use)]
#[async_trait]
pub trait Mt5SnapshotSource: Send + Sync {
    async fn load_latest(&self, scope: &Mt5Scope)
        -> Result<Option<Mt5SnapshotRecord>, SourceError>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum QuoteStatus {
    Fresh,
    Stale,
}

#[derive(Debug, Clone, Serialize)]
pub struct Mt5LatestQuote {
    pub status: QuoteStatus,
    /// Source of the data (configured MT5 pipeline). Says nothing about freshness.
    pub classification: DataClass,
    pub symbol: &'static str,
    pub bid: String,
    pub ask: String,
    pub spread: String,
    pub tick_time: String,
    pub captured_at: String,
    pub sequence: u64,
    /// `now - tick_time`. Not clamped: a tick inside the future tolerance is negative.
    pub quote_age_seconds: f64,
    /// `now - captured_at`. Not clamped.
    pub snapshot_age_seconds: f64,
    pub is_stale: bool,
    pub stale_after_seconds: u64,
}

pub struct Mt5LatestService {
    source: Option<Arc<dyn Mt5SnapshotSource>>,
    scope: Option<Mt5Scope>,
    clock: Arc<dyn Clock>,
    stale_after_seconds: u64,
    query_timeout: Duration,
}

impl Mt5LatestService {
    pub fn new(
        source: Option<Arc<dyn Mt5SnapshotSource>>,
        scope: Option<Mt5Scope>,
        clock: Arc<dyn Clock>,
        stale_after_seconds: u64,
        query_timeout: Duration,
    ) -> Self {
        Self {
            source,
            scope,
            clock,
            stale_after_seconds,
            query_timeout,
        }
    }

    /// No database or scope configured: every request reports unavailable.
    pub fn unconfigured(clock: Arc<dyn Clock>) -> Self {
        Self::new(
            None,
            None,
            clock,
            DEFAULT_STALE_AFTER_SECONDS,
            Duration::from_millis(5000),
        )
    }

    pub async fn latest(&self) -> Result<Mt5LatestQuote, MarketDataError> {
        let (Some(source), Some(scope)) = (&self.source, &self.scope) else {
            return Err(unavailable("MT5 market-data source is not configured"));
        };

        let loaded = tokio::time::timeout(self.query_timeout, source.load_latest(scope))
            .await
            .map_err(|_| MarketDataError::Timeout { provider: PROVIDER })?;

        match loaded {
            Ok(Some(record)) => build_quote(&record, self.clock.now_ms(), self.stale_after_seconds),
            Ok(None) => Err(unavailable(
                "no MT5 market-data snapshot has been persisted",
            )),
            Err(SourceError::Unavailable) => {
                Err(unavailable("MT5 market-data store is unavailable"))
            }
            Err(SourceError::Timeout) => Err(MarketDataError::Timeout { provider: PROVIDER }),
            Err(SourceError::Corrupt) => Err(invalid("stored snapshot failed integrity checks")),
        }
    }
}

fn unavailable(reason: &str) -> MarketDataError {
    MarketDataError::ProviderUnavailable {
        provider: PROVIDER,
        reason: reason.into(),
    }
}

fn invalid(reason: &str) -> MarketDataError {
    MarketDataError::InvalidProviderData {
        provider: PROVIDER,
        reason: reason.into(),
    }
}

fn decimal_field(quote: &Value, field: &str) -> Result<Decimal, MarketDataError> {
    // Decimal strings only: a JSON number may already have lost precision.
    let text = quote
        .get(field)
        .and_then(Value::as_str)
        .ok_or_else(|| invalid("quote price is missing or not a decimal string"))?;
    if !is_plain_decimal(text) {
        let (category, integer_digits, fraction_digits) = classify_decimal_failure(text);
        log_decimal_failure(field, category, integer_digits, fraction_digits);
        return Err(invalid("quote price is not a valid decimal"));
    }
    let value = Decimal::from_str(text).map_err(|_| {
        let (integer_digits, fraction_digits) = decimal_part_lengths(text);
        log_decimal_failure(
            field,
            "decimal_parse_failure",
            integer_digits,
            fraction_digits,
        );
        invalid("quote price is not a valid decimal")
    })?;
    if value <= Decimal::ZERO {
        return Err(invalid("quote price must be positive"));
    }
    Ok(value)
}

/// Lengths of the parts around the first '.', never the characters themselves.
fn decimal_part_lengths(text: &str) -> (usize, usize) {
    match text.split_once('.') {
        Some((integer, fraction)) => (integer.len(), fraction.len()),
        None => (text.len(), 0),
    }
}

/// Diagnostic only: why `is_plain_decimal` rejected `text`. Returns a static
/// category plus part lengths; the price itself is never returned or logged.
fn classify_decimal_failure(text: &str) -> (&'static str, usize, usize) {
    let (integer_digits, fraction_digits) = decimal_part_lengths(text);
    let (integer, fraction) = match text.split_once('.') {
        Some((i, f)) => (i, f),
        None => (text, ""),
    };
    let category = if !integer
        .bytes()
        .chain(fraction.bytes())
        .all(|b| b.is_ascii_digit())
    {
        "invalid_characters"
    } else if integer.is_empty() || (text.contains('.') && fraction.is_empty()) {
        "empty_integer_or_fraction"
    } else if integer_digits > MAX_INTEGER_DIGITS {
        "integer_digits_exceed_limit"
    } else if fraction_digits > MAX_FRACTION_DIGITS {
        "fractional_digits_exceed_limit"
    } else {
        "unclassified"
    };
    (category, integer_digits, fraction_digits)
}

fn log_decimal_failure(field: &str, category: &str, integer_digits: usize, fraction_digits: usize) {
    tracing::warn!(
        field,
        category,
        integer_digits,
        fraction_digits,
        "MT5 stored quote price rejected by decimal validation"
    );
}

const MAX_INTEGER_DIGITS: usize = 10;
// The MT5 EA serializes prices with 16 fractional digits; values are never rounded.
const MAX_FRACTION_DIGITS: usize = 16;

/// Strict grammar: `digits` or `digits.digits`. No sign, exponent, separators,
/// bare `.5` / `5.`, or unbounded scale (at most 16 fractional digits).
fn is_plain_decimal(text: &str) -> bool {
    let (integer, fraction) = match text.split_once('.') {
        Some((i, f)) => (i, Some(f)),
        None => (text, None),
    };
    let digits = |s: &str, max: usize| {
        !s.is_empty() && s.len() <= max && s.bytes().all(|b| b.is_ascii_digit())
    };
    digits(integer, MAX_INTEGER_DIGITS) && fraction.is_none_or(|f| digits(f, MAX_FRACTION_DIGITS))
}

fn seconds(ms: i64) -> f64 {
    ms as f64 / 1000.0
}

fn format_utc(time: DateTime<Utc>) -> String {
    time.to_rfc3339_opts(SecondsFormat::AutoSi, true)
}

/// Validate a stored snapshot and derive the public quote. Pure and clock-injected.
pub fn build_quote(
    record: &Mt5SnapshotRecord,
    now_ms: i64,
    stale_after_seconds: u64,
) -> Result<Mt5LatestQuote, MarketDataError> {
    let quotes = record
        .snapshot
        .get("quotes")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid("snapshot has no quotes array"))?;

    let mut matching = quotes
        .iter()
        .filter(|q| q.get("symbol").and_then(Value::as_str) == Some(MT5_SYMBOL));
    let quote = matching
        .next()
        .ok_or_else(|| invalid("snapshot has no EURUSD quote"))?;
    if matching.next().is_some() {
        return Err(invalid("snapshot has more than one EURUSD quote"));
    }

    let bid = decimal_field(quote, "bid")?;
    let ask = decimal_field(quote, "ask")?;
    if ask < bid {
        return Err(invalid("quote ask is below bid"));
    }

    let tick_raw = quote
        .get("tick_time")
        .and_then(Value::as_str)
        .ok_or_else(|| invalid("quote tick_time is missing"))?;
    let tick_parsed = DateTime::parse_from_rfc3339(tick_raw)
        .map_err(|_| invalid("quote tick_time is not RFC 3339"))?;
    if tick_parsed.offset().local_minus_utc() != 0 {
        return Err(invalid("quote tick_time is not UTC"));
    }
    let tick_time = tick_parsed.with_timezone(&Utc);

    let quote_age_ms = now_ms - tick_time.timestamp_millis();
    let snapshot_age_ms = now_ms - record.captured_at.timestamp_millis();
    if quote_age_ms < -MAX_FUTURE_TOLERANCE_MS {
        return Err(invalid("quote tick_time is in the future"));
    }
    if snapshot_age_ms < -MAX_FUTURE_TOLERANCE_MS {
        return Err(invalid("snapshot captured_at is in the future"));
    }

    let is_stale = quote_age_ms > (stale_after_seconds as i64).saturating_mul(1000);

    Ok(Mt5LatestQuote {
        status: if is_stale {
            QuoteStatus::Stale
        } else {
            QuoteStatus::Fresh
        },
        classification: DataClass::Live,
        symbol: MT5_SYMBOL,
        bid: bid.to_string(),
        ask: ask.to_string(),
        spread: (ask - bid).to_string(),
        tick_time: format_utc(tick_time),
        captured_at: format_utc(record.captured_at),
        sequence: record.sequence,
        quote_age_seconds: seconds(quote_age_ms),
        snapshot_age_seconds: seconds(snapshot_age_ms),
        is_stale,
        stale_after_seconds,
    })
}

const DB_MAX_CONNECTIONS: u32 = 2;
const DB_ACQUIRE_TIMEOUT: Duration = Duration::from_secs(3);
const DB_IDLE_TIMEOUT: Duration = Duration::from_secs(60);
const DB_STATEMENT_TIMEOUT_MS: u32 = 3000;

fn is_local_host(host: &str) -> bool {
    host.starts_with('/') || matches!(host, "localhost" | "127.0.0.1" | "::1" | "[::1]")
}

/// Parse the database URL and enforce the TLS policy. Returns static errors
/// only, because parser messages can echo parts of the URL.
///
/// Loopback and Unix-socket hosts may use any `sslmode`. Any other host must
/// use `require`, `verify-ca` or `verify-full`; `prefer`/`allow`/`disable`
/// (including the sqlx default, `prefer`) are rejected so plaintext fallback
/// is impossible. Only `verify-full` also authenticates the server.
pub fn connect_options(database_url: &str) -> Result<PgConnectOptions, &'static str> {
    let options = PgConnectOptions::from_str(database_url)
        .map_err(|_| "database URL is not a valid PostgreSQL URL")?;
    let tls_enforced = matches!(
        options.get_ssl_mode(),
        PgSslMode::Require | PgSslMode::VerifyCa | PgSslMode::VerifyFull
    );
    if !is_local_host(options.get_host()) && !tls_enforced {
        return Err(
            "non-loopback database hosts require sslmode=require, verify-ca or verify-full",
        );
    }
    // Defense in depth on top of the read-only role: bounded statements and
    // read-only transactions for every session of this pool.
    Ok(options.options([
        ("statement_timeout", DB_STATEMENT_TIMEOUT_MS.to_string()),
        ("default_transaction_read_only", "on".to_string()),
    ]))
}

/// Read-only PostgreSQL source with its own small pool, separate from the
/// shared `persistence::connect` pool. The pool is lazy (no connection at
/// construction) and capped at two connections with a short acquire timeout,
/// so an unreachable database cannot cause a connection stampede: extra
/// requests queue on the pool and fail after the acquire timeout.
pub struct PgMt5SnapshotSource {
    pool: sqlx::PgPool,
}

impl PgMt5SnapshotSource {
    /// Must be called inside a Tokio runtime.
    pub fn new(database_url: &str) -> Result<Self, &'static str> {
        let options = connect_options(database_url)?;
        let pool = PgPoolOptions::new()
            .max_connections(DB_MAX_CONNECTIONS)
            .min_connections(0)
            .acquire_timeout(DB_ACQUIRE_TIMEOUT)
            .idle_timeout(DB_IDLE_TIMEOUT)
            .connect_lazy_with(options);
        Ok(Self { pool })
    }
}

#[async_trait]
impl Mt5SnapshotSource for PgMt5SnapshotSource {
    async fn load_latest(
        &self,
        scope: &Mt5Scope,
    ) -> Result<Option<Mt5SnapshotRecord>, SourceError> {
        // Error details can carry connection information, so none are forwarded.
        match persistence::repos::mt5_market_data::load_latest(
            &self.pool,
            &scope.account_id,
            &scope.terminal_id,
        )
        .await
        {
            Ok(Some(record)) => Ok(Some(Mt5SnapshotRecord {
                sequence: record.sequence,
                captured_at: record.captured_at,
                snapshot: record.snapshot,
            })),
            Ok(None) => Ok(None),
            // The repository uses `Other` for identity/sequence integrity failures.
            Err(persistence::PersistenceError::Other(_)) => Err(SourceError::Corrupt),
            // Connection, pool and decode errors are all reported as unavailable.
            Err(_) => Err(SourceError::Unavailable),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    // 2026-11-02T12:00:00Z
    const NOW_MS: i64 = 1_793_620_800_000;

    struct FixedClock;
    impl Clock for FixedClock {
        fn now_ms(&self) -> i64 {
            NOW_MS
        }
    }

    struct FakeSource(Result<Option<Mt5SnapshotRecord>, SourceError>);

    #[async_trait]
    impl Mt5SnapshotSource for FakeSource {
        async fn load_latest(
            &self,
            _: &Mt5Scope,
        ) -> Result<Option<Mt5SnapshotRecord>, SourceError> {
            self.0.clone()
        }
    }

    fn at(offset_ms: i64) -> DateTime<Utc> {
        DateTime::<Utc>::from_timestamp_millis(NOW_MS + offset_ms).unwrap()
    }

    fn record_with(quote: Value, captured_offset_ms: i64) -> Mt5SnapshotRecord {
        Mt5SnapshotRecord {
            sequence: 5978,
            captured_at: at(captured_offset_ms),
            snapshot: json!({ "quotes": [quote] }),
        }
    }

    fn quote(tick_offset_ms: i64) -> Value {
        json!({
            "symbol": "EURUSD",
            "bid": "1.12115",
            "ask": "1.12126",
            "tick_time": format_utc(at(tick_offset_ms)),
        })
    }

    fn service(
        source: Option<Result<Option<Mt5SnapshotRecord>, SourceError>>,
        scoped: bool,
    ) -> Mt5LatestService {
        Mt5LatestService::new(
            source.map(|r| Arc::new(FakeSource(r)) as Arc<dyn Mt5SnapshotSource>),
            scoped.then(|| Mt5Scope {
                account_id: "a".into(),
                terminal_id: "t".into(),
            }),
            Arc::new(FixedClock),
            60,
            Duration::from_secs(1),
        )
    }

    fn err_code(e: MarketDataError) -> (&'static str, u16) {
        (e.code(), e.http_status())
    }

    #[test]
    fn fresh_valid_quote() {
        let q = build_quote(&record_with(quote(-5_000), -4_000), NOW_MS, 60).unwrap();
        assert_eq!(q.status, QuoteStatus::Fresh);
        assert_eq!(q.classification, DataClass::Live);
        assert_eq!(q.symbol, "EURUSD");
        assert_eq!((q.bid.as_str(), q.ask.as_str()), ("1.12115", "1.12126"));
        assert_eq!(q.spread, "0.00011");
        assert_eq!(q.sequence, 5978);
        assert!(!q.is_stale);
        assert_eq!(q.stale_after_seconds, 60);
        assert!(q.tick_time.ends_with('Z') && q.captured_at.ends_with('Z'));
    }

    #[test]
    fn stale_quote_is_flagged_but_still_live() {
        let q = build_quote(&record_with(quote(-61_000), -1_000), NOW_MS, 60).unwrap();
        assert_eq!(q.status, QuoteStatus::Stale);
        assert!(q.is_stale);
        assert_eq!(q.classification, DataClass::Live);
    }

    #[test]
    fn exactly_at_threshold_is_not_stale() {
        let q = build_quote(&record_with(quote(-60_000), -1_000), NOW_MS, 60).unwrap();
        assert!(!q.is_stale);
    }

    #[test]
    fn quote_age_and_snapshot_age_are_independent() {
        let q = build_quote(&record_with(quote(-90_000), -2_500), NOW_MS, 60).unwrap();
        assert_eq!(q.quote_age_seconds, 90.0);
        assert_eq!(q.snapshot_age_seconds, 2.5);
        // A freshly persisted snapshot must not make an old quote fresh.
        assert!(q.is_stale);
    }

    #[test]
    fn tick_within_future_tolerance_is_not_clamped() {
        let q = build_quote(&record_with(quote(3_000), -1_000), NOW_MS, 60).unwrap();
        assert_eq!(q.quote_age_seconds, -3.0);
    }

    #[test]
    fn tick_beyond_future_tolerance_is_rejected() {
        let e = build_quote(&record_with(quote(5_001), -1_000), NOW_MS, 60).unwrap_err();
        assert_eq!(err_code(e), ("invalid_provider_data", 502));
    }

    #[test]
    fn future_captured_at_is_rejected() {
        let e = build_quote(&record_with(quote(-1_000), 6_000), NOW_MS, 60).unwrap_err();
        assert_eq!(err_code(e), ("invalid_provider_data", 502));
    }

    #[test]
    fn rejects_wrong_or_missing_symbol() {
        for symbol in ["GBPUSD", "EURUSD.a", "eurusd"] {
            let mut q = quote(-1_000);
            q["symbol"] = json!(symbol);
            let e = build_quote(&record_with(q, -1_000), NOW_MS, 60).unwrap_err();
            assert_eq!(err_code(e), ("invalid_provider_data", 502), "{symbol}");
        }
    }

    #[test]
    fn rejects_non_positive_bid_or_ask() {
        for (bid, ask) in [
            ("0", "1.1"),
            ("0.00000", "1.1"),
            ("-1.1", "1.1"),
            ("1.1", "0"),
        ] {
            let mut q = quote(-1_000);
            q["bid"] = json!(bid);
            q["ask"] = json!(ask);
            let e = build_quote(&record_with(q, -1_000), NOW_MS, 60).unwrap_err();
            assert_eq!(err_code(e), ("invalid_provider_data", 502), "{bid}/{ask}");
        }
    }

    #[test]
    fn rejects_ask_below_bid_and_accepts_equal() {
        let mut q = quote(-1_000);
        q["bid"] = json!("1.12126");
        q["ask"] = json!("1.12115");
        let e = build_quote(&record_with(q, -1_000), NOW_MS, 60).unwrap_err();
        assert_eq!(err_code(e), ("invalid_provider_data", 502));

        let mut q = quote(-1_000);
        q["ask"] = q["bid"].clone();
        let ok = build_quote(&record_with(q, -1_000), NOW_MS, 60).unwrap();
        assert_eq!(Decimal::from_str(&ok.spread).unwrap(), Decimal::ZERO);
    }

    #[test]
    fn rejects_malformed_prices() {
        for bad in [
            json!(1.12115),
            json!(null),
            json!("abc"),
            json!("1.1e3"),
            json!(".5"),
            json!("5."),
            json!("1..2"),
            json!("+1.1"),
            json!("1,1"),
            json!("1.12345678901"),
            json!("12345678901.1"),
        ] {
            let mut q = quote(-1_000);
            q["bid"] = bad;
            let e = build_quote(&record_with(q, -1_000), NOW_MS, 60).unwrap_err();
            assert_eq!(err_code(e), ("invalid_provider_data", 502));
        }
    }

    #[test]
    fn preserves_decimal_scale_as_stored() {
        let mut q = quote(-1_000);
        q["bid"] = json!("1.10000");
        q["ask"] = json!("1.10020");
        let ok = build_quote(&record_with(q, -1_000), NOW_MS, 60).unwrap();
        assert_eq!(ok.bid, "1.10000");
        assert_eq!(ok.ask, "1.10020");
        assert_eq!(ok.spread, "0.00020");
    }

    #[test]
    fn plain_decimal_grammar() {
        for ok in [
            "1",
            "1.12115",
            "0.00001",
            "1234567890.1234567890",
            "1.1234567890123456",
        ] {
            assert!(is_plain_decimal(ok), "{ok}");
        }
        for bad in [
            "",
            ".",
            ".5",
            "5.",
            "-1",
            "1e3",
            "1.2.3",
            " 1",
            "1.12345678901234567",
            "12345678901.5",
        ] {
            assert!(!is_plain_decimal(bad), "{bad}");
        }
    }

    #[test]
    fn fractional_digit_limit_applies_to_bid_and_ask_without_rounding() {
        let with = |bid: &str, ask: &str| {
            let mut q = quote(-1_000);
            q["bid"] = json!(bid);
            q["ask"] = json!(ask);
            build_quote(&record_with(q, -1_000), NOW_MS, 60)
        };
        // 10, 11 and 16 fractional digits are accepted and returned unchanged.
        for (bid, ask) in [
            ("1.1211500000", "1.1212600000"),
            ("1.12115000001", "1.12126000001"),
            ("1.1211499999999999", "1.1212600000000001"),
        ] {
            let q = with(bid, ask).unwrap();
            assert_eq!(q.bid, bid);
            assert_eq!(q.ask, ask);
        }
        // 17 fractional digits are rejected, for either field.
        let long = "1.12115000000000001";
        for (bid, ask) in [(long, "1.12126"), ("1.12115", long)] {
            let e = with(bid, ask).unwrap_err();
            assert!(matches!(e, MarketDataError::InvalidProviderData { .. }));
        }
    }

    #[test]
    fn tls_policy_for_database_hosts() {
        for ok in [
            "postgres://u@localhost/d",
            "postgres://u@127.0.0.1:5433/d?sslmode=disable",
            "postgres://u@[::1]/d",
            "postgres://u@db.example.com/d?sslmode=require",
            "postgres://u@db.example.com/d?sslmode=verify-ca",
            "postgres://u@db.example.com/d?sslmode=verify-full",
        ] {
            assert!(connect_options(ok).is_ok(), "{ok}");
        }
        for bad in [
            "postgres://u@db.example.com/d",
            "postgres://u@db.example.com/d?sslmode=prefer",
            "postgres://u@db.example.com/d?sslmode=allow",
            "postgres://u@10.0.0.5/d?sslmode=disable",
            "not a url",
        ] {
            let e = connect_options(bad).unwrap_err();
            assert!(!e.contains("example.com") && !e.contains("10.0.0.5"));
            assert!(connect_options(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn rejects_invalid_or_non_utc_timestamp() {
        for bad in [
            json!("not-a-time"),
            json!(null),
            json!("2026-11-02T13:00:00+01:00"),
        ] {
            let mut q = quote(-1_000);
            q["tick_time"] = bad;
            let e = build_quote(&record_with(q, -1_000), NOW_MS, 60).unwrap_err();
            assert_eq!(err_code(e), ("invalid_provider_data", 502));
        }
    }

    #[test]
    fn rejects_corrupt_or_missing_snapshot_fields() {
        let mut record = record_with(quote(-1_000), -1_000);
        record.snapshot = json!({ "quotes": "nope" });
        assert!(build_quote(&record, NOW_MS, 60).is_err());
        record.snapshot = json!({});
        assert!(build_quote(&record, NOW_MS, 60).is_err());
        record.snapshot = json!({ "quotes": [] });
        assert!(build_quote(&record, NOW_MS, 60).is_err());
        record.snapshot = json!({ "quotes": [quote(-1_000), quote(-1_000)] });
        assert!(build_quote(&record, NOW_MS, 60).is_err());
    }

    #[test]
    fn response_has_no_identity_or_raw_fields() {
        let mut record = record_with(quote(-1_000), -1_000);
        record.snapshot["account_id"] = json!("secret-account");
        record.snapshot["terminal_id"] = json!("secret-terminal");
        record.snapshot["server"] = json!("secret-server");
        let q = build_quote(&record, NOW_MS, 60).unwrap();
        let text = serde_json::to_string(&q).unwrap();
        for forbidden in ["secret", "account", "terminal", "server", "instruments"] {
            assert!(!text.contains(forbidden), "{forbidden}");
        }
    }

    #[tokio::test]
    async fn service_returns_quote() {
        let svc = service(Some(Ok(Some(record_with(quote(-1_000), -500)))), true);
        assert_eq!(svc.latest().await.unwrap().status, QuoteStatus::Fresh);
    }

    #[tokio::test]
    async fn no_snapshot_is_unavailable() {
        let e = service(Some(Ok(None)), true).latest().await.unwrap_err();
        assert_eq!(err_code(e), ("provider_unavailable", 503));
    }

    #[tokio::test]
    async fn missing_scope_or_source_is_unavailable() {
        let no_scope = service(Some(Ok(None)), false).latest().await.unwrap_err();
        assert_eq!(err_code(no_scope), ("provider_unavailable", 503));
        let no_source = service(None, true).latest().await.unwrap_err();
        assert_eq!(err_code(no_source), ("provider_unavailable", 503));
        let none = Mt5LatestService::unconfigured(Arc::new(FixedClock))
            .latest()
            .await
            .unwrap_err();
        assert_eq!(err_code(none), ("provider_unavailable", 503));
    }

    #[tokio::test]
    async fn database_unavailable_maps_to_503() {
        let e = service(Some(Err(SourceError::Unavailable)), true)
            .latest()
            .await
            .unwrap_err();
        assert_eq!(err_code(e), ("provider_unavailable", 503));
    }

    #[tokio::test]
    async fn query_timeout_maps_to_504() {
        let e = service(Some(Err(SourceError::Timeout)), true)
            .latest()
            .await
            .unwrap_err();
        assert_eq!(err_code(e), ("provider_timeout", 504));
    }

    #[tokio::test]
    async fn corrupt_record_maps_to_502_without_details() {
        let e = service(Some(Err(SourceError::Corrupt)), true)
            .latest()
            .await
            .unwrap_err();
        assert_eq!(e.http_status(), 502);
        assert!(!e.to_string().to_lowercase().contains("sql"));
    }
}

//! Mutual-TLS transport boundary for the MT5 v1 bridge.
//!
//! This module authenticates a peer certificate before exposing protocol frames.
//! It does not dispatch orders or create a ready session; callers must still apply
//! durable sequence checks, reconciliation, risk authorization and OMS validation.

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::fs::File;
use std::io::BufReader;
use std::net::SocketAddr;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration as StdDuration;

use chrono::{DateTime, Duration, Utc};
use rustls::pki_types::{CertificateDer, PrivateKeyDer};
use rustls::server::WebPkiClientVerifier;
use rustls::{RootCertStore, ServerConfig};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{Mutex, OwnedSemaphorePermit, Semaphore};
use tokio::task::JoinHandle;
use tokio::time::{interval_at, timeout, Instant};
use tokio_rustls::{server::TlsStream, TlsAcceptor};

use crate::mt5_session::Mt5SessionGate;
use persistence::TradingStore;
use crate::mt5_v1::{BridgeFrame, BridgePayload, NdjsonFrameDecoder};
use crate::{BridgeError, BridgeResult};

/// Host-managed identity mapping. The fingerprint is lowercase hex SHA-256 of
/// the exact leaf certificate DER, with no separators.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Mt5TransportPrincipal {
    pub certificate_sha256: String,
    pub terminal_id: String,
    pub server: String,
    pub allowed_accounts: BTreeSet<String>,
}

impl Mt5TransportPrincipal {
    fn validate(&self) -> BridgeResult<()> {
        if self.certificate_sha256.len() != 64
            || !self
                .certificate_sha256
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit())
            || self.terminal_id.trim().is_empty()
            || self.terminal_id.trim() != self.terminal_id
            || self.server.trim().is_empty()
            || self.server.trim() != self.server
            || self.allowed_accounts.is_empty()
            || self
                .allowed_accounts
                .iter()
                .any(|account| account.trim().is_empty() || account.trim() != account)
        {
            return Err(BridgeError::Invalid(
                "invalid MT5 transport principal configuration".into(),
            ));
        }
        Ok(())
    }
}

/// Runtime bounds for the authenticated transport. Defaults are conservative and
/// cannot be disabled by a terminal frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Mt5TransportLimits {
    pub handshake_timeout: StdDuration,
    pub io_timeout: StdDuration,
    pub heartbeat_timeout: StdDuration,
    pub max_concurrent_connections: usize,
}

impl Default for Mt5TransportLimits {
    fn default() -> Self {
        Self {
            handshake_timeout: StdDuration::from_secs(5),
            io_timeout: StdDuration::from_secs(15),
            heartbeat_timeout: StdDuration::from_secs(30),
            max_concurrent_connections: 64,
        }
    }
}

impl Mt5TransportLimits {
    fn validate(self) -> BridgeResult<Self> {
        if self.handshake_timeout.is_zero()
            || self.io_timeout.is_zero()
            || self.heartbeat_timeout.is_zero()
            || self.max_concurrent_connections == 0
        {
            return Err(BridgeError::Invalid(
                "MT5 transport timeouts must be positive".into(),
            ));
        }
        Ok(self)
    }
}

/// TLS server configuration plus an explicit certificate-to-identity allow-list.
pub struct Mt5TransportConfig {
    acceptor: TlsAcceptor,
    principals: BTreeMap<String, Mt5TransportPrincipal>,
    limits: Mt5TransportLimits,
    connection_slots: Arc<Semaphore>,
}

impl Mt5TransportConfig {
    /// Load TLS 1.3-only mutual TLS configuration. Client certificates must chain
    /// to the configured CA and their exact SHA-256 fingerprint must be allow-listed.
    pub fn from_pem_files(
        server_certificate_chain: impl AsRef<Path>,
        server_private_key: impl AsRef<Path>,
        trusted_client_ca: impl AsRef<Path>,
        principals: impl IntoIterator<Item = Mt5TransportPrincipal>,
    ) -> BridgeResult<Self> {
        let certificates = load_certificates(server_certificate_chain.as_ref())?;
        if certificates.is_empty() {
            return Err(BridgeError::Invalid(
                "MT5 transport server certificate chain is empty".into(),
            ));
        }
        let private_key = load_private_key(server_private_key.as_ref())?;
        let client_ca_certificates = load_certificates(trusted_client_ca.as_ref())?;
        if client_ca_certificates.is_empty() {
            return Err(BridgeError::Invalid(
                "MT5 transport client CA bundle is empty".into(),
            ));
        }

        let mut roots = RootCertStore::empty();
        for certificate in client_ca_certificates {
            roots.add(certificate).map_err(|_| {
                BridgeError::Invalid("invalid certificate in MT5 client CA bundle".into())
            })?;
        }
        let verifier = WebPkiClientVerifier::builder(Arc::new(roots))
            .build()
            .map_err(|_| BridgeError::Invalid("invalid MT5 client certificate verifier".into()))?;
        let tls_config = ServerConfig::builder_with_protocol_versions(&[&rustls::version::TLS13])
            .with_client_cert_verifier(verifier)
            .with_single_cert(certificates, private_key)
            .map_err(|_| BridgeError::Invalid("invalid MT5 server TLS certificate/key".into()))?;

        let mut mapped = BTreeMap::new();
        for principal in principals {
            principal.validate()?;
            let fingerprint = principal.certificate_sha256.to_ascii_lowercase();
            if mapped.insert(fingerprint, principal).is_some() {
                return Err(BridgeError::Invalid(
                    "duplicate MT5 client certificate fingerprint".into(),
                ));
            }
        }
        if mapped.is_empty() {
            return Err(BridgeError::Invalid(
                "MT5 transport requires at least one configured client principal".into(),
            ));
        }

        Ok(Self {
            acceptor: TlsAcceptor::from(Arc::new(tls_config)),
            principals: mapped,
            limits: Mt5TransportLimits::default(),
            connection_slots: Arc::new(Semaphore::new(64)),
        })
    }

    /// Replace runtime bounds with validated, non-zero limits.
    pub fn with_limits(mut self, limits: Mt5TransportLimits) -> BridgeResult<Self> {
        let limits = limits.validate()?;
        self.connection_slots = Arc::new(Semaphore::new(limits.max_concurrent_connections));
        self.limits = limits;
        Ok(self)
    }

    /// Bind a listener only after configuration is validated. The caller controls
    /// the bind address and must not expose this listener unintentionally.
    pub async fn bind(&self, address: SocketAddr) -> BridgeResult<TcpListener> {
        TcpListener::bind(address).await.map_err(BridgeError::from)
    }

    /// Accept TCP and complete mutual TLS before returning a connection. No
    /// protocol frame is exposed until certificate-chain validation and exact
    /// configured fingerprint mapping both succeed.
    pub async fn accept(&self, listener: &TcpListener) -> BridgeResult<AuthenticatedMt5Connection> {
        let (stream, _) = timeout(self.limits.handshake_timeout, listener.accept())
            .await
            .map_err(|_| BridgeError::Invalid("MT5 connection accept timed out".into()))?
            .map_err(BridgeError::from)?;
        self.authenticate(stream).await
    }

    /// Authenticate an already accepted socket. Useful for bounded listener
    /// orchestration and deterministic local integration tests.
    pub async fn authenticate(
        &self,
        stream: TcpStream,
    ) -> BridgeResult<AuthenticatedMt5Connection> {
        let connection_slot = self
            .connection_slots
            .clone()
            .try_acquire_owned()
            .map_err(|_| {
                BridgeError::Invalid("MT5 transport connection capacity exhausted".into())
            })?;
        let tls_stream = timeout(self.limits.handshake_timeout, self.acceptor.accept(stream))
            .await
            .map_err(|_| BridgeError::Invalid("MT5 mutual-TLS handshake timed out".into()))?
            .map_err(|_| BridgeError::Invalid("MT5 mutual-TLS authentication failed".into()))?;
        let certificate = tls_stream
            .get_ref()
            .1
            .peer_certificates()
            .and_then(|chain| chain.first())
            .ok_or_else(|| {
                BridgeError::Invalid("MT5 peer did not present a client certificate".into())
            })?;
        let fingerprint = certificate_fingerprint(certificate);
        let principal = self.principals.get(&fingerprint).cloned().ok_or_else(|| {
            BridgeError::Invalid("MT5 client certificate is not allow-listed".into())
        })?;
        Ok(AuthenticatedMt5Connection {
            stream: tls_stream,
            principal,
            decoder: NdjsonFrameDecoder::new(),
            pending_frames: VecDeque::new(),
            heartbeat_seen: false,
            last_heartbeat_at: None,
            limits: self.limits,
            _connection_slot: connection_slot,
            failed: false,
        })
    }
}

/// Start an autonomous heartbeat watchdog for a shared authenticated connection.
///
/// The host must share the same connection/session mutexes with its transport loop.
/// On the first missing/expired/future heartbeat, the watchdog poisons the connection,
/// degrades the session and exits; callers must close/reconnect and reconcile before reuse.
pub fn spawn_heartbeat_watchdog(
    connection: Arc<Mutex<AuthenticatedMt5Connection>>,
    session: Arc<Mutex<Mt5SessionGate>>,
    poll_interval: StdDuration,
) -> BridgeResult<JoinHandle<()>> {
    if poll_interval.is_zero() {
        return Err(BridgeError::Invalid(
            "MT5 heartbeat watchdog poll interval must be positive".into(),
        ));
    }
    let first_tick = Instant::now() + poll_interval;
    Ok(tokio::spawn(async move {
        let mut ticks = interval_at(first_tick, poll_interval);
        loop {
            ticks.tick().await;
            let mut connection = connection.lock().await;
            let mut session = session.lock().await;
            if connection
                .check_heartbeat_fresh_with_session(&mut session, Utc::now())
                .is_err()
            {
                break;
            }
        }
    }))
}

pub struct AuthenticatedMt5Connection {
    stream: TlsStream<TcpStream>,
    principal: Mt5TransportPrincipal,
    decoder: NdjsonFrameDecoder,
    pending_frames: VecDeque<BridgeFrame>,
    heartbeat_seen: bool,
    last_heartbeat_at: Option<DateTime<Utc>>,
    limits: Mt5TransportLimits,
    _connection_slot: OwnedSemaphorePermit,
    failed: bool,
}

impl AuthenticatedMt5Connection {
    pub fn principal(&self) -> &Mt5TransportPrincipal {
        &self.principal
    }

    /// Fail closed when no valid heartbeat has been received within the configured
    /// freshness window. The runtime must call this from its watchdog/scheduler;
    /// a blocked read alone cannot detect a silent peer.
    pub fn check_heartbeat_fresh(&mut self, now: DateTime<Utc>) -> BridgeResult<()> {
        if enforce_heartbeat_freshness(
            self.last_heartbeat_at,
            now,
            self.limits.heartbeat_timeout,
            None,
        )
        .is_err()
        {
            self.failed = true;
            return Err(BridgeError::Invalid(
                "MT5 heartbeat expired; connection must be reconciled before reuse".into(),
            ));
        }
        Ok(())
    }

    /// Check heartbeat freshness and degrade the associated session gate on expiry.
    /// The host runtime must invoke this from its watchdog/scheduler; this method
    /// does not create a background task or replace reconnect reconciliation.
    pub fn check_heartbeat_fresh_with_session(
        &mut self,
        session: &mut Mt5SessionGate,
        now: DateTime<Utc>,
    ) -> BridgeResult<()> {
        if enforce_heartbeat_freshness(
            self.last_heartbeat_at,
            now,
            self.limits.heartbeat_timeout,
            Some(session),
        )
        .is_err()
        {
            self.failed = true;
            return Err(BridgeError::Invalid(
                "MT5 heartbeat expired; connection and session are degraded".into(),
            ));
        }
        Ok(())
    }

    /// Read the next complete frame, requiring the first application frame to be
    /// a heartbeat and binding heartbeat identity to the authenticated principal.
    /// A framing, identity or ordering error poisons this connection.
    pub async fn read_frame(
        &mut self,
        now: DateTime<Utc>,
        allowed_clock_skew: Duration,
    ) -> BridgeResult<BridgeFrame> {
        if self.failed {
            return Err(BridgeError::Invalid(
                "MT5 authenticated connection is closed after a protocol violation".into(),
            ));
        }
        loop {
            if let Some(frame) = self.pending_frames.pop_front() {
                if let Err(error) = self.validate_frame_identity(&frame, now) {
                    self.failed = true;
                    return Err(error);
                }
                return Ok(frame);
            }

            let mut chunk = [0_u8; 4096];
            let count = match timeout(self.limits.io_timeout, self.stream.read(&mut chunk)).await {
                Err(_) => {
                    self.failed = true;
                    return Err(BridgeError::Invalid(
                        "MT5 authenticated transport read timed out".into(),
                    ));
                }
                Ok(Err(_)) => {
                    self.failed = true;
                    return Err(BridgeError::Invalid(
                        "MT5 authenticated transport read failed".into(),
                    ));
                }
                Ok(Ok(count)) => count,
            };
            if count == 0 {
                self.failed = true;
                if let Err(error) = self.decoder.finish() {
                    return Err(error);
                }
                return Err(BridgeError::Invalid(
                    "MT5 authenticated transport closed".into(),
                ));
            }
            let frames = match self.decoder.push(&chunk[..count], now, allowed_clock_skew) {
                Ok(frames) => frames,
                Err(error) => {
                    self.failed = true;
                    return Err(error);
                }
            };
            self.pending_frames.extend(frames);
        }
    }

    /// Read a frame and atomically connect heartbeat identity/sequence observations
    /// to durable stream readiness and the in-memory session gate. Any persistence or
    /// identity failure poisons the connection and degrades the session.
    pub async fn read_frame_with_session_checkpoint(
        &mut self,
        store: &TradingStore,
        session: &mut Mt5SessionGate,
        now: DateTime<Utc>,
        allowed_clock_skew: Duration,
    ) -> BridgeResult<BridgeFrame> {
        let frame = self.read_frame(now, allowed_clock_skew).await?;
        let heartbeat = match &frame.payload {
            BridgePayload::Heartbeat(value) => value,
            _ => return Ok(frame),
        };
        let account_id = frame.account_id.clone();
        let terminal_id = heartbeat.terminal_id.clone();
        let sequence = heartbeat.sequence;
        let observation = match store
            .observe_mt5_sequence(&account_id, &terminal_id, sequence)
            .await
        {
            Ok(observation) => observation,
            Err(error) => {
                self.failed = true;
                session.degrade();
                return Err(BridgeError::from(error));
            }
        };
        let readiness = match store.mt5_stream_readiness(&account_id, &terminal_id).await {
            Ok(readiness) => readiness,
            Err(error) => {
                self.failed = true;
                session.degrade();
                return Err(BridgeError::from(error));
            }
        };
        if let Err(error) = session.observe_heartbeat(&frame, observation, readiness) {
            self.failed = true;
            session.degrade();
            let _ = store
                .mark_mt5_stream_degraded(&account_id, &terminal_id, sequence)
                .await;
            return Err(error);
        }
        Ok(frame)
    }

    /// Write one validated protocol frame as NDJSON. Transport success means only
    /// bytes were written; it is never an order acceptance/fill/cancel confirmation.
    pub async fn write_frame(&mut self, frame: &BridgeFrame) -> BridgeResult<()> {
        if self.failed || !self.heartbeat_seen {
            return Err(BridgeError::Invalid(
                "MT5 outbound frames are blocked until authenticated heartbeat validation".into(),
            ));
        }
        if !self.principal.allowed_accounts.contains(&frame.account_id) {
            self.failed = true;
            return Err(BridgeError::Invalid(
                "MT5 outbound frame account is not allowed for authenticated principal".into(),
            ));
        }
        let encoded = encode_frame_line(frame)?;
        match timeout(self.limits.io_timeout, async {
            self.stream.write_all(&encoded).await?;
            self.stream.flush().await
        })
        .await
        {
            Err(_) => {
                self.failed = true;
                Err(BridgeError::Invalid(
                    "MT5 authenticated transport write timed out".into(),
                ))
            }
            Ok(Err(_)) => {
                self.failed = true;
                Err(BridgeError::Invalid(
                    "MT5 authenticated transport write failed".into(),
                ))
            }
            Ok(Ok(())) => Ok(()),
        }
    }

    fn validate_frame_identity(
        &mut self,
        frame: &BridgeFrame,
        received_at: DateTime<Utc>,
    ) -> BridgeResult<()> {
        if !self.heartbeat_seen {
            let heartbeat = match &frame.payload {
                BridgePayload::Heartbeat(value) => value,
                _ => {
                    return Err(BridgeError::Invalid(
                        "first MT5 application frame must be a heartbeat".into(),
                    ));
                }
            };
            if !self.principal.allowed_accounts.contains(&frame.account_id)
                || heartbeat.terminal_id != self.principal.terminal_id
                || heartbeat.server != self.principal.server
            {
                return Err(BridgeError::Invalid(
                    "MT5 heartbeat identity does not match authenticated principal".into(),
                ));
            }
            self.heartbeat_seen = true;
            self.last_heartbeat_at = Some(received_at);
        } else if !self.principal.allowed_accounts.contains(&frame.account_id) {
            return Err(BridgeError::Invalid(
                "MT5 frame account is not allowed for authenticated principal".into(),
            ));
        } else if matches!(&frame.payload, BridgePayload::Heartbeat(_)) {
            self.last_heartbeat_at = Some(received_at);
        }
        Ok(())
    }
}

fn encode_frame_line(frame: &BridgeFrame) -> BridgeResult<Vec<u8>> {
    let mut encoded = serde_json::to_vec(frame)?;
    if encoded.len() > crate::mt5_v1::MAX_MT5_BRIDGE_FRAME_BYTES {
        return Err(BridgeError::Invalid(
            "MT5 outbound frame exceeds configured size limit".into(),
        ));
    }
    encoded.push(b'\n');
    Ok(encoded)
}

fn enforce_heartbeat_freshness(
    last_heartbeat_at: Option<DateTime<Utc>>,
    now: DateTime<Utc>,
    timeout: StdDuration,
    mut session: Option<&mut Mt5SessionGate>,
) -> BridgeResult<()> {
    let fresh = last_heartbeat_at.is_some_and(|last| heartbeat_is_fresh(last, now, timeout));
    if !fresh {
        if let Some(session) = session.as_mut() {
            session.degrade();
        }
        return Err(BridgeError::Invalid(
            "MT5 heartbeat expired; connection must be reconciled before reuse".into(),
        ));
    }
    Ok(())
}

fn heartbeat_is_fresh(last: DateTime<Utc>, now: DateTime<Utc>, timeout: StdDuration) -> bool {
    let age = now.signed_duration_since(last);
    age >= Duration::zero()
        && age <= Duration::from_std(timeout).unwrap_or_else(|_| Duration::zero())
}

fn load_certificates(path: &Path) -> BridgeResult<Vec<CertificateDer<'static>>> {
    let file = File::open(path).map_err(|_| {
        BridgeError::Invalid("unable to open configured MT5 transport certificate file".into())
    })?;
    rustls_pemfile::certs(&mut BufReader::new(file))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| BridgeError::Invalid("invalid PEM certificate file".into()))
}

fn load_private_key(path: &Path) -> BridgeResult<PrivateKeyDer<'static>> {
    let file = File::open(path).map_err(|_| {
        BridgeError::Invalid("unable to open configured MT5 transport private-key file".into())
    })?;
    rustls_pemfile::private_key(&mut BufReader::new(file))
        .map_err(|_| BridgeError::Invalid("invalid PEM private-key file".into()))?
        .ok_or_else(|| BridgeError::Invalid("no private key found in PEM file".into()))
}

fn certificate_fingerprint(certificate: &CertificateDer<'_>) -> String {
    let digest = Sha256::digest(certificate.as_ref());
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn principal(fingerprint: &str) -> Mt5TransportPrincipal {
        Mt5TransportPrincipal {
            certificate_sha256: fingerprint.into(),
            terminal_id: "terminal-01".into(),
            server: "broker-demo".into(),
            allowed_accounts: BTreeSet::from(["account-001".into()]),
        }
    }

    #[test]
    fn principal_requires_sha256_fingerprint_and_explicit_identity() {
        assert!(principal("not-a-fingerprint").validate().is_err());
        assert!(principal(&"a".repeat(64)).validate().is_ok());
        let mut invalid = principal(&"b".repeat(64));
        invalid.allowed_accounts.clear();
        assert!(invalid.validate().is_err());
    }

    #[test]
    fn transport_limits_reject_zero_timeouts_and_capacity() {
        assert!(Mt5TransportLimits {
            handshake_timeout: StdDuration::ZERO,
            ..Mt5TransportLimits::default()
        }
        .validate()
        .is_err());
        assert!(Mt5TransportLimits {
            io_timeout: StdDuration::ZERO,
            ..Mt5TransportLimits::default()
        }
        .validate()
        .is_err());
        assert!(Mt5TransportLimits {
            heartbeat_timeout: StdDuration::ZERO,
            ..Mt5TransportLimits::default()
        }
        .validate()
        .is_err());
        assert!(Mt5TransportLimits {
            max_concurrent_connections: 0,
            ..Mt5TransportLimits::default()
        }
        .validate()
        .is_err());
        assert!(Mt5TransportLimits::default().validate().is_ok());
    }

    #[test]
    fn outbound_frame_encoding_uses_exactly_one_lf_delimiter() -> BridgeResult<()> {
        let frame = BridgeFrame {
            schema_version: crate::mt5_v1::MT5_BRIDGE_SCHEMA_VERSION,
            message_id: "message-1".into(),
            correlation_id: "correlation-1".into(),
            account_id: "account-001".into(),
            sent_at: Utc::now(),
            payload: BridgePayload::Heartbeat(crate::mt5_v1::Heartbeat {
                terminal_id: "terminal-01".into(),
                server: "broker-demo".into(),
                sequence: 1,
                terminal_time: Utc::now(),
                connection: crate::mt5_v1::ConnectionState::Connected,
                capabilities: crate::mt5_v1::BridgeCapabilities {
                    market_orders: true,
                    limit_orders: true,
                    stop_orders: true,
                    cancel_requests: true,
                    order_lookup: true,
                    idempotent_client_order_id: true,
                    sequence_numbers: true,
                },
            }),
        };
        let encoded = encode_frame_line(&frame)?;
        assert_eq!(encoded.last(), Some(&b'\n'));
        assert!(!encoded.ends_with(b"\\n"));
        assert_eq!(encoded.iter().filter(|byte| **byte == b'\n').count(), 1);
        let json = encoded
            .strip_suffix(b"\n")
            .ok_or_else(|| BridgeError::Invalid("encoded NDJSON frame is missing LF".into()))?;
        let decoded = serde_json::from_slice::<BridgeFrame>(json)?;
        assert_eq!(decoded, frame);
        Ok(())
    }

    #[test]
    fn heartbeat_expiry_degrades_the_session_gate() -> BridgeResult<()> {
        let now = Utc::now();
        let timeout = StdDuration::from_secs(30);
        let policy = crate::mt5_session::Mt5SessionPolicy::new(["account-001"])?;
        let mut session = Mt5SessionGate::after_transport_authentication(policy);

        assert!(enforce_heartbeat_freshness(
            Some(now - Duration::seconds(31)),
            now,
            timeout,
            Some(&mut session),
        )
        .is_err());
        assert_eq!(
            session.state(),
            crate::mt5_session::Mt5SessionState::Degraded
        );

        let healthy_policy = crate::mt5_session::Mt5SessionPolicy::new(["account-001"])?;
        let mut healthy_session = Mt5SessionGate::after_transport_authentication(healthy_policy);
        assert!(enforce_heartbeat_freshness(
            Some(now - Duration::seconds(10)),
            now,
            timeout,
            Some(&mut healthy_session),
        )
        .is_ok());
        assert_eq!(
            healthy_session.state(),
            crate::mt5_session::Mt5SessionState::AwaitingHeartbeat
        );
        Ok(())
    }

    #[test]
    fn heartbeat_freshness_rejects_expiry_and_future_timestamps() {
        let last = Utc::now();
        let timeout = StdDuration::from_secs(30);
        assert!(heartbeat_is_fresh(
            last,
            last + Duration::seconds(30),
            timeout
        ));
        assert!(!heartbeat_is_fresh(
            last,
            last + Duration::seconds(31),
            timeout
        ));
        assert!(!heartbeat_is_fresh(
            last,
            last - Duration::seconds(1),
            timeout
        ));
    }

    #[test]
    fn certificate_fingerprint_is_lowercase_sha256_hex() {
        let certificate = CertificateDer::from(vec![1_u8, 2, 3, 4]);
        assert_eq!(
            certificate_fingerprint(&certificate),
            "9f64a747e1b97f131fabb6b447296c9b6f0201e79fb3c5356e6c77e89b6a806a"
        );
    }
}

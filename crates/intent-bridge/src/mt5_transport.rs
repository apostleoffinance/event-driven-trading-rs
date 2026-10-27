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

use chrono::{DateTime, Duration, Utc};
use rustls::pki_types::{CertificateDer, PrivateKeyDer};
use rustls::server::WebPkiClientVerifier;
use rustls::{RootCertStore, ServerConfig};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio_rustls::{server::TlsStream, TlsAcceptor};

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
            || !self.certificate_sha256.bytes().all(|byte| byte.is_ascii_hexdigit())
            || self.terminal_id.trim().is_empty()
            || self.terminal_id.trim() != self.terminal_id
            || self.server.trim().is_empty()
            || self.server.trim() != self.server
            || self.allowed_accounts.is_empty()
            || self.allowed_accounts.iter().any(|account| {
                account.trim().is_empty() || account.trim() != account
            })
        {
            return Err(BridgeError::Invalid(
                "invalid MT5 transport principal configuration".into(),
            ));
        }
        Ok(())
    }
}

/// TLS server configuration plus an explicit certificate-to-identity allow-list.
pub struct Mt5TransportConfig {
    acceptor: TlsAcceptor,
    principals: BTreeMap<String, Mt5TransportPrincipal>,
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
        })
    }

    /// Bind a listener only after configuration is validated. The caller controls
    /// the bind address and must not expose this listener unintentionally.
    pub async fn bind(&self, address: SocketAddr) -> BridgeResult<TcpListener> {
        TcpListener::bind(address).await.map_err(BridgeError::from)
    }

    /// Accept TCP and complete mutual TLS before returning a connection. No
    /// protocol frame is exposed until certificate-chain validation and exact
    /// configured fingerprint mapping both succeed.
    pub async fn accept(
        &self,
        listener: &TcpListener,
    ) -> BridgeResult<AuthenticatedMt5Connection> {
        let (stream, _) = listener.accept().await.map_err(BridgeError::from)?;
        self.authenticate(stream).await
    }

    /// Authenticate an already accepted socket. Useful for bounded listener
    /// orchestration and deterministic local integration tests.
    pub async fn authenticate(
        &self,
        stream: TcpStream,
    ) -> BridgeResult<AuthenticatedMt5Connection> {
        let tls_stream = self.acceptor.accept(stream).await.map_err(|_| {
            BridgeError::Invalid("MT5 mutual-TLS authentication failed".into())
        })?;
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
            failed: false,
        })
    }
}

pub struct AuthenticatedMt5Connection {
    stream: TlsStream<TcpStream>,
    principal: Mt5TransportPrincipal,
    decoder: NdjsonFrameDecoder,
    pending_frames: VecDeque<BridgeFrame>,
    heartbeat_seen: bool,
    failed: bool,
}

impl AuthenticatedMt5Connection {
    pub fn principal(&self) -> &Mt5TransportPrincipal {
        &self.principal
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
            return Err(BridgeError::Invalid("MT5 authenticated connection is closed after a protocol violation".into()));
        }
        loop {
            if let Some(frame) = self.pending_frames.pop_front() {
                if let Err(error) = self.validate_frame_identity(&frame) {
                    self.failed = true;
                    return Err(error);
                }
                return Ok(frame);
            }

            let mut chunk = [0_u8; 4096];
            let count = self.stream.read(&mut chunk).await.map_err(|_| {
                BridgeError::Invalid("MT5 authenticated transport read failed".into())
            })?;
            if count == 0 {
                self.decoder.finish()?;
                return Err(BridgeError::Invalid(
                    "MT5 authenticated transport closed".into(),
                ));
            }
            let frames = self.decoder.push(&chunk[..count], now, allowed_clock_skew)?;
            self.pending_frames.extend(frames);
        }
    }

    /// Write one validated protocol frame as NDJSON. Transport success means only
    /// bytes were written; it is never an order acceptance/fill/cancel confirmation.
    pub async fn write_frame(&mut self, frame: &BridgeFrame) -> BridgeResult<()> {
        let encoded = serde_json::to_vec(frame)?;
        if encoded.len() > crate::mt5_v1::MAX_MT5_BRIDGE_FRAME_BYTES {
            return Err(BridgeError::Invalid(
                "MT5 outbound frame exceeds configured size limit".into(),
            ));
        }
        self.stream.write_all(&encoded).await.map_err(|_| {
            BridgeError::Invalid("MT5 authenticated transport write failed".into())
        })?;
        self.stream.write_all(b"\n").await.map_err(|_| {
            BridgeError::Invalid("MT5 authenticated transport write failed".into())
        })?;
        self.stream.flush().await.map_err(|_| {
            BridgeError::Invalid("MT5 authenticated transport flush failed".into())
        })
    }

    fn validate_frame_identity(&mut self, frame: &BridgeFrame) -> BridgeResult<()> {
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
        } else if !self.principal.allowed_accounts.contains(&frame.account_id) {
            return Err(BridgeError::Invalid(
                "MT5 frame account is not allowed for authenticated principal".into(),
            ));
        }
        Ok(())
    }
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
    fn certificate_fingerprint_is_lowercase_sha256_hex() {
        let certificate = CertificateDer::from(vec![1_u8, 2, 3, 4]);
        assert_eq!(
            certificate_fingerprint(&certificate),
            "9f64a747e1b97f131fabb6b447296c9b6f0201e79b3fc5356e6c77e89b6a806a"
        );
    }
}

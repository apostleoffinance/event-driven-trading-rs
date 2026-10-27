use std::collections::BTreeSet;
use std::error::Error;
use std::net::SocketAddr;
use std::sync::Arc;

use chrono::{Duration, Utc};
use rcgen::{
    BasicConstraints, Certificate, CertificateParams, ExtendedKeyUsagePurpose, IsCa, KeyPair,
};
use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer, ServerName};
use rustls::{ClientConfig, RootCertStore};
use sha2::{Digest, Sha256};
use tokio::io::AsyncWriteExt;
use tokio::net::{TcpListener, TcpStream};
use tokio_rustls::{client::TlsStream, TlsConnector};

use intent_bridge::mt5_transport::{AuthenticatedMt5Connection, Mt5TransportConfig, Mt5TransportPrincipal};
use intent_bridge::mt5_v1::{
    BridgeCapabilities, BridgeFrame, BridgePayload, ConnectionState, Heartbeat,
    MT5_BRIDGE_SCHEMA_VERSION,
};

struct TestPki {
    _directory: tempfile::TempDir,
    trusted_ca_path: std::path::PathBuf,
    server_cert_path: std::path::PathBuf,
    server_key_path: std::path::PathBuf,
    client_config: Arc<ClientConfig>,
    untrusted_client_config: Arc<ClientConfig>,
    client_fingerprint: String,
    untrusted_client_fingerprint: String,
}

fn fingerprint(cert: &Certificate) -> String {
    Sha256::digest(cert.der().as_ref())
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn make_pki() -> Result<TestPki, Box<dyn Error>> {
    let directory = tempfile::tempdir()?;
    let trusted_ca_path = directory.path().join("trusted-ca.pem");
    let server_cert_path = directory.path().join("server.pem");
    let server_key_path = directory.path().join("server-key.pem");
    let client_cert_path = directory.path().join("client.pem");
    let client_key_path = directory.path().join("client-key.pem");
    let untrusted_ca_path = directory.path().join("untrusted-ca.pem");
    let untrusted_client_cert_path = directory.path().join("untrusted-client.pem");
    let untrusted_client_key_path = directory.path().join("untrusted-client-key.pem");

    let trusted_ca_key = KeyPair::generate()?;
    let mut trusted_ca_params = CertificateParams::default();
    trusted_ca_params.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
    let trusted_ca = trusted_ca_params.self_signed(&trusted_ca_key)?;
    std::fs::write(&trusted_ca_path, trusted_ca.pem())?;

    let server_key = KeyPair::generate()?;
    let mut server_params = CertificateParams::new(vec!["localhost".to_owned()])?;
    server_params.extended_key_usages = vec![ExtendedKeyUsagePurpose::ServerAuth];
    let server_cert = server_params.signed_by(&server_key, &trusted_ca, &trusted_ca_key)?;
    std::fs::write(&server_cert_path, server_cert.pem())?;
    std::fs::write(&server_key_path, server_key.serialize_pem())?;

    let client_key = KeyPair::generate()?;
    let mut client_params = CertificateParams::default();
    client_params.extended_key_usages = vec![ExtendedKeyUsagePurpose::ClientAuth];
    let client_cert = client_params.signed_by(&client_key, &trusted_ca, &trusted_ca_key)?;
    std::fs::write(&client_cert_path, client_cert.pem())?;
    std::fs::write(&client_key_path, client_key.serialize_pem())?;

    let untrusted_ca_key = KeyPair::generate()?;
    let mut untrusted_ca_params = CertificateParams::default();
    untrusted_ca_params.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
    let untrusted_ca = untrusted_ca_params.self_signed(&untrusted_ca_key)?;
    std::fs::write(&untrusted_ca_path, untrusted_ca.pem())?;

    let untrusted_client_key = KeyPair::generate()?;
    let mut untrusted_client_params = CertificateParams::default();
    untrusted_client_params.extended_key_usages = vec![ExtendedKeyUsagePurpose::ClientAuth];
    let untrusted_client_cert =
        untrusted_client_params.signed_by(&untrusted_client_key, &untrusted_ca, &untrusted_ca_key)?;
    std::fs::write(&untrusted_client_cert_path, untrusted_client_cert.pem())?;
    std::fs::write(&untrusted_client_key_path, untrusted_client_key.serialize_pem())?;

    let mut roots = RootCertStore::empty();
    roots.add(CertificateDer::from(trusted_ca.der().to_vec()))?;
    let client_config = Arc::new(
        ClientConfig::builder()
            .with_root_certificates(roots.clone())
            .with_client_auth_cert(
                vec![CertificateDer::from(client_cert.der().to_vec())],
                PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(client_key.serialize_der())),
            )?,
    );
    let untrusted_client_config = Arc::new(
        ClientConfig::builder()
            .with_root_certificates(roots)
            .with_client_auth_cert(
                vec![CertificateDer::from(untrusted_client_cert.der().to_vec())],
                PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(
                    untrusted_client_key.serialize_der(),
                )),
            )?,
    );

    Ok(TestPki {
        _directory: directory,
        trusted_ca_path,
        server_cert_path,
        server_key_path,
        client_config,
        untrusted_client_config,
        client_fingerprint: fingerprint(&client_cert),
        untrusted_client_fingerprint: fingerprint(&untrusted_client_cert),
    })
}

fn server_config(pki: &TestPki, fingerprint: String) -> Result<Mt5TransportConfig, Box<dyn Error>> {
    let principal = Mt5TransportPrincipal {
        certificate_sha256: fingerprint,
        terminal_id: "terminal-01".to_owned(),
        server: "broker-demo".to_owned(),
        allowed_accounts: BTreeSet::from(["account-001".to_owned()]),
    };
    Ok(Mt5TransportConfig::from_pem_files(
        &pki.server_cert_path,
        &pki.server_key_path,
        &pki.trusted_ca_path,
        [principal],
    )?)
}

async fn establish(
    config: Mt5TransportConfig,
    client_config: Arc<ClientConfig>,
) -> Result<
    (
        AuthenticatedMt5Connection,
        TlsStream<TcpStream>,
        SocketAddr,
    ),
    Box<dyn Error>,
> {
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let address = listener.local_addr()?;
    let server_task = tokio::spawn(async move {
        let (stream, _) = listener.accept().await?;
        config.authenticate(stream).await
    });
    let stream = TcpStream::connect(address).await?;
    let connector = TlsConnector::from(client_config);
    let name = ServerName::try_from("localhost")?;
    let client_stream = connector.connect(name, stream).await?;
    let server_connection = server_task.await??;
    Ok((server_connection, client_stream, address))
}

fn heartbeat_frame(account: &str, terminal: &str, server: &str) -> BridgeFrame {
    let now = Utc::now();
    BridgeFrame {
        schema_version: MT5_BRIDGE_SCHEMA_VERSION,
        message_id: "mtls-test-heartbeat".to_owned(),
        correlation_id: "mtls-test-correlation".to_owned(),
        account_id: account.to_owned(),
        sent_at: now,
        payload: BridgePayload::Heartbeat(Heartbeat {
            terminal_id: terminal.to_owned(),
            server: server.to_owned(),
            sequence: 1,
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
        }),
    }
}

async fn write_frame(
    stream: &mut TlsStream<TcpStream>,
    frame: &BridgeFrame,
) -> Result<(), Box<dyn Error>> {
    let mut encoded = serde_json::to_vec(frame)?;
    encoded.push(b'\n');
    stream.write_all(&encoded).await?;
    stream.flush().await?;
    Ok(())
}

#[tokio::test]
async fn trusted_mtls_peer_and_matching_identity_are_accepted() -> Result<(), Box<dyn Error>> {
    let pki = make_pki()?;
    let config = server_config(&pki, pki.client_fingerprint.clone())?;
    let (mut server, mut client, _) = establish(config, pki.client_config.clone()).await?;
    let frame = heartbeat_frame("account-001", "terminal-01", "broker-demo");
    write_frame(&mut client, &frame).await?;
    let decoded = server
        .read_frame(Utc::now(), Duration::seconds(5))
        .await?;
    assert_eq!(decoded.account_id, "account-001");
    assert_eq!(server.principal().terminal_id, "terminal-01");
    Ok(())
}

#[tokio::test]
async fn untrusted_client_certificate_is_rejected_during_tls_handshake() -> Result<(), Box<dyn Error>> {
    let pki = make_pki()?;
    let config = server_config(&pki, pki.untrusted_client_fingerprint.clone())?;
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let address = listener.local_addr()?;
    let server_task = tokio::spawn(async move {
        let (stream, _) = listener.accept().await?;
        config.authenticate(stream).await
    });
    let stream = TcpStream::connect(address).await?;
    let connector = TlsConnector::from(pki.untrusted_client_config.clone());
    let name = ServerName::try_from("localhost")?;
    let client_result = connector.connect(name, stream).await;
    let server_result = server_task.await?;
    assert!(client_result.is_err() || server_result.is_err());
    assert!(server_result.is_err());
    Ok(())
}

#[tokio::test]
async fn trusted_certificate_with_wrong_fingerprint_is_rejected() -> Result<(), Box<dyn Error>> {
    let pki = make_pki()?;
    let config = server_config(&pki, "00".repeat(32))?;
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let address = listener.local_addr()?;
    let server_task = tokio::spawn(async move {
        let (stream, _) = listener.accept().await?;
        config.authenticate(stream).await
    });
    let stream = TcpStream::connect(address).await?;
    let connector = TlsConnector::from(pki.client_config.clone());
    let name = ServerName::try_from("localhost")?;
    let client_result = connector.connect(name, stream).await;
    let server_result = server_task.await?;
    assert!(client_result.is_ok());
    assert!(server_result.is_err());
    Ok(())
}

#[tokio::test]
async fn terminal_server_and_account_identity_mismatches_fail_closed() -> Result<(), Box<dyn Error>> {
    let pki = make_pki()?;
    for frame in [
        heartbeat_frame("account-001", "other-terminal", "broker-demo"),
        heartbeat_frame("account-001", "terminal-01", "other-server"),
        heartbeat_frame("other-account", "terminal-01", "broker-demo"),
    ] {
        let config = server_config(&pki, pki.client_fingerprint.clone())?;
        let (mut server, mut client, _) = establish(config, pki.client_config.clone()).await?;
        write_frame(&mut client, &frame).await?;
        assert!(server
            .read_frame(Utc::now(), Duration::seconds(5))
            .await
            .is_err());
    }
    Ok(())
}

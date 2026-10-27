//! Read-only MT5 bridge daemon.
//!
//! This process authenticates one configured MT5 worker identity, checkpoints
//! heartbeats, persists fresh account/order/position snapshots, and reconnects after
//! failures. It intentionally exposes no order-submit or cancel command.

use std::collections::BTreeSet;
use std::net::SocketAddr;
use std::time::Duration as StdDuration;

use chrono::Duration;
use clap::Parser;
use intent_bridge::{
    Mt5BridgeRuntime, Mt5SessionGate, Mt5SessionPolicy, Mt5TransportConfig,
    Mt5TransportPrincipal,
};
use persistence::{connect, TradingStore};
use tracing::{info, warn};
use tracing_subscriber::EnvFilter;

#[derive(Debug, Parser)]
#[command(name = "mt5-bridge-daemon")]
#[command(about = "Read-only mTLS MT5 account and position reconciliation daemon")]
struct Args {
    /// PostgreSQL connection string. Defaults to DATABASE_URL.
    #[arg(long, env = "DATABASE_URL")]
    database_url: String,

    /// Listener address. Loopback-only by default because the worker runs on this host.
    #[arg(long, env = "MT5_BRIDGE_BIND", default_value = "127.0.0.1:9443")]
    bind: SocketAddr,

    /// Server certificate PEM chain.
    #[arg(long, env = "MT5_BRIDGE_SERVER_CERT")]
    server_cert: String,

    /// Server private key PEM. Keep file ACLs restricted to the daemon service account.
    #[arg(long, env = "MT5_BRIDGE_SERVER_KEY")]
    server_key: String,

    /// CA bundle used to validate worker certificates.
    #[arg(long, env = "MT5_BRIDGE_CLIENT_CA")]
    client_ca: String,

    /// SHA-256 fingerprint of the exact worker leaf certificate DER (64 hex characters).
    #[arg(long, env = "MT5_BRIDGE_PRINCIPAL_SHA256")]
    principal_sha256: String,

    /// Expected stable terminal identity.
    #[arg(long, env = "MT5_TERMINAL_ID")]
    terminal_id: String,

    /// Exact MT5 broker server name.
    #[arg(long, env = "MT5_SERVER")]
    server: String,

    /// Rust account identifier sent by the worker (not necessarily the numeric MT5 login).
    #[arg(long, env = "MT5_ACCOUNT_ID")]
    account_id: String,

    /// Permit binding to a non-loopback address. Requires explicit true.
    #[arg(long, env = "MT5_BRIDGE_ALLOW_REMOTE_BIND", default_value = "false")]
    allow_remote_bind: bool,
}

fn required_identity(value: &str, label: &str) -> Result<String, Box<dyn std::error::Error>> {
    if value.trim().is_empty() || value.trim() != value {
        return Err(format!("{label} must be non-empty and have no surrounding whitespace").into());
    }
    Ok(value.to_owned())
}

fn validate_bind(address: SocketAddr, allow_remote_bind: bool) -> Result<(), &'static str> {
    if !address.ip().is_loopback() && !allow_remote_bind {
        return Err("non-loopback MT5 bridge binding requires MT5_BRIDGE_ALLOW_REMOTE_BIND=true");
    }
    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive("info".parse()?))
        .init();

    let args = Args::parse();
    validate_bind(args.bind, args.allow_remote_bind)
        .map_err(std::io::Error::other)?;

    let account_id = required_identity(&args.account_id, "MT5_ACCOUNT_ID")?;
    let terminal_id = required_identity(&args.terminal_id, "MT5_TERMINAL_ID")?;
    let server = required_identity(&args.server, "MT5_SERVER")?;
    let principal = Mt5TransportPrincipal {
        certificate_sha256: required_identity(
            &args.principal_sha256,
            "MT5_BRIDGE_PRINCIPAL_SHA256",
        )?,
        terminal_id: terminal_id.clone(),
        server: server.clone(),
        allowed_accounts: BTreeSet::from([account_id.clone()]),
    };
    let transport = Mt5TransportConfig::from_pem_files(
        &args.server_cert,
        &args.server_key,
        &args.client_ca,
        [principal],
    )?;

    let pool = connect(&args.database_url).await?;
    let store = TradingStore::new(pool);
    let listener = transport.bind(args.bind).await?;
    info!(
        bind = %args.bind,
        terminal_id = %terminal_id,
        server = %server,
        account_id = %account_id,
        "read-only MT5 bridge listening; order execution is not exposed"
    );

    loop {
        tokio::select! {
            signal = tokio::signal::ctrl_c() => {
                signal?;
                info!("shutdown requested");
                break;
            }
            accepted = transport.accept(&listener) => {
                let connection = match accepted {
                    Ok(connection) => connection,
                    Err(error) => {
                        warn!(error = %error, "MT5 connection rejected or accept timed out");
                        continue;
                    }
                };
                let authenticated_principal = connection.principal().clone();
                let policy = match Mt5SessionPolicy::new(
                    authenticated_principal.allowed_accounts.iter().cloned(),
                ) {
                    Ok(policy) => policy,
                    Err(error) => {
                        warn!(error = %error, "could not create MT5 session policy");
                        continue;
                    }
                };
                let session = Mt5SessionGate::after_transport_authentication(policy);
                let runtime = match Mt5BridgeRuntime::new(
                    store.clone(),
                    connection,
                    session,
                    StdDuration::from_secs(2),
                ) {
                    Ok(runtime) => runtime,
                    Err(error) => {
                        warn!(error = %error, "could not initialize MT5 runtime");
                        continue;
                    }
                };

                info!(
                    terminal_id = %authenticated_principal.terminal_id,
                    server = %authenticated_principal.server,
                    "authenticated MT5 worker connected"
                );
                loop {
                    tokio::select! {
                        signal = tokio::signal::ctrl_c() => {
                            signal?;
                            info!("shutdown requested while MT5 worker was connected");
                            return Ok(());
                        }
                        result = runtime.reconcile_from_next_frame(
                            Duration::seconds(30),
                            Duration::seconds(30),
                        ) => {
                            match result {
                                Ok(()) => {
                                    info!(
                                        state = ?runtime.session_state().await,
                                        "persisted MT5 account/order/position snapshot"
                                    );
                                }
                                Err(error) => {
                                    warn!(error = %error, "MT5 stream failed; connection will be discarded and must reconnect/reconcile");
                                    break;
                                }
                            }
                        }
                    }
                }
                drop(runtime);
                info!("MT5 connection closed; waiting for a new authenticated connection");
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn listener_defaults_are_safe_and_remote_binding_is_explicit() {
        let loopback = SocketAddr::from(([127, 0, 0, 1], 9443));
        let remote = SocketAddr::from(([0, 0, 0, 0], 9443));
        assert!(validate_bind(loopback, false).is_ok());
        assert!(validate_bind(remote, false).is_err());
        assert!(validate_bind(remote, true).is_ok());
    }

    #[test]
    fn configured_identity_rejects_empty_or_padded_values() {
        assert!(required_identity("account-1", "account").is_ok());
        assert!(required_identity("", "account").is_err());
        assert!(required_identity(" account-1", "account").is_err());
        assert!(required_identity("account-1 ", "account").is_err());
    }
}

# Quant OS MT5 read-only adapter (QOS-015A)

This package connects a Windows-hosted MetaTrader 5 terminal to the Rust MT5 bridge using TLS 1.3 mutual authentication. It is deliberately read-only: there is no order submission, order modification, or cancellation code path. Execution is a separate follow-up after demo reconciliation acceptance.

## Runtime placement

- Run this process on the same Windows VPS/host as the self-hosted MT5 terminal.
- Host the Rust bridge and PostgreSQL wherever the deployment architecture requires; expose the bridge only over a firewall-restricted endpoint with mTLS.
- Vercel is for the Next.js frontend, not the MT5 process. A standard Linux Render service cannot host this Windows-only terminal worker.
- Never put broker passwords or client private keys in frontend variables, browser storage, logs, or source control.

## Install

PowerShell on the Windows VPS, with MT5 installed and logged into the intended demo account:

    py -3.12 -m venv .venv
    .\.venv\Scripts\Activate.ps1
    python -m pip install --upgrade pip
    pip install -e ".[terminal]"

## Required environment variables

| Variable | Purpose |
|---|---|
| MT5_TERMINAL_PATH | Absolute path to terminal64.exe |
| MT5_LOGIN | Expected numeric demo login |
| MT5_PASSWORD | Demo account password; never logged |
| MT5_SERVER | Exact broker demo server name |
| MT5_TERMINAL_ID | Stable host-assigned terminal identity |
| MT5_BRIDGE_HOST | DNS name/IP of the Rust mTLS listener |
| MT5_BRIDGE_PORT | Bridge port; defaults to 9443 |
| MT5_BRIDGE_CA_FILE | CA certificate used to validate the Rust server |
| MT5_BRIDGE_CLIENT_CERT | Client certificate issued for this worker |
| MT5_BRIDGE_CLIENT_KEY | Matching private key, ACL-restricted to the service account |
| MT5_ACCOUNT_ID | Optional Rust account ID; defaults to the numeric login |
| MT5_BRIDGE_SERVER_NAME | Optional TLS hostname override; defaults to bridge host |
| MT5_SEQUENCE_FILE | Optional durable sequence checkpoint path |
| MT5_POLL_INTERVAL_SECONDS | Snapshot interval, 1–300 seconds; defaults to 10 |

The Rust transport principal must allow-list the exact client-certificate fingerprint, terminal ID, server name and account ID. Certificate issuance and rotation must be managed outside this package.

## Contract

Each connection begins with a heartbeat. The worker then sends a complete reconciliation_snapshot frame with account state, every currently open order and every open position. Values representing money, prices and quantities are encoded as decimal strings; symbol names are preserved exactly. The sequence is persisted locally before sending so process restarts do not silently reset it. A send failure can create a sequence gap, which must force reconciliation.

The worker never claims an order was accepted or filled. Heartbeat capabilities explicitly disable market/limit/stop submission and cancellation. Rust can persist the external snapshot, but submit/cancel must remain blocked until an execution-capable adapter passes its separate acceptance gate.

## Local contract tests

    python -m unittest discover -s tests -v

Tests use fake terminal data and do not require an MT5 installation, broker credentials, or network access. A demo-terminal smoke test must be run on the target Windows VPS before any order-execution work begins.

# Quant OS MT5 read-only adapter (QOS-015A)

This package connects a Windows-hosted MetaTrader 5 terminal to the Rust MT5 bridge using TLS 1.3 mutual authentication. It is deliberately read-only: there is no order submission, order modification, or cancellation code path. Execution is a separate follow-up after demo reconciliation acceptance.

## Runtime placement

- Run this process on the same Windows VPS/host as the self-hosted MT5 terminal.
- Run the Rust bridge daemon on the same host for the default loopback binding, or deliberately configure a protected remote endpoint.
- PostgreSQL may be hosted separately, but the Windows daemon must have private network access to it.
- Vercel is for the Next.js frontend, not the MT5 process. A standard Linux Render service cannot host this Windows-only terminal worker.
- Never put broker passwords or client private keys in frontend variables, browser storage, logs, or source control.

## Install the Python worker

PowerShell on the Windows VPS, with MT5 installed and logged into the intended demo account:

    py -3.12 -m venv .venv
    .\.venv\Scripts\Activate.ps1
    python -m pip install --upgrade pip
    pip install -e ".[terminal]"

## Required Python worker environment variables

| Variable | Purpose |
|---|---|
| MT5_TERMINAL_PATH | Absolute path to terminal64.exe |
| MT5_LOGIN | Expected numeric demo login |
| MT5_PASSWORD | Demo account password; never logged |
| MT5_SERVER | Exact broker demo server name |
| MT5_TERMINAL_ID | Stable host-assigned terminal identity |
| MT5_BRIDGE_HOST | Rust listener host; use 127.0.0.1 when colocated |
| MT5_BRIDGE_PORT | Bridge port; defaults to 9443 |
| MT5_BRIDGE_CA_FILE | CA certificate used to validate the Rust server |
| MT5_BRIDGE_CLIENT_CERT | Client certificate issued for this worker |
| MT5_BRIDGE_CLIENT_KEY | Matching private key, ACL-restricted to the service account |
| MT5_ACCOUNT_ID | Optional Rust account ID; defaults to the numeric login |
| MT5_BRIDGE_SERVER_NAME | Optional TLS hostname override; defaults to bridge host |
| MT5_SEQUENCE_FILE | Optional durable sequence checkpoint path |
| MT5_POLL_INTERVAL_SECONDS | Snapshot interval, 1–300 seconds; defaults to 10 |
| MT5_SYMBOLS | Required comma-separated list of 1–64 exact broker symbols to monitor, e.g. EURUSD.a,GBPUSD.a |

## Start the Rust read-only daemon

Build/run from the repository root on the Windows host with a Rust toolchain installed. The database schema must already be migrated using the project's deployment/migration process; the daemon does not run schema migrations automatically.

The daemon reads these environment variables:

| Variable | Purpose |
|---|---|
| DATABASE_URL | PostgreSQL connection string |
| MT5_BRIDGE_BIND | Listener address; defaults to 127.0.0.1:9443 |
| MT5_BRIDGE_SERVER_CERT | Server certificate chain PEM; include the listener host in SAN |
| MT5_BRIDGE_SERVER_KEY | Matching server private key PEM; restrict filesystem ACLs |
| MT5_BRIDGE_CLIENT_CA | CA bundle used to verify the worker certificate chain |
| MT5_BRIDGE_PRINCIPAL_SHA256 | Exact worker leaf-certificate DER SHA-256 fingerprint, 64 hex characters |
| MT5_TERMINAL_ID | Expected terminal identity; must match the worker |
| MT5_SERVER | Exact broker server string; must match the worker |
| MT5_ACCOUNT_ID | Exact Rust account ID sent in worker frames |
| MT5_BRIDGE_ALLOW_REMOTE_BIND | Defaults false; set true only for an intentionally protected non-loopback listener |

Example PowerShell environment setup (use real values; never commit secrets):

    $env:DATABASE_URL = "postgres://..."
    $env:MT5_BRIDGE_BIND = "127.0.0.1:9443"
    $env:MT5_BRIDGE_SERVER_CERT = "C:\QuantOS\certs\server-chain.pem"
    $env:MT5_BRIDGE_SERVER_KEY = "C:\QuantOS\certs\server-key.pem"
    $env:MT5_BRIDGE_CLIENT_CA = "C:\QuantOS\certs\worker-ca.pem"
    $env:MT5_BRIDGE_PRINCIPAL_SHA256 = "<64-character-worker-leaf-sha256>"
    $env:MT5_TERMINAL_ID = "terminal-demo-01"
    $env:MT5_SERVER = "<exact broker demo server>"
    $env:MT5_ACCOUNT_ID = "<same account ID configured for the worker>"
    cargo run -p intent-bridge --bin mt5-bridge-daemon

Configure the Python worker to connect to 127.0.0.1:9443 and use a server certificate valid for that name (or configure a matching MT5_BRIDGE_SERVER_NAME). The daemon rejects non-loopback binds unless MT5_BRIDGE_ALLOW_REMOTE_BIND=true is explicit. If remote binding is needed, also apply host firewall rules and private-network controls; mTLS is not a substitute for network restriction.

The Rust principal allow-lists the configured leaf-certificate fingerprint, terminal ID, broker server, and one account ID. The daemon accepts one active worker at a time, persists fresh account/order/position snapshots, and discards failed streams so the worker must reconnect and reconcile. Certificate issuance, private-key storage, and rotation must be managed outside this package.

## Contract and safety boundary

Each connection begins with a heartbeat. The worker then sends a complete reconciliation_snapshot frame with account state, every currently open order and every open position. Values representing money, prices and quantities are encoded as decimal strings; symbol names are preserved exactly. The sequence is persisted locally before sending so process restarts do not silently reset it. A send failure can create a sequence gap, which must force reconciliation.

The worker and daemon are read-only. Heartbeat capabilities disable market/limit/stop submission and cancellation, and the daemon exposes no order-dispatch command. Each poll now sends a market_data_snapshot containing exact broker symbol metadata (currencies, digits, point, tick size/value, contract size, volume min/max/step, trade mode) and the latest bid/ask tick timestamps for the configured MT5_SYMBOLS list. Rust rejects stale (>60 seconds), future, crossed, malformed, or symbol-mismatched quotes, then persists valid market data in mt5_bridge_market_data_snapshots separately from reconciliation readiness. Snapshot persistence is not proof that a Windows terminal has been smoke-tested. Do not proceed to order execution until a real demo-terminal smoke test, instrument metadata/fresh quote feed, credential lifecycle review, and all acceptance checks pass.

## Windows demo smoke test (QOS-015A / QOS-015B)

After applying migrations and starting the Rust daemon and Python worker in separate PowerShell sessions, open a third PowerShell session on the same Windows host. Set `MT5_EXPECTED_ACCOUNT_CURRENCY` to the expected demo-account currency, ensure the other variables above are set in that session, and run:

```powershell
.\adapters\mt5-python\scripts\mt5-demo-smoke.ps1
```

The script is read-only. It verifies the MT5 login/server/currency, exact configured symbols, fresh ticks and sizing metadata, the worker certificate DER fingerprint against `MT5_BRIDGE_PRINCIPAL_SHA256`, the local daemon listener, required PostgreSQL tables, fresh persisted market data and reconciliation, and at least two distinct market-data sequences. It never submits, modifies, or cancels orders. It intentionally fails if snapshots are stale, identities differ, symbols mismatch, or data is not persisted before the timeout.

Run this on the target Windows VPS with a demo account only. Capture the timestamped console output and redacted daemon/worker logs. After a passing first run, restart the worker and rerun the script to verify reconnect and reconciliation recovery. Separately review server/client private-key filesystem ACLs, certificate expiry, revocation, rotation procedure, and the exact pinned worker fingerprint. Do not paste passwords, private keys, full connection strings, or unredacted logs into tickets.

## Local contract tests

    python -m unittest discover -s tests -v

Tests use fake terminal data and do not require an MT5 installation, broker credentials, or network access. A demo-terminal smoke test must be run on the target Windows VPS before any order-execution work begins.


## macOS developer support and terminal providers

The Rust bridge and portable contract tests can run on macOS. The bundled MetaTraderTerminal uses the official MetaTrader5 Python package and remains Windows-only. The MT5 macOS application by itself does not provide that Windows Python API.

Run the macOS developer checks from the repository root:

    bash adapters/mt5-python/scripts/mt5-macos-preflight.sh

To run only the Python contract tests:

    bash adapters/mt5-python/scripts/mt5-macos-preflight.sh --skip-rust

To fail closed when a real terminal provider is unavailable:

    bash adapters/mt5-python/scripts/mt5-macos-preflight.sh --require-real-terminal

The worker now supports an explicit trusted provider module through MT5_TERMINAL_PROVIDER=module:function. The factory receives the validated Config and must return (terminal, constants), where terminal implements connect, connected, account_info, orders, positions, select_symbol, symbol_info, symbol_info_tick, and shutdown. Do not configure an unreviewed provider. This is an extension interface only: no native macOS MetaTrader provider is bundled, and setting this variable does not prove that a provider works or establish conformance.

A provider intended for real demo use must be separately tested against the actual terminal and account, use the same read-only protocol contract, preserve sequence state across restarts, and produce authenticated persisted snapshots. The current macOS preflight deliberately reports terminal conformance as NOT_RUN and never promotes local contract tests to release evidence.

See docs/QOS-015_MACOS_CONFORMANCE.md for the current platform boundary, certificate review helper, and acceptance requirements. Keep venue execution disabled.


## Native macOS MT5 terminal provider (QOS-015A)

**Implementation status: provider and MQL5 EA added; real-terminal verification is still pending.** The official \`MetaTrader5\` Python package remains Windows-only. On macOS, this provider uses a read-only MQL5 Expert Advisor inside the MetaTrader terminal and a native Python client over a loopback TCP socket. The EA reads terminal/account state and returns it to the existing worker; it does not expose any order submission, modification, or cancellation operation.

### Install and configure the terminal-side EA

1. Open the installed MetaTrader 5 macOS application and log in manually to the designated **demo** account. The provider does not log in to the broker or transmit the broker password.
2. In the terminal, open **File → Open Data Folder** (menu wording may vary) and copy \`adapters/mt5-python/mql5/QuantOSReadOnlyProvider.mq5\` into \`MQL5/Experts/\`.
3. Open the file in MetaEditor, compile it, and attach \`QuantOSReadOnlyProvider\` to a chart. Configure \`InpHost=127.0.0.1\`, \`InpPort=8765\`, \`InpSharedToken\` to the same secret as \`MT5_MACOS_PROVIDER_TOKEN\`, and \`InpAllowedSymbols\` to the exact comma-separated broker symbols in \`MT5_SYMBOLS\`.
4. Keep the terminal open and connected. The EA retries the loopback connection. It uses socket APIs only; it contains no trading calls. Do not enable order execution to use this read-only provider.

Generate a random local shared token and store it in a protected environment file (do not commit it):

    python3 -c 'import secrets; print(secrets.token_urlsafe(48))'

The shared token must be at least 32 characters with no whitespace. The Python listener binds only to \`127.0.0.1\`, checks the connecting peer is loopback, and authenticates the EA before issuing read requests.

### Provider environment

Set these variables in the protected environment used by the Python worker:

| Variable | Value / purpose |
|---|---|
| \`MT5_TERMINAL_PROVIDER\` | \`mt5_adapter.macos_socket_provider:create_terminal_provider\` |
| \`MT5_MACOS_PROVIDER_TOKEN\` | The same random token configured in the EA |
| \`MT5_MACOS_PROVIDER_HOST\` | Optional; must be \`127.0.0.1\` |
| \`MT5_MACOS_PROVIDER_PORT\` | Optional; defaults to \`8765\`, and must match \`InpPort\` |
| \`MT5_MACOS_PROVIDER_CONNECT_TIMEOUT_SECONDS\` | Optional; defaults to 60 seconds, range 5–300 |
| \`MT5_TERMINAL_PATH\` | Path/label for the installed MT5 app; retained as required worker configuration |
| \`MT5_LOGIN\`, \`MT5_SERVER\` | Expected login and exact broker server; checked against the connected terminal |
| \`MT5_SYMBOLS\` | 1–64 unique exact broker symbol names, matching \`InpAllowedSymbols\` |
| \`MT5_TERMINAL_ID\` | Stable deployment-assigned terminal identity |
| \`MT5_BRIDGE_HOST\`, \`MT5_BRIDGE_PORT\` | Rust bridge endpoint |
| \`MT5_BRIDGE_SERVER_NAME\`, \`MT5_BRIDGE_CA_FILE\` | TLS server name and CA bundle |
| \`MT5_BRIDGE_CLIENT_CERT\`, \`MT5_BRIDGE_CLIENT_KEY\` | Worker mTLS certificate and protected private key |
| \`MT5_SEQUENCE_FILE\` | Optional durable sequence checkpoint |
| \`MT5_POLL_INTERVAL_SECONDS\` | Optional snapshot interval, 1–300 seconds |

For this exact native provider, \`MT5_PASSWORD\` is not required because the user logs into MT5 inside the terminal UI. The Python provider never reads, stores, or transmits that password. Other provider configurations retain the existing password requirement.

Start the worker only after compiling/attaching the EA, or start it first and let the EA reconnect. The worker listens on loopback; the EA initiates the connection. If the EA does not connect, confirm the terminal is online, the EA is attached and enabled, the port/token match, and no local process occupies the port. Never expose this socket on a LAN interface.

### Protocol and release boundary

The local protocol uses newline-delimited UTF-8 requests and JSON responses. The only commands are \`PING\`, \`ACCOUNT\`, \`ORDERS\`, \`POSITIONS\`, \`SELECT_SYMBOL\`, \`SYMBOL_INFO\`, and \`TICK\`; there is no order placement, modification, or deletion command. Symbol requests are restricted by both the Python configured-symbol allowlist and the EA allowlist. The bridge traffic from Python to Rust continues to use the existing TLS 1.3 mutual-authentication configuration.

The MQL5 source is not compiled by the repository's Python/Rust CI. Unit tests cover the Python provider handshake, token and identity checks, protocol mappings, symbol allowlisting, and failure paths, but they do not prove that MetaTrader runs the EA correctly. Before treating the provider as operational, compile it in MetaEditor and run \`mt5-macos-demo-smoke.py\` against the real demo terminal and persisted Rust/PostgreSQL snapshots, then complete restart recovery and certificate lifecycle review. **The provider implementation is not itself a conformance PASS. Venue execution must remain disabled, and this does not replace Windows-specific release evidence.**

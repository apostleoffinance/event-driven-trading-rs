# macOS MT5 demo setup (LaunchAgent workflow)

This is the native macOS service-management path for the Quant OS read-only bridge and worker. It requires a **trusted terminal provider that can access a real MT5 demo terminal**. No native macOS MetaTrader5 Python provider is bundled in this repository today. The default MetaTrader5 Python integration remains Windows-only, so do not proceed to a real-terminal claim until a compatible provider has been implemented and independently tested.

## 1. Install prerequisites

- macOS with a supported Python 3 version, stable Rust toolchain, OpenSSL, and PostgreSQL client tools (psql).
- A reachable PostgreSQL database with the Quant OS migrations applied.
- A designated broker demo account only.
- Issued bridge certificates and a trusted CA bundle, with private keys stored outside the repository.
- A reviewed terminal provider module that implements the Terminal protocol described in adapters/mt5-python/README.md.

Install/build the project:

    cd /absolute/path/to/event-driven-trading-rs
    cargo build --release -p intent-bridge --bin mt5-bridge-daemon
    cd adapters/mt5-python
    python3 -m venv .venv
    .venv/bin/python -m pip install --upgrade pip
    .venv/bin/python -m pip install -e .

The terminal provider is not supplied by the package installation above. It must be separately installed and reviewed.

## 2. Create protected configuration and log directories

Use a directory outside the Git checkout for runtime configuration, sequence state, evidence, and logs:

    umask 077
    mkdir -p "$HOME/Library/Application Support/QuantOS/logs"
    chmod 700 "$HOME/Library/Application Support/QuantOS"
    chmod 700 "$HOME/Library/Application Support/QuantOS/logs"

Create the environment file with a text editor. Do not put secrets directly in shell commands or commit this file:

    nano "$HOME/Library/Application Support/QuantOS/quantos.env"
    chmod 600 "$HOME/Library/Application Support/QuantOS/quantos.env"

The file is shell-format configuration sourced by the launch wrapper. Quote values correctly. Set only approved values, including:

- MT5_TERMINAL_PROVIDER to the reviewed provider in module:function form.
- MT5_TERMINAL_PATH, MT5_LOGIN, MT5_PASSWORD, MT5_SERVER, MT5_ACCOUNT_ID, MT5_TERMINAL_ID, MT5_SYMBOLS, and MT5_EXPECTED_ACCOUNT_CURRENCY.
- MT5_BRIDGE_HOST=127.0.0.1, MT5_BRIDGE_PORT=9443, MT5_BRIDGE_SERVER_NAME, MT5_BRIDGE_CA_FILE, MT5_BRIDGE_CLIENT_CERT, MT5_BRIDGE_CLIENT_KEY, and an absolute MT5_SEQUENCE_FILE path.
- DATABASE_URL, MT5_BRIDGE_BIND=127.0.0.1:9443, MT5_BRIDGE_SERVER_CERT, MT5_BRIDGE_SERVER_KEY, MT5_BRIDGE_CLIENT_CA, and MT5_BRIDGE_PRINCIPAL_SHA256 for the daemon.
- MT5_BRIDGE_ALLOW_REMOTE_BIND=false unless a separately reviewed protected remote deployment is explicitly required.
- PYTHONPATH set to the absolute adapters/mt5-python directory if the provider module is installed there.

Never use the sequence checkpoint path inside a temporary directory. Do not set MT5_SEQUENCE_FILE to a path that can be reset during restart testing. Keep the environment file owner-only (0600); the launch wrapper refuses broader permissions.

## 3. Prepare LaunchAgents

Copy the examples into your user LaunchAgents directory and replace every /REPLACE_WITH_... path with an actual absolute path. Do not change the service labels unless you also pass the new exact labels to the restart verifier.

    mkdir -p "$HOME/Library/LaunchAgents"
    cp docs/macos/com.quantos.mt5-bridge.plist.example "$HOME/Library/LaunchAgents/com.quantos.mt5-bridge.plist"
    cp docs/macos/com.quantos.mt5-worker.plist.example "$HOME/Library/LaunchAgents/com.quantos.mt5-worker.plist"
    plutil -lint "$HOME/Library/LaunchAgents/com.quantos.mt5-bridge.plist"
    plutil -lint "$HOME/Library/LaunchAgents/com.quantos.mt5-worker.plist"

Create the log files' parent directory before loading the jobs. The plist files must not contain passwords, database URLs, or private-key bytes. The environment file path is not itself a secret; its contents are protected separately.

Load the daemon first, then the worker:

    launchctl bootstrap "gui/$(id -u)" "$HOME/Library/LaunchAgents/com.quantos.mt5-bridge.plist"
    launchctl bootstrap "gui/$(id -u)" "$HOME/Library/LaunchAgents/com.quantos.mt5-worker.plist"

Confirm both labels are loaded:

    launchctl print "gui/$(id -u)/com.quantos.mt5-bridge"
    launchctl print "gui/$(id -u)/com.quantos.mt5-worker"

Do not paste full launchctl output into tickets; retain it in the protected operations record and redact before sharing.

## 4. Run real demo smoke

In a terminal session, load the same protected environment without printing it:

    set -a
    source "$HOME/Library/Application Support/QuantOS/quantos.env"
    set +a

Run the provider-backed smoke test from the adapter package directory:

    cd /absolute/path/to/event-driven-trading-rs/adapters/mt5-python
    .venv/bin/python scripts/mt5-macos-demo-smoke.py --timeout-seconds 10 --evidence-out "$HOME/Library/Application Support/QuantOS/macos-smoke-evidence.json"

The smoke test checks the provider's authenticated login/server/currency, configured symbols and quote metadata, a TLS 1.3 mTLS handshake to the bridge, and fresh persisted market-data/reconciliation snapshots for the configured account/terminal identity. It requires at least two distinct recent market-data sequences. It is read-only and does not place, modify, or cancel orders.

If no verified provider exists, the smoke test must fail closed. Do not replace it with fixture data or declare a pass because the MetaTrader app window is open.

## 5. Verify worker-only restart recovery

With the same protected environment loaded in the current shell, run:

    .venv/bin/python scripts/mt5-macos-restart-verify.py --worker-label com.quantos.mt5-worker --daemon-label com.quantos.mt5-bridge --worker-plist "$HOME/Library/LaunchAgents/com.quantos.mt5-worker.plist" --timeout-seconds 240 --poll-seconds 5 --evidence-out "$HOME/Library/Application Support/QuantOS/macos-restart-evidence.json"

The verifier runs smoke first, unloads only the worker LaunchAgent, confirms the daemon remains loaded, bootstraps the worker again, waits for a greater market-data sequence and fresh post-stop market-data/reconciliation persistence, then runs smoke again. It never stops the daemon or deletes/reset sequence/database state. The evidence JSON is written with owner-only permissions.

Validate the restart JSON using the existing structural validator:

    .venv/bin/python scripts/validate_restart_evidence.py "$HOME/Library/Application Support/QuantOS/macos-restart-evidence.json"

A structural PASS does not authenticate the evidence or replace independent review.

## 6. Review certificates and permissions

Run the certificate helper with the protected paths and approved client fingerprint. If the current CRL is available, include --crl-file. The helper checks key pairing, certificate validity, CA-chain verification, pin equality, CRL verification when supplied, and local private-key/parent-directory permissions. It deliberately leaves runtime revocation enforcement, rotation/rollback, and the release gate unverified.

The macOS helper is not a substitute for Windows certificate/ACL review when the target deployment is Windows.

## 7. Acceptance and limitations

- A provider-backed smoke and restart pass is evidence for that exact macOS host, provider, account identity, repository commit, and configuration only.
- The worker/daemon remain read-only. Venue execution must remain disabled.
- Keep the raw evidence and logs in the protected operations store; redact operational identifiers before sharing.
- A macOS PASS does not satisfy Windows-specific service, certificate ACL, or target-host requirements in docs/MT5_DEMO_SMOKE_TEST.md.
- The release manifest must remain BLOCKED until every applicable QOS-015A, QOS-015B, QOS-013, certificate lifecycle, and independent-review gate is verified.

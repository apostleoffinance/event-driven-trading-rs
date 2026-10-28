# macOS MT5 demo setup (LaunchAgent workflow)

This is the native macOS service-management path for the Quant OS read-only bridge and worker. The repository now includes a native Python provider paired with `adapters/mt5-python/mql5/QuantOSReadOnlyProvider.mq5`. The official MetaTrader5 Python package remains Windows-only; the macOS path uses a read-only MQL5 EA over authenticated loopback TCP. Source and unit tests are present, but MetaEditor compilation and real demo-terminal verification remain mandatory and are not claimed as passed.

## 1. Install prerequisites

- macOS with a supported Python 3 version, stable Rust toolchain, OpenSSL, and PostgreSQL client tools (psql).
- A reachable PostgreSQL database with the Quant OS migrations applied.
- A designated broker demo account only.
- For production/shared deployments, issued bridge certificates and a trusted CA bundle; for local-only development, use section 2's explicitly non-production development CA. Keep private keys outside the repository.
- The MetaTrader 5 macOS terminal, logged in to a designated demo account, with the repository's read-only MQL5 EA compiled and attached to a chart. See `adapters/mt5-python/README.md` for the provider protocol and setup.

Install/build the project:

    cd /absolute/path/to/event-driven-trading-rs
    cargo build --release -p intent-bridge --bin mt5-bridge-daemon
    cd adapters/mt5-python
    python3 -m venv .venv
    .venv/bin/python -m pip install --upgrade pip
    .venv/bin/python -m pip install -e .

The Python provider ships with the repository. The terminal-side MQL5 EA must be copied into the terminal data folder, compiled in MetaEditor, attached to a chart, and configured with the same loopback port, shared token, and exact symbol allowlist as the worker.

## 2. Create local-development mTLS certificates (development only)

The repository includes a bootstrap helper for local development. It creates a private local CA, a TLS server certificate with `localhost` and `127.0.0.1` SANs, a client certificate with the correct EKU, and a CA CRL. It verifies certificate/key pairing, both certificate chains and CRL status, the server SAN, and certificate purpose before printing the client certificate's SHA-256 DER fingerprint.

**This creates a local development CA, not production-issued certificates.** Use it only for the loopback demo setup on this Mac. For a production or shared environment, use the organization's approved certificate issuance, storage, revocation, rotation, and recovery process. Never copy the CA private key to another host or commit any generated key.

From the repository root, run:

    python3 adapters/mt5-python/scripts/mt5-macos-dev-pki.py

The default output is the protected `~/Library/Application Support/QuantOS/certs` directory. The helper refuses to write into a non-empty output directory. Its private keys are owner-only (`0600`) and the directory is owner-only (`0700`). It prints the client fingerprint; keep that value private to the local configuration and set it as `MT5_BRIDGE_PRINCIPAL_SHA256`.

For this **local-only** certificate set, configure the protected `quantos.env` with these paths (keep quotes around paths containing spaces):

- `MT5_BRIDGE_SERVER_CERT="$HOME/Library/Application Support/QuantOS/certs/server.pem"`
- `MT5_BRIDGE_SERVER_KEY="$HOME/Library/Application Support/QuantOS/certs/server.key.pem"`
- `MT5_BRIDGE_CLIENT_CA="$HOME/Library/Application Support/QuantOS/certs/dev-ca.pem"`
- `MT5_BRIDGE_CA_FILE="$HOME/Library/Application Support/QuantOS/certs/dev-ca.pem"`
- `MT5_BRIDGE_CLIENT_CERT="$HOME/Library/Application Support/QuantOS/certs/client.pem"`
- `MT5_BRIDGE_CLIENT_KEY="$HOME/Library/Application Support/QuantOS/certs/client.key.pem"`
- `MT5_BRIDGE_PRINCIPAL_SHA256` = the exact fingerprint printed by the helper (64 hex characters, no separators).

The same public CA certificate is used to validate the server and client certificate in this isolated development setup. Keep `MT5_BRIDGE_BIND=127.0.0.1:9443` and `MT5_BRIDGE_ALLOW_REMOTE_BIND=false`.

Run the certificate review helper with the generated CRL after configuring the fingerprint:

    python3 adapters/mt5-python/scripts/mt5-macos-certificate-review.py --client-cert "$HOME/Library/Application Support/QuantOS/certs/client.pem" --client-key "$HOME/Library/Application Support/QuantOS/certs/client.key.pem" --server-cert "$HOME/Library/Application Support/QuantOS/certs/server.pem" --server-key "$HOME/Library/Application Support/QuantOS/certs/server.key.pem" --ca-file "$HOME/Library/Application Support/QuantOS/certs/dev-ca.pem" --expected-client-sha256 "$MT5_BRIDGE_PRINCIPAL_SHA256" --crl-file "$HOME/Library/Application Support/QuantOS/certs/dev-ca.crl.pem" --evidence-out "$HOME/Library/Application Support/QuantOS/certificate-review.json"

The expected result is a file-level `PARTIAL PASS`; the release gate intentionally remains `BLOCKED` until runtime revocation enforcement, rotation/rollback, real-terminal smoke and independent review are evidenced. The helper is exercised by the adapter unit tests.

## 3. Create protected configuration and log directories

Use a directory outside the Git checkout for runtime configuration, sequence state, evidence, and logs:

    umask 077
    mkdir -p "$HOME/Library/Application Support/QuantOS/logs"
    chmod 700 "$HOME/Library/Application Support/QuantOS"
    chmod 700 "$HOME/Library/Application Support/QuantOS/logs"

**Recommended: use the local setup helper** after generating the development certificates and building the daemon:

    python3 adapters/mt5-python/scripts/setup-macos-services.py

Run it with the Python interpreter/virtual environment where the adapter was installed. It prompts for the demo terminal identity and database URL if DATABASE_URL is not already exported, then creates the protected `quantos.env` and both LaunchAgent plist files with absolute paths. It calculates the client certificate fingerprint locally and generates a fresh provider token; it does not print the token or start either service. It refuses to overwrite existing configuration files.

The helper copies the new provider token to the clipboard when `pbcopy` is available and stores a private backup at `~/Library/Application Support/QuantOS/certs/provider-token.txt` with mode 0600. Paste the token into the EA's `InpSharedToken` input in MetaTrader and apply the EA inputs before starting the worker. Never share the token or commit the backup.

For manual configuration instead, create the environment file with a text editor and do not put secrets directly in shell commands or commit this file:

    nano "$HOME/Library/Application Support/QuantOS/quantos.env"
    chmod 600 "$HOME/Library/Application Support/QuantOS/quantos.env"

The file is shell-format configuration sourced by the launch wrapper. Quote values correctly. Set only approved values, including:

- QUANTOS_REPOSITORY_COMMIT set to the exact deployed 40- or 64-character Git commit SHA.
- MT5_TERMINAL_PROVIDER to the reviewed provider in module:function form.
- MT5_TERMINAL_PATH, MT5_LOGIN, MT5_SERVER, MT5_ACCOUNT_ID, MT5_TERMINAL_ID, MT5_SYMBOLS, and MT5_EXPECTED_ACCOUNT_CURRENCY. For the native MQL5 provider, set `MT5_TERMINAL_PROVIDER=mt5_adapter.macos_socket_provider:create_terminal_provider` and `MT5_MACOS_PROVIDER_TOKEN`; `MT5_PASSWORD` is not needed because login occurs in the terminal UI.
- MT5_BRIDGE_HOST=127.0.0.1, MT5_BRIDGE_PORT=9443, MT5_BRIDGE_SERVER_NAME, MT5_BRIDGE_CA_FILE, MT5_BRIDGE_CLIENT_CERT, MT5_BRIDGE_CLIENT_KEY, and an absolute MT5_SEQUENCE_FILE path.
- DATABASE_URL, MT5_BRIDGE_BIND=127.0.0.1:9443, MT5_BRIDGE_SERVER_CERT, MT5_BRIDGE_SERVER_KEY, MT5_BRIDGE_CLIENT_CA, and MT5_BRIDGE_PRINCIPAL_SHA256 for the daemon.
- MT5_BRIDGE_ALLOW_REMOTE_BIND=false unless a separately reviewed protected remote deployment is explicitly required.
- `MT5_MACOS_PROVIDER_PORT=8765` (or a matching custom port), and `PYTHONPATH` set to the absolute adapters/mt5-python directory.

Never use the sequence checkpoint path inside a temporary directory. Do not set MT5_SEQUENCE_FILE to a path that can be reset during restart testing. Keep the environment file owner-only (0600); the launch wrapper refuses broader permissions.

## 4. Prepare and load LaunchAgents

If you used `setup-macos-services.py`, it has already generated both plist files with absolute paths and run `plutil -lint` on them. Otherwise, copy the examples into your user LaunchAgents directory and replace every /REPLACE_WITH_... path with an actual absolute path:

    mkdir -p "$HOME/Library/LaunchAgents"
    cp docs/macos/com.quantos.mt5-bridge.plist.example "$HOME/Library/LaunchAgents/com.quantos.mt5-bridge.plist"
    cp docs/macos/com.quantos.mt5-worker.plist.example "$HOME/Library/LaunchAgents/com.quantos.mt5-worker.plist"
    plutil -lint "$HOME/Library/LaunchAgents/com.quantos.mt5-bridge.plist"
    plutil -lint "$HOME/Library/LaunchAgents/com.quantos.mt5-worker.plist"

Before loading either job, verify the generated environment values and make sure the EA has the matching fresh token. Do not change service labels unless you also pass the new exact labels to the restart verifier.

Create the log files' parent directory before loading the jobs. The plist files must not contain passwords, database URLs, or private-key bytes. The environment file path is not itself a secret; its contents are protected separately.

Load the daemon first, then the worker:

    launchctl bootstrap "gui/$(id -u)" "$HOME/Library/LaunchAgents/com.quantos.mt5-bridge.plist"
    launchctl bootstrap "gui/$(id -u)" "$HOME/Library/LaunchAgents/com.quantos.mt5-worker.plist"

Confirm both labels are loaded:

    launchctl print "gui/$(id -u)/com.quantos.mt5-bridge"
    launchctl print "gui/$(id -u)/com.quantos.mt5-worker"

Do not paste full launchctl output into tickets; retain it in the protected operations record and redact before sharing.

## 5. Run real demo smoke

In a terminal session, load the same protected environment without printing it:

    set -a
    source "$HOME/Library/Application Support/QuantOS/quantos.env"
    set +a

Run the provider-backed smoke test from the adapter package directory:

    cd /absolute/path/to/event-driven-trading-rs/adapters/mt5-python
    .venv/bin/python scripts/mt5-macos-demo-smoke.py --timeout-seconds 10 --evidence-out "$HOME/Library/Application Support/QuantOS/macos-smoke-evidence.json"

The default smoke test directly connects to the MQL5 EA and checks authenticated login/server/currency, configured symbols and quote metadata, TLS 1.3/mTLS client configuration, and fresh persisted market-data/reconciliation snapshots from the configured account/terminal identity. Because the EA supports one client at a time, stop the supervised worker before running the direct probe; restart it promptly afterward. The verifier uses `--persisted-only` while the worker is active to avoid opening a competing EA connection. It checks that the latest market and reconciliation snapshots match in sequence and identity; the restart verifier separately proves sequence advancement across restart. The smoke is read-only and does not place, modify, or cancel orders.

If no verified provider exists, the smoke test must fail closed. Do not replace it with fixture data or declare a pass because the MetaTrader app window is open.

## 6. Verify worker-only restart recovery

With the same protected environment loaded in the current shell, run:

    .venv/bin/python scripts/mt5-macos-restart-verify.py --worker-label com.quantos.mt5-worker --daemon-label com.quantos.mt5-bridge --worker-plist "$HOME/Library/LaunchAgents/com.quantos.mt5-worker.plist" --timeout-seconds 240 --poll-seconds 5 --evidence-out "$HOME/Library/Application Support/QuantOS/macos-restart-evidence.json"

The verifier checks the active worker's fresh persisted stream, unloads only the worker LaunchAgent, confirms the daemon remains loaded, bootstraps the worker again, waits for a greater market-data sequence and fresh post-stop market-data/reconciliation persistence, then checks the persisted stream again. It uses persisted-only smoke mode so the single-client MQL5 EA is never asked to serve a competing probe connection. It never stops the daemon or deletes/reset sequence/database state. The evidence JSON is written with owner-only permissions.

Validate the restart JSON using the existing structural validator:

    .venv/bin/python scripts/validate_restart_evidence.py "$HOME/Library/Application Support/QuantOS/macos-restart-evidence.json"

A structural PASS does not authenticate the evidence or replace independent review.

## 7. Review certificates and permissions

Run the certificate helper with the protected paths and approved client fingerprint. If the current CRL is available, include --crl-file. The helper checks key pairing, certificate validity, CA-chain verification, pin equality, CRL verification when supplied, and local private-key/parent-directory permissions. It deliberately leaves runtime revocation enforcement, rotation/rollback, and the release gate unverified.

The macOS helper is not a substitute for Windows certificate/ACL review when the target deployment is Windows.

## 8. Acceptance and limitations

- A provider-backed smoke and restart pass is evidence for that exact macOS host, provider, account identity, repository commit, and configuration only.
- The worker/daemon remain read-only. Venue execution must remain disabled.
- Keep the raw evidence and logs in the protected operations store; redact operational identifiers before sharing.
- A macOS PASS does not satisfy Windows-specific service, certificate ACL, or target-host requirements in docs/MT5_DEMO_SMOKE_TEST.md.
- The release manifest must remain BLOCKED until every applicable QOS-015A, QOS-015B, QOS-013, certificate lifecycle, and independent-review gate is verified.

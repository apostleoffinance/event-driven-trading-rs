# QOS-015 macOS developer and conformance support

**Current status: a native macOS MQL5 socket provider and terminal-side read-only EA are implemented; the current local source now uses MT5 Bridge v2 freshness states and passes automated adapter/Rust tests. MetaEditor compiled an isolated copy of the staged EA with 0 errors and 0 warnings, but the Wine process returned exit code 1. The updated EA has not been installed or reloaded. Real macOS MT5 terminal conformance remains BLOCKED pending a cleanly verified deployment, fresh demo data, persistence/restart checks, and the remaining release gates.**

This document separates operating-system support from terminal API support. The Rust bridge and protocol checks can run natively on macOS. The existing MetaTraderTerminal implementation in adapters/mt5-python/mt5_adapter/worker.py uses the official MetaTrader5 Python package and intentionally rejects non-Windows platforms. Installing the MetaTrader 5 macOS application alone does not make that Windows Python API available to native macOS Python.

## Supported on macOS in this iteration

- Run the portable Python adapter contract tests.
- Run Rust format, workspace check, test, and Clippy checks.
- Inspect certificate/key pairs, the pinned client-certificate SHA-256 fingerprint, certificate validity, CA-chain verification, optional CRL verification, and local key-file/parent-directory permissions.
- Produce local diagnostic output without promoting it to Windows or release evidence.

## Not yet a real-terminal PASS

The macOS preflight reports the real terminal integration as NOT_RUN unless a provider is configured, and still blocks release approval when --require-real-terminal is used. A provider variable or passing unit tests are not proof that the EA works. The native provider uses `adapters/mt5-python/mql5/QuantOSReadOnlyProvider.mq5` plus `mt5_adapter.macos_socket_provider`; follow `adapters/mt5-python/README.md` for terminal setup. A real implementation must establish all of the following against the designated demo account:

1. Authenticated terminal login matches the configured account and exact server.
2. Account currency, exact broker symbols, bid/ask, timestamps, and instrument specifications come from the actual terminal.
3. The adapter emits the same versioned heartbeat, market-data, and reconciliation contracts as the Windows adapter.
4. The Rust bridge accepts the expected mTLS identity and persists the snapshots to PostgreSQL.
5. A worker-only restart advances the persisted sequence and results in fresh post-restart market-data and reconciliation snapshots while the daemon remains running.
6. Certificate trust, fingerprint pinning, revocation enforcement, key permissions, rotation, rollback, and rejection of a retired/untrusted identity are separately verified.
7. Evidence is redacted, tied to the exact repository commit, and independently reviewed.

Do not use fabricated or simulated data to satisfy these requirements.

## MT5 quote clock and freshness

`TimeCurrent()` is the last quote time and can remain frozen when no new quote arrives. The native EA therefore sends `TimeTradeServer()` as its current server-time reference. The adapter estimates the server/host timezone-like offset, normalizes the original quote timestamp, and retains the strict 60-second UTC-age guard. A stale tick now produces a v2 `stale` market-data status with its actual bid/ask and original timestamp; a missing tick produces `unavailable` with null prices/timestamp. Neither condition ends the authenticated worker session. Future/invalid timestamps produce an explicit `invalid` state, not a fresh quote. Do not loosen freshness checks or fabricate prices to make a closed market pass.

The 2026-10-31 Mac diagnostic showed a frozen `TimeCurrent()`/quote timestamp and an advancing `TimeTradeServer()`, while host NTP offset was approximately 0.45 seconds. The earlier EA was compiled with zero errors/warnings, installed in the Wine terminal's Experts folder, and reattached. Live diagnostics confirmed the server-time field advances; the then-running worker rejected the configured quote because it was stale by hours. For the staged EA build 2.00, an isolated MetaEditor compile log reports `Result: 0 errors, 0 warnings` and `code generated` (`X64 Regular`, 2118 ms), but the Wine launcher returned status 1. No source or binary was copied into the terminal. The installed EA source and binary hashes differ from the current source/build, and the exact binary loaded by the active chart could not be established from open file handles or timestamp-correlated terminal logs.

The supervised worker and daemon run from `~/rust-trading-engine/event-trading` at `3e1259063d3194e5b26cf93766ba50306c13fec1`; both source sides still use protocol v1. The checkout has local modifications to `Cargo.lock`, the macOS launch wrapper, and `docker-compose.yml`, which were left untouched. This is incompatible with the v2 worker/daemon in the current repository, so live v2 testing was not attempted. One earlier quote was persisted fresh at sequence 5329, together with reconciliation data, but no snapshots had been added since 02:15:42Z and the stream was degraded at sequence 5330. Until the v2 worker and daemon are staged together, the EA-compatible provider is installed in a confirmed active data folder, and real fresh data/recovery is verified, QOS-015 remains blocked.

## Controlled v2 runtime conformance plan — prepared, not run

Do not run the real-terminal matrix against the current v1 worker/daemon. First stage a matching v2 worker and daemon from the same reviewed commit, preserve all existing backups/configuration/checkpoints, confirm the active EA data folder and binary, verify the expected demo account/server/currency, and confirm the daemon still exposes no order execution. Keep the sequence checkpoint and PostgreSQL snapshots intact. Capture redacted UTC-timestamped worker, daemon, EA, and database evidence.

| Scenario | Automated evidence | Real-terminal status and required result |
|---|---|---|
| A. Weekend/stale data | Python worker and Rust mTLS tests cover stale-session continuity; reconciliation persistence and stale-sizing rejection are covered. | **NOT RUN.** On a connected demo account with no new tick, verify stable authenticated socket/heartbeats, account/orders/positions reconciliation, stale or unavailable persisted status, degraded readiness, and rejected sizing. |
| B. Fresh data | Rust mTLS test covers fresh-state recovery after a stale cycle; worker and persistence tests cover current quote encoding/storage. | **NOT RUN.** Wait for an actual broker tick; verify original tick timestamp, strict age limit, persisted fresh quote, full reconciliation, and readiness recovery. No synthetic data or forced timestamp is allowed. |
| C1. Worker restart | Durable sequence/persistence tests exist. | **NOT RUN.** Restart only the worker; keep daemon running and verify checkpoint continuity plus new market-data and reconciliation snapshots. |
| C2. Daemon restart | Persistence/reconciliation restart tests exist. | **NOT RUN.** Restart daemon with worker paused; preserve database and sequence state, then verify reconnect, reconciliation, and monotonically advancing persisted sequence. |
| C3. Terminal restart | No live-terminal evidence. | **NOT RUN.** Restart the demo terminal only, then verify EA reconnect, account identity, new heartbeat, and data/reconciliation persistence. |
| C4. Socket disconnect | Adapter EOF/I/O and transport recovery paths have automated tests. | **NOT RUN.** Force a controlled socket drop; verify it is classified as transport failure and reconnects without bypassing reconciliation. |
| C5. Authentication failure | mTLS integration tests reject untrusted client certificates. | **NOT RUN.** Use a controlled untrusted test identity; verify authentication is rejected and readiness/exposure stays fail-closed. |
| C6. Protocol mismatch | Rust contract tests reject unsupported schema versions. | **NOT RUN.** Present a v1 frame to the v2 listener in an isolated test; verify explicit rejection, no v1 reinterpretation, and no readiness. |

Automated PASS results are not real-terminal PASS evidence. Do not enable order execution or mark Phase 1 complete from this matrix until each applicable real-host test has evidence tied to the deployed commit.

## QOS-015A — protocol v2 staging and deployment readiness

**Staging status:** The v2 source has been overlaid onto a detached worktree at base commit `aa0cc3da3258067f46da4485c54f6f0427f67013`; the release daemon was built from that final staged tree and the Python contract suite passed (67 tests). The source snapshot fingerprint is `7a5924f59c0cede1980ba9ea925c051850ee297ad95494ca77d06657d2d70761`, computed over sorted file SHA-256 values excluding docs, status trackers, target outputs, Python caches, `.git`, and the manifest itself. The implementation is still uncommitted in the main checkout, so there is not yet a commit containing the reviewed v2 implementation. The staging worktree is an exact source snapshot for preparation, not a substitute for the release commit. Do not promote this snapshot until it is committed, reviewed, and rebuilt from that commit.

The bridge protocol and terminal-provider protocol are separate layers. The Python worker and Rust daemon speak MT5 Bridge **v2** (`schema_version: 2`). The MQL5 EA speaks the local loopback provider protocol **v1**; it does not emit bridge frames. The EA build is version `2.00` and now reports both `provider_build_version` and `provider_protocol_version` in its authenticated-by-token PING response. The Python worker rejects missing or mismatched values. This permits checking the loaded EA build without incorrectly labeling its local socket protocol as bridge v2.

The new `adapters/mt5-python/scripts/mt5_protocol_preflight.py` fails closed unless the worker and Rust sources declare bridge v2, the Python and MQL5 provider agree on provider protocol v1/build 2.00, the daemon binary is inside the staged checkout, and the worker, daemon, EA source, and EA binary match hashes from the reviewed build manifest. Run this preflight before **each** staged service start. The MQL5 response check provides a second, live rejection if an older EA is loaded. Hash values belong in a protected deployment manifest/evidence file, not in this guide.

### Current terminal/process evidence and limits

- The running terminal executable is `/Applications/MetaTrader 5.app`, Wine process `terminal64.exe`. The installation tree is under the Wine prefix's `drive_c/Program Files/MetaTrader 5`.
- The existing EA files observed in that installation tree are `MQL5/Experts/QuantOSReadOnlyProvider.mq5` (SHA-256 `eed359d586fe54e395b640497f3efd17236a7fbfcef8d1dbf6793b61850953cd`) and `.ex5` (SHA-256 `0eed63bf6f27ea81be6401f7f1b4b6f8d118bad11e2c45170c7ebac86ee94296`). The old source reports EA version `1.00`. Both files were only read; neither was overwritten.
- A terminal-specific roaming profile directory exists at `drive_c/users/user/AppData/Roaming/MetaQuotes/Terminal/D0E8209F77C8CF37AD8BF550E51FF075`, but it contains no MQL5/Experts directory. Terminal logs and MQL5 logs are present under the installation tree. The exact data-directory mode and exact EX5 image loaded by the current chart are **not conclusively established** by process listings or open file handles. Before deployment, use MT5 **File → Open Data Folder** and record the path; do not infer it from the install directory.
- After approval and EA reload, verify the Experts log reports `build=2.00 provider_protocol=1`, then verify the worker's PING handshake succeeds. The existing EA does not report this version marker, so the active chart is not presently verified as v2-compatible. Do not use the chart's displayed EA name alone as proof.
- A fresh isolated compile of the final staged source generated `QuantOSReadOnlyProvider.ex5`: source SHA-256 `f43bbf5c4f834f2763689066e6d57c15af789c533c14397f75f1122b0a8e002c`; EX5 SHA-256 `60c66148efef6acd1b6b8bdf0adc496aa19b440491495a56d927d24f9a41aa45`; output mtime `2026-10-31T04:43:18+01:00`; compiler log reports code generated, 0 errors, 0 warnings, X64 Regular, 2118 ms. Wine/MetaEditor still returned exit code 1 and emitted only Wine `fixme` diagnostics. A MetaEditor process was already running before the CLI compile and remained running, so GUI/single-instance handoff is a plausible explanation; the evidence does not prove that cause. Treat the compile log and generated artifact as compiler success, but keep the nonzero process status unresolved. The artifact remains isolated and has not been installed.
- The staged release daemon SHA-256 is `81b002f054aef7ea6734c160d57f54d9b644877f52eef2c9a164b6fce48a51ac` (release build with debug info stripped, built at `2026-10-31T04:47:22+01:00`). The Python worker SHA-256 is `3f0e71348ab8919bc72f439273eab5eb1e3dcfc2d459542d23d2a586c4c3db97`; provider SHA-256 is `fd9dfd87babe68ae66d59c27462f0626ad464331a7c5bf45a9d98bc20ae5477f`. The protocol preflight passed against these staged files and both EA artifacts. The full source snapshot remains uncommitted; its exact digest is recorded with the local staging evidence, not a Git commit.
- Active v1 worker and daemon remain in `~/rust-trading-engine/event-trading` at commit `3e1259063d3194e5b26cf93766ba50306c13fec1`; their three pre-existing local changes remain untouched. No active service or terminal process was restarted.

### Controlled deployment sequence (approval required before terminal switch)

Use an immutable staging checkout and dedicated staging ports, not the active v1 ports: daemon `127.0.0.1:19443`; EA-provider listener `127.0.0.1:18765`. Keep active v1 on its existing ports and checkout. Do not launch an EA against staging until an operator approves switching that chart; do not run two workers against the same terminal/account identity.

Deploy only these reviewed artifacts:

1. `target/release/mt5-bridge-daemon` built from the same committed staging revision as the worker.
2. The complete `adapters/mt5-python/mt5_adapter/` package, its declared runtime dependencies, and `scripts/mt5_protocol_preflight.py`.
3. `mql5/QuantOSReadOnlyProvider.mq5` for audit and its hash-matched compiled `.ex5` for the approved terminal data directory.
4. The v2 schema/docs and the redacted build manifest containing source commit, worker/daemon/source/binary SHA-256 hashes, compiler log result, and build timestamp.

Load environment variables from the protected service/secret manager; never put credential values in shell history or the manifest. The daemon requires `DATABASE_URL`, `MT5_BRIDGE_BIND`, `MT5_BRIDGE_SERVER_CERT`, `MT5_BRIDGE_SERVER_KEY`, `MT5_BRIDGE_CLIENT_CA`, `MT5_BRIDGE_PRINCIPAL_SHA256`, `MT5_TERMINAL_ID`, `MT5_SERVER`, and `MT5_ACCOUNT_ID`. The worker requires `MT5_TERMINAL_PROVIDER`, `MT5_TERMINAL_PATH`, `MT5_LOGIN`, `MT5_SERVER`, `MT5_TERMINAL_ID`, `MT5_BRIDGE_HOST`, `MT5_BRIDGE_PORT`, `MT5_BRIDGE_SERVER_NAME`, `MT5_BRIDGE_CA_FILE`, `MT5_BRIDGE_CLIENT_CERT`, `MT5_BRIDGE_CLIENT_KEY`, `MT5_SYMBOLS`, and `MT5_SEQUENCE_FILE`; the native provider additionally requires `MT5_MACOS_PROVIDER_TOKEN` and optionally `MT5_MACOS_PROVIDER_PORT=18765`. Set daemon bind and worker bridge port to `127.0.0.1:19443` / `19443`. Use a protected, staging-specific sequence file and token; do not reuse or reset the active v1 sequence state.

With the staged checkout and manifest values assigned through the protected deployment environment, run the preflight and then start the services in this order:

```sh
python3 "$STAGE/adapters/mt5-python/scripts/mt5_protocol_preflight.py" \
  --repo-root "$STAGE" \
  --daemon-binary "$STAGE/target/release/mt5-bridge-daemon" \
  --ea-source "$STAGE/adapters/mt5-python/mql5/QuantOSReadOnlyProvider.mq5" \
  --ea-binary "$STAGED_EA_EX5" \
  --expected-worker-sha256 "$EXPECTED_WORKER_SHA256" \
  --expected-provider-sha256 "$EXPECTED_PROVIDER_SHA256" \
  --expected-daemon-sha256 "$EXPECTED_DAEMON_SHA256" \
  --expected-ea-source-sha256 "$EXPECTED_EA_SOURCE_SHA256" \
  --expected-ea-binary-sha256 "$EXPECTED_EA_BINARY_SHA256" \
&& "$STAGE/target/release/mt5-bridge-daemon"
```

In the separate service environment, run the same preflight command first, then start `PYTHONPATH="$STAGE/adapters/mt5-python" python3 -m mt5_adapter.worker`. The two `&&`-guarded startup commands must use the same immutable stage and manifest. Before changing the active chart, verify the daemon process executable/cwd resolves under `$STAGE` (`ps` plus `lsof -a -p <pid> -d cwd,txt`), the worker command and cwd resolve under `$STAGE`, and its runtime PING reports provider build `2.00` / local protocol `1`. Confirm the daemon logs a v2 authenticated session and that persistence sequence advances only in the intended staging data scope.

**Rollback:** Before copying the EA, record hashes of the existing `.mq5`/`.ex5` and make a separately named, non-overwriting backup in the confirmed data directory. Copy the new EX5 to a temporary filename there, verify its hash, then rename into place only after explicit approval and with the chart detached. If any check fails, detach the new EA, restore the original files from the verified backup (never overwrite a backup), stop only staged processes, and resume the unchanged v1 services from their original checkout/configuration. Preserve PostgreSQL checkpoints and sequence files; do not roll them back or reset them as part of EA rollback.

### Runtime conformance readiness

The existing matrix above is the test plan; all real-terminal rows remain **NOT RUN**. Run stale/weekend, account/orders/positions reconciliation, fresh-tick recovery, worker/daemon/terminal restart, actual socket disconnect, authentication failure, protocol mismatch, and persistence/sequence-continuity checks one at a time with UTC-correlated redacted logs. The v1/v2 mismatch test must be isolated and must confirm explicit rejection without readiness. A market-closed result is only a stale/unavailable-data test; it cannot prove the fresh quote recovery row. Keep order execution disabled throughout.

## Local preflight

From the repository root on macOS, run:

    bash adapters/mt5-python/scripts/mt5-macos-preflight.sh

To run only Python adapter contract tests and skip Rust checks:

    bash adapters/mt5-python/scripts/mt5-macos-preflight.sh --skip-rust

To explicitly require real terminal support and fail closed while no verified provider is available:

    bash adapters/mt5-python/scripts/mt5-macos-preflight.sh --require-real-terminal

The preflight output is a local developer report, not a QOS-015 release manifest. Rust tests may need a reachable PostgreSQL test database, depending on the repository test configuration.

## Certificate review

Use the helper only with the protected demo/staging certificate files and a trusted CA bundle. The expected client fingerprint must be obtained from the approved deployment configuration; never copy private-key bytes or secrets into the command, repository, or evidence report.

    python3 adapters/mt5-python/scripts/mt5-macos-certificate-review.py --client-cert /protected/path/client-cert.pem --client-key /protected/path/client-key.pem --server-cert /protected/path/server-cert.pem --server-key /protected/path/server-key.pem --ca-file /protected/path/bridge-ca.pem --expected-client-sha256 YOUR_APPROVED_64_HEX_FINGERPRINT --crl-file /protected/path/current-ca.crl --evidence-out /protected/path/macos-certificate-review.json

Omit --crl-file only when you explicitly accept that the report will be blocked for missing revocation evidence. A CRL check is a point-in-time file-level check; it does not prove the running TLS stack enforces revocation. The helper never reports the overall release gate as PASS. Runtime negative tests and rotation/rollback evidence remain required.

## macOS services and restart recovery

macOS uses launchd, not Windows Services. Service supervision can be implemented with per-user LaunchAgents or system LaunchDaemons, but the service definitions must not contain broker passwords, private-key contents, or database credentials in plaintext. Use a protected configuration mechanism and ensure the service identity can read only the required key files.

A restart test must stop/restart only the worker and must not reset or delete the durable sequence checkpoint or database snapshots. It must verify the daemon remained running, the market-data sequence advanced, and fresh market-data plus reconciliation snapshots were persisted after the worker stopped. The native provider, read-only MQL5 EA, smoke tooling, and LaunchAgent restart verifier are present. Until the EA is reloaded and verified against fresh data from a real terminal, these commands cannot establish a real-terminal PASS.

## Capturing macOS EA/provider reconnect diagnostics

When the EA reconnects or reports a socket read failure, capture Toolbox output together with timestamped worker and daemon logs for the same reproduction window. Use separate Terminal windows, create a protected log directory, and do not enable shell tracing or print environment variables:

    umask 077
    mkdir -p "$HOME/mt5-diagnostics"
    cd /path/to/event-driven-trading-rs
    RUST_LOG=info cargo run -p intent-bridge --bin mt5-bridge-daemon 2>&1 | while IFS= read -r line; do printf '%s %s\n' "$(date -u '+%Y-%m-%dT%H:%M:%SZ')" "$line"; done | tee "$HOME/mt5-diagnostics/daemon.log"

In another Terminal window, from the adapter directory and with the protected runtime configuration already loaded:

    umask 077
    cd /path/to/event-driven-trading-rs/adapters/mt5-python
    python3 -u -m mt5_adapter.worker 2>&1 | while IFS= read -r line; do printf '%s %s\n' "$(date -u '+%Y-%m-%dT%H:%M:%SZ')" "$line"; done | tee "$HOME/mt5-diagnostics/worker.log"

Start the daemon first, then the worker, attach/keep the read-only EA on the demo chart, reproduce the reconnect, and stop both processes after a short shared interval. Save the Toolbox Expert output for that same interval. The current-checkout worker emits its own UTC timestamp plus a fixed stage (`provider_connect`, `tls_context`, `bridge_tcp_connect`, `bridge_tls_handshake`, or `snapshot_poll`); OS/TLS error text is intentionally omitted, while a numeric errno may be included. A supervised worker from a separate older checkout may not include these stage labels. Redact account/login identifiers, broker/server and host details, local paths, certificate fingerprints, tokens, and other infrastructure identifiers before sharing any output.

The supplied Toolbox excerpt proves the EA sent its handshake and received PING commands, but it does not log the PING responses. In the provider source, the worker accepts the `HELLO` token before issuing its first PING, and its request/response calls are synchronous. Repeated PINGs strongly suggest the preceding PING/PONG exchanges completed, but the EA log does not independently show the responses. The worker's next stages are TLS-context creation and the Rust daemon TCP/TLS connection; snapshot commands follow only after that succeeds. Thus PING-only output points to a failure before snapshot polling, but does not identify which subsequent stage failed.

The EA reports `SocketRead` returning `-1`, error `5273`, and `SocketIsConnected=false`; its current `DrainSocket` logic then closes its local socket. This establishes that the peer was observed disconnected at that check, but neither the error code nor the screenshot establishes which process initiated the original close. The worker's reconnect path calls the macOS provider's `shutdown()` before accepting the next EA connection, so a worker retry can subsequently close the old session. Correlate timestamps from all three sources before attributing the initial disconnect or changing socket behavior.

## Release policy

- Local macOS tests can prove portable code contracts only.
- macOS native certificate/service checks cannot substitute for Windows ACL/service evidence when the target deployment is Windows.
- Windows deployment remains subject to docs/MT5_DEMO_SMOKE_TEST.md and the consolidated QOS-015 release manifest.
- Keep venue_execution_enabled explicitly false. No order submission or cancellation work is authorized by this developer tooling.

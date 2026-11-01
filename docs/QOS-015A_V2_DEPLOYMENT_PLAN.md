# QOS-015A — MT5 Bridge v2 RC1 deployment and rollback plan

**Status: PREPARED — NOT APPROVED OR EXECUTED.** Deployment steps remain unrun. Read-only diagnostics were performed 2026-11-01; no v1 service, terminal, EA, certificate, or credential was modified. Order execution is disabled throughout.

Release evidence: implementation commit `76006c1`, evidence commit `7bfd435`, branch `release/mt5-bridge-v2-rc1`. `main` (`d9c448f`) lacks the RC quote-freshness fix and must not be merged or deployed.

## 1. Environment inventory (observed)

| Item | Observation | Status |
|---|---|---|
| RC branch | local HEAD and `origin/release/mt5-bridge-v2-rc1` are `7bfd4358…`; `76006c1` is in history | VERIFIED |
| `origin/main` | `d9c448f` (no freshness fix) | VERIFIED |
| v1 launchd jobs | `com.quantos.mt5-worker`, `com.quantos.mt5-bridge`; `KeepAlive=true`, `RunAtLoad=true`, `ThrottleInterval=10`; plists in `~/Library/LaunchAgents`; both use `QUANTOS_ENV_FILE=~/Library/Application Support/QuantOS/quantos.env` | VERIFIED |
| v1 worker | launchd-managed; runs `mt5_adapter.worker` from `~/rust-trading-engine/event-trading/adapters/mt5-python`; listens on 127.0.0.1:8765 (current PID observed 22853 at 19:48) | VERIFIED (PIDs are not stable) |
| v1 daemon | `~/rust-trading-engine/event-trading/target/release/mt5-bridge-daemon`, sha256 `734f58cf…86bb39`; current PID observed 22894 at 19:48; stderr contains repeated `Error: Db(PoolTimedOut)`; worker stderr contains repeated `ConnectionRefusedError` | VERIFIED — unhealthy/restarting; not touched |
| MT5 terminal | No `terminal64.exe` process was observed at the latest check (2026-11-01 19:48); the earlier process observation is historical | VERIFIED at latest check |
| Chart EA evidence | Historical MQL5 logs show `QuantOSReadOnlyProvider (EURUSD,H1)` connecting to 127.0.0.1:8765, answering PING, and reconnecting about every 30 s (`error=5273`), consistent with the v1 worker | Historical log evidence; terminal not currently running |
| MT5 install tree | `…/net.metaquotes.wine.metatrader5/drive_c/Program Files/MetaTrader 5` | VERIFIED |
| Candidate data dir | `…/drive_c/users/user/AppData/Roaming/MetaQuotes/Terminal/D0E8209F77C8CF37AD8BF550E51FF075` contains only `config`, `temp`, `origin.txt` (-> `C:\Program Files\MetaTrader 5`) and an empty `portable.txt`; **no `MQL5/` folder** | Observed layout suggests the install tree is the effective data root (portable mode). **Full path UNCONFIRMED until the operator reports File > Open Data Folder** |
| Experts directory (observed) | `<install tree>/MQL5/Experts/` holds `QuantOSReadOnlyProvider.mq5` (14,389 B, sha256 `eed359d5…0953cd`, EA build **1.00**) and `.ex5` (40,736 B, sha256 `0eed63bf…ee94296`, 31 Oct 03:24) | VERIFIED. The `.mq5` present is the **v1 source**, not RC source `f43bbf5c…`. Operator-copied relative path `MQL5/Experts/QuantOSReadOnlyProvider.mq5` matches, but the content is not the RC |
| Disk | **4.0 GiB free** at 21:34 after targeted Rust checks; earlier checks observed 5.1 GiB at start, 4.5 GiB, 2.1 GiB, 1.6 GiB, and 583 MiB | BLOCKER: below the 5 GiB preparation target; no release build or cleanup was attempted |
| Postgres | Configured v1 endpoint is `127.0.0.1:15433/trading`; read-only connection attempt was refused. A different listener on 5433 is reachable but rejected the configured database role. | OPEN — likely endpoint/config mismatch; no config changed |

### Read-only database diagnosis (2026-11-01, 19:48 +01)

- Both v1 LaunchAgents load `~/Library/Application Support/QuantOS/quantos.env` through the repository's launch wrapper. The wrapper requires a user-owned, non-symlink environment file with mode `0600` or `0400`. The daemon reads `DATABASE_URL`; no credential value was printed or changed.
- Parsing the configured URI without printing its username/password identified database `trading` at `127.0.0.1:15433`. A read-only PostgreSQL query failed with **connection refused** at that exact endpoint. TCP readiness succeeded at `127.0.0.1:5433`, but the same read-only query on that different port was rejected because the configured database role does not exist there. The repository's development Compose file maps its PostgreSQL container to host port `15432`, which is a third endpoint and is not evidence that the v1 target is available.
- The daemon calls `persistence::connect(DATABASE_URL)` before binding its listener. Therefore an unavailable database prevents the bridge from accepting worker connections; launchd has `KeepAlive=true`, consistent with the observed repeated daemon errors and worker connection refusals. The port mismatch is a likely explanation for the current failure, not proof of the original timeout's sole cause. Credentials, database existence, migrations, and the intended server were not verified.
- The v2 daemon uses the same `DATABASE_URL`/`TradingStore` persistence contract and migrations. It has no built-in database isolation. Pointing v2 at the v1 URI would share the same database/schema; **staging must use a separately approved database/instance and sequence namespace**. No staging database is currently verified.
- Required connection configuration: PostgreSQL URI for the intended host/port/database, a valid least-privilege role/password, network reachability, and privileges required by the applied migrations. Keep it only in the protected service environment. Do not copy the development Compose credentials into a deployment.
- Detection/recovery: current v1 detection is process/launchd state plus daemon/worker stderr; `pg_isready` alone is not sufficient because it does not authenticate or prove the target database. Verify the configured endpoint with an authenticated read-only query and migration-state check. V2 startup currently gates listener binding on a successful pool connection; runtime persistence errors degrade the session, but there is no independent database-health endpoint. After an approved staging configuration, verify stable daemon uptime, DB readiness, persisted market-data and reconciliation rows, monotonic sequence continuity, then fresh-data recovery. Do not restart or change v1 as part of this plan.

### Prepared v2 staging helpers (not executed)

- `docker-compose.mt5-v2-staging.yml` defines only a PostgreSQL service, bound to `127.0.0.1:55439`, with a project-scoped persistent volume. Admin username and password have no repository defaults and must come from the environment. This is separate from the development Compose service and v1 endpoint. The service was not started.
- `scripts/mt5-v2-staging-init-app-role.sh` accepts only an admin URL for `127.0.0.1:55439/mt5_v2_staging`, provisions the fixed `mt5_v2_app` login with non-superuser privileges, and reads its password from `MT5_V2_STAGING_APP_PASSWORD`. It does not create a database, print credentials, or target another host. It requires `python3` and `psql`; it was syntax-checked and its non-staging URL rejection was tested, but provisioning was not run.
- `cargo run --locked -p persistence --bin persistence-migrate` runs the existing SQLx migrations only when explicitly invoked with `DATABASE_URL`. The daemon does not invoke it automatically. Running without `DATABASE_URL` was verified to fail closed; no migration was applied.
- `Mt5HealthSnapshot` models the eight independent health signals and reports `HEALTHY` only when all are healthy; unknown/degraded values yield overall degraded, and any unhealthy signal yields unhealthy. It is an internal model only: no endpoint, daemon wiring, or order-execution behavior was added.
- `check_database_health` executes `SELECT 1`. The new SQLx test verifies a closed application pool fails the probe and a newly opened pool recovers. Its test target compiles, but it was not run because no `DATABASE_URL`/test database is configured; it does not simulate an actual PostgreSQL server outage.

## 2. Artifact inventory

| Artifact | Identity | Status |
|---|---|---|
| RC source | `76006c16…` (26 files); docs follow-up `7bfd4358…` | VERIFIED on GitHub |
| Python worker `worker.py` | sha256 `3f0e7134…c3db97` | VERIFIED |
| Python provider `macos_socket_provider.py` | sha256 `fd9dfd87…e5477f` | VERIFIED |
| RC daemon (path-remapped) | sha256 `bbc9e103…f5d36`, 7,808,064 bytes; two existing build outputs match exactly; rustc/cargo 1.98.1, locked release build with path remapping for the worktree, `~/.cargo`, and `~/.rustup`; located at `../rc-verification/build-1/release/mt5-bridge-daemon` | HASH/SIZE VERIFIED; exact remap destination values are not preserved, so do not claim independent reproduction from this checkout |
| Unremapped daemon | `179eac93…` (in-repo `target/release`) | Not byte-reproducible: it embeds absolute home paths; superseded by the remapped build for deployment |
| Non-RC daemon | `~/event-driven-trading-rs/target/release/mt5-bridge-daemon`, `14ef1871…` | NON-RC, provenance inferred; never deploy |
| EA source (RC) | `adapters/mt5-python/mql5/QuantOSReadOnlyProvider.mq5`, sha256 `f43bbf5c…e002c`, build 2.00, provider protocol 1; contains no trade API (grep for `OrderSend`, `CTrade`, `Position*`, `TRADE_ACTION`: none) | VERIFIED read-only by inspection |
| EA binary (RC) | `files/mt5-v2-artifacts/QuantOSReadOnlyProvider.ex5`, `60c66148…` (41,036 B). MetaEditor output is nondeterministic (other compiles: `9467c3e1…`, `a98fb2eb…`, `5496ea69…`) | **NOT ACCEPTED**: accept only after compiler evidence + source hash + artifact metadata + runtime PING build/protocol check after an approved reload |
| MetaEditor | `/compile` exits **1 on success, 0 on failure** here; the `/log` file with `0 errors` and an existing EX5 are the evidence | VERIFIED by probe |

Protocols: Python worker <-> Rust daemon = bridge **schema v2**; Python worker <-> EA = provider **protocol v1**, EA build **2.00**.

### Artifact preflight
`adapters/mt5-python/scripts/mt5_protocol_preflight.py` accepts the explicitly supplied `--daemon-binary` path regardless of its location. It rejects symlink/non-regular/non-executable daemon files and requires a valid expected SHA-256 plus positive expected byte size. Worker/provider/daemon/EA source/EX5 hashes and the source protocol declarations are checked; the EX5 must also be non-empty and have the expected EX5 header. This confirms reviewed artifact identity and static compatibility only; it cannot establish that the EX5 loads or that a terminal, broker, or database is healthy. Sixteen focused preflight tests pass after replacing the former artifact-root restriction. Real-binary invocation and full adapter-suite results are recorded in the current manifest; no build was performed in this turn.

## 3. Account binding specification (TenTrade demo)

Expected identity: broker/company **TenTrade**; login **788251**; server **TenTrade-Server**; currency **USD**; account type **Demo**.

| Property | Current implementation | Required (gap) |
|---|---|---|
| Login | Worker compares `account.login` with `MT5_LOGIN` (`worker.py`) and fails with `AdapterError` | Keep; also assert on every snapshot |
| Server | Worker compares `account.server` with `MT5_SERVER` | Keep |
| Currency | Read and required non-empty, but not compared to an expected value | **GAP**: add `MT5_EXPECTED_CURRENCY=USD`; mismatch fails closed |
| Company/broker | Not collected by the EA | **GAP**: EA to report `ACCOUNT_COMPANY`; compare with expected `TenTrade` |
| Demo vs live | Not checked | **GAP**: EA to report `ACCOUNT_TRADE_MODE`; require `ACCOUNT_TRADE_MODE_DEMO`; live fails closed |
| Permissions | Not checked | **GAP**: report `ACCOUNT_TRADE_ALLOWED`/`TERMINAL_TRADE_ALLOWED` as informational only; the system never trades |
| Terminal/EA build, protocol | PING returns protocol and build | Require `provider_protocol_version == 1` and `provider_build_version == "2.00"` before any data is trusted |

These gaps are a proposal and are **not implemented**. A configured `MT5_ACCOUNT_ID` is never treated as proof; identity comes from the connected terminal. Any mismatch -> `AUTHENTICATION_FAILED`, no snapshots persisted, no exposure authorised.

### Measurable market-data tests (to run only after approval)
- Bid/ask come from the live broker; `bid > 0`, `ask >= bid`; null is never coerced to zero.
- Symbol identity equals the requested symbol; contract size, digits, volume min/max/step present.
- `time_msc`/`TimeTradeServer()` advance; `TimeCurrent()` may freeze when the market is closed; UTC and server time handled separately; age is measured on the daemon's trusted clock against the 60 s limit (not relaxed).
- Duplicate ticks detected by sequence/timestamp; gaps flagged; stale -> `stale` with real price and timestamp; missing -> `unavailable`; future/invalid -> `invalid`.
- Orders/positions: identifiers, volume and price precision, snapshot timestamps, reconciliation sequence, duplicates/missing records.

## 4. Persistence and recovery acceptance tests (end-to-end NOT RUN)

Existing committed Rust tests cover idempotency, market-data snapshots, restart persistence, sequence handling, and selected mTLS/reconciliation cases. Current local checks add health-state unit tests and a compiled pool-close/reconnect integration test. The database test was not run, and the pool test does not simulate a PostgreSQL server outage. The matrix below remains the target-environment acceptance gate; true DB outage/recovery and an actual transport disconnect/reconnect cycle remain unverified.

| # | Scenario | Expected, measurable outcome |
|---|---|---|
| 1 | Worker restart | New session resumes; sequence strictly increases; no duplicate rows (idempotency keys) |
| 2 | Daemon restart | Worker reconnects within the backoff window; persisted state retained; reconciliation resumes |
| 3 | Terminal restart | EA reconnects; account identity re-verified before any snapshot is accepted |
| 4 | DB interruption | Daemon degrades, no acknowledgement of unpersisted data; recovers when the DB returns; no lost acknowledged rows |
| 5 | Network interruption | Transport DISCONNECTED -> RECOVERING; account/market health reported separately |
| 6 | Certificate expiry | Connection refused; fail closed; clear structured error |
| 7 | Authentication failure | AUTHENTICATION_FAILED; no retries that bypass identity checks |
| 8 | Protocol mismatch (v1 vs v2) | Rejected by preflight and at handshake |
| 9 | Duplicate messages | Idempotent; row count unchanged |
| 10 | Sequence gaps | RECONCILIATION_REQUIRED |
| 11 | Stale market data | Transport stays CONNECTED; market data STALE_MARKET_DATA; sizing rejects new exposure |
| 12 | Recovery from persisted state | Sequence continuity across restarts |

## 5. Security checklist (review items; not yet verified on the target host)
- mTLS: client and server identity binding, server name matching `MT5_BRIDGE_SERVER_NAME`, certificate chain validation.
- Expiry monitoring, rotation and revocation procedure.
- Private-key mode `0600`; keys outside the repo and never in logs.
- Secrets only in `quantos.env` (never in plists, docs or commits); redact logins, passwords, tokens and certificate material in logs.
- Worker/EA socket bound to 127.0.0.1 only (observed 127.0.0.1:8765); confirm no wider exposure.
- Authentication failures fail closed; reconnect backoff does not spin on predictable conditions (market closed).
- Structured logging with redaction.

## 6. Operational states
The internal `Mt5HealthSnapshot` type reports eight separate health indicators: **transport**, **authentication**, **terminal connectivity**, **account identity**, **market-data freshness**, **database persistence**, **reconciliation**, and **recovery**. `HEALTHY` requires every signal healthy; unknown or degraded signals produce overall `DEGRADED`, and any unhealthy signal produces overall `UNHEALTHY`. A stale quote can affect market-data freshness without falsely reporting a transport disconnect.

This is not a complete runtime observability implementation: the type is not yet populated from live components, the daemon does not expose these signals, and there is no independent database-health endpoint. The readiness type does not change order-execution behavior. New exposure remains gated by existing session/account/freshness checks.

## 7. Backup inventory (to create before approval; none created yet)
- `~/Library/LaunchAgents/com.quantos.mt5-worker.plist` and `com.quantos.mt5-bridge.plist`.
- v1 daemon binary `~/rust-trading-engine/event-trading/target/release/mt5-bridge-daemon` (`734f58cf…`).
- v1 checkout state (HEAD `3e12590…` plus its three local modifications: `Cargo.lock`, `adapters/mt5-python/scripts/mt5-macos-launch-wrapper.sh`, `docker-compose.yml`).
- `quantos.env` (permissions preserved; not copied into the repo or logs).
- Installed EA `QuantOSReadOnlyProvider.mq5` (`eed359d5…`) and `.ex5` (`0eed63bf…`), plus the MT5 `config/` folder.
- Backups go to a new, dated directory outside the repo; existing backups and unrelated terminal files are never overwritten. **Requires >= 2 GB free space first.**

## 8. Deployment procedure (NOT EXECUTED; needs separate approval)

Pre-deployment
1. Free disk space (target >= 5 GB) without deleting artifacts of unknown provenance.
2. Operator reports File > Open Data Folder, and confirms login 788251, TenTrade, TenTrade-Server, USD, Demo.
3. Diagnose and fix the v1 `Db(PoolTimedOut)` crash loop, or record that v1 is already unhealthy.
4. Create the section 7 backups and verify their hashes.
5. Re-run the preflight with the explicit path-remapped daemon, manifest SHA-256 and exact byte size, EA source `f43bbf5c…`, and the EX5 compiled for deployment.
6. Verify certificates, permissions, and the isolated staging database URI using an authenticated read-only query and migration-state check. Never reuse v1's failing `DATABASE_URL`.
7. Record the current v1 runtime (processes, launchd state, listening ports).

Deployment
8. Stage the RC checkout; use new launchd labels (e.g. `com.quantos.mt5-*-v2`) and a different port or a stopped v1; never modify the v1 plists.
9. Stop only the v1 jobs that conflict (`launchctl bootout`), with approval; keep their plists.
10. Install the remapped RC daemon and the RC worker; configure mTLS through `quantos.env`.
11. Compile the RC EA in MetaEditor (exit 1 plus `0 errors` log), record the hash, and place the files in `MQL5/Experts/`.
12. Reload the EA on the chart.
13. Verify PING: protocol 1, build "2.00".
14. Verify account identity (section 3).
15. Verify fresh broker quotes (or STALE/market-closed state during closure; never fabricate).
16. Verify account, orders and positions.
17. Verify DB rows and reconciliation.
18. Run controlled restart and failure tests (section 4).

Rollback
R1. Stop only the new v2 launchd jobs (`launchctl bootout gui/$UID/<v2 label>`).
R2. Restore the backed-up EA `.mq5`/`.ex5` and reload the chart (only if changed).
R3. Restore the v1 plists and binary from backup if altered; verify their hashes.
R4. With separate operator approval, `launchctl bootstrap` the v1 jobs.
R5. Verify: one worker, one daemon, the expected listening ports, and the EA reporting build 1.00 on PING.
R6. Do not delete database rows or other terminal files.

## 9. Acceptance matrix

| Test | Expected result | Evidence required | Status |
|---|---|---|---|
| RC artifact provenance | Commit and hashes match the manifest | Git, hashes, manifest | PASS (EX5 excepted) |
| Daemon/worker protocol compatibility | Schema v2 on both sides | Preflight | PASS (static) |
| EA compiler evidence | 0 errors, EX5 generated | MetaEditor log | PASS (not accepted as deployable) |
| EX5 acceptance | Runtime build/protocol check | PING | NOT RUN |
| Runtime PING | Protocol 1, build 2.00 | MQL5 log / worker log | NOT RUN |
| TenTrade account binding | Login, server, company, currency, demo match | Account snapshot | NOT RUN |
| Fresh broker quotes | Real bid/ask within 60 s | Persisted tick | NOT RUN |
| Quote freshness enforcement | Stale rejected, never fabricated | Test + runtime | PASS (unit); runtime NOT RUN |
| Account snapshot | Persisted, verified | DB row | NOT RUN |
| Orders and positions snapshots | Retrievable, IDs/precision valid | DB rows | NOT RUN |
| Persistent database records | Original timestamps kept | DB query | PASS (9 integration tests, isolated DB); runtime NOT RUN |
| Idempotency | No duplicate rows | DB | PASS (integration tests); runtime NOT RUN |
| Reconciliation | Sequence continuity | Logs/DB | NOT RUN |
| Worker restart | See section 4 | Evidence | NOT RUN |
| Daemon restart | See section 4 | Evidence | NOT RUN |
| Terminal restart | See section 4 | Evidence | NOT RUN |
| Database interruption | See section 4 | Evidence | NOT RUN |
| Network interruption | See section 4 | Evidence | NOT RUN |
| Certificate validation | Valid mTLS accepted, invalid rejected | Tests/logs | PASS (7 mTLS tests); host NOT RUN |
| Certificate rotation/revocation | Procedure tested | Evidence | NOT RUN |
| Recovery after failure | Returns to HEALTHY | Evidence | NOT RUN |
| Windows MT5 conformance | Windows host evidence | Evidence | NOT RUN |

## 10. Unresolved prerequisites
1. Operator-confirmed full MT5 data-folder path.
2. Installed `.mq5` is the v1 source (`eed359d5…`), not the RC source.
3. v1 daemon crash loop (`Db(PoolTimedOut)`); cause unknown.
4. Disk below the 5 GiB preparation target (4.0 GiB free at the latest observation, 2026-11-01 21:34, after targeted Rust checks); do not run release builds or create backups until space is safely above the target. Do not clean files of unknown provenance.
5. Account-binding gaps: company, currency, demo mode (section 3).
6. EX5 not accepted; the artifact to deploy must be compiled and recorded at deployment time.
7. The v1 database URI targets `127.0.0.1:15433/trading`, which refused the read-only connection; the reachable port 5433 rejected the configured role. Intended database credentials/server are unresolved. V2 staging DB isolation is not yet configured or verified.
8. Operator environment file and certificates not yet reviewed on the host.

## 11. Operator approvals required (each separate)
A. Disk clean-up plan. B. Creating backups. C. Fixing/restarting v1 services. D. Compiling and installing the EA. E. Starting v2 services and switching from v1. F. Reloading the chart or restarting MT5. G. Failure-injection tests. H. Any merge to `main` (not requested).

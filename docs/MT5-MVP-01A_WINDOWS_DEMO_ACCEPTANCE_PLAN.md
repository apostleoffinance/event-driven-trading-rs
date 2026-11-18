# MT5-MVP-01A Windows DEMO Acceptance Plan

**Purpose:** acceptance procedure for one managed Windows DEMO terminal, the official MetaTrader5 Python provider, one authenticated Rust bridge principal, and the read-only persistence path. This is a plan only; no Windows terminal, package, service, or broker account was accessed.

## Entry gates

Before running the matrix:

1. Approve the Windows host owner, supported Windows build, service identity, terminal installation path/version, and patch/reboot policy.
2. Approve credential custody and delivery. The worker now rejects plaintext `MT5_PASSWORD` and supports a CurrentUser DPAPI-protected secret file provisioned by `scripts/protect-mt5-password.ps1`. This is a local encrypted cache, not a managed secret manager; the selected host/provider, service identity, ACL review, rotation and revocation owner still require approval. The real-host credential gate remains **BLOCKED** until those controls are approved.
3. Resolve the DEMO-to-Quant account environment mapping (`accounts.environment` currently allows PAPER/SHADOW/LIVE, while the broker registry allows DEMO/LIVE/UNKNOWN).
4. Provision an organization entitlement and external broker account binding. Have a separate authorized operator verify the broker login/server/environment evidence, create/approve a `VERIFIED` binding revision, and record evidence and approval references. The daemon now has a per-frame database-backed admission guard; it fails closed on binding, entitlement, evidence, verifier-authority, worker-assignment or READY-session revocation. This is repository implementation, not yet real-host acceptance evidence.
5. Pin build/source hashes for worker and daemon, Python and official `MetaTrader5` package versions, Windows terminal build, configuration revision, client certificate fingerprint, and protocol schema version. Keep certificate/key secrets outside the evidence bundle.
6. Confirm isolated target database and migrations through the approved QOS-020 lane. No MT5 staging or production database is permitted for this acceptance plan.
7. Confirm order submission/modification/cancellation/close are unavailable and remain disabled. Use DEMO account only.

## Registry admission provisioning

The daemon requires these registry references in its protected service configuration:

- `MT5_ORGANIZATION_ID`: owning organization.
- `MT5_ACCOUNT_ID`: QuantOS account ID (never the numeric MT5 login).
- `MT5_EXTERNAL_ACCOUNT_ID`: opaque external broker-account registry ID.
- `MT5_BINDING_ID`: verified QuantOS-to-broker binding ID.
- `MT5_BROKER_ID` and `MT5_BROKER_SERVER_ID`: canonical registry IDs, not display labels.

The worker's mTLS leaf-certificate SHA-256 fingerprint is the immutable `worker_ref`. Provision the same lowercase fingerprint as a non-human service principal ID in `identity_users`, give that service principal an active membership in the owning organization, and use the same value in `broker_worker_assignments.worker_ref`. The assignment must be unique and in scope for the organization, QuantOS account, binding, external account, broker and server. Its session must be READY and unexpired. The service principal is the authenticated actor for the read-only admission check because the certificate authenticates that identity.

The daemon re-evaluates the persisted admission snapshot before every incoming frame is sequence-checkpointed or persisted. Database errors, missing facts, expired/revoked evidence, revoked verifier authority, suspended entitlement, assignment/session mismatch, or stale scope deny ingestion. The permit's QuantOS account must match the account configured in the authenticated worker principal.

## Execution evidence rules

Capture timestamped host/version metadata, redacted command output, test result, worker/daemon build hashes, schema and binding revisions, sequence numbers, persistence row identities/counts, and health/freshness states. Redact broker login except where a restricted evidence record requires it; do not put password, private key, full database URL, access token, or unredacted exception payloads in the report. Use synthetic or masked identifiers in broadly shared evidence.

## Acceptance matrix

| # | Test | Preconditions | Action | Expected result | Evidence | Pass criterion |
|---:|---|---|---|---|---|---|
| 1 | Windows host identity | Managed host enrolled; maintenance window | Record Windows edition/build, architecture, hostname alias, service identity and patch level | Approved supported host; identifiers consistent | Signed host inventory excerpt | All approved values match; no credentials in output |
| 2 | Terminal installation identity | DEMO terminal installed but not yet accepted | Record terminal executable path, publisher, version/build, install/data-folder identity | Exact approved terminal; no duplicate/ambiguous installation | File metadata and terminal About/version capture | Path/build match the host registration |
| 3 | Python/package identity | Approved venv and pinned package plan | Record Python architecture/version and `MetaTrader5` package version from isolated venv | Windows-compatible Python and reviewed provider version | Package metadata and venv path/hash manifest | Versions match the approved lock/manifest |
| 4 | Terminal initialization | Secret delivery and terminal config approved | Start worker/provider and initialize only the intended terminal | Initialize succeeds; worker logs only safe stage/status | Redacted worker log and process identity | No credential printed; expected terminal process is selected |
| 5 | DEMO login/server/environment | Approved binding evidence and operator present | Read account info; independently compare observed login/server against approved evidence | Exact DEMO login and server; account environment is DEMO and mapping decision is recorded | Restricted broker evidence reference, masked login, binding revision | All match; unknown/mismatch fails closed |
| 6 | Account snapshot | Provider authenticated and binding eligible | Collect account info | Currency, balance, equity, margin and free margin represented as exact decimal strings | Redacted schema-validated sample; persistence readback | Values correspond to provider; no float rounding; timestamps are collection-time UTC |
| 7 | Symbol catalogue/specification | Approved exact symbol allowlist | Collect `symbol_info` for each symbol without silently expanding scope | Exact symbol names and available spec values; absent optional fields remain absent/null per approved schema | Contract validation output and symbol list hash | Required fields validate; no guessed currencies/specs |
| 8 | Tick collection/freshness | Market open or known tick conditions; clock synchronized | Collect quotes across repeated polling intervals | Per-symbol fresh/stale/unavailable/invalid status; source tick time distinct from collection time | Sequence/timestamp/status samples | `fresh` meets configured age/skew; stale/unavailable never represented as fresh |
| 9 | Position collection | DEMO account with independently known open-position fixture, or empty account evidence | Fetch all open positions and compare tickets/state | Complete current open set; no omissions; known v2 fields exact | Masked provider comparison and persisted reconciliation snapshot | Every expected item matches; limitations for SL/TP/swap/current price documented |
| 10 | Open-order collection | DEMO account with known pending order fixture, or no pending orders | Fetch all open orders and compare tickets/state | Complete current open-order set | Masked provider comparison and persisted snapshot | Every expected item matches; initial/remaining volume distinctions recorded |
| 11 | TLS 1.3 and mTLS | Issued server/client certs, trusted CA, allowlisted client fingerprint | Connect the worker to daemon; inspect negotiated protocol and principal mapping | TLS 1.3 only; server name/chain and client cert valid; exact leaf fingerprint, terminal/server/account principal accepted | Redacted TLS handshake/version and principal ID | Correct certificate succeeds and binds to one expected stream |
| 12 | Invalid certificate rejection | Isolated non-production test identity | Connect with untrusted/expired/revoked or unallowlisted cert | Handshake/principal rejected before frames persist | Redacted daemon rejection category and unchanged test DB identity/checkpoint | No frame or stream row accepted |
| 13 | Account binding mismatch | Test with unapproved account/server or terminal ID | Attempt connection/frame using mismatch | Rejected; no operational eligibility/persistence; stream degraded only as designed | Rejection log and before/after row counts | No unverified account can create accepted reconciliation |
| 14 | Malformed protocol rejection | Isolated test client only | Send invalid JSON, unknown schema, unknown field, oversized/incomplete NDJSON, invalid decimal/timestamp | Connection/frame rejected and decoder poisoned/closed | Test output and rejection category | No malformed value is persisted or applied |
| 15 | Sequence ordering/duplicate | Isolated stream with recorded checkpoint | Send increasing, duplicate, regressing and gap sequences | Monotonic accepted; duplicate/regression rejected; gap forces reconciliation | Checkpoint snapshots and result codes | No duplicate/regressing observation advances ready state |
| 16 | Persistence/reconciliation | Eligible binding; isolated migrated DB and restricted role | Ingest account, market, full order/position snapshots; verify row state | Market latest-row and reconciliation append/checkpoint agree on account, terminal, server, sequence and capture time | Read-only DB evidence from approved guard lane | Complete valid snapshot persisted; readiness only after required reconciliation and fresh data |
| 17 | Worker-only restart | Passing steady-state baseline; daemon remains healthy | Restart worker once using managed supervisor | TLS reconnect, sequence continues, fresh complete reconciliation precedes readiness | Supervisor events, old/new process times, sequence and readiness | No sequence reset; no ready state before persisted reconciliation |
| 18 | Terminal disconnect/reconnect | Passing baseline; DEMO only | Disconnect/reconnect terminal or test controlled network loss | Worker reports safe failure, backs off, reconnects, verifies login/server again | Redacted logs and timestamped state transitions | No stale connection is reported as fresh; recovery requires current snapshots |
| 19 | Stale snapshot detection | Stable clock; isolated fixture | Stop tick updates while connection/heartbeat remains alive beyond freshness threshold | Account transport may remain connected, but market status becomes stale and risk consumers reject it | Tick source time, collection time, freshness and consumer decision | Connected status never masks stale market data |
| 20 | Log/credential redaction | Secret scanner and review checklist available | Exercise expected success/failure paths; inspect approved logs only | Password, private key, connection strings and tokens absent | Redacted log review and automated scan result | Zero secret matches; diagnostic categories are sufficient for support |
| 21 | Execution remains disabled | Static source/config review and DEMO account | Inspect worker capabilities and daemon interface; attempt no trading call | Capabilities for market/limit/stop/cancel remain false; no dispatch invoked | Source/version hash, config evidence, supervisor authorization | No order submission, modification, cancellation, or close path is enabled or called |

## Exit criteria

All 21 tests must pass on the one managed Windows DEMO host and isolated database. Fail closed on identity, freshness, protocol, certificate, or persistence disagreement. Preserve test evidence and hash the release artifacts. Any schema additions must first pass an approved versioning decision; do not silently extend schema v2. No transition to execution is implied by passing this plan.


# QOS-021A-E3A-R3A — Broker Verification Authority and Evidence Architecture

**Final status: COMPLETE — investigation and proposed architecture documented; decisions remain PROPOSED pending explicit review.**  
**Repository:** `/Users/olaoluwatunmise/event-driven-trading-rs`  
**Branch / HEAD:** `main` / `f6446dd33dc74a86ede9064f35aef2ec082a1c59`  
**Scope:** Read-only architecture investigation. Only this report was created.

## 1. Repository and worktree preflight

### Identity

Observed outputs:

```text
$ pwd
/Users/olaoluwatunmise/event-driven-trading-rs
$ git rev-parse --show-toplevel
/Users/olaoluwatunmise/event-driven-trading-rs
$ git branch --show-current
main
$ git rev-parse HEAD
f6446dd33dc74a86ede9064f35aef2ec082a1c59
$ git remote -v
origin  https://github.com/apostleoffinance/event-driven-trading-rs.git (fetch)
origin  https://github.com/apostleoffinance/event-driven-trading-rs.git (push)
```

No `AGENTS.md` was found in the repository. No fetch, build, test, database, Docker, provider or service action was performed.

### Initial status

The complete `git status --short --untracked-files=all` at preflight was:

```text
 M .github/workflows/ci.yml
MM Cargo.lock
 M crates/account-engine/src/simulated.rs
 M crates/domain/src/account.rs
 M crates/domain/src/ids.rs
 M crates/domain/src/lib.rs
 M crates/domain/tests/phase1_architecture.rs
 M crates/event-trading/src/execution/fill.rs
 M crates/execution-engine/src/oms.rs
 M crates/intent-bridge/src/lib.rs
 M crates/intent-bridge/src/mt5_runtime.rs
 M crates/intent-bridge/src/mt5_transport.rs
 M crates/intent-bridge/src/mt5_v1.rs
 M crates/intent-bridge/tests/mt5_transport_mtls.rs
 M crates/journal-api/Cargo.toml
 M crates/journal-api/src/dashboard.rs
 M crates/journal-api/src/main.rs
 M crates/persistence/src/lib.rs
 M crates/persistence/src/pool.rs
 M crates/persistence/src/repos/accounts.rs
 M crates/persistence/src/repos/mod.rs
 M crates/persistence/tests/survive_restart.rs
 M crates/reconciliation/tests/venue_vs_internal.rs
 M crates/trading-runtime/src/multi_account.rs
 M crates/trading-runtime/src/pipeline.rs
 M crates/trading-runtime/src/runtime.rs
 M crates/venue-connectors/src/adapter.rs
 M crates/venue-connectors/src/bridge.rs
 M crates/venue-connectors/src/lib.rs
 M crates/venue-connectors/src/prop.rs
 M crates/venue-connectors/src/simulated.rs
 D crates/venue-connectors/tests/ambiguous_unknown.rs
 D crates/venue-connectors/tests/cancel_confirmation.rs
 M crates/venue-connectors/tests/e2e_pipeline.rs
 D crates/venue-connectors/tests/failure_injection.rs
 M crates/venue-connectors/tests/prop_pipeline.rs
M  docker-compose.yml
?? crates/domain/src/broker_registry.rs
?? crates/domain/src/execution_authorization.rs
?? crates/domain/src/organization.rs
?? crates/journal-api/src/broker_registry.rs
?? crates/journal-api/src/organizations_api.rs
?? crates/journal-api/tests/fixtures/r2_test_private.pem
?? crates/journal-api/tests/fixtures/r2_test_public.pem
?? crates/persistence/src/repos/broker_registry.rs
?? crates/persistence/src/repos/organizations.rs
?? crates/persistence/tests/qos021a_e2_migrations.rs
?? crates/persistence/tests/qos021a_e3a_r2_concurrency.rs
?? crates/persistence/tests/qos021a_e3a_r2_tenant.rs
?? crates/persistence/tests/qos021a_e3a_r3_broker_registry.rs
?? crates/venue-connectors/src/tests/ambiguous_unknown.rs
?? crates/venue-connectors/src/tests/cancel_confirmation.rs
?? crates/venue-connectors/src/tests/failure_injection.rs
?? docs/QOS-020_TEST_INFRASTRUCTURE.md
?? docs/QOS-021A-E1-R1_GENERIC_VENUE_EXECUTION_BOUNDARY.md
?? docs/QOS-021A-E1_EXECUTION_AUTHORIZATION_FOUNDATION.md
?? docs/QOS-021A-E2_ACCOUNT_ENVIRONMENT_AND_MIGRATION_FOUNDATION.md
?? docs/QOS-021A-E3A-R1_ARCHITECTURE_DECISION_RESOLUTION.md
?? docs/QOS-021A-E3A-R2-F_VALIDATION_AND_COMPLETION.md
?? docs/QOS-021A-E3A-R2_ORGANIZATION_TENANT_FOUNDATION.md
?? docs/QOS-021A-E3A-R3_BROKER_SERVER_ACCOUNT_REGISTRY.md
?? docs/QOS-021A-E3A_TRUSTED_IDENTITY_AND_IMMUTABLE_RISK_ARCHITECTURE.md
?? docs/QOS-021A-S_ARCHITECTURE_AND_INFRASTRUCTURE_RECOVERY.md
?? migrations/20261111000001_journal_trading_account_fk_repair.sql
?? migrations/20261111000002_account_environment_provenance.sql
?? migrations/20261111000003_execution_authorizations_and_claims.sql
?? migrations/20261112000001_organization_tenant_foundation.sql
?? migrations/20261113000001_broker_account_registry.sql
?? scripts/qos020/docker-compose.qos020.yml
?? scripts/qos020/init/10-qos020-init.sql
?? scripts/qos020/provision.py
?? scripts/qos020/qos020_guard.py
?? scripts/qos020/test_guard.py
?? scripts/traderos-migration/migrate.py.before-http-debug
?? scripts/traderos-migration/migrate.py.debug-backup
```

`git status --short --ignored` additionally reported existing ignored content: `.conda/`, `.pytest_cache/`, `.venv/`, `.vscode/`, MT5 Python adapter caches/egg-info, frontend `.next/`, `.open-next/`, `.wrangler/`, `node_modules/`, generated frontend metadata, `migration-report.json`, `migration-state.json`, `migration/`, Python caches, strategy egg-info, and `target/`. These were not inspected or changed beyond status enumeration.

### Final status and preservation

The final status is the preflight list above plus this one new path:

```text
?? docs/QOS-021A-E3A-R3A_BROKER_VERIFICATION_AUTHORITY_AND_EVIDENCE_ARCHITECTURE.md
```

The pre-existing staged changes remain staged (`MM Cargo.lock`, `M  docker-compose.yml`); no other file was staged. All pre-existing unstaged, deleted, untracked and ignored artifacts were preserved. No source, migration, database, container, network, volume, secret, certificate, provider or running service was changed.

### Protected hashes observed at preflight

| Path | SHA-256 |
|---|---|
| Worktree `Cargo.lock` | `549d67b306e609fc6360a6f8d20281dcd33c17949904d510e2807d6eedd891d5` |
| Staged `git show :Cargo.lock` | `0114a3432961bae1989c479073e12ea84df6507c27debda84e832d950e35e0a0` |
| `docker-compose.yml` | `0a761eb4c42c5d9b72cd9f642ea7ef8411fc70106205b2acbcb2e90ef4789866` |
| `.github/workflows/ci.yml` | `7eb6c98d3518e05e97497f2e8ec1163d6ca90488556af870098f3cd2cbcbd1f6` |
| R1 decisions `docs/QOS-021A-E3A-R1_ARCHITECTURE_DECISION_RESOLUTION.md` | `3a8d8792788d37332bbb0b090d13cf57253c6dcd9fc5381d4a6144d9e6e4c3f6` |
| R2 foundation report `docs/QOS-021A-E3A-R2_ORGANIZATION_TENANT_FOUNDATION.md` | `1bd31dd6bdfac6c46a5205c549a2d0daf66b040b6483af6e1d5ca6ea138df890` |
| R2-F report `docs/QOS-021A-E3A-R2-F_VALIDATION_AND_COMPLETION.md` | `736f0f530e2a042dfb6e1c73fe014fa9c39628ac8bbf492449a6379664be4518` |
| R3 report `docs/QOS-021A-E3A-R3_BROKER_SERVER_ACCOUNT_REGISTRY.md` | `9dce6962b51b045fcbd0f41f7b85345ab6986de28443f9becb08ad92bfb7c7a1` |
| R3 migration | `ee76686a784a3b62d0ae6743a78549ee4c4bbe9a548820115efd59f03c24b2be` |
| MT5 runtime | `6799d6d47f5b497366aeb90ba945658e9b741138459e0ed42571120f0eff5ccc` |
| MT5 v1 protocol | `514c6bd0b7da5846e558dfc6e221e1e5192472e4b359f7200d493f67dbeddcb2` |
| MT5 transport | `a3343826b93724c0e0fc1de6722e2fc0b9b5d231b29557c8562e4b369d0a2ea0` |

The distinct worktree/staged `Cargo.lock` hashes confirm an existing staged change plus subsequent worktree modification; both layers were preserved. The hashes above were rechecked after creating the report and remained unchanged. Only the new report hash differs by design and is recorded in the final response context.

## 2. Current R3 architecture findings

**VERIFIED from the local R3 implementation and migration:**

- R3 correctly separates brokers, broker servers, external broker accounts, Quant account bindings, evidence and verification history from the existing `venues` abstraction.
- A broker binding references the R2 `(organization_id, account_id)` entitlement and registration rechecks active OWNER membership and active entitlement transactionally.
- The R3 API permits organization OWNERs to initiate a pending registration and suspend/revoke a binding. It does not provide provider-evidence ingestion, verifier review, approval, catalogue mutation or explicit reassignment.
- R3 key configuration is process environment configuration: `EDGENEST_BROKER_ID_HMAC_KEYSET` and `EDGENEST_BROKER_ID_HMAC_ACTIVE_KEY_VERSION`, with single-key compatibility variables. The API computes domain-separated HMAC-SHA256 aliases for every configured key version and writes aliases transactionally. Raw identifiers are not retained in recoverable form. There is no persisted key lifecycle or re-key job protocol.
- Current migration uses `identity_users` for initiating and audit actors but has no platform authority table. `approval_authority` and `approval_reference` are strings, not an authorization system. The database verified-state guard requires authenticated evidence and approval reference fields, but it does not independently validate a platform permission.
- The current tenant identity implementation uses the verified JWT subject and revalidates R2 membership from the database. Organization roles are OWNER/ADMIN/TRADER/RISK_MANAGER/VIEWER. These are tenant roles and must not grant platform verification privileges.
- Identity API issues JWTs with a subject; no platform-verifier claim/role or separate platform operations identity subsystem was found.
- MT5 transport documentation specifies TLS 1.3 mTLS, authenticated worker principal mapping, certificate fingerprint allow-listing, configured terminal/server/account binding, and that a heartbeat identity is an assertion rather than proof. The current Python adapter is read-only and has no R3 evidence-ingestion contract. Official Windows MetaTrader5 Python API remains the production direction subject to demo conformance.

**PARTIALLY VERIFIED:** repository documents and local source describe the provider path and certificate model; no live Windows host, terminal, provider account, certificate issuance system or real provider evidence was inspected in this read-only phase. No external source was used to claim that MetaTrader reports legal account ownership.

The accepted R1 architecture already requires separate provider evidence and human approval, explicitly warns that terminal connectivity does not prove legal ownership, and defers implementation until separately authorized. This report proposes decisions to close that gap; it does not mark any proposal accepted.

## 3. Proposed ADR-1 — Platform Verification Authority

**Status: PROPOSED; requires explicit approval.**

### Problem and context

A trusted human must review evidence and approve or reject an organization-to-external-account association. R2 tenant roles cannot establish platform authority. Reusing `identity_users` as the authenticated human subject is reasonable, but `identity_users` alone is not an authorization grant. Current R3 free-form `approval_authority` metadata must never be treated as proof.

### Options

| Option | Advantages | Risks / costs |
|---|---|---|
| A. Add platform roles/permissions to existing identity system | Reuses verified human identity, session controls and `sub`; least initial system duplication | Must strictly separate platform grants from tenant membership; broad identity service impact; role claims can become stale unless every action rechecks authority |
| B. Separate platform operations identity and authorization subsystem | Strong administrative plane isolation and independent policy lifecycle | New identity, login, recovery, MFA, audit, support and availability burden; duplicate account lifecycle; more integration and operational cost |
| C. Controlled approval service with independently authenticated human action | Narrow service API and clear approval ceremony; can isolate review from public API | Still needs authoritative human identity/permission source; a service account alone does not prove which human approved; manual processes can become opaque |

### Recommended decision

Recommend **A**, implemented as a separate platform authorization domain attached to the existing authenticated human identity, not as an organization role and not as a broad `ADMIN` boolean. Add named, narrowly scoped grants such as `broker_verification.review`, `broker_verification.approve`, `broker_verification.suspend`, `broker_registry.manage`, and `broker_reassignment.approve`. A platform grant is explicit, time-bounded where practical, revisioned, independently audited and checked server-side against current database state on each sensitive operation. Do not place enduring authority solely in a long-lived JWT claim.

Represent authority grant state separately from `organization_memberships`; no inheritance from OWNER, ADMIN, staff email domain or account creator. Grant/revoke requires a distinct authorized grantor and expected grant revision. Store immutable grant/revoke events and the authority revision and permission set used on each decision. Identity remains `identity_users.id`; FK deletion is restricted so the actor remains attributable after suspension or membership removal. Identity disable blocks new approvals, but historical decisions remain attributable.

Require two-person approval for high-risk cross-organization reassignment, disputed ownership, overrides of contradictory evidence, and authority/key compromise recovery. Normal evidence approval should require one independent verifier plus separation from initiator; risk-based second approval can be configured for flagged cases. Neither approver may be the organization registration actor or an actor who submitted the evidence being reviewed.

### Consequences, migration and security

Requires additive platform authority/grant tables, revisions, revocation state, append-only events, permission endpoints, and a narrow platform-verifier API/UI. Existing R3 rows remain pending; no authority is retroactively fabricated. Existing `approved_by` and free-text authority fields cannot be interpreted as a valid platform decision without independently reconstructable evidence. Security depends on MFA/strong reauthentication for privileged actions, current-permission reads, immutable actor attribution, session revocation and audited emergency access. A stolen verifier account can approve fraud; least privilege, short sessions, second approval and alerting reduce but do not eliminate this risk.

### Acceptance criteria

- Tenant OWNER/ADMIN roles cannot obtain verifier permissions implicitly.
- Grant, revoke, suspend and approval operations require authenticated subject, exact permission, expected revision and reason.
- A revoked or suspended grant is rejected immediately even when an earlier JWT is still valid.
- Actor ID and authority revision at decision time remain in immutable history.
- Initiator/evidence-submitter self-approval is rejected at service and database transaction boundary.
- Concurrency tests allow one decision per expected verification revision and preserve the losing attempt as an auditable rejection where appropriate.

## 4. Proposed ADR-2 — Authenticated Provider Evidence

**Status: PROPOSED; requires explicit approval.**

### Problem and context

Three facts must remain separate:

1. **Provider connection authenticated:** a registered EdgeNest workload presented a valid credential.
2. **Provider observation:** that workload's configured MT5 Python integration reported broker/server/account metadata at a particular time.
3. **Organization association approved:** a platform verifier accepted sufficient evidence that the organization may associate the account.

mTLS proves which configured workload connected; it does not prove a broker signed the reported data or that the organization legally owns the account. An MT5 terminal's successful login indicates technical access/control at observation time, not legal or beneficial ownership.

### Recommendation and API contract

Use the official Windows MT5 Python provider as the initial evidence source, subject to a demo conformance gate. Each managed gateway workload receives a unique, non-exportable or ACL-protected workload key/certificate and immutable workload identity bound server-side to approved terminal ID, broker/server allow-list, tenant registration challenge, and permitted account enrollment. Reuse TLS 1.3 mTLS concepts from the bridge; do not treat existing bridge certificate identity as an R3 evidence credential until its purpose, trust root, principal and scope are explicitly separated and reviewed.

Proposed provider exchange:

1. OWNER initiates registration; API returns opaque registration ID and short-lived, single-use nonce/challenge. It does not reveal whether an external identity exists elsewhere.
2. An authorized workload obtains the challenge through its authenticated channel and queries official provider account metadata. The workload submits a canonical evidence envelope over mTLS to a distinct provider-facing endpoint.
3. Envelope includes protocol/schema version, evidence ID, registration challenge ID, nonce, monotonic workload sequence, workload ID, certificate key ID, provider library and terminal build versions, broker/server/account observation, account category/currency if available, observation time, server receipt time, and digest of canonical bytes. Sensitive identifier is accepted only on this protected channel, normalized and HMACed in memory, then discarded/zeroized.
4. Server validates certificate chain and revocation state, principal allow-list, workload/account mapping, nonce consumption, sequence/replay key, schema, clock skew, freshness window, broker/server catalogue match and identifier HMAC match. It stores only minimal normalized metadata, attestation/evidence digest and redacted identity references. Evidence row and registration transition commit atomically.
5. Provider evidence becomes `AUTHENTICATED` only after transport and envelope checks; it advances the registration to `PENDING_APPROVAL`, never directly to VERIFIED.

TLS client authentication plus a server nonce and unique `(workload_id, sequence)` / `evidence_id` idempotency constraints prevent simple replay. The nonce must be bound to registration/account candidate, expire quickly, be one-use, and be cryptographically covered by a workload signature/MAC or authenticated channel. A signed envelope with a hardware/OS protected key gives stronger evidence integrity across queues than mTLS alone; it remains evidence from the workload, not from the broker. Validate `observed_at` and receive time, reject excessive skew/future dates, and measure freshness from server receipt too. Keep durable sequence and challenge consumption transactional.

### Ownership evidence

Use two assurance levels and label them precisely:

- **Technical account-control evidence:** fresh authenticated provider metadata from the mapped terminal/session, matching broker/server/account identity. Proves that the managed provider session could observe the account at that time.
- **Association/ownership evidence:** reviewable evidence that the organization is entitled to connect the account. Prefer broker-issued account confirmation or broker portal/API proof with account identity and organization relationship. Where unavailable, require a documented organization attestation plus an independent verifier's review against approved policy; mark assurance level and limitations. Do not call this legal beneficial ownership unless legal-grade evidence and counsel-approved policy support that claim.

Do not collect broker passwords, investor passwords, full portal credentials or unnecessary personal data. If documents are required, store them in a separate encrypted evidence vault with access logs, narrow reviewer access, malware scanning, retention/expiry, deletion/legal-hold rules and opaque references; PostgreSQL stores only digest, evidence class, reviewer decision and vault reference. Prefer redacted attestations over identity documents.

### Alternatives and consequences

- Provider mTLS only: authenticates workload, but not freshness or broker-signed fact; insufficient alone.
- Signed provider envelope plus mTLS and challenge: recommended defense-in-depth; requires workload key issuance/rotation, sequence persistence and canonical serialization.
- Direct broker API attestation: strongest when available but not assumed across brokers and requires broker-specific OAuth/API contracts and secret management.
- EA heartbeat: rejected as ownership evidence; an authenticated bridge heartbeat still reports terminal assertions.

Requires a provider workload registry, certificate lifecycle, challenge/replay tables, envelope schema, evidence service, redacted audit and demo conformance tests. Compromised workload credentials can produce false observations for its allow-listed sessions; rapid revocation, narrow account scope, re-observation and independent human evidence remain required.

## 5. Proposed ADR-3 — Registration and Verification Lifecycle

**Status: PROPOSED; requires explicit approval.**

### Lifecycle

Use separate external-account verification status and organization-binding status; do not compress provider identity and tenant association into one field. Proposed flow:

```text
Binding: PENDING_EVIDENCE -> PENDING_APPROVAL -> VERIFIED
                       \\-> REJECTED / EXPIRED
Verified binding -> SUSPENDED -> REVERIFICATION_REQUIRED -> PENDING_APPROVAL
Any non-terminal binding -> REVOKED (terminal historical record)
External account identity can separately be ACTIVE / SUSPENDED / REVOKED.
```

`EVIDENCE_RECEIVED` is an evidence event/status, not necessarily a binding state; this keeps multiple evidence submissions and rejection history explicit. Add `EXPIRED` and `REVERIFICATION_REQUIRED` as distinct transitions rather than overwriting a prior verification row.

### Transition authority

| Transition | Actor and preconditions |
|---|---|
| Create -> PENDING_EVIDENCE | Active organization OWNER, current entitlement, selected trusted catalogue entry, reason; insert account/binding atomically |
| PENDING_EVIDENCE -> PENDING_APPROVAL | Authenticated provider workload submits fresh matching evidence; nonce/sequence/idempotency valid |
| PENDING_EVIDENCE -> REJECTED / EXPIRED | Platform verifier or expiry service; reason and event required |
| PENDING_APPROVAL -> VERIFIED | Platform verifier with current permission; evidence and association-proof policy satisfied; independent from initiator/evidence submitter; expected verification and binding revisions match |
| PENDING_APPROVAL -> REJECTED | Platform verifier; reason, evidence references and revision recorded |
| VERIFIED -> SUSPENDED | Platform verifier, authorized organization OWNER for their own binding, security incident automation, or system invalidation, with actor kind and reason distinguished |
| SUSPENDED -> REVERIFICATION_REQUIRED | Security/provider event or verifier decision; preserve prior verification and reason |
| REVERIFICATION_REQUIRED -> PENDING_APPROVAL | New fresh evidence received; previous evidence can support context but not automatically satisfy freshness |
| Any nonterminal -> REVOKED | Authorized organization OWNER for own binding or platform verifier; immutable history and explicit reason; release reservation only under reassignment protocol |
| REVOKED -> anything | Prohibited; create a new binding/workflow |

Use monotonically increasing binding and verification revisions. Each command carries expected revisions; one transaction locks external-account, binding, entitlement, relevant authority grant and latest verification row in a documented lock order. It revalidates current permissions and membership and appends the decision/event before commit. A second concurrent command against the same revision fails with conflict and makes no partial state change. Background expiry and revocation jobs use the same command path and idempotency keys.

Evidence freshness should be policy-defined per provider and evidence class, with an explicit expiry timestamp and max observation age. Revalidation triggers include changed broker/server/account assertion, changed workload identity or certificate compromise, evidence expiration, organization entitlement revocation, disputed association, verifier/authority compromise, HMAC compromise, and any reassignment. Existing VERIFIED status is not an eternal fact.

### Consequences

Requires additive state columns/tables, transition functions/repository commands, evidence challenge and expiry jobs, outbox notifications, idempotency keys and concurrency tests. No partial success: if event, evidence link, revision update or notification-outbox insert fails, the transaction rolls back. External notification delivery uses an outbox after commit. Rejection and expiry retain records. A verified association is still not execution authorization.

## 6. Proposed ADR-4 — Explicit Broker Account Reassignment

**Status: PROPOSED; requires explicit approval.**

### Options

- **A. Revoke then register again:** minimal workflow, but a gap and race exist between release and new reservation; evidence and disputes can be mishandled.
- **B. Bilateral organization transfer transaction:** both organizations authorize release/receipt; fair and auditable, but old owner may be unavailable or malicious and workflow is complex.
- **C. Platform-mediated reassignment:** one durable transfer case coordinates release, receiving entitlement, evidence, review and atomic binding replacement; most controlled, still requires due process for disputes.

### Recommendation

Recommend **C**, with bilateral consent as the normal path and a documented verifier escalation path only for loss of access, organization closure, court/broker evidence, or unresolved dispute. Reassignment must never be a PATCH that changes `organization_id` or `quant_account_id` on the existing binding.

1. Source OWNER creates a release request with expected binding revision, source organization, reason and target reference. Target OWNER separately accepts, identifies a currently entitled Quant account and supplies reason. Both identities and active memberships are revalidated at each step.
2. System creates a transfer case with unique idempotency key and reserves the external account against competing new claims. Existing binding remains historical and access is suspended/frozen for sensitive use while under dispute.
3. Platform reviewer checks fresh provider account-control evidence and organization association evidence. Evidence from old binding may be referenced for context, but it never auto-verifies the new organization. Require fresh evidence and two independent platform approvers for cross-tenant transfer or any override.
4. Final transaction locks transfer case, external identity, source/target entitlements, old binding and authority revisions; checks expected revisions and both consent records; marks old binding REVOKED/TRANSFERRED, creates a new `PENDING_APPROVAL` binding and links history. A separate verifier decision is required before new binding becomes VERIFIED.
5. Commit event history and notification outbox atomically. Retry uses idempotency key. Before final commit, cancellation returns the case to its previous safe reservation state; after commit, reversal is a new audited transfer, never record mutation.

If source approval is unavailable, the case stays blocked pending independent platform/legal review; no operator can silently take ownership. Notifications go to both organizations using verified contact channels without exposing identifiers. Add appeal/dispute state, evidence access logging and retention policy. A race with revocation or entitlement suspension serializes on locked rows; whichever valid transition commits first determines the next expected revision, and stale operations fail.

### Consequences and unresolved policy

Requires transfer cases, source-release and target-acceptance events, target-entitlement FK, dispute/appeal policy, reservation constraints covering cases, two-person approval, outbox and database concurrency tests. User approval is needed on who may adjudicate source-consent exceptions, how long a transfer freeze lasts, and what evidence justifies overriding unavailable/contested consent.

## 7. Proposed ADR-5 — HMAC Key Rotation and Retirement

**Status: PROPOSED; requires explicit approval.**

### Constraint

R3 stores only HMAC lookup tokens and deliberately does not persist recoverable account identifiers. Given `HMAC(old_key, identifier)`, the system cannot compute `HMAC(new_key, identifier)`. Existing aliases cannot be algebraically transformed. A permanent plaintext shadow column is not acceptable.

### Lifecycle and recommended process

```text
ACTIVE -> ROTATING -> DUAL_LOOKUP -> REKEY_VERIFIED -> RETIREMENT_APPROVED -> RETIRED
```

- **Provision:** generate new 256-bit key in an approved secret manager/KMS, never in source/migration/report or ordinary environment history. Assign immutable key version ID and purpose. Require separate operator and security approver for production key lifecycle.
- **Dual lookup:** roll out all API instances with old and new keys; new registrations query every retained alias and write aliases for old plus new versions transactionally. Enforce unique constraints per server/key version and reject mixed/unknown keysets. Record active/retained versions and rollout generation in a controlled key manifest, not database key material.
- **Re-key:** re-enroll each account through fresh authenticated provider observation or an organization OWNER re-entry flow. The trusted boundary computes the new token in memory, matches it against the existing external-account row using a retained old alias plus broker/server and a one-use rekey challenge, then inserts the new alias. Require no change to identity row or binding and append an audit event. Raw identifier is zeroized and never logged or persisted.
- **Partial recovery:** make per-account alias insertion idempotent. Persist per-key migration status and counts without token values. Resume by scanning accounts missing the new alias; preserve old key and uniqueness until all eligible rows have the new alias. Registration is rejected if its process lacks any key declared active/retained by the current rollout generation.
- **Coverage proof:** compare eligible account count to accounts with new alias, include pending/suspended/revoked rows according to retention policy, detect duplicates under the new key before insertion, and produce signed/dual-approved completion evidence. Reconcile counts and SQL constraints; do not report identifiers/tokens.
- **Retire:** require two independent approvals, complete fleet rollout, zero missing aliases for rows that must remain lookupable, no in-flight old-key writer, successful duplicate/integrity scan, recovery window elapsed, backup/restore plan and incident owner. Remove old key from runtime only after approval and audit event. Preserve key version metadata, not key bytes.
- **Compromise:** mark key compromised, block or tightly restrict registrations/lookup, rotate immediately, force provider re-observation/re-enrollment, preserve old token aliases for forensic matching but do not trust them for new verification, invalidate affected verification according to incident policy, and alert. A key compromise cannot reveal original identifiers but can enable dictionary attacks if account-number space is enumerable.

For accounts without provider access, require controlled owner re-entry with an opaque, short-lived challenge and dual control; if no re-enrollment is possible, retain the old key for lookup or retire the account identity and require full new registration. Do not claim complete rotation by deleting old key while rows remain unreachable. Never put raw identifiers in logs, tickets, metrics, audit events or bulk exports.

### Consequences

Requires key-version lifecycle metadata, rotation jobs, alias completeness proofs, concurrency handling, operator review UI and runbooks. During rotation all API versions must share the overlap window; mixed configuration is fail-closed. Re-keying can take time and needs provider availability or human re-entry. Retaining old keys increases compromise exposure; premature retirement breaks duplicate detection. Key retirement must require independent approval.

## 8. Proposed ADR-6 — Verification Revocation, Audit and Privacy

**Status: PROPOSED; requires explicit approval.**

### Audit record

Use append-only, transactionally coupled domain events with: event ID, aggregate/account/binding/transfer ID, previous/next state and revision, actor kind (`USER`, `PLATFORM_VERIFIER`, `PROVIDER_WORKLOAD`, `SYSTEM`, `OPERATOR`), stable actor/workload ID, authority-grant ID and revision, timestamp from server, reason code plus bounded free-text reason, correlation/idempotency ID, evidence IDs/digests, certificate/key version identifiers (never key material), and source IP/device metadata only where privacy policy permits. Separate provider evidence from human decision records. Keep retention and legal hold explicit. Restrict evidence/document reads and log every access. Use append-only database permissions for service roles, independent backups and external tamper-evident audit export; triggers alone cannot prevent a database owner from changing data.

Store evidence digests over canonical versioned bytes, not an unspecified JSON serialization. Preserve canonicalization version and evidence schema. Do not store passwords, bearer tokens, private keys, full account IDs, raw portal screenshots or personal documents in ordinary relational rows/logs. Store a minimal redacted snapshot plus encrypted vault reference only where policy requires.

### Invalidation triggers

- Broker/server identity mismatch or catalogue identity correction: suspend binding and require re-verification; never rewrite history.
- External account mismatch: reject evidence, suspend current verification, security alert and investigation.
- Provider credential/certificate compromise or revocation: mark workload untrusted, stop new evidence, invalidate observations after last known-good boundary and require fresh evidence.
- Evidence age/expiry: transition to `REVERIFICATION_REQUIRED`; no silent extension.
- Reassignment or entitlement revocation: suspend/revoke old binding; new binding starts pending and needs independent review.
- Verifier authority compromise: freeze affected approvals, identify decision interval and actors, review/revoke affected records with a second authorized reviewer; preserve the original decision event.
- HMAC key compromise: initiate rotation, determine affected lookup scope, require re-observation/re-enrollment and reassess verification assurance.

Verification revocation means the identity association is no longer trusted. It does not cancel broker orders, close positions, reconcile order state or imply any execution action. Those behaviors remain outside R3A and require separate execution/reconciliation policy.

## 9. API and database architecture

### Proposed additive entities and controls

Forward-only migrations only; historical migrations remain immutable. Likely additive structures:

- `platform_authority_grants` / `platform_authority_events`: identity user FK, named permission, status, validity window, revision, grantor/revoker, reason; unique current grant and append-only events.
- `broker_provider_workloads`, `workload_credentials` and `workload_account_scopes`: workload principal, certificate/key ID/fingerprint, provider/version, allowed terminal/server/organization challenge scope, status/revision/validity; no private keys.
- `broker_registration_challenges`: binding/registration, nonce digest, target server/identity scope, expiry, consumed timestamp, workload sequence/idempotency, revision. Nonce secret itself returned once and not stored plaintext.
- `broker_account_evidence` extension or normalized evidence envelope table: canonicalization/schema version, observed and received times, provider build, workload identity, challenge, freshness, revocation status, digest and minimal evidence class.
- `broker_verification_cases` / decision events: independent approver identity, authority grant/revision, evidence set, decision/reason, expected binding/account revisions, validity deadline, second approver when policy requires.
- `broker_account_transfers`, source-release and target-acceptance events, dispute state, reservation, expected revisions and idempotency key.
- `broker_identifier_key_versions` and `broker_identifier_rekey_progress`: version/status, active generation, creation/retirement approvals, coverage counts and completion digests; never key bytes or identifiers.
- transactional outbox and audit integrity metadata where existing conventions do not already provide them.

Use FK `ON DELETE RESTRICT`, uniqueness for one active binding/transfer reservation, one-use nonce constraints, unique `(workload, sequence)` and evidence idempotency, status checks, expected-revision compare-and-swap, explicit lock order and append-only events. Do not make provider-supplied strings trusted catalogue identities. Keep organization entitlement as the source of tenant ownership.

### API surfaces

| Surface | Example contract | Authorization |
|---|---|---|
| Organization-facing | `POST /organizations/{org}/broker-bindings` initiate; `POST .../{binding}/evidence-challenge`; `GET .../{binding}/verification-status`; `POST .../{binding}/transfer-request` and target acceptance | Existing JWT subject + live R2 membership/OWNER and entitlement check; no raw identifiers in reads; generic duplicate conflict |
| Provider/workload-facing | `POST /provider/v1/registrations/{id}/evidence` with mTLS, workload identity, nonce, sequence, signed canonical envelope | Certificate principal mapped to active workload and exact account/server/registration scope; no user JWT substitution; replay/freshness checks |
| Platform-verifier-facing | `GET /platform/verifications/queue`; `POST /platform/verifications/{id}/approve|reject`; `POST .../suspend|revoke`; transfer adjudication | Current named platform permission loaded from DB; step-up authentication; expected revisions; self-approval and evidence-submitter conflict checks; dual control when required |
| Restricted operator-facing | `POST /platform/identifier-keys/rotation`; status/coverage; retirement approval; workload credential revoke | Separate operator permission, audited change ticket/correlation, two-person approval for retire/compromise recovery; no key bytes in API response |

Do not return external identifier, HMAC alias, nonce digest, certificate private key, secret-manager path containing secret values, or evidence document to organization API clients. A verifier may receive only masked identity/context needed for the decision, with audited, time-limited evidence access. API edge logs must redact request bodies and identifier-bearing headers.

### Transaction boundary

A final approve/reassign command must atomically: lock latest aggregate revisions in fixed order; validate current actor grants and tenant membership/entitlements; validate evidence freshness and challenge consumption; validate uniqueness/reservation; append decision and audit events; update aggregate state; add outbox event; commit. Provider calls, object-store operations and notifications occur outside the DB transaction through idempotent outbox/saga steps. Fail closed on partial evidence upload; uploaded objects remain quarantined until digest and policy validation succeed.

## 10. Threat model

| Threat | Attacker / impact | Existing R3 protection | Proposed control | Residual risk and required test |
|---|---|---|---|---|
| 1. OWNER self-approval | Tenant owner fabricates own verification | API currently cannot set VERIFIED | Separate platform grant; deny initiator/evidence-submitter; dual review on exceptions | Collusion remains; test direct API and DB path |
| 2. Forged provider evidence | User submits fabricated account metadata | No public evidence endpoint; DB requires evidence row | mTLS workload scope, signed canonical envelope, challenge and independent verifier | Workload itself can lie; test invalid signature, mismatched principal and identity |
| 3. Replayed evidence | Replay old valid frame to reverify | Existing bridge sequences/freshness do not bind R3 challenge | One-use nonce, sequence and idempotency uniqueness, freshness | Compromised live workload can replay before expiry; race tests |
| 4. Compromised provider workload | Attacker reports false account identity in allowed scope | Existing bridge cert principal/identity mapping is a transport boundary, not ownership proof | Narrow allow-list, hardware/ACL key, rapid revoke, independent association evidence | Active compromise can report false values; incident simulation |
| 5. Compromised platform verifier | Fraudulent approval | No verifier exists yet | Least privilege, MFA/step-up, short sessions, immutable audit, second approver for high-risk | Colluding or privileged DB owner risk; red-team revocation drill |
| 6. Cross-org account claiming | Tenant learns/claims another tenant's account | Generic duplicate conflict and unique identity reservation | Uniform response/timing mitigation, private verifier queue, transfer process | Timing side-channel; cross-tenant enumeration test |
| 7. Broker/server substitution | Wrong broker/server mapped to real account | R3 FKs constrain server to broker | Provider scope and signed identity binding; immutable catalogue identity and controlled catalogue approval | Broker naming ambiguity; fixture tests for same server name across brokers |
| 8. Account identity substitution | Attacker replaces claimed login with another ID | HMAC scoped to broker/server and unique lookup | Challenge binds account candidate; provider-observed HMAC must match aliases; step-up review | Provider assertion can still be false; substitution tests |
| 9. Concurrent reassignment | Two orgs acquire one account or duplicate binding | Partial unique indexes reserve pending/active account; no transfer API | Transfer reservation, fixed lock order, expected revisions and single transaction | Deadlocks/availability; concurrent transfer/revoke tests |
| 10. Revocation race | Approval commits while entitlement/authority is revoked | R3 locks membership/entitlement during registration, but no verifier workflow | Lock/check authority, entitlement and verification revisions in approval transaction | External revocation latency; barrier-based race test |
| 11. Stale evidence | Old terminal observation accepted as current | No provider evidence collection yet | Explicit max age, server receipt time, nonce expiry, revalidation state | Provider clock unreliable; skew/future-time tests |
| 12. Stale authorization | Revoked verifier still approves with old JWT | Existing tenant membership rechecked from DB | Do not trust JWT role claim; DB current grant revision on every privileged call | Cache bugs; revocation-before-request test |
| 13. HMAC key compromise | Dictionary attacks or false duplicate signal | HMAC, domain separation, no plaintext storage | KMS custody, scoped purpose, compromise workflow, re-enrollment, dual approval | Low-entropy account IDs may be guessed by attacker with key; compromise exercise |
| 14. Incomplete rotation | Some rows lack new aliases; duplicates evade detection | R3 supports overlap only when process has all keys | Persist coverage, block retirement/unsafe registration, reconcile every retained row | Long-lived disconnected accounts; incomplete-coverage test |
| 15. Premature old-key retirement | Legacy identities become unfindable | No retirement workflow | Two-person retirement, coverage proof, backup/restore and grace window | Incorrect inventory; restore/rekey rehearsal |
| 16. Duplicate creation during rotation | Concurrent old/new-key registrations bypass uniqueness | R3 checks configured aliases transactionally | Require consistent fleet keyset generation and write all retained aliases atomically | Mixed deployment race; mixed-version integration test must fail closed |
| 17. Audit tampering | Insider alters approval/evidence history | R3 append-only triggers | App DB role INSERT-only for events, external immutable export, independent backups | DB owner can disable triggers; tamper-evidence test and privileged threat review |
| 18. Sensitive identifier leakage | Logs/API/errors expose account number or HMAC | R3 masks API and zeroizes request buffers | Body/header redaction, masked reviewer views, restricted vault, DLP and leak tests | APM/proxy capture risk; end-to-end canary identifier test |
| 19. Insider misuse | Operator views or transfers accounts without cause | R3 no platform admin interface | Just-in-time permission, reason/ticket, two-person control, audited evidence reads and alerts | Collusion; periodic access review and anomaly test |
| 20. Tenant-role privilege escalation | OWNER/ADMIN treated as global verifier | R3 only permits OWNER initiation/lifecycle | Separate grant schema and distinct middleware; deny-by-default | Future endpoint omission; authorization matrix and negative route tests |

## 11. Unresolved decisions requiring user approval

The recommendations above are proposals, not accepted decisions. Review is required for:

1. Whether platform verifier permissions should use the existing identity service with a separate authorization domain (recommended) or a fully separate operations identity service.
2. Required identity assurance/MFA/step-up and verifier eligibility process.
3. Whether every approval requires one verifier or selected categories require two; recommendation is two for reassignment, overrides, disputed ownership and key retirement.
4. What counts as sufficient organization-association evidence where a broker does not issue organization proof; legal ownership must not be inferred from terminal access.
5. Evidence retention, document vault, access, privacy jurisdiction and deletion/legal-hold policy.
6. Challenge lifetime, provider clock tolerance, observation freshness window and certificate issuance/revocation ownership.
7. Reassignment source-consent exception rules, appeal process, transfer freeze duration and notification channels.
8. Whether accounts without provider re-enrollment remain on old HMAC versions indefinitely or can be retired from duplicate detection after explicit user action.
9. Who can provision the trusted broker/server catalogue and who approves key compromise response.
10. Required DB service roles, external audit sink and operational ownership.

## 12. Dependency-ordered R3B roadmap

1. **Approve policy decisions:** authority model, proof levels, freshness, retention, reassignment due process, dual-control thresholds and key custody. No implementation should guess these.
2. **Authority/identity foundation:** additive grant/revision/audit model; step-up auth; permission recheck; grant/revoke APIs; verifier audit. Establish platform operators without elevating tenant roles.
3. **Provider workload identity:** distinct workload registry, certificate/key scope and lifecycle, server-side registration challenge and replay ledger. Reuse mTLS only after trust-domain separation review.
4. **Evidence contract/conformance:** canonical envelope, provider metadata mapping, HMAC in-memory processing, evidence digest and privacy controls; demo-conformance using official Windows MetaTrader5 Python API. Keep MQL5 heartbeat out of ownership proof.
5. **Registration/verification workflow:** evidence receipt, pending approval, independent approve/reject, freshness/expiry, optimistic revisions, audited events and outbox. Continue fail-closed pending state until all gates pass.
6. **Reassignment:** transfer case and reservation, dual organization consent, independent platform approval, dispute/appeal path, atomic finalization and concurrency tests.
7. **HMAC rotation:** key manager contract, retained keyset rollout, re-enrollment/re-key challenge, coverage proof, duplicate scan, retirement approval and compromise drill.
8. **Database/API/UI controls:** forward-only migrations, least-privilege runtime roles/RLS review, verifier and operator interfaces, log redaction, accessibility and evidence access audit.
9. **Verification and release controls:** QOS-020 fresh install/upgrade, authorization matrix, replay and concurrency tests, incident/restore exercise, red-team privacy checks, demo provider conformance, security review and explicit release approval. No production rollout is implied by passing local tests.

Steps 2 and policy approval are prerequisites to verifier APIs; step 3 precedes provider evidence; steps 3–4 precede approval lifecycle; step 5 precedes reassignment; key provisioning can be designed in parallel but rotation execution depends on key custody and re-enrollment; production release waits for all policy, security, provider and recovery gates.

## 13. Final confirmation

**COMPLETE for this investigation only.** Six proposed ADRs, recommended authority/evidence/lifecycle/reassignment/key-rotation/audit architecture, API/database contracts, threat model, and dependency-ordered R3B roadmap are documented. They remain proposals pending user decision.

No source implementation, migration, database write, Docker operation, provider connection, terminal operation, credential/certificate change, service restart, deployment, test, build, commit or push occurred. No staging/production system or broker was contacted. R3 remains PARTIAL and unchanged. R4 and R3B implementation require separate authorization.

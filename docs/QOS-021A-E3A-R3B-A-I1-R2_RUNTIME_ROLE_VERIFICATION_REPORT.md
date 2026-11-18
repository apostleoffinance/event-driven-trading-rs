# QOS-021A-E3A-R3B-A-I1-R2 Runtime Role Verification

**Result: PASS for the isolated runtime-role verification.** The service startup guard accepted the intended non-owner runtime role, rejected the tested unsafe role configurations for both APIs, and failed closed against an unmigrated database. This evidence applies only to the disposable QOS-021A-I1-R2 environment; it is not evidence of staging or production readiness.

## Scope and repository preservation

- Repository: `/Users/olaoluwatunmise/event-driven-trading-rs`
- Branch: `main`
- HEAD: `f6446dd33dc74a86ede9064f35aef2ec082a1c59`
- No source, migration, QOS-020 guard/manifest, staging or production files were edited for this verification. The only repository addition from this R2 phase is this report.
- The authorized Cargo build produced or updated ignored build outputs under `target/debug`. Their pre-run hashes were not recorded, so byte-for-byte preservation of pre-existing ignored build artifacts cannot be asserted. No cleanup or restoration of those outputs was performed.
- Before the report was added, `git status --short --untracked-files=all` contained 103 entries. It showed the pre-existing staged and unstaged changes, deletions and untracked work from earlier authorized phases. Those entries were preserved. The report is an additional untracked file; none of the earlier entries were staged, unstaged or discarded.
- Protected implementation hashes observed during the run:

  | File | SHA-256 |
  | --- | --- |
  | `crates/persistence/src/pool.rs` | `a089fa55db792be2a05c23a6b582eb68773613e82f5e2ae57be072e510c41934` |
  | `crates/identity-api/src/main.rs` | `805b91cc4dc146958b947d2606cfb15b872e1ef3a5e1ecbce40fc695cd5f9cf5` |
  | `crates/journal-api/src/main.rs` | `017ce2c6e47234ad614aefff59509ddbd05063b3df458724a8cf8e03b44741b8` |
  | `migrations/20261115000001_identity_suspension_and_audit_hardening.sql` | `740c7f7074381c84f1aa9f326ecbb00cc844705d6f2f2e28611bfa8e6c28fef9` |
  | `scripts/qos020/qos020_guard.py` | `08698f0c013d223886720010a47c5e70248221832134368820f855a178633e1b` |

The QOS-020 guard hash matches the previously authorized value. No credential values, database URLs, access tokens or passwords are included in this report.

## Isolated environment and safety boundaries

Docker context was `desktop-linux` (Docker Engine 27.5.1, Docker Compose 2.32.4, Linux ARM64). The test used immutable image reference `postgres@sha256:57c72fd2a128e416c7fcc499958864df5301e940bca0a56f58fddf30ffc07777` and loopback endpoint `127.0.0.1:51557`.

Resources created solely for this run had verification label `20261112t122822z`:

| Resource | Identity | Outcome |
| --- | --- | --- |
| PostgreSQL container | `qos021a-i1-r2-20261112t122822z-postgres`, ID `09b6dc8f137f25b71c2e737ed071363fba9c864a954149143bcacf56ad820648` | Healthy during checks; removed after verification |
| Volume | `qos021a-i1-r2-20261112t122822z-data` | Removed after verification |
| Network | `qos021a-i1-r2-20261112t122822z-net`, ID `51423d17842abf7d9cd0c7906489da0f97aaffa5e00a6a22a6b5a9a834f425ab` | Removed after verification |

The run used disposable databases `qos021a_i1_r2` and `qos021a_i1_r2_unmigrated`, with a private run-identity marker. Credentials were held in private temporary files and were not printed. The bootstrap principal was used only for disposable-cluster provisioning and read-only verification queries; the application connections used the distinct runtime principal. A separate migrator principal applied migrations to the disposable primary database.

Protected-resource checks before cleanup confirmed:

- QOS-020 container ID `f6f4f45ee58930f5059845a58a023e3f2c2643e1770a0c8ef6f47cb596a0ad82`, run ID `20261110-230504-ffb59dfd0ab4`, remained running and healthy.
- MT5 staging container ID `dc9abdf6c9ae2a7560f95fb1ce5e5903e1ab4def23627bbd0d48c4bdff03a2e7` remained running and healthy; protected volume `edgenest_mt5_bridge_v2_staging_data` remained present.
- Protected networks remained present: `edgenest-mt5-bridge-v2-staging_default` (ID `0667fe1641afae35aadc28227d1ae729450784065704a6969630a0310cd066d8`) and `event-driven-trading-rs_default` (ID `7fad26c50f925bef3a0e5ea2f88368cab289d4e0da702fd33fa42ab6dabced13`).

## Implementation paths examined

- `crates/persistence/src/pool.rs:297-362`, `validate_identity_runtime_role`: checks the exact successful SQLx checksum for migration `20261115000001`, required audit/suspension schema protections, non-superuser/non-owner status, lack of owner-role membership, restricted session-event privileges, no schema CREATE, and sequence USAGE without UPDATE.
- `crates/identity-api/src/main.rs:38-105`: connects with `MIGRATOR_DATABASE_URL`, applies migrations, closes the migrator pool, connects with `DATABASE_URL`, then validates the runtime role before binding the API.
- `crates/journal-api/src/main.rs:230-273`: follows the same separate migrator/runtime pool sequence and validates the runtime role before serving requests.
- `migrations/20261115000001_identity_suspension_and_audit_hardening.sql`: installs the suspension invalidation trigger, composite session/user event relationship, append-only event protections and runtime grants. Its SHA-256 is recorded above.

## Verification commands and results

The following database-capable operations were preceded by the private safety preflight. The preflight checked the active Docker context, exact protected QOS-020/staging identities, exact R2 container/image/labels/attachments/loopback port, and the expected environment marker. Private environment files were sourced without displaying their values.

| Command / operation | Result |
| --- | --- |
| `docker context show` | `desktop-linux` |
| `python3 /private/tmp/qos021a-i1-r2-20261112t122822z/safety_preflight.py --phase live` with the isolated service environment | Exit 0; expected run/container/database identity verified |
| `cargo run --locked -p persistence --bin persistence-migrate` with `DATABASE_URL` set to the isolated migrator URL | Exit 0; output: `database migrations applied successfully` |
| `cargo build --locked -p identity-api -p journal-api` | Exit 0 |
| Positive local service verification | Identity `/readyz` HTTP 200; registration HTTP 201 with access and refresh credentials returned to the harness and suppressed; authenticated Journal `GET /api/v1/organizations` HTTP 200 |
| Read-only `pg_stat_activity` query after startup | 4 connections as `edgenest_runtime`; 0 as `qos021a_migrator` |
| Runtime role privilege vector query | `false|false|false|false|false|true|true|false|false|false|false|false|true|false` in order: superuser, CREATEDB, CREATEROLE, owns event table, member of owner, SELECT, INSERT, UPDATE, DELETE, TRUNCATE, TRIGGER, schema CREATE, sequence USAGE, sequence UPDATE |
| Direct role-specific access checks on `identity_session_events` | SELECT succeeded; UPDATE, DELETE, TRUNCATE, trigger creation and schema CREATE were denied by PostgreSQL permissions |

### Negative startup matrix

For each case below, both `identity-api` and `journal-api` exited with code `2` before listening and emitted the restricted-role validator diagnostic. The complete captured case list is represented here; test accounts and passwords are omitted.

| Rejected condition | Identity API | Journal API |
| --- | --- | --- |
| Superuser | PASS, exit 2 | PASS, exit 2 |
| CREATEDB | PASS, exit 2 | PASS, exit 2 |
| CREATEROLE | PASS, exit 2 | PASS, exit 2 |
| Migration/table owner | PASS, exit 2 | PASS, exit 2 |
| Inherited owner membership | PASS, exit 2 | PASS, exit 2 |
| Event-table UPDATE | PASS, exit 2 | PASS, exit 2 |
| Event-table DELETE | PASS, exit 2 | PASS, exit 2 |
| Event-table TRUNCATE | PASS, exit 2 | PASS, exit 2 |
| Event-table TRIGGER | PASS, exit 2 | PASS, exit 2 |
| Missing event-table SELECT | PASS, exit 2 | PASS, exit 2 |
| Missing event-table INSERT | PASS, exit 2 | PASS, exit 2 |
| Missing sequence USAGE | PASS, exit 2 | PASS, exit 2 |
| Schema CREATE | PASS, exit 2 | PASS, exit 2 |

Aggregate: 26/26 rejected as expected. Each case passed safety preflight before starting the API binary.

### Wrong-database fail-closed check

The safety preflight separately verified the identity marker in `qos021a_i1_r2_unmigrated`. With the runtime URL pointed to that database, both services exited `2` before listening. Each emitted:

```text
database runtime role is not sufficiently restricted: database error: error returned from database: relation "public._sqlx_migrations" does not exist
```

The first attempted preflight in this resumed turn was denied because the shell still held the primary database URL while the requested preflight database was the mismatch database. No database operation followed that denial. The URL path was then selected in memory for the already-created, run-marked mismatch database; the safety preflight passed, and both fail-closed startup checks completed with exit code 2. No credential value was printed or persisted in the report.

## Cleanup and final state

Only the R2-labeled container, volume and network listed above were removed. Their exact labels and IDs were checked immediately before removal. The two local API processes started for positive verification were confirmed by PID and executable path, then sent SIGTERM; both stopped and ports 18180/18181 had no remaining listener.

After cleanup, Docker inspection confirmed the protected QOS-020 and MT5 staging containers remained healthy and the protected staging volume and networks remained. The test container, volume and network are absent. No staging/production database, container, volume, network, service, or credential configuration was changed.

**Conclusion:** The R1 runtime-role startup enforcement is demonstrated for both APIs in the disposable environment. The intended runtime role can start and serve the tested requests; the tested over-privileged, under-privileged, ownership and schema-mismatch cases fail closed. This report does not authorize I2 or any staging/production operation.

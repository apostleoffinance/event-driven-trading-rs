# QOS-021A-E3A-R3B-R — QOS-020 Read-Only Recovery Investigation

**Overall status: VERIFIED**  
**Classification: A — ORIGINAL ENVIRONMENT VERIFIED**  
**Repository:** `/Users/olaoluwatunmise/event-driven-trading-rs`  
**Branch / HEAD:** `main` / `f6446dd33dc74a86ede9064f35aef2ec082a1c59`  
**Investigation:** 2026-11-12, read-only

## 1. Repository preflight and preservation

Observed repository identity:

```text
$ pwd
/Users/olaoluwatunmise/event-driven-trading-rs
$ git rev-parse --show-toplevel
/Users/olaoluwatunmise/event-driven-trading-rs
$ git branch --show-current
main
$ git rev-parse HEAD
f6446dd33dc74a86ede9064f35aef2ec082a1c59
```

The pre-investigation `git status --short --untracked-files=all` contained:

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
?? docs/QOS-021A-E3A-R3A_BROKER_VERIFICATION_AUTHORITY_AND_EVIDENCE_ARCHITECTURE.md
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

Pre-existing ignored paths recorded at preflight were `.conda/`, `.pytest_cache/`, `.venv/`, `.vscode/`, MT5 Python adapter cache/egg-info directories, frontend build/cache/node_modules/generated metadata, `migration-report.json`, `migration-state.json`, `migration/`, Python caches, strategy egg-info and `target/`. They were not modified.

The only repository change in this investigation is this report. The existing staged, unstaged, deleted, untracked and ignored work was preserved. No source file, QOS-020 manifest, guard, Docker resource or database was modified.

## 2. Relevant QOS-020 file hashes

SHA-256 before and after the investigation:

| File | SHA-256 |
|---|---|
| `scripts/qos020/qos020_guard.py` | `08698f0c013d223886720010a47c5e70248221832134368820f855a178633e1b` |
| `scripts/qos020/provision.py` | `b48af5f99dc6af11f8504d5272f470384046dbde58cc07194800a5e0db35525d` |
| `scripts/qos020/docker-compose.qos020.yml` | `4a8348f8474d5862f2b56c3b7f24b36db6f9f4dee61bada3001c515c535e89f7` |
| `scripts/qos020/init/10-qos020-init.sql` | `a3eaa377444d0f6a030a5320880a2de8f7237ae3aff43b41b64caa1a4ac225ed` |
| `docs/QOS-020_TEST_INFRASTRUCTURE.md` | `a439e0ca37f4d863a381c523bf3b3ba3548a2eab0d678b0e947a3b946a167bbb` |

Manifest path: `/var/folders/9f/dspzp56j3pjcftph22hs9gw40000gn/T/qos020/20261110-230504-ffb59dfd0ab4/manifest.json`. It exists. Its non-secret identity fields match the expected QOS-020 run, project, container, network, volume, image, endpoint, database, role contract and compose paths.

The existing QOS-020 guard is unchanged. It reads the container ID from the manifest and inspects that exact ID; it checks the configured image reference, container status and labels, Compose project/name/service/working directory/config file, volume mount and labels, network identity/labels/attached containers, PostgreSQL major version, database name and environment/run marker, and restricted SQLx role attributes. Its identity checks are bound to the manifest's exact container ID; it does not discover a replacement container by name.

## 3. Docker CLI, context and daemon

Read-only command results:

```text
$ command -v docker
/usr/local/bin/docker
$ docker context show
desktop-linux
$ docker context ls
NAME              DESCRIPTION                               DOCKER ENDPOINT
 default           Current DOCKER_HOST based configuration   unix:///var/run/docker.sock
desktop-linux *   Docker Desktop                            unix:///Users/olaoluwatunmise/.docker/run/docker.sock
$ docker context inspect desktop-linux --format '{{json .Endpoints.docker}}'
{"Host":"unix:///Users/olaoluwatunmise/.docker/run/docker.sock","SkipTLSVerify":false}
$ docker info --format ...
server=27.5.1 os=linux arch=aarch64 name=docker-desktop root=/var/lib/docker cpus=8 mem=4108275712
```

Docker Desktop daemon is reachable in the `desktop-linux` context. The first default-sandbox Docker calls returned `permission denied ... operation not permitted` for the Docker socket. The same read-only inspections succeeded after requesting elevated permission for the explicitly authorized Docker metadata reads. This was a sandbox socket-access restriction, not evidence that the container was missing or Docker Desktop was stopped.

The manifest does not record the Docker context name used at provisioning time. Therefore historical context equality cannot be independently established from the manifest. However, the exact recorded container and associated resources are present in the current `desktop-linux` context and pass the existing guard. No commands were run against the `default` context endpoint.

## 4. Original container and associated resources

The all-container inventory showed the recorded QOS-020 container running and healthy. Other active project containers were listed by Docker but not changed. The bridge staging container was visible as a separate project and was not queried beyond metadata or modified.

Exact container inspection (no environment variables or database contents were requested):

```text
f6f4f45ee58930f5059845a58a023e3f2c2643e1770a0c8ef6f47cb596a0ad82
qos020-integration-20261110-230504-ffb59dfd0ab4-postgres-1
image reference: postgres@sha256:57c72fd2a128e416c7fcc499958864df5301e940bca0a56f58fddf30ffc07777
state: running=true; Up 34 hours (healthy)
created: 2026-11-10T23:05:05.612508173Z
Compose project: qos020-integration-20261110-230504-ffb59dfd0ab4
Compose service: postgres
QOS-020 run_id: 20261110-230504-ffb59dfd0ab4
QOS-020 provisioning_id: 20261110-230504-ffb59dfd0ab4
Compose working directory: /Users/olaoluwatunmise/event-driven-trading-rs/scripts/qos020
Compose file: /Users/olaoluwatunmise/event-driven-trading-rs/scripts/qos020/docker-compose.qos020.yml
```

Container mount metadata shows the expected named volume mounted at `/var/lib/postgresql/data`. A separate read-only secret-file bind mount exists at `/run/secrets/qos020_bootstrap_password`; its contents were not read. Docker image inspection reports image ID `sha256:7e7dbab8d3b431a20793a6d99cb5a6bc84e44914309917f1bf5589a7568cdefd`, repo digest `postgres@sha256:57c72fd2a128e416c7fcc499958864df5301e940bca0a56f58fddf30ffc07777`, platform `linux/arm64`.

The exact ID filter, Compose-project label filter and volume-use filter each returned only this QOS-020 container.

## 5. Volume and network identity

**Volume:**

```text
name: qos020_it_20261110_230504_ffb59dfd0ab4
driver: local
mountpoint: /var/lib/docker/volumes/qos020_it_20261110_230504_ffb59dfd0ab4/_data
created: 2026-11-10T23:05:05Z
labels: project=qos020-integration-20261110-230504-ffb59dfd0ab4
        run_id=20261110-230504-ffb59dfd0ab4
        provisioning_id=20261110-230504-ffb59dfd0ab4
        environment=integration-test
```

The volume is attached to the expected container and remains present. It was not mounted by an external process, copied, exported or read.

**Network:**

```text
name: qos020-integration-net-20261110-230504-ffb59dfd0ab4
id: 4a23a164c583ba8f8bcc33bf2f29ab20066d6df65f490c173fc17d0d1285647b
driver: bridge
scope: local
labels: project=qos020-integration-20261110-230504-ffb59dfd0ab4
        run_id=20261110-230504-ffb59dfd0ab4
        provisioning_id=20261110-230504-ffb59dfd0ab4
        environment=integration-test
attached containers: exactly the recorded QOS-020 container ID
```

## 6. Manifest/guard comparison and identity verification

The manifest records:

```text
run_id=20261110-230504-ffb59dfd0ab4
project=qos020-integration-20261110-230504-ffb59dfd0ab4
container_id=f6f4f45ee58930f5059845a58a023e3f2c2643e1770a0c8ef6f47cb596a0ad82
network=qos020-integration-net-20261110-230504-ffb59dfd0ab4
volume=qos020_it_20261110_230504_ffb59dfd0ab4
image_reference=postgres@sha256:57c72fd2a128e416c7fcc499958864df5301e940bca0a56f58fddf30ffc07777
host=127.0.0.1
port=51462
database=qos020_test_master
postgres_major=16
lane=integration-test
environment_type=integration-test
provisioning_id=20261110-230504-ffb59dfd0ab4
```

The provisioner saves the actual returned container ID and image reference in the manifest. The guard reads that immutable manifest ID and validates it against Docker inspect data; it also validates project, run/provisioning labels, volume and network association, image and database marker. It does not modify the manifest or infer a replacement identity.

The guard was rerun only after independent Docker metadata confirmed the exact original resources. Command and result:

```text
$ python3 scripts/qos020/qos020_guard.py \
  --manifest /var/folders/9f/dspzp56j3pjcftph22hs9gw40000gn/T/qos020/20261110-230504-ffb59dfd0ab4/manifest.json \
  --secrets-dir /var/folders/9f/dspzp56j3pjcftph22hs9gw40000gn/T/qos020/20261110-230504-ffb59dfd0ab4/secrets \
  --lane integration-test \
  --run-id 20261110-230504-ffb59dfd0ab4 \
  -- python3 -c 'print("QOS-020 read-only identity verification passed")'
QOS-020 read-only identity verification passed
QOS-020 identity verified: project=qos020-integration-20261110-230504-ffb59dfd0ab4 run_id=20261110-230504-ffb59dfd0ab4 host=127.0.0.1 port=51462 database=qos020_test_master role=qos020_sqlx
exit code: 0
```

The guard verified container/image/project/network/volume and database marker/version/role identity, then ran only the harmless print command. No migrations, tests or ad hoc SQL were run. The guard performed only its documented read-only identity queries; database contents were not inspected.

## 7. Classification and recovery conclusion

**A. ORIGINAL ENVIRONMENT VERIFIED.** The exact container ID and corresponding project, image, network, volume, endpoint and database identity are present and pass the existing guard in the current Docker Desktop context. This is not classification B, C, D, E or F.

The original environment is already running and healthy; no recovery or recreation is needed. The earlier guard failure was caused by sandbox permission to the Docker socket. With explicitly elevated permission for read-only inspection, both resource inspection and the same unchanged guard succeeded.

The QOS-020 instance can be used for future authorized guarded integration testing without recreating it. This finding alone does not authorize tests or R3B work; separate scope restrictions and the earlier R3B step-up/MFA blocker remain applicable.

## 8. Final status, hashes and no-change confirmation

After the investigation, the Git status is the preflight status above plus this new report only. Staged entries remain `MM Cargo.lock` and `M  docker-compose.yml`; no other file was staged. The five QOS-020 protected file hashes remained exactly as listed in Section 2. No reset, stash, clean, checkout, restore, commit or push occurred.

No container, image, network, volume, database, manifest, QOS-020 guard, repository source file, staging resource or production resource was modified. No Docker lifecycle operation, database migration, ad hoc SQL statement, test, SQLx operation, provider connection, terminal operation, broker operation or trading action occurred. The existing guard performed only read-only identity queries. The QOS-020 container, network and volume remain present and running/attached as observed.

## 9. Outstanding risk and next action

The Docker context used during original provisioning is not recorded in the manifest, so historical context-name equality is unknown. The current selected Docker Desktop context contains the exact recorded resources and the guard confirms their identity, making this limitation non-blocking for QOS-020 use.

The recommended next action is to review this recovery report. Any subsequent R3B implementation must still satisfy its separate authorization, including step-up/MFA for platform approval operations. The existing identity service provides subject-based JWT authentication but no verified step-up/MFA claim or mechanism was found during the prior R3B preflight; this investigation did not change or resolve that gap. No R3B implementation was performed here.

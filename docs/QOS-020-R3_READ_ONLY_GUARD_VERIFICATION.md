# QOS-020-R3 — Read-only Guard Verification

**Result: PASS.** The unchanged QOS-020 guard verified the original isolated integration-test instance and found no SQLx-generated test databases remaining. No test suite or migration was run.

## 1. Repository identity and preservation

Before guard invocation:

```text
$ pwd
/Users/olaoluwatunmise/event-driven-trading-rs
$ git rev-parse --show-toplevel
/Users/olaoluwatunmise/event-driven-trading-rs
$ git branch --show-current
main
$ git rev-parse HEAD
f6446dd33dc74a86ede9064f35aef2ec082a1c59
$ git status --short --untracked-files=all
<complete status captured: 100 entries>
$ git diff --cached --stat
 Cargo.lock         | 11 +++++++++++
 docker-compose.yml |  2 +-
 2 files changed, 12 insertions(+), 1 deletion(-)
```

The pre-run status contained the existing staged, unstaged, deleted and untracked work, including the prior QOS-020-R2 and MT5 MVP documents. The staged paths were `Cargo.lock` and `docker-compose.yml`; `Cargo.lock` also had unstaged changes. No Git command that changes repository state was run.

After the guard, the complete `git status --short --untracked-files=all` was captured again and compared entry-for-entry with the pre-run status: the 100 pre-existing entries were unchanged. After creating this report, the only additional status entry is this requested untracked report. No pre-existing file or ignored work was changed.

## 2. Manifest and guard hashes

```text
$ shasum -a 256 /var/folders/9f/dspzp56j3pjcftph22hs9gw40000gn/T/qos020/20261110-230504-ffb59dfd0ab4/manifest.json scripts/qos020/qos020_guard.py
f79bfdab4262f6d47a19205e7823c052f2a8778c967a1c579e97bfc6dbbeea9e  /var/folders/9f/dspzp56j3pjcftph22hs9gw40000gn/T/qos020/20261110-230504-ffb59dfd0ab4/manifest.json
08698f0c013d223886720010a47c5e70248221832134368820f855a178633e1b  scripts/qos020/qos020_guard.py
```

Both hashes match the authorized expected values. The guard and manifest were not modified. No credential values were printed, copied or inspected outside the specifically authorized guard operation.

## 3. Docker context and resource preflight

```text
$ docker context show
desktop-linux
exit code: 0

$ docker info --format 'Name={{.Name}} ServerVersion={{.ServerVersion}} OperatingSystem={{.OperatingSystem}} OSType={{.OSType}} Architecture={{.Architecture}}'
Name=docker-desktop ServerVersion=27.5.1 OperatingSystem=Docker Desktop OSType=linux Architecture=aarch64
exit code: 0
```

Read-only filtered Docker inspection confirmed these manifest identities before running the guard:

| Resource | Current evidence | Result |
|---|---|---|
| Container | `f6f4f45ee58930f5059845a58a023e3f2c2643e1770a0c8ef6f47cb596a0ad82`; `qos020-integration-20261110-230504-ffb59dfd0ab4-postgres-1`; running, healthy | Match |
| Project/service/run | Project `qos020-integration-20261110-230504-ffb59dfd0ab4`; service `postgres`; run/provisioning ID `20261110-230504-ffb59dfd0ab4` | Match |
| Image | `postgres@sha256:57c72fd2a128e416c7fcc499958864df5301e940bca0a56f58fddf30ffc07777`; local image ID `sha256:7e7dbab8d3b431a20793a6d99cb5a6bc84e44914309917f1bf5589a7568cdefd` | Match |
| Compose provenance | Working directory `/Users/olaoluwatunmise/event-driven-trading-rs/scripts/qos020`; file `scripts/qos020/docker-compose.qos020.yml` | Match |
| Volume | `qos020_it_20261110_230504_ffb59dfd0ab4`, attached at `/var/lib/postgresql/data`, expected project/run/provisioning labels | Match |
| Network | `qos020-integration-net-20261110-230504-ffb59dfd0ab4`; ID `4a23a164c583ba8f8bcc33bf2f29ab20066d6df65f490c173fc17d0d1285647b`; only the expected container attached | Match |
| Published endpoint | PostgreSQL `5432/tcp` bound to `127.0.0.1:51462` | Match; loopback-only |

No Docker resource lifecycle or configuration operation was performed. These preflight inspections used filtered output; container environment values were not emitted.

## 4. Guard invocation

The current context was already `desktop-linux`; it was not switched. The exact unchanged guard was invoked with the manifest's run ID and its read-only SQLx cleanup verification mode:

```text
$ python3 scripts/qos020/qos020_guard.py \
    --manifest /var/folders/9f/dspzp56j3pjcftph22hs9gw40000gn/T/qos020/20261110-230504-ffb59dfd0ab4/manifest.json \
    --secrets-dir /var/folders/9f/dspzp56j3pjcftph22hs9gw40000gn/T/qos020/20261110-230504-ffb59dfd0ab4/secrets \
    --lane integration-test \
    --run-id 20261110-230504-ffb59dfd0ab4 \
    --verify-sqlx-cleanup
QOS-020 identity verified: project=qos020-integration-20261110-230504-ffb59dfd0ab4 run_id=20261110-230504-ffb59dfd0ab4 host=127.0.0.1 port=51462 database=qos020_test_master role=qos020_sqlx
SQLx generated test databases: none remain
exit code: 0
```

No child test command was supplied. The guard read the SQLx credential file internally as explicitly authorized; the value was not emitted.

## 5. Guard check results

Every check below passed because the unchanged guard completed all validation and returned exit code 0. The output intentionally reports only the run/project/endpoint/database/role identifiers, not credential material.

| Guard validation | Result | Evidence |
|---|---|---|
| Explicit integration-test lane and supplied run ID match manifest | PASS | Guard accepted the invocation and printed the expected run ID. |
| Manifest project, generated volume/network, integration-test environment, PostgreSQL major, immutable image digest and provisioning ID are valid | PASS | Guard completed; manifest hash and identity fields were independently checked in preflight. |
| Endpoint is allowed and matches the manifest | PASS | Guard reported `127.0.0.1:51462`, `qos020_test_master`, role `qos020_sqlx`; no protected endpoint was accessed. |
| Exact container ID, expected image reference/image ID, running state, Compose project/service/name/working directory/config file and QOS-020 labels | PASS | Guard completed; read-only Docker preflight independently confirmed these Docker identities. |
| Container publishes PostgreSQL only on the expected loopback host port | PASS | Preflight showed `5432/tcp` → `127.0.0.1:51462`; guard also validates the binding. |
| Exact volume exists, labels match, and it is mounted at the PostgreSQL data directory | PASS | Guard completed; preflight confirmed the volume name, labels and mount. |
| Exact network exists, labels match, and only the expected container is attached | PASS | Guard completed; preflight confirmed network ID and sole attachment. |
| PostgreSQL server major and database name | PASS | Guard's read-only `SHOW server_version_num` and database identity check passed; expected major is 16 and database is `qos020_test_master`. |
| Database environment marker, run ID, provisioning ID, expected major and instance-creation identity | PASS | Guard's read-only marker query passed all equality checks against the manifest. |
| Dedicated SQLx role attributes | PASS | Guard confirmed `LOGIN` and `CREATEDB`; confirmed not superuser, not createrole, no replication privilege and no `BYPASSRLS`. |
| SQLx credential-file consistency | PASS | Guard confirmed a credential was present and matched its provisioned secret file; no value was displayed. |
| SQLx generated database cleanup | PASS | `SQLx generated test databases: none remain`. |

The guard's database work comprised its documented read-only version, marker, role and SQLx database-list queries. No SQL write or cleanup mutation was run.

## 6. Access boundary and residual uncertainty

This verification used authorized elevated access to the Docker Desktop socket after the prior R2 diagnosis established that default sandbox socket access was denied. The selected context remained `desktop-linux`. The guard passed without modifying or substituting resources.

This verifies the QOS-020 identity and the guard's current read-only database checks. It does not run migration tests, inspect application schemas, establish backup/restore evidence, or prove future availability. No staging or production endpoint was accessed. No SQLx integration tests were run.

## 7. Prohibited-operation confirmation

No migrations, schema changes, SQL writes, SQLx tests, application tests, staging/production access, Docker lifecycle operations, credential rotation, source remediation, commits, pushes, resets, stashes, checkouts or cleanup occurred. No MT5 dispatch or execution behavior was touched. No recovery dump was used. The report itself is the only intended repository addition.

**Next gate:** QOS-020 guard verification is now passing. Begin QOS-021A-E3A-R3B-A-I1-R1 only under its separate authorization; do not run guarded SQLx tests or begin I1-R1 as part of this QOS-020-R3 task.

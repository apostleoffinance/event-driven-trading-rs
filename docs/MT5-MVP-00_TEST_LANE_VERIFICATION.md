# MT5-MVP-00 Test Lane Verification

**Review date:** 2026-11-12  
**Result: PARTIALLY VERIFIED.** QOS-020 repository configuration and manifest identity are consistent, and QOS-020-R3 previously passed. Current runtime verification is blocked because Docker inspection cannot connect to the `desktop-linux` socket. No test, migration, database write, Docker lifecycle command, or alternate context was used.

## Repository and QOS-020 identity

Repository baseline at inspection: `/Users/olaoluwatunmise/event-driven-trading-rs`, branch `main`, HEAD `f6446dd33dc74a86ede9064f35aef2ec082a1c59`.

| Identity | Manifest/config evidence |
|---|---|
| Run / project | `20261110-230504-ffb59dfd0ab4` / `qos020-integration-20261110-230504-ffb59dfd0ab4` |
| Container | `f6f4f45ee58930f5059845a58a023e3f2c2643e1770a0c8ef6f47cb596a0ad82`; name `qos020-integration-20261110-230504-ffb59dfd0ab4-postgres-1` |
| Volume / network | `qos020_it_20261110_230504_ffb59dfd0ab4` / `qos020-integration-net-20261110-230504-ffb59dfd0ab4` |
| Endpoint / database / role | `127.0.0.1:51462` / `qos020_test_master` / `qos020_sqlx` |
| PostgreSQL image | `postgres@sha256:57c72fd2a128e416c7fcc499958864df5301e940bca0a56f58fddf30ffc07777` |
| Compose working directory / file | `scripts/qos020` / `scripts/qos020/docker-compose.qos020.yml` |
| Manifest | `/private/var/folders/9f/dspzp56j3pjcftph22hs9gw40000gn/T/qos020/20261110-230504-ffb59dfd0ab4/manifest.json`; SHA-256 `f79bfdab4262f6d47a19205e7823c052f2a8778c967a1c579e97bfc6dbbeea9e` |
| Guard | `scripts/qos020/qos020_guard.py`; SHA-256 `08698f0c013d223886720010a47c5e70248221832134368820f855a178633e1b` |

Manifest identity fields were read through an allowlisted field list; credential material was not read or printed. `docker-compose.qos020.yml` pins the same PostgreSQL digest, binds the generated host port to `127.0.0.1`, and labels generated resources. `provision.py` generates isolated run/project/volume/network IDs and mode-0600 credential files under the OS temporary directory. The provisioner is not part of this verification and was not run.

`qos020_guard.py` validates the run/project/resource naming contract, image digest, endpoint, role, protected ports/databases, Docker labels, mounted volume, network attachment, PostgreSQL major version, environment marker, role flags, credential-file match, and optionally SQLx database cleanup. `test_guard.py` contains negative checks for protected staging/legacy ports and databases, production-like hosts, marker/identity/resource mismatches, unsafe role flags, malformed URLs, and missing/mismatched credentials. `init/10-qos020-init.sql` creates the restricted SQLx role (`CREATEDB` only among elevated role flags), grants access to the disposable master database, and records an environment identity marker. These test source files were inspected, not executed.

## Current access attempt

Command and exact result:

```text
$ docker context show
desktop-linux
$ python3 scripts/qos020/qos020_guard.py --manifest /private/var/folders/9f/dspzp56j3pjcftph22hs9gw40000gn/T/qos020/20261110-230504-ffb59dfd0ab4/manifest.json --secrets-dir /private/var/folders/9f/dspzp56j3pjcftph22hs9gw40000gn/T/qos020/20261110-230504-ffb59dfd0ab4/secrets --lane integration-test --run-id 20261110-230504-ffb59dfd0ab4 --verify-sqlx-cleanup
QOS-020 guard rejected command: CalledProcessError: Command '['docker', 'inspect', 'f6f4f45ee58930f5059845a58a023e3f2c2643e1770a0c8ef6f47cb596a0ad82']' returned non-zero exit status 1.
exit 2
```

The guard failed during Docker identity inspection; its PostgreSQL checks and SQLx cleanup check were not reached. No Docker context change or permission workaround was attempted. Current QOS-020 live identity and health are therefore **NOT VERIFIED** in this phase.

## Historical evidence

`docs/QOS-020-R3_READ_ONLY_GUARD_VERIFICATION.md` records that the unchanged guard previously exited `0` for this exact run, verified the container/volume/network/endpoint and database identity, passed documented read-only role and SQLx cleanup checks, and preserved repository status. That is historical evidence and does not override the failed current invocation.

QOS-020 remains the only authorized disposable SQLx test lane. Do not substitute MT5 staging (`127.0.0.1:15543`, database `mt5_v2_staging`) or legacy (`127.0.0.1:15433`, database `trading`) resources. No staging or production database was accessed.

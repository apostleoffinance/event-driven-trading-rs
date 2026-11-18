# QOS-020-R2 — Read-only Test Infrastructure Recovery Diagnosis

**Classification: A — Original resources verified; guard failure was caused by Docker socket access in the restricted execution context.**  
**Date:** 2026-11-12  
**Scope:** read-only repository, manifest and Docker identity/state inspection. No database connection or test was made during this diagnosis.

## Summary

The previously verified QOS-020 PostgreSQL container was not missing. The exact manifest-listed container exists in Docker context `desktop-linux`, is running and healthy, and has the exact QOS-020 run/project labels, image digest, loopback port binding, persistent volume and network. The manifest hash matches the previously recorded SHA-256. The volume and network exist and carry matching project/run/provisioning labels.

The earlier guard failure was a misleading wrapper result: in the default sandbox, the Docker CLI could not access Docker Desktop's Unix socket and returned `permission denied ... operation not permitted`. The guard wraps a failed `docker inspect` in `CalledProcessError`, so its output identified the command and exit failure but did not expose the underlying socket-permission cause. With authorized elevated access and a filtered format that excluded environment variables, the same exact container inspection succeeded. The current Docker context remains `desktop-linux`; no context mismatch was found.

The previous recovery report, `docs/QOS-021A-E3A-R3B-R_QOS020_RECOVERY_INVESTIGATION.md`, records that the unchanged guard passed with exit code 0 under `desktop-linux` after authorized Docker access. Its recorded manifest, container, network, volume, image, host port and labels agree with the present read-only Docker observations. This diagnosis did **not** rerun the guard: the guard reads the SQLx credential file and performs database identity/cleanup queries. No credential inspection or database-capable test was authorized here. Consequently, current SQL-level marker, role, PostgreSQL health and SQLx cleanup remain unverified by this run.

## Repository preservation

Commands:

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
<complete status captured; 99 entries before this requested report>
$ git diff --cached --stat
 Cargo.lock         | 11 +++++++++++
 docker-compose.yml |  2 +-
 2 files changed, 12 insertions(+), 1 deletion(-)
$ git diff --stat
44 files changed, 2751 insertions(+), 1007 deletions(-)
```

The complete status included staged and unstaged modifications, three deleted venue connector tests, and untracked source, tests, migrations, reports, QOS-020 files and migration backups. The only staged paths were `Cargo.lock` and `docker-compose.yml`; `Cargo.lock` also had unstaged changes. No Git state was changed. This requested diagnosis is the only intended addition; ignored work was not enumerated beyond the user's request to preserve it.

## Manifest and guard identity

Manifest path:

`/var/folders/9f/dspzp56j3pjcftph22hs9gw40000gn/T/qos020/20261110-230504-ffb59dfd0ab4/manifest.json`

Command:

```text
$ shasum -a 256 <manifest> scripts/qos020/qos020_guard.py scripts/qos020/provision.py docs/QOS-021A-E3A-R3B-R_QOS020_RECOVERY_INVESTIGATION.md
f79bfdab4262f6d47a19205e7823c052f2a8778c967a1c579e97bfc6dbbeea9e  <manifest>
08698f0c013d223886720010a47c5e70248221832134368820f855a178633e1b  scripts/qos020/qos020_guard.py
b48af5f99dc6af11f8504d5272f470384046dbde58cc07194800a5e0db35525d  scripts/qos020/provision.py
ba0dfa29f5ee30e2af463415df04a75f76f12744f04af29f628f8ec3506dc8b2  docs/QOS-021A-E3A-R3B-R_QOS020_RECOVERY_INVESTIGATION.md
```

The manifest hash exactly matches the known hash. The guard hash matches the prior recovery report's recorded hash, so the guard source is unchanged. Non-secret manifest identity fields:

| Field | Manifest value |
|---|---|
| Run / provisioning ID | `20261110-230504-ffb59dfd0ab4` |
| Compose project | `qos020-integration-20261110-230504-ffb59dfd0ab4` |
| Container ID | `f6f4f45ee58930f5059845a58a023e3f2c2643e1770a0c8ef6f47cb596a0ad82` |
| Volume | `qos020_it_20261110_230504_ffb59dfd0ab4` |
| Network | `qos020-integration-net-20261110-230504-ffb59dfd0ab4` |
| Expected endpoint | `127.0.0.1:51462` |
| Database / role | `qos020_test_master` / `qos020_sqlx` |
| Image reference | `postgres@sha256:57c72fd2a128e416c7fcc499958864df5301e940bca0a56f58fddf30ffc07777` |
| Local image ID | `sha256:7e7dbab8d3b431a20793a6d99cb5a6bc84e44914309917f1bf5589a7568cdefd` |
| Lane / environment | `integration-test` / `integration-test` |

No secret values were read into the report.

## Docker context and availability

Commands and outcomes:

```text
$ docker context show
desktop-linux
exit code: 0

$ docker context ls
NAME              DESCRIPTION                               DOCKER ENDPOINT                                         ERROR
default           Current DOCKER_HOST based configuration   unix:///var/run/docker.sock
desktop-linux *   Docker Desktop                            unix:///Users/olaoluwatunmise/.docker/run/docker.sock
exit code: 0
```

In the default restricted execution context, Docker Desktop socket access was denied. For example:

```text
$ docker inspect f6f4f45ee58930f5059845a58a023e3f2c2643e1770a0c8ef6f47cb596a0ad82
permission denied while trying to connect to the Docker daemon socket at unix:///Users/olaoluwatunmise/.docker/run/docker.sock: ... operation not permitted
exit code: 1
```

The non-elevated `docker info --format ...` likewise printed a socket permission error and empty formatted fields (the CLI command wrapper returned exit code 0). Thus exit code alone for that formatted info invocation was not a successful daemon-health result.

After obtaining authorized elevated permission for read-only metadata inspection, Docker reported:

```text
$ docker info --format 'Name={{.Name}} ServerVersion={{.ServerVersion}} OperatingSystem={{.OperatingSystem}} OSType={{.OSType}} Architecture={{.Architecture}}'
Name=docker-desktop ServerVersion=27.5.1 OperatingSystem=Docker Desktop OSType=linux Architecture=aarch64
exit code: 0
```

The context name and endpoint match the prior recovery report. The `default` context (`/var/run/docker.sock`) was not used. No Docker configuration or resource lifecycle operation was performed.

## Exact container verification

To avoid printing container environment values, the successful inspection used a filtered Go template limited to ID, name, image, state, selected labels, mounts and networks:

```text
$ docker inspect --format 'ID={{.Id}} name={{.Name}} image_ref={{.Config.Image}} image_id={{.Image}} status={{.State.Status}} started={{.State.StartedAt}} health={{if .State.Health}}{{.State.Health.Status}}{{else}}none{{end}} project={{index .Config.Labels "com.docker.compose.project"}} service={{index .Config.Labels "com.docker.compose.service"}} working_dir={{index .Config.Labels "com.docker.compose.project.working_dir"}} config={{index .Config.Labels "com.docker.compose.project.config_files"}} run={{index .Config.Labels "qos020.run_id"}} provisioning={{index .Config.Labels "qos020.provisioning_id"}} mounts={{json .Mounts}} networks={{json .NetworkSettings.Networks}}' <container-id>
ID=f6f4f45ee58930f5059845a58a023e3f2c2643e1770a0c8ef6f47cb596a0ad82
name=/qos020-integration-20261110-230504-ffb59dfd0ab4-postgres-1
image_ref=postgres@sha256:57c72fd2a128e416c7fcc499958864df5301e940bca0a56f58fddf30ffc07777
image_id=sha256:7e7dbab8d3b431a20793a6d99cb5a6bc84e44914309917f1bf5589a7568cdefd
status=running
started=2026-11-10T23:05:05.738360215Z
health=healthy
project=qos020-integration-20261110-230504-ffb59dfd0ab4
service=postgres
working_dir=/Users/olaoluwatunmise/event-driven-trading-rs/scripts/qos020
config=/Users/olaoluwatunmise/event-driven-trading-rs/scripts/qos020/docker-compose.qos020.yml
run=20261110-230504-ffb59dfd0ab4
provisioning=20261110-230504-ffb59dfd0ab4
mounts=<persistent volume qos020_it_20261110_230504_ffb59dfd0ab4 mounted read-write at /var/lib/postgresql/data; no mount contents read>
networks=<qos020-integration-net-20261110-230504-ffb59dfd0ab4, network ID 4a23a164c583ba8f8bcc33bf2f29ab20066d6df65f490c173fc17d0d1285647b>
exit code: 0
```

The container labels, Compose provenance, image reference and image ID match the manifest and prior recovery report. Current start time is `2026-11-10T23:05:05.738360215Z`; this is container state evidence, not a restart performed by this investigation.

The read-only port inspection returned:

```text
$ docker inspect --format 'ID={{.Id}} status={{.State.Status}} health={{if .State.Health}}{{.State.Health.Status}}{{else}}none{{end}} port_bindings={{json .HostConfig.PortBindings}} effective_ports={{json .NetworkSettings.Ports}} restart={{.HostConfig.RestartPolicy.Name}}' f6f4f45ee58930f5059845a58a023e3f2c2643e1770a0c8ef6f47cb596a0ad82
ID=f6f4f45ee58930f5059845a58a023e3f2c2643e1770a0c8ef6f47cb596a0ad82 status=running health=healthy port_bindings={"5432/tcp":[{"HostIp":"127.0.0.1","HostPort":"51462"}]} effective_ports={"5432/tcp":[{"HostIp":"127.0.0.1","HostPort":"51462"}]} restart=unless-stopped
exit code: 0
```

The endpoint binding is loopback-only and exactly matches the manifest.

## Persistent volume and network verification

Filtered read-only inspections returned:

```text
$ docker volume inspect --format '{{.Name}}|driver={{.Driver}}|scope={{.Scope}}|project={{index .Labels "com.docker.compose.project"}}|run={{index .Labels "qos020.run_id"}}|provisioning={{index .Labels "qos020.provisioning_id"}}' qos020_it_20261110_230504_ffb59dfd0ab4
qos020_it_20261110_230504_ffb59dfd0ab4|driver=local|scope=local|project=qos020-integration-20261110-230504-ffb59dfd0ab4|run=20261110-230504-ffb59dfd0ab4|provisioning=20261110-230504-ffb59dfd0ab4
exit code: 0

$ docker network inspect --format '{{.Name}}|{{.Id}}|driver={{.Driver}}|project={{index .Labels "com.docker.compose.project"}}|run={{index .Labels "qos020.run_id"}}|provisioning={{index .Labels "qos020.provisioning_id"}}|containers={{range $k,$v := .Containers}}{{$k}}:{{$v.Name}};{{end}}' qos020-integration-net-20261110-230504-ffb59dfd0ab4
qos020-integration-net-20261110-230504-ffb59dfd0ab4|4a23a164c583ba8f8bcc33bf2f29ab20066d6df65f490c173fc17d0d1285647b|driver=bridge|project=qos020-integration-20261110-230504-ffb59dfd0ab4|run=20261110-230504-ffb59dfd0ab4|provisioning=20261110-230504-ffb59dfd0ab4|containers=f6f4f45ee58930f5059845a58a023e3f2c2643e1770a0c8ef6f47cb596a0ad82:qos020-integration-20261110-230504-ffb59dfd0ab4-postgres-1
exit code: 0
```

The persistent volume exists, is the exact manifest volume, uses the local driver, and carries the expected QOS-020 project/run/provisioning labels. It is mounted at the PostgreSQL data directory. The network exists with ID `4a23a164c583ba8f8bcc33bf2f29ab20066d6df65f490c173fc17d0d1285647b` and is attached to exactly the manifest-listed container in the inspected output. Volume contents and database rows were not read; resource existence does not establish data integrity or SQL-level health.

## Prior evidence comparison and root cause

The prior recovery report records:

- The same manifest hash and exact non-secret identity fields.
- The same container ID, image reference/image ID, project, service, Compose working directory/file, volume, network ID, labels and loopback port `127.0.0.1:51462`.
- Docker context `desktop-linux`.
- A successful run of the unchanged guard, exit code 0, after Docker access was authorized.
- A failed default-sandbox Docker socket call followed by successful authorized read-only inspections.

The current observations agree with those recorded identities. In the immediately preceding QOS-021A-E3A-R3B-A-I1-R1 attempt, the guard surfaced:

```text
QOS-020 guard rejected command: CalledProcessError: Command '['docker', 'inspect', 'f6f4f45ee58930f5059845a58a023e3f2c2643e1770a0c8ef6f47cb596a0ad82']' returned non-zero exit status 1.
```

That result does not establish that Docker could not find the ID: an inaccessible Docker socket also makes `docker inspect` exit nonzero. The current non-elevated direct call exposed the underlying permission-denied message; the authorized filtered call then found the exact container. **Root cause: sandbox permission boundary on the Docker Desktop socket, not container deletion, manifest mismatch, or context selection.**

One intermediate elevated filtered inspection used an invalid Go template and returned exit code 1 (`template parsing error ... map has no entry for key "Name"`) because the template referenced a nonexistent field in a network-map value. A corrected filtered template succeeded; this was an inspection-command formatting error, not a Docker resource failure.

## Verification limits and residual uncertainty

- The unchanged guard was not rerun in this diagnosis. Its previous successful exit code 0 is recorded in the prior recovery report and its current SHA matches that report. Current Docker identity fields were independently rechecked.
- No SQLx credential value was viewed or emitted. The guard reads that file internally; to honor this task's no-credential-inspection boundary, this diagnosis did not invoke the guard.
- No database connection was made. PostgreSQL protocol responsiveness, database marker, role attributes, applied migrations and SQLx cleanup state were not checked in this run.
- Container health reports `healthy`; its health-check implementation result is Docker evidence, not an independently run SQL query.
- The volume identity and attachment are verified; the contents, backup consistency and restoreability are not.
- The manifest does not record a Docker context field. Current `desktop-linux` and exact resource identity agree with the historical report, but provisioning-time context cannot be proven from the manifest itself.
- No resource was created, started, stopped, restarted, removed, recreated, pruned, renamed, detached or modified. No SQL, migration, or test command was run.

## Recommended next separately authorized action

Authorize a read-only QOS-020 guard verification in an execution context that has access to the Docker Desktop `desktop-linux` socket, with explicit approval for the guard's narrowly scoped SQLx credential read and read-only database identity/cleanup queries. Require the guard to report the same run/project/container/volume/network/endpoint and exit 0 before authorizing guarded SQLx tests. Do not switch to the `default` Docker context, alter the manifest, repair/recreate resources, use staging/production, or run tests outside the unchanged guard.

**No repair or recreation is recommended:** the exact original resources are present. The recovery action is to run verification with appropriate read-only Docker socket permissions, then separately authorize guarded PostgreSQL test work.

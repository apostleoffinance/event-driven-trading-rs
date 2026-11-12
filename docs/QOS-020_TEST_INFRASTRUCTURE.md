# QOS-020 Isolated PostgreSQL Test Infrastructure

QOS-020 provides a disposable PostgreSQL 16 cluster for SQLx integration tests. It is deliberately separate from both repository Compose projects and MT5 staging.

## Local use

Requirements: Docker Desktop with Compose, Python 3, and `psql` on `PATH`. From the repository root:

```sh
python3 scripts/qos020/test_guard.py
python3 scripts/qos020/provision.py
```

Provisioning creates one generated project, network, container, and named volume. The generated manifest and mode-0600 credential files are kept under the system temporary directory in `qos020/<run-id>/`. Credentials and database URLs must not be copied into logs or committed.

Run Rust commands through the guard using the exact manifest, secrets directory, and run ID printed by the provisioner:

```sh
python3 scripts/qos020/qos020_guard.py \
  --manifest <system-temp>/qos020/<run-id>/manifest.json \
  --secrets-dir <system-temp>/qos020/<run-id>/secrets \
  --lane integration-test \
  --run-id <run-id> \
  -- cargo test --workspace
```

The guard verifies the connection to the generated master database, its marker, PostgreSQL major version, the dedicated SQLx role privileges, and the exact Compose container/network/volume identities before it exposes `DATABASE_URL` to the child process. SQLx test database names are created only inside that disposable cluster. Do not run concurrent Cargo processes against one QOS-020 instance.

The bootstrap credential is mounted as a Compose secret and is not granted to Cargo. The SQLx role has `LOGIN`, `CREATEDB`, database connect/create access, and access to the control marker. It has no superuser, role-creation, replication, or RLS-bypass attributes.

## Lifecycle

The provisioner intentionally leaves the instance running. Do not use global Docker cleanup. To remove an instance later, first verify its run ID, project, container, network, and volume against its manifest and Docker labels, then use only the exact project-scoped Compose invocation after separate authorization. Do not use `--volumes` without confirming the exact generated volume identity.

GitHub Actions provisions the same dedicated Compose topology inside an isolated job and leaves Python provider tests independent of PostgreSQL.

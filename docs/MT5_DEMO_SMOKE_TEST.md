# MT5 Demo Smoke Test — QOS-015A / QOS-015B

**Purpose:** produce real-host evidence that the read-only Windows MT5 worker authenticates to the Rust daemon, reconciles account/orders/positions, and persists fresh instrument metadata and quotes. This is not an execution test and does not authorize live trading.

## Preconditions

- Run on the target Windows VPS that hosts the intended MetaTrader 5 terminal.
- Use a broker demo account only; confirm the terminal is logged into the intended demo server.
- Apply repository PostgreSQL migrations using the normal deployment process. The daemon does not run migrations.
- Install Rust, Python 3.12, PostgreSQL client tools (`psql`), and the adapter's terminal extra (`pip install -e ".[terminal]"`).
- Start `mt5-bridge-daemon` and the read-only worker in separate PowerShell sessions using the environment variables in [the adapter README](../adapters/mt5-python/README.md).
- Configure `MT5_SYMBOLS` with exact broker symbols; do not normalize suffixes or guess symbols.
- Set `MT5_EXPECTED_ACCOUNT_CURRENCY` for the preflight check.

## Run

In a third PowerShell session on the same host:

```powershell
.\adapters\mt5-python\scripts\mt5-demo-smoke.ps1
```

Optional timeout/poll settings:

```powershell
.\adapters\mt5-python\scripts\mt5-demo-smoke.ps1 -TimeoutSeconds 240 -PollSeconds 10
```

The harness checks:
1. Required environment values and certificate files exist; the worker leaf-certificate DER SHA-256 fingerprint matches the daemon's configured pin.
2. The configured MT5 terminal initializes with the expected demo login and server; account currency matches the operator-supplied expected value.
3. Every configured exact symbol (already selected by the running worker) has fresh bid/ask and valid tick size/value, contract size, and volume constraints. The smoke probe does not call `symbol_select` or modify Market Watch.
4. The daemon's configured TCP listener is reachable and PostgreSQL has both market-data and reconciliation snapshot tables.
5. The database contains a fresh market-data snapshot and a fresh reconciliation snapshot for the exact account/terminal; server, account currency, symbols, bid/ask validity, quote freshness, and at least two distinct market-data sequences are checked.

A passing run proves only the checks above for that host and time. Preserve the console output and timestamps as release evidence. Never record the MT5 password, private keys, full database connection string, or unredacted broker diagnostics.

## Required follow-up: reconnect and credential lifecycle

After the first pass:
1. Stop the Python worker cleanly, leave the daemon running, and verify the daemon logs show the connection was discarded/closed.
2. Restart the worker with the same sequence checkpoint file. Run the harness again and confirm fresh market data and reconciliation return without manually clearing database state or resetting the sequence file.
3. Review filesystem ACLs on server/client private keys: only the relevant service identity and approved administrators should be able to read them.
4. Record certificate subject/issuer, expiry, pinned leaf fingerprint, revocation method, rotation owner, and the tested overlap/roll-forward/rollback procedure in the deployment's protected operations record. Do not put private key material in this repository.
5. Verify an untrusted or wrong-fingerprint client is rejected using the existing TLS integration tests; do not test credential failure by weakening TLS validation.

## Acceptance / stop conditions

- [ ] Correct demo account/server/currency and terminal identity confirmed.
- [ ] Exact configured symbols, metadata, and fresh ticks validated.
- [ ] Market-data and reconciliation snapshots are both fresh and persisted for the same account/terminal.
- [ ] Market-data sequence advances across repeated polls.
- [ ] Worker restart recovers through a fresh authenticated connection and reconciliation.
- [ ] Certificate pin, expiry/revocation and private-key ACL review recorded.
- [ ] Output and logs are redacted and attached to the change/release review.

Any identity mismatch, missing symbol, stale quote, absent snapshot, sequence regression, TLS failure, or inability to verify the certificate/key ACLs blocks the release gate. No order submission/cancellation is part of this test; live execution remains deferred.

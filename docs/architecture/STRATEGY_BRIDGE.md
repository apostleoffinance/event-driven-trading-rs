# Strategy Bridge (Python → Rust)

**Strategies are Python. Execution and control plane are Rust.**

```text
strategies/python/          → TradeIntent NDJSON
crates/intent-bridge/       → parse + TradingRuntime pipeline
crates/{risk,execution,venue,persistence,recon}/
```

## Owns (Python)

- Signal generation / research-friendly strategy logic
- Emitting validated `TradeIntent` JSON (no orders, no account sizing)

## Owns (Rust `intent-bridge`)

- Deserializing the wire format into domain `TradeIntent`
- Feeding intents into risk → OMS → venue
- CLI for stdin / file NDJSON ingest

## Must NOT (Python)

- Submit orders
- Call venues
- Enforce account risk limits / position sizing from equity

## Wire format

NDJSON: one JSON object per line. Decimals are **strings**. See
`docs/contracts/trade_intent.schema.json`.

Example:

```json
{"id":"ti-abc","strategy_id":"btc-mean-reversion","strategy_version":"v1","deployment_id":"deployment-001","instrument_id":"BTCUSDT","side":"Buy","entry_price":"90","stop_loss":"88.2","timestamp":"2026-03-06T12:00:00Z"}
```

## Run

```bash
# Python unit tests
cd strategies/python
python3 -m venv .venv && source .venv/bin/activate && pip install -e .
python -m unittest discover -s tests -v

# End-to-end: Python intents → Rust prop venue
python -m trading_strategies.mean_reversion --prices 100,100,100,90 \
  | cargo run -p intent-bridge -- --venue prop
```

## Legacy

`strategies/mean-reversion` (Rust) remains as a reference / Phase 3 artifact.
New strategy work should land under `strategies/python/`.

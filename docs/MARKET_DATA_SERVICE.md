# EdgeNest Market-Data Service

Read-only market data for the `/markets` workspace (candle history) plus one read-only MT5 latest-quote snapshot endpoint. Crate: `crates/market-data` (library + binary `market-data-api`).

## Architecture decision

The workspace had **no HTTP server** (only the TCP/mTLS listener in `intent-bridge`) and no candle abstraction, so there was nothing to extend. A new read-only crate was added rather than putting an HTTP framework into the trading path:

- axum 0.8 (HTTP), reqwest 0.12 with rustls (Binance public REST), tower-http (CORS). No Redis/Kafka/ClickHouse. The only database access is the optional, read-only, scoped MT5 latest-quote read (see below); it is off unless configured.
- **Dependency coupling remains:** to reuse the scoped `load_latest` query, the crate links `persistence`, which in turn depends on `domain` and `execution-engine`. Those crates are linked into the binary but no trading or order code is called. Decoupling (for example a small read-only snapshot crate) is an open decision.
- It holds no credentials unless the optional MT5 database URL is set, and exposes only `GET` routes.
- Binds `127.0.0.1:8787` by default.

```
MarketChart → frontend client → GET /api/v1/market-data/candles
            → MarketDataService → provider adapters: demo | binance-public | mt5-bridge | biquote-live

/mt5 (frontend, not yet wired) → GET /api/v1/market-data/mt5/latest
            → Mt5LatestService → read-only PostgreSQL snapshot (optional)
```

## API

`GET /api/v1/market-data/candles`

| Param | Required | Notes |
|---|---|---|
| `instrument` | yes | Canonical id, e.g. `EURUSD`, `BTCUSDT`, `BTCUSDT-PERP` (case-insensitive) |
| `timeframe` | yes | `M1 M5 M15 M30 H1 H4 D1 W1` (no default) |
| `from`, `to` | no | Epoch ms or RFC 3339; `from < to` |
| `limit` | no | 1–1000, default 300; out-of-range is rejected, not clamped |
| `provider` | no | `demo`, `binance-public`, `mt5-bridge`, `biquote-live`. Omitted = the instrument's configured default provider. **Demo is never a default.** |

Response (200): `instrument` (metadata), `provider {id,name,status,provider_symbol}`, `timeframe`, `classification` (`demo`|`live`), `freshness`, `count`, `last_candle_timestamp_ms`, `last_candle_at`, `last_updated_at`, `generated_at_ms`, `candles[]`.

- Timestamps are UTC. `timestamp_ms` is the candle **opening** time; the newest candle may still be forming.
- Candles are oldest-first, strictly ascending, unique, and validated (`high ≥ low/open/close`, `low ≤ open/close`, finite positive prices, volume ≥ 0). Invalid provider data is **rejected** (`invalid_provider_data`), never repaired.
- An empty result is `200` with `candles: []` and `freshness: unknown`.
- Prices are `f64`: fine for charts, not for accounting.
- Freshness: `demo`; `fresh` (newest open within 2× timeframe of now); `stale`; `historical` (`to` is in the past); `unknown` (empty). Closed markets (e.g. forex weekends) may read as `stale`.

Errors (all `classification: "unavailable"`, no candles): `{"error":{code,message,classification,provider?,retryable,retry_after_seconds?}}`

| Code | HTTP | Meaning |
|---|---|---|
| `invalid_request` | 400 | Bad/missing parameter |
| `unknown_instrument` | 404 | Not in the catalogue |
| `unsupported_by_provider` | 422 | e.g. perpetual on Binance spot |
| `provider_disabled` | 503 | Provider switched off by configuration |
| `capability_not_implemented` | 501 | MT5 candles |
| `provider_unavailable` | 503 | Network/region failure (retryable) |
| `rate_limited` | 429 | Includes `Retry-After` |
| `provider_timeout` | 504 | Retryable |
| `invalid_provider_data` | 502 | Malformed or invalid upstream data |
| `upstream_error` | 502 | Other upstream failure |

Also: `GET /api/v1/market-data/providers`, `GET /api/v1/market-data/instruments`, `GET /healthz`. Responses are `Cache-Control: no-store`.

Instrument identity is separate from provider aliases (`instruments.rs`): `BTCUSDT-PERP` has **no** Binance spot alias, so it is unsupported rather than silently served from spot.

## BiQuote public live candles

`biquote-live` is a read-only external live market feed for charting FX and crypto OHLC candles. It is enabled by default (`MARKET_DATA_BIQUOTE_ENABLED=true`) and can be switched off explicitly. It is an external market-data feed, not the trader's broker quote; broker spreads, execution prices, and venue-specific prices may differ. Do not use this feed for order execution, position sizing, accounting, or broker reconciliation. It is not a substitute for broker-specific MT5 candles. The frontend refreshes the current chart series every 15 seconds while the page is active. Weekly candles are aggregated from daily OHLC data.

## Configuration (environment)

| Variable | Default |
|---|---|
| `MARKET_DATA_BIND` | `127.0.0.1:8787` |
| `MARKET_DATA_BINANCE_ENABLED` | `true` (public spot candles; can be disabled) |
| `MARKET_DATA_BIQUOTE_ENABLED` | `true` (public external live feed; can be disabled) |
| `MARKET_DATA_BIQUOTE_BASE_URL` | `https://biquote.io` |
| `MARKET_DATA_BINANCE_BASE_URL` | `https://api.binance.com` |
| `MARKET_DATA_REQUEST_TIMEOUT_MS` | `5000` (100–30000) |
| `MARKET_DATA_CORS_ORIGINS` | `http://localhost:3000,http://localhost:3107` |
| `MARKET_DATA_DATABASE_URL` | unset. Optional `postgres://` URL for a dedicated **read-only** role. Never the bridge daemon's credentials. Non-loopback hosts must set `sslmode=require`, `verify-ca` or `verify-full` (see TLS below). |
| `MARKET_DATA_MT5_ACCOUNT_ID` | unset. Server-side scope; required for the MT5 endpoint. |
| `MARKET_DATA_MT5_TERMINAL_ID` | unset. Server-side scope; required for the MT5 endpoint. |
| `MARKET_DATA_MT5_STALE_AFTER_SECONDS` | `60` (1–3600). Product default, configurable. |

Invalid values fail startup (error messages never echo the value), including a database URL that is not valid or that targets a non-loopback host without TLS. Missing or partial MT5 settings do **not** fail startup; the MT5 endpoint then answers 503.

## MT5 latest quote: `GET /api/v1/market-data/mt5/latest`

Returns the latest persisted **EURUSD** quote from `mt5_bridge_market_data_snapshots` for the single configured account/terminal. The table holds one upserted latest snapshot per account/terminal: this is **snapshot-only data, not historical ticks or candles**, and it does not make MT5 chart history available. No query parameters are accepted (any query string returns 400); scope is server-side only.

```json
{
  "status": "fresh",
  "classification": "live",
  "symbol": "EURUSD",
  "bid": "1.12115",
  "ask": "1.12126",
  "spread": "0.00011",
  "tick_time": "2026-11-02T10:00:00Z",
  "captured_at": "2026-11-02T10:00:05Z",
  "sequence": 5978,
  "quote_age_seconds": 5.0,
  "snapshot_age_seconds": 5.0,
  "is_stale": false,
  "stale_after_seconds": 60
}
```

(Illustrative values only.) `bid`/`ask`/`spread` are decimal strings; timestamps are UTC RFC 3339. `classification: "live"` only means the data came from the configured MT5 pipeline (never mock/demo); it does not imply freshness, so a `live` response can have `status: "stale"`.

- `quote_age_seconds = now - tick_time` (age of the broker quote); `snapshot_age_seconds = now - captured_at` (age of the last persistence write). They are independent JSON **numbers that may be fractional** (millisecond resolution). `status`/`is_stale` use **quote age** only (stale when strictly older than `stale_after_seconds`); a recent snapshot never makes an old quote fresh. Stale means the last received quote is old (for example the forex market is closed), not necessarily that MT5 or the database is disconnected. The 60 s default is a configurable product choice, not a verified requirement.
- **Negative ages:** a timestamp up to 5 s ahead of the server clock is accepted and the age is returned as-is (not clamped), so it can be slightly negative. This represents clock skew between the broker/worker and this host. Beyond 5 s ahead the record is rejected (502). Clients must tolerate small negative values.
- Validation: symbol is exactly `EURUSD`; bid and ask are JSON strings in plain `digits` or `digits.digits` form (at most 10 integer and 16 fractional digits, matching the MT5 EA's 16-digit serialization; no sign, exponent, `.5`, `5.` or separators), positive, and ask >= bid (the decimal text is returned with its stored scale, so no float rounding); `tick_time` is UTC RFC 3339 and not more than 5 s in the future. Missing or malformed fields are rejected, never clamped or substituted. There is no mock fallback.
- Errors use the standard envelope with generic messages (no SQL, DSN, database names or identifiers): no snapshot / missing config / database unavailable -> 503; query timeout -> 504; invalid stored record -> 502. There is no generic 500 path: unexpected failures are mapped to 503 or 502. Known limitation: only the repository's own integrity failures (identity/sequence mismatch) and invalid quote content are reported as 502 "corrupt"; connection, pool and row-decode errors (including a corrupt column type) are all reported as 503 "unavailable", so the two cannot always be told apart from the response.
- Read-only: the endpoint only runs the existing scoped `load_latest` SELECT (`WHERE account_id = $1 AND terminal_id = $2`). It uses its own pool, separate from the shared `persistence::connect` pool: lazy (no connection at startup), at most 2 connections, 3 s acquire timeout, 60 s idle timeout, and per-session `statement_timeout=3000` and `default_transaction_read_only=on`. At most two connection attempts run at once; further requests wait on the pool and fail after the acquire timeout, so an unreachable database does not cause a connection stampede. There is no negative-result backoff beyond that. (`default_transaction_read_only`/`statement_timeout` are sent as startup options; some connection poolers, such as PgBouncer, reject them.)
- **Database grants (not provisioned by this change):** use a dedicated login role, not the bridge daemon's. The existing repository query reads the columns `account_id`, `terminal_id`, `server`, `sequence`, `source`, `account_currency`, `captured_at` and `snapshot`, so the role needs `CONNECT` on the database, `USAGE` on the schema, and `SELECT` on at least those columns of `mt5_bridge_market_data_snapshots` (simplest: table-level `SELECT`). No other privileges. The identity columns are read for the repository's integrity check and are never returned in the response.
- **TLS:** the URL's `sslmode` is the only control. Startup is refused for any host other than `localhost`, `127.0.0.1`, `::1` or a Unix socket unless `sslmode` is `require`, `verify-ca` or `verify-full` (the sqlx default `prefer`, plus `allow` and `disable`, are rejected). `require` encrypts but does not authenticate the server; use `verify-full` with a trusted CA wherever the certificate setup allows. Loopback/Unix-socket hosts are not forced to TLS. Certificate roots and any `sslrootcert` setting are deployment configuration and are not verified by this repository.
- Keep the default loopback bind (`127.0.0.1:8787`); no auth is added by this change. The browser never talks to PostgreSQL or the bridge daemon.
- When `MARKET_DATA_DATABASE_URL` or either scope variable is missing, the endpoint returns 503 and all other routes and `/healthz` are unaffected.

## Provider status

| Provider | Status |
|---|---|
| `demo` | Works; deterministic synthetic candles; classification `demo`. |
| `binance-public` | Implemented against Binance's documented `/api/v3/klines` format; **disabled by default**; covered by fixture tests plus a loopback timeout test. **Not verified against the real exchange from this repository.** |
| `mt5-bridge` | **Candles not implemented** (see below). The separate latest-quote snapshot endpoint is implemented and has not been compiled, tested or run against a database yet. |

## MT5 assessment

The EA, Python worker and daemon support only `PING, ACCOUNT, ORDERS, POSITIONS, SELECT_SYMBOL, SYMBOL_INFO, TICK`. There is no candle/rates command, so MT5 candle history cannot be served. Required work (not done; nothing invented here):

1. A read-only `RATES` command in the MQL5 EA (`CopyRates`, with symbol, timeframe, from/count and a hard cap).
2. Worker handling and validation in the Python bridge.
3. A matching read-only request in the Rust daemon protocol.
4. A real `Mt5Provider` that maps it to canonical candles.

Until an end-to-end path is verified, MT5 **chart/candle** data must not be labelled live. This does not apply to the latest-quote snapshot endpoint above, which is sourced from the persisted MT5 pipeline and reports `classification: "live"` independently of freshness.

## Limitations

- No response cache or WebSocket/SSE: the frontend polls, so many tabs pass through to Binance. Rate limits are mapped to `429`, not prevented.
- Live bid/ask is only available as the single latest EURUSD snapshot from `GET /api/v1/market-data/mt5/latest` (no history, no other symbols, no streaming); the candle endpoint has no live bid/ask.
- Binance perpetuals are unsupported.
- `limit` caps one request (≤1000); no pagination across a long history yet.

# QuantOS Journal — TraderOS parity upgrade

## Runtime configuration

The Journal API requires an S3-compatible private object store for direct screenshot uploads:

- `JOURNAL_S3_ENDPOINT_URL`: S3-compatible endpoint (for Cloudflare R2, the account's R2 S3 API endpoint).
- `JOURNAL_S3_ACCESS_KEY_ID`: least-privilege object-storage access key.
- `JOURNAL_S3_SECRET_ACCESS_KEY`: secret for that key.
- `JOURNAL_S3_BUCKET`: private bucket name.
- `JOURNAL_S3_REGION`: region, normally `auto` for R2.

Do not commit credentials or put them in browser-exposed environment variables. Configure these values in the Journal API's secret environment. The API returns short-lived presigned URLs (15 minutes); the browser never receives storage credentials.

### Bucket CORS

Configure the bucket to allow the QuantOS frontend origin(s) to perform:
- Methods: `PUT`, `GET`, `HEAD`
- Allowed headers: `Content-Type`
- Exposed headers: `ETag`
- Max age: 3600 seconds

The browser uploads directly to the private bucket using a presigned PUT URL. The API verifies the uploaded object's existence, content type and actual size before writing attachment metadata. Downloads use owner-authorized presigned GET URLs. Deletion removes the object before deleting its metadata.

## Calculation policy

### FX P&L

For recognized six-character ISO currency pairs, standard-lot quantity and a matching quote/P&L currency:

`gross = (exit - entry) × direction × lots × 100,000`

Long direction is +1; short direction is -1. Gross P&L is not guessed when quote-currency conversion is needed or the instrument is not a recognized FX pair.

`net = gross - abs(commission) + swap - abs(other_fees)`

Commission and other fees are treated as costs whether entered signed or unsigned. Swap remains signed because it may be a credit or debit. Imported and broker-sourced values are preserved as source facts; manually entered trades prefer derived values.

Stop-based risk is derived as:

`risk = abs(entry - stop_loss) × lots × 100,000`

R-multiple is net P&L divided by positive risk. This standard-lot formula is intentionally limited to recognized FX pairs. CFDs, metals, indices, crypto, non-standard broker contract sizes and account-currency conversions must use broker/instrument metadata before automatic P&L can be enabled for those instruments.

### Sessions

The session is classified at the trade's UTC open timestamp using IANA timezone data for Europe/London and America/New_York, so daylight-saving changes are applied. The London and New York windows are 08:00–17:00 local time. Non-FX instruments are not assigned an FX session.

### MFE and MAE

Closed trades automatically request historical candles when opened in Journal details. Requests are paged in windows of at most 900 bars to stay within the market-data service's 1,000-candle request cap. The calculation stores the most favorable and adverse observed prices, R multiples relative to stop distance (when available), provider, timeframe, candle count and coverage window.

MFE/MAE are estimates bounded by the selected provider's OHLC timeframe and available historical coverage. A candle's high/low does not reveal the intrabar path or exact fill ordering. The UI must not present them as tick-accurate. No synthetic demo candles are used for persisted excursion calculations. If the returned candles do not cover the entry-to-exit interval within the timeframe tolerance, metrics are not persisted.

## Validation

Run from the repository root:

- `cargo fmt --all -- --check`
- `cargo test -p journal-api`
- `cargo test --workspace`
- `npm --prefix frontend run lint` (if configured)
- `npm --prefix frontend run build`

Review CI on the feature branch before merging. Database migrations are forward-only; existing imported journal values and attachment records are retained.

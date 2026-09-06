-- Constitution hardening: InstrumentSpec + Unknown-ready (status is text).

ALTER TABLE instruments
    ADD COLUMN IF NOT EXISTS tick_size NUMERIC(38, 18),
    ADD COLUMN IF NOT EXISTS lot_size NUMERIC(38, 18),
    ADD COLUMN IF NOT EXISTS min_quantity NUMERIC(38, 18),
    ADD COLUMN IF NOT EXISTS min_notional NUMERIC(38, 18),
    ADD COLUMN IF NOT EXISTS price_precision INTEGER,
    ADD COLUMN IF NOT EXISTS quantity_precision INTEGER;

-- Backfill defaults for existing crypto rows (paper-friendly).
UPDATE instruments
SET
    tick_size = COALESCE(tick_size, 0.01),
    lot_size = COALESCE(lot_size, 0.00000001),
    min_quantity = COALESCE(min_quantity, 0.00000001),
    min_notional = COALESCE(min_notional, 5),
    price_precision = COALESCE(price_precision, 8),
    quantity_precision = COALESCE(quantity_precision, 8)
WHERE tick_size IS NULL;

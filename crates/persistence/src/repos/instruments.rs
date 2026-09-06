use domain::{Instrument, InstrumentType};
use sqlx::PgPool;

use crate::error::PersistenceResult;

pub async fn upsert(pool: &PgPool, instrument: &Instrument) -> PersistenceResult<()> {
    let instrument_type = match instrument.instrument_type {
        InstrumentType::Crypto => "crypto",
        InstrumentType::Fx => "fx",
        InstrumentType::Equity => "equity",
        InstrumentType::Future => "future",
        InstrumentType::Option => "option",
        InstrumentType::Rate => "rate",
        InstrumentType::Credit => "credit",
    };
    sqlx::query(
        r#"
        INSERT INTO instruments (
            id, instrument_type, symbol, base_asset, quote_asset, enabled,
            tick_size, lot_size, min_quantity, min_notional,
            price_precision, quantity_precision
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)
        ON CONFLICT (id) DO UPDATE SET
            instrument_type = EXCLUDED.instrument_type,
            symbol = EXCLUDED.symbol,
            base_asset = EXCLUDED.base_asset,
            quote_asset = EXCLUDED.quote_asset,
            enabled = EXCLUDED.enabled,
            tick_size = EXCLUDED.tick_size,
            lot_size = EXCLUDED.lot_size,
            min_quantity = EXCLUDED.min_quantity,
            min_notional = EXCLUDED.min_notional,
            price_precision = EXCLUDED.price_precision,
            quantity_precision = EXCLUDED.quantity_precision
        "#,
    )
    .bind(instrument.id.as_str())
    .bind(instrument_type)
    .bind(&instrument.symbol)
    .bind(instrument.base.as_deref())
    .bind(instrument.quote.as_deref())
    .bind(instrument.enabled)
    .bind(instrument.spec.tick_size)
    .bind(instrument.spec.lot_size)
    .bind(instrument.spec.min_quantity)
    .bind(instrument.spec.min_notional)
    .bind(instrument.spec.price_precision as i32)
    .bind(instrument.spec.quantity_precision as i32)
    .execute(pool)
    .await?;
    Ok(())
}

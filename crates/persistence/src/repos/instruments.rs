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
        INSERT INTO instruments (id, instrument_type, symbol, base_asset, quote_asset, enabled)
        VALUES ($1, $2, $3, $4, $5, $6)
        ON CONFLICT (id) DO UPDATE SET
            instrument_type = EXCLUDED.instrument_type,
            symbol = EXCLUDED.symbol,
            base_asset = EXCLUDED.base_asset,
            quote_asset = EXCLUDED.quote_asset,
            enabled = EXCLUDED.enabled
        "#,
    )
    .bind(instrument.id.as_str())
    .bind(instrument_type)
    .bind(&instrument.symbol)
    .bind(instrument.base.as_deref())
    .bind(instrument.quote.as_deref())
    .bind(instrument.enabled)
    .execute(pool)
    .await?;
    Ok(())
}

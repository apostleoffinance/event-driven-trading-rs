//! Strict adapter from persisted MT5 market data to instrument-aware sizing inputs.
//! This module performs no I/O and never submits orders. Missing/stale FX data fails closed.

use chrono::{DateTime, Duration, Utc};
use rust_decimal::Decimal;
use serde_json::Value;
use std::str::FromStr;

use crate::instrument_sizing::{InstrumentSizingContext, LossModel};

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum Mt5SizingDataError {
    #[error("MT5 market-data snapshot is malformed or incomplete")]
    Malformed,
    #[error("MT5 snapshot or quote is stale or from the future")]
    Stale,
    #[error("account currency does not match authenticated snapshot")]
    AccountCurrencyMismatch,
    #[error("target instrument is absent")]
    InstrumentUnavailable,
    #[error("fresh FX quotes do not provide a conversion path to account currency")]
    FxConversionUnavailable,
    #[error("explicit cost or margin inputs are invalid")]
    InvalidSupplement,
    #[error("decimal parsing or arithmetic failed")]
    Decimal,
}

/// Costs and margin are not in the MT5 market-data frame and must be supplied by a trusted service.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Mt5SizingSupplement {
    pub estimated_cost_per_quantity: Decimal,
    pub slippage_gap_buffer_per_quantity: Decimal,
    pub pending_order_exposure: Decimal,
    pub available_margin: Decimal,
    pub pending_order_margin: Decimal,
    pub margin_per_quantity: Decimal,
    pub min_notional_account_currency: Option<Decimal>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreparedMt5Sizing {
    pub entry_price: Decimal,
    pub context: InstrumentSizingContext,
    pub snapshot_captured_at: DateTime<Utc>,
    pub quote_time: DateTime<Utc>,
    pub source: String,
    pub sequence: u64,
}

#[derive(Debug, Clone)]
struct Instrument {
    symbol: String,
    base: String,
    quote: String,
    contract: Decimal,
    tick: Decimal,
    tick_value: Decimal,
    min: Decimal,
    max: Decimal,
    step: Decimal,
}

#[derive(Debug, Clone)]
struct Quote {
    symbol: String,
    bid: Decimal,
    ask: Decimal,
    time: DateTime<Utc>,
}

fn text<'a>(value: &'a Value, key: &str) -> Result<&'a str, Mt5SizingDataError> {
    value
        .get(key)
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty() && value.trim() == *value)
        .ok_or(Mt5SizingDataError::Malformed)
}

fn decimal(value: &Value) -> Result<Decimal, Mt5SizingDataError> {
    value
        .as_str()
        .and_then(|value| Decimal::from_str(value).ok())
        .filter(|value| *value > Decimal::ZERO)
        .ok_or(Mt5SizingDataError::Malformed)
}

fn timestamp(value: &Value, key: &str) -> Result<DateTime<Utc>, Mt5SizingDataError> {
    DateTime::parse_from_rfc3339(text(value, key)?)
        .map(|value| value.with_timezone(&Utc))
        .map_err(|_| Mt5SizingDataError::Malformed)
}

fn fresh(time: &DateTime<Utc>, now: &DateTime<Utc>, max_age: Duration) -> bool {
    max_age >= Duration::zero()
        && time <= now
        && now.clone().signed_duration_since(time.clone()) <= max_age
}

fn parse_instruments(snapshot: &Value) -> Result<Vec<Instrument>, Mt5SizingDataError> {
    let rows = snapshot
        .get("instruments")
        .and_then(Value::as_array)
        .filter(|rows| !rows.is_empty() && rows.len() <= 64)
        .ok_or(Mt5SizingDataError::Malformed)?;
    let mut result = Vec::new();
    for value in rows {
        let instrument = Instrument {
            symbol: text(value, "symbol")?.into(),
            base: text(value, "base_currency")?.into(),
            quote: text(value, "quote_currency")?.into(),
            contract: decimal(value.get("contract_size").ok_or(Mt5SizingDataError::Malformed)?)?,
            tick: decimal(value.get("tick_size").ok_or(Mt5SizingDataError::Malformed)?)?,
            tick_value: decimal(
                value
                    .get("tick_value_account_currency")
                    .ok_or(Mt5SizingDataError::Malformed)?,
            )?,
            min: decimal(value.get("volume_min").ok_or(Mt5SizingDataError::Malformed)?)?,
            max: decimal(value.get("volume_max").ok_or(Mt5SizingDataError::Malformed)?)?,
            step: decimal(value.get("volume_step").ok_or(Mt5SizingDataError::Malformed)?)?,
        };
        if instrument.max < instrument.min
            || instrument
                .min
                .checked_div(instrument.step)
                .is_none_or(|ratio| ratio != ratio.trunc())
            || result
                .iter()
                .any(|existing: &Instrument| existing.symbol == instrument.symbol)
        {
            return Err(Mt5SizingDataError::Malformed);
        }
        result.push(instrument);
    }
    Ok(result)
}

fn parse_quotes(
    snapshot: &Value,
    now: DateTime<Utc>,
    max_age: Duration,
) -> Result<Vec<Quote>, Mt5SizingDataError> {
    let rows = snapshot
        .get("quotes")
        .and_then(Value::as_array)
        .filter(|rows| !rows.is_empty() && rows.len() <= 64)
        .ok_or(Mt5SizingDataError::Malformed)?;
    let mut result = Vec::new();
    for value in rows {
        let quote = Quote {
            symbol: text(value, "symbol")?.into(),
            bid: decimal(value.get("bid").ok_or(Mt5SizingDataError::Malformed)?)?,
            ask: decimal(value.get("ask").ok_or(Mt5SizingDataError::Malformed)?)?,
            time: timestamp(value, "tick_time")?,
        };
        if quote.ask < quote.bid {
            return Err(Mt5SizingDataError::Malformed);
        }
        if !fresh(&quote.time, &now, max_age) {
            return Err(Mt5SizingDataError::Stale);
        }
        if result
            .iter()
            .any(|existing: &Quote| existing.symbol == quote.symbol)
        {
            return Err(Mt5SizingDataError::Malformed);
        }
        result.push(quote);
    }
    Ok(result)
}

/// Converts currency with up to three fresh FX edges. Uses bid for direct and 1/ask for inverse.
fn fx_rate(
    from: &str,
    to: &str,
    instruments: &[Instrument],
    quotes: &[Quote],
) -> Result<Decimal, Mt5SizingDataError> {
    if from == to {
        return Ok(Decimal::ONE);
    }
    let mut frontier = vec![(from.to_owned(), Decimal::ONE, 0_usize)];
    let mut visited = vec![from.to_owned()];
    while let Some((currency, rate, depth)) = frontier.pop() {
        if depth >= 3 {
            continue;
        }
        for instrument in instruments {
            let Some(quote) = quotes.iter().find(|quote| quote.symbol == instrument.symbol) else {
                continue;
            };
            let edge = if instrument.base == currency {
                Some((instrument.quote.as_str(), quote.bid))
            } else if instrument.quote == currency {
                Some((
                    instrument.base.as_str(),
                    Decimal::ONE
                        .checked_div(quote.ask)
                        .ok_or(Mt5SizingDataError::Decimal)?,
                ))
            } else {
                None
            };
            if let Some((next, edge_rate)) = edge {
                if edge_rate <= Decimal::ZERO || visited.iter().any(|seen| seen.as_str() == next) {
                    continue;
                }
                let combined = rate
                    .checked_mul(edge_rate)
                    .ok_or(Mt5SizingDataError::Decimal)?;
                if next == to {
                    return Ok(combined);
                }
                visited.push(next.into());
                frontier.push((next.into(), combined, depth + 1));
            }
        }
    }
    Err(Mt5SizingDataError::FxConversionUnavailable)
}

/// Build sizing inputs using ask for buys, bid for sells, tick value in account currency,
/// broker volume limits, and a fresh conversion path from quote currency to account currency.
pub fn prepare_mt5_sizing(
    snapshot: &Value,
    symbol: &str,
    account_currency: &str,
    is_buy: bool,
    now: DateTime<Utc>,
    max_age: Duration,
    supplement: &Mt5SizingSupplement,
) -> Result<PreparedMt5Sizing, Mt5SizingDataError> {
    if symbol.trim().is_empty()
        || account_currency.trim().is_empty()
        || max_age < Duration::zero()
    {
        return Err(Mt5SizingDataError::Malformed);
    }
    if text(snapshot, "account_currency")? != account_currency {
        return Err(Mt5SizingDataError::AccountCurrencyMismatch);
    }
    let captured_at = timestamp(snapshot, "captured_at")?;
    if !fresh(&captured_at, &now, max_age) {
        return Err(Mt5SizingDataError::Stale);
    }
    if supplement.estimated_cost_per_quantity < Decimal::ZERO
        || supplement.slippage_gap_buffer_per_quantity < Decimal::ZERO
        || supplement.pending_order_exposure < Decimal::ZERO
        || supplement.available_margin < Decimal::ZERO
        || supplement.pending_order_margin < Decimal::ZERO
        || supplement.margin_per_quantity <= Decimal::ZERO
        || supplement
            .min_notional_account_currency
            .is_some_and(|value| value < Decimal::ZERO)
    {
        return Err(Mt5SizingDataError::InvalidSupplement);
    }

    let instruments = parse_instruments(snapshot)?;
    let quotes = parse_quotes(snapshot, now, max_age)?;
    if instruments.len() != quotes.len()
        || instruments
            .iter()
            .any(|instrument| !quotes.iter().any(|quote| quote.symbol == instrument.symbol))
    {
        return Err(Mt5SizingDataError::Malformed);
    }
    let instrument = instruments
        .iter()
        .find(|instrument| instrument.symbol == symbol)
        .ok_or(Mt5SizingDataError::InstrumentUnavailable)?;
    let quote = quotes
        .iter()
        .find(|quote| quote.symbol == symbol)
        .ok_or(Mt5SizingDataError::InstrumentUnavailable)?;
    let conversion = fx_rate(&instrument.quote, account_currency, &instruments, &quotes)?;
    let notional_rate = instrument
        .contract
        .checked_mul(conversion)
        .ok_or(Mt5SizingDataError::Decimal)?;

    Ok(PreparedMt5Sizing {
        entry_price: if is_buy { quote.ask } else { quote.bid },
        context: InstrumentSizingContext {
            loss_model: LossModel::TickValue {
                tick_size: instrument.tick,
                tick_value_per_quantity: instrument.tick_value,
                tick_value_to_account_rate: Decimal::ONE,
            },
            inputs_fresh: true,
            notional_to_account_rate: notional_rate,
            quantity_step: instrument.step,
            min_quantity: instrument.min,
            max_quantity: Some(instrument.max),
            min_notional_account_currency: supplement.min_notional_account_currency,
            estimated_cost_per_quantity: supplement.estimated_cost_per_quantity,
            slippage_gap_buffer_per_quantity: supplement.slippage_gap_buffer_per_quantity,
            pending_order_exposure: supplement.pending_order_exposure,
            available_margin: Some(supplement.available_margin),
            pending_order_margin: supplement.pending_order_margin,
            margin_per_quantity: Some(supplement.margin_per_quantity),
        },
        snapshot_captured_at: captured_at,
        quote_time: quote.time,
        source: text(snapshot, "source")?.into(),
        sequence: snapshot
            .get("sequence")
            .and_then(Value::as_u64)
            .ok_or(Mt5SizingDataError::Malformed)?,
    })
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::panic)]
    use super::*;
    use serde_json::json;

    fn snapshot(now: DateTime<Utc>) -> Value {
        json!({
            "captured_at": now.to_rfc3339(),
            "account_currency": "USD",
            "source": "mt5_python_readonly_v1",
            "sequence": 8,
            "instruments": [
                {"symbol":"EURUSD.a","base_currency":"EUR","quote_currency":"USD","contract_size":"100000","tick_size":"0.00001","tick_value_account_currency":"1.00","volume_min":"0.01","volume_max":"100.00","volume_step":"0.01"},
                {"symbol":"EURGBP.a","base_currency":"EUR","quote_currency":"GBP","contract_size":"100000","tick_size":"0.00001","tick_value_account_currency":"1.20","volume_min":"0.01","volume_max":"100.00","volume_step":"0.01"},
                {"symbol":"GBPUSD.a","base_currency":"GBP","quote_currency":"USD","contract_size":"100000","tick_size":"0.00001","tick_value_account_currency":"1.50","volume_min":"0.01","volume_max":"100.00","volume_step":"0.01"}
            ],
            "quotes": [
                {"symbol":"EURUSD.a","bid":"1.10000","ask":"1.10010","tick_time":now.to_rfc3339()},
                {"symbol":"EURGBP.a","bid":"0.85000","ask":"0.85010","tick_time":now.to_rfc3339()},
                {"symbol":"GBPUSD.a","bid":"1.25000","ask":"1.25010","tick_time":now.to_rfc3339()}
            ]
        })
    }

    fn supplement() -> Mt5SizingSupplement {
        Mt5SizingSupplement {
            estimated_cost_per_quantity: Decimal::new(2, 1),
            slippage_gap_buffer_per_quantity: Decimal::new(1, 1),
            pending_order_exposure: Decimal::from(100),
            available_margin: Decimal::from(1000),
            pending_order_margin: Decimal::from(100),
            margin_per_quantity: Decimal::from(50),
            min_notional_account_currency: None,
        }
    }

    #[test]
    fn uses_ask_and_broker_steps_and_tick_value() {
        let now = Utc::now();
        let snapshot = snapshot(now.clone());
        let result = prepare_mt5_sizing(
            &snapshot, "EURUSD.a", "USD", true, now, Duration::seconds(30), &supplement(),
        ).unwrap();
        assert_eq!(result.entry_price, Decimal::new(11001, 4));
        assert_eq!(result.context.quantity_step, Decimal::new(1, 2));
        assert_eq!(result.context.notional_to_account_rate, Decimal::from(100000));
        assert_eq!(result.context.available_margin, Some(Decimal::from(1000)));
    }

    #[test]
    fn derives_cross_currency_conversion_from_fresh_quotes() {
        let now = Utc::now();
        let snapshot = snapshot(now.clone());
        let result = prepare_mt5_sizing(
            &snapshot, "EURGBP.a", "USD", false, now, Duration::seconds(30), &supplement(),
        ).unwrap();
        assert!(result.context.notional_to_account_rate > Decimal::from(100000));
    }

    #[test]
    fn stale_and_mismatched_account_fail_closed() {
        let now = Utc::now();
        let stale = snapshot(now.clone() - Duration::seconds(60));
        assert_eq!(
            prepare_mt5_sizing(&stale, "EURUSD.a", "USD", true, now.clone(), Duration::seconds(30), &supplement()).unwrap_err(),
            Mt5SizingDataError::Stale
        );
        let current = snapshot(now.clone());
        assert_eq!(
            prepare_mt5_sizing(&current, "EURUSD.a", "EUR", true, now, Duration::seconds(30), &supplement()).unwrap_err(),
            Mt5SizingDataError::AccountCurrencyMismatch
        );
    }

    #[test]
    fn missing_fx_path_fails_closed() {
        let now = Utc::now();
        let mut value = snapshot(now.clone());
        value["quotes"] = json!([
            {"symbol":"EURUSD.a","bid":"1.1","ask":"1.1001","tick_time":now.to_rfc3339()}
        ]);
        assert_eq!(
            prepare_mt5_sizing(&value, "EURGBP.a", "USD", true, now, Duration::seconds(30), &supplement()).unwrap_err(),
            Mt5SizingDataError::Malformed
        );
    }
}

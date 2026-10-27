//! Strict adapter from persisted MT5 market data to instrument-aware sizing inputs.
//! This module performs no I/O and never submits orders. Missing/stale FX data fails closed.
use chrono::{DateTime, Duration, Utc};
use rust_decimal::Decimal;
use serde_json::Value;
use std::str::FromStr;
use crate::instrument_sizing::{InstrumentSizingContext, LossModel};

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum Mt5SizingDataError {
    #[error("MT5 market-data snapshot is malformed or incomplete")] Malformed,
    #[error("MT5 snapshot or quote is stale or from the future")] Stale,
    #[error("account currency does not match authenticated snapshot")] AccountCurrencyMismatch,
    #[error("target instrument is absent")] InstrumentUnavailable,
    #[error("fresh FX quotes do not provide a conversion path to account currency")] FxConversionUnavailable,
    #[error("explicit cost or margin inputs are invalid")] InvalidSupplement,
    #[error("decimal parsing or arithmetic failed")] Decimal,
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
struct Instrument { symbol: String, base: String, quote: String, contract: Decimal, tick: Decimal, tick_value: Decimal, min: Decimal, max: Decimal, step: Decimal }
#[derive(Debug, Clone)]
struct Quote { symbol: String, bid: Decimal, ask: Decimal, time: DateTime<Utc> }

fn text<'a>(v: &'a Value, key: &str) -> Result<&'a str, Mt5SizingDataError> {
    v.get(key).and_then(Value::as_str).filter(|s| !s.trim().is_empty() && s.trim() == *s).ok_or(Mt5SizingDataError::Malformed)
}
fn dec(v: &Value) -> Result<Decimal, Mt5SizingDataError> {
    v.as_str().and_then(|s| Decimal::from_str(s).ok()).filter(|n| *n > Decimal::ZERO).ok_or(Mt5SizingDataError::Malformed)
}
fn time(v: &Value, key: &str) -> Result<DateTime<Utc>, Mt5SizingDataError> {
    DateTime::parse_from_rfc3339(text(v,key)?).map(|d| d.with_timezone(&Utc)).map_err(|_| Mt5SizingDataError::Malformed)
}
fn fresh(t: DateTime<Utc>, now: DateTime<Utc>, age: Duration) -> bool {
    age >= Duration::zero() && t <= now && now.signed_duration_since(t) <= age
}
fn instruments(s: &Value) -> Result<Vec<Instrument>, Mt5SizingDataError> {
    let rows=s.get("instruments").and_then(Value::as_array).filter(|a| !a.is_empty() && a.len()<=64).ok_or(Mt5SizingDataError::Malformed)?;
    let mut out=Vec::new();
    for v in rows {
        let i=Instrument { symbol:text(v,"symbol")?.into(), base:text(v,"base_currency")?.into(), quote:text(v,"quote_currency")?.into(),
            contract:dec(v.get("contract_size").ok_or(Mt5SizingDataError::Malformed)?)?, tick:dec(v.get("tick_size").ok_or(Mt5SizingDataError::Malformed)?)?,
            tick_value:dec(v.get("tick_value_account_currency").ok_or(Mt5SizingDataError::Malformed)?)?, min:dec(v.get("volume_min").ok_or(Mt5SizingDataError::Malformed)?)?,
            max:dec(v.get("volume_max").ok_or(Mt5SizingDataError::Malformed)?)?, step:dec(v.get("volume_step").ok_or(Mt5SizingDataError::Malformed)?)? };
        if i.max<i.min || i.min.checked_div(i.step).is_none_or(|n| n!=n.trunc()) || out.iter().any(|x: &Instrument| x.symbol==i.symbol) { return Err(Mt5SizingDataError::Malformed); }
        out.push(i);
    }
    Ok(out)
}
fn quotes(s: &Value, now: DateTime<Utc>, age: Duration) -> Result<Vec<Quote>, Mt5SizingDataError> {
    let rows=s.get("quotes").and_then(Value::as_array).filter(|a| !a.is_empty() && a.len()<=64).ok_or(Mt5SizingDataError::Malformed)?;
    let mut out=Vec::new();
    for v in rows {
        let q=Quote { symbol:text(v,"symbol")?.into(), bid:dec(v.get("bid").ok_or(Mt5SizingDataError::Malformed)?)?, ask:dec(v.get("ask").ok_or(Mt5SizingDataError::Malformed)?)?, time:time(v,"tick_time")? };
        if q.ask<q.bid { return Err(Mt5SizingDataError::Malformed); }
        if !fresh(q.time,now,age) { return Err(Mt5SizingDataError::Stale); }
        if out.iter().any(|x: &Quote| x.symbol==q.symbol) { return Err(Mt5SizingDataError::Malformed); }
        out.push(q);
    }
    Ok(out)
}
/// Converts currency with up to three fresh FX edges. Uses bid for direct and 1/ask for inverse.
fn fx_rate(from: &str, to: &str, specs: &[Instrument], qs: &[Quote]) -> Result<Decimal,Mt5SizingDataError> {
    if from==to { return Ok(Decimal::ONE); }
    let mut todo=vec![(from.to_owned(),Decimal::ONE,0_usize)];
    let mut seen=vec![from.to_owned()];
    while let Some((cur,rate,depth))=todo.pop() {
        if depth>=3 { continue; }
        for i in specs {
            let Some(q)=qs.iter().find(|q| q.symbol==i.symbol) else { continue };
            let edge=if i.base==cur { Some((i.quote.as_str(),q.bid)) }
                else if i.quote==cur { Some((i.base.as_str(),Decimal::ONE.checked_div(q.ask).ok_or(Mt5SizingDataError::Decimal)?)) } else { None };
            if let Some((next,edge_rate))=edge {
                if edge_rate<=Decimal::ZERO || seen.iter().any(|x| x==next) { continue; }
                let combined=rate.checked_mul(edge_rate).ok_or(Mt5SizingDataError::Decimal)?;
                if next==to { return Ok(combined); }
                seen.push(next.into()); todo.push((next.into(),combined,depth+1));
            }
        }
    }
    Err(Mt5SizingDataError::FxConversionUnavailable)
}

/// Build a sizing context using ask for buys, bid for sells, tick value in account currency,
/// broker volume limits, and a fresh conversion path from instrument quote currency to account currency.
pub fn prepare_mt5_sizing(snapshot:&Value, symbol:&str, account_currency:&str, is_buy:bool, now:DateTime<Utc>, max_age:Duration, extra:&Mt5SizingSupplement)->Result<PreparedMt5Sizing,Mt5SizingDataError> {
    if symbol.trim().is_empty() || account_currency.trim().is_empty() || max_age<Duration::zero() { return Err(Mt5SizingDataError::Malformed); }
    if text(snapshot,"account_currency")? != account_currency { return Err(Mt5SizingDataError::AccountCurrencyMismatch); }
    let captured=time(snapshot,"captured_at")?;
    if !fresh(captured,now,max_age) { return Err(Mt5SizingDataError::Stale); }
    if extra.estimated_cost_per_quantity<Decimal::ZERO || extra.slippage_gap_buffer_per_quantity<Decimal::ZERO || extra.pending_order_exposure<Decimal::ZERO || extra.available_margin<Decimal::ZERO || extra.pending_order_margin<Decimal::ZERO || extra.margin_per_quantity<=Decimal::ZERO || extra.min_notional_account_currency.is_some_and(|x|x<Decimal::ZERO) { return Err(Mt5SizingDataError::InvalidSupplement); }
    let specs=instruments(snapshot)?; let qs=quotes(snapshot,now,max_age)?;
    if specs.len()!=qs.len() || specs.iter().any(|i| !qs.iter().any(|q|q.symbol==i.symbol)) { return Err(Mt5SizingDataError::Malformed); }
    let i=specs.iter().find(|i|i.symbol==symbol).ok_or(Mt5SizingDataError::InstrumentUnavailable)?;
    let q=qs.iter().find(|q|q.symbol==symbol).ok_or(Mt5SizingDataError::InstrumentUnavailable)?;
    let conversion=fx_rate(&i.quote,account_currency,&specs,&qs)?;
    let notional_rate=i.contract.checked_mul(conversion).ok_or(Mt5SizingDataError::Decimal)?;
    Ok(PreparedMt5Sizing {
        entry_price:if is_buy {q.ask}else{q.bid},
        context:InstrumentSizingContext { loss_model:LossModel::TickValue{tick_size:i.tick,tick_value_per_quantity:i.tick_value,tick_value_to_account_rate:Decimal::ONE},
            inputs_fresh:true,notional_to_account_rate:notional_rate,quantity_step:i.step,min_quantity:i.min,max_quantity:Some(i.max),
            min_notional_account_currency:extra.min_notional_account_currency,estimated_cost_per_quantity:extra.estimated_cost_per_quantity,
            slippage_gap_buffer_per_quantity:extra.slippage_gap_buffer_per_quantity,pending_order_exposure:extra.pending_order_exposure,
            available_margin:Some(extra.available_margin),pending_order_margin:extra.pending_order_margin,margin_per_quantity:Some(extra.margin_per_quantity)},
        snapshot_captured_at:captured,quote_time:q.time,source:text(snapshot,"source")?.into(),
        sequence:snapshot.get("sequence").and_then(Value::as_u64).ok_or(Mt5SizingDataError::Malformed)? })
}

#[cfg(test)]
mod tests {
 #![allow(clippy::unwrap_used, clippy::panic)]
 use super::*; use serde_json::json;
 fn snap(now:DateTime<Utc>)->Value { json!({"captured_at":now.to_rfc3339(),"account_currency":"USD","source":"mt5_python_readonly_v1","sequence":8,
 "instruments":[
 {"symbol":"EURUSD.a","base_currency":"EUR","quote_currency":"USD","contract_size":"100000","tick_size":"0.00001","tick_value_account_currency":"1.00","volume_min":"0.01","volume_max":"100.00","volume_step":"0.01"},
 {"symbol":"EURGBP.a","base_currency":"EUR","quote_currency":"GBP","contract_size":"100000","tick_size":"0.00001","tick_value_account_currency":"1.20","volume_min":"0.01","volume_max":"100.00","volume_step":"0.01"},
 {"symbol":"GBPUSD.a","base_currency":"GBP","quote_currency":"USD","contract_size":"100000","tick_size":"0.00001","tick_value_account_currency":"1.50","volume_min":"0.01","volume_max":"100.00","volume_step":"0.01"}],
 "quotes":[{"symbol":"EURUSD.a","bid":"1.10000","ask":"1.10010","tick_time":now.to_rfc3339()},{"symbol":"EURGBP.a","bid":"0.85000","ask":"0.85010","tick_time":now.to_rfc3339()},{"symbol":"GBPUSD.a","bid":"1.25000","ask":"1.25010","tick_time":now.to_rfc3339()}]}) }
 fn extra()->Mt5SizingSupplement { Mt5SizingSupplement{estimated_cost_per_quantity:Decimal::new(2,1),slippage_gap_buffer_per_quantity:Decimal::new(1,1),pending_order_exposure:Decimal::from(100),available_margin:Decimal::from(1000),pending_order_margin:Decimal::from(100),margin_per_quantity:Decimal::from(50),min_notional_account_currency:None} }
 #[test] fn uses_ask_and_broker_steps_and_tick_value(){let n=Utc::now();let r=prepare_mt5_sizing(&snap(n),"EURUSD.a","USD",true,n,Duration::seconds(30),&extra()).unwrap();assert_eq!(r.entry_price,Decimal::new(11001,4));assert_eq!(r.context.quantity_step,Decimal::new(1,2));assert_eq!(r.context.notional_to_account_rate,Decimal::from(100000));assert_eq!(r.context.available_margin,Some(Decimal::from(1000)));}
 #[test] fn derives_cross_currency_conversion_from_fresh_quotes(){let n=Utc::now();let r=prepare_mt5_sizing(&snap(n),"EURGBP.a","USD",false,n,Duration::seconds(30),&extra()).unwrap();assert!(r.context.notional_to_account_rate>Decimal::from(100000));}
 #[test] fn stale_and_mismatched_account_fail_closed(){let n=Utc::now();assert_eq!(prepare_mt5_sizing(&snap(n-Duration::seconds(60)),"EURUSD.a","USD",true,n,Duration::seconds(30),&extra()).unwrap_err(),Mt5SizingDataError::Stale);assert_eq!(prepare_mt5_sizing(&snap(n),"EURUSD.a","EUR",true,n,Duration::seconds(30),&extra()).unwrap_err(),Mt5SizingDataError::AccountCurrencyMismatch);}
 #[test] fn missing_fx_path_fails_closed(){let n=Utc::now();let mut s=snap(n);s["quotes"]=json!([{"symbol":"EURUSD.a","bid":"1.1","ask":"1.1001","tick_time":n.to_rfc3339()}]);assert_eq!(prepare_mt5_sizing(&s,"EURGBP.a","USD",true,n,Duration::seconds(30),&extra()).unwrap_err(),Mt5SizingDataError::Malformed);}
}

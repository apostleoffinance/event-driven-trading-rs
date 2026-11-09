//! Live FX conversion for calculator/account-currency math.
//! Rates are fetched from the configured live candle provider; demo and stale
//! data are never silently accepted. Cross rates may triangulate through USD.

use std::str::FromStr;

use chrono::Utc;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use crate::{
    error::MarketDataError,
    instruments::INSTRUMENTS,
    model::{AssetClass, Freshness},
    service::{CandleQuery, MarketDataService},
};

#[derive(Debug, Deserialize)]
pub struct FxConversionQuery {
    pub from: String,
    pub to: String,
    pub amount: Option<Decimal>,
    #[serde(default)]
    pub allow_stale: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct FxRate {
    pub rate: String,
    pub from: String,
    pub to: String,
    pub source: String,
    pub method: String,
    pub freshness: String,
    pub age_seconds: Option<i64>,
    pub timestamp: Option<String>,
    pub assumed: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct FxConversionResponse {
    pub from: String,
    pub to: String,
    pub amount: String,
    pub converted_amount: String,
    pub rate: String,
    pub source: String,
    pub method: String,
    pub freshness: String,
    pub age_seconds: Option<i64>,
    pub timestamp: Option<String>,
    pub assumed: bool,
    pub stale_allowed: bool,
    pub disclaimer: &'static str,
}

fn currency(raw: &str) -> Result<String, MarketDataError> {
    let code = raw.trim().to_ascii_uppercase();
    if code.len() != 3 || !code.bytes().all(|b| b.is_ascii_uppercase()) {
        return Err(MarketDataError::InvalidRequest(
            "Currency codes must be three uppercase letters (e.g. USD, EUR, GBP).".into(),
        ));
    }
    Ok(code)
}

fn resolve_pair(from: &str, to: &str) -> Result<(String, bool), MarketDataError> {
    let direct = format!("{from}{to}");
    let inverse = format!("{to}{from}");
    if INSTRUMENTS
        .iter()
        .any(|i| i.id == direct && i.asset_class == AssetClass::Forex)
    {
        Ok((direct, false))
    } else if INSTRUMENTS
        .iter()
        .any(|i| i.id == inverse && i.asset_class == AssetClass::Forex)
    {
        Ok((inverse, true))
    } else {
        Err(MarketDataError::Unsupported {
            provider: crate::model::ProviderId::BiQuote,
            reason: format!("No live FX pair mapping is available for {from}/{to}."),
        })
    }
}

async fn pair_rate(
    service: &MarketDataService,
    from: &str,
    to: &str,
    allow_stale: bool,
) -> Result<FxRate, MarketDataError> {
    if from == to {
        return Ok(FxRate {
            rate: "1".into(),
            from: from.into(),
            to: to.into(),
            source: "identity".into(),
            method: "same_currency".into(),
            freshness: "not_required".into(),
            age_seconds: Some(0),
            timestamp: None,
            assumed: false,
        });
    }

    let (symbol, inverted) = resolve_pair(from, to)?;

    let query = CandleQuery {
        instrument: Some(symbol.clone()),
        timeframe: Some("M1".into()),
        from: None,
        to: None,
        limit: Some("1".into()),
        provider: Some("biquote-live".into()),
    };
    let quote = service.get_candles(&query).await?;
    if quote.classification != crate::model::DataClass::Live {
        return Err(MarketDataError::ProviderUnavailable {
            provider: crate::model::ProviderId::BiQuote,
            reason: "FX conversion requires live provider data; demo data is not accepted.".into(),
        });
    }

    let candle = quote
        .candles
        .last()
        .ok_or_else(|| MarketDataError::ProviderUnavailable {
            provider: crate::model::ProviderId::BiQuote,
            reason: format!("No current quote available for {symbol}."),
        })?;

    match quote.freshness {
        Freshness::Fresh => {}
        Freshness::Stale if allow_stale => {}
        Freshness::Stale => {
            return Err(MarketDataError::ProviderUnavailable {
                provider: crate::model::ProviderId::BiQuote,
                reason: format!(
                    "FX quote {symbol} is stale. Explicit stale-data override is required for an estimate."
                ),
            });
        }
        Freshness::Unknown | Freshness::Historical | Freshness::Demo => {
            return Err(MarketDataError::ProviderUnavailable {
                provider: crate::model::ProviderId::BiQuote,
                reason: format!(
                    "FX quote {symbol} is not a current live quote; historical/unknown data cannot be used for conversion."
                ),
            });
        }
    }

    let raw = Decimal::from_str(&candle.close.to_string()).map_err(|_| {
        MarketDataError::InvalidProviderData {
            provider: crate::model::ProviderId::BiQuote,
            reason: "FX quote could not be represented as a decimal.".into(),
        }
    })?;
    if raw <= Decimal::ZERO {
        return Err(MarketDataError::InvalidProviderData {
            provider: crate::model::ProviderId::BiQuote,
            reason: "FX quote must be positive.".into(),
        });
    }

    let rate = if inverted { Decimal::ONE / raw } else { raw };
    let age = Some((Utc::now().timestamp_millis() - candle.timestamp_ms).max(0) / 1000);
    Ok(FxRate {
        rate: rate.normalize().to_string(),
        from: from.into(),
        to: to.into(),
        source: quote.provider.name.into(),
        method: if inverted {
            "inverse_pair"
        } else {
            "direct_pair"
        }
        .into(),
        freshness: format!("{:?}", quote.freshness).to_ascii_lowercase(),
        age_seconds: age,
        timestamp: quote.last_candle_at,
        assumed: false,
    })
}

pub async fn convert(
    service: &MarketDataService,
    q: FxConversionQuery,
) -> Result<FxConversionResponse, MarketDataError> {
    let from = currency(&q.from)?;
    let to = currency(&q.to)?;
    let amount = q.amount.unwrap_or(Decimal::ONE);
    if amount <= Decimal::ZERO {
        return Err(MarketDataError::InvalidRequest(
            "Amount must be greater than zero.".into(),
        ));
    }

    let result = if from == to {
        pair_rate(service, &from, &to, q.allow_stale).await?
    } else {
        match pair_rate(service, &from, &to, q.allow_stale).await {
            Ok(rate) => rate,
            Err(MarketDataError::Unsupported { .. }) => {
                let first = pair_rate(service, &from, "USD", q.allow_stale).await?;
                let second = pair_rate(service, "USD", &to, q.allow_stale).await?;
                let first_rate = Decimal::from_str(&first.rate).map_err(|_| {
                    MarketDataError::InvalidProviderData {
                        provider: crate::model::ProviderId::BiQuote,
                        reason: "Invalid conversion rate.".into(),
                    }
                })?;
                let second_rate = Decimal::from_str(&second.rate).map_err(|_| {
                    MarketDataError::InvalidProviderData {
                        provider: crate::model::ProviderId::BiQuote,
                        reason: "Invalid conversion rate.".into(),
                    }
                })?;
                let combined = first_rate * second_rate;
                FxRate {
                    rate: combined.normalize().to_string(),
                    from: from.clone(),
                    to: to.clone(),
                    source: "BiQuote Live".into(),
                    method: "USD_triangulation".into(),
                    freshness: if first.freshness == "not_required" {
                        second.freshness.clone()
                    } else if second.freshness == "not_required" {
                        first.freshness.clone()
                    } else if first.freshness == "fresh" && second.freshness == "fresh" {
                        "fresh".into()
                    } else {
                        "stale".into()
                    },
                    age_seconds: Some(
                        first
                            .age_seconds
                            .unwrap_or(0)
                            .max(second.age_seconds.unwrap_or(0)),
                    ),
                    timestamp: first.timestamp.clone().or(second.timestamp.clone()),
                    assumed: false,
                }
            }
            Err(other) => return Err(other),
        }
    };

    let rate =
        Decimal::from_str(&result.rate).map_err(|_| MarketDataError::InvalidProviderData {
            provider: crate::model::ProviderId::BiQuote,
            reason: "Invalid conversion rate.".into(),
        })?;
    let converted = (amount * rate).round_dp(8);
    Ok(FxConversionResponse {
        from,
        to,
        amount: amount.normalize().to_string(),
        converted_amount: converted.normalize().to_string(),
        rate: rate.normalize().to_string(),
        source: result.source,
        method: result.method,
        freshness: result.freshness,
        age_seconds: result.age_seconds,
        timestamp: result.timestamp,
        assumed: result.assumed,
        stale_allowed: q.allow_stale,
        disclaimer: "Live FX conversion for analysis only. Verify broker contract/tick value and account currency before trading; no order is sent.",
    })
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;

    #[test]
    fn resolves_direct_and_inverse_pairs() {
        assert_eq!(
            resolve_pair("USD", "JPY").unwrap(),
            ("USDJPY".into(), false)
        );
        assert_eq!(resolve_pair("JPY", "USD").unwrap(), ("USDJPY".into(), true));
        assert_eq!(
            resolve_pair("USD", "NGN").unwrap(),
            ("USDNGN".into(), false)
        );
    }

    #[test]
    fn validates_currency_code() {
        assert_eq!(currency("usd").unwrap(), "USD");
        assert!(currency("US").is_err());
        assert!(currency("US1").is_err());
        assert!(currency("USDT").is_err());
    }
}

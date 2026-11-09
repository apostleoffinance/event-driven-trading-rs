//! Instrument registry. Instrument identity (`id`) is independent of the
//! symbol each provider uses (`aliases`). An instrument with no alias for a
//! provider is simply not supported by that provider.
//!
//! These are demo-grade definitions for charting, not a confirmed list of
//! tradable symbols on any account or venue.

use serde::Serialize;

use crate::model::{AssetClass, ProviderId, VolumeKind};

#[derive(Debug, Clone, Copy)]
pub struct Instrument {
    pub id: &'static str,
    pub display_symbol: &'static str,
    pub asset_class: AssetClass,
    pub venue: Option<&'static str>,
    /// Provider used when a request names none. Never `Demo`: demo data must
    /// be requested explicitly.
    pub default_provider: ProviderId,
    pub price_precision: u8,
    pub volume_kind: Option<VolumeKind>,
    /// Provider-specific symbols.
    pub aliases: &'static [(ProviderId, &'static str)],
    /// Anchor for the synthetic demo provider only.
    pub demo_reference_price: f64,
}

impl Instrument {
    pub fn alias(&self, provider: ProviderId) -> Option<&'static str> {
        if provider == ProviderId::Demo {
            return Some(self.id);
        }
        self.aliases
            .iter()
            .find(|(p, _)| *p == provider)
            .map(|(_, symbol)| *symbol)
    }

    pub fn meta(&self) -> InstrumentMeta {
        InstrumentMeta {
            id: self.id,
            symbol: self.display_symbol,
            asset_class: self.asset_class,
            venue: self.venue,
            price_precision: self.price_precision,
            volume_kind: self.volume_kind,
            default_provider: self.default_provider,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct InstrumentMeta {
    pub id: &'static str,
    pub symbol: &'static str,
    pub asset_class: AssetClass,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub venue: Option<&'static str>,
    pub price_precision: u8,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub volume_kind: Option<VolumeKind>,
    pub default_provider: ProviderId,
}

pub static INSTRUMENTS: &[Instrument] = &[
    Instrument {
        id: "EURUSD",
        display_symbol: "EUR/USD",
        asset_class: AssetClass::Forex,
        venue: None,
        default_provider: ProviderId::BiQuote,
        price_precision: 5,
        volume_kind: Some(VolumeKind::Tick),
        aliases: &[
            (ProviderId::Mt5Bridge, "EURUSD"),
            (ProviderId::BiQuote, "EURUSD"),
        ],
        demo_reference_price: 1.0841,
    },
    Instrument {
        id: "GBPUSD",
        display_symbol: "GBP/USD",
        asset_class: AssetClass::Forex,
        venue: None,
        default_provider: ProviderId::BiQuote,
        price_precision: 5,
        volume_kind: Some(VolumeKind::Tick),
        aliases: &[
            (ProviderId::Mt5Bridge, "GBPUSD"),
            (ProviderId::BiQuote, "GBPUSD"),
        ],
        demo_reference_price: 1.269,
    },
    Instrument {
        id: "USDJPY",
        display_symbol: "USD/JPY",
        asset_class: AssetClass::Forex,
        venue: None,
        default_provider: ProviderId::BiQuote,
        price_precision: 3,
        volume_kind: Some(VolumeKind::Tick),
        aliases: &[
            (ProviderId::Mt5Bridge, "USDJPY"),
            (ProviderId::BiQuote, "USDJPY"),
        ],
        demo_reference_price: 151.2,
    },
    Instrument {
        id: "AUDUSD",
        display_symbol: "AUD/USD",
        asset_class: AssetClass::Forex,
        venue: None,
        default_provider: ProviderId::BiQuote,
        price_precision: 5,
        volume_kind: Some(VolumeKind::Tick),
        aliases: &[(ProviderId::BiQuote, "AUDUSD")],
        demo_reference_price: 0.66,
    },
    Instrument {
        id: "USDCAD",
        display_symbol: "USD/CAD",
        asset_class: AssetClass::Forex,
        venue: None,
        default_provider: ProviderId::BiQuote,
        price_precision: 5,
        volume_kind: Some(VolumeKind::Tick),
        aliases: &[(ProviderId::BiQuote, "USDCAD")],
        demo_reference_price: 1.36,
    },
    Instrument {
        id: "USDCHF",
        display_symbol: "USD/CHF",
        asset_class: AssetClass::Forex,
        venue: None,
        default_provider: ProviderId::BiQuote,
        price_precision: 5,
        volume_kind: Some(VolumeKind::Tick),
        aliases: &[(ProviderId::BiQuote, "USDCHF")],
        demo_reference_price: 0.89,
    },
    Instrument {
        id: "NZDUSD",
        display_symbol: "NZD/USD",
        asset_class: AssetClass::Forex,
        venue: None,
        default_provider: ProviderId::BiQuote,
        price_precision: 5,
        volume_kind: Some(VolumeKind::Tick),
        aliases: &[(ProviderId::BiQuote, "NZDUSD")],
        demo_reference_price: 0.61,
    },
    Instrument {
        id: "EURGBP",
        display_symbol: "EUR/GBP",
        asset_class: AssetClass::Forex,
        venue: None,
        default_provider: ProviderId::BiQuote,
        price_precision: 5,
        volume_kind: Some(VolumeKind::Tick),
        aliases: &[(ProviderId::BiQuote, "EURGBP")],
        demo_reference_price: 0.85,
    },
    Instrument { id: "EURJPY", display_symbol: "EUR/JPY", asset_class: AssetClass::Forex, venue: None, default_provider: ProviderId::BiQuote, price_precision: 3, volume_kind: Some(VolumeKind::Tick), aliases: &[(ProviderId::BiQuote, "EURJPY")], demo_reference_price: 161.0 },
    Instrument { id: "GBPJPY", display_symbol: "GBP/JPY", asset_class: AssetClass::Forex, venue: None, default_provider: ProviderId::BiQuote, price_precision: 3, volume_kind: Some(VolumeKind::Tick), aliases: &[(ProviderId::BiQuote, "GBPJPY")], demo_reference_price: 190.0 },
    Instrument {
        id: "BTCUSDT",
        display_symbol: "BTC/USDT",
        asset_class: AssetClass::CryptoSpot,
        venue: Some("Binance Spot"),
        default_provider: ProviderId::BinancePublic,
        price_precision: 2,
        volume_kind: Some(VolumeKind::Base),
        aliases: &[(ProviderId::BinancePublic, "BTCUSDT")],
        demo_reference_price: 67_250.0,
    },
    Instrument {
        id: "ETHUSDT",
        display_symbol: "ETH/USDT",
        asset_class: AssetClass::CryptoSpot,
        venue: Some("Binance Spot"),
        default_provider: ProviderId::BinancePublic,
        price_precision: 2,
        volume_kind: Some(VolumeKind::Base),
        aliases: &[(ProviderId::BinancePublic, "ETHUSDT")],
        demo_reference_price: 3_480.0,
    },
    Instrument {
        id: "SOLUSDT",
        display_symbol: "SOL/USDT",
        asset_class: AssetClass::CryptoSpot,
        venue: Some("Binance Spot"),
        default_provider: ProviderId::BinancePublic,
        price_precision: 2,
        volume_kind: Some(VolumeKind::Base),
        aliases: &[(ProviderId::BinancePublic, "SOLUSDT")],
        demo_reference_price: 172.0,
    },
    // Perpetuals use a different Binance market (USDⓈ-M futures) than the spot
    // adapter, so no `binance-public` alias exists: requesting it is rejected
    // rather than silently served from the spot market.
    Instrument {
        id: "BTCUSDT-PERP",
        display_symbol: "BTCUSDT PERP",
        asset_class: AssetClass::CryptoPerpetual,
        venue: Some("Binance USDⓈ-M"),
        default_provider: ProviderId::BinancePublic,
        price_precision: 1,
        volume_kind: Some(VolumeKind::Base),
        aliases: &[],
        demo_reference_price: 67_240.0,
    },
];

pub fn find(id: &str) -> Option<&'static Instrument> {
    let id = id.trim();
    INSTRUMENTS.iter().find(|i| i.id.eq_ignore_ascii_case(id))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_are_unique_and_never_default_to_demo() {
        let mut ids: Vec<_> = INSTRUMENTS.iter().map(|i| i.id).collect();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), INSTRUMENTS.len());
        assert!(INSTRUMENTS
            .iter()
            .all(|i| i.default_provider != ProviderId::Demo));
    }

    #[test]
    fn identity_is_separate_from_provider_alias() {
        let btc = find("btcusdt").unwrap();
        assert_eq!(btc.id, "BTCUSDT");
        assert_eq!(btc.alias(ProviderId::BinancePublic), Some("BTCUSDT"));
        assert_eq!(btc.alias(ProviderId::Mt5Bridge), None);
        assert_eq!(btc.alias(ProviderId::Demo), Some("BTCUSDT"));
    }

    #[test]
    fn perpetual_is_not_mapped_to_spot() {
        let perp = find("BTCUSDT-PERP").unwrap();
        assert_eq!(perp.asset_class, AssetClass::CryptoPerpetual);
        assert_eq!(perp.alias(ProviderId::BinancePublic), None);
    }

    #[test]
    fn unknown_instrument_is_none() {
        assert!(find("NOPE").is_none());
        assert!(find("").is_none());
    }
}

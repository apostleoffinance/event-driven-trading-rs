use crate::model::Candle;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CandleInvalid {
    NonFinite { index: usize },
    NonPositivePrice { index: usize },
    HighBelowLow { index: usize },
    HighBelowOpenOrClose { index: usize },
    LowAboveOpenOrClose { index: usize },
    NegativeVolume { index: usize },
    DuplicateTimestamp { index: usize },
    OutOfOrder { index: usize },
}

impl std::fmt::Display for CandleInvalid {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use CandleInvalid::*;
        match self {
            NonFinite { index } => write!(f, "candle {index} has a non-finite value"),
            NonPositivePrice { index } => write!(f, "candle {index} has a non-positive price"),
            HighBelowLow { index } => write!(f, "candle {index}: high < low"),
            HighBelowOpenOrClose { index } => write!(f, "candle {index}: high < open or close"),
            LowAboveOpenOrClose { index } => write!(f, "candle {index}: low > open or close"),
            NegativeVolume { index } => write!(f, "candle {index} has negative volume"),
            DuplicateTimestamp { index } => {
                write!(f, "candle {index} repeats the previous timestamp")
            }
            OutOfOrder { index } => write!(f, "candle {index} is older than the previous candle"),
        }
    }
}

/// Validate a single candle's internal consistency.
pub fn validate_candle(c: &Candle, index: usize) -> Result<(), CandleInvalid> {
    let prices = [c.open, c.high, c.low, c.close];
    if prices.iter().any(|p| !p.is_finite()) || c.volume.is_some_and(|v| !v.is_finite()) {
        return Err(CandleInvalid::NonFinite { index });
    }
    if prices.iter().any(|p| *p <= 0.0) {
        return Err(CandleInvalid::NonPositivePrice { index });
    }
    if c.high < c.low {
        return Err(CandleInvalid::HighBelowLow { index });
    }
    if c.high < c.open || c.high < c.close {
        return Err(CandleInvalid::HighBelowOpenOrClose { index });
    }
    if c.low > c.open || c.low > c.close {
        return Err(CandleInvalid::LowAboveOpenOrClose { index });
    }
    if c.volume.is_some_and(|v| v < 0.0) {
        return Err(CandleInvalid::NegativeVolume { index });
    }
    Ok(())
}

/// Validate a series: every candle, plus strictly ascending timestamps.
/// Duplicates and out-of-order candles are rejected, not repaired.
pub fn validate_series(candles: &[Candle]) -> Result<(), CandleInvalid> {
    for (index, candle) in candles.iter().enumerate() {
        validate_candle(candle, index)?;
        if index > 0 {
            let prev = candles[index - 1].timestamp_ms;
            if candle.timestamp_ms == prev {
                return Err(CandleInvalid::DuplicateTimestamp { index });
            }
            if candle.timestamp_ms < prev {
                return Err(CandleInvalid::OutOfOrder { index });
            }
        }
    }
    Ok(())
}

/// Order candles oldest-first. This is the only normalisation applied; it
/// never drops or alters values.
pub fn sort_ascending(candles: &mut [Candle]) {
    candles.sort_by_key(|c| c.timestamp_ms);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn c(ts: i64, o: f64, h: f64, l: f64, cl: f64, v: Option<f64>) -> Candle {
        Candle {
            timestamp_ms: ts,
            open: o,
            high: h,
            low: l,
            close: cl,
            volume: v,
        }
    }

    #[test]
    fn accepts_valid_candle_with_and_without_volume() {
        assert!(validate_candle(&c(1, 10.0, 12.0, 9.0, 11.0, Some(0.0)), 0).is_ok());
        assert!(validate_candle(&c(1, 10.0, 10.0, 10.0, 10.0, None), 0).is_ok());
    }

    #[test]
    fn rejects_each_ohlc_violation() {
        assert_eq!(
            validate_candle(&c(1, 10.0, 9.0, 10.0, 10.0, None), 3),
            Err(CandleInvalid::HighBelowLow { index: 3 })
        );
        // high >= low but below open
        assert_eq!(
            validate_candle(&c(1, 11.0, 10.5, 10.0, 10.2, None), 0),
            Err(CandleInvalid::HighBelowOpenOrClose { index: 0 })
        );
        // high below close
        assert_eq!(
            validate_candle(&c(1, 10.0, 10.5, 10.0, 11.0, None), 0),
            Err(CandleInvalid::HighBelowOpenOrClose { index: 0 })
        );
        // low above open
        assert_eq!(
            validate_candle(&c(1, 10.0, 12.0, 10.5, 11.0, None), 0),
            Err(CandleInvalid::LowAboveOpenOrClose { index: 0 })
        );
        // low above close
        assert_eq!(
            validate_candle(&c(1, 11.0, 12.0, 10.5, 10.0, None), 0),
            Err(CandleInvalid::LowAboveOpenOrClose { index: 0 })
        );
    }

    #[test]
    fn rejects_bad_numbers() {
        assert!(matches!(
            validate_candle(&c(1, f64::NAN, 2.0, 1.0, 1.5, None), 0),
            Err(CandleInvalid::NonFinite { .. })
        ));
        assert!(matches!(
            validate_candle(&c(1, 1.0, 2.0, 1.0, 1.5, Some(f64::INFINITY)), 0),
            Err(CandleInvalid::NonFinite { .. })
        ));
        assert!(matches!(
            validate_candle(&c(1, 0.0, 2.0, 0.0, 1.5, None), 0),
            Err(CandleInvalid::NonPositivePrice { .. })
        ));
        assert!(matches!(
            validate_candle(&c(1, 1.0, 2.0, 1.0, 1.5, Some(-0.1)), 0),
            Err(CandleInvalid::NegativeVolume { .. })
        ));
    }

    #[test]
    fn series_checks_order_and_duplicates() {
        let a = c(1_000, 1.0, 2.0, 1.0, 1.5, None);
        let b = c(2_000, 1.0, 2.0, 1.0, 1.5, None);
        assert!(validate_series(&[a, b]).is_ok());
        assert!(validate_series(&[]).is_ok());
        assert_eq!(
            validate_series(&[a, a]),
            Err(CandleInvalid::DuplicateTimestamp { index: 1 })
        );
        assert_eq!(
            validate_series(&[b, a]),
            Err(CandleInvalid::OutOfOrder { index: 1 })
        );
    }

    #[test]
    fn sort_orders_oldest_first_without_dropping() {
        let mut v = vec![
            c(3, 1.0, 2.0, 1.0, 1.5, None),
            c(1, 1.0, 2.0, 1.0, 1.5, None),
            c(2, 1.0, 2.0, 1.0, 1.5, None),
        ];
        sort_ascending(&mut v);
        assert_eq!(
            v.iter().map(|c| c.timestamp_ms).collect::<Vec<_>>(),
            [1, 2, 3]
        );
    }
}

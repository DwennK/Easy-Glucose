use crate::i18n::Text;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

pub const STALE_SECONDS: i64 = 600;
pub const GAP_SECONDS: i64 = 450;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Unit {
    #[default]
    Mmol,
    Mgdl,
}
impl Unit {
    pub fn label(self) -> &'static str {
        match self {
            Self::Mmol => "mmol/L",
            Self::Mgdl => "mg/dL",
        }
    }
    fn mgdl_per_unit(self) -> f64 {
        match self {
            Self::Mmol => 18.0,
            Self::Mgdl => 1.0,
        }
    }
    pub fn convert(self, value: f64) -> f64 {
        value / self.mgdl_per_unit()
    }
    pub(crate) fn guide_step(self) -> f64 {
        match self {
            Self::Mmol => 0.1,
            Self::Mgdl => 1.0,
        }
    }
    pub(crate) fn guide_value(self, mgdl: u16) -> f64 {
        self.round_guide(self.convert(mgdl.into()))
    }
    pub(crate) fn round_guide(self, value: f64) -> f64 {
        match self {
            Self::Mmol => (value * 10.0).round() / 10.0,
            Self::Mgdl => value.round(),
        }
    }
    pub(crate) fn guide_mgdl(self, value: f64) -> u16 {
        (value * self.mgdl_per_unit()).round() as u16
    }
    pub fn format(self, value: f64) -> String {
        match self {
            Self::Mmol => crate::i18n::decimal(format!("{:.1}", self.convert(value))),
            Self::Mgdl => format!("{value:.0}"),
        }
    }
    pub fn delta(self, value: f64) -> String {
        match self {
            Self::Mmol => crate::i18n::decimal(format!("{:+.1}", self.convert(value))),
            Self::Mgdl => format!("{value:+.0}"),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Trend {
    Unknown,
    DoubleUp,
    Up,
    SlightUp,
    Flat,
    SlightDown,
    Down,
    DoubleDown,
    NotComputable,
    OutOfRange,
}
impl Trend {
    pub fn parse(value: &serde_json::Value) -> Self {
        match value.as_str().unwrap_or("") {
            "DoubleUp" => Self::DoubleUp,
            "SingleUp" => Self::Up,
            "FortyFiveUp" => Self::SlightUp,
            "Flat" => Self::Flat,
            "FortyFiveDown" => Self::SlightDown,
            "SingleDown" => Self::Down,
            "DoubleDown" => Self::DoubleDown,
            "NotComputable" => Self::NotComputable,
            "RateOutOfRange" => Self::OutOfRange,
            _ => match value.as_u64() {
                Some(1) => Self::DoubleUp,
                Some(2) => Self::Up,
                Some(3) => Self::SlightUp,
                Some(4) => Self::Flat,
                Some(5) => Self::SlightDown,
                Some(6) => Self::Down,
                Some(7) => Self::DoubleDown,
                Some(8) => Self::NotComputable,
                Some(9) => Self::OutOfRange,
                _ => Self::Unknown,
            },
        }
    }
    pub fn arrow(self) -> &'static str {
        match self {
            Self::DoubleUp => "↑↑",
            Self::Up => "↑",
            Self::SlightUp => "↗",
            Self::Flat => "→",
            Self::SlightDown => "↘",
            Self::Down => "↓",
            Self::DoubleDown => "↓↓",
            _ => "—",
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Self::DoubleUp => Text::FastRise.get(),
            Self::Up => Text::Rising.get(),
            Self::SlightUp => Text::SlightRise.get(),
            Self::Flat => Text::Stable.get(),
            Self::SlightDown => Text::SlightFall.get(),
            Self::Down => Text::Falling.get(),
            Self::DoubleDown => Text::FastFall.get(),
            _ => Text::NoTrend.get(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Limit {
    Below(u16),
    Above(u16),
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Reading {
    pub mgdl: u16,
    pub measured_at: DateTime<Utc>,
    pub trend: Trend,
    #[serde(default)]
    pub limit: Option<Limit>,
}
impl Reading {
    pub fn age_seconds(&self, now: DateTime<Utc>) -> i64 {
        (now - self.measured_at).num_seconds()
    }
    pub fn stale(&self, now: DateTime<Utc>) -> bool {
        self.age_seconds(now) >= STALE_SECONDS || self.age_seconds(now) < -60
    }
    pub fn valid(&self, now: DateTime<Utc>) -> bool {
        (20..=600).contains(&self.mgdl)
            && self.measured_at.timestamp() > 0
            && self.measured_at <= now + chrono::Duration::seconds(60)
    }
    pub fn display(&self, unit: Unit) -> String {
        match self.limit {
            Some(Limit::Below(n)) => format!("< {}", unit.format(n.into())),
            Some(Limit::Above(n)) => format!("> {}", unit.format(n.into())),
            None => unit.format(self.mgdl.into()),
        }
    }
}

/// Strict UTC milliseconds. A malformed timestamp must never become "now".
pub fn parse_timestamp(raw: &str) -> Option<DateTime<Utc>> {
    let raw = raw
        .trim_matches('/')
        .strip_prefix("Date(")?
        .strip_suffix(')')?;
    let end = raw
        .char_indices()
        .skip(1)
        .find(|(_, c)| !c.is_ascii_digit())
        .map(|(i, _)| i)
        .unwrap_or(raw.len());
    let (millis, suffix) = raw.split_at(end);
    if !(suffix.is_empty()
        || (suffix.len() == 5
            && matches!(suffix.as_bytes()[0], b'+' | b'-')
            && suffix[1..].bytes().all(|c| c.is_ascii_digit())))
    {
        return None;
    }
    DateTime::from_timestamp_millis(millis.parse().ok()?)
}

pub fn merge_readings(
    history: &mut Vec<Reading>,
    incoming: Vec<Reading>,
    now: DateTime<Utc>,
    days: u8,
) {
    history.extend(incoming);
    let cutoff = now - chrono::Duration::days(i64::from(days.clamp(1, 30)));
    history.retain(|r| r.valid(now) && r.measured_at >= cutoff);
    history.sort_by_key(|r| r.measured_at);
    history.dedup_by(|later, earlier| {
        if later.measured_at == earlier.measured_at {
            earlier.clone_from(later);
            true
        } else {
            false
        }
    });
    if history.len() > 43200 {
        history.drain(..history.len() - 43200);
    }
}

/// Consecutive real samples only; gaps are separate drawable segments.
pub fn chart_segments(readings: &[Reading], start: i64, unit: Unit) -> Vec<Vec<[f64; 2]>> {
    let mut segments: Vec<Vec<[f64; 2]>> = vec![vec![]];
    let mut previous = None;
    for r in readings
        .iter()
        .filter(|r| r.measured_at.timestamp() >= start)
    {
        let t = r.measured_at.timestamp();
        if previous.is_some_and(|prev| t - prev > GAP_SECONDS) {
            segments.push(vec![]);
        }
        if let Some(segment) = segments.last_mut() {
            segment.push([t as f64, unit.convert(r.mgdl.into())]);
        }
        previous = Some(t);
    }
    segments
}

/// Select an actual visible sample by time alone. Never invent a value between samples.
/// Ties prefer the earlier sample so moving across a midpoint behaves consistently.
pub fn nearest_reading(readings: &[Reading], time: f64, start: f64, end: f64) -> Option<&Reading> {
    if !time.is_finite() || !start.is_finite() || !end.is_finite() || start > end {
        return None;
    }
    readings
        .iter()
        .filter(|r| {
            let t = r.measured_at.timestamp() as f64;
            t >= start && t <= end
        })
        .min_by(|a, b| {
            let a_time = a.measured_at.timestamp() as f64;
            let b_time = b.measured_at.timestamp() as f64;
            (a_time - time)
                .abs()
                .total_cmp(&(b_time - time).abs())
                .then_with(|| a.measured_at.cmp(&b.measured_at))
        })
}

pub fn recent_delta(history: &[Reading]) -> Option<(f64, i64)> {
    let pair = history.get(history.len().checked_sub(2)?..)?;
    let seconds = (pair[1].measured_at - pair[0].measured_at).num_seconds();
    if !(45..=GAP_SECONDS).contains(&seconds) || pair.iter().any(|r| r.limit.is_some()) {
        return None;
    }
    Some((
        f64::from(pair[1].mgdl) - f64::from(pair[0].mgdl),
        (seconds + 30) / 60,
    ))
}

pub fn demo_readings(now: DateTime<Utc>) -> Vec<Reading> {
    let end = now.timestamp() / 300 * 300;
    (0..288)
        .filter(|i| !(171..176).contains(i))
        .filter_map(|i| {
            let phase = i as f64;
            let value = 111.0
                + 19.0 * (phase / 17.0).sin()
                + 9.0 * (phase / 7.0).cos()
                + 34.0 * (-((phase - 242.0) / 11.0).powi(2)).exp();
            Some(Reading {
                mgdl: value.round() as u16,
                measured_at: DateTime::from_timestamp(end - (287 - i) * 300, 0)?,
                trend: Trend::Flat,
                limit: None,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn conversions() {
        assert_eq!(Unit::Mmol.format(100.0), "5.6");
        assert_eq!(Unit::Mgdl.format(100.0), "100");
        assert_eq!(Unit::Mmol.delta(-18.0), "-1.0");
    }
    #[test]
    fn chart_guides_move_one_displayed_step() {
        assert_eq!(Unit::Mmol.guide_value(70), 3.9);
        assert_eq!(Unit::Mmol.guide_mgdl(4.0), 72);
        assert_eq!(Unit::Mgdl.guide_value(70), 70.0);
        for unit in [Unit::Mmol, Unit::Mgdl] {
            for mgdl in 40..=400 {
                for direction in [-1.0, 1.0] {
                    let next = unit.guide_value(mgdl) + direction * unit.guide_step();
                    assert_eq!(
                        unit.guide_value(unit.guide_mgdl(next)),
                        unit.round_guide(next)
                    );
                }
            }
        }
    }
    #[test]
    fn trends() {
        for (n, name) in [
            (1, "DoubleUp"),
            (2, "SingleUp"),
            (3, "FortyFiveUp"),
            (4, "Flat"),
            (5, "FortyFiveDown"),
            (6, "SingleDown"),
            (7, "DoubleDown"),
        ] {
            assert_eq!(
                Trend::parse(&serde_json::json!(n)),
                Trend::parse(&serde_json::json!(name))
            );
        }
        assert_eq!(Trend::parse(&serde_json::json!(99)), Trend::Unknown);
    }
    #[test]
    fn timestamps() {
        assert_eq!(
            parse_timestamp("/Date(1691455258000-0400)/").map(|t| t.timestamp()),
            Some(1691455258)
        );
        assert_eq!(
            parse_timestamp("Date(1691455258000)"),
            parse_timestamp("Date(1691455258000+0200)")
        );
        for s in ["", "now", "Date(foo)", "Date(123x)", "Date(123+020)"] {
            assert!(parse_timestamp(s).is_none());
        }
    }
    #[test]
    fn freshness_and_cache() {
        let now = Utc::now();
        let mut r = Reading {
            mgdl: 110,
            measured_at: now - chrono::Duration::seconds(600),
            trend: Trend::Flat,
            limit: None,
        };
        assert!(r.stale(now));
        r.measured_at += chrono::Duration::seconds(1);
        assert!(!r.stale(now));
        let mut h = vec![r.clone(), r];
        merge_readings(&mut h, vec![], now, 7);
        assert_eq!(h.len(), 1);
        assert!(recent_delta(&h).is_none());
    }
    #[test]
    fn no_delta_across_gaps_or_limits() {
        let now = Utc::now();
        let mut h = vec![
            Reading {
                mgdl: 39,
                measured_at: now - chrono::Duration::minutes(5),
                trend: Trend::Flat,
                limit: Some(Limit::Below(40)),
            },
            Reading {
                mgdl: 110,
                measured_at: now,
                trend: Trend::Flat,
                limit: None,
            },
        ];
        assert!(recent_delta(&h).is_none());
        h[0].mgdl = 100;
        h[0].limit = None;
        assert_eq!(recent_delta(&h), Some((10.0, 5)));
        h[0].measured_at -= chrono::Duration::minutes(5);
        assert!(recent_delta(&h).is_none());
        for (seconds, minutes) in [(45, 1), (59, 1), (60, 1), (89, 1), (90, 2), (299, 5)] {
            h[0].measured_at = now - chrono::Duration::seconds(seconds);
            assert_eq!(recent_delta(&h), Some((10.0, minutes)));
        }
    }
    #[test]
    fn chart_selection_snaps_to_time_and_breaks_ties_consistently() {
        let base = Utc::now().timestamp();
        let reading = |offset, mgdl| Reading {
            mgdl,
            measured_at: DateTime::from_timestamp(base + offset, 0).unwrap(),
            trend: Trend::Flat,
            limit: None,
        };
        // Deliberately unsorted, and values very far apart: height must not affect selection.
        let readings = vec![reading(300, 250), reading(0, 50), reading(600, 100)];
        let select = |offset| {
            nearest_reading(
                &readings,
                (base + offset) as f64,
                base as f64,
                (base + 600) as f64,
            )
            .unwrap()
            .mgdl
        };
        assert_eq!(select(-100), 50);
        assert_eq!(select(149), 50);
        assert_eq!(select(150), 50);
        assert_eq!(select(151), 250);
        assert_eq!(select(300), 250);
        assert_eq!(select(1000), 100);
    }
    #[test]
    fn chart_selection_uses_visible_samples_and_preserves_limits_and_gaps() {
        let base = Utc::now().timestamp();
        let low = Reading {
            mgdl: 39,
            measured_at: DateTime::from_timestamp(base, 0).unwrap(),
            trend: Trend::Unknown,
            limit: Some(Limit::Below(40)),
        };
        let high = Reading {
            mgdl: 180,
            measured_at: DateTime::from_timestamp(base + 3600, 0).unwrap(),
            limit: None,
            ..low.clone()
        };
        let readings = vec![low.clone(), high.clone()];
        let selected = nearest_reading(
            &readings,
            (base + 1700) as f64,
            base as f64,
            (base + 3600) as f64,
        )
        .unwrap();
        assert_eq!(selected, &low);
        assert_eq!(selected.display(Unit::Mgdl), "< 40");
        assert_eq!(
            nearest_reading(
                &readings,
                base as f64,
                (base + 1) as f64,
                (base + 3600) as f64
            ),
            Some(&high)
        );
        assert!(
            nearest_reading(
                &readings,
                base as f64,
                (base + 1) as f64,
                (base + 3599) as f64
            )
            .is_none()
        );
        assert!(nearest_reading(&[], base as f64, 0.0, f64::MAX).is_none());
        for invalid in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            assert!(nearest_reading(&readings, invalid, 0.0, f64::MAX).is_none());
        }
    }
}

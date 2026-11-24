use crate::{
    core::{Reading, STALE_SECONDS, Trend},
    i18n::Text,
    sources::Provider,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    UrgentLow,
    Low,
    High,
    SignalLoss,
    Rise,
    Fall,
}
impl Kind {
    pub const ALL: [Self; 6] = [
        Self::UrgentLow,
        Self::Low,
        Self::High,
        Self::SignalLoss,
        Self::Rise,
        Self::Fall,
    ];
    pub fn sound_label(self) -> String {
        let title = if self == Self::SignalLoss {
            Text::SignalLossTone
        } else {
            self.title()
        };
        format!("G7 · {}", title.get())
    }
    pub fn title(self) -> Text {
        match self {
            Self::UrgentLow => Text::UrgentLow,
            Self::Low => Text::Low,
            Self::High => Text::High,
            Self::SignalLoss => Text::MissingReadings,
            Self::Rise => Text::FastRise,
            Self::Fall => Text::FastFall,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Intensity {
    Soft,
    #[default]
    Medium,
    Intense,
}
impl Intensity {
    pub fn title(self) -> Text {
        match self {
            Self::Soft => Text::SoftTone,
            Self::Medium => Text::MediumTone,
            Self::Intense => Text::IntenseTone,
        }
    }
    pub fn gain(self) -> f32 {
        match self {
            Self::Soft => 0.35,
            Self::Medium => 0.65,
            Self::Intense => 1.0,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Imported {
    pub digest: String,
    pub filename: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Rule {
    pub enabled: bool,
    pub tone: Kind,
    pub intensity: Intensity,
    pub imported: Option<Imported>,
    pub use_import: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub rules: [Rule; 6],
    pub urgent: u16,
    pub low: u16,
    pub high: u16,
    pub minutes: u16,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            rules: Kind::ALL.map(|tone| Rule {
                enabled: false,
                tone,
                intensity: Intensity::Medium,
                imported: None,
                use_import: false,
            }),
            urgent: 55,
            low: 70,
            high: 180,
            minutes: 20,
        }
    }
}
impl Settings {
    pub fn normalize(&mut self) {
        for rule in &mut self.rules {
            if rule.use_import
                && let Some((tone, intensity)) =
                    rule.imported.as_ref().and_then(Imported::g7_preset)
            {
                rule.tone = tone;
                rule.intensity = intensity;
                rule.use_import = false;
                rule.imported = None;
            }
        }
        self.urgent = self.urgent.clamp(40, 100);
        self.low = self.low.clamp(self.urgent + 1, 200);
        self.high = self.high.clamp(self.low + 1, 400);
        self.minutes = self.minutes.clamp(5, 120);
    }
}

// Only readings returned by a live fetch enter this state. Cache, charts, and
// simulation never arm glucose alerts. Repeated polls share one active episode.
#[derive(Default)]
pub struct Monitor {
    active: [bool; 6],
    started: Option<DateTime<Utc>>,
    latest: Option<DateTime<Utc>>,
}
impl Monitor {
    pub fn start(&mut self, now: DateTime<Utc>) {
        self.started.get_or_insert(now);
    }
    pub fn settings_changed(&mut self, settings: &Settings) {
        for (active, rule) in self.active.iter_mut().zip(&settings.rules) {
            *active &= rule.enabled;
        }
    }
    fn transition(&mut self, kind: Kind, condition: bool, settings: &Settings) -> bool {
        let index = kind as usize;
        let on = condition && settings.rules[index].enabled;
        let previous = std::mem::replace(&mut self.active[index], on);
        on && !previous
    }
    pub fn reading(
        &mut self,
        readings: &[Reading],
        now: DateTime<Utc>,
        settings: &Settings,
    ) -> Vec<Kind> {
        self.reading_from(Provider::Dexcom, readings, None, now, settings)
    }

    pub(crate) fn reading_from(
        &mut self,
        provider: Provider,
        readings: &[Reading],
        latest_known: Option<DateTime<Utc>>,
        now: DateTime<Utc>,
        settings: &Settings,
    ) -> Vec<Kind> {
        let Some(r) = readings
            .iter()
            .filter(|r| r.valid(now))
            .max_by_key(|r| r.measured_at)
        else {
            return vec![];
        };
        if self
            .latest
            .max(latest_known)
            .is_some_and(|time| r.measured_at < time)
        {
            return vec![];
        }
        let previous = self.latest.replace(r.measured_at);
        if r.age_seconds(now) >= STALE_SECONDS {
            return vec![];
        }
        if r.age_seconds(now) < i64::from(settings.minutes) * 60 {
            self.active[Kind::SignalLoss as usize] = false;
        }
        // Intervening samples can end a previous episode. Notify only for
        // conditions still present in the latest sample, never for a resolved one.
        if let Some(previous) = previous {
            for sample in readings.iter().filter(|sample| {
                sample.measured_at >= previous
                    && sample.measured_at < r.measured_at
                    && sample.valid(now)
            }) {
                for (kind, condition) in conditions(sample, provider, settings) {
                    // Libre history may omit the arrow supplied with the live sample.
                    if matches!(kind, Kind::Rise | Kind::Fall)
                        && matches!(
                            sample.trend,
                            Trend::Unknown | Trend::NotComputable | Trend::OutOfRange
                        )
                    {
                        continue;
                    }
                    self.active[kind as usize] &= condition;
                }
            }
        }
        let mut alerts = Vec::new();
        for (kind, condition) in conditions(r, provider, settings) {
            if self.transition(kind, condition, settings) {
                alerts.push(kind);
            }
        }
        alerts
    }
    pub fn missing_deadline(&self, settings: &Settings) -> Option<DateTime<Utc>> {
        if self.active[Kind::SignalLoss as usize] {
            return None;
        }
        Some(self.latest.or(self.started)? + chrono::Duration::minutes(settings.minutes.into()))
    }
    pub fn missing(&mut self, now: DateTime<Utc>, settings: &Settings) -> bool {
        let late = self
            .latest
            .or(self.started)
            .is_some_and(|last| (now - last).num_seconds() >= i64::from(settings.minutes) * 60);
        self.transition(Kind::SignalLoss, late, settings)
    }
}

fn conditions(r: &Reading, provider: Provider, settings: &Settings) -> [(Kind, bool); 5] {
    let urgent = r.mgdl <= settings.urgent && settings.rules[Kind::UrgentLow as usize].enabled;
    let libre = provider == Provider::LibreLinkUp;
    [
        (Kind::UrgentLow, urgent),
        (Kind::Low, !urgent && r.mgdl <= settings.low),
        (Kind::High, r.mgdl >= settings.high),
        (
            Kind::Rise,
            r.trend == Trend::DoubleUp || (libre && r.trend == Trend::Up),
        ),
        (
            Kind::Fall,
            r.trend == Trend::DoubleDown || (libre && r.trend == Trend::Down),
        ),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    fn all_enabled() -> Settings {
        let mut s = Settings::default();
        for rule in &mut s.rules {
            rule.enabled = true;
        }
        s
    }
    #[test]
    fn older_live_readings_cannot_raise_an_alert_over_newer_cached_data() {
        let now = Utc::now();
        let older = Reading {
            mgdl: 60,
            trend: Trend::Flat,
            measured_at: now - chrono::Duration::minutes(4),
            limit: None,
        };
        let provider = Provider::Nightscout;
        let settings = all_enabled();
        let mut monitor = Monitor::default();
        assert!(
            monitor
                .reading_from(
                    provider,
                    std::slice::from_ref(&older),
                    Some(now),
                    now,
                    &settings
                )
                .is_empty()
        );
        assert!(monitor.missing_deadline(&settings).is_none());
        let correction = Reading {
            measured_at: now,
            ..older
        };
        assert_eq!(
            monitor.reading_from(provider, &[correction], Some(now), now, &settings),
            [Kind::Low]
        );
    }
    #[test]
    fn only_fresh_episodes_alert_with_urgent_priority_and_both_trends() {
        let now = Utc::now();
        let settings = all_enabled();
        let mut monitor = Monitor::default();
        let mut r = Reading {
            mgdl: 51,
            trend: Trend::DoubleDown,
            measured_at: now - chrono::Duration::minutes(12),
            limit: None,
        };
        assert!(monitor.reading(&[r.clone()], now, &settings).is_empty());
        r.measured_at = now;
        assert_eq!(
            monitor.reading(&[r.clone()], now, &settings),
            [Kind::UrgentLow, Kind::Fall]
        );
        assert!(monitor.reading(&[r.clone()], now, &settings).is_empty());
        r.mgdl = 65;
        assert_eq!(monitor.reading(&[r.clone()], now, &settings), [Kind::Low]);
        r.mgdl = 200;
        r.trend = Trend::DoubleUp;
        assert_eq!(
            monitor.reading(&[r.clone()], now, &settings),
            [Kind::High, Kind::Rise]
        );
        r.measured_at -= chrono::Duration::minutes(1);
        r.mgdl = 40;
        assert!(monitor.reading(&[r], now, &settings).is_empty());
    }
    #[test]
    fn batched_recovery_rearms_only_the_current_episode() {
        let now = Utc::now();
        let sample = |minutes, mgdl| Reading {
            mgdl,
            measured_at: now + chrono::Duration::minutes(minutes),
            trend: Trend::Flat,
            limit: None,
        };
        let settings = all_enabled();
        let mut monitor = Monitor::default();
        let first = sample(0, 65);
        assert_eq!(
            monitor.reading(std::slice::from_ref(&first), now, &settings),
            [Kind::Low]
        );
        let batch = [sample(2, 64), first, sample(1, 95)];
        let later = batch[0].measured_at;
        assert_eq!(monitor.reading(&batch, later, &settings), [Kind::Low]);
        assert!(monitor.reading(&batch, later, &settings).is_empty());
        let recovered = [sample(3, 195), sample(4, 105)];
        assert!(
            monitor
                .reading(&recovered, recovered[1].measured_at, &settings)
                .is_empty()
        );
    }

    #[test]
    fn rapid_trend_rules_follow_each_sources_arrows() {
        let now = Utc::now();
        let settings = all_enabled();
        for (arrow, libre_alert) in [
            (1, Some(Kind::Fall)),
            (2, None),
            (3, None),
            (4, None),
            (5, Some(Kind::Rise)),
            (0, None),
        ] {
            let batch = crate::sources::libre::parse_graph(&serde_json::json!({
                "graphData": [], "connection": {"glucoseMeasurement": {
                    "FactoryTimestamp": now.to_rfc3339(), "TrendArrow": arrow, "ValueInMgPerDl": 105
                }}
            }))
            .unwrap();
            let mut monitor = Monitor::default();
            let expected: Vec<_> = libre_alert.into_iter().collect();
            assert_eq!(
                monitor.reading_from(Provider::LibreLinkUp, &batch, None, now, &settings),
                expected
            );
            if libre_alert.is_some() {
                let mut next = batch[0].clone();
                next.measured_at += chrono::Duration::minutes(1);
                let mut historical = batch[0].clone();
                historical.trend = Trend::Unknown;
                let later = next.measured_at;
                assert!(
                    monitor
                        .reading_from(
                            Provider::LibreLinkUp,
                            &[historical, next],
                            None,
                            later,
                            &settings
                        )
                        .is_empty()
                );
            }
            for provider in [Provider::Dexcom, Provider::Nightscout] {
                assert!(
                    Monitor::default()
                        .reading_from(provider, &batch, None, now, &settings)
                        .is_empty()
                );
            }
        }
    }

    #[test]
    fn missing_data_with_empty_and_repeated_responses_fires_once_until_recovery() {
        let now = Utc::now();
        let mut settings = all_enabled();
        settings.minutes = 5;
        let mut monitor = Monitor::default();
        assert!(monitor.missing_deadline(&settings).is_none());
        monitor.start(now);
        let due = now + chrono::Duration::minutes(5);
        assert_eq!(monitor.missing_deadline(&settings), Some(due));
        assert!(!monitor.missing(now, &settings));
        monitor.reading(&[], now, &settings);
        assert!(monitor.missing(due, &settings));
        assert!(monitor.missing_deadline(&settings).is_none());
        let mut r = Reading {
            mgdl: 110,
            trend: Trend::Flat,
            measured_at: now,
            limit: None,
        };
        monitor.reading(&[r.clone()], due, &settings);
        assert!(!monitor.missing(due, &settings));
        r.measured_at = due;
        monitor.reading(&[r], due, &settings);
        assert!(!monitor.missing(due, &settings));
        assert!(monitor.missing(due + chrono::Duration::minutes(5), &settings));
    }
}

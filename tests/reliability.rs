use chrono::{Duration, Utc};
use easy_glucose::{
    core::{Reading, Trend, Unit, chart_segments, merge_readings},
    dexcom::{Error, parse_readings},
    services::{Command, Event, Service, State},
    storage::{Config, Store},
};
use serde_json::json;

#[test]
fn missing_data_is_not_connected_by_chart() {
    let now = Utc::now();
    let history = vec![
        Reading {
            mgdl: 110,
            measured_at: now - Duration::minutes(20),
            trend: Trend::Flat,
            limit: None,
        },
        Reading {
            mgdl: 115,
            measured_at: now - Duration::minutes(15),
            trend: Trend::Flat,
            limit: None,
        },
        Reading {
            mgdl: 105,
            measured_at: now,
            trend: Trend::Flat,
            limit: None,
        },
    ];
    let segments = chart_segments(&history, (now - Duration::hours(1)).timestamp(), Unit::Mgdl);
    assert_eq!(
        segments.iter().map(Vec::len).collect::<Vec<_>>(),
        vec![2, 1]
    );
    assert_eq!(segments.iter().map(Vec::len).sum::<usize>(), history.len());
}
#[test]
fn cache_discards_expired_and_future_readings_and_accepts_corrections() {
    let now = Utc::now();
    let r = Reading {
        mgdl: 100,
        measured_at: now,
        trend: Trend::Flat,
        limit: None,
    };
    let mut history = vec![
        r.clone(),
        Reading {
            measured_at: now - Duration::days(8),
            ..r.clone()
        },
        Reading {
            measured_at: now + Duration::hours(1),
            ..r.clone()
        },
    ];
    merge_readings(&mut history, vec![Reading { mgdl: 101, ..r }], now, 7);
    assert_eq!(history.len(), 1);
    assert_eq!(history[0].mgdl, 101);
}
#[test]
fn malformed_or_future_values_do_not_become_current() {
    for (value, stamp) in [
        (0, "Date(1691455258000)".into()),
        (402, "Date(1691455258000)".into()),
        (100, "broken".into()),
        (
            100,
            format!(
                "Date({})",
                (Utc::now() + Duration::hours(2)).timestamp_millis()
            ),
        ),
    ] {
        assert_eq!(
            parse_readings(json!([{"WT":stamp,"Value":value,"Trend":"Flat"}])),
            Err(Error::Protocol)
        );
    }
}
#[test]
fn off_scale_values_are_explicit_limits() {
    let r = Reading {
        mgdl: 39,
        measured_at: Utc::now(),
        trend: Trend::Unknown,
        limit: Some(easy_glucose::core::Limit::Below(40)),
    };
    assert_eq!(r.display(Unit::Mgdl), "< 40");
    assert_eq!(r.display(Unit::Mmol), "< 2.2");
    assert_eq!(
        Reading {
            mgdl: 401,
            limit: Some(easy_glucose::core::Limit::Above(400)),
            ..r
        }
        .display(Unit::Mgdl),
        "> 400"
    );
}
#[test]
fn simulation_never_replaces_or_clears_the_real_cache() {
    let dir = tempfile::tempdir().expect("temp");
    let store = Store {
        directory: dir.path().into(),
    };
    let real = vec![Reading {
        mgdl: 123,
        measured_at: Utc::now(),
        trend: Trend::Flat,
        limit: None,
    }];
    store.save_cache(&real).expect("cache");
    let service = Service::start(
        store.clone(),
        Config::default(),
        eframe::egui::Context::default(),
    )
    .expect("worker");
    service.commands.send(Command::Demo(9)).expect("demo");
    let event = service
        .events
        .recv_timeout(std::time::Duration::from_secs(3))
        .expect("snapshot");
    assert!(
        matches!(event,Event::Data(s) if s.generation==9 && s.state==State::Simulation && !s.readings.is_empty())
    );
    service
        .commands
        .send(Command::Save(Config {
            history_days: 1,
            ..Config::default()
        }))
        .expect("save");
    service.commands.send(Command::Clear).expect("clear");
    let event = service
        .events
        .recv_timeout(std::time::Duration::from_secs(3))
        .expect("cleared");
    assert!(matches!(event,Event::Data(s) if s.readings.is_empty()));
    drop(service);
    assert_eq!(store.load_cache(7).expect("real cache"), real);
    assert_eq!(store.load_config().expect("config").history_days, 1);
}
#[test]
fn corrupt_settings_survive_recovery_save() {
    let dir = tempfile::tempdir().expect("temp");
    let store = Store {
        directory: dir.path().into(),
    };
    std::fs::write(dir.path().join("settings.json"), "not-json").expect("fixture");
    assert!(store.load_config().is_err());
    store.save_config(&Config::default()).expect("recovery");
    assert!(
        std::fs::read_dir(dir.path())
            .expect("list")
            .flatten()
            .any(|p| p
                .file_name()
                .to_string_lossy()
                .starts_with("settings-corrupt-"))
    );
    assert!(store.load_config().is_ok());
}

use easy_glucose::{
    alerts::{Imported, Intensity, Kind},
    i18n::{self, LANGUAGES, Text},
    storage::{Config, Store},
};

#[test]
fn imported_g7_sound_and_intensity_survive_all_locales_and_a_restart() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store {
        directory: dir.path().into(),
    };
    let mut config: Config = serde_json::from_str("{}").unwrap();
    assert!(config.alerts.rules.iter().all(|r| !r.enabled));
    let rule = &mut config.alerts.rules[Kind::UrgentLow as usize];
    rule.enabled = true;
    rule.intensity = Intensity::Soft;
    rule.use_import = true;
    rule.imported = Some(Imported {
        digest: "880baa2159e8f1d06d5ed3f24be87858fd3ac4fdb05e7bcafe010ca0aa487d6b".into(),
        filename: "renamed.wav".into(),
    });
    let expected = config.alerts.clone();
    for &(code, _) in LANGUAGES {
        config.language = Some(code.into());
        store.save_config(&config).unwrap();
        let restored = store.load_config().unwrap();
        i18n::activate(restored.language.as_deref());
        assert_eq!(restored.alerts, expected);
        let rule = &restored.alerts.rules[Kind::UrgentLow as usize];
        assert_eq!(
            rule.imported.as_ref().unwrap().label(),
            format!(
                "G7 · {} · {}",
                Text::UrgentLow.get(),
                Text::MediumTone.get()
            )
        );
        assert_eq!(rule.intensity.title().get(), Text::SoftTone.get());
    }
}

use easy_glucose::{
    core::Unit,
    i18n::{self, LANGUAGES, Text},
    sources::Error,
    storage::{Config, Store},
};

#[test]
fn a_saved_language_changes_numbers_and_existing_service_messages() {
    let folder = tempfile::tempdir().unwrap();
    let store = Store {
        directory: folder.path().into(),
    };
    let legacy: Config = serde_json::from_str(r#"{"widget_open":true}"#).unwrap();
    assert!(legacy.language.is_none());
    let config = Config {
        language: Some("fr".into()),
        ..legacy
    };
    store.save_config(&config).unwrap();
    let saved = store.load_config().unwrap();
    assert!(saved.widget_open);
    i18n::activate(saved.language.as_deref());
    let earlier_error = Error::Session.to_string();
    assert_eq!(Text::Settings.get(), "Réglages");
    assert_eq!(Unit::Mmol.format(100.0), "5,6");
    assert_eq!(i18n::notice(&earlier_error), Text::SessionExpired.get());
    for &(code, _) in LANGUAGES {
        i18n::activate(Some(code));
        assert_eq!(i18n::notice(&earlier_error), Text::SessionExpired.get());
    }
    i18n::activate(Some("en-US"));
    assert_eq!(Text::Settings.get(), "Settings");
    assert_eq!(Unit::Mmol.format(100.0), "5.6");
    assert_eq!(
        i18n::notice(&earlier_error),
        "Session expired. Please sign in again."
    );
}

use crate::i18n::Text;
use crate::{
    core::{Limit, Reading, Unit, merge_readings},
    dexcom::Region,
    sources::{Credentials, Provider},
};
use chrono::Utc;
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Theme {
    #[default]
    System,
    Dark,
    Light,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum WidgetSize {
    Compact,
    #[default]
    Normal,
    Extended,
}
impl WidgetSize {
    pub fn dimensions(self) -> [f32; 2] {
        match self {
            Self::Compact => [256.0, 154.0],
            Self::Normal => [320.0, 240.0],
            Self::Extended => [410.0, 300.0],
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Self::Compact => Text::Small.get(),
            Self::Normal => Text::Medium.get(),
            Self::Extended => Text::Large.get(),
        }
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub theme: Theme,
    pub language: Option<String>,
    pub unit: Unit,
    pub region: Region,
    pub provider: Provider,
    pub widget_size: WidgetSize,
    pub widget_position: Option<[f32; 2]>,
    pub widget_open: bool,
    pub always_on_top: bool,
    pub start_minimized: bool,
    pub close_to_tray: bool,
    pub minimize_to_tray: bool,
    pub autostart: bool,
    pub history_days: u8,
    pub low: u16,
    pub high: u16,
    pub period_hours: u8,
    pub onboarding_done: bool,
}
impl Default for Config {
    fn default() -> Self {
        Self {
            theme: Theme::System,
            language: None,
            unit: Unit::Mmol,
            region: Region::International,
            provider: Provider::Dexcom,
            widget_size: WidgetSize::Normal,
            widget_position: None,
            widget_open: false,
            always_on_top: true,
            start_minimized: false,
            close_to_tray: true,
            minimize_to_tray: false,
            autostart: false,
            history_days: 7,
            low: 70,
            high: 180,
            period_hours: 6,
            onboarding_done: false,
        }
    }
}
impl Config {
    pub const HISTORY_PERIODS: [u8; 4] = [3, 6, 12, 24];

    pub fn normalize(&mut self) {
        self.history_days = self.history_days.clamp(1, 30);
        self.low = self.low.clamp(40, 200);
        self.high = self.high.clamp(self.low + 1, 400);
        if !Self::HISTORY_PERIODS.contains(&self.period_hours) {
            self.period_hours = if self.period_hours == 1 { 3 } else { 6 };
        }
        if self
            .widget_position
            .is_some_and(|p| !p.iter().all(|v| v.is_finite() && v.abs() < 50000.0))
        {
            self.widget_position = None;
        }
    }
}
#[derive(Clone)]
pub struct Store {
    pub directory: PathBuf,
}
impl Store {
    pub fn open() -> Result<Self, String> {
        let directory = if let Some(p) = std::env::var_os("EASY_GLUCOSE_DATA_DIR") {
            PathBuf::from(p)
        } else {
            directories::ProjectDirs::from("ch", "Peaks", "Easy Glucose")
                .ok_or(Text::NoUserDir.english())?
                .data_local_dir()
                .to_owned()
        };
        fs::create_dir_all(&directory).map_err(|_| Text::CreateDir.english())?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&directory, fs::Permissions::from_mode(0o700))
                .map_err(|_| Text::DirPerm.english())?;
        }
        Ok(Self { directory })
    }
    fn read<T: DeserializeOwned>(&self, name: &str) -> Result<Option<T>, String> {
        let path = self.directory.join(name);
        match fs::metadata(&path) {
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(_) => return Err(Text::ReadError.english().into()),
            Ok(m) if m.len() > 16_777_216 => return Err(Text::TooLarge.english().into()),
            _ => {}
        }
        let bytes = fs::read(path).map_err(|_| Text::ReadError.english())?;
        serde_json::from_slice(&bytes)
            .map(Some)
            .map_err(|_| Text::Corrupt.english().into())
    }
    pub fn load_config(&self) -> Result<Config, String> {
        let mut c: Config = self.read("settings.json")?.unwrap_or_default();
        c.normalize();
        Ok(c)
    }
    pub fn save_config(&self, c: &Config) -> Result<(), String> {
        // Keep corrupt configuration available for diagnosis, even after a later save.
        if self.read::<Config>("settings.json").is_err() {
            let source = self.directory.join("settings.json");
            let backup = self.directory.join(format!(
                "settings-corrupt-{}.json",
                Utc::now().timestamp_millis()
            ));
            fs::rename(source, backup).map_err(|_| Text::Preserve.english())?;
        }
        atomic_json(&self.directory.join("settings.json"), c)
    }
    pub fn load_source_cache(
        &self,
        credentials: &Credentials,
        days: u8,
        legacy_dexcom: bool,
    ) -> Result<Vec<Reading>, String> {
        let file = format!("readings-{}.json", credentials.provider().key());
        let cached: Option<SourceCache> = self.read(&file)?;
        let mut readings = match cached {
            Some(c) if c.identity == credentials.cache_identity() => c.readings,
            Some(_) => vec![],
            None if legacy_dexcom && credentials.provider() == Provider::Dexcom => {
                self.load_cache(days)?
            }
            None => vec![],
        };
        merge_readings(&mut readings, vec![], Utc::now(), days);
        Ok(readings)
    }
    pub fn save_source_cache(
        &self,
        credentials: &Credentials,
        readings: &[Reading],
    ) -> Result<(), String> {
        let file = format!("readings-{}.json", credentials.provider().key());
        atomic_json(
            &self.directory.join(file),
            &SourceCache {
                identity: credentials.cache_identity(),
                readings: readings.to_vec(),
            },
        )
    }
    pub fn clear_source_cache(&self, provider: Provider) -> Result<(), String> {
        let file = self
            .directory
            .join(format!("readings-{}.json", provider.key()));
        match fs::remove_file(file) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(_) => return Err(Text::ClearSource.english().into()),
        }
        if provider == Provider::Dexcom {
            self.clear_cache()?;
        }
        Ok(())
    }

    pub fn load_cache(&self, days: u8) -> Result<Vec<Reading>, String> {
        let mut h: Vec<Reading> = self.read("readings.json")?.unwrap_or_default();
        // This untagged file was only ever produced by the original Dexcom client.
        for reading in &mut h {
            if reading.limit.is_none() {
                reading.limit = match reading.mgdl {
                    39 => Some(Limit::Below(40)),
                    401 => Some(Limit::Above(400)),
                    _ => None,
                };
            }
        }
        merge_readings(&mut h, vec![], Utc::now(), days);
        Ok(h)
    }
    pub fn save_cache(&self, h: &[Reading]) -> Result<(), String> {
        atomic_json(&self.directory.join("readings.json"), h)
    }
    pub fn clear_cache(&self) -> Result<(), String> {
        match fs::remove_file(self.directory.join("readings.json")) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(_) => Err(Text::ClearLocal.english().into()),
        }
    }
}
#[derive(Serialize, Deserialize)]
struct SourceCache {
    identity: String,
    readings: Vec<Reading>,
}

fn atomic_json<T: Serialize + ?Sized>(path: &Path, value: &T) -> Result<(), String> {
    let parent = path.parent().ok_or(Text::StoragePath.english())?;
    let mut temp =
        tempfile::NamedTempFile::new_in(parent).map_err(|_| Text::WriteError.english())?;
    serde_json::to_writer(&mut temp, value).map_err(|_| Text::SaveError.english())?;
    temp.flush().map_err(|_| Text::IncompleteWrite.english())?;
    temp.as_file()
        .sync_all()
        .map_err(|_| Text::SyncFile.english())?;
    temp.persist(path)
        .map_err(|_| Text::ReplaceFile.english())?;
    Ok(())
}
pub trait CredentialStore: Send {
    fn load(&self, provider: Provider) -> Result<Option<(Credentials, bool)>, String>;
    fn save(&self, credentials: &Credentials) -> Result<(), String>;
    fn delete(&self, provider: Provider) -> Result<(), String>;
}
pub struct SystemCredentials;
impl CredentialStore for SystemCredentials {
    fn load(&self, provider: Provider) -> Result<Option<(Credentials, bool)>, String> {
        load_credentials(provider)
    }
    fn save(&self, credentials: &Credentials) -> Result<(), String> {
        save_credentials(credentials)
    }
    fn delete(&self, provider: Provider) -> Result<(), String> {
        delete_credentials(provider)
    }
}

fn entry(provider: Provider) -> Result<keyring::Entry, String> {
    // An isolated data directory also gets an isolated vault. QA never touches real logins.
    let service = if let Some(directory) = std::env::var_os("EASY_GLUCOSE_DATA_DIR") {
        use sha2::{Digest, Sha256};
        format!(
            "ch.peaks.easy-glucose.qa.{:x}",
            Sha256::digest(directory.to_string_lossy().as_bytes())
        )
    } else {
        "ch.peaks.easy-glucose".into()
    };
    keyring::Entry::new(&service, provider.key()).map_err(|_| Text::Keyring.english().into())
}
pub fn save_credentials(c: &Credentials) -> Result<(), String> {
    let json = zeroize::Zeroizing::new(
        serde_json::to_string(c).map_err(|_| Text::InvalidCredentials.english())?,
    );
    entry(c.provider())?
        .set_password(&json)
        .map_err(|_| Text::KeyringSave.english().into())
}
pub fn load_credentials(provider: Provider) -> Result<Option<(Credentials, bool)>, String> {
    match entry(provider)?.get_password() {
        Ok(value) => {
            let value = zeroize::Zeroizing::new(value);
            decode_credentials(&value, provider).map(Some)
        }
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(_) => Err(Text::KeyringRead.english().into()),
    }
}
fn decode_credentials(value: &str, provider: Provider) -> Result<(Credentials, bool), String> {
    if let Ok(credentials) = serde_json::from_str::<Credentials>(value) {
        if credentials.provider() == provider {
            return Ok((credentials, false));
        }
    } else if provider == Provider::Dexcom
        && let Ok(c) = serde_json::from_str::<crate::dexcom::Credentials>(value)
    {
        return Ok((Credentials::Dexcom(c), true));
    }
    Err(Text::KeyringInvalid.english().into())
}
pub fn delete_credentials(provider: Provider) -> Result<(), String> {
    match entry(provider)?.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(_) => Err(Text::KeyringDelete.english().into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn config_cache_round_trip() {
        let dir = tempfile::tempdir().expect("directory");
        let s = Store {
            directory: dir.path().into(),
        };
        let c = Config::default();
        s.save_config(&c).expect("save");
        assert_eq!(s.load_config().expect("load"), c);
        let h = crate::core::demo_readings(Utc::now());
        s.save_cache(&h).expect("cache");
        assert_eq!(s.load_cache(7).expect("load"), h);
        s.clear_cache().expect("clear");
        assert!(s.load_cache(7).expect("empty").is_empty());
    }
    #[test]
    fn legacy_one_hour_history_opens_at_three_hours() {
        let dir = tempfile::tempdir().expect("directory");
        let store = Store {
            directory: dir.path().into(),
        };
        fs::write(dir.path().join("settings.json"), r#"{"period_hours":1}"#).expect("fixture");
        assert_eq!(store.load_config().expect("load").period_hours, 3);
    }
    #[test]
    fn corrupt_data_not_silently_erased() {
        let dir = tempfile::tempdir().expect("directory");
        let s = Store {
            directory: dir.path().into(),
        };
        fs::write(dir.path().join("settings.json"), "broken").expect("fixture");
        assert!(s.load_config().is_err());
        assert_eq!(
            fs::read_to_string(dir.path().join("settings.json")).expect("preserved"),
            "broken"
        );
    }
    #[test]
    fn settings_are_bounded() {
        let mut c = Config {
            low: 400,
            high: 20,
            history_days: 255,
            widget_position: Some([f32::NAN, 0.0]),
            ..Config::default()
        };
        c.normalize();
        assert_eq!((c.low, c.high, c.history_days), (200, 201, 30));
        assert!(c.widget_position.is_none());
    }
    fn libre_credentials(person: &str) -> Credentials {
        Credentials::LibreLinkUp(crate::sources::libre::Credentials {
            username: "fixture@example.invalid".into(),
            password: zeroize::Zeroizing::new("fixture-password".into()),
            patient_id: Some(person.into()),
        })
    }
    #[test]
    fn source_caches_are_bound_to_account_and_person() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store {
            directory: dir.path().into(),
        };
        let a = libre_credentials("person-a");
        let b = libre_credentials("person-b");
        let mut other_account = a.clone();
        if let Credentials::LibreLinkUp(c) = &mut other_account {
            c.username = "other@example.invalid".into();
        }
        let ns = Credentials::Nightscout(crate::sources::nightscout::Credentials {
            url: "https://fixture.invalid/".into(),
            token: zeroize::Zeroizing::new("fixture-token".into()),
            auth: crate::sources::nightscout::Auth::Token,
        });
        let readings = crate::core::demo_readings(Utc::now());
        store.save_source_cache(&a, &readings).unwrap();
        store.save_source_cache(&ns, &readings).unwrap();
        assert_eq!(store.load_source_cache(&a, 7, false).unwrap(), readings);
        assert!(store.load_source_cache(&b, 7, false).unwrap().is_empty());
        assert!(
            store
                .load_source_cache(&other_account, 7, false)
                .unwrap()
                .is_empty()
        );
        let raw = fs::read_to_string(dir.path().join("readings-librelinkup.json")).unwrap();
        for secret in ["fixture@example.invalid", "fixture-password", "person-a"] {
            assert!(!raw.contains(secret));
        }
        store.clear_source_cache(Provider::LibreLinkUp).unwrap();
        assert!(store.load_source_cache(&a, 7, false).unwrap().is_empty());
        assert_eq!(store.load_source_cache(&ns, 7, false).unwrap(), readings);
    }
    #[test]
    fn legacy_dexcom_migration_is_explicit_and_provider_specific() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store {
            directory: dir.path().into(),
        };
        let old =
            r#"{"username":"fixture","password":"fixture-password","region":"International"}"#;
        let (credentials, legacy) = decode_credentials(old, Provider::Dexcom).unwrap();
        assert!(legacy);
        assert!(decode_credentials(old, Provider::LibreLinkUp).is_err());
        let tagged = serde_json::to_string(&credentials).unwrap();
        assert!(!decode_credentials(&tagged, Provider::Dexcom).unwrap().1);
        assert!(decode_credentials(&tagged, Provider::Nightscout).is_err());
        let reading = serde_json::json!([{"mgdl":39,"measured_at":Utc::now(),"trend":"Flat"}]);
        fs::write(dir.path().join("readings.json"), reading.to_string()).unwrap();
        assert!(
            store
                .load_source_cache(&credentials, 7, false)
                .unwrap()
                .is_empty()
        );
        let migrated = store.load_source_cache(&credentials, 7, true).unwrap();
        assert_eq!(migrated[0].limit, Some(Limit::Below(40)));
        assert!(
            store
                .load_source_cache(&libre_credentials("person-a"), 7, true)
                .unwrap()
                .is_empty()
        );
        fs::write(
            dir.path().join("settings.json"),
            r#"{"unit":"Mgdl","region":"Us","history_days":14}"#,
        )
        .unwrap();
        let config = store.load_config().unwrap();
        assert_eq!(config.provider, Provider::Dexcom);
        assert_eq!(config.region, Region::Us);
        assert_eq!(config.unit, Unit::Mgdl);
        assert_eq!(config.history_days, 14);
    }
}

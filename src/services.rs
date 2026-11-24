use crate::i18n::Text;
use crate::{
    core::{Reading, demo_readings, merge_readings},
    sources::{Client, Credentials, Error, Patient, Provider},
    storage::{self, Config, Store},
};
use chrono::{DateTime, Utc};
use eframe::egui;
use std::{sync::mpsc, time::Duration};
use tokio::{sync::mpsc as channel, time::Instant};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum State {
    Disconnected,
    Connecting,
    Syncing,
    Connected,
    Empty,
    Offline,
    InvalidCredentials,
    ActionRequired,
    SelectingPatient,
    Simulation,
}
impl State {
    pub fn needs_account(&self) -> bool {
        matches!(
            self,
            Self::Disconnected
                | Self::InvalidCredentials
                | Self::ActionRequired
                | Self::SelectingPatient
        )
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::Disconnected => Text::Disconnected.get(),
            Self::Connecting => Text::Connecting.get(),
            Self::Syncing => Text::Syncing.get(),
            Self::Connected => Text::Connected.get(),
            Self::Empty => Text::NoShared.get(),
            Self::Offline => Text::Offline.get(),
            Self::InvalidCredentials => Text::Denied.get(),
            Self::ActionRequired => Text::ActionRequired.get(),
            Self::SelectingPatient => Text::SelectPerson.get(),
            Self::Simulation => Text::Simulation.get(),
        }
    }
    pub fn after_result(result: &Result<Vec<Reading>, Error>) -> Self {
        match result {
            Ok(v) if v.is_empty() => Self::Empty,
            Ok(_) => Self::Connected,
            Err(Error::Credentials) => Self::InvalidCredentials,
            Err(Error::SelectPatient) => Self::SelectingPatient,
            Err(e) if e.requires_action() => Self::ActionRequired,
            Err(_) => Self::Offline,
        }
    }
}
#[derive(Clone)]
pub struct Snapshot {
    pub generation: u64,
    pub provider: Provider,
    pub patients: Vec<Patient>,
    pub patient_name: Option<String>,
    pub readings: Vec<Reading>,
    pub state: State,
    pub last_sync: Option<DateTime<Utc>>,
    pub next_sync: Option<DateTime<Utc>>,
    pub error: Option<String>,
}
impl Default for Snapshot {
    fn default() -> Self {
        Self {
            generation: 0,
            provider: Provider::default(),
            patients: vec![],
            patient_name: None,
            readings: vec![],
            state: State::Disconnected,
            last_sync: None,
            next_sync: None,
            error: None,
        }
    }
}
pub enum Command {
    Connect(u64, Credentials),
    Disconnect(u64, Provider),
    Demo(u64),
    Restore(u64, Provider),
    SelectPatient(u64, String),
    Retry,
    Refresh,
    Save(Config),
    Preview(crate::alerts::Rule),
    Clear,
    Shutdown,
}
pub enum Event {
    Data(Snapshot),
    Alert(u64, Vec<crate::alerts::Kind>),
    Notice(String),
    Autostart(bool),
}
pub struct Service {
    pub commands: channel::UnboundedSender<Command>,
    pub events: mpsc::Receiver<Event>,
    join: Option<std::thread::JoinHandle<()>>,
    cancel: tokio::sync::watch::Sender<bool>,
}
impl Service {
    pub fn start(store: Store, config: Config, ctx: egui::Context) -> Result<Self, String> {
        Self::start_with_credentials(store, config, ctx, Box::new(storage::SystemCredentials))
    }
    fn start_with_credentials(
        store: Store,
        config: Config,
        ctx: egui::Context,
        vault: Box<dyn storage::CredentialStore>,
    ) -> Result<Self, String> {
        let (tx, rx) = channel::unbounded_channel();
        let (cancel, cancelled) = tokio::sync::watch::channel(false);
        let (events, receiver) = mpsc::channel();
        let join = std::thread::Builder::new()
            .name("easy-glucose-sync".into())
            .spawn(move || {
                let runtime = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build();
                match runtime {
                    Ok(rt) => rt.block_on(run(store, config, rx, events, ctx, cancelled, vault)),
                    Err(_) => {
                        let _ = events.send(Event::Notice(Text::ServiceStart.english().into()));
                        ctx.request_repaint_of(egui::ViewportId::ROOT);
                    }
                }
            })
            .map_err(|_| Text::CannotStartSync.english())?;
        Ok(Self {
            commands: tx,
            events: receiver,
            join: Some(join),
            cancel,
        })
    }
}
impl Drop for Service {
    fn drop(&mut self) {
        let _ = self.cancel.send(true);
        let _ = self.commands.send(Command::Shutdown);
        if let Some(join) = self.join.take() {
            let _ = join.join();
        }
    }
}
fn emit(tx: &mpsc::Sender<Event>, ctx: &egui::Context, event: Event) {
    let _ = tx.send(event);
    ctx.request_repaint_of(egui::ViewportId::ROOT);
}
pub fn backoff(failures: u32, error: &Error) -> u64 {
    if *error == Error::RateLimited {
        1800
    } else {
        (30_u64.saturating_mul(1_u64 << failures.min(5))).min(900)
    }
}

struct RateLimit {
    failures: u32,
    last_attempt: Option<Instant>,
    cooldown: Instant,
}
impl RateLimit {
    fn new() -> Self {
        Self {
            failures: 0,
            last_attempt: None,
            cooldown: Instant::now(),
        }
    }
}

async fn run(
    store: Store,
    mut config: Config,
    mut commands: channel::UnboundedReceiver<Command>,
    events: mpsc::Sender<Event>,
    ctx: egui::Context,
    mut cancelled: tokio::sync::watch::Receiver<bool>,
    vault: Box<dyn storage::CredentialStore>,
) {
    let mut snapshot = Snapshot::default();
    let mut client: Option<Client> = None;
    let mut due = Instant::now() + Duration::from_secs(86400);
    let mut demo = false;
    let mut provider = config.provider;
    let mut rate_limits = std::collections::HashMap::<Provider, RateLimit>::new();
    let mut persist_login = false;
    let mut monitor = crate::alerts::Monitor::default();
    let audio_sequence = std::sync::Arc::new(std::sync::atomic::AtomicU64::new(0));
    let play = |kinds: Vec<crate::alerts::Kind>, rules: Vec<crate::alerts::Rule>| {
        let ticket = audio_sequence.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
        crate::alert_audio::dispatch(
            kinds,
            rules,
            store.directory.clone(),
            events.clone(),
            ctx.clone(),
            audio_sequence.clone(),
            ticket,
        );
    };

    loop {
        use crate::alerts::Kind;
        let watch_gap = client.is_some()
            && !demo
            && (!snapshot.state.needs_account() || snapshot.last_sync.is_some())
            && config.alerts.rules[Kind::SignalLoss as usize].enabled;
        if watch_gap && monitor.missing(Utc::now(), &config.alerts) {
            let alerts = vec![Kind::SignalLoss];
            emit(
                &events,
                &ctx,
                Event::Alert(snapshot.generation, alerts.clone()),
            );
            play(
                alerts,
                vec![config.alerts.rules[Kind::SignalLoss as usize].clone()],
            );
        }
        let gap_due = monitor
            .missing_deadline(&config.alerts)
            .filter(|_| watch_gap);
        let wake = gap_due
            .map(|time| Instant::now() + (time - Utc::now()).to_std().unwrap_or_default())
            .unwrap_or(due);
        let command = tokio::select! {
            c=commands.recv()=>c,
            _=tokio::time::sleep_until(due)=>Some(Command::Refresh),
            _=tokio::time::sleep_until(wake), if gap_due.is_some() => continue
        };
        let Some(command) = command else {
            break;
        };
        if matches!(
            command,
            Command::Demo(_)
                | Command::Disconnect(..)
                | Command::Restore(..)
                | Command::Connect(..)
                | Command::SelectPatient(..)
        ) {
            monitor = crate::alerts::Monitor::default();
            audio_sequence.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        }
        match command {
            Command::Shutdown => {
                audio_sequence.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                break;
            }
            Command::Preview(rule) => {
                play(Vec::new(), vec![rule]);
                continue;
            }
            Command::Save(mut c) => {
                c.normalize();
                if c.autostart != config.autostart
                    && let Err(e) = crate::platform::set_autostart(c.autostart)
                {
                    emit(&events, &ctx, Event::Notice(e));
                    c.autostart = config.autostart;
                    emit(&events, &ctx, Event::Autostart(c.autostart));
                }
                let retention_changed = config.history_days != c.history_days;
                #[cfg(target_os = "macos")]
                if !config.alerts.rules.iter().any(|r| r.enabled)
                    && c.alerts.rules.iter().any(|r| r.enabled)
                {
                    let sender = events.clone();
                    let context = ctx.clone();
                    std::thread::spawn(move || {
                        if !matches!(notify_rust::request_auth_blocking(), Ok(true)) {
                            emit(
                                &sender,
                                &context,
                                Event::Notice(Text::NotificationError.english().into()),
                            );
                        }
                    });
                }
                monitor.settings_changed(&c.alerts);
                config = c;
                if let Err(e) = store.save_config(&config) {
                    emit(&events, &ctx, Event::Notice(e));
                }
                if !demo
                    && !persist_login
                    && retention_changed
                    && let Some(c) = client.as_ref()
                {
                    merge_readings(
                        &mut snapshot.readings,
                        vec![],
                        Utc::now(),
                        config.history_days,
                    );
                    if let Err(e) = store.save_source_cache(&c.credentials(), &snapshot.readings) {
                        emit(&events, &ctx, Event::Notice(e));
                    }
                    emit(&events, &ctx, Event::Data(snapshot.clone()));
                }
                continue;
            }
            Command::Clear => {
                if !demo && let Err(e) = store.clear_source_cache(provider) {
                    emit(&events, &ctx, Event::Notice(e));
                    continue;
                }
                snapshot.readings.clear();
                emit(&events, &ctx, Event::Data(snapshot.clone()));
                continue;
            }
            Command::Demo(g) => {
                client = None;
                demo = true;
                persist_login = false;
                snapshot = Snapshot {
                    generation: g,
                    provider,
                    state: State::Simulation,
                    readings: demo_readings(Utc::now()),
                    ..Snapshot::default()
                };
                due = Instant::now() + Duration::from_secs(300);
                emit(&events, &ctx, Event::Data(snapshot.clone()));
                continue;
            }
            Command::Disconnect(g, source) => {
                provider = source;
                client = None;
                demo = false;
                persist_login = false;
                snapshot = Snapshot {
                    generation: g,
                    provider,
                    ..Snapshot::default()
                };
                if let Err(e) = vault.delete(provider) {
                    snapshot.error = Some(e);
                }
                if let Err(e) = store.clear_source_cache(provider) {
                    snapshot.error = Some(e);
                }
                due = Instant::now() + Duration::from_secs(86400);
                emit(&events, &ctx, Event::Data(snapshot.clone()));
                continue;
            }
            Command::Restore(g, source) => {
                provider = source;
                demo = false;
                persist_login = false;
                client = None;
                snapshot = Snapshot {
                    generation: g,
                    provider,
                    ..Snapshot::default()
                };
                snapshot.state = State::Connecting;
                emit(&events, &ctx, Event::Data(snapshot.clone()));
                match vault.load(provider) {
                    Ok(Some((credentials, legacy))) => {
                        persist_login = legacy;
                        match store.load_source_cache(&credentials, config.history_days, legacy) {
                            Ok(h) => snapshot.readings = h,
                            Err(e) => emit(&events, &ctx, Event::Notice(e)),
                        }
                        match Client::new(credentials) {
                            Ok(c) => client = Some(c),
                            Err(e) => snapshot.error = Some(e.to_string()),
                        }
                    }
                    Ok(None) => {}
                    Err(e) => snapshot.error = Some(e),
                }
                if client.is_none() {
                    snapshot.state = State::Disconnected;
                    due = Instant::now() + Duration::from_secs(86400);
                    emit(&events, &ctx, Event::Data(snapshot.clone()));
                    continue;
                }
                snapshot.state = State::Connecting;
            }
            Command::Connect(g, credentials) => {
                provider = credentials.provider();
                demo = false;
                persist_login = true;
                client = None;
                snapshot = Snapshot {
                    generation: g,
                    provider,
                    state: State::Connecting,
                    ..Snapshot::default()
                };
                // A new attempt starts with no readings. The previous saved login is only
                // replaced after successful authentication; each cache verifies account identity.
                match Client::new(credentials) {
                    Ok(c) => client = Some(c),
                    Err(e) => {
                        snapshot.error = Some(e.to_string());
                        snapshot.state = State::after_result(&Err(e));
                        due = Instant::now() + Duration::from_secs(86400);
                        emit(&events, &ctx, Event::Data(snapshot.clone()));
                        continue;
                    }
                }
            }
            Command::SelectPatient(g, id) => {
                let Some(c) = client.as_mut() else {
                    continue;
                };
                if c.select_patient(&id).is_err() {
                    continue;
                }
                snapshot = Snapshot {
                    generation: g,
                    provider,
                    state: State::Connecting,
                    ..Snapshot::default()
                };
                persist_login = true;
                if let Some(rate) = rate_limits.get_mut(&provider) {
                    rate.last_attempt = None;
                    rate.cooldown = Instant::now();
                }
            }
            Command::Retry => {
                if client.is_none() {
                    continue;
                }
                snapshot.state = State::Connecting;
            }
            Command::Refresh => {
                if demo {
                    snapshot.readings = demo_readings(Utc::now());
                    snapshot.state = State::Simulation;
                    due = Instant::now() + Duration::from_secs(300);
                    emit(&events, &ctx, Event::Data(snapshot.clone()));
                    continue;
                }
                if client.is_none()
                    || matches!(
                        snapshot.state,
                        State::InvalidCredentials | State::ActionRequired | State::SelectingPatient
                    )
                {
                    due = Instant::now() + Duration::from_secs(86400);
                    continue;
                }
                snapshot.state = State::Syncing;
            }
        }
        let rate = rate_limits.entry(provider).or_insert_with(RateLimit::new);
        if Instant::now() < rate.cooldown {
            due = rate.cooldown;
            snapshot.error = Some(Text::RefreshLimit.english().into());
            snapshot.state = State::Offline;
            emit(&events, &ctx, Event::Data(snapshot.clone()));
            continue;
        }
        if let Some(last) = rate.last_attempt
            && last.elapsed() < Duration::from_secs(30)
        {
            due = last + Duration::from_secs(30);
            snapshot.next_sync = Some(Utc::now() + chrono::Duration::seconds(30));
            emit(&events, &ctx, Event::Data(snapshot.clone()));
            continue;
        }
        emit(&events, &ctx, Event::Data(snapshot.clone()));
        let Some(c) = client.as_mut() else {
            continue;
        };
        rate.last_attempt = Some(Instant::now());
        if *cancelled.borrow() {
            continue;
        }
        monitor.start(Utc::now());
        let result = tokio::select! {
            result = c.fetch() => result,
            _ = cancelled.changed() => continue,
        };
        snapshot.state = State::after_result(&result);
        snapshot.patients = c.patients();
        snapshot.patient_name = c.selected_patient();
        match result {
            Ok(readings) => {
                rate.failures = 0;
                snapshot.error = None;
                let now = Utc::now();
                snapshot.last_sync = Some(now);
                let kinds = monitor.reading_from(provider, &readings, now, &config.alerts);
                if !kinds.is_empty() {
                    emit(
                        &events,
                        &ctx,
                        Event::Alert(snapshot.generation, kinds.clone()),
                    );
                    let rules = kinds
                        .iter()
                        .map(|&kind| config.alerts.rules[kind as usize].clone())
                        .collect();
                    play(kinds, rules);
                }
                merge_readings(
                    &mut snapshot.readings,
                    readings,
                    Utc::now(),
                    config.history_days,
                );
                if persist_login {
                    if let Err(e) = vault.save(&c.credentials()) {
                        emit(&events, &ctx, Event::Notice(e));
                    }
                    persist_login = false;
                }
                if let Err(e) = store.save_source_cache(&c.credentials(), &snapshot.readings) {
                    emit(&events, &ctx, Event::Notice(e));
                }
                let delay = snapshot
                    .readings
                    .last()
                    .map(|r| {
                        (r.measured_at + chrono::Duration::seconds(provider.interval() as i64 + 10)
                            - Utc::now())
                        .num_seconds()
                        .clamp(60, provider.interval() as i64) as u64
                    })
                    .unwrap_or(provider.interval());
                due = Instant::now() + Duration::from_secs(delay);
            }
            Err(error) => {
                tracing::warn!("source_sync_failed");
                if error == Error::SelectPatient {
                    snapshot.readings.clear();
                }
                snapshot.error = Some(error.to_string());
                rate.failures = rate.failures.saturating_add(1);
                let delay = if error.requires_action() {
                    86400
                } else {
                    backoff(rate.failures - 1, &error)
                };
                due = Instant::now() + Duration::from_secs(delay);
                rate.cooldown = if error.requires_action() {
                    Instant::now() + Duration::from_secs(30)
                } else {
                    due
                };
            }
        }
        snapshot.next_sync = if matches!(
            snapshot.state,
            State::InvalidCredentials | State::ActionRequired | State::SelectingPatient
        ) {
            None
        } else {
            Some(
                Utc::now()
                    + chrono::Duration::seconds(
                        due.saturating_duration_since(Instant::now()).as_secs() as i64,
                    ),
            )
        };
        emit(&events, &ctx, Event::Data(snapshot.clone()));
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn states_and_backoff() {
        assert_eq!(State::after_result(&Ok(vec![])), State::Empty);
        assert_eq!(
            State::after_result(&Err(Error::Credentials)),
            State::InvalidCredentials
        );
        assert_eq!(State::after_result(&Err(Error::Network)), State::Offline);
        assert_eq!(backoff(0, &Error::Network), 30);
        assert_eq!(backoff(99, &Error::Network), 900);
        assert_eq!(backoff(0, &Error::RateLimited), 1800);
    }
    #[derive(Clone, Default)]
    struct MemoryVault(
        std::sync::Arc<std::sync::Mutex<std::collections::HashMap<Provider, Credentials>>>,
    );
    impl storage::CredentialStore for MemoryVault {
        fn load(&self, provider: Provider) -> Result<Option<(Credentials, bool)>, String> {
            Ok(self
                .0
                .lock()
                .unwrap()
                .get(&provider)
                .cloned()
                .map(|c| (c, false)))
        }
        fn save(&self, credentials: &Credentials) -> Result<(), String> {
            self.0
                .lock()
                .unwrap()
                .insert(credentials.provider(), credentials.clone());
            Ok(())
        }
        fn delete(&self, provider: Provider) -> Result<(), String> {
            self.0.lock().unwrap().remove(&provider);
            Ok(())
        }
    }
    async fn snapshot(service: &Service, generation: u64, state: State) -> Snapshot {
        tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                while let Ok(event) = service.events.try_recv() {
                    if let Event::Data(s) = event
                        && s.generation == generation
                        && s.state == state
                    {
                        return s;
                    }
                }
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        })
        .await
        .expect("service event")
    }
    #[tokio::test]
    async fn pruning_and_clearing_history_update_the_ui_and_preserve_sync() {
        use crate::sources::nightscout::{Auth, Credentials as NsCredentials};
        use storage::CredentialStore;
        let (url, requests) = crate::sources::test_support::server(vec![(
            200,
            serde_json::json!([
                {"sgv": 95, "date": (Utc::now() - chrono::Duration::days(2)).timestamp_millis()},
                {"sgv": 105, "date": Utc::now().timestamp_millis()}
            ]),
        )])
        .await;
        let saved = Credentials::Nightscout(NsCredentials {
            url,
            token: Default::default(),
            auth: Auth::Token,
        });
        let dir = tempfile::tempdir().unwrap();
        let store = Store {
            directory: dir.path().into(),
        };
        let vault = MemoryVault::default();
        let service = Service::start_with_credentials(
            store.clone(),
            Config::default(),
            egui::Context::default(),
            Box::new(vault.clone()),
        )
        .unwrap();
        service.commands.send(Command::Connect(1, saved)).unwrap();
        let before = snapshot(&service, 1, State::Connected).await;
        assert_eq!(before.readings.len(), 2);
        service
            .commands
            .send(Command::Save(Config {
                history_days: 1,
                ..Config::default()
            }))
            .unwrap();
        let pruned = snapshot(&service, 1, State::Connected).await;
        assert_eq!(pruned.readings, before.readings[1..]);
        assert_eq!(pruned.next_sync, before.next_sync);
        let (credentials, _) = vault.load(Provider::Nightscout).unwrap().unwrap();
        assert_eq!(
            store.load_source_cache(&credentials, 7, false).unwrap(),
            pruned.readings
        );
        service.commands.send(Command::Clear).unwrap();
        let after = snapshot(&service, 1, State::Connected).await;
        assert!(after.readings.is_empty());
        assert!(!after.state.needs_account());
        assert!(after.next_sync.is_some());
        assert_eq!(after.next_sync, before.next_sync);
        let (credentials, _) = vault.load(Provider::Nightscout).unwrap().unwrap();
        assert!(
            store
                .load_source_cache(&credentials, 7, false)
                .unwrap()
                .is_empty()
        );
        assert_eq!(requests.await.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn failed_replacement_preserves_saved_source_and_switching_clears_visible_data() {
        use crate::sources::nightscout::{Auth, Credentials as NsCredentials};
        use storage::CredentialStore;
        let (base, requests) =
            crate::sources::test_support::server(vec![(401, serde_json::json!({}))]).await;
        let saved = Credentials::Nightscout(NsCredentials {
            url: format!("{base}/old/"),
            token: zeroize::Zeroizing::new("old-token".into()),
            auth: Auth::Token,
        });
        let attempted = Credentials::Nightscout(NsCredentials {
            url: format!("{base}/new/"),
            token: zeroize::Zeroizing::new("new-token".into()),
            auth: Auth::Token,
        });
        let dir = tempfile::tempdir().unwrap();
        let store = Store {
            directory: dir.path().into(),
        };
        let readings = demo_readings(Utc::now());
        store.save_source_cache(&saved, &readings).unwrap();
        let vault = MemoryVault::default();
        vault.save(&saved).unwrap();
        let service = Service::start_with_credentials(
            store.clone(),
            Config::default(),
            egui::Context::default(),
            Box::new(vault.clone()),
        )
        .unwrap();
        service
            .commands
            .send(Command::Connect(1, attempted))
            .unwrap();
        let failed = snapshot(&service, 1, State::InvalidCredentials).await;
        assert!(failed.readings.is_empty());
        assert!(failed.next_sync.is_none());
        service
            .commands
            .send(Command::Save(Config {
                history_days: 14,
                ..Config::default()
            }))
            .unwrap();
        service
            .commands
            .send(Command::Restore(2, Provider::Dexcom))
            .unwrap();
        let switched = snapshot(&service, 2, State::Disconnected).await;
        assert_eq!(switched.provider, Provider::Dexcom);
        assert!(switched.readings.is_empty());
        assert_eq!(
            vault
                .load(Provider::Nightscout)
                .unwrap()
                .unwrap()
                .0
                .cache_identity(),
            saved.cache_identity()
        );
        assert_eq!(store.load_source_cache(&saved, 7, false).unwrap(), readings);
        service
            .commands
            .send(Command::Restore(3, Provider::Nightscout))
            .unwrap();
        let restored = snapshot(&service, 3, State::Offline).await;
        assert_eq!(restored.readings, readings);
        assert_eq!(restored.provider, Provider::Nightscout);
        assert_eq!(requests.await.unwrap().len(), 1);
    }
}

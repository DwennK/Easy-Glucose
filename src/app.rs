use crate::i18n::Text;
use crate::{
    platform::tray::{Action, Tray},
    services::{Command, Event, Service, Snapshot, State},
    sources::{Credentials, Provider, libre, nightscout},
    storage::{Config, Store},
    ui::{self, theme},
};
use eframe::egui::{self, RichText};
use std::time::{Duration, Instant};
use zeroize::Zeroize;

#[derive(Clone, Copy, PartialEq)]
pub enum Page {
    Dashboard,
    Settings,
    Onboarding,
}
pub struct App {
    pub diagnostics_start: Option<Instant>,
    pub ctx: egui::Context,
    pub config: Config,
    pub snapshot: Snapshot,
    pub page: Page,
    pub account_open: bool,
    pub settings_tab: usize,
    pub username: String,
    pub password: String,
    pub source_url: String,
    pub nightscout_auth: nightscout::Auth,
    pub last_refresh: Instant,
    pub confirm_clear: bool,
    pub reset_widget: bool,
    pub popover: ui::popover::Popover,
    service: Service,
    tray: Option<Tray>,
    generation: u64,
    dirty: Option<Instant>,
    notice: Option<String>,
    quit: bool,
    hidden: bool,
    first_frame: bool,
    widget_was_open: bool,
    background: bool,
}
impl App {
    pub fn new(
        cc: &eframe::CreationContext<'_>,
        store: Store,
        config: Config,
        notice: Option<String>,
        demo: bool,
        background: bool,
    ) -> Result<Self, String> {
        let ctx = cc.egui_ctx.clone();
        crate::i18n::activate(config.language.as_deref());
        theme::install(&ctx);
        theme::apply(&ctx, config.theme);
        let service = Service::start(store, config.clone(), ctx.clone())?;
        let (tray, notice) = match Tray::new(ctx.clone()) {
            Ok(tray) => (Some(tray), notice),
            Err(e) => (None, Some(e)),
        };
        let page = if config.onboarding_done {
            Page::Dashboard
        } else {
            Page::Onboarding
        };
        let provider = config.provider;
        let mut app = Self {
            diagnostics_start: None,
            ctx,
            config,
            snapshot: Snapshot {
                provider,
                state: State::Connecting,
                ..Snapshot::default()
            },
            page,
            account_open: false,
            settings_tab: 0,
            username: String::new(),
            password: String::new(),
            source_url: String::new(),
            nightscout_auth: nightscout::Auth::Token,
            last_refresh: Instant::now() - Duration::from_secs(31),
            confirm_clear: false,
            reset_widget: false,
            popover: ui::popover::Popover::default(),
            service,
            tray,
            generation: 0,
            dirty: None,
            notice,
            quit: false,
            hidden: false,
            first_frame: true,
            widget_was_open: false,
            background,
        };
        if demo {
            app.demo();
        } else {
            app.send(Command::Restore(0, app.config.provider));
        }
        Ok(app)
    }
    pub fn apply_language(&mut self) {
        crate::i18n::activate(self.config.language.as_deref());
        if let Some(tray) = &self.tray {
            tray.localize();
        }
        self.ctx.request_repaint();
    }
    pub fn send(&mut self, command: Command) {
        if self.service.commands.send(command).is_err() {
            self.notice = Some(Text::ServiceStopped.english().into());
        }
    }
    pub fn mark_config(&mut self) {
        self.dirty = Some(Instant::now());
        self.ctx.request_repaint_of(egui::ViewportId::ROOT);
    }
    pub fn refresh(&mut self) {
        if self.last_refresh.elapsed() >= Duration::from_secs(30) {
            self.last_refresh = Instant::now();
            self.send(Command::Refresh);
        }
    }
    pub fn connect(&mut self) {
        self.generation += 1;
        self.snapshot = Snapshot {
            generation: self.generation,
            provider: self.config.provider,
            state: State::Connecting,
            ..Snapshot::default()
        };
        let password = zeroize::Zeroizing::new(std::mem::take(&mut self.password));
        let credentials = match self.config.provider {
            Provider::Dexcom => Credentials::Dexcom(crate::dexcom::Credentials {
                username: self.username.trim().into(),
                password,
                region: self.config.region,
            }),
            Provider::LibreLinkUp => Credentials::LibreLinkUp(libre::Credentials {
                username: self.username.trim().into(),
                password,
                patient_id: None,
            }),
            Provider::Nightscout => Credentials::Nightscout(nightscout::Credentials {
                url: self.source_url.trim().into(),
                token: password,
                auth: self.nightscout_auth,
            }),
        };
        self.send(Command::Connect(self.generation, credentials));
    }
    pub fn disconnect(&mut self) {
        self.generation += 1;
        self.snapshot = Snapshot {
            generation: self.generation,
            provider: self.config.provider,
            ..Snapshot::default()
        };
        self.password.zeroize();
        self.username.clear();
        self.send(Command::Disconnect(self.generation, self.config.provider));
    }
    pub fn demo(&mut self) {
        self.generation += 1;
        self.snapshot = Snapshot {
            generation: self.generation,
            provider: self.config.provider,
            state: State::Simulation,
            ..Snapshot::default()
        };
        self.send(Command::Demo(self.generation));
        self.config.onboarding_done = true;
        self.page = Page::Dashboard;
        self.mark_config();
    }
    pub fn restore(&mut self) {
        self.generation += 1;
        self.snapshot = Snapshot {
            generation: self.generation,
            provider: self.config.provider,
            state: State::Connecting,
            ..Snapshot::default()
        };
        self.send(Command::Restore(self.generation, self.config.provider));
    }
    pub fn select_source(&mut self, provider: Provider) {
        self.config.provider = provider;
        self.username.clear();
        self.password.zeroize();
        self.source_url.clear();
        self.nightscout_auth = nightscout::Auth::Token;
        self.restore();
        self.mark_config();
    }
    pub fn select_patient(&mut self, id: String) {
        self.generation += 1;
        self.snapshot = Snapshot {
            generation: self.generation,
            provider: self.config.provider,
            state: State::Connecting,
            ..Snapshot::default()
        };
        self.send(Command::SelectPatient(self.generation, id));
    }
    pub fn show_dashboard(&mut self) {
        self.popover.close(Instant::now());
        self.ctx.request_repaint_of(egui::ViewportId::ROOT);
        self.hidden = false;
        self.page = Page::Dashboard;
        self.ctx
            .send_viewport_cmd_to(egui::ViewportId::ROOT, egui::ViewportCommand::Visible(true));
        self.ctx.send_viewport_cmd_to(
            egui::ViewportId::ROOT,
            egui::ViewportCommand::Minimized(false),
        );
        self.ctx
            .send_viewport_cmd_to(egui::ViewportId::ROOT, egui::ViewportCommand::Focus);
    }
    fn events(&mut self) {
        while let Ok(event) = self.service.events.try_recv() {
            match event {
                Event::Notice(e) => self.notice = Some(e),
                Event::Autostart(value) => self.config.autostart = value,
                Event::Data(s) if s.generation == self.generation => {
                    if matches!(s.state, State::Connected | State::Empty)
                        && !self.config.onboarding_done
                    {
                        self.config.onboarding_done = true;
                        self.page = Page::Dashboard;
                        self.mark_config();
                    }
                    self.snapshot = s;
                }
                _ => {}
            }
        }
        let mut actions = vec![];
        if let Some(tray) = &self.tray {
            while let Ok(a) = tray.events.try_recv() {
                actions.push(a);
            }
        }
        for action in actions {
            match action {
                Action::Dashboard => self.show_dashboard(),
                Action::Settings => {
                    self.show_dashboard();
                    self.page = Page::Settings;
                }
                #[cfg(target_os = "macos")]
                Action::Popover => {
                    if let Some(position) = self
                        .tray
                        .as_ref()
                        .and_then(|t| t.popover_position(ui::popover::SIZE))
                    {
                        self.popover.toggle(position, Instant::now());
                    }
                }
                Action::Widget => {
                    self.config.widget_open = !self.config.widget_open;
                    self.mark_config();
                }
                Action::Quit => {
                    self.quit = true;
                    self.ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
            }
        }
        if let Some(changed) = self.dirty {
            let remaining = Duration::from_millis(500).saturating_sub(changed.elapsed());
            if remaining.is_zero() {
                self.send(Command::Save(self.config.clone()));
                self.dirty = None;
            } else {
                // Intermediate frames can replace an earlier repaint deadline.
                self.ctx
                    .request_repaint_after_for(remaining, egui::ViewportId::ROOT);
            }
        }
    }
    fn navigation(&mut self, ctx: &egui::Context) {
        let p = theme::palette(ctx);
        egui::TopBottomPanel::top("navigation")
            .frame(
                egui::Frame::new()
                    .fill(p.surface)
                    .inner_margin(egui::Margin::symmetric(28, 16)),
            )
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui::logo(ui, 32.0);
                    ui.label(RichText::new("Easy Glucose").size(20.0).strong());
                    if self.page != Page::Onboarding {
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui::choice(
                                ui,
                                &mut self.page,
                                Page::Settings,
                                Text::Settings.get(),
                                150.0,
                            );
                            ui::choice(
                                ui,
                                &mut self.page,
                                Page::Dashboard,
                                Text::Dashboard.get(),
                                170.0,
                            );
                        });
                    }
                });
            });
    }
}
impl eframe::App for App {
    fn clear_color(&self, _: &egui::Visuals) -> [f32; 4] {
        // Unpainted corners of the rounded menu-bar window must remain transparent.
        [0.0; 4]
    }
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if let Some(started) = self.diagnostics_start.take() {
            eprintln!(
                "diagnostic: first UI callback after {:.2} ms",
                started.elapsed().as_secs_f64() * 1000.0
            );
        }
        self.events();
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::Comma)) {
            self.page = Page::Settings;
        }
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::R)) {
            self.refresh();
        }
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::W)) {
            self.config.widget_open = !self.config.widget_open;
            self.mark_config();
        }
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) && !self.account_open {
            self.page = Page::Dashboard;
        }
        if self.first_frame {
            self.first_frame = false;
            // Keep the shared alpha-capable GL configuration selected at startup,
            // but restore AppKit's opaque background (including the title bar)
            // for the main window only. The rounded popup stays transparent.
            #[cfg(target_os = "macos")]
            ctx.send_viewport_cmd_to(
                egui::ViewportId::ROOT,
                egui::ViewportCommand::Transparent(false),
            );
            if (self.config.start_minimized || self.background)
                && self.tray.is_some()
                && self.config.onboarding_done
            {
                self.hidden = true;
                ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
            }
        }
        if ctx.input(|i| i.viewport().close_requested())
            && !self.quit
            && self.config.close_to_tray
            && self.tray.is_some()
        {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
            self.hidden = true;
        }
        if self.config.minimize_to_tray
            && self.tray.is_some()
            && ctx.input(|i| i.viewport().minimized == Some(true))
        {
            ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
            self.hidden = true;
        }
        self.navigation(ctx);
        egui::CentralPanel::default()
            .frame(
                egui::Frame::new()
                    .fill(theme::palette(ctx).bg)
                    .inner_margin(egui::Margin::symmetric(24, 24)),
            )
            .show(ctx, |ui| {
                if let Some(message) = self.notice.clone() {
                    egui::Frame::new()
                        .fill(theme::ORANGE.gamma_multiply(0.1))
                        .inner_margin(10)
                        .corner_radius(8)
                        .show(ui, |ui| {
                            ui.horizontal_wrapped(|ui| {
                                ui.label(crate::i18n::notice(&message));
                                if ui.small_button(Text::Close.get()).clicked() {
                                    self.notice = None;
                                }
                            });
                        });
                    ui.add_space(10.0);
                }
                if self.page == Page::Settings {
                    let available = ui.available_width();
                    ui.horizontal(|ui| {
                        ui.add_space(((available - 800.0) / 2.0).max(0.0));
                        ui.vertical(|ui| {
                            ui.set_width(available.min(800.0));
                            ui::settings::header(self, ui);
                        });
                    });
                    ui.add_space(12.0);
                }
                egui::ScrollArea::vertical()
                    .id_salt((
                        match self.page {
                            Page::Dashboard => "dashboard",
                            Page::Settings => "settings",
                            Page::Onboarding => "onboarding",
                        },
                        if self.page == Page::Settings {
                            self.settings_tab
                        } else {
                            0
                        },
                    ))
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        let width = match self.page {
                            Page::Dashboard => 640.0,
                            Page::Settings => 800.0,
                            Page::Onboarding => 620.0,
                        };
                        let available = ui.available_width();
                        ui.horizontal(|ui| {
                            ui.add_space(((available - width) / 2.0).max(0.0));
                            ui.vertical(|ui| {
                                ui.set_width(available.min(width));
                                match self.page {
                                    Page::Dashboard => ui::dashboard::show(self, ui),
                                    Page::Settings => ui::settings::show(self, ui),
                                    Page::Onboarding => ui::settings::onboarding(self, ui),
                                }
                            });
                        });
                    });
            });
        ui::settings::account_dialog(self, ctx);
        if self.config.widget_open {
            let id = egui::ViewportId::from_hash_of("glucose_widget");
            let mut builder = egui::ViewportBuilder::default()
                .with_title(format!("Easy Glucose · {}", Text::Widget.get()))
                .with_movable_by_background(true)
                .with_decorations(false)
                .with_resizable(false)
                .with_inner_size(self.config.widget_size.dimensions())
                .with_always_on_top();
            if !self.config.always_on_top {
                builder = builder.with_window_level(egui::WindowLevel::Normal);
            }
            // Restore once per opening. Feeding live coordinates back into the builder
            // can issue new native move commands while the user is dragging.
            if !self.widget_was_open
                && let Some(pos) = self.config.widget_position
            {
                builder = builder.with_position(pos);
            }
            if self.reset_widget {
                ctx.send_viewport_cmd_to(
                    id,
                    egui::ViewportCommand::OuterPosition(egui::pos2(80.0, 80.0)),
                );
                self.reset_widget = false;
            }
            ctx.show_viewport_immediate(id, builder, |ctx, _| {
                ui::widget::show(self, ctx);
            });
        }
        self.widget_was_open = self.config.widget_open;
        if self.popover.open {
            let id = egui::ViewportId::from_hash_of("menubar_graph");
            let builder = egui::ViewportBuilder::default()
                .with_title(format!("Easy Glucose · {}", Text::Preview.get()))
                .with_transparent(true)
                .with_decorations(false)
                .with_resizable(false)
                .with_taskbar(false)
                .with_active(true)
                .with_always_on_top()
                .with_inner_size(ui::popover::SIZE)
                .with_position(self.popover.position);
            ctx.show_viewport_immediate(id, builder, |ctx, _| ui::popover::show(self, ctx));
        }
        if let Some(tray) = &mut self.tray {
            let title = self
                .snapshot
                .readings
                .last()
                .map(|r| {
                    let mut parts = vec![r.display(self.config.unit), r.trend.arrow().into()];
                    if r.stale(chrono::Utc::now()) {
                        parts.push(Text::Stale.get().into());
                    }
                    if self.snapshot.state == State::Simulation {
                        parts.push(Text::Demo.get().into());
                    }
                    parts.join(" ")
                })
                .unwrap_or("—".into());
            let tooltip = format!(
                "Easy Glucose · {title} {}\n{}\n{}",
                self.config.unit.label(),
                self.snapshot.state.label(),
                self.snapshot
                    .readings
                    .last()
                    .map(ui::dashboard::freshness)
                    .unwrap_or_default()
            );
            tray.update(&title, &tooltip);
        }
        // One age update per minute, including when the dashboard is hidden. No animation loop.
        ctx.request_repaint_after(Duration::from_secs(30));
    }
    fn on_exit(&mut self, _: Option<&eframe::glow::Context>) {
        self.password.zeroize();
        self.send(Command::Save(self.config.clone()));
        self.send(Command::Shutdown);
    }
}

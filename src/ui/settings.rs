use crate::i18n::Text;
use crate::{
    app::{App, Page},
    core::Unit,
    dexcom::Region,
    services::{Command, State},
    sources::{Provider, nightscout::Auth},
    storage::{Theme, WidgetSize},
    ui::{self, Segment, theme},
};
use eframe::egui::{self, RichText};

pub fn account(app: &mut App, ui: &mut egui::Ui) {
    let previous = app.config.provider;
    row(ui, Text::Provider.get(), |ui| {
        egui::ComboBox::from_id_salt("source_provider")
            .width(ui.available_width().min(300.0))
            .selected_text(app.config.provider.label())
            .show_ui(ui, |ui| {
                for provider in Provider::ALL {
                    if ui
                        .selectable_value(&mut app.config.provider, provider, provider.label())
                        .clicked()
                    {
                        ui.close();
                    }
                }
            });
    });
    if app.config.provider != previous {
        app.select_source(app.config.provider);
    }
    let connected = matches!(app.snapshot.state, State::Connected | State::Empty)
        || (app.snapshot.state == State::Syncing && app.snapshot.last_sync.is_some());
    if connected {
        ui.horizontal_wrapped(|ui| {
            ui::pill(
                ui,
                &format!(
                    "✓  {} · {}",
                    app.config.provider.label(),
                    Text::Connected.get()
                ),
                theme::GREEN,
            );
            if app.snapshot.state != State::Connected {
                ui::muted(ui, app.snapshot.state.label());
            }
        });
    } else {
        ui::muted(
            ui,
            format!(
                "{} · {}",
                app.config.provider.label(),
                app.snapshot.state.label()
            ),
        );
    }
    if let Some(name) = &app.snapshot.patient_name {
        ui::muted(ui, format!("{} · {name}", Text::Following.get()));
    }
    if let Some(error) = &app.snapshot.error {
        ui.colored_label(
            theme::ink(ui.ctx(), theme::ORANGE),
            crate::i18n::notice(error),
        );
    }
    if let Some(next) = app.snapshot.next_sync {
        ui::muted(
            ui,
            format!(
                "{} · {}",
                Text::NextSync.get(),
                next.with_timezone(&chrono::Local).format("%H:%M")
            ),
        );
    }
    if connected {
        ui.collapsing(Text::ChangeAccount.get(), |ui| sign_in_form(app, ui));
    } else {
        sign_in_form(app, ui);
    }
    if !matches!(app.snapshot.state, State::Disconnected | State::Simulation)
        && ui.button(Text::Disconnect.get()).clicked()
    {
        app.disconnect();
    }
}

fn sign_in_form(app: &mut App, ui: &mut egui::Ui) {
    ui.add_space(4.0);
    ui::muted(
        ui,
        match app.config.provider {
            Provider::Dexcom => Text::DexcomHelp.get(),
            Provider::LibreLinkUp => Text::LibreHelp.get(),
            Provider::Nightscout => Text::NightscoutHelp.get(),
        },
    );
    if app.snapshot.state == State::SelectingPatient {
        ui.add_space(10.0);
        ui.label(RichText::new(Text::Who.get()).strong());
        for (index, patient) in app.snapshot.patients.clone().into_iter().enumerate() {
            let label = if patient.name.is_empty() {
                format!("{} {}", Text::Person.get(), index + 1)
            } else {
                patient.name
            };
            if ui
                .add_sized([ui.available_width(), 30.0], egui::Button::new(label))
                .clicked()
            {
                app.select_patient(patient.id);
            }
        }
        if ui.button(Text::CancelConnection.get()).clicked() {
            app.restore();
        }
        return;
    }
    ui.add_space(4.0);
    if app.config.provider == Provider::Dexcom {
        row(ui, Text::Region.get(), |ui| {
            egui::ComboBox::from_id_salt("region")
                .selected_text(app.config.region.label())
                .width(ui.available_width().min(300.0))
                .show_ui(ui, |ui| {
                    for region in [Region::International, Region::Us, Region::Japan] {
                        if ui
                            .selectable_value(&mut app.config.region, region, region.label())
                            .clicked()
                        {
                            ui.close();
                        }
                    }
                });
        });
    }
    let nightscout = app.config.provider == Provider::Nightscout;
    if nightscout {
        ui.label(Text::SiteAddress.get());
        ui.add(
            egui::TextEdit::singleline(&mut app.source_url)
                .hint_text("https://example.org")
                .desired_width(f32::INFINITY)
                .margin(egui::vec2(8.0, 6.0)),
        );
        row(ui, Text::Access.get(), |ui| {
            egui::ComboBox::from_id_salt("nightscout_auth")
                .width(ui.available_width().min(300.0))
                .selected_text(if app.nightscout_auth == Auth::Token {
                    Text::ReadToken.get()
                } else {
                    Text::ApiSecret.get()
                })
                .show_ui(ui, |ui| {
                    if ui
                        .selectable_value(
                            &mut app.nightscout_auth,
                            Auth::Token,
                            Text::ReadToken.get(),
                        )
                        .clicked()
                    {
                        ui.close();
                    }
                    if ui
                        .selectable_value(
                            &mut app.nightscout_auth,
                            Auth::ApiSecret,
                            Text::ApiSecret.get(),
                        )
                        .clicked()
                    {
                        ui.close();
                    }
                });
        });
        ui.label(if app.nightscout_auth == Auth::Token {
            Text::OptionalToken.get()
        } else {
            Text::SiteSecret.get()
        });
    } else {
        ui.label(if app.config.provider == Provider::Dexcom {
            Text::DexcomLogin.get()
        } else {
            Text::LibreEmail.get()
        });
        ui.add(
            egui::TextEdit::singleline(&mut app.username)
                .hint_text(if app.config.provider == Provider::Dexcom {
                    Text::LoginHint.get()
                } else {
                    "name@example.org"
                })
                .desired_width(f32::INFINITY)
                .margin(egui::vec2(8.0, 6.0)),
        );
        ui.label(Text::Password.get());
    }
    let password = ui.add(
        egui::TextEdit::singleline(&mut app.password)
            .password(true)
            .hint_text(if nightscout {
                Text::TokenHint.get()
            } else {
                Text::PasswordHint.get()
            })
            .desired_width(f32::INFINITY)
            .margin(egui::vec2(8.0, 6.0)),
    );
    ui::muted(ui, Text::KeyringHelp.get());
    let ready = if nightscout {
        !app.source_url.trim().is_empty()
            && (app.nightscout_auth == Auth::Token || !app.password.is_empty())
    } else {
        !app.username.trim().is_empty() && !app.password.is_empty()
    };
    let busy = matches!(app.snapshot.state, State::Connecting | State::Syncing);
    ui.horizontal(|ui| {
        let click = ui
            .add_enabled_ui(ready && !busy, |ui| ui::primary(ui, Text::Connect.get()))
            .inner
            .clicked();
        if click
            || (ready
                && !busy
                && password.lost_focus()
                && ui.input(|i| i.key_pressed(egui::Key::Enter)))
        {
            app.connect();
        }
        if matches!(
            app.snapshot.state,
            State::ActionRequired | State::InvalidCredentials | State::Offline
        ) && ui
            .add_enabled(
                app.last_refresh.elapsed().as_secs() >= 30,
                egui::Button::new(Text::Retry.get()),
            )
            .clicked()
        {
            app.last_refresh = std::time::Instant::now();
            app.send(Command::Retry);
        }
    });
}

pub fn onboarding(app: &mut App, ui: &mut egui::Ui) {
    ui.add_space(8.0);
    language_picker(app, ui);
    ui.add_space(12.0);
    ui.heading(Text::Welcome.get());
    ui::muted(ui, Text::WelcomeHint.get());
    ui.add_space(16.0);
    ui::group(ui, Text::Sources.get(), |ui| account(app, ui));
    ui.add_space(10.0);
    ui.horizontal(|ui| {
        if ui.button(Text::TryDemo.get()).clicked() {
            app.demo();
        }
        if ui.button(Text::Later.get()).clicked() {
            app.config.onboarding_done = true;
            app.page = Page::Dashboard;
            app.mark_config();
        }
    });
    ui.add_space(12.0);
    ui::muted(ui, Text::SecondaryUse.get());
}

pub fn header(app: &mut App, ui: &mut egui::Ui) {
    ui::segments(
        ui,
        &mut app.settings_tab,
        &[
            (0, Segment::Text(Text::Display.get())),
            (1, Segment::Text(Text::Sources.get())),
            (2, Segment::Text(Text::Advanced.get())),
        ],
    );
}

pub fn show(app: &mut App, ui: &mut egui::Ui) {
    let before = app.config.clone();
    match app.settings_tab {
        0 => display(app, ui),
        1 => {
            ui::group(ui, Text::Sources.get(), |ui| account(app, ui));
        }
        _ => advanced(app, ui),
    }
    if before != app.config {
        app.config.normalize();
        theme::apply(&app.ctx, app.config.theme);
        app.mark_config();
    }
}

fn display(app: &mut App, ui: &mut egui::Ui) {
    ui::group(ui, Text::AppearanceUnits.get(), |ui| {
        language_picker(app, ui);
        row(ui, Text::Appearance.get(), |ui| {
            ui::segments(
                ui,
                &mut app.config.theme,
                &[
                    (Theme::Light, Segment::Text(Text::Light.get())),
                    (Theme::Dark, Segment::Text(Text::Dark.get())),
                    (Theme::System, Segment::Text(Text::Automatic.get())),
                ],
            );
        });
        row(ui, Text::Units.get(), |ui| {
            ui::segments(
                ui,
                &mut app.config.unit,
                &[
                    (Unit::Mmol, Segment::Text(Unit::Mmol.label())),
                    (Unit::Mgdl, Segment::Text(Unit::Mgdl.label())),
                ],
            );
        });
    });
    ui.add_space(6.0);
    ui::group(ui, Text::Widget.get(), |ui| {
        ui::switch(
            ui,
            &mut app.config.widget_open,
            Text::ShowWidget.get(),
            Text::WidgetHint.get(),
        );
        row(ui, Text::Size.get(), |ui| {
            let sizes = [
                WidgetSize::Compact,
                WidgetSize::Normal,
                WidgetSize::Extended,
            ];
            ui::segments(
                ui,
                &mut app.config.widget_size,
                &sizes.map(|size| (size, Segment::Window(size))),
            );
        });
        ui::switch(ui, &mut app.config.always_on_top, Text::OnTop.get(), "");
        if app.config.widget_open && ui.button(Text::ResetWidget.get()).clicked() {
            app.config.widget_position = Some([80.0, 80.0]);
            app.reset_widget = true;
        }
    });
}

pub fn account_dialog(app: &mut App, ctx: &egui::Context) {
    if !app.account_open {
        return;
    }
    let before = app.config.clone();
    let modal = egui::Modal::new(egui::Id::new("account_dialog"))
        .frame(egui::Frame::window(&ctx.style()).inner_margin(20))
        .show(ctx, |ui| {
            ui.set_width((ctx.content_rect().width() - 64.0).min(440.0));
            ui.horizontal(|ui| {
                ui.label(RichText::new(Text::Sources.get()).size(18.0).strong());
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button(Text::Close.get()).clicked() {
                        app.account_open = false;
                    }
                });
            });
            ui.add_space(4.0);
            egui::ScrollArea::vertical()
                .max_height((ctx.content_rect().height() - 88.0).max(150.0))
                .show(ui, |ui| account(app, ui));
        });
    if modal.should_close() {
        app.account_open = false;
    }
    if before != app.config {
        app.mark_config();
    }
}

fn advanced(app: &mut App, ui: &mut egui::Ui) {
    ui::group(ui, Text::Startup.get(), |ui| {
        ui::switch(ui, &mut app.config.autostart, Text::LoginStart.get(), "");
        ui::switch(
            ui,
            &mut app.config.start_minimized,
            Text::StartHidden.get(),
            Text::TrayHint.get(),
        );
        ui::switch(
            ui,
            &mut app.config.close_to_tray,
            Text::KeepRunning.get(),
            Text::BackgroundHint.get(),
        );
        ui::switch(
            ui,
            &mut app.config.minimize_to_tray,
            Text::HideMinimized.get(),
            "",
        );
    });
    ui.add_space(8.0);
    ui::group(ui, Text::ChartGuides.get(), |ui| {
        ui::muted(ui, Text::GuidesHint.get());
        let max_low = app.config.high.saturating_sub(1).min(200);
        bound(
            ui,
            Text::LowGuide.get(),
            &mut app.config.low,
            40,
            max_low,
            app.config.unit,
        );
        bound(
            ui,
            Text::HighGuide.get(),
            &mut app.config.high,
            app.config.low + 1,
            400,
            app.config.unit,
        );
    });
    ui.add_space(8.0);
    ui::group(ui, Text::LocalHistory.get(), |ui| {
        row(ui, Text::Retention.get(), |ui| {
            egui::ComboBox::from_id_salt("history_days")
                .selected_text(format!(
                    "{} · {}",
                    Text::Days.get(),
                    app.config.history_days
                ))
                .show_ui(ui, |ui| {
                    for days in 1..=30 {
                        ui.selectable_value(&mut app.config.history_days, days, days.to_string());
                    }
                });
        });
        ui::muted(
            ui,
            format!(
                "{} · {}. {}",
                Text::Count.get(),
                app.snapshot.readings.len(),
                Text::Unencrypted.get()
            ),
        );
        if !app.confirm_clear {
            if ui.button(Text::DeleteHistory.get()).clicked() {
                app.confirm_clear = true;
            }
        } else {
            ui.label(Text::DeleteQuestion.get());
            ui.horizontal(|ui| {
                if ui.button(Text::Cancel.get()).clicked() {
                    app.confirm_clear = false;
                }
                if ui
                    .button(
                        RichText::new(Text::DeleteReadings.get())
                            .color(theme::ink(ui.ctx(), theme::RED)),
                    )
                    .clicked()
                {
                    app.send(Command::Clear);
                    app.confirm_clear = false;
                }
            });
            ui::muted(ui, Text::ReloadHint.get());
        }
    });
    ui.add_space(8.0);
    ui::group(ui, Text::Demo.get(), |ui| {
        ui::muted(ui, Text::DemoHint.get());
        if app.snapshot.state == State::Simulation {
            if ui.button(Text::ExitDemo.get()).clicked() {
                app.restore();
            }
        } else if ui.button(Text::TryDemo.get()).clicked() {
            app.demo();
        }
    });
    ui.add_space(8.0);
    ui::group(ui, Text::About.get(), |ui| {
        ui::muted(
            ui,
            format!(
                "Easy Glucose {} · {} · {}",
                env!("CARGO_PKG_VERSION"),
                Text::NativeApp.get(),
                Text::License.get()
            ),
        );
        ui::muted(
            ui,
            format!("{} {}", Text::SecondaryUse.get(), Text::Protocols.get()),
        );
        ui::muted(ui, Text::Schedule.get());
        ui::muted(
            ui,
            format!(
                "{} · ⌘/Ctrl + , ({}) · R ({}) · W ({})",
                Text::Shortcuts.get(),
                Text::Settings.get(),
                Text::Refresh.get(),
                Text::Widget.get()
            ),
        );
    });
}

fn row(ui: &mut egui::Ui, label: &str, content: impl FnOnce(&mut egui::Ui)) {
    ui.horizontal(|ui| {
        ui.set_min_height(ui::SETTINGS_ROW_HEIGHT);
        ui.label(label);
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), content);
    });
}
fn bound(ui: &mut egui::Ui, label: &str, value: &mut u16, min: u16, max: u16, unit: Unit) {
    row(ui, label, |ui| {
        let width = if unit == Unit::Mmol { 250.0 } else { 162.0 };
        ui.allocate_ui_with_layout(
            egui::vec2(width, 26.0),
            egui::Layout::left_to_right(egui::Align::Center),
            |ui| {
                if ui
                    .add_enabled(
                        *value > min,
                        egui::Button::new("−").min_size(egui::vec2(26.0, 26.0)),
                    )
                    .on_hover_text(Text::Decrease.get())
                    .clicked()
                {
                    *value = value.saturating_sub(1).max(min);
                }
                ui.add_sized(
                    [84.0, 26.0],
                    egui::DragValue::new(value)
                        .range(min..=max)
                        .suffix(" mg/dL")
                        .speed(0.0),
                )
                .on_hover_text(Text::EnterValue.get());
                if ui
                    .add_enabled(
                        *value < max,
                        egui::Button::new("+").min_size(egui::vec2(26.0, 26.0)),
                    )
                    .on_hover_text(Text::Increase.get())
                    .clicked()
                {
                    *value = value.saturating_add(1).min(max);
                }
                if unit == Unit::Mmol {
                    ui::muted(ui, format!("{} mmol/L", Unit::Mmol.format((*value).into())));
                }
            },
        );
    });
}

fn language_picker(app: &mut App, ui: &mut egui::Ui) {
    let previous = app.config.language.clone();
    row(ui, Text::Language.get(), |ui| {
        let name = crate::i18n::LANGUAGES
            .iter()
            .find(|(id, _)| Some(*id) == previous.as_deref())
            .map_or(Text::SystemLanguage.get(), |(_, name)| *name);
        egui::ComboBox::from_id_salt("interface_language")
            .selected_text(name)
            .width(208.0)
            .height(360.0)
            .show_ui(ui, |ui| {
                ui.spacing_mut().interact_size.y = 28.0;
                for &(code, name) in crate::i18n::LANGUAGES {
                    ui.selectable_value(&mut app.config.language, Some(code.to_owned()), name);
                }
                ui.separator();
                ui.selectable_value(&mut app.config.language, None, Text::SystemLanguage.get());
            });
    });
    if previous != app.config.language {
        app.apply_language();
        app.mark_config();
    }
}

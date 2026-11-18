use crate::{
    app::{App, Page},
    core::Unit,
    dexcom::Region,
    services::{Command, State},
    sources::{Provider, nightscout::Auth},
    storage::{Theme, WidgetSize},
    ui::{self, theme},
};
use eframe::egui::{self, RichText};

pub fn account(app: &mut App, ui: &mut egui::Ui) {
    let previous = app.config.provider;
    ui.horizontal(|ui| {
        ui.label("Fournisseur");
        egui::ComboBox::from_id_salt("source_provider")
            .width(340.0)
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
    ui.add_space(4.0);
    ui::muted(
        ui,
        match app.config.provider {
            Provider::Dexcom => {
                "Compte du porteur du capteur. Activez Share et ajoutez un abonné dans Dexcom."
            }
            Provider::LibreLinkUp => {
                "Utilisez votre compte LibreLinkUp après avoir accepté une invitation de partage."
            }
            Provider::Nightscout => {
                "Connectez le site Nightscout ou Nocturne qui reçoit déjà vos mesures."
            }
        },
    );
    if app.snapshot.state == State::SelectingPatient {
        ui.add_space(10.0);
        ui.label(RichText::new("Quelle personne souhaitez-vous suivre ?").strong());
        for patient in app.snapshot.patients.clone() {
            if ui
                .add_sized(
                    [ui.available_width(), 44.0],
                    egui::Button::new(&patient.name),
                )
                .clicked()
            {
                app.select_patient(patient.id);
            }
        }
        if ui.button("Annuler la connexion").clicked() {
            app.restore();
        }
        return;
    }
    ui.add_space(4.0);
    if app.config.provider == Provider::Dexcom {
        ui.horizontal(|ui| {
            ui.label("Région");
            egui::ComboBox::from_id_salt("region")
                .selected_text(app.config.region.label())
                .width(340.0)
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
        ui.label("Adresse du site");
        ui.add(
            egui::TextEdit::singleline(&mut app.source_url)
                .hint_text("https://mon-site.example")
                .desired_width(f32::INFINITY)
                .margin(egui::vec2(12.0, 10.0)),
        );
        ui.horizontal(|ui| {
            ui.label("Accès");
            egui::ComboBox::from_id_salt("nightscout_auth")
                .width(300.0)
                .selected_text(if app.nightscout_auth == Auth::Token {
                    "Jeton de lecture"
                } else {
                    "API secret"
                })
                .show_ui(ui, |ui| {
                    if ui
                        .selectable_value(&mut app.nightscout_auth, Auth::Token, "Jeton de lecture")
                        .clicked()
                    {
                        ui.close();
                    }
                    if ui
                        .selectable_value(&mut app.nightscout_auth, Auth::ApiSecret, "API secret")
                        .clicked()
                    {
                        ui.close();
                    }
                });
        });
        ui.label(if app.nightscout_auth == Auth::Token {
            "Jeton — facultatif si le site est public"
        } else {
            "API secret du site"
        });
    } else {
        ui.label(if app.config.provider == Provider::Dexcom {
            "Identifiant Dexcom"
        } else {
            "Adresse e-mail LibreLinkUp"
        });
        ui.add(
            egui::TextEdit::singleline(&mut app.username)
                .hint_text(if app.config.provider == Provider::Dexcom {
                    "E-mail, téléphone ou identifiant"
                } else {
                    "vous@exemple.ch"
                })
                .desired_width(f32::INFINITY)
                .margin(egui::vec2(12.0, 10.0)),
        );
        ui.label("Mot de passe");
    }
    let password = ui.add(
        egui::TextEdit::singleline(&mut app.password)
            .password(true)
            .hint_text(if nightscout {
                "Votre jeton ou secret d’accès"
            } else {
                "Votre mot de passe"
            })
            .desired_width(f32::INFINITY)
            .margin(egui::vec2(12.0, 10.0)),
    );
    ui::muted(
        ui,
        "Identifiants enregistrés dans le trousseau, séparément pour chaque fournisseur.",
    );
    let ready = if nightscout {
        !app.source_url.trim().is_empty()
            && (app.nightscout_auth == Auth::Token || !app.password.is_empty())
    } else {
        !app.username.trim().is_empty() && !app.password.is_empty()
    };
    let busy = matches!(app.snapshot.state, State::Connecting | State::Syncing);
    ui.horizontal(|ui| {
        let click = ui
            .add_enabled_ui(ready && !busy, |ui| ui::primary(ui, "Se connecter"))
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
        if !matches!(app.snapshot.state, State::Disconnected | State::Simulation)
            && ui.button("Se déconnecter").clicked()
        {
            app.disconnect();
        }
        if matches!(
            app.snapshot.state,
            State::ActionRequired | State::InvalidCredentials | State::Offline
        ) && ui
            .add_enabled(
                app.last_refresh.elapsed().as_secs() >= 30,
                egui::Button::new("Réessayer"),
            )
            .clicked()
        {
            app.last_refresh = std::time::Instant::now();
            app.send(Command::Retry);
        }
    });
    ui::muted(
        ui,
        format!(
            "{} · {}",
            app.config.provider.label(),
            app.snapshot.state.label()
        ),
    );
    if let Some(name) = &app.snapshot.patient_name {
        ui::muted(ui, format!("Personne suivie : {name}"));
    }
    if let Some(error) = &app.snapshot.error {
        ui.colored_label(theme::ink(ui.ctx(), theme::ORANGE), error);
    }
    if let Some(next) = app.snapshot.next_sync {
        ui::muted(
            ui,
            format!(
                "Prochaine synchronisation à {}",
                next.with_timezone(&chrono::Local).format("%H:%M")
            ),
        );
    }
}

pub fn onboarding(app: &mut App, ui: &mut egui::Ui) {
    ui.add_space(8.0);
    ui.heading("Votre glycémie, en un coup d’œil.");
    ui::muted(
        ui,
        "Choisissez votre fournisseur pour retrouver vos mesures ici et dans la mini-fenêtre.",
    );
    ui.add_space(16.0);
    ui::group(ui, "Sources de données", |ui| account(app, ui));
    ui.add_space(10.0);
    ui.horizontal(|ui| {
        if ui.button("Essayer la démo").clicked() {
            app.demo();
        }
        if ui.button("Plus tard").clicked() {
            app.config.onboarding_done = true;
            app.page = Page::Dashboard;
            app.mark_config();
        }
    });
    ui.add_space(12.0);
    ui::muted(
        ui,
        "Consultation secondaire, sans alarme. Gardez l’application de votre capteur pour les décisions de traitement.",
    );
}

pub fn header(app: &mut App, ui: &mut egui::Ui) {
    ui.horizontal(|ui| {
        ui.heading("Réglages");
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui::muted(ui, "Enregistrement automatique")
        });
    });
    ui.add_space(12.0);
    let width = (ui.available_width() - 20.0) / 3.0;
    ui.horizontal(|ui| {
        for (tab, title) in [
            (0, "Affichage"),
            (1, "Sources de données"),
            (2, "Options avancées"),
        ] {
            ui::choice(ui, &mut app.settings_tab, tab, title, width);
        }
    });
}

pub fn show(app: &mut App, ui: &mut egui::Ui) {
    let before = app.config.clone();
    match app.settings_tab {
        0 => display(app, ui),
        1 => {
            ui::group(ui, "Sources de données", |ui| account(app, ui));
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
    ui::group(ui, "Apparence et unité", |ui| {
        row(ui, "Apparence", |ui| {
            for (value, label) in [
                (Theme::Light, "Clair"),
                (Theme::Dark, "Sombre"),
                (Theme::System, "Automatique"),
            ] {
                ui::choice(ui, &mut app.config.theme, value, label, 138.0);
            }
        });
        row(ui, "Unité", |ui| {
            for value in [Unit::Mmol, Unit::Mgdl] {
                ui::choice(ui, &mut app.config.unit, value, value.label(), 138.0);
            }
        });
    });
    ui.add_space(6.0);
    ui::group(ui, "Mini-fenêtre", |ui| {
        ui::switch(
            ui,
            &mut app.config.widget_open,
            "Afficher la mini-fenêtre",
            "Votre glycémie, à côté de vos autres applications.",
        );
        row(ui, "Taille", |ui| {
            for (value, label) in [
                (WidgetSize::Compact, "Petite"),
                (WidgetSize::Normal, "Moyenne"),
                (WidgetSize::Extended, "Grande"),
            ] {
                ui::choice(ui, &mut app.config.widget_size, value, label, 138.0);
            }
        });
        ui::switch(
            ui,
            &mut app.config.always_on_top,
            "Toujours au premier plan",
            "",
        );
        if app.config.widget_open && ui.button("Replacer la mini-fenêtre à l’écran").clicked()
        {
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
            ui.set_width(470.0);
            ui.horizontal(|ui| {
                ui.label(RichText::new("Sources de données").size(24.0).strong());
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("Fermer").clicked() {
                        app.account_open = false;
                    }
                });
            });
            ui.add_space(12.0);
            account(app, ui);
        });
    if modal.should_close() {
        app.account_open = false;
    }
    if before != app.config {
        app.mark_config();
    }
}

fn advanced(app: &mut App, ui: &mut egui::Ui) {
    ui::group(ui, "Ouverture et fermeture", |ui| {
        ui::switch(
            ui,
            &mut app.config.autostart,
            "Lancer à l’ouverture de session",
            "",
        );
        ui::switch(
            ui,
            &mut app.config.start_minimized,
            "Démarrer sans ouvrir la fenêtre",
            "Accessible depuis l’icône de la barre de menus.",
        );
        ui::switch(
            ui,
            &mut app.config.close_to_tray,
            "Continuer après avoir fermé la fenêtre",
            "La synchronisation reste active en arrière-plan.",
        );
        ui::switch(
            ui,
            &mut app.config.minimize_to_tray,
            "Masquer l’application quand je la réduis",
            "",
        );
    });
    ui.add_space(8.0);
    ui::group(ui, "Repères du graphique", |ui| {
        ui::muted(
            ui,
            "Repères visuels uniquement. Ils ne déclenchent aucune alarme.",
        );
        let max_low = app.config.high.saturating_sub(1).min(200);
        bound(
            ui,
            "Repère bas · rouge",
            &mut app.config.low,
            40,
            max_low,
            app.config.unit,
        );
        bound(
            ui,
            "Repère haut · jaune",
            &mut app.config.high,
            app.config.low + 1,
            400,
            app.config.unit,
        );
    });
    ui.add_space(8.0);
    ui::group(ui, "Historique sur cet ordinateur", |ui| {
        row(ui, "Conservation", |ui| {
            egui::ComboBox::from_id_salt("history_days")
                .selected_text(format!("{} jours", app.config.history_days))
                .show_ui(ui, |ui| {
                    for days in 1..=30 {
                        ui.selectable_value(
                            &mut app.config.history_days,
                            days,
                            format!("{days} jours"),
                        );
                    }
                });
        });
        ui::muted(
            ui,
            format!(
                "{} mesures. Fichier local non chiffré par l’application.",
                app.snapshot.readings.len()
            ),
        );
        if !app.confirm_clear {
            if ui.button("Effacer l’historique…").clicked() {
                app.confirm_clear = true;
            }
        } else {
            ui.label("Effacer les mesures enregistrées sur cet ordinateur ?");
            ui.horizontal(|ui| {
                if ui.button("Annuler").clicked() {
                    app.confirm_clear = false;
                }
                if ui
                    .button(
                        RichText::new("Effacer les mesures")
                            .color(theme::ink(ui.ctx(), theme::RED)),
                    )
                    .clicked()
                {
                    app.send(Command::Clear);
                    app.snapshot.readings.clear();
                    app.confirm_clear = false;
                }
            });
            ui::muted(
                ui,
                "Share pourra récupérer les dernières 24 heures à la prochaine synchronisation.",
            );
        }
    });
    ui.add_space(8.0);
    ui::group(ui, "Démo", |ui| {
        ui::muted(ui, "Des mesures fictives pour découvrir l’application.");
        if app.snapshot.state == State::Simulation {
            if ui.button("Quitter la démo").clicked() {
                app.restore();
            }
        } else if ui.button("Essayer la démo").clicked() {
            app.demo();
        }
    });
    ui.add_space(8.0);
    ui::group(ui, "À propos", |ui| {
        ui::muted(
            ui,
            format!(
                "Easy Glucose {} · Application native · Licence MIT",
                env!("CARGO_PKG_VERSION")
            ),
        );
        ui::muted(
            ui,
            "Consultation secondaire, sans alarme. Utilisez l’application de votre capteur pour les décisions de traitement. Share et LibreLinkUp utilisent des protocoles non officiels.",
        );
        ui::muted(
            ui,
            "Synchronisation : environ 5 min pour Dexcom, 1 min pour les autres sources. Une mesure de 10 minutes ou plus est signalée comme ancienne.",
        );
        ui::muted(
            ui,
            "Raccourcis : ⌘/Ctrl + , réglages · R actualiser · W mini-fenêtre",
        );
    });
}

fn row(ui: &mut egui::Ui, label: &str, content: impl FnOnce(&mut egui::Ui)) {
    ui.horizontal(|ui| {
        ui.add_sized([140.0, 42.0], egui::Label::new(label));
        content(ui);
    });
}
fn bound(ui: &mut egui::Ui, label: &str, value: &mut u16, min: u16, max: u16, unit: Unit) {
    ui.horizontal(|ui| {
        ui.add_sized([190.0, 42.0], egui::Label::new(label));
        if ui
            .add_enabled(
                *value > min,
                egui::Button::new("−").min_size(egui::vec2(42.0, 42.0)),
            )
            .on_hover_text("Diminuer de 1 mg/dL")
            .clicked()
        {
            *value = value.saturating_sub(1).max(min);
        }
        ui.add_sized(
            [110.0, 42.0],
            egui::DragValue::new(value)
                .range(min..=max)
                .suffix(" mg/dL")
                .speed(0.0),
        )
        .on_hover_text("Cliquez pour saisir une valeur");
        if ui
            .add_enabled(
                *value < max,
                egui::Button::new("+").min_size(egui::vec2(42.0, 42.0)),
            )
            .on_hover_text("Augmenter de 1 mg/dL")
            .clicked()
        {
            *value = value.saturating_add(1).min(max);
        }
        if unit == Unit::Mmol {
            ui::muted(ui, format!("{} mmol/L", Unit::Mmol.format((*value).into())));
        }
    });
}

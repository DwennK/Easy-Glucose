use crate::{
    app::App,
    services::State,
    storage::WidgetSize,
    ui::{self, dashboard, theme},
};
use eframe::egui::{self, RichText};

pub fn show(app: &mut App, ctx: &egui::Context) {
    if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        app.config.widget_open = false;
        app.mark_config();
    }
    if ctx.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::Comma)) {
        app.show_dashboard();
        app.page = crate::app::Page::Settings;
    }
    let p = theme::palette(ctx);
    let compact = app.config.widget_size == WidgetSize::Compact;
    egui::CentralPanel::default()
        .frame(
            egui::Frame::new()
                .fill(p.surface)
                .inner_margin(if compact { 14 } else { 20 })
                .corner_radius(14),
        )
        .show(ctx, |ui| {
            ui.spacing_mut().item_spacing.y = 4.0;
            ui.spacing_mut().button_padding = egui::vec2(5.0, 2.0);
            ui.spacing_mut().interact_size.y = 20.0;
            let header = ui.horizontal(|ui| {
                ui::logo(ui, 18.0);
                ui.add(egui::Label::new(
                    RichText::new(if app.snapshot.state == State::Simulation {
                        "SIMULATION"
                    } else {
                        "Easy Glucose"
                    })
                    .size(10.0)
                    .strong()
                    .color(if app.snapshot.state == State::Simulation {
                        theme::ink(ctx, theme::ORANGE)
                    } else {
                        p.muted
                    }),
                ));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let close = ui.small_button("×").on_hover_text("Masquer le widget");
                    if close.clicked() {
                        app.config.widget_open = false;
                        app.mark_config();
                    }
                    let open = ui.small_button("↗").on_hover_text("Ouvrir Ma glycémie");
                    if open.clicked() {
                        app.show_dashboard();
                    }
                    open.rect.left()
                })
                .inner
            });
            let mut handle = header.response.rect;
            handle.max.x = header.inner - 2.0;
            let drag = ui
                .interact(handle, ui.id().with("widget_drag"), egui::Sense::drag())
                .on_hover_cursor(egui::CursorIcon::Grab);
            if !cfg!(target_os = "macos") && drag.drag_started_by(egui::PointerButton::Primary) {
                ctx.send_viewport_cmd(egui::ViewportCommand::StartDrag);
            }
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(
                        app.snapshot
                            .readings
                            .last()
                            .map(|r| r.display(app.config.unit))
                            .unwrap_or("—".into()),
                    )
                    .size(if compact { 39.0 } else { 54.0 }),
                );
                ui.label(
                    RichText::new(
                        app.snapshot
                            .readings
                            .last()
                            .map(|r| r.trend.arrow())
                            .unwrap_or("—"),
                    )
                    .size(29.0)
                    .color(theme::ink(ctx, dashboard::reading_color(app))),
                );
                ui::muted(ui, app.config.unit.label());
            });
            if let Some(r) = app.snapshot.readings.last() {
                if compact {
                    let age = r.age_seconds(chrono::Utc::now()).max(0) / 60;
                    let time = r.measured_at.with_timezone(&chrono::Local).format("%H:%M");
                    let stale = r.stale(chrono::Utc::now());
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new(format!(
                                "{time} · {age} min{}",
                                if stale { " · ancienne" } else { "" }
                            ))
                            .size(10.0)
                            .color(if stale {
                                theme::ink(ctx, theme::ORANGE)
                            } else {
                                p.muted
                            }),
                        );
                        if !stale
                            && let Some((delta, _)) =
                                crate::core::recent_delta(&app.snapshot.readings)
                        {
                            ui.label(
                                RichText::new(format!("Δ {}", app.config.unit.delta(delta)))
                                    .size(10.0)
                                    .color(p.muted),
                            );
                        }
                    });
                } else {
                    ui.label(RichText::new(dashboard::freshness(r)).size(11.0).color(
                        if r.stale(chrono::Utc::now()) {
                            theme::ink(ctx, theme::ORANGE)
                        } else {
                            p.muted
                        },
                    ));
                }
                if !compact {
                    ui.horizontal(|ui| {
                        ui::muted(
                            ui,
                            r.measured_at
                                .with_timezone(&chrono::Local)
                                .format("Mesurée à %H:%M")
                                .to_string(),
                        );
                        if let Some((d, m)) = crate::core::recent_delta(&app.snapshot.readings) {
                            ui::muted(ui, format!("{} / {m} min", app.config.unit.delta(d)));
                        }
                    });
                }
            } else {
                ui::muted(ui, "En attente de mesures");
            }
            dashboard::chart(
                ui,
                "widget_chart",
                &app.snapshot.readings,
                &app.config,
                if compact {
                    22.0
                } else {
                    (ui.available_height() - 6.0).max(35.0)
                },
                true,
            );
            if ctx.input(|i| i.viewport().close_requested()) {
                app.config.widget_open = false;
                app.mark_config();
            }
            if let Some(rect) = ctx.input(|i| i.viewport().outer_rect) {
                let pos = [rect.min.x, rect.min.y];
                if app.config.widget_position != Some(pos) {
                    app.config.widget_position = Some(pos);
                    app.mark_config();
                }
            }
        });
}

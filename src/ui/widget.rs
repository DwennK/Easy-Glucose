use crate::i18n::Text;
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
                .inner_margin(if compact { 10 } else { 14 })
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
                        Text::Demo.get()
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
                    let close = ui::tool_button(
                        ui,
                        egui::Button::image(ui::tool_icon(
                            ui,
                            egui::include_image!("../../assets/close.svg"),
                        )),
                        Text::HideWidget.get(),
                        true,
                    );
                    if close.clicked() {
                        app.config.widget_open = false;
                        app.mark_config();
                    }
                    let open = ui::tool_button(
                        ui,
                        egui::Button::image(ui::tool_icon(
                            ui,
                            egui::include_image!("../../assets/expand.svg"),
                        )),
                        Text::OpenApp.get(),
                        true,
                    );
                    if open.clicked() {
                        app.show_dashboard();
                    }
                    let hours = app.config.period_hours;
                    let period = egui::ComboBox::from_id_salt("widget_history_period")
                        .width(48.0)
                        .selected_text(format!("{hours} {}", Text::Hour.get()))
                        .show_ui(ui, |ui| {
                            for h in crate::storage::Config::HISTORY_PERIODS {
                                if ui
                                    .selectable_value(
                                        &mut app.config.period_hours,
                                        h,
                                        format!("{h} {}", Text::Hour.get()),
                                    )
                                    .clicked()
                                {
                                    ui.close();
                                }
                            }
                        });
                    if app.config.period_hours != hours {
                        app.mark_config();
                    }
                    period.response.rect.left()
                })
                .inner
            });
            let mut handle = header.response.rect;
            handle.max.x = header.inner - 2.0;
            let drag = ui
                .interact(handle, ui.id().with("widget_drag"), egui::Sense::drag())
                .on_hover_cursor(egui::CursorIcon::Grab)
                .on_hover_text(Text::WidgetPosition.get());
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
                let now = chrono::Utc::now();
                if compact {
                    let age = r.age_seconds(now).max(0) / 60;
                    let time = r.measured_at.with_timezone(&chrono::Local).format("%H:%M");
                    let stale = r.stale(now);
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new(format!(
                                "{time} · {age} {} {}",
                                Text::Minute.get(),
                                if stale { Text::Stale.get() } else { "" }
                            ))
                            .size(10.0)
                            .color(if stale {
                                theme::ink(ctx, theme::ORANGE)
                            } else {
                                p.muted
                            }),
                        );
                        if let Some((delta, _)) = app.snapshot.current_delta(now) {
                            ui.label(
                                RichText::new(format!("Δ {}", app.config.unit.delta(delta)))
                                    .size(10.0)
                                    .color(p.muted),
                            );
                        }
                    });
                } else {
                    ui.label(RichText::new(dashboard::freshness(r)).size(11.0).color(
                        if r.stale(now) {
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
                            format!(
                                "{} · {}",
                                Text::Measured.get(),
                                r.measured_at.with_timezone(&chrono::Local).format("%H:%M")
                            ),
                        );
                        if let Some((d, m)) = app.snapshot.current_delta(now) {
                            ui::muted(
                                ui,
                                format!(
                                    "{} / {m} {}",
                                    app.config.unit.delta(d),
                                    Text::Minute.get()
                                ),
                            );
                        }
                    });
                }
            } else {
                ui::muted(ui, Text::Waiting.get());
            }
            dashboard::chart(
                ui,
                "widget_chart",
                &app.snapshot.readings,
                &app.config,
                ui.available_height().max(if compact { 20.0 } else { 35.0 }),
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

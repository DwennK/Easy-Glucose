use crate::i18n::Text;
use crate::{
    app::App,
    core::{GAP_SECONDS, Reading, Unit, chart_segments, nearest_reading, recent_delta},
    services::State,
    ui::{self, theme},
};
use chrono::{Local, Utc};
use eframe::egui::{self, RichText};
use egui_plot::{HLine, Plot, PlotPoints, Points, Polygon, VLine};

pub fn reading_color(app: &App) -> egui::Color32 {
    let p = theme::palette(&app.ctx);
    match app.snapshot.readings.last() {
        Some(r) if !r.stale(Utc::now()) => {
            if r.mgdl < app.config.low {
                theme::RED
            } else if r.mgdl > app.config.high {
                theme::ORANGE
            } else {
                theme::GREEN
            }
        }
        _ => p.muted,
    }
}
pub fn freshness(reading: &Reading) -> String {
    let age = reading.age_seconds(Utc::now());
    if age < -60 {
        Text::Clock.get().into()
    } else if age < 60 {
        Text::JustNow.get().into()
    } else if age < 3600 {
        let duration = format!("{} {}", age / 60, Text::Minute.get());
        if age >= 600 {
            format!("{duration} · {}", Text::Stale.get())
        } else {
            format!("{} · {duration}", Text::Age.get())
        }
    } else {
        format!(
            "{} {} {} {} · {}",
            age / 3600,
            Text::Hour.get(),
            (age % 3600) / 60,
            Text::Minute.get(),
            Text::Stale.get()
        )
    }
}
pub fn show(app: &mut App, ui: &mut egui::Ui) {
    let p = theme::palette(ui.ctx());
    ui.horizontal(|ui| {
        ui.heading(Text::Dashboard.get());
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui
                .button(if app.config.widget_open {
                    Text::HideWidget.get()
                } else {
                    Text::ShowWidget.get()
                })
                .clicked()
            {
                app.config.widget_open = !app.config.widget_open;
                app.mark_config();
            }
            let enabled = !matches!(
                app.snapshot.state,
                State::Disconnected
                    | State::Syncing
                    | State::Connecting
                    | State::InvalidCredentials
                    | State::ActionRequired
                    | State::SelectingPatient
            ) && app.last_refresh.elapsed().as_secs() >= 30;
            if ui
                .add_enabled(enabled, egui::Button::new(Text::Refresh.get()))
                .on_hover_text(Text::Fetch.get())
                .clicked()
            {
                app.refresh();
            }
        });
    });
    ui.add_space(10.0);
    if app.snapshot.state == State::Simulation {
        ui.horizontal(|ui| {
            ui::pill(ui, Text::DemoReadings.get(), theme::ORANGE);
            ui::muted(ui, Text::DemoDisclaimer.get());
        });
        ui.add_space(6.0);
    }
    if let Some(error) = &app.snapshot.error {
        ui.colored_label(
            theme::ink(ui.ctx(), theme::ORANGE),
            crate::i18n::notice(error),
        );
        ui.add_space(6.0);
    }
    egui::Frame::new()
        .fill(p.surface)
        .corner_radius(16)
        .inner_margin(16)
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.columns(2, |cols| {
                cols[0].vertical(|ui| {
                    ui::muted(ui, Text::Latest.get());
                    ui.horizontal(|ui| {
                        let reading = app.snapshot.readings.last();
                        ui.label(
                            RichText::new(
                                reading
                                    .map(|r| r.display(app.config.unit))
                                    .unwrap_or("—".into()),
                            )
                            .size(68.0)
                            .strong(),
                        );
                        ui.label(
                            RichText::new(reading.map(|r| r.trend.arrow()).unwrap_or("—"))
                                .size(38.0)
                                .color(theme::ink(ui.ctx(), reading_color(app))),
                        );
                        ui::muted(ui, app.config.unit.label());
                    });
                    ui::muted(
                        ui,
                        app.snapshot
                            .readings
                            .last()
                            .map(freshness)
                            .unwrap_or(Text::ConnectHint.get().into()),
                    );
                });
                cols[1].with_layout(egui::Layout::top_down(egui::Align::RIGHT), |ui| {
                    ui.with_layout(egui::Layout::top_down(egui::Align::RIGHT), |ui| {
                        if let Some(r) = app.snapshot.readings.last() {
                            let label = if r.stale(Utc::now()) {
                                Text::Stale.get()
                            } else if r.mgdl < app.config.low {
                                Text::Below.get()
                            } else if r.mgdl > app.config.high {
                                Text::Above.get()
                            } else {
                                Text::Within.get()
                            };
                            ui::pill(ui, label, reading_color(app));
                            ui.label(r.trend.label());
                            if let Some((delta, minutes)) = recent_delta(&app.snapshot.readings) {
                                ui::muted(
                                    ui,
                                    format!(
                                        "{} {} / {minutes} {}",
                                        app.config.unit.delta(delta),
                                        app.config.unit.label(),
                                        Text::Minute.get()
                                    ),
                                );
                            }
                        } else if ui::primary(ui, Text::ConnectSource.get()).clicked() {
                            app.account_open = true;
                        }
                    });
                });
            });
        });
    ui.add_space(12.0);
    egui::Frame::new()
        .fill(p.surface)
        .corner_radius(16)
        .inner_margin(16)
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.label(RichText::new(Text::History.get()).size(18.0).strong());
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    for hours in [24, 12, 6, 3, 1] {
                        if ui::choice(
                            ui,
                            &mut app.config.period_hours,
                            hours,
                            &format!("{hours} {}", Text::Hour.get()),
                            75.0,
                        ) {
                            app.mark_config();
                        }
                    }
                });
            });
            ui.add_space(14.0);
            chart(
                ui,
                "main_chart",
                &app.snapshot.readings,
                &app.config,
                195.0,
                false,
            );
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                ui.colored_label(
                    theme::ink(ui.ctx(), theme::RED),
                    format!(
                        "{} · {}",
                        Text::Low.get(),
                        app.config.unit.format(app.config.low.into())
                    ),
                );
                ui.colored_label(
                    theme::ink(ui.ctx(), theme::ORANGE),
                    format!(
                        "{} · {} {}",
                        Text::High.get(),
                        app.config.unit.format(app.config.high.into()),
                        app.config.unit.label()
                    ),
                );
            });
        });
    ui.add_space(8.0);
    ui.horizontal(|ui| {
        ui::muted(
            ui,
            format!(
                "{} · {}",
                app.snapshot.provider.label(),
                app.snapshot.state.label()
            ),
        );
        if let Some(t) = app.snapshot.last_sync {
            ui::muted(
                ui,
                format!(
                    "· {} {}",
                    Text::Synced.get(),
                    t.with_timezone(&Local).format("%H:%M")
                ),
            );
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui::muted(ui, Text::HoverHelp.get())
        });
    });
}

pub fn chart(
    ui: &mut egui::Ui,
    id: &str,
    readings: &[Reading],
    config: &crate::storage::Config,
    height: f32,
    mini: bool,
) {
    let now = Utc::now().timestamp() as f64;
    let hours = if mini { 3 } else { config.period_hours };
    let start = now - f64::from(hours) * 3600.0;
    let end = now + f64::from(hours) * 54.0;
    let unit = config.unit;
    let low = unit.convert(config.low.into());
    let high = unit.convert(config.high.into());
    let segments = chart_segments(readings, start as i64, unit);
    let all_points: Vec<_> = segments
        .iter()
        .flatten()
        .copied()
        .filter(|p| p[0] <= end)
        .collect();
    let dark = ui.visuals().dark_mode;
    let y_low = if mini {
        all_points.iter().map(|p| p[1]).fold(low, f64::min) - unit.convert(15.0)
    } else {
        all_points
            .iter()
            .map(|p| p[1])
            .fold(unit.convert(35.0), f64::min)
            - unit.convert(5.0)
    };
    let y_high = if mini {
        all_points.iter().map(|p| p[1]).fold(high, f64::max) + unit.convert(20.0)
    } else {
        all_points
            .iter()
            .map(|p| p[1])
            .fold(unit.convert(410.0), f64::max)
            + unit.convert(5.0)
    };
    let point_color = if dark {
        egui::Color32::from_rgb(237, 241, 247)
    } else {
        egui::Color32::from_rgb(35, 39, 44)
    };
    let latest = all_points.last().copied();
    let response = Plot::new((id, hours, unit.label()))
        .height(height)
        .show_axes([!mini, !mini])
        .show_grid([false, false])
        .y_axis_position(egui_plot::HPlacement::Right)
        .show_background(false)
        .allow_drag(false)
        .allow_zoom(false)
        .allow_boxed_zoom(false)
        .allow_double_click_reset(false)
        .allow_scroll(false)
        .show_x(false)
        .show_y(false)
        .include_x(start)
        .include_x(end)
        .include_y(y_low)
        .include_y(y_high)
        .default_x_bounds(start, end)
        .default_y_bounds(y_low, y_high)
        .set_margin_fraction(egui::vec2(0.02, 0.0))
        .x_grid_spacer(egui_plot::uniform_grid_spacer(move |input| {
            let hours = (input.bounds.1 - input.bounds.0) / 3600.0;
            if hours <= 1.5 {
                [300.0, 900.0, 1800.0]
            } else if hours <= 6.5 {
                [900.0, 3600.0, 7200.0]
            } else {
                [3600.0, 10800.0, 21600.0]
            }
        }))
        .x_axis_formatter(|mark, _| {
            chrono::DateTime::from_timestamp(mark.value as i64, 0)
                .map(|t| t.with_timezone(&Local).format("%H:%M").to_string())
                .unwrap_or_default()
        })
        .y_axis_formatter(move |mark, _| {
            if unit == Unit::Mmol {
                crate::i18n::decimal(format!("{:.1}", mark.value))
            } else {
                format!("{:.0}", mark.value)
            }
        })
        .show(ui, |plot| {
            plot.polygon(
                Polygon::new(
                    Text::Guides.get(),
                    PlotPoints::from(vec![[start, low], [end, low], [end, high], [start, high]]),
                )
                .fill_color(if dark {
                    egui::Color32::from_rgb(51, 59, 69)
                } else {
                    egui::Color32::from_rgb(235, 237, 240)
                })
                .stroke(egui::Stroke::NONE)
                .allow_hover(false),
            );
            for (bound, color) in [
                (low, egui::Color32::from_rgb(232, 64, 75)),
                (high, egui::Color32::from_rgb(233, 184, 32)),
            ] {
                plot.hline(
                    HLine::new(Text::Guide.get(), bound)
                        .color(color)
                        .width(if mini { 1.0 } else { 1.5 })
                        .allow_hover(false),
                );
            }
            // Each dot is an actual sample. Missing samples remain gaps, with no interpolation.
            plot.points(
                Points::new(Text::Reading.get(), PlotPoints::from(all_points))
                    .color(point_color)
                    .radius(if mini { 1.5 } else { 2.8 })
                    .allow_hover(false),
            );
            if let Some(last) = latest {
                plot.points(
                    Points::new(Text::Latest.get(), PlotPoints::from(vec![last]))
                        .color(point_color)
                        .radius(if mini { 2.7 } else { 5.0 })
                        .allow_hover(false),
                );
            }
            // The whole plot is the hit target. Only horizontal position chooses a reading.
            let pointer = plot.ctx().input(|i| i.pointer.hover_pos())?;
            if !plot.response().hovered() || !plot.transform().frame().contains(pointer) {
                return None;
            }
            let time = plot.pointer_coordinate()?.x;
            let selected = nearest_reading(readings, time, start, end)?;
            let t = selected.measured_at.timestamp() as f64;
            let y = unit.convert(selected.mgdl.into());
            plot.vline(
                VLine::new(Text::SelectedReading.get(), t)
                    .color(theme::ACCENT)
                    .width(1.0)
                    .allow_hover(false),
            );
            plot.points(
                Points::new(Text::Selection.get(), PlotPoints::from(vec![[t, y]]))
                    .color(theme::ink(plot.ctx(), theme::ACCENT))
                    .radius(if mini { 3.5 } else { 5.5 })
                    .allow_hover(false),
            );
            Some((selected, (time - t).abs(), pointer))
        });
    if let Some((reading, distance, pointer)) = response.inner {
        // A non-interactive overlay cannot steal hover from the chart as the pointer moves.
        // Both text and marker use the same selected sample, including sensor limit values.
        let mut text = format!(
            "{} {}\n{}",
            reading.display(unit),
            unit.label(),
            reading
                .measured_at
                .with_timezone(&Local)
                .format("%d.%m · %H:%M")
        );
        if distance > GAP_SECONDS as f64 / 2.0 {
            text.push_str(&format!(
                "\n{} · {} {}",
                Text::Nearest.get(),
                (distance / 60.0).ceil() as u64,
                Text::Minute.get()
            ));
        }
        let p = theme::palette(ui.ctx());
        let painter = ui.ctx().layer_painter(egui::LayerId::new(
            egui::Order::Tooltip,
            response.response.id.with("reading"),
        ));
        let bounds = ui.ctx().content_rect().shrink(6.0);
        let galley = painter.layout(
            text,
            egui::FontId::proportional(13.0),
            p.text,
            (bounds.width() - 20.0).clamp(1.0, 280.0),
        );
        let size = galley.size() + egui::vec2(20.0, 16.0);
        let mut origin = pointer + egui::vec2(14.0, 14.0);
        if origin.x + size.x > bounds.right() {
            origin.x = pointer.x - size.x - 14.0;
        }
        if origin.y + size.y > bounds.bottom() {
            origin.y = pointer.y - size.y - 14.0;
        }
        origin = origin
            .max(bounds.min)
            .min((bounds.max - size).max(bounds.min));
        let rect = egui::Rect::from_min_size(origin, size);
        painter.rect_filled(rect, 7, p.surface);
        painter.rect_stroke(
            rect,
            7,
            egui::Stroke::new(1.0, p.line),
            egui::StrokeKind::Inside,
        );
        painter.galley(rect.min + egui::vec2(10.0, 8.0), galley, p.text);
    }
    if readings.is_empty() && !mini {
        ui::muted(ui, Text::NoData.get());
    }
}

use crate::i18n::Text;
use crate::{
    app::App,
    core::{
        GAP_SECONDS, Reading, STALE_SECONDS, Unit, chart_segments, nearest_reading, recent_delta,
    },
    services::State,
    storage::Config,
    ui::{self, theme},
};
use chrono::{Local, Utc};
use eframe::egui::{self, RichText};
use egui_plot::{HLine, Plot, PlotPoints, Points, Polygon, VLine};

const LOW_GUIDE: egui::Color32 = egui::Color32::from_rgb(232, 64, 75);
const HIGH_GUIDE: egui::Color32 = egui::Color32::from_rgb(233, 184, 32);

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
    relative_age(reading.age_seconds(Utc::now()))
}

fn relative_age(age: i64) -> String {
    if age < -60 {
        return Text::Clock.get().into();
    }
    if age < 60 {
        return Text::JustNow.get().into();
    }
    let minutes = (age % 3600) / 60;
    let duration = if age < 3600 {
        format!("{minutes} {}", Text::Minute.get())
    } else if minutes == 0 {
        format!("{} {}", age / 3600, Text::Hour.get())
    } else {
        format!(
            "{} {} {minutes} {}",
            age / 3600,
            Text::Hour.get(),
            Text::Minute.get()
        )
    };
    let elapsed = Text::Ago.get().replace("{duration}", &duration);
    if age >= STALE_SECONDS {
        format!("{elapsed} · {}", Text::Stale.get())
    } else {
        elapsed
    }
}

pub fn toolbar(app: &mut App, ui: &mut egui::Ui) {
    let label = if app.config.widget_open {
        Text::HideWidget
    } else {
        Text::ShowWidget
    };
    let widget = ui::tool_icon(ui, egui::include_image!("../../assets/widget.svg"));
    if ui::tool_button(
        ui,
        egui::Button::image(widget).selected(app.config.widget_open),
        label.get(),
        true,
    )
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
    let refresh = ui::tool_icon(ui, egui::include_image!("../../assets/refresh.svg"));
    if ui::tool_button(
        ui,
        egui::Button::image(refresh),
        Text::Refresh.get(),
        enabled,
    )
    .clicked()
    {
        app.refresh();
    }
}

pub fn show(app: &mut App, ui: &mut egui::Ui) {
    if let Some(error) = &app.snapshot.error {
        ui.colored_label(
            theme::ink(ui.ctx(), theme::ORANGE),
            crate::i18n::notice(error),
        );
        ui.add_space(6.0);
    }
    if let Some(reading) = app.snapshot.readings.last() {
        ui.horizontal(|ui| {
            ui.label(RichText::new(reading.display(app.config.unit)).size(52.0));
            ui::muted(ui, app.config.unit.label());
            ui.label(
                RichText::new(reading.trend.arrow())
                    .size(27.0)
                    .color(theme::ink(ui.ctx(), reading_color(app))),
            );
            let delta = recent_delta(&app.snapshot.readings);
            let width = ui.available_width();
            ui.allocate_ui_with_layout(
                egui::vec2(width, if delta.is_some() { 45.0 } else { 18.0 }),
                egui::Layout::top_down(egui::Align::Max),
                |ui| {
                    ui.spacing_mut().item_spacing.y = 1.0;
                    if let Some((change, minutes)) = delta {
                        let p = theme::palette(ui.ctx());
                        let mut line = egui::text::LayoutJob::default();
                        let amount = if change == 0.0 {
                            app.config.unit.format(0.0)
                        } else {
                            app.config.unit.delta(change)
                        };
                        line.append(
                            &amount,
                            0.0,
                            egui::TextFormat::simple(egui::FontId::proportional(25.0), p.text),
                        );
                        line.append(
                            app.config.unit.label(),
                            4.0,
                            egui::TextFormat::simple(egui::FontId::proportional(13.0), p.muted),
                        );
                        ui.label(line);
                        ui::muted(
                            ui,
                            format!(
                                "{} · {minutes} {}",
                                reading.trend.label(),
                                Text::Minute.get()
                            ),
                        );
                    } else {
                        ui::muted(ui, reading.trend.label());
                    }
                },
            );
        });
        ui.horizontal_wrapped(|ui| {
            if app.snapshot.state == State::Simulation {
                ui.scope(|ui| ui::pill(ui, Text::DemoReadings.get(), theme::ORANGE))
                    .response
                    .on_hover_text(Text::DemoDisclaimer.get());
            }
            if !reading.stale(Utc::now()) {
                if reading.mgdl < app.config.low {
                    ui::pill(ui, Text::Below.get(), theme::RED);
                } else if reading.mgdl > app.config.high {
                    ui::pill(ui, Text::Above.get(), theme::ORANGE);
                }
            }
            ui::muted(
                ui,
                format!(
                    "{} {}  ·  {}",
                    Text::Measured.get(),
                    reading.measured_at.with_timezone(&Local).format("%H:%M"),
                    freshness(reading)
                ),
            );
        });
    } else if app.snapshot.state.needs_account() {
        ui::muted(ui, Text::ConnectHint.get());
        if ui::primary(ui, Text::ConnectSource.get()).clicked() {
            app.account_open = true;
        }
    } else {
        ui::muted(ui, Text::Waiting.get());
    }
    ui.separator();
    ui.horizontal(|ui| {
        ui.label(RichText::new(Text::History.get()).size(16.0).strong())
            .on_hover_text(Text::HoverHelp.get());
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui::history_period(ui, &mut app.config.period_hours) {
                app.mark_config();
            }
        });
    });
    ui.add_space(4.0);
    chart(
        ui,
        "main_chart",
        &app.snapshot.readings,
        &app.config,
        ui.available_width() / 1.8,
        false,
    );
    ui.add_space(2.0);
    ui.horizontal(|ui| {
        for (title, level, color) in [
            (Text::Low, app.config.low, LOW_GUIDE),
            (Text::High, app.config.high, HIGH_GUIDE),
        ] {
            let (swatch, _) = ui.allocate_exact_size(egui::vec2(12.0, 6.0), egui::Sense::hover());
            ui.painter().hline(
                swatch.x_range(),
                swatch.center().y,
                egui::Stroke::new(2.0, color),
            );
            ui.small(format!(
                "{} {}",
                title.get(),
                app.config.unit.format(level.into())
            ));
        }
        ui.small(app.config.unit.label());
        let mut source = format!(
            "{} · {}",
            app.snapshot.provider.label(),
            app.snapshot.state.label()
        );
        if let Some(t) = app.snapshot.last_sync {
            source.push_str(&format!(
                " · {} {}",
                Text::Synced.get(),
                t.with_timezone(&Local).format("%H:%M")
            ));
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.add(
                egui::Label::new(
                    RichText::new(source)
                        .size(11.0)
                        .color(theme::palette(ui.ctx()).muted),
                )
                .wrap(),
            );
        });
    });
}

// Strict comparisons keep samples on either guide inside the target range.
fn glucose_zone(value: f64, low: f64, high: f64) -> usize {
    if value < low {
        0
    } else if value > high {
        2
    } else {
        1
    }
}

pub fn chart(
    ui: &mut egui::Ui,
    id: &str,
    readings: &[Reading],
    config: &Config,
    height: f32,
    mini: bool,
) {
    let now = Utc::now().timestamp() as f64;
    let hours = config.period_hours;
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
    let grid_color = theme::palette(ui.ctx()).line.gamma_multiply(0.55);
    let latest = all_points.last().copied();
    let response = Plot::new((id, hours, unit.label()))
        .height(height)
        .show_axes([!mini, !mini])
        .show_grid([false, false])
        .y_grid_spacer(egui_plot::uniform_grid_spacer(move |_| {
            if unit == Unit::Mmol {
                [5.0, 10.0, 20.0]
            } else {
                [50.0, 100.0, 200.0]
            }
        }))
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
            if hours <= 6.5 {
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
            if !mini {
                let interval = if unit == Unit::Mmol { 5.0 } else { 50.0 };
                for tick in (y_low / interval).ceil() as i32..=(y_high / interval).floor() as i32 {
                    plot.hline(
                        HLine::new("", f64::from(tick) * interval)
                            .color(grid_color)
                            .width(0.6)
                            .allow_hover(false),
                    );
                }
            }
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
            for (bound, color) in [(low, LOW_GUIDE), (high, HIGH_GUIDE)] {
                plot.hline(
                    HLine::new(Text::Guide.get(), bound)
                        .color(color)
                        .width(if mini { 1.0 } else { 1.5_f32 })
                        .allow_hover(false),
                );
            }
            // Each dot is an actual sample. Missing samples remain gaps, with no interpolation.
            let colors = [LOW_GUIDE, point_color, HIGH_GUIDE];
            let mut groups: [Vec<[f64; 2]>; 3] = Default::default();
            for point in all_points {
                groups[glucose_zone(point[1], low, high)].push(point);
            }
            for (points, color) in groups.into_iter().zip(colors) {
                if !points.is_empty() {
                    plot.points(
                        Points::new(Text::Reading.get(), PlotPoints::from(points))
                            .color(color)
                            .radius(if mini { 1.5 } else { 2.8 })
                            .allow_hover(false),
                    );
                }
            }
            if let Some(last) = latest {
                plot.points(
                    Points::new(Text::Latest.get(), PlotPoints::from(vec![last]))
                        .color(colors[glucose_zone(last[1], low, high)])
                        .radius(if mini { 2.7 } else { 5.0_f32 })
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
                    .width(1.0_f32)
                    .allow_hover(false),
            );
            plot.points(
                Points::new(Text::Selection.get(), PlotPoints::from(vec![[t, y]]))
                    .color(colors[glucose_zone(y, low, high)])
                    .radius(if mini { 3.5 } else { 5.5_f32 })
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
            egui::Stroke::new(1.0_f32, p.line),
            egui::StrokeKind::Inside,
        );
        painter.galley(rect.min + egui::vec2(10.0, 8.0), galley, p.text);
    }
    if readings.is_empty() && !mini {
        ui::muted(ui, Text::NoData.get());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sample_colors_follow_the_guides_in_both_units() {
        for unit in [Unit::Mmol, Unit::Mgdl] {
            for (value, expected) in [(69.0, 0), (70.0, 1), (180.0, 1), (181.0, 2)] {
                assert_eq!(
                    glucose_zone(unit.convert(value), unit.convert(70.0), unit.convert(180.0)),
                    expected
                );
            }
        }
    }

    #[test]
    fn relative_age_keeps_clock_and_stale_indicators_at_their_boundaries() {
        assert_eq!(relative_age(-61), Text::Clock.get());
        assert_eq!(relative_age(-60), Text::JustNow.get());
        assert_eq!(relative_age(59), Text::JustNow.get());
        assert_eq!(relative_age(120), "2 min ago");
        assert_eq!(relative_age(STALE_SECONDS - 1), "9 min ago");
        assert_eq!(relative_age(STALE_SECONDS), "10 min ago · Stale reading");
        assert_eq!(relative_age(3600), "1 h ago · Stale reading");
        assert_eq!(relative_age(3660), "1 h 1 min ago · Stale reading");
    }
}

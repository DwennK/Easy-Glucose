use crate::{
    app::App,
    i18n::Text,
    services::State,
    ui::{dashboard, popover, theme},
};
use chrono::Utc;
use eframe::egui::{self, Align2, Color32, FontId};
use std::time::Instant;

const GAP: f32 = 8.0;

fn fit(position: egui::Pos2, size: egui::Vec2, bounds: egui::Rect) -> egui::Pos2 {
    position.clamp(bounds.min, (bounds.max - size).max(bounds.min))
}

pub fn show(app: &mut App, ctx: &egui::Context, work: egui::Rect, screen: egui::Rect) {
    let p = theme::palette(ctx);
    let color = theme::ink(ctx, dashboard::reading_color(app));
    let now = Utc::now();
    let reading = app.snapshot.readings.last();
    let unit = app.config.unit;
    let value = reading.map_or_else(|| "—".to_owned(), |r| r.display(unit));
    let mut delta = String::new();
    let mut details = Vec::new();
    if app.snapshot.state == State::Simulation {
        details.push(Text::Demo.get().to_owned());
    }
    if let Some(r) = reading {
        if r.stale(now) {
            details.push(Text::Stale.get().to_owned());
        } else if let Some((change, _)) = app.snapshot.current_delta(now) {
            delta = unit.delta(change);
        }
        details.push(format!(
            "{} {}",
            r.age_seconds(now).max(0) / 60,
            Text::Minute.get()
        ));
    } else {
        details.push(Text::NoReading.get().to_owned());
    }
    let caption = details.join(" · ");
    let caption_width = ctx.fonts_mut(|fonts| {
        fonts
            .layout_no_wrap(caption.clone(), FontId::proportional(11.0), p.muted)
            .size()
            .x
    });
    let size = egui::vec2(115.0_f32.max(caption_width.ceil() + 12.0), 44.0);
    let position = fit(
        app.config.clock_widget_position.map_or_else(
            || (work.right_bottom() - size - egui::vec2(GAP, GAP)).max(work.min),
            egui::Pos2::from,
        ),
        size,
        screen,
    );
    let id = egui::ViewportId::from_hash_of("clock_widget");
    let title = format!("Easy Glucose · {}", Text::ClockWidget.get());
    let builder = egui::ViewportBuilder::default()
        .with_position(position)
        .with_inner_size(size)
        .with_taskbar(false)
        .with_resizable(false)
        .with_active(false)
        .with_always_on_top();
    super::show_floating_window(ctx, id, title, builder, |ctx| {
        if ctx.input(|i| i.viewport().close_requested()) {
            app.config.clock_widget = false;
            app.mark_config();
            return;
        }
        let fill =
            Color32::from_rgba_unmultiplied(p.surface.r(), p.surface.g(), p.surface.b(), 184);
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(fill).corner_radius(8))
            .show(ctx, |ui| {
                let rect = ui.max_rect();
                let center = rect.center();
                let mut line = egui::text::LayoutJob::default();
                line.append(
                    &value,
                    0.0,
                    egui::TextFormat::simple(
                        FontId::new(21.0, egui::FontFamily::Name("clock-value".into())),
                        color,
                    ),
                );
                if !delta.is_empty() {
                    line.append(
                        &delta,
                        6.0,
                        egui::TextFormat::simple(FontId::proportional(15.0), color),
                    );
                }
                let galley = ui.painter().layout_job(line);
                let origin =
                    center - egui::vec2(galley.size().x / 2.0, 9.0 + galley.size().y / 2.0);
                ui.painter().galley(origin, galley, color);
                ui.painter().text(
                    center + egui::vec2(0.0, 12.0),
                    Align2::CENTER_CENTER,
                    &caption,
                    FontId::proportional(11.0),
                    p.muted,
                );
                let response = ui
                    .interact(
                        rect,
                        ui.id().with("open_graph"),
                        egui::Sense::click_and_drag(),
                    )
                    .on_hover_cursor(egui::CursorIcon::Grab);
                response.widget_info(|| {
                    egui::WidgetInfo::labeled(
                        egui::WidgetType::Button,
                        true,
                        format!(
                            "{value} {} {delta} · {caption} · {}",
                            unit.label(),
                            Text::Preview.get()
                        ),
                    )
                });
                if response.dragged_by(egui::PointerButton::Primary)
                    && let Some(press) = ctx.input(|i| i.pointer.press_origin())
                    && let Some(cursor) =
                        crate::platform::clock::cursor_position(ctx.pixels_per_point())
                {
                    let next = fit(cursor - press.to_vec2(), size, screen);
                    if next != position {
                        app.config.clock_widget_position = Some(next.into());
                        app.mark_config();
                    }
                }
                if response.clicked() {
                    let above = position.y - GAP - popover::SIZE.y;
                    let y = if above >= work.top() {
                        above
                    } else {
                        position.y + size.y + GAP
                    };
                    let graph = fit(
                        egui::pos2(position.x + size.x - popover::SIZE.x, y),
                        popover::SIZE,
                        work,
                    );
                    app.popover.toggle(graph, Instant::now());
                    ctx.request_repaint_of(egui::ViewportId::ROOT);
                }
            });
    });
}

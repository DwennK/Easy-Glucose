use crate::{
    app::App,
    i18n::Text,
    services::State,
    ui::{dashboard, popover, theme},
};
use chrono::Utc;
use eframe::egui::{self, Align2, Color32, FontId};
use std::time::Instant;

const SIZE: egui::Vec2 = egui::vec2(152.0, 50.0);
const GAP: f32 = 8.0;

pub fn show(app: &mut App, ctx: &egui::Context, work: egui::Rect) {
    let position = (work.right_bottom() - SIZE - egui::vec2(GAP, GAP)).max(work.min);
    let id = egui::ViewportId::from_hash_of("clock_widget");
    let title = format!("Easy Glucose · {}", Text::ClockWidget.get());
    let builder = egui::ViewportBuilder::default()
        .with_title(&title)
        .with_position(position)
        .with_inner_size(SIZE)
        .with_transparent(true)
        .with_decorations(false)
        .with_taskbar(false)
        .with_resizable(false)
        .with_active(false)
        .with_always_on_top();
    ctx.show_viewport_immediate(id, builder, |ctx, _| {
        if let Some(monitor) = &mut app.clock_monitor {
            monitor.enable_alpha(&title);
        }
        if ctx.input(|i| i.viewport().close_requested()) {
            app.config.clock_widget = false;
            app.mark_config();
            return;
        }
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
        let fill =
            Color32::from_rgba_unmultiplied(p.surface.r(), p.surface.g(), p.surface.b(), 220);
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
                let font = FontId::proportional(11.0);
                let galley = ui
                    .painter()
                    .layout_no_wrap(caption.clone(), font.clone(), p.muted);
                let fit = ((rect.width() - 8.0) / galley.size().x).min(1.0);
                ui.painter().text(
                    center + egui::vec2(0.0, 12.0),
                    Align2::CENTER_CENTER,
                    &caption,
                    FontId::proportional(font.size * fit),
                    p.muted,
                );
                let response = ui
                    .interact(rect, ui.id().with("open_graph"), egui::Sense::click())
                    .on_hover_cursor(egui::CursorIcon::PointingHand);
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
                if response.clicked() {
                    let graph = egui::pos2(
                        work.right() - GAP - popover::SIZE.x,
                        position.y - GAP - popover::SIZE.y,
                    )
                    .max(work.min);
                    app.popover.toggle(graph, Instant::now());
                    ctx.request_repaint_of(egui::ViewportId::ROOT);
                }
            });
    });
}

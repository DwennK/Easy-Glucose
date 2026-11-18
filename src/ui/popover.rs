use crate::i18n::Text;
use crate::{
    app::{App, Page},
    services::State,
    storage::Config,
    ui::{self, dashboard, theme},
};
use eframe::egui::{self, RichText};
use std::time::{Duration, Instant};

pub const SIZE: egui::Vec2 = egui::vec2(420.0, 382.0);

pub struct Popover {
    pub open: bool,
    pub position: egui::Pos2,
    pub hours: u8,
    focused_once: bool,
    position_pending: bool,
    closed_at: Option<Instant>,
}
impl Default for Popover {
    fn default() -> Self {
        Self {
            open: false,
            position: egui::Pos2::ZERO,
            hours: 3,
            focused_once: false,
            position_pending: false,
            closed_at: None,
        }
    }
}
impl Popover {
    pub fn toggle(&mut self, position: egui::Pos2, now: Instant) {
        if self.open {
            self.close(now);
            return;
        }
        // Clicking the status item can first deliver focus loss, then mouse-up.
        // That same click must close the panel, rather than immediately reopening it.
        if self
            .closed_at
            .is_some_and(|at| now.saturating_duration_since(at) < Duration::from_millis(250))
        {
            return;
        }
        self.position = position;
        self.position_pending = true;
        self.focused_once = false;
        self.open = true;
    }
    pub fn close(&mut self, now: Instant) {
        if self.open {
            self.closed_at = Some(now);
        }
        self.open = false;
        self.focused_once = false;
    }
    pub fn focus(&mut self, focused: bool, now: Instant) {
        if focused {
            self.focused_once = true;
        } else if self.focused_once {
            self.close(now);
        }
    }
}

pub fn position(anchor: egui::Rect, screen: egui::Rect, size: egui::Vec2) -> egui::Pos2 {
    let min = screen.min + egui::vec2(8.0, 4.0);
    let max = (screen.max - size - egui::vec2(8.0, 8.0)).max(min);
    egui::pos2(
        (anchor.center().x - size.x / 2.0).clamp(min.x, max.x),
        (anchor.bottom() + 4.0).clamp(min.y, max.y),
    )
}

pub fn show(app: &mut App, ctx: &egui::Context) {
    let now = Instant::now();
    if app.popover.position_pending {
        app.popover.position_pending = false;
        // Apply again after creation: AppKit may adjust a new window's initial frame.
        ctx.send_viewport_cmd(egui::ViewportCommand::OuterPosition(app.popover.position));
        ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
        if std::env::args().any(|a| a == "--diagnostics") {
            eprintln!(
                "diagnostic: menu initial={:?} requested={:?}",
                ctx.input(|i| i.viewport().outer_rect),
                app.popover.position
            );
        }
    }
    app.popover.focus(ctx.input(|i| i.focused), now);
    if ctx.input(|i| i.key_pressed(egui::Key::Escape) || i.viewport().close_requested()) {
        app.popover.close(now);
    }
    if !app.popover.open {
        ctx.request_repaint_of(egui::ViewportId::ROOT);
        return;
    }
    let p = theme::palette(ctx);
    egui::CentralPanel::default()
        .frame(
            egui::Frame::new()
                .fill(p.surface)
                .inner_margin(12)
                .corner_radius(12),
        )
        .show(ctx, |ui| {
            // This transient desktop panel uses compact controls, independently of settings.
            ui.spacing_mut().item_spacing = egui::vec2(6.0, 4.0);
            ui.spacing_mut().button_padding = egui::vec2(8.0, 3.0);
            ui.spacing_mut().interact_size = egui::vec2(24.0, 26.0);
            ui.style_mut()
                .text_styles
                .insert(egui::TextStyle::Button, egui::FontId::proportional(12.0));
            ui.style_mut()
                .text_styles
                .insert(egui::TextStyle::Body, egui::FontId::proportional(12.0));
            let widgets = &mut ui.style_mut().visuals.widgets;
            for widget in [
                &mut widgets.inactive,
                &mut widgets.hovered,
                &mut widgets.active,
            ] {
                widget.corner_radius = egui::CornerRadius::same(6);
            }
            ui.horizontal(|ui| {
                ui::logo(ui, 18.0);
                ui.label(RichText::new("Easy Glucose").strong().size(14.0));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if header_button(ui, egui::Button::new("×"), Text::ClosePreview.get()).clicked()
                    {
                        app.popover.close(now);
                    }
                    if header_button(
                        ui,
                        egui::Button::image(
                            egui::Image::new(egui::include_image!("../../assets/settings.svg"))
                                .fit_to_exact_size(egui::vec2(16.0, 16.0)),
                        ),
                        Text::Settings.get(),
                    )
                    .clicked()
                    {
                        app.show_dashboard();
                        app.page = Page::Settings;
                    }
                    if header_button(
                        ui,
                        egui::Button::image(
                            egui::Image::new(egui::include_image!("../../assets/dashboard.svg"))
                                .fit_to_exact_size(egui::vec2(16.0, 16.0)),
                        ),
                        Text::OpenApp.get(),
                    )
                    .clicked()
                    {
                        app.show_dashboard();
                    }
                });
            });
            if app.snapshot.state == State::Simulation {
                ui.colored_label(theme::ink(ctx, theme::ORANGE), Text::DemoReadings.get());
            }
            ui.horizontal(|ui| {
                let current = app.snapshot.readings.last();
                ui.label(
                    RichText::new(
                        current
                            .map(|r| r.display(app.config.unit))
                            .unwrap_or("—".into()),
                    )
                    .size(40.0),
                );
                ui.label(
                    RichText::new(current.map(|r| r.trend.arrow()).unwrap_or("—"))
                        .size(24.0)
                        .color(theme::ink(ctx, dashboard::reading_color(app))),
                );
                ui::muted(ui, app.config.unit.label());
            });
            if let Some(reading) = app.snapshot.readings.last() {
                ui.label(
                    RichText::new(dashboard::freshness(reading))
                        .size(12.0)
                        .color(if reading.stale(chrono::Utc::now()) {
                            theme::ink(ctx, theme::ORANGE)
                        } else {
                            p.muted
                        }),
                );
            } else {
                ui::muted(ui, Text::NoSource.get());
            }
            ui.label(
                RichText::new(format!(
                    "{} · {}",
                    app.snapshot.provider.label(),
                    app.snapshot.state.label()
                ))
                .size(12.0)
                .color(p.muted),
            );
            if let Some(name) = &app.snapshot.patient_name {
                ui.label(RichText::new(name).size(12.0).color(p.muted));
            }
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.label(RichText::new(Text::History.get()).size(12.0).color(p.muted));
                ui.add_space(4.0);
                ui.spacing_mut().item_spacing.x = 4.0;
                for hours in Config::HISTORY_PERIODS {
                    let selected = app.popover.hours == hours;
                    let label = format!("{hours} {}", Text::Hour.get());
                    let response = ui.add_sized(
                        [48.0, 26.0],
                        egui::Button::selectable(
                            selected,
                            RichText::new(&label).size(12.0).color(if selected {
                                egui::Color32::WHITE
                            } else {
                                p.text
                            }),
                        )
                        .fill(if selected { theme::ACCENT } else { p.secondary })
                        .stroke(egui::Stroke::new(
                            1.0_f32,
                            if selected { theme::ACCENT } else { p.line },
                        ))
                        .corner_radius(5),
                    );
                    response.widget_info(|| {
                        egui::WidgetInfo::selected(
                            egui::WidgetType::RadioButton,
                            ui.is_enabled(),
                            selected,
                            &label,
                        )
                    });
                    if response.clicked() {
                        app.popover.hours = hours;
                    }
                }
            });
            let mut config = app.config.clone();
            config.period_hours = app.popover.hours;
            let height = (ui.available_height()
                - if app.snapshot.readings.is_empty() {
                    24.0
                } else {
                    0.0
                })
            .max(100.0);
            dashboard::chart(
                ui,
                "menubar_chart",
                &app.snapshot.readings,
                &config,
                height,
                false,
            );
        });
    if !app.popover.open {
        ctx.request_repaint_of(egui::ViewportId::ROOT);
    }
    ctx.request_repaint_after(Duration::from_secs(30));
}

fn header_button(ui: &mut egui::Ui, button: egui::Button<'_>, label: &str) -> egui::Response {
    let response = ui.add_sized([30.0, 26.0], button);
    response.widget_info(|| {
        egui::WidgetInfo::labeled(egui::WidgetType::Button, ui.is_enabled(), label)
    });
    response.on_hover_text(label)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn stays_below_tray_and_inside_each_screen_in_logical_points() {
        let screen = egui::Rect::from_min_max(egui::pos2(0.0, 25.0), egui::pos2(1470.0, 900.0));
        let anchor = egui::Rect::from_min_size(egui::pos2(1390.0, 0.0), egui::vec2(80.0, 25.0));
        assert_eq!(position(anchor, screen, SIZE), egui::pos2(1042.0, 29.0));
        let central_anchor =
            egui::Rect::from_min_size(egui::pos2(800.0, 0.0), egui::vec2(80.0, 25.0));
        let centered = position(central_anchor, screen, SIZE);
        assert_eq!(centered.x + SIZE.x / 2.0, central_anchor.center().x);
        assert_eq!(centered.y, central_anchor.bottom() + 4.0);
        let left_screen = screen.translate(egui::vec2(-1470.0, -300.0));
        let left_anchor = anchor.translate(egui::vec2(-1470.0, -300.0));
        assert_eq!(
            position(left_anchor, left_screen, SIZE),
            egui::pos2(-428.0, -271.0)
        );
        let left_edge =
            egui::Rect::from_min_size(egui::pos2(-1465.0, -300.0), egui::vec2(80.0, 25.0));
        assert_eq!(position(left_edge, left_screen, SIZE).x, -1462.0);
    }
    #[test]
    fn focus_loss_closes_but_initial_unfocused_frame_does_not() {
        let now = Instant::now();
        let mut panel = Popover::default();
        panel.toggle(egui::Pos2::ZERO, now);
        panel.focus(false, now);
        assert!(panel.open);
        panel.focus(true, now);
        panel.focus(false, now + Duration::from_millis(500));
        assert!(!panel.open);
        panel.toggle(egui::Pos2::ZERO, now + Duration::from_millis(510));
        assert!(!panel.open, "same tray click must not reopen after blur");
        panel.toggle(egui::Pos2::ZERO, now + Duration::from_secs(1));
        assert!(panel.open);
        panel.toggle(egui::Pos2::ZERO, now + Duration::from_secs(2));
        assert!(!panel.open);
    }
}

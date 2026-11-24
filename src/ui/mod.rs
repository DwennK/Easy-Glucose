use crate::storage::WidgetSize;
#[cfg(windows)]
pub mod clock;
pub mod dashboard;
pub mod popover;
pub mod settings;
pub mod theme;
pub mod widget;
use eframe::egui::{self, RichText};

pub fn escape_outside_popup(ctx: &egui::Context) -> bool {
    !egui::Popup::is_any_open(ctx)
        && ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape))
}

pub fn centered_column(ui: &mut egui::Ui, max_width: f32, content: impl FnOnce(&mut egui::Ui)) {
    let width = ui.available_width().min(max_width);
    ui.vertical_centered(|ui| {
        ui.allocate_ui_with_layout(
            egui::vec2(width, 0.0),
            egui::Layout::top_down(egui::Align::Min),
            content,
        );
    });
}

pub fn history_period(ui: &mut egui::Ui, hours: &mut u8) -> bool {
    let labels = crate::storage::Config::HISTORY_PERIODS
        .map(|h| (h, format!("{h} {}", crate::i18n::Text::Hour.get())));
    segments(
        ui,
        hours,
        &labels
            .each_ref()
            .map(|(h, label)| (*h, Segment::Text(label))),
    )
}

pub fn eyebrow(ui: &mut egui::Ui, text: &str) {
    ui.label(
        RichText::new(text)
            .size(11.0)
            .color(theme::palette(ui.ctx()).muted)
            .strong(),
    );
}
pub fn muted(ui: &mut egui::Ui, text: impl Into<String>) {
    ui.label(
        RichText::new(text.into())
            .size(13.0)
            .color(theme::palette(ui.ctx()).muted),
    );
}
pub fn primary(ui: &mut egui::Ui, text: &str) -> egui::Response {
    ui.add(
        egui::Button::new(RichText::new(text).color(egui::Color32::WHITE).size(13.0))
            .fill(theme::ACCENT)
            .corner_radius(6)
            .min_size(egui::vec2(0.0, 28.0)),
    )
}
pub fn pill(ui: &mut egui::Ui, text: &str, color: egui::Color32, checked: bool) {
    let mut margin = egui::Margin::symmetric(7, 2);
    if checked {
        margin.left += 14;
    }
    egui::Frame::new()
        .fill(color.gamma_multiply(0.12))
        .corner_radius(6)
        .inner_margin(margin)
        .show(ui, |ui| {
            let ink = theme::ink(ui.ctx(), color);
            let label = ui.label(RichText::new(text).size(11.0).strong().color(ink));
            if checked {
                let start = label.rect.left_center() - egui::vec2(13.0, 0.0);
                ui.painter().add(egui::Shape::line(
                    vec![
                        start,
                        start + egui::vec2(3.0, 3.0),
                        start + egui::vec2(8.5, -3.0),
                    ],
                    egui::Stroke::new(1.5_f32, ink),
                ));
            }
        });
}
pub fn logo(ui: &mut egui::Ui, size: f32) {
    ui.add(
        egui::Image::new(egui::include_image!("../../assets/logo.svg"))
            .fit_to_exact_size(egui::vec2(size, size)),
    );
}

pub fn tool_icon(ui: &egui::Ui, source: egui::ImageSource<'static>) -> egui::Image<'static> {
    egui::Image::new(source)
        .fit_to_exact_size(egui::Vec2::splat(17.0))
        .tint(theme::palette(ui.ctx()).muted)
}

pub fn tool_button(
    ui: &mut egui::Ui,
    button: egui::Button<'_>,
    label: &str,
    enabled: bool,
) -> egui::Response {
    let response = ui.add_enabled(
        enabled,
        button
            .frame_when_inactive(false)
            .min_size(egui::vec2(26.0, 24.0)),
    );
    response.widget_info(|| {
        egui::WidgetInfo::labeled(egui::WidgetType::Button, response.enabled(), label)
    });
    response.on_hover_text(label)
}

pub enum Segment<'a> {
    Text(&'a str),
    Window(WidgetSize),
}

impl<'a> Segment<'a> {
    fn label(&self) -> &'a str {
        match self {
            Self::Text(text) => text,
            Self::Window(size) => size.label(),
        }
    }
}

/// Allocate the whole control first so the options keep their reading order in right-aligned rows.
pub fn segments<T: Copy + PartialEq>(
    ui: &mut egui::Ui,
    value: &mut T,
    options: &[(T, Segment<'_>)],
) -> bool {
    let p = theme::palette(ui.ctx());
    let font = egui::FontId::proportional(12.0);
    let painter = ui.painter().clone();
    let width = options
        .iter()
        .map(|(_, option)| match option {
            Segment::Text(label) => {
                painter
                    .layout_no_wrap((*label).into(), font.clone(), p.text)
                    .size()
                    .x
                    + 16.0
            }
            Segment::Window(_) => 45.0,
        })
        .fold(35.0_f32, f32::max)
        .ceil();
    let (track, group) = ui.allocate_exact_size(
        egui::vec2(4.0 + width * options.len() as f32, 27.0),
        egui::Sense::hover(),
    );
    painter.rect_filled(track, 7, p.secondary);
    let mut changed = false;
    for (index, (candidate, option)) in options.iter().enumerate() {
        let rect = egui::Rect::from_min_size(
            track.min + egui::vec2(2.0 + index as f32 * width, 2.0),
            egui::vec2(width, track.height() - 4.0),
        );
        let label = option.label();
        let mut response = ui
            .interact(rect, group.id.with(index), egui::Sense::click())
            .on_hover_cursor(egui::CursorIcon::PointingHand);
        if response.clicked() && *value != *candidate {
            *value = *candidate;
            response.mark_changed();
            changed = true;
        }
        let selected = *value == *candidate;
        response.widget_info(|| {
            egui::WidgetInfo::selected(
                egui::WidgetType::RadioButton,
                ui.is_enabled(),
                selected,
                label,
            )
        });
        if selected || response.hovered() {
            painter.rect_filled(rect, 5, p.surface);
        }
        if selected || response.has_focus() {
            let stroke = if response.has_focus() {
                egui::Stroke::new(2.0_f32, theme::ACCENT)
            } else {
                egui::Stroke::new(1.0_f32, p.line)
            };
            painter.rect_stroke(rect, 5, stroke, egui::StrokeKind::Inside);
        }
        let ink = if selected {
            theme::ink(ui.ctx(), theme::ACCENT)
        } else {
            p.text
        };
        match option {
            Segment::Text(text) => {
                painter.text(
                    rect.center(),
                    egui::Align2::CENTER_CENTER,
                    text,
                    font.clone(),
                    ink,
                );
            }
            Segment::Window(size) => {
                let window = egui::Rect::from_center_size(
                    rect.center(),
                    egui::Vec2::from(size.dimensions()) * 0.062,
                );
                let stroke = egui::Stroke::new(1.5_f32, ink);
                painter.rect_stroke(window, 2, stroke, egui::StrokeKind::Inside);
                let bar = window.top() + 4.0;
                painter.line_segment(
                    [
                        egui::pos2(window.left(), bar),
                        egui::pos2(window.right(), bar),
                    ],
                    stroke,
                );
                response.on_hover_text(label);
            }
        }
    }
    changed
}

pub const SETTINGS_ROW_HEIGHT: f32 = 40.0;

pub fn switch(ui: &mut egui::Ui, value: &mut bool, title: &str, hint: &str) -> egui::Response {
    let p = theme::palette(ui.ctx());
    let width = ui.available_width();
    // Let translated descriptions wrap before the switch, including at minimum window width.
    let painter = ui.painter().clone();
    let text_width = (width - 44.0).max(1.0);
    let title_text = painter.layout(
        title.to_owned(),
        egui::TextStyle::Body.resolve(ui.style()),
        p.text,
        text_width,
    );
    let help = (!hint.is_empty()).then(|| {
        painter.layout(
            hint.to_owned(),
            egui::FontId::proportional(12.0),
            p.muted,
            text_width,
        )
    });
    let text_height = title_text.size().y + help.as_ref().map_or(0.0, |g| 2.0 + g.size().y);
    let (rect, mut response) = ui.allocate_exact_size(
        egui::vec2(width, SETTINGS_ROW_HEIGHT.max(text_height + 8.0)),
        egui::Sense::click(),
    );
    let top = egui::pos2(rect.left(), rect.center().y - text_height * 0.5);
    let help_top = top + egui::vec2(0.0, title_text.size().y + 2.0);
    painter.galley(top, title_text, p.text);
    if let Some(help) = help {
        painter.galley(help_top, help, p.muted);
    }
    if response.clicked() {
        *value = !*value;
        response.mark_changed();
    }
    response.widget_info(|| {
        egui::WidgetInfo::selected(egui::WidgetType::Checkbox, ui.is_enabled(), *value, title)
    });
    let track = egui::Rect::from_center_size(
        egui::pos2(rect.right() - 16.0, rect.center().y),
        egui::vec2(32.0, 19.0),
    );
    ui.painter()
        .rect_filled(track, 10, if *value { theme::ACCENT } else { p.line });
    if response.has_focus() {
        ui.painter().rect_stroke(
            track.expand(2.0),
            12,
            egui::Stroke::new(1.5_f32, theme::ACCENT),
            egui::StrokeKind::Outside,
        );
    }
    let fraction = ui.ctx().animate_bool_with_time(response.id, *value, 0.1);
    ui.painter().circle_filled(
        egui::pos2(track.left() + 9.5 + fraction * 13.0, track.center().y),
        7.5,
        egui::Color32::WHITE,
    );
    response.on_hover_cursor(egui::CursorIcon::PointingHand)
}

pub fn group<R>(ui: &mut egui::Ui, title: &str, content: impl FnOnce(&mut egui::Ui) -> R) -> R {
    let p = theme::palette(ui.ctx());
    ui.label(RichText::new(title).size(13.0).strong());
    egui::Frame::new()
        .fill(p.surface)
        .stroke(egui::Stroke::new(0.7_f32, p.line))
        .corner_radius(10)
        .inner_margin(12)
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            content(ui)
        })
        .inner
}

#[cfg(test)]
mod keyboard_tests {
    use super::*;

    #[test]
    fn escape_closes_the_popup_before_the_parent_view() {
        let ctx = egui::Context::default();
        let popup = egui::Id::new("period_menu");
        let escape = egui::Event::Key {
            key: egui::Key::Escape,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        };
        let input = egui::RawInput {
            events: vec![escape.clone()],
            ..Default::default()
        };
        let _ = ctx.run(input, |ctx| {
            egui::Popup::open_id(ctx, popup);
            assert!(!escape_outside_popup(ctx));
            egui::CentralPanel::default().show(ctx, |ui| {
                egui::Popup::menu(&ui.button("Period"))
                    .id(popup)
                    .show(|ui| ui.label("3 h"));
            });
            assert!(!egui::Popup::is_id_open(ctx, popup));
        });
        let _ = ctx.run(
            egui::RawInput {
                events: vec![escape],
                ..Default::default()
            },
            |ctx| {
                assert!(escape_outside_popup(ctx));
            },
        );
    }
}

use crate::storage::WidgetSize;
pub mod dashboard;
pub mod popover;
pub mod settings;
pub mod theme;
pub mod widget;
use eframe::egui::{self, RichText};

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
        egui::Button::new(
            RichText::new(text)
                .color(egui::Color32::WHITE)
                .size(14.0)
                .strong(),
        )
        .fill(theme::ACCENT)
        .corner_radius(9)
        .min_size(egui::vec2(0.0, 44.0)),
    )
}
pub fn pill(ui: &mut egui::Ui, text: &str, color: egui::Color32) {
    egui::Frame::new()
        .fill(color.gamma_multiply(0.12))
        .corner_radius(6)
        .inner_margin(egui::Margin::symmetric(9, 5))
        .show(ui, |ui| {
            ui.label(
                RichText::new(text)
                    .size(11.0)
                    .strong()
                    .color(theme::ink(ui.ctx(), color)),
            );
        });
}
pub fn logo(ui: &mut egui::Ui, size: f32) {
    ui.add(
        egui::Image::new(egui::include_image!("../../assets/logo.svg"))
            .fit_to_exact_size(egui::vec2(size, size)),
    );
}

/// A choice is visibly a button even before hover. Selection has both color and a check.
pub fn choice<T: PartialEq + Copy>(
    ui: &mut egui::Ui,
    value: &mut T,
    candidate: T,
    label: &str,
    width: f32,
) -> bool {
    let selected = *value == candidate;
    let p = theme::palette(ui.ctx());
    let text = if selected {
        format!("✓  {label}")
    } else {
        label.into()
    };
    let response = ui.add_sized(
        [width, 42.0],
        egui::Button::new(RichText::new(text).color(if selected {
            egui::Color32::WHITE
        } else {
            p.text
        }))
        .fill(if selected { theme::ACCENT } else { p.secondary })
        .stroke(egui::Stroke::new(
            1.0_f32,
            if selected { theme::ACCENT } else { p.line },
        )),
    );
    response.widget_info(|| {
        egui::WidgetInfo::selected(
            egui::WidgetType::RadioButton,
            ui.is_enabled(),
            selected,
            label,
        )
    });
    if response.clicked() && !selected {
        *value = candidate;
        true
    } else {
        false
    }
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
    let font = egui::FontId::proportional(14.0);
    let painter = ui.painter().clone();
    let width = options
        .iter()
        .map(|(_, option)| match option {
            Segment::Text(label) => {
                painter
                    .layout_no_wrap((*label).into(), font.clone(), p.text)
                    .size()
                    .x
                    + 22.0
            }
            Segment::Window(_) => 52.0,
        })
        .fold(42.0_f32, f32::max)
        .ceil();
    let (track, group) = ui.allocate_exact_size(
        egui::vec2(4.0 + width * options.len() as f32, 34.0),
        egui::Sense::hover(),
    );
    painter.rect_filled(track, 8, p.secondary);
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
            painter.rect_filled(rect, 6, p.surface);
        }
        if selected || response.has_focus() {
            let stroke = if response.has_focus() {
                egui::Stroke::new(2.0_f32, theme::ACCENT)
            } else {
                egui::Stroke::new(1.0_f32, p.line)
            };
            painter.rect_stroke(rect, 6, stroke, egui::StrokeKind::Inside);
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
                    egui::Vec2::from(size.dimensions()) * 0.074,
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

pub fn switch(ui: &mut egui::Ui, value: &mut bool, title: &str, hint: &str) -> egui::Response {
    let p = theme::palette(ui.ctx());
    let height = if hint.is_empty() { 42.0 } else { 54.0 };
    let (rect, mut response) = ui.allocate_exact_size(
        egui::vec2(ui.available_width(), height),
        egui::Sense::click(),
    );
    if response.clicked() {
        *value = !*value;
        response.mark_changed();
    }
    response.widget_info(|| {
        egui::WidgetInfo::selected(egui::WidgetType::Checkbox, ui.is_enabled(), *value, title)
    });
    if response.hovered() || response.has_focus() {
        ui.painter().rect_filled(rect, 9, p.secondary);
    }
    if response.has_focus() {
        ui.painter().rect_stroke(
            rect,
            9,
            egui::Stroke::new(1.5_f32, theme::ACCENT),
            egui::StrokeKind::Inside,
        );
    }
    let y = if hint.is_empty() {
        rect.center().y
    } else {
        rect.top() + 19.0
    };
    ui.painter().text(
        egui::pos2(rect.left(), y),
        egui::Align2::LEFT_CENTER,
        title,
        egui::FontId::proportional(15.0),
        p.text,
    );
    if !hint.is_empty() {
        ui.painter().text(
            rect.left_top() + egui::vec2(0.0, 37.0),
            egui::Align2::LEFT_CENTER,
            hint,
            egui::FontId::proportional(12.0),
            p.muted,
        );
    }
    let track = egui::Rect::from_center_size(
        egui::pos2(rect.right() - 22.0, rect.center().y),
        egui::vec2(44.0, 26.0),
    );
    ui.painter()
        .rect_filled(track, 14, if *value { theme::ACCENT } else { p.line });
    let fraction = ui.ctx().animate_bool_with_time(response.id, *value, 0.1);
    ui.painter().circle_filled(
        egui::pos2(track.left() + 13.0 + fraction * 18.0, track.center().y),
        10.0,
        egui::Color32::WHITE,
    );
    response.on_hover_cursor(egui::CursorIcon::PointingHand)
}

pub fn group<R>(ui: &mut egui::Ui, title: &str, content: impl FnOnce(&mut egui::Ui) -> R) -> R {
    let p = theme::palette(ui.ctx());
    egui::Frame::new()
        .fill(p.surface)
        .corner_radius(14)
        .inner_margin(16)
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.label(RichText::new(title).size(18.0).strong());
            ui.add_space(6.0);
            content(ui)
        })
        .inner
}

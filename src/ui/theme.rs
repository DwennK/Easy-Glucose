use crate::storage::Theme;
use eframe::egui::{self, Color32, FontId, TextStyle};
pub const ACCENT: Color32 = Color32::from_rgb(0, 102, 204);
pub const GREEN: Color32 = Color32::from_rgb(45, 178, 126);
pub const ORANGE: Color32 = Color32::from_rgb(232, 166, 66);
pub const RED: Color32 = Color32::from_rgb(230, 100, 112);
#[derive(Clone, Copy)]
pub struct Palette {
    pub bg: Color32,
    pub surface: Color32,
    pub secondary: Color32,
    pub text: Color32,
    pub muted: Color32,
    pub line: Color32,
}
fn colors(dark: bool) -> Palette {
    if dark {
        Palette {
            bg: Color32::from_rgb(30, 30, 32),
            surface: Color32::from_rgb(40, 40, 42),
            secondary: Color32::from_rgb(58, 58, 61),
            text: Color32::from_rgb(242, 242, 245),
            muted: Color32::from_rgb(183, 183, 190),
            line: Color32::from_rgb(79, 79, 84),
        }
    } else {
        Palette {
            bg: Color32::from_rgb(245, 245, 247),
            surface: Color32::WHITE,
            secondary: Color32::from_rgb(233, 233, 237),
            text: Color32::from_rgb(30, 30, 33),
            muted: Color32::from_rgb(96, 96, 104),
            line: Color32::from_rgb(207, 207, 212),
        }
    }
}
pub fn palette(ctx: &egui::Context) -> Palette {
    colors(ctx.style().visuals.dark_mode)
}
pub fn install(ctx: &egui::Context) {
    egui_extras::install_image_loaders(ctx);
    let mut fonts = egui::FontDefinitions::default();
    let path = if cfg!(target_os = "macos") {
        "/System/Library/Fonts/SFNS.ttf"
    } else {
        "C:\\Windows\\Fonts\\segoeui.ttf"
    };
    if let Ok(bytes) = std::fs::read(path) {
        fonts
            .font_data
            .insert("system".into(), egui::FontData::from_owned(bytes).into());
        fonts
            .families
            .entry(egui::FontFamily::Proportional)
            .or_default()
            .insert(0, "system".into());
    }
    ctx.set_fonts(fonts);
}
pub fn apply(ctx: &egui::Context, theme: Theme) {
    ctx.set_theme(match theme {
        Theme::Dark => egui::ThemePreference::Dark,
        Theme::Light => egui::ThemePreference::Light,
        Theme::System => egui::ThemePreference::System,
    });
    for mode in [egui::Theme::Dark, egui::Theme::Light] {
        ctx.style_mut_of(mode, |s| {
            let dark = mode == egui::Theme::Dark;
            let p = colors(dark);
            s.visuals = if dark {
                egui::Visuals::dark()
            } else {
                egui::Visuals::light()
            };
            s.visuals.panel_fill = p.bg;
            s.visuals.window_fill = p.surface;
            s.visuals.extreme_bg_color = p.surface;
            s.visuals.override_text_color = Some(p.text);
            s.visuals.selection.bg_fill = ACCENT;
            s.visuals.selection.stroke = egui::Stroke::new(1.0_f32, Color32::WHITE);
            let idle = &mut s.visuals.widgets.inactive;
            idle.bg_fill = p.surface;
            idle.weak_bg_fill = p.surface;
            idle.bg_stroke = egui::Stroke::new(1.0_f32, p.line);
            idle.fg_stroke = egui::Stroke::new(1.2_f32, p.text);
            let hovered = &mut s.visuals.widgets.hovered;
            hovered.bg_fill = if dark {
                Color32::from_rgb(70, 70, 74)
            } else {
                Color32::from_rgb(226, 226, 231)
            };
            hovered.weak_bg_fill = hovered.bg_fill;
            hovered.bg_stroke = egui::Stroke::new(1.0_f32, p.line);
            hovered.fg_stroke = egui::Stroke::new(1.5_f32, p.text);
            let active = &mut s.visuals.widgets.active;
            active.bg_fill = p.secondary;
            active.weak_bg_fill = p.secondary;
            active.bg_stroke = egui::Stroke::new(1.0_f32, p.line);
            active.fg_stroke = egui::Stroke::new(1.5_f32, p.text);
            s.visuals.widgets.noninteractive.bg_stroke =
                egui::Stroke::new(1.0_f32, p.line.gamma_multiply(0.45));
            s.visuals.widgets.noninteractive.fg_stroke = egui::Stroke::new(1.0_f32, p.muted);
            for w in [
                &mut s.visuals.widgets.inactive,
                &mut s.visuals.widgets.hovered,
                &mut s.visuals.widgets.active,
            ] {
                w.corner_radius = egui::CornerRadius::same(6);
            }
            s.spacing.item_spacing = egui::vec2(8.0, 6.0);
            s.spacing.button_padding = egui::vec2(9.0, 4.0);
            s.spacing.interact_size = egui::vec2(26.0, 26.0);
            s.spacing.combo_width = 158.0;
            s.visuals.window_corner_radius = egui::CornerRadius::same(12);
            s.visuals.window_stroke = egui::Stroke::new(1.0_f32, p.line);
            s.animation_time = 0.1;
            s.text_styles
                .insert(TextStyle::Body, FontId::proportional(13.0));
            s.text_styles
                .insert(TextStyle::Button, FontId::proportional(13.0));
            s.text_styles
                .insert(TextStyle::Small, FontId::proportional(12.0));
            s.text_styles
                .insert(TextStyle::Heading, FontId::proportional(20.0));
        });
    }
}

/// Darker semantic inks preserve contrast on the light surfaces.
pub fn ink(ctx: &egui::Context, color: Color32) -> Color32 {
    if ctx.style().visuals.dark_mode {
        return if color == ACCENT {
            Color32::from_rgb(144, 187, 255)
        } else {
            color
        };
    }
    match color {
        GREEN => Color32::from_rgb(18, 125, 82),
        ORANGE => Color32::from_rgb(145, 88, 18),
        RED => Color32::from_rgb(183, 49, 65),
        ACCENT => Color32::from_rgb(36, 86, 188),
        _ => color,
    }
}

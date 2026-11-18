use crate::storage::Theme;
use eframe::egui::{self, Color32, FontId, TextStyle};
pub const ACCENT: Color32 = Color32::from_rgb(55, 112, 225);
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
            bg: Color32::from_rgb(17, 23, 32),
            surface: Color32::from_rgb(25, 34, 46),
            secondary: Color32::from_rgb(38, 50, 66),
            text: Color32::from_rgb(241, 245, 251),
            muted: Color32::from_rgb(170, 183, 202),
            line: Color32::from_rgb(67, 83, 105),
        }
    } else {
        Palette {
            bg: Color32::from_rgb(244, 246, 249),
            surface: Color32::WHITE,
            secondary: Color32::from_rgb(235, 240, 248),
            text: Color32::from_rgb(28, 41, 60),
            muted: Color32::from_rgb(95, 110, 132),
            line: Color32::from_rgb(190, 202, 219),
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
            s.visuals.selection.stroke = egui::Stroke::new(1.0, Color32::WHITE);
            let idle = &mut s.visuals.widgets.inactive;
            idle.bg_fill = p.secondary;
            idle.weak_bg_fill = p.secondary;
            idle.bg_stroke = egui::Stroke::new(1.0, p.line);
            idle.fg_stroke = egui::Stroke::new(1.2, p.text);
            let hovered = &mut s.visuals.widgets.hovered;
            hovered.bg_fill = if dark {
                Color32::from_rgb(45, 66, 94)
            } else {
                Color32::from_rgb(219, 232, 255)
            };
            hovered.weak_bg_fill = hovered.bg_fill;
            hovered.bg_stroke = egui::Stroke::new(1.5, ACCENT);
            hovered.fg_stroke = egui::Stroke::new(1.5, p.text);
            let active = &mut s.visuals.widgets.active;
            active.bg_fill = p.secondary;
            active.weak_bg_fill = p.secondary;
            active.bg_stroke = egui::Stroke::new(1.5, ACCENT);
            active.fg_stroke = egui::Stroke::new(1.5, p.text);
            s.visuals.widgets.noninteractive.bg_stroke =
                egui::Stroke::new(1.0, p.line.gamma_multiply(0.45));
            s.visuals.widgets.noninteractive.fg_stroke = egui::Stroke::new(1.0, p.muted);
            for w in [
                &mut s.visuals.widgets.inactive,
                &mut s.visuals.widgets.hovered,
                &mut s.visuals.widgets.active,
            ] {
                w.corner_radius = egui::CornerRadius::same(10);
            }
            s.spacing.item_spacing = egui::vec2(10.0, 10.0);
            s.spacing.button_padding = egui::vec2(16.0, 11.0);
            s.spacing.interact_size = egui::vec2(40.0, 40.0);
            s.spacing.combo_width = 240.0;
            s.visuals.window_corner_radius = egui::CornerRadius::same(18);
            s.visuals.window_stroke = egui::Stroke::new(1.0, p.line);
            s.animation_time = 0.1;
            s.text_styles
                .insert(TextStyle::Body, FontId::proportional(15.0));
            s.text_styles
                .insert(TextStyle::Button, FontId::proportional(14.0));
            s.text_styles
                .insert(TextStyle::Small, FontId::proportional(12.0));
            s.text_styles
                .insert(TextStyle::Heading, FontId::proportional(28.0));
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

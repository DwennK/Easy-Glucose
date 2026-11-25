use crate::{i18n::Text, storage::Config};
use eframe::egui;
use std::sync::mpsc::{self, Receiver};
#[cfg(windows)]
use tray_icon::menu::CheckMenuItem;
use tray_icon::{
    TrayIcon, TrayIconBuilder,
    menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem},
};

#[derive(Clone, Copy)]
pub enum Action {
    Dashboard,
    Widget,
    #[cfg(windows)]
    ClockWidget,
    Settings,
    #[cfg(target_os = "macos")]
    Popover,
    Quit,
}
pub struct Tray {
    icon: TrayIcon,
    items: [MenuItem; 3],
    #[cfg(not(windows))]
    widget_item: MenuItem,
    #[cfg(windows)]
    widget_item: CheckMenuItem,
    #[cfg(windows)]
    clock_item: CheckMenuItem,
    pub events: Receiver<Action>,
    last_title: String,
}
impl Tray {
    pub fn new(ctx: egui::Context, _config: &Config) -> Result<Self, String> {
        let menu = Menu::new();
        let dashboard = MenuItem::new(Text::OpenApp.get(), true, None);
        let settings = MenuItem::new(Text::Settings.get(), true, None);
        #[cfg(not(windows))]
        let widget = MenuItem::new(Text::ToggleWidget.get(), true, None);
        #[cfg(windows)]
        let widget = CheckMenuItem::new(Text::ToggleWidget.get(), true, _config.widget_open, None);
        #[cfg(windows)]
        let clock_item =
            CheckMenuItem::new(Text::ClockWidget.get(), true, _config.clock_widget, None);
        let quit = MenuItem::new(Text::Quit.get(), true, None);
        menu.append_items(&[
            &dashboard,
            &settings,
            &widget,
            #[cfg(windows)]
            &clock_item,
            &PredefinedMenuItem::separator(),
            &quit,
        ])
        .map_err(|_| Text::TrayError.english())?;
        // 44 pixels map exactly to the 22-point macOS menu item on Retina displays.
        let icon = tray_icon::Icon::from_rgba(icon_rgba(44, cfg!(target_os = "macos")), 44, 44)
            .map_err(|_| Text::IconError.english())?;
        let builder = TrayIconBuilder::new();
        #[cfg(target_os = "macos")]
        let builder = builder
            .with_icon_templated(icon)
            .with_menu_on_left_click(false);
        #[cfg(not(target_os = "macos"))]
        let builder = builder.with_icon(icon);
        let icon = builder
            .with_tooltip(format!("Easy Glucose · {}", Text::NoReading.get()))
            .with_menu(Box::new(menu))
            .build()
            .map_err(|_| Text::TrayUnavailable.english())?;
        let (tx, rx) = mpsc::channel();
        #[cfg(target_os = "macos")]
        {
            let click_tx = tx.clone();
            let click_ctx = ctx.clone();
            let id = icon.id().clone();
            tray_icon::TrayIconEvent::set_event_handler(Some(move |event| {
                if let tray_icon::TrayIconEvent::Click {
                    id: clicked,
                    button: tray_icon::MouseButton::Left,
                    button_state: tray_icon::MouseButtonState::Up,
                    ..
                } = event
                    && clicked == id
                {
                    let _ = click_tx.send(Action::Popover);
                    click_ctx.request_repaint_of(egui::ViewportId::ROOT);
                }
            }));
        }
        let sid = settings.id().clone();
        let did = dashboard.id().clone();
        let wid = widget.id().clone();
        #[cfg(windows)]
        let cid = clock_item.id().clone();
        let qid = quit.id().clone();
        MenuEvent::set_event_handler(Some(move |event: MenuEvent| {
            #[cfg(windows)]
            if event.id == cid {
                let _ = tx.send(Action::ClockWidget);
                ctx.request_repaint_of(egui::ViewportId::ROOT);
                return;
            }
            let action = if event.id == did {
                Some(Action::Dashboard)
            } else if event.id == sid {
                Some(Action::Settings)
            } else if event.id == wid {
                Some(Action::Widget)
            } else if event.id == qid {
                Some(Action::Quit)
            } else {
                None
            };
            if let Some(action) = action {
                let _ = tx.send(action);
                ctx.request_repaint_of(egui::ViewportId::ROOT);
            }
        }));
        Ok(Self {
            icon,
            items: [dashboard, settings, quit],
            widget_item: widget,
            #[cfg(windows)]
            clock_item,
            events: rx,
            last_title: String::new(),
        })
    }
    /// Cocoa coordinates are points. Use the status item's own display, not the
    /// main window's Retina scale, so a second monitor can have a different DPI.
    #[cfg(target_os = "macos")]
    pub fn popover_position(&self, size: egui::Vec2) -> Option<egui::Pos2> {
        let mtm = objc2::MainThreadMarker::new()?;
        let item = self.icon.ns_status_item()?;
        let button = item.button(mtm)?;
        let window = button.window()?;
        let screen = window.screen()?;
        let screens = objc2_app_kit::NSScreen::screens(mtm);
        let primary_height = screens.firstObject()?.frame().size.height;
        let to_rect = |x: f64, y: f64, width: f64, height: f64| {
            egui::Rect::from_min_size(
                egui::pos2(x as f32, (primary_height - y - height) as f32),
                egui::vec2(width as f32, height as f32),
            )
        };
        // The status window can include padding beyond the clickable button.
        let anchor = window.convertRectToScreen(button.convertRect_toView(button.bounds(), None));
        let bounds = screen.visibleFrame();
        let position = crate::ui::popover::position(
            to_rect(
                anchor.origin.x,
                anchor.origin.y,
                anchor.size.width,
                anchor.size.height,
            ),
            to_rect(
                bounds.origin.x,
                bounds.origin.y,
                bounds.size.width,
                bounds.size.height,
            ),
            size,
        );
        if std::env::args().any(|a| a == "--diagnostics") {
            eprintln!(
                "diagnostic: menu button={anchor:?} window={:?} screen={bounds:?} primary_height={primary_height} target={position:?}",
                window.frame()
            );
        }
        Some(position)
    }
    #[cfg(windows)]
    pub fn sync_widget_checks(&self, config: &Config) {
        for (item, checked) in [
            (&self.widget_item, config.widget_open),
            (&self.clock_item, config.clock_widget),
        ] {
            if item.is_checked() != checked {
                item.set_checked(checked);
            }
        }
    }
    pub fn localize(&self) {
        self.widget_item.set_text(Text::ToggleWidget.get());
        #[cfg(windows)]
        self.clock_item.set_text(Text::ClockWidget.get());
        for (item, text) in self
            .items
            .iter()
            .zip([Text::OpenApp, Text::Settings, Text::Quit])
        {
            item.set_text(text.get());
        }
    }
    pub fn update(&mut self, title: &str, tooltip: &str) {
        let combined = format!("{title}|{tooltip}");
        if self.last_title == combined {
            return;
        }
        self.icon.set_title(Some(title));
        if self.icon.set_tooltip(Some(tooltip)).is_err() {
            tracing::warn!("tray_tooltip_update_failed");
        }
        self.last_title = combined;
    }
}
/// Rasterize the vector originals with antialiasing, including the macOS alpha mask.
pub fn icon_rgba(size: u32, monochrome: bool) -> Vec<u8> {
    let svg: &[u8] = if monochrome {
        include_bytes!("../../assets/tray.svg")
    } else {
        include_bytes!("../../assets/app-icon.svg")
    };
    let image = egui_extras::image::load_svg_bytes_with_size(
        svg,
        egui::load::SizeHint::Width(size),
        &Default::default(),
    )
    .expect("bundled icon SVG must be valid");
    image
        .pixels
        .iter()
        .flat_map(|pixel| pixel.to_srgba_unmultiplied())
        .collect()
}

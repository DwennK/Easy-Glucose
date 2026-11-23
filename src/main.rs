#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]
use easy_glucose::{app::App, platform::tray::icon_rgba, storage::Store};
use eframe::egui;
use fs2::FileExt;
use std::fs::OpenOptions;
use tracing_subscriber::prelude::*;

fn application_icon() -> egui::IconData {
    // In a macOS bundle, the empty IconData tells eframe to keep the native
    // multi-resolution ICNS. Omitting the icon selects eframe's own default.
    #[cfg(target_os = "macos")]
    if std::env::current_exe()
        .ok()
        .and_then(|exe| {
            exe.parent()?
                .parent()
                .map(|p| p.join("Resources/AppIcon.icns"))
        })
        .is_some_and(|icon| icon.is_file())
    {
        return egui::IconData::default();
    }
    egui::IconData {
        rgba: icon_rgba(256, false),
        width: 256,
        height: 256,
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    velopack::VelopackApp::build()
        .set_auto_apply_on_startup(false)
        .run();
    // Fixed filter: dependencies cannot accidentally log HTTP requests or authentication URLs.
    let started = std::time::Instant::now();
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::fmt::layer()
                .with_target(false)
                .with_filter(tracing_subscriber::filter::filter_fn(|metadata| {
                    metadata.target().starts_with("easy_glucose")
                        && *metadata.level() <= tracing::Level::WARN
                })),
        )
        .init();
    easy_glucose::i18n::activate(None);
    let store = Store::open().map_err(|e| easy_glucose::i18n::notice(&e).to_owned())?;
    let lock = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(store.directory.join("instance.lock"))?;
    if lock.try_lock_exclusive().is_err() {
        eprintln!("{}", easy_glucose::i18n::Text::AlreadyOpen.get());
        return Ok(());
    }
    let (config, notice) = match store.load_config() {
        Ok(c) => (c, None),
        Err(e) => (Default::default(), Some(e)),
    };
    let args: Vec<_> = std::env::args().collect();
    let demo = args.iter().any(|s| s == "--demo");
    let background = args.iter().any(|s| s == "--background");
    let diagnostics = args.iter().any(|s| s == "--diagnostics");
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Easy Glucose")
            // eframe selects the shared GL alpha configuration from this initial flag.
            // App::update makes only the root window opaque on its first frame;
            // the menu popup still needs this alpha-capable GL configuration.
            .with_transparent(cfg!(target_os = "macos"))
            .with_fullsize_content_view(cfg!(target_os = "macos"))
            .with_titlebar_shown(!cfg!(target_os = "macos"))
            .with_title_shown(!cfg!(target_os = "macos"))
            .with_inner_size([600.0, 570.0])
            .with_min_inner_size([510.0, 440.0])
            .with_icon(application_icon()),
        renderer: eframe::Renderer::Glow,
        vsync: true,
        centered: true,
        ..Default::default()
    };
    eframe::run_native(
        "Easy Glucose",
        options,
        Box::new(move |cc| {
            let mut app = App::new(cc, store, config, notice, demo, background)?;
            if diagnostics {
                app.diagnostics_start = Some(started);
            }
            Ok(Box::new(app))
        }),
    )?;
    drop(lock);
    Ok(())
}

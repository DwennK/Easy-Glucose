#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]
use easy_glucose::{app::App, platform::tray::icon_rgba, storage::Store};
use eframe::egui;
use fs2::FileExt;
use std::fs::OpenOptions;
use tracing_subscriber::prelude::*;

fn main() -> Result<(), Box<dyn std::error::Error>> {
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
    let store = Store::open()?;
    let lock = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(store.directory.join("instance.lock"))?;
    if lock.try_lock_exclusive().is_err() {
        eprintln!("Easy Glucose est déjà ouvert. Utilisez son icône dans la barre de menus.");
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
            .with_inner_size([1120.0, 800.0])
            .with_min_inner_size([900.0, 660.0])
            .with_icon(egui::IconData {
                rgba: icon_rgba(64, false),
                width: 64,
                height: 64,
            }),
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

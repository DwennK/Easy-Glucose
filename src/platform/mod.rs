pub mod tray;

pub fn set_autostart(enabled: bool) -> Result<(), String> {
    let exe = std::env::current_exe().map_err(|_| "Chemin de l’application indisponible.")?;
    let path = exe.to_str().ok_or("Chemin de l’application invalide.")?;
    let mut builder = auto_launch::AutoLaunchBuilder::new();
    builder.set_app_name("Easy Glucose").set_app_path(path);
    #[cfg(target_os = "macos")]
    builder.set_macos_launch_mode(auto_launch::MacOSLaunchMode::LaunchAgent);
    let launcher = builder
        .build()
        .map_err(|_| "Configuration du lancement automatique impossible.")?;
    if enabled {
        launcher.enable()
    } else {
        launcher.disable()
    }
    .map_err(|_| "Le système a refusé la modification du lancement automatique.".into())
}

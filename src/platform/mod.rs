use crate::i18n::Text;
#[cfg(windows)]
pub mod clock;
pub mod tray;

pub fn set_autostart(enabled: bool) -> Result<(), String> {
    let exe = std::env::current_exe().map_err(|_| Text::ExePath.english())?;
    let path = exe.to_str().ok_or(Text::InvalidPath.english())?;
    let mut builder = auto_launch::AutoLaunchBuilder::new();
    builder.set_app_name("Easy Glucose").set_app_path(path);
    #[cfg(target_os = "macos")]
    builder.set_macos_launch_mode(auto_launch::MacOSLaunchMode::LaunchAgent);
    let launcher = builder.build().map_err(|_| Text::AutoConfig.english())?;
    if enabled {
        launcher.enable()
    } else {
        launcher.disable()
    }
    .map_err(|_| Text::AutoDenied.english().into())
}

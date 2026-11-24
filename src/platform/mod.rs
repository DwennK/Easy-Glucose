use crate::i18n::Text;
#[cfg(windows)]
pub mod clock;
#[cfg(windows)]
pub mod dashboard;
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

#[cfg(windows)]
pub fn enable_window_alpha(title: &str) {
    use std::ptr::null_mut;
    use windows_sys::Win32::{
        Graphics::{
            Dwm::*,
            Gdi::{CreateRectRgn, DeleteObject},
        },
        UI::WindowsAndMessaging::{FindWindowExW, GetPropW, GetWindowThreadProcessId, SetPropW},
    };
    // glutin can clear the transparency flag on WGL windows even with an RGBA
    // pixel format. Enable composition once per native window, including reopens.
    let title: Vec<u16> = title.encode_utf16().chain([0]).collect();
    unsafe {
        let mut hwnd = null_mut();
        loop {
            hwnd = FindWindowExW(null_mut(), hwnd, std::ptr::null(), title.as_ptr());
            if hwnd.is_null() {
                return;
            }
            let mut pid = 0;
            GetWindowThreadProcessId(hwnd, &mut pid);
            if pid == std::process::id() {
                break;
            }
        }
        let property = windows_sys::core::w!("EasyGlucose.Composited");
        if !GetPropW(hwnd, property).is_null() {
            return;
        }
        let region = CreateRectRgn(0, 0, -1, -1);
        if region.is_null() {
            return;
        }
        let blur = DWM_BLURBEHIND {
            dwFlags: DWM_BB_ENABLE | DWM_BB_BLURREGION,
            fEnable: 1,
            hRgnBlur: region,
            ..Default::default()
        };
        if DwmEnableBlurBehindWindow(hwnd, &blur) >= 0 {
            SetPropW(hwnd, property, std::ptr::dangling_mut());
        }
        DeleteObject(region);
    }
}

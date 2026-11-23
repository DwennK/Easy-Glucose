use eframe::egui;
use sha2::{Digest, Sha256};
use std::{mem::size_of, os::windows::ffi::OsStrExt, path::Path, ptr, time::Duration};
use windows_sys::Win32::{
    Foundation::{HANDLE, HWND},
    UI::Shell::{DefSubclassProc, RemoveWindowSubclass, SetWindowSubclass},
    UI::WindowsAndMessaging::{
        AllowSetForegroundWindow, FindWindowExW, GetPropW, GetWindowThreadProcessId, IsIconic,
        IsWindowVisible, PostMessageW, RegisterWindowMessageW, RemovePropW, SW_RESTORE, SW_SHOW,
        SetPropW, ShowWindowAsync,
    },
};

// Each profile owns an independent instance lock and dashboard.
fn activation_property(directory: &Path) -> Vec<u16> {
    let path = directory
        .canonicalize()
        .unwrap_or_else(|_| directory.to_owned());
    let mut hash = Sha256::new();
    for unit in path.as_os_str().encode_wide() {
        hash.update(unit.to_le_bytes());
    }
    format!("EasyGlucose.Activate.{:x}", hash.finalize())
        .encode_utf16()
        .chain([0])
        .collect()
}

pub fn activate_existing(directory: &Path) -> bool {
    let property = activation_property(directory);
    // A second launch can arrive after the file lock but before window creation.
    let started = std::time::Instant::now();
    loop {
        unsafe {
            let mut hwnd = ptr::null_mut();
            loop {
                hwnd = FindWindowExW(
                    ptr::null_mut(),
                    hwnd,
                    ptr::null(),
                    windows_sys::core::w!("Easy Glucose"),
                );
                if hwnd.is_null() {
                    break;
                }
                let message = GetPropW(hwnd, property.as_ptr()) as usize as u32;
                if message == 0 {
                    continue;
                }
                let mut pid = 0;
                GetWindowThreadProcessId(hwnd, &mut pid);
                AllowSetForegroundWindow(pid);
                return PostMessageW(hwnd, message, 0, 0) != 0;
            }
        }
        if started.elapsed() >= Duration::from_secs(5) {
            return false;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

struct Activation {
    ctx: egui::Context,
    requested: std::cell::Cell<bool>,
}

unsafe extern "system" fn on_message(
    hwnd: HWND,
    msg: u32,
    wparam: usize,
    lparam: isize,
    message_id: usize,
    data: usize,
) -> isize {
    if msg as usize == message_id {
        // The boxed state lives on the window thread until the subclass is removed.
        let activation = unsafe { &*(data as *const Activation) };
        activation.requested.set(true);
        unsafe {
            if IsIconic(hwnd) != 0 {
                ShowWindowAsync(hwnd, SW_RESTORE);
            } else if IsWindowVisible(hwnd) == 0 {
                ShowWindowAsync(hwnd, SW_SHOW);
            }
        }
        activation.ctx.request_repaint_of(egui::ViewportId::ROOT);
        return 0;
    }
    unsafe { DefSubclassProc(hwnd, msg, wparam, lparam) }
}

// Immediate egui viewports depend on WM_PAINT for the main window. Keep its
// message pump running while hiding it from the compositor and task switcher.
pub struct DashboardWindow {
    handle: raw_window_handle::Win32WindowHandle,
    saved_style: Option<isize>,
    property: Vec<u16>,
    message: usize,
    activation: Box<Activation>,
}

impl DashboardWindow {
    pub fn new(
        handle: raw_window_handle::Win32WindowHandle,
        directory: &Path,
        ctx: egui::Context,
    ) -> std::io::Result<Self> {
        let property = activation_property(directory);
        let activation = Box::new(Activation {
            ctx,
            requested: Default::default(),
        });
        let hwnd = handle.hwnd.get() as HWND;
        let message =
            unsafe { RegisterWindowMessageW(windows_sys::core::w!("EasyGlucose.ShowDashboard")) }
                as usize;
        unsafe {
            if message == 0
                || SetWindowSubclass(
                    hwnd,
                    Some(on_message),
                    message,
                    (&*activation as *const Activation) as usize,
                ) == 0
            {
                return Err(std::io::Error::last_os_error());
            }
            if SetPropW(hwnd, property.as_ptr(), message as HANDLE) == 0 {
                let error = std::io::Error::last_os_error();
                RemoveWindowSubclass(hwnd, Some(on_message), message);
                return Err(error);
            }
        }
        Ok(Self {
            handle,
            saved_style: None,
            property,
            message,
            activation,
        })
    }

    pub fn take_open_request(&self) -> bool {
        self.activation.requested.replace(false)
    }

    pub fn set_hidden(&mut self, hidden: bool) -> bool {
        use windows_sys::Win32::{
            Graphics::Dwm::{DWMWA_CLOAK, DwmSetWindowAttribute},
            UI::WindowsAndMessaging::{
                GWL_EXSTYLE, GetWindowLongPtrW, SW_HIDE, SW_SHOWNOACTIVATE, SetWindowLongPtrW,
                ShowWindow, WS_EX_APPWINDOW, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW,
            },
        };
        if self.saved_style.is_some() == hidden {
            return true;
        }
        let hwnd = self.handle.hwnd.get() as HWND;
        let cloak = i32::from(hidden);
        unsafe {
            if DwmSetWindowAttribute(
                hwnd,
                DWMWA_CLOAK as u32,
                (&raw const cloak).cast(),
                size_of::<i32>() as u32,
            ) < 0
            {
                return false;
            }
            ShowWindow(hwnd, SW_HIDE);
            let style = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
            let mask = (WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE | WS_EX_APPWINDOW) as isize;
            let flags = if hidden {
                self.saved_style = Some(style & mask);
                (WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE) as isize
            } else {
                self.saved_style.take().unwrap_or_default()
            };
            SetWindowLongPtrW(hwnd, GWL_EXSTYLE, (style & !mask) | flags);
            ShowWindow(hwnd, SW_SHOWNOACTIVATE);
        }
        true
    }
}

impl Drop for DashboardWindow {
    fn drop(&mut self) {
        unsafe {
            let hwnd = self.handle.hwnd.get() as HWND;
            RemovePropW(hwnd, self.property.as_ptr());
            RemoveWindowSubclass(hwnd, Some(on_message), self.message);
        }
    }
}

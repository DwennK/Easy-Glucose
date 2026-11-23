use eframe::egui;
use std::{
    mem::size_of,
    sync::mpsc::{self, Receiver, Sender},
    time::Duration,
};
use windows_sys::Win32::{
    Foundation::{HWND, POINT, RECT},
    Graphics::Gdi::{
        ClientToScreen, GetMonitorInfoW, MONITOR_DEFAULTTOPRIMARY, MONITORINFO, MonitorFromPoint,
    },
    UI::{
        HiDpi::GetDpiForSystem,
        WindowsAndMessaging::{
            GetClassNameW, GetClientRect, GetDesktopWindow, GetForegroundWindow, GetShellWindow,
        },
    },
};

#[derive(Clone, Copy, PartialEq)]
struct WorkArea {
    edges: [i32; 4],
    dpi: u32,
}

// Use the primary monitor: Windows places its notification area and clock there.
// Compare the client area so an ordinary maximized window keeps the widget visible.
fn foreground_covers(screen: &RECT) -> bool {
    unsafe {
        let hwnd: HWND = GetForegroundWindow();
        if hwnd.is_null() || hwnd == GetDesktopWindow() || hwnd == GetShellWindow() {
            return false;
        }
        let mut name = [0; 32];
        let len = GetClassNameW(hwnd, name.as_mut_ptr(), name.len() as i32);
        if name[..len as usize]
            .iter()
            .copied()
            .eq("WorkerW".encode_utf16())
        {
            return false;
        }
        let mut client = RECT::default();
        let mut origin = POINT::default();
        GetClientRect(hwnd, &mut client) != 0
            && ClientToScreen(hwnd, &mut origin) != 0
            && origin.x <= screen.left
            && origin.y <= screen.top
            && origin.x + client.right >= screen.right
            && origin.y + client.bottom >= screen.bottom
    }
}

fn work_area() -> Option<WorkArea> {
    unsafe {
        let monitor = MonitorFromPoint(POINT::default(), MONITOR_DEFAULTTOPRIMARY);
        let mut info = MONITORINFO {
            cbSize: size_of::<MONITORINFO>() as u32,
            ..Default::default()
        };
        if GetMonitorInfoW(monitor, &mut info) == 0 || foreground_covers(&info.rcMonitor) {
            return None;
        }
        let r = info.rcWork;
        Some(WorkArea {
            edges: [r.left, r.top, r.right, r.bottom],
            dpi: GetDpiForSystem().max(96),
        })
    }
}

pub struct ClockMonitor {
    current: Option<WorkArea>,
    alpha_ready: bool,
    updates: Receiver<Option<WorkArea>>,
    // Dropping the sender wakes the worker immediately when the widget is disabled.
    _stop: Sender<()>,
}

impl ClockMonitor {
    pub fn start(ctx: egui::Context) -> Self {
        let (tx, updates) = mpsc::channel();
        let (stop, stopped) = mpsc::channel();
        let current = work_area();
        std::thread::spawn(move || {
            let mut last = current;
            while stopped.recv_timeout(Duration::from_millis(500))
                == Err(mpsc::RecvTimeoutError::Timeout)
            {
                let area = work_area();
                if area != last {
                    if tx.send(area).is_err() {
                        break;
                    }
                    last = area;
                    ctx.request_repaint_of(egui::ViewportId::ROOT);
                }
            }
        });
        Self {
            current,
            alpha_ready: false,
            updates,
            _stop: stop,
        }
    }

    pub fn enable_alpha(&mut self, title: &str) {
        use std::ptr::null_mut;
        use windows_sys::Win32::{
            Graphics::{
                Dwm::*,
                Gdi::{CreateRectRgn, DeleteObject},
            },
            UI::WindowsAndMessaging::{FindWindowExW, GetWindowThreadProcessId},
        };
        if self.alpha_ready {
            return;
        }
        // Some Windows GL drivers expose an alpha buffer but glutin disables the
        // DWM composition flag on secondary viewports. Apply it to our widget only.
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
            self.alpha_ready = DwmEnableBlurBehindWindow(hwnd, &blur) >= 0;
            DeleteObject(region);
        }
    }

    pub fn work_area(&mut self, zoom: f32) -> Option<egui::Rect> {
        for update in self.updates.try_iter() {
            self.current = update;
            if update.is_none() {
                self.alpha_ready = false;
            }
        }
        self.current.map(|area| {
            let scale = area.dpi as f32 / 96.0 * zoom;
            let [l, t, r, b] = area.edges.map(|v| v as f32 / scale);
            egui::Rect::from_min_max(egui::pos2(l, t), egui::pos2(r, b))
        })
    }
}

//! Selection logic for region and window selection

use capture_wgc::Rect;
use windows::Win32::Foundation::{HWND, RECT, BOOL, LPARAM};
use windows::Win32::Graphics::Dwm::{DwmGetWindowAttribute, DWMWA_CLOAKED};
use windows::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GetAncestor, GetClassNameW, GetWindow,
    GetWindowLongW, GetWindowRect, IsWindowVisible, GA_ROOT,
    GWL_EXSTYLE, GWL_STYLE, GW_OWNER,
    WS_DISABLED, WS_EX_TOOLWINDOW,
};
use std::ffi::OsString;
use std::os::windows::ffi::OsStringExt;

/// Selection mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectionMode {
    Region,
    Window,
}

/// Selection result
#[derive(Debug, Clone)]
pub struct SelectionResult {
    pub mode: SelectionMode,
    pub rect: Rect,
    pub hwnd: Option<isize>,
}

/// Window information for selection
#[derive(Debug, Clone)]
pub struct WindowInfo {
    pub hwnd: isize,
    pub rect: Rect,
    pub class_name: String,
    pub z_order: usize,
}

impl WindowInfo {
    /// Check if point is inside window
    pub fn contains(&self, x: i32, y: i32) -> bool {
        self.rect.contains(x, y)
    }
}

/// Enumerate visible windows in Z-order
pub fn enumerate_windows() -> Vec<WindowInfo> {
    let mut windows = Vec::new();

    unsafe {
        let _ = EnumWindows(
            Some(enum_window_callback),
            LPARAM(&mut windows as *mut Vec<WindowInfo> as isize),
        );
    }

    windows
}

unsafe extern "system" fn enum_window_callback(hwnd: HWND, lparam: LPARAM) -> BOOL {
    let windows = &mut *(lparam.0 as *mut Vec<WindowInfo>);

    if should_include_window(hwnd) {
        if let Some(info) = get_window_info(hwnd, windows.len()) {
            windows.push(info);
        }
    }

    BOOL(1) // Continue enumeration
}

unsafe fn should_include_window(hwnd: HWND) -> bool {
    // Must be visible
    if !IsWindowVisible(hwnd).as_bool() {
        return false;
    }

    // Must not be disabled
    let style = GetWindowLongW(hwnd, GWL_STYLE) as u32;
    if style & WS_DISABLED.0 != 0 {
        return false;
    }

    // Must not be a tool window
    let ex_style = GetWindowLongW(hwnd, GWL_EXSTYLE) as u32;
    if ex_style & WS_EX_TOOLWINDOW.0 != 0 {
        return false;
    }

    // Must not be cloaked (virtual desktop)
    let mut cloaked: u32 = 0;
    if DwmGetWindowAttribute(
        hwnd,
        DWMWA_CLOAKED,
        &mut cloaked as *mut _ as *mut _,
        std::mem::size_of::<u32>() as u32,
    ).is_ok() && cloaked != 0 {
        return false;
    }

    // Must not be owned (popup)
    if let Ok(owner) = GetWindow(hwnd, GW_OWNER) {
        if !owner.is_invalid() {
            return false;
        }
    }

    // Must be root window
    let root = GetAncestor(hwnd, GA_ROOT);
    if root != hwnd {
        return false;
    }

    // Minimum size check
    let mut rect = RECT::default();
    if GetWindowRect(hwnd, &mut rect).is_err() {
        return false;
    }

    let width = rect.right - rect.left;
    let height = rect.bottom - rect.top;

    width > 50 && height > 50
}

unsafe fn get_window_info(hwnd: HWND, z_order: usize) -> Option<WindowInfo> {
    let mut rect = RECT::default();
    if GetWindowRect(hwnd, &mut rect).is_err() {
        return None;
    }

    // Get class name
    let mut class_name_buf = [0u16; 256];
    let len = GetClassNameW(hwnd, &mut class_name_buf);
    let class_name = if len > 0 {
        OsString::from_wide(&class_name_buf[..len as usize])
            .to_string_lossy()
            .into_owned()
    } else {
        String::new()
    };

    Some(WindowInfo {
        hwnd: hwnd.0 as isize,
        rect: Rect::new(
            rect.left,
            rect.top,
            (rect.right - rect.left) as u32,
            (rect.bottom - rect.top) as u32,
        ),
        class_name,
        z_order,
    })
}

/// Find window at screen coordinates (using Z-order enumeration)
pub fn find_window_at(windows: &[WindowInfo], screen_x: i32, screen_y: i32) -> Option<&WindowInfo> {
    // Windows are already in Z-order, find first hit
    windows.iter().find(|w| w.contains(screen_x, screen_y))
}

/// Calculate selection rectangle from drag points
pub fn calc_selection_rect(
    start_x: i32,
    start_y: i32,
    end_x: i32,
    end_y: i32,
) -> Rect {
    let x = start_x.min(end_x);
    let y = start_y.min(end_y);
    let width = (start_x - end_x).unsigned_abs();
    let height = (start_y - end_y).unsigned_abs();

    Rect::new(x, y, width, height)
}

/// Minimum selection size
pub const MIN_SELECTION_SIZE: u32 = 16;

/// Check if selection is valid
pub fn is_valid_selection(rect: &Rect) -> bool {
    rect.width >= MIN_SELECTION_SIZE && rect.height >= MIN_SELECTION_SIZE
}

//! Main panel UI

use crate::state::StateMachine;
use crate::tray::{SystemTray, WM_TRAYICON, ID_TRAY_EXIT, ID_TRAY_RECORD, ID_TRAY_SHOW, ID_TRAY_STOP};
use overlay::{destroy_recording_outline, show_recording_outline};
use once_cell::sync::OnceCell;
use parking_lot::Mutex;
use std::cell::RefCell;
use std::sync::Arc;
use windows::core::{w, PCWSTR};
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM, HINSTANCE};
use windows::Win32::Graphics::Gdi::{
    BeginPaint, CreateSolidBrush, DeleteObject, EndPaint, FillRect, GetStockObject,
    InvalidateRect, SelectObject, UpdateWindow, CreateFontW, SetBkMode, SetTextColor,
    TextOutW, PAINTSTRUCT, TRANSPARENT, WHITE_BRUSH, FW_NORMAL, FW_BOLD,
    DEFAULT_CHARSET, OUT_DEFAULT_PRECIS, CLIP_DEFAULT_PRECIS, DEFAULT_QUALITY,
    DEFAULT_PITCH, FF_SWISS,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Input::KeyboardAndMouse::EnableWindow;
use windows::Win32::UI::WindowsAndMessaging::*;

/// Window dimensions
const WINDOW_WIDTH: i32 = 800;
const WINDOW_HEIGHT: i32 = 160;

/// Button IDs
const ID_BTN_RECORD: u16 = 101;
const ID_BTN_STOP: u16 = 102;
const ID_BTN_EXPORT: u16 = 103;

const BTN_WIDTH: i32 = 120;
const BTN_HEIGHT: i32 = 40;
const BTN_Y: i32 = 30;
const BTN_SPACING: i32 = 20;
const BTN_START_X: i32 = 30;

/// Custom messages
pub const WM_APP_UPDATE_STATE: u32 = WM_USER + 100;

fn make_int_resource(id: u16) -> PCWSTR {
    PCWSTR(id as *const u16)
}

static UI_STATE: OnceCell<Arc<Mutex<UiState>>> = OnceCell::new();

thread_local! {
    static TRAY: RefCell<Option<SystemTray>> = RefCell::new(None);
}

// Store handles as isize for thread safety
pub struct UiState {
    pub state_machine: StateMachine,
    pub btn_record: isize,
    pub btn_stop: isize,
    pub btn_export: isize,
    pub status_text: String,
    pub frame_count: usize,
    pub recording_outline_hwnd: isize,
    pub on_record: Option<Arc<dyn Fn() + Send + Sync>>,
    pub on_stop: Option<Arc<dyn Fn() + Send + Sync>>,
    pub on_export: Option<Arc<dyn Fn() + Send + Sync>>,
}

impl UiState {
    fn new() -> Self {
        Self {
            state_machine: StateMachine::new(),
            btn_record: 0,
            btn_stop: 0,
            btn_export: 0,
            status_text: "就绪".to_string(),
            frame_count: 0,
            recording_outline_hwnd: 0,
            on_record: None,
            on_stop: None,
            on_export: None,
        }
    }
}

fn hwnd_to_isize(hwnd: HWND) -> isize {
    hwnd.0 as isize
}

fn isize_to_hwnd(val: isize) -> HWND {
    HWND(val as *mut std::ffi::c_void)
}

/// Main window
pub struct MainWindow {
    hwnd: HWND,
}

impl MainWindow {
    const CLASS_NAME: PCWSTR = w!("TinyCaptureMain");

    /// Create the main window
    pub fn create() -> windows::core::Result<(Self, Arc<Mutex<UiState>>)> {
        let state = Arc::new(Mutex::new(UiState::new()));
        let _ = UI_STATE.set(state.clone());

        unsafe {
            let hmodule = GetModuleHandleW(None)?;
            let hinstance = HINSTANCE(hmodule.0);

            // Register window class
            let bg_color = 0x00F5F5F5; // Light gray background
            let bg_brush = CreateSolidBrush(windows::Win32::Foundation::COLORREF(bg_color));

            let wc = WNDCLASSEXW {
                cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
                style: CS_HREDRAW | CS_VREDRAW,
                lpfnWndProc: Some(Self::wnd_proc),
                hInstance: hinstance,
                hIcon: LoadIconW(hinstance, make_int_resource(1)).unwrap_or_default(),
                hCursor: LoadCursorW(None, IDC_ARROW)?,
                hbrBackground: bg_brush,
                lpszClassName: Self::CLASS_NAME,
                ..Default::default()
            };

            RegisterClassExW(&wc);

            // Calculate window position (center of primary monitor)
            let screen_width = GetSystemMetrics(SM_CXSCREEN);
            let screen_height = GetSystemMetrics(SM_CYSCREEN);
            let x = (screen_width - WINDOW_WIDTH) / 2;
            let y = (screen_height - WINDOW_HEIGHT) / 2;

            // Create window
            let hwnd = CreateWindowExW(
                WS_EX_TOPMOST,
                Self::CLASS_NAME,
                w!("TinyCapture"),
                WS_OVERLAPPED | WS_CAPTION | WS_SYSMENU | WS_MINIMIZEBOX,
                x,
                y,
                WINDOW_WIDTH,
                WINDOW_HEIGHT,
                HWND::default(),
                HMENU::default(),
                hinstance,
                None,
            )?;

            // Create buttons
            Self::create_buttons(hwnd, hinstance)?;

            // Create tray
            TRAY.with(|tray| {
                let mut tray = tray.borrow_mut();
                let mut new_tray = SystemTray::new(hwnd);
                let _ = new_tray.show();
                *tray = Some(new_tray);
            });

            Ok((Self { hwnd }, state))
        }
    }

    unsafe fn create_buttons(hwnd: HWND, hinstance: HINSTANCE) -> windows::core::Result<()> {
        // Record button
        let btn_record = CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            w!("BUTTON"),
            w!("录制"),
            WS_CHILD | WS_VISIBLE | WINDOW_STYLE(BS_PUSHBUTTON as u32),
            BTN_START_X,
            BTN_Y,
            BTN_WIDTH,
            BTN_HEIGHT,
            hwnd,
            HMENU(ID_BTN_RECORD as _),
            hinstance,
            None,
        )?;

        // Stop button
        let btn_stop = CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            w!("BUTTON"),
            w!("停止"),
            WS_CHILD | WS_VISIBLE | WS_DISABLED | WINDOW_STYLE(BS_PUSHBUTTON as u32),
            BTN_START_X + BTN_WIDTH + BTN_SPACING,
            BTN_Y,
            BTN_WIDTH,
            BTN_HEIGHT,
            hwnd,
            HMENU(ID_BTN_STOP as _),
            hinstance,
            None,
        )?;

        // Export button
        let btn_export = CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            w!("BUTTON"),
            w!("导出 GIF"),
            WS_CHILD | WS_VISIBLE | WS_DISABLED | WINDOW_STYLE(BS_PUSHBUTTON as u32),
            BTN_START_X + (BTN_WIDTH + BTN_SPACING) * 2,
            BTN_Y,
            BTN_WIDTH,
            BTN_HEIGHT,
            hwnd,
            HMENU(ID_BTN_EXPORT as _),
            hinstance,
            None,
        )?;

        // Store button handles as isize
        if let Some(state) = UI_STATE.get() {
            let mut state = state.lock();
            state.btn_record = hwnd_to_isize(btn_record);
            state.btn_stop = hwnd_to_isize(btn_stop);
            state.btn_export = hwnd_to_isize(btn_export);
        }

        Ok(())
    }

    /// Show the window
    pub fn show(&self) {
        unsafe {
            ShowWindow(self.hwnd, SW_SHOW);
            let _ = UpdateWindow(self.hwnd);
        }
    }

    /// Hide the window
    #[allow(dead_code)]
    pub fn hide(&self) {
        unsafe {
            ShowWindow(self.hwnd, SW_HIDE);
        }
    }

    /// Get window handle
    pub fn hwnd(&self) -> HWND {
        self.hwnd
    }

    /// Run message loop
    pub fn run_message_loop() -> i32 {
        unsafe {
            let mut msg = MSG::default();
            while GetMessageW(&mut msg, None, 0, 0).into() {
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
            msg.wParam.0 as i32
        }
    }

    /// Update UI state
    pub fn update_state(hwnd: HWND) {
        if let Some(state) = UI_STATE.get() {
            let mut state = state.lock();
            let app_state = state.state_machine.state();

            unsafe {
                // Update button states
                let _ = EnableWindow(isize_to_hwnd(state.btn_record), app_state.can_record());
                let _ = EnableWindow(isize_to_hwnd(state.btn_stop), app_state.can_stop());
                let _ = EnableWindow(isize_to_hwnd(state.btn_export), app_state.can_export());

                // Recording outline
                if matches!(app_state, crate::state::AppState::Recording) {
                    if state.recording_outline_hwnd == 0 {
                        if let Some(session) = state.state_machine.session() {
                            if let Ok(hwnd_raw) = show_recording_outline(session.region) {
                                state.recording_outline_hwnd = hwnd_raw;
                            }
                        }
                    }
                } else if state.recording_outline_hwnd != 0 {
                    destroy_recording_outline(state.recording_outline_hwnd);
                    state.recording_outline_hwnd = 0;
                }

                // Redraw
                let _ = InvalidateRect(hwnd, None, true);
            }
        }
    }

    unsafe extern "system" fn wnd_proc(
        hwnd: HWND,
        msg: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT {
        match msg {
            WM_CREATE => {
                LRESULT(0)
            }

            WM_PAINT => {
                Self::on_paint(hwnd);
                LRESULT(0)
            }

            WM_COMMAND => {
                let id = (wparam.0 & 0xFFFF) as u16;

                match id {
                    ID_BTN_RECORD => Self::on_record_click(),
                    ID_BTN_STOP => Self::on_stop_click(),
                    ID_BTN_EXPORT => Self::on_export_click(),
                    _ => {}
                }

                // Handle tray menu commands
                match wparam.0 as u32 {
                    ID_TRAY_SHOW => {
                        ShowWindow(hwnd, SW_SHOW);
                        let _ = SetForegroundWindow(hwnd);
                    }
                    ID_TRAY_RECORD => Self::on_record_click(),
                    ID_TRAY_STOP => Self::on_stop_click(),
                    ID_TRAY_EXIT => {
                        let _ = DestroyWindow(hwnd);
                    }
                    _ => {}
                }

                LRESULT(0)
            }

            WM_TRAYICON => {
                let event = (lparam.0 & 0xFFFF) as u32;
                if event == WM_RBUTTONUP {
                    if let Some(state) = UI_STATE.get() {
                        let state = state.lock();
                        let app_state = state.state_machine.state();
                        TRAY.with(|tray| {
                            if let Some(ref tray) = *tray.borrow() {
                                let _ = tray.show_context_menu(
                                    app_state.can_record(),
                                    app_state.can_stop(),
                                );
                            }
                        });
                    }
                } else if event == WM_LBUTTONDBLCLK {
                    ShowWindow(hwnd, SW_SHOW);
                    let _ = SetForegroundWindow(hwnd);
                }
                LRESULT(0)
            }

            WM_APP_UPDATE_STATE => {
                Self::update_state(hwnd);
                LRESULT(0)
            }

            WM_CLOSE => {
                // Minimize to tray instead of closing
                ShowWindow(hwnd, SW_HIDE);
                LRESULT(0)
            }

            WM_DESTROY => {
                TRAY.with(|tray| {
                    *tray.borrow_mut() = None;
                });
                PostQuitMessage(0);
                LRESULT(0)
            }

            _ => DefWindowProcW(hwnd, msg, wparam, lparam),
        }
    }

    unsafe fn on_paint(hwnd: HWND) {
        let mut ps = PAINTSTRUCT::default();
        let hdc = BeginPaint(hwnd, &mut ps);

        // Draw title
        let title_font = CreateFontW(
            28,
            0,
            0,
            0,
            FW_BOLD.0 as i32,
            0,
            0,
            0,
            DEFAULT_CHARSET.0 as u32,
            OUT_DEFAULT_PRECIS.0 as u32,
            CLIP_DEFAULT_PRECIS.0 as u32,
            DEFAULT_QUALITY.0 as u32,
            (DEFAULT_PITCH.0 | FF_SWISS.0) as u32,
            w!("Microsoft YaHei UI"),
        );
        let old_font = SelectObject(hdc, title_font);

        let title_text: Vec<u16> = "TinyCapture"
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();

        SetBkMode(hdc, TRANSPARENT);
        SetTextColor(hdc, windows::Win32::Foundation::COLORREF(0x00333333));
        let _ = TextOutW(hdc, BTN_START_X, BTN_Y - 22, &title_text[..title_text.len() - 1]);

        SelectObject(hdc, old_font);
        DeleteObject(title_font);

        // Draw status text with better styling
        if let Some(state) = UI_STATE.get() {
            let state = state.lock();

            let status = state.status_text.clone();
            let text_wide: Vec<u16> = status.encode_utf16().chain(std::iter::once(0)).collect();

            let status_font = CreateFontW(
                18,
                0,
                0,
                0,
                FW_NORMAL.0 as i32,
                0,
                0,
                0,
                DEFAULT_CHARSET.0 as u32,
                OUT_DEFAULT_PRECIS.0 as u32,
                CLIP_DEFAULT_PRECIS.0 as u32,
                DEFAULT_QUALITY.0 as u32,
                (DEFAULT_PITCH.0 | FF_SWISS.0) as u32,
                w!("Microsoft YaHei UI"),
            );
            let old_font = SelectObject(hdc, status_font);

            SetBkMode(hdc, TRANSPARENT);

            // Color based on state
            let color = match state.state_machine.state() {
                crate::state::AppState::Recording => 0x000088FF, // Orange-red for recording
                crate::state::AppState::Exporting => 0x00FF8800, // Blue for exporting
                _ => 0x00666666, // Gray for idle/other
            };
            SetTextColor(hdc, windows::Win32::Foundation::COLORREF(color));

            let status_y = BTN_Y + BTN_HEIGHT + 15;
            let _ = TextOutW(hdc, BTN_START_X, status_y, &text_wide[..text_wide.len() - 1]);

            // Draw frame count if recording
            if state.frame_count > 0 {
                let frame_text = format!("已录制帧数: {}", state.frame_count);
                let frame_wide: Vec<u16> = frame_text
                    .encode_utf16()
                    .chain(std::iter::once(0))
                    .collect();

                SetTextColor(hdc, windows::Win32::Foundation::COLORREF(0x00888888));
                let _ = TextOutW(
                    hdc,
                    BTN_START_X,
                    status_y + 25,
                    &frame_wide[..frame_wide.len() - 1],
                );
            }

            SelectObject(hdc, old_font);
            DeleteObject(status_font);
        }

        let _ = EndPaint(hwnd, &ps);
    }

    fn on_record_click() {
        if let Some(state) = UI_STATE.get() {
            // Clone the callback Arc to avoid holding lock during execution
            let callback = {
                let state = state.lock();
                state.on_record.clone()
            };

            if let Some(cb) = callback {
                cb();
            }
        }
    }

    fn on_stop_click() {
        if let Some(state) = UI_STATE.get() {
            // Clone the callback Arc to avoid holding lock during execution
            let callback = {
                let state = state.lock();
                state.on_stop.clone()
            };

            if let Some(cb) = callback {
                cb();
            }
        }
    }

    fn on_export_click() {
        if let Some(state) = UI_STATE.get() {
            // Clone the callback Arc to avoid holding lock during execution
            let callback = {
                let state = state.lock();
                state.on_export.clone()
            };

            if let Some(cb) = callback {
                cb();
            }
        }
    }
}

/// Post state update message
pub fn post_update_state(hwnd: HWND) {
    unsafe {
        let _ = PostMessageW(hwnd, WM_APP_UPDATE_STATE, WPARAM(0), LPARAM(0));
    }
}

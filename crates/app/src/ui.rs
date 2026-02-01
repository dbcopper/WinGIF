//! Main panel UI

use crate::state::StateMachine;
use crate::tray::{SystemTray, WM_TRAYICON, ID_TRAY_EXIT, ID_TRAY_RECORD, ID_TRAY_SHOW, ID_TRAY_STOP};
use once_cell::sync::OnceCell;
use parking_lot::Mutex;
use std::cell::RefCell;
use std::sync::Arc;
use windows::core::{w, PCWSTR};
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM, HINSTANCE};
use windows::Win32::Graphics::Gdi::{
    BeginPaint, EndPaint, GetStockObject, InvalidateRect, UpdateWindow,
    SetBkMode, SetTextColor, TextOutW, PAINTSTRUCT, TRANSPARENT, WHITE_BRUSH,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Input::KeyboardAndMouse::EnableWindow;
use windows::Win32::UI::WindowsAndMessaging::*;

/// Window dimensions
const WINDOW_WIDTH: i32 = 720;
const WINDOW_HEIGHT: i32 = 120;

/// Button IDs
const ID_BTN_RECORD: u16 = 101;
const ID_BTN_STOP: u16 = 102;
const ID_BTN_EXPORT: u16 = 103;

const BTN_WIDTH: i32 = 100;
const BTN_HEIGHT: i32 = 32;
const BTN_Y: i32 = 16;
const BTN_SPACING: i32 = 20;
const BTN_START_X: i32 = 20;

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
    pub on_record: Option<Box<dyn Fn() + Send + Sync>>,
    pub on_stop: Option<Box<dyn Fn() + Send + Sync>>,
    pub on_export: Option<Box<dyn Fn() + Send + Sync>>,
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
            let wc = WNDCLASSEXW {
                cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
                style: CS_HREDRAW | CS_VREDRAW,
                lpfnWndProc: Some(Self::wnd_proc),
                hInstance: hinstance,
                hIcon: LoadIconW(hinstance, make_int_resource(1)).unwrap_or_default(),
                hCursor: LoadCursorW(None, IDC_ARROW)?,
                hbrBackground: windows::Win32::Graphics::Gdi::HBRUSH(
                    GetStockObject(WHITE_BRUSH).0,
                ),
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
            let state = state.lock();
            let app_state = state.state_machine.state();

            unsafe {
                // Update button states
                let _ = EnableWindow(isize_to_hwnd(state.btn_record), app_state.can_record());
                let _ = EnableWindow(isize_to_hwnd(state.btn_stop), app_state.can_stop());
                let _ = EnableWindow(isize_to_hwnd(state.btn_export), app_state.can_export());

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

        // Draw status text
        if let Some(state) = UI_STATE.get() {
            let state = state.lock();

            let status = state.status_text.clone();
            let text_wide: Vec<u16> = status.encode_utf16().chain(std::iter::once(0)).collect();

            SetBkMode(hdc, TRANSPARENT);
            SetTextColor(hdc, windows::Win32::Foundation::COLORREF(0x00333333));

            let status_x = BTN_START_X + (BTN_WIDTH + BTN_SPACING) * 3 + 10;
            let _ = TextOutW(hdc, status_x, 24, &text_wide[..text_wide.len() - 1]);
        }

        let _ = EndPaint(hwnd, &ps);
    }

    fn on_record_click() {
        if let Some(state) = UI_STATE.get() {
            let callback = {
                let state = state.lock();
                state.on_record.as_ref().map(|f| unsafe {
                    std::mem::transmute::<&Box<dyn Fn() + Send + Sync>, &Box<dyn Fn() + Send + Sync>>(f)
                }).map(|f| f.as_ref() as *const dyn Fn())
            };

            if let Some(cb_ptr) = callback {
                unsafe {
                    (*cb_ptr)();
                }
            }
        }
    }

    fn on_stop_click() {
        if let Some(state) = UI_STATE.get() {
            let callback = {
                let state = state.lock();
                state.on_stop.as_ref().map(|f| unsafe {
                    std::mem::transmute::<&Box<dyn Fn() + Send + Sync>, &Box<dyn Fn() + Send + Sync>>(f)
                }).map(|f| f.as_ref() as *const dyn Fn())
            };

            if let Some(cb_ptr) = callback {
                unsafe {
                    (*cb_ptr)();
                }
            }
        }
    }

    fn on_export_click() {
        if let Some(state) = UI_STATE.get() {
            let callback = {
                let state = state.lock();
                state.on_export.as_ref().map(|f| unsafe {
                    std::mem::transmute::<&Box<dyn Fn() + Send + Sync>, &Box<dyn Fn() + Send + Sync>>(f)
                }).map(|f| f.as_ref() as *const dyn Fn())
            };

            if let Some(cb_ptr) = callback {
                unsafe {
                    (*cb_ptr)();
                }
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

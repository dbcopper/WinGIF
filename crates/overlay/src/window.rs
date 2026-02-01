//! Overlay window implementation

use crate::{
    render::OverlayRenderer,
    screenshot::{get_virtual_desktop_rect, Screenshot},
    selection::{
        calc_selection_rect, enumerate_windows, find_window_at, is_valid_selection,
        SelectionMode, WindowInfo,
    },
    OverlayError, OverlayResult, SelectionOutcome,
};
use capture_wgc::Rect;
use parking_lot::Mutex;
use std::cell::RefCell;
use std::sync::Arc;
use windows::core::{w, PCWSTR};
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::Graphics::Gdi::{InvalidateRect, UpdateWindow};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW,
    GetMessageW, GetWindowLongPtrW, LoadCursorW, PostQuitMessage, RegisterClassExW,
    SetWindowLongPtrW, ShowWindow, TranslateMessage, CS_HREDRAW, CS_VREDRAW,
    GWLP_USERDATA, IDC_CROSS, MSG, SW_SHOW, WM_DESTROY, WM_KEYDOWN, WM_LBUTTONDOWN,
    WM_CLOSE, WM_LBUTTONUP, WM_MOUSEMOVE, WM_PAINT, WNDCLASSEXW, WS_EX_TOPMOST, WS_POPUP,
};
use windows::Win32::Foundation::HINSTANCE;

thread_local! {
    static OVERLAY_STATE: RefCell<Option<Box<OverlayState>>> = RefCell::new(None);
}

struct OverlayState {
    renderer: Option<OverlayRenderer>,
    windows: Vec<WindowInfo>,
    selection: Option<Rect>,
    drag_start: Option<(i32, i32)>,
    is_dragging: bool,
    mode: SelectionMode,
    result: Option<SelectionOutcome>,
    selected_window: Option<WindowInfo>,
}

impl OverlayState {
    fn new() -> Self {
        Self {
            renderer: None,
            windows: Vec::new(),
            selection: None,
            drag_start: None,
            is_dragging: false,
            mode: SelectionMode::Region,
            result: None,
            selected_window: None,
        }
    }
}

/// Overlay window for selection
pub struct OverlayWindow;

impl OverlayWindow {
    const CLASS_NAME: PCWSTR = w!("TinyCaptureOverlay");
    const DRAG_THRESHOLD: i32 = 4;

    /// Create and show overlay window
    pub fn show() -> OverlayResult<SelectionOutcome> {
        // Initialize state
        let mut state = Box::new(OverlayState::new());

        unsafe {
            // Register window class
            let hmodule = GetModuleHandleW(None)?;
            let hinstance = HINSTANCE(hmodule.0);

            let wc = WNDCLASSEXW {
                cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
                style: CS_HREDRAW | CS_VREDRAW,
                lpfnWndProc: Some(Self::wnd_proc),
                hInstance: hinstance,
                hCursor: LoadCursorW(None, IDC_CROSS)?,
                lpszClassName: Self::CLASS_NAME,
                ..Default::default()
            };

            RegisterClassExW(&wc);

            // Get virtual desktop bounds
            let vd = get_virtual_desktop_rect();

            // Take screenshot
            let screenshot = Screenshot::capture_virtual_desktop()?;

            // Enumerate windows for selection
            let windows = enumerate_windows();

            // Initialize renderer in state
            state.renderer = Some(OverlayRenderer::new(screenshot));
            state.windows = windows;

            // Store state in thread-local
            OVERLAY_STATE.with(|s| {
                *s.borrow_mut() = Some(state);
            });

            // Create window covering virtual desktop
            let hwnd = CreateWindowExW(
                WS_EX_TOPMOST,
                Self::CLASS_NAME,
                w!("TinyCapture Selection"),
                WS_POPUP,
                vd.left,
                vd.top,
                vd.right - vd.left,
                vd.bottom - vd.top,
                None,
                None,
                hinstance,
                None,
            )?;

            ShowWindow(hwnd, SW_SHOW);
            let _ = UpdateWindow(hwnd);

            // Message loop
            let mut msg = MSG::default();
            loop {
                let ret = GetMessageW(&mut msg, None, 0, 0);
                if !ret.as_bool() {
                    break;
                }
                TranslateMessage(&msg);
                DispatchMessageW(&msg);

                // Check for result
                let has_result = OVERLAY_STATE.with(|s| {
                    s.borrow().as_ref().map(|state| state.result.is_some()).unwrap_or(false)
                });
                if has_result {
                    break;
                }
            }

            // Get result
            let result = OVERLAY_STATE.with(|s| {
                s.borrow().as_ref().and_then(|state| state.result.clone())
            });

            // Cleanup
            let _ = DestroyWindow(hwnd);
            OVERLAY_STATE.with(|s| {
                *s.borrow_mut() = None;
            });

            result.ok_or(OverlayError::Cancelled)
        }
    }

    unsafe extern "system" fn wnd_proc(
        hwnd: HWND,
        msg: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT {
        match msg {
            WM_PAINT => {
                OVERLAY_STATE.with(|s| {
                    if let Some(ref state) = *s.borrow() {
                        if let Some(ref renderer) = state.renderer {
                            renderer.render(hwnd);
                        }
                    }
                });
                LRESULT(0)
            }

            WM_LBUTTONDOWN => {
                Self::handle_mouse_down(hwnd, lparam);
                LRESULT(0)
            }

            WM_MOUSEMOVE => {
                Self::handle_mouse_move(hwnd, lparam);
                LRESULT(0)
            }

            WM_LBUTTONUP => {
                Self::handle_mouse_up(hwnd, lparam);
                LRESULT(0)
            }

            WM_KEYDOWN => {
                Self::handle_key_down(hwnd, wparam);
                LRESULT(0)
            }

            WM_CLOSE => {
                OVERLAY_STATE.with(|s| {
                    if let Some(ref mut state) = *s.borrow_mut() {
                        if state.result.is_none() {
                            state.result = Some(SelectionOutcome::Cancelled);
                        }
                    }
                });
                let _ = DestroyWindow(hwnd);
                LRESULT(0)
            }

            WM_DESTROY => {
                LRESULT(0)
            }

            _ => DefWindowProcW(hwnd, msg, wparam, lparam),
        }
    }

    unsafe fn handle_mouse_down(_hwnd: HWND, lparam: LPARAM) {
        OVERLAY_STATE.with(|s| {
            if let Some(ref mut state) = *s.borrow_mut() {
                let x = (lparam.0 & 0xFFFF) as i16 as i32;
                let y = ((lparam.0 >> 16) & 0xFFFF) as i16 as i32;

                // Convert to screen coordinates
                let (screen_x, screen_y) = if let Some(ref renderer) = state.renderer {
                    renderer.screenshot().local_to_screen(x, y)
                } else {
                    (x, y)
                };

                state.drag_start = Some((screen_x, screen_y));
                state.is_dragging = false;
                state.mode = SelectionMode::Region;

                if let Some(ref mut renderer) = state.renderer {
                    renderer.set_dragging(false);
                }
            }
        });
    }

    unsafe fn handle_mouse_move(hwnd: HWND, lparam: LPARAM) {
        OVERLAY_STATE.with(|s| {
            if let Some(ref mut state) = *s.borrow_mut() {
                let x = (lparam.0 & 0xFFFF) as i16 as i32;
                let y = ((lparam.0 >> 16) & 0xFFFF) as i16 as i32;

                let (screen_x, screen_y) = if let Some(ref renderer) = state.renderer {
                    renderer.screenshot().local_to_screen(x, y)
                } else {
                    (x, y)
                };

                if let Some((start_x, start_y)) = state.drag_start {
                    if !state.is_dragging {
                        let dx = (screen_x - start_x).abs();
                        let dy = (screen_y - start_y).abs();
                        if dx >= Self::DRAG_THRESHOLD || dy >= Self::DRAG_THRESHOLD {
                            state.is_dragging = true;
                            if let Some(ref mut renderer) = state.renderer {
                                renderer.set_dragging(true);
                            }
                        }
                    }
                }

                if state.is_dragging {
                    // Update selection rectangle
                    if let Some((start_x, start_y)) = state.drag_start {
                        let rect = calc_selection_rect(start_x, start_y, screen_x, screen_y);
                        state.selection = Some(rect);

                        if let Some(ref mut renderer) = state.renderer {
                            renderer.set_selection(Some(rect));
                            renderer.set_hover(None);
                        }
                    }
                } else {
                    // Window hover detection
                    let windows = state.windows.clone();
                    if let Some(win) = find_window_at(&windows, screen_x, screen_y) {
                        state.selected_window = Some(win.clone());

                        if let Some(ref mut renderer) = state.renderer {
                            renderer.set_hover(Some(win.rect));
                        }
                    } else {
                        state.selected_window = None;
                        if let Some(ref mut renderer) = state.renderer {
                            renderer.set_hover(None);
                        }
                    }
                }
            }
        });

        // Redraw
        let _ = InvalidateRect(hwnd, None, false);
    }

    unsafe fn handle_mouse_up(hwnd: HWND, lparam: LPARAM) {
        OVERLAY_STATE.with(|s| {
            if let Some(ref mut state) = *s.borrow_mut() {
                if !state.is_dragging {
                    // Click = window selection
                    let x = (lparam.0 & 0xFFFF) as i16 as i32;
                    let y = ((lparam.0 >> 16) & 0xFFFF) as i16 as i32;

                    let (screen_x, screen_y) = if let Some(ref renderer) = state.renderer {
                        renderer.screenshot().local_to_screen(x, y)
                    } else {
                        (x, y)
                    };

                    let windows = state.windows.clone();
                    if let Some(win) = find_window_at(&windows, screen_x, screen_y) {
                        state.selection = Some(win.rect);
                        state.selected_window = Some(win.clone());
                        state.mode = SelectionMode::Window;

                        if let Some(ref mut renderer) = state.renderer {
                            renderer.set_selection(Some(win.rect));
                        }
                    }
                } else {
                    // End of drag
                    state.is_dragging = false;
                    state.mode = SelectionMode::Region;

                    if let Some(ref mut renderer) = state.renderer {
                        renderer.set_dragging(false);
                    }

                    // If selection too small, clear it
                    if let Some(rect) = state.selection {
                        if !is_valid_selection(&rect) {
                            state.selection = None;
                            if let Some(ref mut renderer) = state.renderer {
                                renderer.set_selection(None);
                            }
                        }
                    }
                }

                state.drag_start = None;
            }
        });

        let _ = InvalidateRect(hwnd, None, false);
    }

    unsafe fn handle_key_down(hwnd: HWND, wparam: WPARAM) {
        const VK_ESCAPE: usize = 0x1B;
        const VK_RETURN: usize = 0x0D;

        OVERLAY_STATE.with(|s| {
            if let Some(ref mut state) = *s.borrow_mut() {
                match wparam.0 {
                    VK_ESCAPE => {
                        state.result = Some(SelectionOutcome::Cancelled);
                        let _ = DestroyWindow(hwnd);
                    }
                    VK_RETURN => {
                        if let Some(rect) = state.selection {
                            state.result = Some(match state.mode {
                                SelectionMode::Region => SelectionOutcome::Region(rect),
                                SelectionMode::Window => {
                                    if let Some(ref win) = state.selected_window {
                                        SelectionOutcome::Window {
                                            hwnd: win.hwnd,
                                            rect: win.rect,
                                        }
                                    } else {
                                        SelectionOutcome::Region(rect)
                                    }
                                }
                            });
                            let _ = DestroyWindow(hwnd);
                        }
                    }
                    _ => {}
                }
            }
        });
    }
}

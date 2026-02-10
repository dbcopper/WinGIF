//! Recording outline overlay (topmost, click-through).

use capture_wgc::Rect;
use std::sync::Once;
use windows::core::{w, PCWSTR};
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM, HINSTANCE, RECT};
use windows::Win32::Graphics::Gdi::{
    BeginPaint, CreatePen, DeleteObject, EndPaint, GetStockObject,
    SelectObject, Rectangle, HOLLOW_BRUSH, PAINTSTRUCT, PS_SOLID, UpdateWindow,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, RegisterClassExW, ShowWindow,
    WNDCLASSEXW, WS_EX_TOPMOST, WS_EX_TOOLWINDOW, WS_EX_TRANSPARENT,
    WS_EX_NOACTIVATE, WS_POPUP, SW_SHOWNOACTIVATE, WM_NCHITTEST, WM_PAINT,
    HTTRANSPARENT,
};

use crate::{OverlayError, OverlayResult};

const OUTLINE_CLASS: PCWSTR = w!("WinGIFRecordingOutline");
const OUTLINE_THICKNESS: i32 = 2;

static REGISTER: Once = Once::new();

fn register_class() -> OverlayResult<()> {
    let mut result: Result<(), OverlayError> = Ok(());
    REGISTER.call_once(|| unsafe {
        let hmodule = match GetModuleHandleW(None) {
            Ok(h) => h,
            Err(e) => {
                result = Err(e.into());
                return;
            }
        };
        let hinstance = HINSTANCE(hmodule.0);
        let wc = WNDCLASSEXW {
            cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
            lpfnWndProc: Some(outline_wnd_proc),
            hInstance: hinstance,
            lpszClassName: OUTLINE_CLASS,
            ..Default::default()
        };

        let _ = RegisterClassExW(&wc);
    });

    result
}

pub fn show_recording_outline(rect: Rect) -> OverlayResult<isize> {
    register_class()?;

    unsafe {
        let hmodule = GetModuleHandleW(None)?;
        let hinstance = HINSTANCE(hmodule.0);

        let hwnd = CreateWindowExW(
            WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_TRANSPARENT | WS_EX_NOACTIVATE,
            OUTLINE_CLASS,
            w!("WinGIF Recording"),
            WS_POPUP,
            rect.x,
            rect.y,
            rect.width as i32,
            rect.height as i32,
            None,
            None,
            hinstance,
            None,
        )?;

        ShowWindow(hwnd, SW_SHOWNOACTIVATE);
        let _ = UpdateWindow(hwnd);

        Ok(hwnd.0 as isize)
    }
}

pub fn update_recording_outline(hwnd_raw: isize, rect: Rect) -> OverlayResult<()> {
    unsafe {
        use windows::Win32::UI::WindowsAndMessaging::SetWindowPos;
        use windows::Win32::UI::WindowsAndMessaging::{SWP_NOACTIVATE, SWP_NOZORDER, SWP_SHOWWINDOW};

        let hwnd = HWND(hwnd_raw as *mut std::ffi::c_void);
        let _ = SetWindowPos(
            hwnd,
            None,
            rect.x,
            rect.y,
            rect.width as i32,
            rect.height as i32,
            SWP_NOZORDER | SWP_NOACTIVATE | SWP_SHOWWINDOW,
        );
    }

    Ok(())
}

pub fn destroy_recording_outline(hwnd_raw: isize) {
    if hwnd_raw == 0 {
        return;
    }

    unsafe {
        let hwnd = HWND(hwnd_raw as *mut std::ffi::c_void);
        let _ = DestroyWindow(hwnd);
    }
}

unsafe extern "system" fn outline_wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_NCHITTEST => LRESULT(HTTRANSPARENT as isize),
        WM_PAINT => {
            use windows::Win32::UI::WindowsAndMessaging::GetClientRect;

            let mut ps = PAINTSTRUCT::default();
            let hdc = BeginPaint(hwnd, &mut ps);

            let mut rect = RECT::default();
            let _ = GetClientRect(hwnd, &mut rect);

            // Green outline color
            let outline_color = windows::Win32::Foundation::COLORREF(0x0000FF00);
            let pen = CreatePen(PS_SOLID, OUTLINE_THICKNESS, outline_color);
            let old_pen = SelectObject(hdc, pen);
            let old_brush = SelectObject(hdc, GetStockObject(HOLLOW_BRUSH));

            let _ = Rectangle(hdc, rect.left, rect.top, rect.right, rect.bottom);

            let _ = SelectObject(hdc, old_pen);
            let _ = SelectObject(hdc, old_brush);
            let _ = DeleteObject(pen);

            let _ = EndPaint(hwnd, &ps);
            LRESULT(0)
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

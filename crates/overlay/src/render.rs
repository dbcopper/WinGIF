//! GDI+ rendering for overlay

use crate::screenshot::Screenshot;
use capture_wgc::Rect;
use windows::Win32::Foundation::{HWND, RECT};
use windows::Win32::Graphics::Gdi::{
    BeginPaint, CreateSolidBrush, DeleteObject, EndPaint, FillRect, SelectObject,
    SetBkMode, SetTextColor, TextOutW, CreatePen, Rectangle,
    HDC, PAINTSTRUCT, TRANSPARENT, PS_SOLID,
    SetDIBitsToDevice, BITMAPINFO, BITMAPINFOHEADER, BI_RGB, DIB_RGB_COLORS,
};
use std::mem::size_of;

/// Overlay renderer
pub struct OverlayRenderer {
    screenshot: Screenshot,
    selection_rect: Option<Rect>,
    hover_rect: Option<Rect>,
    is_dragging: bool,
}

impl OverlayRenderer {
    /// Create a new renderer with screenshot
    pub fn new(screenshot: Screenshot) -> Self {
        Self {
            screenshot,
            selection_rect: None,
            hover_rect: None,
            is_dragging: false,
        }
    }

    /// Set selection rectangle
    pub fn set_selection(&mut self, rect: Option<Rect>) {
        self.selection_rect = rect;
    }

    /// Set hover rectangle (for window preview)
    pub fn set_hover(&mut self, rect: Option<Rect>) {
        self.hover_rect = rect;
    }

    /// Set dragging state
    pub fn set_dragging(&mut self, dragging: bool) {
        self.is_dragging = dragging;
    }

    /// Render to window
    pub fn render(&self, hwnd: HWND) {
        unsafe {
            let mut ps = PAINTSTRUCT::default();
            let hdc = BeginPaint(hwnd, &mut ps);

            self.draw_screenshot(hdc);
            self.draw_overlay(hdc);

            if let Some(ref rect) = self.hover_rect {
                if !self.is_dragging {
                    self.draw_window_highlight(hdc, rect);
                }
            }

            if let Some(ref rect) = self.selection_rect {
                self.draw_selection(hdc, rect);
            }

            self.draw_info_bar(hdc);

            EndPaint(hwnd, &ps);
        }
    }

    unsafe fn draw_screenshot(&self, hdc: HDC) {
        let width = self.screenshot.width as i32;
        let height = self.screenshot.height as i32;

        let bmi = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: width,
                biHeight: -height, // Top-down
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                biSizeImage: 0,
                biXPelsPerMeter: 0,
                biYPelsPerMeter: 0,
                biClrUsed: 0,
                biClrImportant: 0,
            },
            bmiColors: [Default::default()],
        };

        let result = SetDIBitsToDevice(
            hdc,
            0,
            0,
            width as u32,
            height as u32,
            0,
            0,
            0,
            height as u32,
            self.screenshot.data.as_ptr() as *const _,
            &bmi,
            DIB_RGB_COLORS,
        );

        // If SetDIBitsToDevice fails, the window will be black
        // This ensures we can debug the issue
        if result == 0 {
            // Draw a fallback message
            use windows::Win32::Graphics::Gdi::{TextOutW, SetTextColor};
            let msg = "截图加载失败";
            let msg_wide: Vec<u16> = msg.encode_utf16().chain(std::iter::once(0)).collect();
            SetBkMode(hdc, TRANSPARENT);
            SetTextColor(hdc, windows::Win32::Foundation::COLORREF(0x00FFFFFF));
            let _ = TextOutW(hdc, 50, 50, &msg_wide[..msg_wide.len() - 1]);
        }
    }

    unsafe fn draw_overlay(&self, _hdc: HDC) {
        // Semi-transparent dark overlay (simulated with dithered brush)
        // Note: True alpha blending would require GDI+ or layered window
        // For simplicity, we skip the dark overlay when there's a selection
        if self.selection_rect.is_none() && self.hover_rect.is_none() {
            // Draw subtle overlay effect via dithered pattern
            // In production, consider using UpdateLayeredWindow with alpha
        }
    }

    unsafe fn draw_window_highlight(&self, hdc: HDC, rect: &Rect) {
        // Draw thick, vibrant orange border for window highlight
        let pen = CreatePen(PS_SOLID, 4, windows::Win32::Foundation::COLORREF(0x0000AAFF)); // Bright orange
        let old_pen = SelectObject(hdc, pen);

        // Hollow rectangle
        let brush = windows::Win32::Graphics::Gdi::GetStockObject(
            windows::Win32::Graphics::Gdi::NULL_BRUSH,
        );
        let old_brush = SelectObject(hdc, brush);

        let (local_x, local_y) = self.screenshot.screen_to_local(rect.x, rect.y);
        Rectangle(
            hdc,
            local_x,
            local_y,
            local_x + rect.width as i32,
            local_y + rect.height as i32,
        );

        SelectObject(hdc, old_brush);
        SelectObject(hdc, old_pen);
        DeleteObject(pen);
    }

    unsafe fn draw_selection(&self, hdc: HDC, rect: &Rect) {
        // Draw selection border with solid line for better visibility
        let pen = CreatePen(PS_SOLID, 3, windows::Win32::Foundation::COLORREF(0x0000FF00)); // Bright green
        let old_pen = SelectObject(hdc, pen);

        let brush = windows::Win32::Graphics::Gdi::GetStockObject(
            windows::Win32::Graphics::Gdi::NULL_BRUSH,
        );
        let old_brush = SelectObject(hdc, brush);

        let (local_x, local_y) = self.screenshot.screen_to_local(rect.x, rect.y);
        Rectangle(
            hdc,
            local_x,
            local_y,
            local_x + rect.width as i32,
            local_y + rect.height as i32,
        );

        SelectObject(hdc, old_brush);
        SelectObject(hdc, old_pen);
        DeleteObject(pen);

        // Draw size info with background for better readability
        use windows::Win32::Graphics::Gdi::CreateFontW;
        use windows::Win32::Graphics::Gdi::{FW_BOLD, DEFAULT_CHARSET, OUT_DEFAULT_PRECIS,
            CLIP_DEFAULT_PRECIS, DEFAULT_QUALITY, DEFAULT_PITCH, FF_SWISS};
        use windows::core::w;

        let size_text = format!("{} × {} px", rect.width, rect.height);
        let size_wide: Vec<u16> = size_text
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();

        // Create larger font for size display
        let font = CreateFontW(
            20, 0, 0, 0,
            FW_BOLD.0 as i32,
            0, 0, 0,
            DEFAULT_CHARSET.0 as u32,
            OUT_DEFAULT_PRECIS.0 as u32,
            CLIP_DEFAULT_PRECIS.0 as u32,
            DEFAULT_QUALITY.0 as u32,
            (DEFAULT_PITCH.0 | FF_SWISS.0) as u32,
            w!("Microsoft YaHei UI"),
        );
        let old_font = SelectObject(hdc, font);

        // Draw semi-transparent background for text
        let bg_brush = windows::Win32::Graphics::Gdi::CreateSolidBrush(
            windows::Win32::Foundation::COLORREF(0x00333333)
        );
        let text_bg_rect = windows::Win32::Foundation::RECT {
            left: local_x + 4,
            top: local_y + rect.height as i32 + 4,
            right: local_x + 180,
            bottom: local_y + rect.height as i32 + 32,
        };
        windows::Win32::Graphics::Gdi::FillRect(hdc, &text_bg_rect, bg_brush);
        DeleteObject(bg_brush);

        SetBkMode(hdc, TRANSPARENT);
        SetTextColor(hdc, windows::Win32::Foundation::COLORREF(0x00FFFFFF)); // White

        TextOutW(
            hdc,
            local_x + 8,
            local_y + rect.height as i32 + 8,
            &size_wide[..size_wide.len() - 1],
        );

        SelectObject(hdc, old_font);
        DeleteObject(font);
    }

    unsafe fn draw_info_bar(&self, hdc: HDC) {
        use windows::Win32::Graphics::Gdi::CreateFontW;
        use windows::Win32::Graphics::Gdi::{FW_NORMAL, DEFAULT_CHARSET, OUT_DEFAULT_PRECIS,
            CLIP_DEFAULT_PRECIS, DEFAULT_QUALITY, DEFAULT_PITCH, FF_SWISS};
        use windows::core::w;

        // Draw info bar at bottom with better height
        let bar_height = 40;
        let bar_top = self.screenshot.height as i32 - bar_height;

        // Semi-transparent dark background
        let brush = CreateSolidBrush(windows::Win32::Foundation::COLORREF(0x00222222));
        let bar_rect = RECT {
            left: 0,
            top: bar_top,
            right: self.screenshot.width as i32,
            bottom: self.screenshot.height as i32,
        };
        FillRect(hdc, &bar_rect, brush);
        DeleteObject(brush);

        // Instructions text with better font
        let text = if self.is_dragging {
            "拖动选择区域 | 松开确认"
        } else {
            "拖动框选区域 | 点击选择窗口 | Enter 确认 | Esc 取消"
        };

        let text_wide: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();

        let font = CreateFontW(
            18, 0, 0, 0,
            FW_NORMAL.0 as i32,
            0, 0, 0,
            DEFAULT_CHARSET.0 as u32,
            OUT_DEFAULT_PRECIS.0 as u32,
            CLIP_DEFAULT_PRECIS.0 as u32,
            DEFAULT_QUALITY.0 as u32,
            (DEFAULT_PITCH.0 | FF_SWISS.0) as u32,
            w!("Microsoft YaHei UI"),
        );
        let old_font = SelectObject(hdc, font);

        SetBkMode(hdc, TRANSPARENT);
        SetTextColor(hdc, windows::Win32::Foundation::COLORREF(0x00FFFFFF));

        TextOutW(hdc, 15, bar_top + 10, &text_wide[..text_wide.len() - 1]);

        SelectObject(hdc, old_font);
        DeleteObject(font);
    }

    /// Get screenshot reference
    pub fn screenshot(&self) -> &Screenshot {
        &self.screenshot
    }
}

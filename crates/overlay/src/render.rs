//! GDI+ rendering for overlay

use crate::screenshot::Screenshot;
use capture_wgc::Rect;
use windows::Win32::Foundation::{HWND, RECT};
use windows::Win32::Graphics::Gdi::{
    BeginPaint, CreateSolidBrush, DeleteObject, EndPaint, FillRect, SelectObject,
    SetBkMode, SetTextColor, TextOutW, CreatePen, Rectangle,
    HDC, PAINTSTRUCT, TRANSPARENT, PS_SOLID, PS_DASH,
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

        SetDIBitsToDevice(
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
        let pen = CreatePen(PS_SOLID, 3, windows::Win32::Foundation::COLORREF(0x00FF8800)); // Orange
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
        // Draw selection border
        let pen = CreatePen(PS_DASH, 2, windows::Win32::Foundation::COLORREF(0x0000FF00)); // Green
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

        // Draw size text
        let size_text: Vec<u16> = format!("{}x{}", rect.width, rect.height)
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();

        SetBkMode(hdc, TRANSPARENT);
        SetTextColor(hdc, windows::Win32::Foundation::COLORREF(0x00FFFFFF)); // White

        TextOutW(
            hdc,
            local_x + 4,
            local_y + rect.height as i32 + 4,
            &size_text[..size_text.len() - 1],
        );
    }

    unsafe fn draw_info_bar(&self, hdc: HDC) {
        // Draw info bar at bottom
        let bar_height = 32;
        let bar_top = self.screenshot.height as i32 - bar_height;

        // Dark background
        let brush = CreateSolidBrush(windows::Win32::Foundation::COLORREF(0x00333333));
        let bar_rect = RECT {
            left: 0,
            top: bar_top,
            right: self.screenshot.width as i32,
            bottom: self.screenshot.height as i32,
        };
        FillRect(hdc, &bar_rect, brush);
        DeleteObject(brush);

        // Instructions text
        let text = if self.is_dragging {
            "拖动选择区域 | 松开确认"
        } else {
            "拖动框选区域 | 点击选择窗口 | Enter 确认 | Esc 取消"
        };

        let text_wide: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();

        SetBkMode(hdc, TRANSPARENT);
        SetTextColor(hdc, windows::Win32::Foundation::COLORREF(0x00FFFFFF));

        TextOutW(hdc, 10, bar_top + 8, &text_wide[..text_wide.len() - 1]);
    }

    /// Get screenshot reference
    pub fn screenshot(&self) -> &Screenshot {
        &self.screenshot
    }
}

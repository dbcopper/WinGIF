//! Virtual desktop screenshot using GDI

use crate::OverlayResult;
use windows::Win32::Foundation::RECT;
use windows::Win32::Graphics::Gdi::{
    BitBlt, CreateCompatibleBitmap, CreateCompatibleDC, DeleteDC, DeleteObject,
    GetDC, GetDIBits, ReleaseDC, SelectObject, BITMAPINFO, BITMAPINFOHEADER,
    BI_RGB, DIB_RGB_COLORS, SRCCOPY,
};
use windows::Win32::UI::WindowsAndMessaging::{
    GetSystemMetrics, SM_CXVIRTUALSCREEN, SM_CYVIRTUALSCREEN,
    SM_XVIRTUALSCREEN, SM_YVIRTUALSCREEN,
};

/// Screenshot data
pub struct Screenshot {
    pub data: Vec<u8>,
    pub width: u32,
    pub height: u32,
    pub virtual_left: i32,
    pub virtual_top: i32,
}

impl Screenshot {
    /// Capture the entire virtual desktop
    pub fn capture_virtual_desktop() -> OverlayResult<Self> {
        unsafe {
            // Get virtual desktop bounds
            let virtual_left = GetSystemMetrics(SM_XVIRTUALSCREEN);
            let virtual_top = GetSystemMetrics(SM_YVIRTUALSCREEN);
            let virtual_width = GetSystemMetrics(SM_CXVIRTUALSCREEN);
            let virtual_height = GetSystemMetrics(SM_CYVIRTUALSCREEN);

            // Get screen DC
            let screen_dc = GetDC(None);
            if screen_dc.is_invalid() {
                return Err(crate::OverlayError::Screenshot("Failed to get screen DC".into()));
            }

            // Create compatible DC and bitmap
            let mem_dc = CreateCompatibleDC(screen_dc);
            let bitmap = CreateCompatibleBitmap(screen_dc, virtual_width, virtual_height);
            let old_bitmap = SelectObject(mem_dc, bitmap);

            // Copy screen to bitmap
            BitBlt(
                mem_dc,
                0,
                0,
                virtual_width,
                virtual_height,
                screen_dc,
                virtual_left,
                virtual_top,
                SRCCOPY,
            )?;

            // Prepare bitmap info
            let mut bmi = BITMAPINFO {
                bmiHeader: BITMAPINFOHEADER {
                    biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                    biWidth: virtual_width,
                    biHeight: -virtual_height, // Top-down DIB
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

            // Allocate buffer and get bits
            let buffer_size = (virtual_width * virtual_height * 4) as usize;
            let mut data = vec![0u8; buffer_size];

            GetDIBits(
                mem_dc,
                bitmap,
                0,
                virtual_height as u32,
                Some(data.as_mut_ptr() as *mut _),
                &mut bmi,
                DIB_RGB_COLORS,
            );

            // Cleanup
            SelectObject(mem_dc, old_bitmap);
            DeleteObject(bitmap);
            DeleteDC(mem_dc);
            ReleaseDC(None, screen_dc);

            Ok(Screenshot {
                data,
                width: virtual_width as u32,
                height: virtual_height as u32,
                virtual_left,
                virtual_top,
            })
        }
    }

    /// Convert screen coordinates to screenshot coordinates
    pub fn screen_to_local(&self, x: i32, y: i32) -> (i32, i32) {
        (x - self.virtual_left, y - self.virtual_top)
    }

    /// Convert screenshot coordinates to screen coordinates
    pub fn local_to_screen(&self, x: i32, y: i32) -> (i32, i32) {
        (x + self.virtual_left, y + self.virtual_top)
    }
}

/// Get virtual desktop bounds
pub fn get_virtual_desktop_rect() -> RECT {
    unsafe {
        RECT {
            left: GetSystemMetrics(SM_XVIRTUALSCREEN),
            top: GetSystemMetrics(SM_YVIRTUALSCREEN),
            right: GetSystemMetrics(SM_XVIRTUALSCREEN) + GetSystemMetrics(SM_CXVIRTUALSCREEN),
            bottom: GetSystemMetrics(SM_YVIRTUALSCREEN) + GetSystemMetrics(SM_CYVIRTUALSCREEN),
        }
    }
}

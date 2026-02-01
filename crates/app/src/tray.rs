//! System tray implementation

use windows::core::{w, PCWSTR};
use windows::Win32::Foundation::{HWND, HINSTANCE};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Shell::{
    Shell_NotifyIconW, NIF_ICON, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE,
    NIM_MODIFY, NOTIFYICONDATAW,
};
use windows::Win32::UI::WindowsAndMessaging::{
    AppendMenuW, CreatePopupMenu, DestroyMenu, GetCursorPos, LoadIconW,
    SetForegroundWindow, TrackPopupMenu,
    MF_STRING, TPM_BOTTOMALIGN, TPM_LEFTALIGN, WM_USER,
};

fn make_int_resource(id: u16) -> PCWSTR {
    PCWSTR(id as *const u16)
}

/// Tray icon message
pub const WM_TRAYICON: u32 = WM_USER + 1;

/// Tray menu commands
pub const ID_TRAY_SHOW: u32 = 1001;
pub const ID_TRAY_RECORD: u32 = 1002;
pub const ID_TRAY_STOP: u32 = 1003;
pub const ID_TRAY_EXIT: u32 = 1004;

/// System tray manager
pub struct SystemTray {
    hwnd: HWND,
    nid: NOTIFYICONDATAW,
    visible: bool,
}

impl SystemTray {
    /// Create a new system tray
    pub fn new(hwnd: HWND) -> Self {
        let mut nid = NOTIFYICONDATAW::default();
        nid.cbSize = std::mem::size_of::<NOTIFYICONDATAW>() as u32;
        nid.hWnd = hwnd;
        nid.uID = 1;
        nid.uFlags = NIF_ICON | NIF_MESSAGE | NIF_TIP;
        nid.uCallbackMessage = WM_TRAYICON;

        // Set tooltip
        let tip = "TinyCapture - 屏幕录制";
        let tip_wide: Vec<u16> = tip.encode_utf16().collect();
        let len = tip_wide.len().min(127);
        nid.szTip[..len].copy_from_slice(&tip_wide[..len]);

        Self {
            hwnd,
            nid,
            visible: false,
        }
    }

    /// Show the tray icon
    pub fn show(&mut self) -> windows::core::Result<()> {
        if self.visible {
            return Ok(());
        }

        unsafe {
            // Load icon from resource
            let hmodule = GetModuleHandleW(None)?;
            let hinstance = HINSTANCE(hmodule.0);
            let icon = LoadIconW(hinstance, make_int_resource(1)).unwrap_or_default();
            self.nid.hIcon = icon;

            let _ = Shell_NotifyIconW(NIM_ADD, &self.nid);
            self.visible = true;
        }
        Ok(())
    }

    /// Hide the tray icon
    pub fn hide(&mut self) -> windows::core::Result<()> {
        if !self.visible {
            return Ok(());
        }

        unsafe {
            let _ = Shell_NotifyIconW(NIM_DELETE, &self.nid);
            self.visible = false;
        }
        Ok(())
    }

    /// Update tooltip
    #[allow(dead_code)]
    pub fn set_tooltip(&mut self, text: &str) -> windows::core::Result<()> {
        let tip_wide: Vec<u16> = text.encode_utf16().collect();
        let len = tip_wide.len().min(127);

        self.nid.szTip = [0; 128];
        self.nid.szTip[..len].copy_from_slice(&tip_wide[..len]);

        if self.visible {
            unsafe {
                let _ = Shell_NotifyIconW(NIM_MODIFY, &self.nid);
            }
        }
        Ok(())
    }

    /// Show context menu
    pub fn show_context_menu(&self, can_record: bool, can_stop: bool) -> windows::core::Result<()> {
        unsafe {
            let menu = CreatePopupMenu()?;

            // Add menu items
            let _ = AppendMenuW(menu, MF_STRING, ID_TRAY_SHOW as usize, w!("显示窗口"));

            if can_record {
                let _ = AppendMenuW(menu, MF_STRING, ID_TRAY_RECORD as usize, w!("开始录制"));
            }

            if can_stop {
                let _ = AppendMenuW(menu, MF_STRING, ID_TRAY_STOP as usize, w!("停止录制"));
            }

            let _ = AppendMenuW(menu, MF_STRING, ID_TRAY_EXIT as usize, w!("退出"));

            // Get cursor position
            let mut pt = windows::Win32::Foundation::POINT::default();
            let _ = GetCursorPos(&mut pt);

            // Show menu
            let _ = SetForegroundWindow(self.hwnd);
            TrackPopupMenu(
                menu,
                TPM_LEFTALIGN | TPM_BOTTOMALIGN,
                pt.x,
                pt.y,
                0,
                self.hwnd,
                None,
            );

            let _ = DestroyMenu(menu);
        }
        Ok(())
    }
}

impl Drop for SystemTray {
    fn drop(&mut self) {
        let _ = self.hide();
    }
}

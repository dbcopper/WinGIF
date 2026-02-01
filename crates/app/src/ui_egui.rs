//! Modern UI using egui framework

use crate::state::{AppState, StateMachine};
use overlay::{destroy_recording_outline, show_recording_outline};
use eframe::egui;
use parking_lot::Mutex;
use std::sync::Arc;

/// Callback type for button actions
pub type ActionCallback = Arc<dyn Fn() + Send + Sync>;

/// UI State shared between threads
pub struct EguiUiState {
    pub state_machine: StateMachine,
    pub status_text: String,
    pub frame_count: usize,
    pub main_hwnd: isize,
    pub recording_outline_hwnd: isize,
    pub on_record: Option<ActionCallback>,
    pub on_stop: Option<ActionCallback>,
    pub on_export: Option<ActionCallback>,
}

impl EguiUiState {
    pub fn new() -> Self {
        Self {
            state_machine: StateMachine::new(),
            status_text: "就绪".to_string(),
            frame_count: 0,
            main_hwnd: 0,
            recording_outline_hwnd: 0,
            on_record: None,
            on_stop: None,
            on_export: None,
        }
    }
}

/// Main application using egui
pub struct TinyCaptureApp {
    state: Arc<Mutex<EguiUiState>>,
}

impl TinyCaptureApp {
    pub fn new(cc: &eframe::CreationContext<'_>, state: Arc<Mutex<EguiUiState>>) -> Self {
        // 配置中文字体
        Self::setup_custom_fonts(&cc.egui_ctx);
        Self { state }
    }

    fn setup_custom_fonts(ctx: &egui::Context) {
        use std::fs;

        let mut fonts = egui::FontDefinitions::default();

        // 尝试加载 Windows 系统自带的中文字体
        let font_paths = [
            "C:\\Windows\\Fonts\\msyh.ttc",      // 微软雅黑
            "C:\\Windows\\Fonts\\simhei.ttf",    // 黑体
            "C:\\Windows\\Fonts\\simsun.ttc",    // 宋体
        ];

        let mut font_loaded = false;
        for font_path in &font_paths {
            if let Ok(font_data) = fs::read(font_path) {
                fonts.font_data.insert(
                    "chinese_font".to_owned(),
                    egui::FontData::from_owned(font_data),
                );
                font_loaded = true;
                break;
            }
        }

        if font_loaded {
            // 将中文字体设为最高优先级
            fonts
                .families
                .entry(egui::FontFamily::Proportional)
                .or_default()
                .insert(0, "chinese_font".to_owned());

            fonts
                .families
                .entry(egui::FontFamily::Monospace)
                .or_default()
                .push("chinese_font".to_owned());
        }

        ctx.set_fonts(fonts);
    }
}

impl eframe::App for TinyCaptureApp {
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        {
            use raw_window_handle::{HasWindowHandle, RawWindowHandle};
            let mut state = self.state.lock();
            if state.main_hwnd == 0 {
                if let Ok(handle) = frame.window_handle() {
                    if let RawWindowHandle::Win32(win32) = handle.as_raw() {
                        state.main_hwnd = win32.hwnd.get();
                    }
                }
            }
        }

        {
            let mut state = self.state.lock();
            let app_state = state.state_machine.state().clone();
            if matches!(app_state, AppState::Recording) {
                if state.recording_outline_hwnd == 0 {
                    if let Some(session) = state.state_machine.session() {
                        if let Ok(hwnd) = show_recording_outline(session.region) {
                            state.recording_outline_hwnd = hwnd;
                        }
                    }
                }
            } else if state.recording_outline_hwnd != 0 {
                destroy_recording_outline(state.recording_outline_hwnd);
                state.recording_outline_hwnd = 0;
            }
        }

        // Clone necessary data to avoid holding lock during UI rendering
        let (app_state, status_text, frame_count, on_record, on_stop, on_export) = {
            let state = self.state.lock();
            (
                state.state_machine.state().clone(),
                state.status_text.clone(),
                state.frame_count,
                state.on_record.clone(),
                state.on_stop.clone(),
                state.on_export.clone(),
            )
        };

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.vertical_centered(|ui| {
                ui.add_space(20.0);

                // Title with custom style
                ui.heading(
                    egui::RichText::new("🎬 TinyCapture")
                        .size(32.0)
                        .color(egui::Color32::from_rgb(51, 51, 51))
                );

                ui.add_space(10.0);
                ui.separator();
                ui.add_space(20.0);

                // Buttons row
                ui.horizontal(|ui| {
                    ui.add_space(20.0);

                    // Record button
                    let record_btn = egui::Button::new(
                        egui::RichText::new("🔴 录制")
                            .size(16.0)
                            .color(egui::Color32::WHITE)
                    )
                    .fill(if app_state.can_record() {
                        egui::Color32::from_rgb(220, 53, 69) // Red
                    } else {
                        egui::Color32::from_rgb(108, 117, 125) // Gray
                    })
                    .min_size(egui::vec2(120.0, 45.0))
                    .rounding(8.0);

                    if ui.add_enabled(app_state.can_record(), record_btn).clicked() {
                        if let Some(ref callback) = on_record {
                            callback();
                        }
                    }

                    ui.add_space(15.0);

                    // Stop button
                    let stop_btn = egui::Button::new(
                        egui::RichText::new("⏹ 停止")
                            .size(16.0)
                            .color(egui::Color32::WHITE)
                    )
                    .fill(if app_state.can_stop() {
                        egui::Color32::from_rgb(255, 193, 7) // Amber
                    } else {
                        egui::Color32::from_rgb(108, 117, 125) // Gray
                    })
                    .min_size(egui::vec2(120.0, 45.0))
                    .rounding(8.0);

                    if ui.add_enabled(app_state.can_stop(), stop_btn).clicked() {
                        if let Some(ref callback) = on_stop {
                            callback();
                        }
                    }

                    ui.add_space(15.0);

                    // Export button
                    let export_btn = egui::Button::new(
                        egui::RichText::new("💾 导出 GIF")
                            .size(16.0)
                            .color(egui::Color32::WHITE)
                    )
                    .fill(if app_state.can_export() {
                        egui::Color32::from_rgb(40, 167, 69) // Green
                    } else {
                        egui::Color32::from_rgb(108, 117, 125) // Gray
                    })
                    .min_size(egui::vec2(120.0, 45.0))
                    .rounding(8.0);

                    if ui.add_enabled(app_state.can_export(), export_btn).clicked() {
                        if let Some(ref callback) = on_export {
                            callback();
                        }
                    }
                });

                ui.add_space(25.0);

                // Status display with color coding
                let status_color = match app_state {
                    AppState::Recording => egui::Color32::from_rgb(255, 136, 0), // Orange
                    AppState::Exporting => egui::Color32::from_rgb(0, 136, 255), // Blue
                    AppState::Recorded => egui::Color32::from_rgb(40, 167, 69),  // Green
                    _ => egui::Color32::from_rgb(102, 102, 102), // Gray
                };

                ui.label(
                    egui::RichText::new(&status_text)
                        .size(18.0)
                        .color(status_color)
                );

                // Frame count display
                if frame_count > 0 {
                    ui.add_space(10.0);
                    ui.label(
                        egui::RichText::new(format!("已录制帧数: {}", frame_count))
                            .size(14.0)
                            .color(egui::Color32::from_rgb(136, 136, 136))
                    );
                }

                ui.add_space(20.0);

                // Info panel
                egui::Frame::none()
                    .fill(egui::Color32::from_rgb(245, 245, 245))
                    .inner_margin(15.0)
                    .rounding(8.0)
                    .show(ui, |ui| {
                        ui.label(
                            egui::RichText::new("💡 提示")
                                .size(14.0)
                                .color(egui::Color32::from_rgb(51, 51, 51))
                        );
                        ui.add_space(5.0);
                        ui.label(
                            egui::RichText::new("点击录制后，拖动框选区域或点击选择窗口")
                                .size(12.0)
                                .color(egui::Color32::from_rgb(102, 102, 102))
                        );
                    });
            });
        });

        // Request repaint for smooth animations
        ctx.request_repaint();
    }
}

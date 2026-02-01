//! TinyCapture - Modern UI version with egui

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod state;
mod tray;
mod ui_egui;

use crate::state::{RecordingSession, RecordingTarget};
use crate::ui_egui::{EguiUiState, TinyCaptureApp};
use capture_wgc::{CaptureController, CaptureTarget, FrameProcessor, Rect};
use crossbeam_channel::{bounded, Receiver, Sender};
use eframe::egui;
use export::{GifExportConfig, GifExporter};
use overlay::{show_recording_outline, destroy_recording_outline, OverlayWindow, SelectionOutcome};
use parking_lot::Mutex;
use std::path::PathBuf;
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};
use windows::Win32::Foundation::HWND;
use windows::Win32::System::WinRT::{RoInitialize, RoUninitialize, RO_INIT_MULTITHREADED};
use windows::Win32::UI::HiDpi::{
    SetProcessDpiAwarenessContext, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2,
};
use windows::Win32::UI::WindowsAndMessaging::{SetForegroundWindow, ShowWindow, SW_HIDE, SW_SHOW};

/// Capture worker commands
enum CaptureCommand {
    Start {
        target: CaptureTarget,
        crop_rect: Option<Rect>,
        output_dir: PathBuf,
        fps: u8,
    },
    Stop,
    Shutdown,
}

/// Capture worker result
enum CaptureResult {
    Started,
    Progress { elapsed_secs: u64, frame_count: usize },
    Stopped { frame_count: usize, duration_secs: f64 },
    Error(String),
}

fn main() -> anyhow::Result<()> {
    // Set DPI awareness
    unsafe {
        let _ = SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
    }

    // Create shared state
    let ui_state = Arc::new(Mutex::new(EguiUiState::new()));

    // Create capture worker channels
    let (cmd_tx, cmd_rx): (Sender<CaptureCommand>, Receiver<CaptureCommand>) = bounded(4);
    let (result_tx, result_rx): (Sender<CaptureResult>, Receiver<CaptureResult>) = bounded(4);

    // Start capture worker thread
    let capture_handle = thread::spawn(move || {
        capture_worker(cmd_rx, result_tx);
    });

    // Setup callbacks
    let cmd_tx_clone = cmd_tx.clone();
    let ui_state_clone = ui_state.clone();
    {
        let mut state = ui_state.lock();
        state.on_record = Some(Arc::new(move || {
            on_record_click(ui_state_clone.clone(), cmd_tx_clone.clone());
        }));
    }

    let cmd_tx_clone = cmd_tx.clone();
    let ui_state_clone = ui_state.clone();
    {
        let mut state = ui_state.lock();
        state.on_stop = Some(Arc::new(move || {
            on_stop_click(ui_state_clone.clone(), cmd_tx_clone.clone());
        }));
    }

    let ui_state_clone = ui_state.clone();
    {
        let mut state = ui_state.lock();
        state.on_export = Some(Arc::new(move || {
            on_export_click(ui_state_clone.clone());
        }));
    }

    // Start result handler thread
    let ui_state_clone = ui_state.clone();
    let _result_handle = thread::spawn(move || {
        result_handler(ui_state_clone, result_rx);
    });

    // Run egui app
    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([800.0, 400.0])
            .with_min_inner_size([600.0, 300.0])
            .with_title("TinyCapture")
            .with_resizable(true),
        ..Default::default()
    };

    let ui_state_for_app = ui_state.clone();
    let _ = eframe::run_native(
        "TinyCapture",
        native_options,
        Box::new(|cc| Ok(Box::new(TinyCaptureApp::new(cc, ui_state_for_app)))),
    );

    // Cleanup
    let _ = cmd_tx.send(CaptureCommand::Shutdown);
    let _ = capture_handle.join();

    Ok(())
}

fn set_main_window_visible(ui_state: &Arc<Mutex<EguiUiState>>, visible: bool) {
    let hwnd_raw = {
        let state = ui_state.lock();
        state.main_hwnd
    };
    if hwnd_raw == 0 {
        return;
    }

    let hwnd = HWND(hwnd_raw as *mut std::ffi::c_void);
    unsafe {
        ShowWindow(hwnd, if visible { SW_SHOW } else { SW_HIDE });
        if visible {
            let _ = SetForegroundWindow(hwnd);
        }
    }
}

fn on_record_click(ui_state: Arc<Mutex<EguiUiState>>, cmd_tx: Sender<CaptureCommand>) {
    // Start selecting
    {
        let mut state = ui_state.lock();
        if !state.state_machine.start_selecting() {
            return;
        }
        state.status_text = "选择区域...".to_string();
    }

    set_main_window_visible(&ui_state, false);

    // Show overlay in a new thread to avoid blocking the UI
    let ui_state_for_overlay = ui_state.clone();
    thread::spawn(move || {
        // Small delay for window to update
        thread::sleep(Duration::from_millis(100));

        // Show overlay and get selection
        let selection_result = OverlayWindow::show();

        match selection_result {
            Ok(SelectionOutcome::Region(rect)) => {
                // Create temp directory
                let temp_dir = std::env::temp_dir().join(format!("tinycapture_{}", uuid::Uuid::new_v4()));
                std::fs::create_dir_all(&temp_dir).ok();

                // Determine capture target
                let (capture_target, crop_rect, recording_rect) = determine_monitor_capture(&rect);

                // Start recording
                let session = RecordingSession::new(
                    capture_target.clone(),
                    recording_rect,
                    temp_dir.clone(),
                    15,
                );

                {
                    let mut state = ui_state.lock();
                    state.state_machine.start_recording(session);
                    state.status_text = "录制中...".to_string();
                    state.frame_count = 0;

                }

                // Send capture command
                let wgc_target = match capture_target {
                    RecordingTarget::Monitor { hmonitor, .. } => CaptureTarget::Monitor(hmonitor),
                    RecordingTarget::Window { hwnd } => CaptureTarget::Window(hwnd),
                };

                let _ = cmd_tx.send(CaptureCommand::Start {
                    target: wgc_target,
                    crop_rect,
                    output_dir: temp_dir,
                    fps: 15,
                });
            }
            Ok(SelectionOutcome::Window { hwnd, rect }) => {
                let temp_dir = std::env::temp_dir().join(format!("tinycapture_{}", uuid::Uuid::new_v4()));
                std::fs::create_dir_all(&temp_dir).ok();

                let capture_target = RecordingTarget::Window { hwnd };
                let session = RecordingSession::new(capture_target.clone(), rect, temp_dir.clone(), 15);

                {
                    let mut state = ui_state.lock();
                    state.state_machine.start_recording(session);
                    state.status_text = "录制中...".to_string();
                    state.frame_count = 0;

                }

                let wgc_target = CaptureTarget::Window(hwnd);
                let _ = cmd_tx.send(CaptureCommand::Start {
                    target: wgc_target,
                    crop_rect: None,
                    output_dir: temp_dir,
                    fps: 15,
                });
            }
            Ok(SelectionOutcome::Cancelled) | Err(_) => {
                let mut state = ui_state.lock();
                state.state_machine.cancel_selecting();
                state.status_text = "已取消".to_string();
            }
        }

        set_main_window_visible(&ui_state_for_overlay, true);
    });
}

fn determine_monitor_capture(rect: &Rect) -> (RecordingTarget, Option<Rect>, Rect) {
    let center_x = rect.x + rect.width as i32 / 2;
    let center_y = rect.y + rect.height as i32 / 2;

    unsafe {
        use windows::Win32::Foundation::POINT;
        use windows::Win32::Graphics::Gdi::{
            GetMonitorInfoW, MonitorFromPoint, MONITORINFO, MONITOR_DEFAULTTONEAREST,
        };

        let point = POINT { x: center_x, y: center_y };
        let hmonitor = MonitorFromPoint(point, MONITOR_DEFAULTTONEAREST);

        let mut mi = MONITORINFO::default();
        mi.cbSize = std::mem::size_of::<MONITORINFO>() as u32;
        let _ = GetMonitorInfoW(hmonitor, &mut mi);

        let monitor_left = mi.rcMonitor.left;
        let monitor_top = mi.rcMonitor.top;
        let monitor_width = (mi.rcMonitor.right - mi.rcMonitor.left).max(0) as u32;
        let monitor_height = (mi.rcMonitor.bottom - mi.rcMonitor.top).max(0) as u32;

        let mut x = rect.x - monitor_left;
        let mut y = rect.y - monitor_top;
        let mut w = rect.width;
        let mut h = rect.height;

        if x < 0 {
            w = w.saturating_sub((-x) as u32);
            x = 0;
        }
        if y < 0 {
            h = h.saturating_sub((-y) as u32);
            y = 0;
        }

        let crop_rect = if monitor_width == 0 || monitor_height == 0 {
            None
        } else {
            let ux = x as u32;
            let uy = y as u32;
            if ux >= monitor_width || uy >= monitor_height {
                None
            } else {
                let cw = w.min(monitor_width - ux);
                let ch = h.min(monitor_height - uy);
                if cw == 0 || ch == 0 {
                    None
                } else {
                    Some(Rect { x, y, width: cw, height: ch })
                }
            }
        };

        let recording_rect = if let Some(cr) = crop_rect {
            Rect {
                x: monitor_left + cr.x,
                y: monitor_top + cr.y,
                width: cr.width,
                height: cr.height,
            }
        } else {
            *rect
        };

        (
            RecordingTarget::Monitor {
                hmonitor: hmonitor.0 as isize,
                region: *rect,
            },
            crop_rect,
            recording_rect,
        )
    }
}

fn on_stop_click(ui_state: Arc<Mutex<EguiUiState>>, cmd_tx: Sender<CaptureCommand>) {
    let _ = cmd_tx.send(CaptureCommand::Stop);

    {
        let mut state = ui_state.lock();
        state.state_machine.stop_recording();
        state.status_text = "录制完成".to_string();

        // Destroy recording outline
        if state.recording_outline_hwnd != 0 {
            destroy_recording_outline(state.recording_outline_hwnd);
            state.recording_outline_hwnd = 0;
        }
    }
}

fn on_export_click(ui_state: Arc<Mutex<EguiUiState>>) {
    // Get frame paths
    let (frame_paths, frame_count, duration_secs) = {
        let state = ui_state.lock();
        if let Some(session) = state.state_machine.session() {
            (session.all_frame_paths(), session.frame_count, session.duration_secs)
        } else {
            return;
        }
    };

    // 检查帧数
    if frame_count == 0 || frame_paths.is_empty() {
        let mut state = ui_state.lock();
        state.status_text = "无可导出的帧，请先录制".to_string();
        return;
    }

    // 过滤掉不存在的文件
    let valid_frame_paths: Vec<PathBuf> = frame_paths
        .into_iter()
        .filter(|p| p.exists())
        .collect();

    if valid_frame_paths.is_empty() {
        let mut state = ui_state.lock();
        state.status_text = format!("没有找到录制的帧文件（预期 {} 帧）", frame_count);
        return;
    }

    if valid_frame_paths.len() < frame_count {
        // 有些帧丢失了，但继续尝试导出
        eprintln!("警告: 预期 {} 帧，实际找到 {} 帧", frame_count, valid_frame_paths.len());
    }

    // Show save dialog
    let output_path = rfd::FileDialog::new()
        .add_filter("GIF 图像", &["gif"])
        .set_file_name("recording.gif")
        .save_file();

    let output_path = match output_path {
        Some(path) => path,
        None => return,
    };

    // Start exporting
    {
        let mut state = ui_state.lock();
        state.state_machine.start_exporting();
        state.status_text = "导出中...".to_string();
    }

    // Export in background thread
    let ui_state_clone = ui_state.clone();
    thread::spawn(move || {
        let mut fps = 15u8;
        if duration_secs > 0.0 && frame_count > 0 {
            let calc = (frame_count as f64 / duration_secs).round() as i32;
            let clamped = calc.clamp(1, 60);
            fps = clamped as u8;
        }

        let config = GifExportConfig {
            output_path: output_path.clone(),
            fps,
            quality: 90,
            ..Default::default()
        };

        let result = GifExporter::export_from_pngs(&valid_frame_paths, config, None);

        let mut state = ui_state_clone.lock();
        match result {
            Ok(_) => {
                state.state_machine.finish_exporting();
                state.status_text = format!("已导出: {}", output_path.display());

                // Cleanup temp files
                if let Some(session) = state.state_machine.session() {
                    let _ = std::fs::remove_dir_all(&session.temp_dir);
                }
            }
            Err(e) => {
                state.state_machine.cancel_exporting();
                state.status_text = format!("导出失败: {}", e);
            }
        }
    });
}

fn capture_worker(cmd_rx: Receiver<CaptureCommand>, result_tx: Sender<CaptureResult>) {
    unsafe {
        if let Err(e) = RoInitialize(RO_INIT_MULTITHREADED) {
            let _ = result_tx.send(CaptureResult::Error(format!("WinRT init 失败: {}", e)));
            return;
        }
    }

    let mut controller: Option<CaptureController> = None;
    let mut processor: Option<FrameProcessor> = None;
    let mut running = false;
    let mut last_frame_time = Instant::now();
    let mut frame_interval = Duration::from_secs_f64(1.0 / 15.0);
    let mut start_time: Option<Instant> = None;
    let mut last_progress_secs: u64 = 0;

    loop {
        match cmd_rx.try_recv() {
            Ok(CaptureCommand::Start {
                target,
                crop_rect,
                output_dir,
                fps: target_fps,
            }) => {
                match CaptureController::new() {
                    Ok(mut ctrl) => {
                        ctrl.set_crop_rect(crop_rect);
                        if let Err(e) = ctrl.start(target) {
                            let _ = result_tx.send(CaptureResult::Error(e.to_string()));
                            continue;
                        }

                        let mut proc = FrameProcessor::new(output_dir);
                        // Crop is already applied in CaptureController::process_frame.
                        proc.set_crop_rect(None);

                        controller = Some(ctrl);
                        processor = Some(proc);
                        running = true;
                        frame_interval = Duration::from_secs_f64(1.0 / target_fps as f64);
                        last_frame_time = Instant::now();
                        start_time = Some(Instant::now());
                        last_progress_secs = 0;

                        let _ = result_tx.send(CaptureResult::Started);
                    }
                    Err(e) => {
                        let _ = result_tx.send(CaptureResult::Error(e.to_string()));
                    }
                }
            }
            Ok(CaptureCommand::Stop) => {
                if let Some(ctrl) = controller.take() {
                    drop(ctrl);
                }

                let frame_count = processor.as_ref().map(|p| p.frame_count()).unwrap_or(0);
                let duration_secs = start_time.map(|t| t.elapsed().as_secs_f64()).unwrap_or(0.0);
                processor = None;
                running = false;
                start_time = None;

                let _ = result_tx.send(CaptureResult::Stopped {
                    frame_count,
                    duration_secs,
                });
            }
            Ok(CaptureCommand::Shutdown) => {
                break;
            }
            Err(_) => {}
        }

        if running {
            if let (Some(ref ctrl), Some(ref mut proc)) = (&controller, &mut processor) {
                let now = Instant::now();
                if now.duration_since(last_frame_time) >= frame_interval {
                    if let Some(frame) = ctrl.try_get_frame() {
                        let _ = proc.process_frame(frame);
                        last_frame_time = now;
                    }
                }
            }

            if let Some(start) = start_time {
                let elapsed_secs = start.elapsed().as_secs();
                if elapsed_secs > last_progress_secs {
                    last_progress_secs = elapsed_secs;
                    let frame_count = processor.as_ref().map(|p| p.frame_count()).unwrap_or(0);
                    let _ = result_tx.send(CaptureResult::Progress {
                        elapsed_secs,
                        frame_count,
                    });
                }
            }
        }

        thread::sleep(Duration::from_millis(1));
    }

    unsafe {
        RoUninitialize();
    }
}

fn result_handler(ui_state: Arc<Mutex<EguiUiState>>, result_rx: Receiver<CaptureResult>) {
    loop {
        match result_rx.recv() {
            Ok(CaptureResult::Started) => {
                // Already handled
            }
            Ok(CaptureResult::Progress { elapsed_secs, frame_count }) => {
                let mut state = ui_state.lock();
                if matches!(
                    state.state_machine.state(),
                    crate::state::AppState::Recording
                ) {
                    state.status_text = format!("录制中... {}s", elapsed_secs);
                    state.frame_count = frame_count;
                    // 调试输出
                    eprintln!("进度更新: {}s, 帧数: {}", elapsed_secs, frame_count);
                }
            }
            Ok(CaptureResult::Stopped {
                frame_count,
                duration_secs,
            }) => {
                let mut state = ui_state.lock();
                state.frame_count = frame_count;
                if let Some(session) = state.state_machine.session_mut() {
                    session.frame_count = frame_count;
                    session.duration_secs = duration_secs;
                }
                let secs = duration_secs.max(0.0).round() as u64;
                state.status_text = format!("录制完成 ({}s)", secs);
            }
            Ok(CaptureResult::Error(msg)) => {
                let mut state = ui_state.lock();
                state.status_text = format!("错误: {}", msg);

                // Destroy recording outline
                if state.recording_outline_hwnd != 0 {
                    destroy_recording_outline(state.recording_outline_hwnd);
                    state.recording_outline_hwnd = 0;
                }

                // Cleanup temp files on error
                if let Some(session) = state.state_machine.session() {
                    let _ = std::fs::remove_dir_all(&session.temp_dir);
                }

                state.state_machine.reset();
            }
            Err(_) => {
                break;
            }
        }
    }
}

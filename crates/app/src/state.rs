//! State machine for WinGIF

use capture_wgc::Rect;
use std::path::PathBuf;

/// Application state
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AppState {
    /// Idle state - ready to record
    Idle,
    /// Selecting region/window
    Selecting,
    /// Recording in progress
    Recording,
    /// Recording finished, ready to export
    Recorded,
    /// Exporting in progress
    Exporting,
}

impl AppState {
    /// Get display text for current state
    pub fn display_text(&self) -> &'static str {
        match self {
            AppState::Idle => "就绪",
            AppState::Selecting => "选择区域...",
            AppState::Recording => "录制中...",
            AppState::Recorded => "录制完成",
            AppState::Exporting => "导出中...",
        }
    }

    /// Check if recording button should be enabled
    pub fn can_record(&self) -> bool {
        matches!(self, AppState::Idle | AppState::Recorded)
    }

    /// Check if stop button should be enabled
    pub fn can_stop(&self) -> bool {
        matches!(self, AppState::Recording)
    }

    /// Check if export button should be enabled
    pub fn can_export(&self) -> bool {
        matches!(self, AppState::Recorded)
    }
}

/// Recording session data
#[derive(Debug, Clone)]
pub struct RecordingSession {
    /// Capture target type
    pub target: RecordingTarget,
    /// Region to capture (screen coordinates)
    pub region: Rect,
    /// Temp directory for frames
    pub temp_dir: PathBuf,
    /// Frame count
    pub frame_count: usize,
    /// Recording duration in seconds
    pub duration_secs: f64,
    /// FPS setting
    pub fps: u8,
}

/// Recording target type
#[derive(Debug, Clone)]
pub enum RecordingTarget {
    /// Capture a specific region of a monitor
    Monitor { hmonitor: isize, region: Rect },
    /// Capture a window
    Window { hwnd: isize },
}

impl RecordingSession {
    /// Create a new recording session
    pub fn new(target: RecordingTarget, region: Rect, temp_dir: PathBuf, fps: u8) -> Self {
        Self {
            target,
            region,
            temp_dir,
            frame_count: 0,
            duration_secs: 0.0,
            fps,
        }
    }

    /// Get frame file path
    pub fn frame_path(&self, index: usize) -> PathBuf {
        self.temp_dir.join(format!("frame_{:05}.png", index))
    }

    /// Get all frame paths
    pub fn all_frame_paths(&self) -> Vec<PathBuf> {
        (0..self.frame_count)
            .map(|i| self.frame_path(i))
            .collect()
    }
}

/// State machine transitions
pub struct StateMachine {
    state: AppState,
    session: Option<RecordingSession>,
}

impl StateMachine {
    /// Create a new state machine
    pub fn new() -> Self {
        Self {
            state: AppState::Idle,
            session: None,
        }
    }

    /// Get current state
    pub fn state(&self) -> &AppState {
        &self.state
    }

    /// Get current session
    pub fn session(&self) -> Option<&RecordingSession> {
        self.session.as_ref()
    }

    /// Get mutable session
    pub fn session_mut(&mut self) -> Option<&mut RecordingSession> {
        self.session.as_mut()
    }

    /// Transition to selecting state
    pub fn start_selecting(&mut self) -> bool {
        if self.state.can_record() {
            self.state = AppState::Selecting;
            true
        } else {
            false
        }
    }

    /// Transition to recording state with session
    pub fn start_recording(&mut self, session: RecordingSession) -> bool {
        if matches!(self.state, AppState::Selecting) {
            self.session = Some(session);
            self.state = AppState::Recording;
            true
        } else {
            false
        }
    }

    /// Cancel selection and return to idle
    pub fn cancel_selecting(&mut self) -> bool {
        if matches!(self.state, AppState::Selecting) {
            self.state = AppState::Idle;
            true
        } else {
            false
        }
    }

    /// Stop recording
    pub fn stop_recording(&mut self) -> bool {
        if self.state.can_stop() {
            self.state = AppState::Recorded;
            true
        } else {
            false
        }
    }

    /// Start exporting
    pub fn start_exporting(&mut self) -> bool {
        if self.state.can_export() {
            self.state = AppState::Exporting;
            true
        } else {
            false
        }
    }

    /// Export finished, return to idle
    pub fn finish_exporting(&mut self) {
        self.state = AppState::Idle;
        self.session = None;
    }

    /// Export cancelled or failed, return to recorded
    pub fn cancel_exporting(&mut self) {
        if matches!(self.state, AppState::Exporting) {
            self.state = AppState::Recorded;
        }
    }

    /// Reset to idle
    pub fn reset(&mut self) {
        self.state = AppState::Idle;
        self.session = None;
    }
}

impl Default for StateMachine {
    fn default() -> Self {
        Self::new()
    }
}

//! Overlay module for TinyCapture
//!
//! Provides frozen screenshot overlay with region/window selection.

pub mod render;
pub mod screenshot;
pub mod selection;
pub mod window;

pub use selection::{SelectionMode, SelectionResult, WindowInfo};
pub use window::OverlayWindow;

use capture_wgc::Rect;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum OverlayError {
    #[error("Windows API error: {0}")]
    Windows(#[from] windows::core::Error),

    #[error("Screenshot failed: {0}")]
    Screenshot(String),

    #[error("Selection cancelled")]
    Cancelled,

    #[error("No selection made")]
    NoSelection,
}

pub type OverlayResult<T> = Result<T, OverlayError>;

/// Selection outcome
#[derive(Debug, Clone)]
pub enum SelectionOutcome {
    /// User selected a region
    Region(Rect),
    /// User selected a window
    Window { hwnd: isize, rect: Rect },
    /// User cancelled
    Cancelled,
}

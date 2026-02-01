//! Windows Graphics Capture module for TinyCapture
//!
//! Provides screen and window capture using WGC API.

pub mod capture;
pub mod d3d11;
pub mod frame;

pub use capture::{CaptureController, CaptureTarget};
pub use d3d11::D3D11Device;
pub use frame::{FrameData, FrameProcessor};

use thiserror::Error;
use windows::core::Error as WinError;

#[derive(Error, Debug)]
pub enum CaptureError {
    #[error("Windows API error: {0}")]
    Windows(#[from] WinError),

    #[error("D3D11 error: {0}")]
    D3D11(String),

    #[error("Capture not supported")]
    NotSupported,

    #[error("Invalid capture target")]
    InvalidTarget,

    #[error("Frame pool error: {0}")]
    FramePool(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Image error: {0}")]
    Image(#[from] image::ImageError),

    #[error("Capture stopped")]
    Stopped,
}

pub type CaptureResult<T> = Result<T, CaptureError>;

/// Rectangle in physical pixels
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

impl Rect {
    pub fn new(x: i32, y: i32, width: u32, height: u32) -> Self {
        Self { x, y, width, height }
    }

    pub fn right(&self) -> i32 {
        self.x + self.width as i32
    }

    pub fn bottom(&self) -> i32 {
        self.y + self.height as i32
    }

    pub fn contains(&self, x: i32, y: i32) -> bool {
        x >= self.x && x < self.right() && y >= self.y && y < self.bottom()
    }

    pub fn intersects(&self, other: &Rect) -> bool {
        self.x < other.right() && self.right() > other.x &&
        self.y < other.bottom() && self.bottom() > other.y
    }
}

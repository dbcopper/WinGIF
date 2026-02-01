//! Export module for TinyCapture
//!
//! Provides GIF and PNG export functionality.

mod gif;
mod png;

pub use gif::{GifExporter, GifExportConfig};
pub use png::PngExporter;

use std::path::PathBuf;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ExportError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Image error: {0}")]
    Image(#[from] image::ImageError),

    #[error("GIF encoding error: {0}")]
    GifEncode(String),

    #[error("No frames to export")]
    NoFrames,

    #[error("Export cancelled")]
    Cancelled,
}

pub type ExportResult<T> = Result<T, ExportError>;

/// Progress callback type
pub type ProgressCallback = Box<dyn Fn(f32) + Send>;

/// Export format
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportFormat {
    Gif,
    PngSequence,
}

/// Common export configuration
#[derive(Debug, Clone)]
pub struct ExportConfig {
    pub format: ExportFormat,
    pub output_path: PathBuf,
    pub fps: u8,
    pub quality: u8,
}

impl Default for ExportConfig {
    fn default() -> Self {
        Self {
            format: ExportFormat::Gif,
            output_path: PathBuf::new(),
            fps: 15,
            quality: 90,
        }
    }
}

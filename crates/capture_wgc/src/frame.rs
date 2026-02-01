//! Frame processing and PNG saving

use crate::{CaptureResult, Rect};
use image::{ImageBuffer, RgbaImage};
use std::path::Path;
use std::time::Instant;

/// Frame data from capture
#[derive(Debug, Clone)]
pub struct FrameData {
    pub data: Vec<u8>,
    pub width: u32,
    pub height: u32,
    pub timestamp: Instant,
}

impl FrameData {
    /// Convert BGRA data to RGBA image
    pub fn to_rgba_image(&self) -> RgbaImage {
        let mut rgba_data = self.data.clone();

        // Convert BGRA to RGBA
        for chunk in rgba_data.chunks_exact_mut(4) {
            chunk.swap(0, 2);
        }

        ImageBuffer::from_raw(self.width, self.height, rgba_data)
            .expect("Failed to create image buffer")
    }

    /// Save as PNG
    pub fn save_png(&self, path: &Path) -> CaptureResult<()> {
        let img = self.to_rgba_image();
        img.save(path)?;
        Ok(())
    }

    /// Crop frame to rectangle
    pub fn crop(&self, rect: &Rect) -> FrameData {
        let src_x = rect.x.max(0) as u32;
        let src_y = rect.y.max(0) as u32;
        let crop_width = rect.width.min(self.width.saturating_sub(src_x));
        let crop_height = rect.height.min(self.height.saturating_sub(src_y));

        let mut cropped_data = Vec::with_capacity((crop_width * crop_height * 4) as usize);

        for y in 0..crop_height {
            let src_offset = ((src_y + y) * self.width + src_x) as usize * 4;
            let row_data = &self.data[src_offset..src_offset + (crop_width as usize * 4)];
            cropped_data.extend_from_slice(row_data);
        }

        FrameData {
            data: cropped_data,
            width: crop_width,
            height: crop_height,
            timestamp: self.timestamp,
        }
    }
}

/// Frame processor for recording
pub struct FrameProcessor {
    output_dir: std::path::PathBuf,
    frame_count: usize,
    crop_rect: Option<Rect>,
}

impl FrameProcessor {
    /// Create a new frame processor
    pub fn new(output_dir: std::path::PathBuf) -> Self {
        Self {
            output_dir,
            frame_count: 0,
            crop_rect: None,
        }
    }

    /// Set crop rectangle
    pub fn set_crop_rect(&mut self, rect: Option<Rect>) {
        self.crop_rect = rect;
    }

    /// Process and save a frame
    pub fn process_frame(&mut self, frame: FrameData) -> CaptureResult<std::path::PathBuf> {
        let frame_to_save = if let Some(ref rect) = self.crop_rect {
            frame.crop(rect)
        } else {
            frame
        };

        let filename = format!("frame_{:05}.png", self.frame_count);
        let path = self.output_dir.join(&filename);

        frame_to_save.save_png(&path)?;
        self.frame_count += 1;

        Ok(path)
    }

    /// Get current frame count
    pub fn frame_count(&self) -> usize {
        self.frame_count
    }

    /// Get all saved frame paths
    pub fn get_frame_paths(&self) -> Vec<std::path::PathBuf> {
        (0..self.frame_count)
            .map(|i| self.output_dir.join(format!("frame_{:05}.png", i)))
            .collect()
    }

    /// Reset frame count
    pub fn reset(&mut self) {
        self.frame_count = 0;
    }
}

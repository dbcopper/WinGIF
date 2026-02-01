//! PNG sequence export

use crate::{ExportError, ExportResult, ProgressCallback};
use std::fs;
use std::path::{Path, PathBuf};

/// PNG sequence exporter
pub struct PngExporter;

impl PngExporter {
    /// Copy PNG sequence to output directory
    pub fn export(
        png_paths: &[PathBuf],
        output_dir: &Path,
        progress: Option<ProgressCallback>,
    ) -> ExportResult<PathBuf> {
        if png_paths.is_empty() {
            return Err(ExportError::NoFrames);
        }

        // Create output directory
        fs::create_dir_all(output_dir)?;

        let total = png_paths.len();

        for (i, src_path) in png_paths.iter().enumerate() {
            let filename = format!("frame_{:05}.png", i);
            let dest_path = output_dir.join(&filename);
            fs::copy(src_path, &dest_path)?;

            if let Some(ref cb) = progress {
                cb((i + 1) as f32 / total as f32);
            }
        }

        Ok(output_dir.to_path_buf())
    }

    /// Get frame count from a directory
    pub fn count_frames(dir: &Path) -> ExportResult<usize> {
        let mut count = 0;
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            if entry.path().extension().map_or(false, |e| e == "png") {
                count += 1;
            }
        }
        Ok(count)
    }
}

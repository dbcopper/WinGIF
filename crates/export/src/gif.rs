//! GIF export using gifski

use crate::{ExportError, ExportResult, ProgressCallback};
use crossbeam_channel::{bounded, Receiver, Sender};
use gifski::{Collector, Settings, Writer};
use image::RgbaImage;
use imgref::ImgVec;
use rgb::RGBA8;
use std::fs::File;
use std::path::{Path, PathBuf};
use std::thread;

/// GIF export configuration
#[derive(Debug, Clone)]
pub struct GifExportConfig {
    pub output_path: PathBuf,
    pub fps: u8,
    pub quality: u8,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub fast: bool,
}

impl Default for GifExportConfig {
    fn default() -> Self {
        Self {
            output_path: PathBuf::new(),
            fps: 15,
            quality: 90,
            width: None,
            height: None,
            fast: false,
        }
    }
}

/// Convert image::RgbaImage to imgref::ImgVec<RGBA8>
fn rgba_image_to_imgvec(img: RgbaImage) -> ImgVec<RGBA8> {
    let width = img.width() as usize;
    let height = img.height() as usize;
    let raw = img.into_raw();

    // Convert Vec<u8> to Vec<RGBA8>
    let pixels: Vec<RGBA8> = raw
        .chunks_exact(4)
        .map(|chunk| RGBA8::new(chunk[0], chunk[1], chunk[2], chunk[3]))
        .collect();

    ImgVec::new(pixels, width, height)
}

/// Frame data for GIF export
pub struct GifFrame {
    pub image: ImgVec<RGBA8>,
    pub timestamp: f64,
}

/// GIF exporter using gifski
pub struct GifExporter {
    config: GifExportConfig,
    frame_sender: Option<Sender<GifFrame>>,
    collector_handle: Option<thread::JoinHandle<ExportResult<()>>>,
    writer_handle: Option<thread::JoinHandle<ExportResult<()>>>,
    frame_count: usize,
}

impl GifExporter {
    /// Create a new GIF exporter
    pub fn new(config: GifExportConfig) -> ExportResult<Self> {
        Ok(Self {
            config,
            frame_sender: None,
            collector_handle: None,
            writer_handle: None,
            frame_count: 0,
        })
    }

    /// Start the export process
    pub fn start(&mut self) -> ExportResult<()> {
        let settings = Settings {
            width: self.config.width,
            height: self.config.height,
            quality: self.config.quality,
            fast: self.config.fast,
            repeat: gifski::Repeat::Infinite,
        };

        let (collector, writer) = gifski::new(settings)
            .map_err(|e| ExportError::GifEncode(e.to_string()))?;

        let (frame_tx, frame_rx): (Sender<GifFrame>, Receiver<GifFrame>) = bounded(16);
        self.frame_sender = Some(frame_tx);

        // Collector thread
        let collector_handle = thread::spawn(move || {
            Self::collector_thread(collector, frame_rx)
        });
        self.collector_handle = Some(collector_handle);

        // Writer thread
        let output_path = self.config.output_path.clone();
        let writer_handle = thread::spawn(move || {
            Self::writer_thread(writer, &output_path)
        });
        self.writer_handle = Some(writer_handle);

        Ok(())
    }

    fn collector_thread(collector: Collector, frame_rx: Receiver<GifFrame>) -> ExportResult<()> {
        let mut index = 0;
        for frame in frame_rx {
            collector.add_frame_rgba(index, frame.image, frame.timestamp)
                .map_err(|e| ExportError::GifEncode(e.to_string()))?;
            index += 1;
        }
        Ok(())
    }

    fn writer_thread(writer: Writer, output_path: &Path) -> ExportResult<()> {
        let file = File::create(output_path)?;
        writer.write(file, &mut gifski::progress::NoProgress {})
            .map_err(|e| ExportError::GifEncode(e.to_string()))?;
        Ok(())
    }

    /// Add a frame to the GIF
    pub fn add_frame(&mut self, image: RgbaImage) -> ExportResult<()> {
        let sender = self.frame_sender.as_ref()
            .ok_or_else(|| ExportError::GifEncode("Exporter not started".to_string()))?;

        let timestamp = self.frame_count as f64 / self.config.fps as f64;
        let imgvec = rgba_image_to_imgvec(image);

        sender.send(GifFrame { image: imgvec, timestamp })
            .map_err(|_| ExportError::GifEncode("Failed to send frame".to_string()))?;

        self.frame_count += 1;
        Ok(())
    }

    /// Finish the export and return the output path
    pub fn finish(mut self) -> ExportResult<PathBuf> {
        if self.frame_count == 0 {
            return Err(ExportError::NoFrames);
        }

        // Drop sender to signal completion
        drop(self.frame_sender.take());

        // Wait for collector
        if let Some(handle) = self.collector_handle.take() {
            handle.join()
                .map_err(|_| ExportError::GifEncode("Collector thread panicked".to_string()))??;
        }

        // Wait for writer
        if let Some(handle) = self.writer_handle.take() {
            handle.join()
                .map_err(|_| ExportError::GifEncode("Writer thread panicked".to_string()))??;
        }

        Ok(self.config.output_path.clone())
    }

    /// Export PNG files to GIF
    pub fn export_from_pngs(
        png_paths: &[PathBuf],
        config: GifExportConfig,
        progress: Option<ProgressCallback>,
    ) -> ExportResult<PathBuf> {
        if png_paths.is_empty() {
            return Err(ExportError::NoFrames);
        }

        let settings = Settings {
            width: config.width,
            height: config.height,
            quality: config.quality,
            fast: config.fast,
            repeat: gifski::Repeat::Infinite,
        };

        let (collector, writer) = gifski::new(settings)
            .map_err(|e| ExportError::GifEncode(e.to_string()))?;

        let total = png_paths.len();
        let fps = config.fps;
        let paths = png_paths.to_vec();

        // Collector thread
        let collector_handle = thread::spawn(move || -> ExportResult<()> {
            for (i, path) in paths.iter().enumerate() {
                let img = image::open(path)?.to_rgba8();
                let imgvec = rgba_image_to_imgvec(img);
                let timestamp = i as f64 / fps as f64;
                collector.add_frame_rgba(i, imgvec, timestamp)
                    .map_err(|e| ExportError::GifEncode(e.to_string()))?;

                if let Some(ref cb) = progress {
                    cb((i + 1) as f32 / total as f32 * 0.8);
                }
            }
            Ok(())
        });

        // Writer thread
        let output_path = config.output_path.clone();
        let writer_handle = thread::spawn(move || -> ExportResult<()> {
            let file = File::create(&output_path)?;
            writer.write(file, &mut gifski::progress::NoProgress {})
                .map_err(|e| ExportError::GifEncode(e.to_string()))?;
            Ok(())
        });

        collector_handle.join()
            .map_err(|_| ExportError::GifEncode("Collector thread panicked".to_string()))??;

        writer_handle.join()
            .map_err(|_| ExportError::GifEncode("Writer thread panicked".to_string()))??;

        Ok(config.output_path)
    }
}

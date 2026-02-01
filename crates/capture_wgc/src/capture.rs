//! WGC capture core

use crate::{CaptureResult, D3D11Device, FrameData, Rect};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use windows::{
    Graphics::Capture::{Direct3D11CaptureFramePool, GraphicsCaptureItem, GraphicsCaptureSession},
    Graphics::DirectX::DirectXPixelFormat,
    Graphics::SizeInt32,
    Win32::Foundation::HWND,
    Win32::Graphics::Direct3D11::{
        ID3D11Texture2D, D3D11_CPU_ACCESS_READ,
        D3D11_TEXTURE2D_DESC, D3D11_USAGE_STAGING,
    },
    Win32::Graphics::Dxgi::Common::DXGI_FORMAT_B8G8R8A8_UNORM,
    Win32::System::WinRT::Graphics::Capture::IGraphicsCaptureItemInterop,
};

/// Capture target
#[derive(Debug, Clone)]
pub enum CaptureTarget {
    Window(isize),
    Monitor(isize),
}

/// Capture controller
pub struct CaptureController {
    device: D3D11Device,
    session: Option<GraphicsCaptureSession>,
    frame_pool: Option<Direct3D11CaptureFramePool>,
    crop_rect: Option<Rect>,
    running: Arc<AtomicBool>,
}

impl CaptureController {
    /// Create a new capture controller
    pub fn new() -> CaptureResult<Self> {
        let device = D3D11Device::new()?;
        Ok(Self {
            device,
            session: None,
            frame_pool: None,
            crop_rect: None,
            running: Arc::new(AtomicBool::new(false)),
        })
    }

    /// Set crop rectangle (in physical pixels relative to capture target)
    pub fn set_crop_rect(&mut self, rect: Option<Rect>) {
        self.crop_rect = rect;
    }

    /// Start capture
    pub fn start(&mut self, target: CaptureTarget) -> CaptureResult<()> {
        let item = self.create_capture_item(&target)?;
        let size = item.Size()?;

        // Create frame pool
        let frame_pool = Direct3D11CaptureFramePool::CreateFreeThreaded(
            self.device.d3d_device(),
            DirectXPixelFormat::B8G8R8A8UIntNormalized,
            2,
            size,
        )?;

        // Create session
        let session = frame_pool.CreateCaptureSession(&item)?;

        self.running.store(true, Ordering::SeqCst);
        session.StartCapture()?;

        self.session = Some(session);
        self.frame_pool = Some(frame_pool);

        Ok(())
    }

    /// Stop capture
    pub fn stop(&mut self) {
        self.running.store(false, Ordering::SeqCst);

        if let Some(session) = self.session.take() {
            let _ = session.Close();
        }

        if let Some(pool) = self.frame_pool.take() {
            let _ = pool.Close();
        }
    }

    /// Try to get the next frame (polling approach)
    pub fn try_get_frame(&self) -> Option<FrameData> {
        if !self.running.load(Ordering::SeqCst) {
            return None;
        }

        let frame_pool = self.frame_pool.as_ref()?;
        let frame = frame_pool.TryGetNextFrame().ok()?;
        let size = frame.ContentSize().ok()?;
        let surface = frame.Surface().ok()?;

        Self::process_frame(&self.device, &surface, size, self.crop_rect).ok()
    }

    /// Check if running
    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::SeqCst)
    }

    fn create_capture_item(&self, target: &CaptureTarget) -> CaptureResult<GraphicsCaptureItem> {
        unsafe {
            let interop: IGraphicsCaptureItemInterop =
                windows::core::factory::<GraphicsCaptureItem, IGraphicsCaptureItemInterop>()?;

            match target {
                CaptureTarget::Window(hwnd) => {
                    let item: GraphicsCaptureItem = interop.CreateForWindow(HWND(*hwnd as _))?;
                    Ok(item)
                }
                CaptureTarget::Monitor(hmonitor) => {
                    let item: GraphicsCaptureItem = interop.CreateForMonitor(
                        windows::Win32::Graphics::Gdi::HMONITOR(*hmonitor as _),
                    )?;
                    Ok(item)
                }
            }
        }
    }

    fn process_frame(
        device: &D3D11Device,
        surface: &windows::Graphics::DirectX::Direct3D11::IDirect3DSurface,
        size: SizeInt32,
        crop_rect: Option<Rect>,
    ) -> CaptureResult<FrameData> {
        unsafe {
            // Get D3D11 texture from surface
            let texture: ID3D11Texture2D = D3D11Device::get_d3d11_interface(surface)?;

            // Determine actual copy region
            let (src_x, src_y, width, height) = if let Some(rect) = crop_rect {
                (
                    rect.x.max(0) as u32,
                    rect.y.max(0) as u32,
                    rect.width.min((size.Width as i32 - rect.x.max(0)) as u32),
                    rect.height.min((size.Height as i32 - rect.y.max(0)) as u32),
                )
            } else {
                (0, 0, size.Width as u32, size.Height as u32)
            };

            // Create staging texture
            let desc = D3D11_TEXTURE2D_DESC {
                Width: width,
                Height: height,
                MipLevels: 1,
                ArraySize: 1,
                Format: DXGI_FORMAT_B8G8R8A8_UNORM,
                SampleDesc: windows::Win32::Graphics::Dxgi::Common::DXGI_SAMPLE_DESC {
                    Count: 1,
                    Quality: 0,
                },
                Usage: D3D11_USAGE_STAGING,
                BindFlags: 0,
                CPUAccessFlags: D3D11_CPU_ACCESS_READ.0 as u32,
                MiscFlags: 0,
            };

            let mut staging_texture: Option<ID3D11Texture2D> = None;
            device.device().CreateTexture2D(&desc, None, Some(&mut staging_texture))?;
            let staging_texture = staging_texture.unwrap();

            // Copy region
            let src_box = windows::Win32::Graphics::Direct3D11::D3D11_BOX {
                left: src_x,
                top: src_y,
                front: 0,
                right: src_x + width,
                bottom: src_y + height,
                back: 1,
            };

            device.context().CopySubresourceRegion(
                &staging_texture,
                0,
                0,
                0,
                0,
                &texture,
                0,
                Some(&src_box),
            );

            // Map and read data
            let mut mapped = windows::Win32::Graphics::Direct3D11::D3D11_MAPPED_SUBRESOURCE::default();
            device.context().Map(
                &staging_texture,
                0,
                windows::Win32::Graphics::Direct3D11::D3D11_MAP_READ,
                0,
                Some(&mut mapped),
            )?;

            // Copy pixel data
            let row_pitch = mapped.RowPitch as usize;
            let mut data = Vec::with_capacity((width * height * 4) as usize);

            for y in 0..height {
                let src_row = std::slice::from_raw_parts(
                    (mapped.pData as *const u8).add(y as usize * row_pitch),
                    width as usize * 4,
                );
                data.extend_from_slice(src_row);
            }

            device.context().Unmap(&staging_texture, 0);

            Ok(FrameData {
                data,
                width,
                height,
                timestamp: std::time::Instant::now(),
            })
        }
    }
}

impl Drop for CaptureController {
    fn drop(&mut self) {
        self.stop();
    }
}

//! D3D11 device management

use crate::CaptureResult;
use windows::{
    core::Interface,
    Graphics::DirectX::Direct3D11::IDirect3DDevice,
    Win32::Graphics::{
        Direct3D::{D3D_DRIVER_TYPE_HARDWARE, D3D_FEATURE_LEVEL_11_0},
        Direct3D11::{
            D3D11CreateDevice, ID3D11Device, ID3D11DeviceContext,
            D3D11_CREATE_DEVICE_BGRA_SUPPORT, D3D11_SDK_VERSION,
        },
        Dxgi::IDXGIDevice,
    },
    Win32::System::WinRT::Direct3D11::{
        CreateDirect3D11DeviceFromDXGIDevice, IDirect3DDxgiInterfaceAccess,
    },
};

/// D3D11 device wrapper
pub struct D3D11Device {
    device: ID3D11Device,
    context: ID3D11DeviceContext,
    d3d_device: IDirect3DDevice,
}

impl D3D11Device {
    /// Create a new D3D11 device
    pub fn new() -> CaptureResult<Self> {
        unsafe {
            let mut device: Option<ID3D11Device> = None;
            let mut context: Option<ID3D11DeviceContext> = None;

            D3D11CreateDevice(
                None,
                D3D_DRIVER_TYPE_HARDWARE,
                None,
                D3D11_CREATE_DEVICE_BGRA_SUPPORT,
                Some(&[D3D_FEATURE_LEVEL_11_0]),
                D3D11_SDK_VERSION,
                Some(&mut device),
                None,
                Some(&mut context),
            )?;

            let device = device.unwrap();
            let context = context.unwrap();

            // Get IDirect3DDevice for WGC
            let dxgi_device: IDXGIDevice = device.cast()?;
            let inspectable = CreateDirect3D11DeviceFromDXGIDevice(&dxgi_device)?;
            let d3d_device: IDirect3DDevice = inspectable.cast()?;

            Ok(Self {
                device,
                context,
                d3d_device,
            })
        }
    }

    /// Get the D3D11 device
    pub fn device(&self) -> &ID3D11Device {
        &self.device
    }

    /// Get the device context
    pub fn context(&self) -> &ID3D11DeviceContext {
        &self.context
    }

    /// Get the WinRT Direct3D device
    pub fn d3d_device(&self) -> &IDirect3DDevice {
        &self.d3d_device
    }

    /// Get the underlying D3D11 device from a WinRT texture
    pub fn get_d3d11_interface<T: Interface>(
        wrapper: &impl Interface,
    ) -> CaptureResult<T> {
        unsafe {
            let access: IDirect3DDxgiInterfaceAccess = wrapper.cast()?;
            Ok(access.GetInterface()?)
        }
    }
}

impl Clone for D3D11Device {
    fn clone(&self) -> Self {
        Self {
            device: self.device.clone(),
            context: self.context.clone(),
            d3d_device: self.d3d_device.clone(),
        }
    }
}

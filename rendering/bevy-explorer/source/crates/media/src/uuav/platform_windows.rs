//! Windows: audio plays, video does not yet.
//!
//! The client derives its device from an `ID3D11Texture2D` probe and presents each frame into an
//! NV12 `ID3D11Texture2D` on that device. The renderer runs on whichever backend wgpu picks
//! (Vulkan or DX12), so that texture cannot be wrapped the way the Metal planes are.
//!
//! TODO(uuav-windows): evaluate and pick the frame path. The candidates are a shared NT handle
//! from the client's presentation texture opened on a DX12 renderer (which pins the backend
//! to DX12 and needs `TEXTURE_FORMAT_NV12` plus cross-API synchronisation), Vulkan external
//! memory import of the same handle, and a CPU readback through a D3D11 staging texture
//! uploaded like the Linux planes (the software fallback that works on every backend). Until
//! then `make_planes` returns nothing and the target image keeps its placeholder.

use std::{os::raw::c_void, sync::Once};

use bevy::{
    log::warn,
    render::renderer::{RenderAdapter, RenderDevice, RenderQueue},
};
use windows::{
    Win32::{
        Foundation::HMODULE,
        Graphics::{
            Direct3D::D3D_DRIVER_TYPE_HARDWARE,
            Direct3D11::{
                D3D11_BIND_SHADER_RESOURCE, D3D11_CREATE_DEVICE_FLAG, D3D11_SDK_VERSION,
                D3D11_TEXTURE2D_DESC, D3D11_USAGE_DEFAULT, D3D11CreateDevice, ID3D11Device,
                ID3D11Texture2D,
            },
            Dxgi::Common::{DXGI_FORMAT_R8G8B8A8_UNORM, DXGI_SAMPLE_DESC},
        },
    },
    core::Interface as _,
};

/// No frames are presented yet (see the module doc): the render world skips the per-player
/// render event, so the client makes no D3D11 copy nothing would show.
pub const PRESENTS_FRAMES: bool = false;

/// A 1x1 texture on a D3D11 device of our own: the decode device the client captures. It is
/// not the renderer's device, which is what the TODO above is about.
pub struct Probe {
    _device: ID3D11Device,
    texture: ID3D11Texture2D,
}

impl Probe {
    pub fn ptr(&self) -> *const c_void {
        self.texture.as_raw().cast_const()
    }
}

pub fn probe(_device: &RenderDevice, _adapter: &RenderAdapter) -> Result<Probe, String> {
    let mut device: Option<ID3D11Device> = None;
    unsafe {
        D3D11CreateDevice(
            None,
            D3D_DRIVER_TYPE_HARDWARE,
            HMODULE::default(),
            D3D11_CREATE_DEVICE_FLAG(0),
            None,
            D3D11_SDK_VERSION,
            Some(&mut device),
            None,
            None,
        )
    }
    .map_err(|e| format!("D3D11CreateDevice failed: {e}"))?;
    let device = device.ok_or_else(|| "D3D11CreateDevice returned no device".to_owned())?;

    let desc = D3D11_TEXTURE2D_DESC {
        Width: 1,
        Height: 1,
        MipLevels: 1,
        ArraySize: 1,
        Format: DXGI_FORMAT_R8G8B8A8_UNORM,
        SampleDesc: DXGI_SAMPLE_DESC {
            Count: 1,
            Quality: 0,
        },
        Usage: D3D11_USAGE_DEFAULT,
        BindFlags: D3D11_BIND_SHADER_RESOURCE.0 as u32,
        CPUAccessFlags: 0,
        MiscFlags: 0,
    };
    let mut texture: Option<ID3D11Texture2D> = None;
    unsafe { device.CreateTexture2D(&desc, None, Some(&mut texture)) }
        .map_err(|e| format!("probe CreateTexture2D failed: {e}"))?;
    let texture = texture.ok_or_else(|| "CreateTexture2D returned no texture".to_owned())?;
    Ok(Probe {
        _device: device,
        texture,
    })
}

pub fn make_planes(
    _device: &RenderDevice,
    _y: *const c_void,
    _uv: *const c_void,
    _width: u32,
    _height: u32,
) -> Option<(wgpu::Texture, wgpu::Texture)> {
    static TODO: Once = Once::new();
    TODO.call_once(|| {
        warn!(
            "video frames are not shown on Windows yet: the D3D11 presentation texture has no \
             path into the renderer (TODO(uuav-windows) in media/src/uuav/platform_windows.rs)"
        );
    });
    None
}

pub fn upload(
    _queue: &RenderQueue,
    _planes: (&wgpu::Texture, &wgpu::Texture),
    _y: *const c_void,
    _uv: *const c_void,
    _width: u32,
    _height: u32,
) {
}

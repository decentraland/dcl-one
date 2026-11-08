//! Imports the client's presentation texture into the renderer without a copy: on DX12 through
//! `ID3D12Device::OpenSharedHandle`, on Vulkan as external memory.

use std::{
    os::{
        raw::c_void,
        windows::io::{AsRawHandle as _, FromRawHandle as _, OwnedHandle},
    },
    sync::Once,
};

use bevy::{
    log::{debug, warn},
    render::{
        render_resource::{Extent3d, TextureDimension, TextureFormat, TextureUsages},
        renderer::RenderDevice,
    },
};
use windows::{
    Win32::{
        Foundation::HANDLE,
        Graphics::{
            Direct3D11::ID3D11Texture2D,
            Direct3D12::{ID3D12Device, ID3D12Resource},
            Dxgi::{DXGI_SHARED_RESOURCE_READ, DXGI_SHARED_RESOURCE_WRITE, IDXGIResource1},
        },
    },
    core::{Interface as _, PCWSTR},
};

use super::{
    InteropError,
    decode_device::{DecodeDevice, Renderer},
};
use crate::uuav::render::PlaneTextures;

/// Imports the presentation texture (`y` and `uv` are the same `ID3D11Texture2D`) into the
/// renderer. The client keeps it alive across frames and for one poll after a resolution change;
/// the import holds the shared memory on top.
pub fn make_planes(
    device: &RenderDevice,
    decode_device: &DecodeDevice,
    y: *const c_void,
    uv: *const c_void,
    width: u32,
    height: u32,
) -> Option<PlaneTextures> {
    if y.is_null() || y != uv {
        return None;
    }
    match import(device, &decode_device.renderer, y, width, height) {
        Ok(texture) => Some(PlaneTextures::nv12(texture)),
        Err(e) => {
            static ONCE: Once = Once::new();
            ONCE.call_once(|| {
                warn!("video frames cannot be imported into the renderer: {e}");
            });
            debug!("uuav: presentation texture not imported: {e}");
            None
        }
    }
}

fn import(
    device: &RenderDevice,
    renderer: &Renderer,
    ptr: *const c_void,
    width: u32,
    height: u32,
) -> Result<wgpu::Texture, InteropError> {
    if !device
        .features()
        .contains(wgpu::Features::TEXTURE_FORMAT_NV12)
    {
        return Err("the render device has no NV12 texture support".into());
    }
    let raw = ptr.cast_mut();
    // SAFETY: the client hands out a live `ID3D11Texture2D` it keeps alive until one poll after
    // it replaces it; `from_raw_borrowed` adds no reference and the cast below takes its own
    let texture =
        unsafe { ID3D11Texture2D::from_raw_borrowed(&raw) }.ok_or("null presentation texture")?;
    let resource: IDXGIResource1 = texture.cast().map_err(InteropError::caused(
        "the presentation texture is not an IDXGIResource1",
    ))?;
    let handle = unsafe {
        resource.CreateSharedHandle(
            None,
            DXGI_SHARED_RESOURCE_READ.0 | DXGI_SHARED_RESOURCE_WRITE.0,
            PCWSTR::null(),
        )
    }
    .map_err(InteropError::caused(
        "CreateSharedHandle failed (was the presentation texture created shareable?)",
    ))?;
    // SAFETY: the handle was just created and nothing else owns it. Neither import takes it
    // over: the opened resource keeps its own references, and the handle closes on return.
    let owned = unsafe { OwnedHandle::from_raw_handle(handle.0) };
    let handle = HANDLE(owned.as_raw_handle());
    let size = Extent3d {
        width,
        height,
        depth_or_array_layers: 1,
    };
    match renderer {
        Renderer::Dx12(d3d12) => open_dx12(device, d3d12, handle, size),
        Renderer::Vulkan => open_vulkan(device, handle, size),
    }
}

fn descriptor(size: Extent3d) -> wgpu::TextureDescriptor<'static> {
    wgpu::TextureDescriptor {
        label: Some("uuav presentation"),
        size,
        mip_level_count: 1,
        sample_count: 1,
        dimension: TextureDimension::D2,
        format: TextureFormat::NV12,
        usage: TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    }
}

fn open_dx12(
    device: &RenderDevice,
    d3d12: &ID3D12Device,
    handle: HANDLE,
    size: Extent3d,
) -> Result<wgpu::Texture, InteropError> {
    let mut resource: Option<ID3D12Resource> = None;
    unsafe { d3d12.OpenSharedHandle(handle, &mut resource) }.map_err(InteropError::caused(
        "ID3D12Device::OpenSharedHandle failed",
    ))?;
    let resource = resource.ok_or("OpenSharedHandle returned no resource")?;
    // SAFETY: the resource is a 2D single-mip NV12 texture of `size` opened on the renderer's
    // device, and the wrapper takes the reference `OpenSharedHandle` returned
    let hal = unsafe {
        wgpu::hal::dx12::Device::texture_from_raw(
            resource,
            TextureFormat::NV12,
            TextureDimension::D2,
            size,
            1,
            1,
        )
    };
    Ok(unsafe {
        device
            .wgpu_device()
            .create_texture_from_hal::<wgpu::hal::api::Dx12>(hal, &descriptor(size))
    })
}

fn open_vulkan(
    device: &RenderDevice,
    handle: HANDLE,
    size: Extent3d,
) -> Result<wgpu::Texture, InteropError> {
    let hal = unsafe {
        device
            .wgpu_device()
            .as_hal::<wgpu::hal::api::Vulkan, _, _>(|hal| {
                let hal = hal.ok_or("the renderer is not on Vulkan")?;
                hal.texture_from_d3d11_shared_handle(
                    handle,
                    &wgpu::hal::TextureDescriptor {
                        label: Some("uuav presentation"),
                        size,
                        mip_level_count: 1,
                        sample_count: 1,
                        dimension: TextureDimension::D2,
                        format: TextureFormat::NV12,
                        usage: wgpu::hal::TextureUses::RESOURCE,
                        memory_flags: wgpu::hal::MemoryFlags::empty(),
                        view_formats: vec![],
                    },
                )
                .map_err(InteropError::caused(
                    "Vulkan import of the D3D11 texture failed",
                ))
            })
    }?;
    Ok(unsafe {
        device
            .wgpu_device()
            .create_texture_from_hal::<wgpu::hal::api::Vulkan>(hal, &descriptor(size))
    })
}

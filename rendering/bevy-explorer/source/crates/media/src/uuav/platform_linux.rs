//! Linux: the client presents each frame into two tightly packed CPU planes (plane 0 = Y 8-bit
//! at w x h, plane 1 = interleaved CbCr 8-bit at w/2 x h/2); they are uploaded into two owned
//! textures every frame.

use std::os::raw::c_void;

use bevy::render::{
    render_resource::{
        Extent3d, TexelCopyBufferLayout, TexelCopyTextureInfo, TextureAspect, TextureDescriptor,
        TextureDimension, TextureFormat, TextureUsages,
    },
    renderer::{RenderAdapter, RenderDevice, RenderQueue},
};

/// Frames are presented on this platform.
pub const PRESENTS_FRAMES: bool = true;

/// Nothing is derived from a host GPU object on Linux; the client accepts a null probe.
pub struct Probe;

impl Probe {
    pub fn ptr(&self) -> *const c_void {
        std::ptr::null()
    }
}

pub fn probe(_device: &RenderDevice, _adapter: &RenderAdapter) -> Result<Probe, String> {
    Ok(Probe)
}

pub fn make_planes(
    device: &RenderDevice,
    y: *const c_void,
    uv: *const c_void,
    width: u32,
    height: u32,
) -> Option<(wgpu::Texture, wgpu::Texture)> {
    if y.is_null() || uv.is_null() {
        return None;
    }
    Some((
        create(device, TextureFormat::R8Unorm, width, height),
        create(device, TextureFormat::Rg8Unorm, width / 2, height / 2),
    ))
}

fn create(device: &RenderDevice, format: TextureFormat, width: u32, height: u32) -> wgpu::Texture {
    device.wgpu_device().create_texture(&TextureDescriptor {
        label: Some("uuav plane"),
        size: Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: TextureDimension::D2,
        format,
        usage: TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST,
        view_formats: &[],
    })
}

/// Copies the client's planes into the textures; `width` and `height` are the planes' own size,
/// read together with the pointers. Both planes are `width` bytes per row: the CbCr plane is
/// half as wide with two bytes per texel.
pub fn upload(
    queue: &RenderQueue,
    planes: (&wgpu::Texture, &wgpu::Texture),
    y: *const c_void,
    uv: *const c_void,
    width: u32,
    height: u32,
) {
    if y.is_null() || uv.is_null() {
        return;
    }
    let row = width as usize;
    // SAFETY: the client created the planes for this size, `height` rows of `width` bytes
    // behind the Y pointer and half as many behind the CbCr pointer, and keeps them alive until
    // one poll after it hands out new ones; the player is freed only after this system ran
    let y_bytes = unsafe { std::slice::from_raw_parts(y.cast::<u8>(), row * height as usize) };
    let uv_bytes =
        unsafe { std::slice::from_raw_parts(uv.cast::<u8>(), row * (height / 2) as usize) };
    write(queue, planes.0, y_bytes, width, width, height);
    write(queue, planes.1, uv_bytes, width, width / 2, height / 2);
}

fn write(
    queue: &RenderQueue,
    texture: &wgpu::Texture,
    bytes: &[u8],
    bytes_per_row: u32,
    width: u32,
    height: u32,
) {
    queue.write_texture(
        TexelCopyTextureInfo {
            texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: TextureAspect::All,
        },
        bytes,
        TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(bytes_per_row),
            rows_per_image: Some(height),
        },
        Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
    );
}

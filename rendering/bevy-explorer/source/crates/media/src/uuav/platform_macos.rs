//! Metal: the client presents each frame into two `MTLTexture`s on the renderer's device (plane
//! 0 = Y `R8Unorm`, plane 1 = CbCr `RG8Unorm`); they are wrapped as wgpu textures, no copy.

use std::os::raw::c_void;

use bevy::render::{
    render_resource::{Extent3d, TextureDimension, TextureFormat, TextureUsages},
    renderer::RenderDevice,
};
use metal::foreign_types::{ForeignType, ForeignTypeRef};

use super::render::PlaneTextures;

/// A live `id<MTLTexture>` on the renderer's device: `uuav_init` derives the device from it and
/// retains the device itself, so the probe only has to outlive the call.
pub struct Probe {
    texture: metal::Texture,
}

impl Probe {
    /// `Err` when the renderer is not on Metal or its device is busy on the render thread this
    /// frame.
    pub fn new(device: &RenderDevice) -> Result<Self, &'static str> {
        let texture = unsafe {
            device
                .wgpu_device()
                .as_hal::<wgpu::hal::api::Metal, _, _>(|hal| {
                    let hal = hal.ok_or("the renderer is not on Metal")?;
                    // the renderer holds the device lock while it submits; refused for this frame
                    let raw = hal
                        .raw_device()
                        .try_lock()
                        .ok_or("the Metal device is busy")?;
                    let descriptor = metal::TextureDescriptor::new();
                    descriptor.set_pixel_format(metal::MTLPixelFormat::RGBA8Unorm);
                    descriptor.set_width(1);
                    descriptor.set_height(1);
                    Ok::<_, &str>(raw.new_texture(&descriptor))
                })
        }?;
        Ok(Self { texture })
    }

    pub fn ptr(&self) -> *const c_void {
        self.texture.as_ptr().cast()
    }
}

/// Wraps the two presentation planes. The client keeps them alive across frames and for one
/// poll after a resolution change; the wrappers hold their own retain on top.
pub fn make_planes(
    device: &RenderDevice,
    y: *const c_void,
    uv: *const c_void,
    width: u32,
    height: u32,
) -> Option<PlaneTextures> {
    Some(PlaneTextures::pair(
        wgpu_texture_from_metal(device, y, TextureFormat::R8Unorm, width, height)?,
        wgpu_texture_from_metal(device, uv, TextureFormat::Rg8Unorm, width / 2, height / 2)?,
    ))
}

/// A wgpu texture over the client's `id<MTLTexture>` itself, sharing its memory; `None` for a
/// null texture.
fn wgpu_texture_from_metal(
    device: &RenderDevice,
    metal_texture: *const c_void,
    format: TextureFormat,
    width: u32,
    height: u32,
) -> Option<wgpu::Texture> {
    if metal_texture.is_null() {
        return None;
    }
    // `to_owned` retains: the client owns the +1 the pointer carries
    let raw = unsafe {
        metal::TextureRef::from_ptr(metal_texture.cast_mut().cast::<metal::MTLTexture>())
    }
    .to_owned();
    let hal = unsafe {
        wgpu::hal::metal::Device::texture_from_raw(
            raw,
            format,
            metal::MTLTextureType::D2,
            1,
            1,
            wgpu::hal::CopyExtent {
                width,
                height,
                depth: 1,
            },
        )
    };
    let texture = unsafe {
        device
            .wgpu_device()
            .create_texture_from_hal::<wgpu::hal::api::Metal>(
                hal,
                &wgpu::TextureDescriptor {
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
                    usage: TextureUsages::TEXTURE_BINDING,
                    view_formats: &[],
                },
            )
    };
    Some(texture)
}

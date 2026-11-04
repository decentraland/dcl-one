//! The render-world half: each frame, every video player's latest presented NV12 planes are
//! converted into its target image once the main world has sized the image to the planes (it
//! reads the size from [`super::Target`] and resizes the asset, which rebinds the materials;
//! this side never replaces the image's texture or uploads pixels through the main world).
//! Players the main world closed are freed here, once their planes are no longer read.

use std::{collections::HashMap, sync::Arc};

use bevy::{
    prelude::*,
    render::{
        Render, RenderSet,
        render_asset::{RenderAssets, prepare_assets},
        render_resource::{TextureFormat, TextureId, TextureUsages},
        renderer::{RenderDevice, RenderQueue},
        texture::GpuImage,
    },
};
use common::structs::SnapshotInProgress;

use super::{Shared, ffi, ffi::PlayerId, platform};

/// Pixels a player's planes may hold: DCI 4K and a little more. A larger stream is refused
/// before it reaches the device.
const MAX_PIXELS: u64 = 4096 * 4096;
/// The smallest edge a video may have: what the `av` sink keeps its image at, so a smaller video
/// would never match the image and never be drawn.
const MIN_EDGE: u32 = 16;

/// The player table the main world maintains ([`super::UuavRuntime`]), read here.
#[derive(Resource)]
struct UuavRenderShared(Arc<Shared>);

pub fn build(render_app: &mut SubApp, shared: Arc<Shared>) {
    render_app
        .insert_resource(UuavRenderShared(shared))
        .init_resource::<PlayerPlanes>()
        .add_systems(
            Render,
            // before the gpu images are (re)prepared, so a material rebind the same frame
            // binds the texture the conversion resized
            present_uuav_frames
                .in_set(RenderSet::PrepareAssets)
                .before(prepare_assets::<GpuImage>),
        );
}

/// Runs once the render device exists; the main world learns the device's texture edge limit
/// through [`Shared`] here.
pub fn finish(render_app: &mut SubApp) {
    if let Some(device) = render_app.world().get_resource::<RenderDevice>() {
        let max_dimension = device.limits().max_texture_dimension_2d;
        render_app
            .world()
            .resource::<UuavRenderShared>()
            .0
            .max_dimension
            .store(max_dimension, std::sync::atomic::Ordering::Release);
        render_app.init_resource::<Nv12Pipeline>();
    } else {
        warn!("no render device: uuav video frames will not be presented");
    }
}

#[derive(Resource)]
struct Nv12Pipeline {
    shader: wgpu::ShaderModule,
    bind_group_layout: wgpu::BindGroupLayout,
    layout: wgpu::PipelineLayout,
    sampler: wgpu::Sampler,
    /// One pipeline per target image format seen.
    pipelines: HashMap<TextureFormat, wgpu::RenderPipeline>,
}

impl FromWorld for Nv12Pipeline {
    fn from_world(world: &mut World) -> Self {
        let device = world.resource::<RenderDevice>().wgpu_device();
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("uuav nv12"),
            source: wgpu::ShaderSource::Wgsl(include_str!("nv12.wgsl").into()),
        });
        let texture = wgpu::BindingType::Texture {
            sample_type: wgpu::TextureSampleType::Float { filterable: true },
            view_dimension: wgpu::TextureViewDimension::D2,
            multisampled: false,
        };
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("uuav nv12"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: texture,
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: texture,
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("uuav nv12"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("uuav nv12"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        Self {
            shader,
            bind_group_layout,
            layout,
            sampler,
            pipelines: HashMap::new(),
        }
    }
}

impl Nv12Pipeline {
    fn pipeline(&mut self, device: &wgpu::Device, format: TextureFormat) -> &wgpu::RenderPipeline {
        self.pipelines.entry(format).or_insert_with(|| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("uuav nv12"),
                layout: Some(&self.layout),
                vertex: wgpu::VertexState {
                    module: &self.shader,
                    entry_point: Some("vs_main"),
                    compilation_options: Default::default(),
                    buffers: &[],
                },
                primitive: Default::default(),
                depth_stencil: None,
                multisample: Default::default(),
                fragment: Some(wgpu::FragmentState {
                    module: &self.shader,
                    entry_point: Some("fs_main"),
                    compilation_options: Default::default(),
                    targets: &[Some(wgpu::ColorTargetState {
                        format,
                        blend: None,
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                }),
                multiview: None,
                cache: None,
            })
        })
    }

    fn bind_group(
        &self,
        device: &wgpu::Device,
        planes: &(wgpu::Texture, wgpu::Texture),
    ) -> wgpu::BindGroup {
        let y = planes.0.create_view(&Default::default());
        let uv = planes.1.create_view(&Default::default());
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("uuav nv12"),
            layout: &self.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&y),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&uv),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
            ],
        })
    }
}

/// A player's wrapped (or uploaded-into) planes, rebuilt when the client hands out new plane
/// pointers or a new size. The pointers are kept as addresses: they are only compared.
struct Planes {
    y: usize,
    uv: usize,
    width: u32,
    height: u32,
    textures: (wgpu::Texture, wgpu::Texture),
    bind_group: wgpu::BindGroup,
    /// The client's frame count at the last upload; `None` before the first.
    frames: Option<u64>,
    /// The target texture the last conversion drew into: a new one needs the frame again.
    drawn_into: Option<TextureId>,
}

impl Planes {
    fn holds(&self, presented: &ffi::Presented) -> bool {
        self.y == presented.y as usize
            && self.uv == presented.uv as usize
            && self.width == presented.width
            && self.height == presented.height
    }
}

#[derive(Resource, Default)]
struct PlayerPlanes(HashMap<PlayerId, Planes>);

/// Why a video of this size cannot be shown: an edge under [`MIN_EDGE`], over the device's
/// `max_dimension` (0 for a device not known yet: no edge limit), or more than [`MAX_PIXELS`].
pub(super) fn check_bounds(width: u32, height: u32, max_dimension: u32) -> Result<(), String> {
    if width < MIN_EDGE || height < MIN_EDGE {
        return Err(format!("{width}x{height} is under {MIN_EDGE} px"));
    }
    if max_dimension != 0 && (width > max_dimension || height > max_dimension) {
        return Err(format!(
            "{width}x{height} exceeds the device's {max_dimension} texture edge"
        ));
    }
    if u64::from(width) * u64::from(height) > MAX_PIXELS {
        return Err(format!(
            "{width}x{height} exceeds the {MAX_PIXELS} pixel cap"
        ));
    }
    Ok(())
}

/// Why planes of this size cannot become textures: out of [`check_bounds`], or not an NV12
/// size.
fn check_size(width: u32, height: u32, max_dimension: u32) -> Result<(), String> {
    check_bounds(width, height, max_dimension)?;
    if !width.is_multiple_of(2) || !height.is_multiple_of(2) {
        return Err(format!("{width}x{height} is not an NV12 size"));
    }
    Ok(())
}

fn present_uuav_frames(
    shared: Res<UuavRenderShared>,
    pipeline: Option<ResMut<Nv12Pipeline>>,
    mut planes: ResMut<PlayerPlanes>,
    mut images: ResMut<RenderAssets<GpuImage>>,
    render_device: Res<RenderDevice>,
    render_queue: Res<RenderQueue>,
    snapshot: Option<Res<SnapshotInProgress>>,
) {
    // Players the main world closed: their textures go first, then the client may release the
    // planes behind them. Nothing below reads a retired player.
    for id in shared.0.take_retired() {
        planes.0.remove(&id);
        uuav::uuav_player_free(id);
    }

    let Some(mut pipeline) = pipeline else {
        return;
    };
    if !platform::PRESENTS_FRAMES {
        return;
    }
    // A capture holds the frame it was requested on until it is read back.
    if snapshot.is_some_and(|snapshot| snapshot.0) {
        return;
    }

    let targets: Vec<_> = shared
        .0
        .targets
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .iter()
        .map(|(id, target)| (*id, target.clone()))
        .collect();

    let device = render_device.wgpu_device();
    let max_dimension = render_device.limits().max_texture_dimension_2d;
    let render_callback = uuav::uuav_get_render_callback();

    for (id, target) in targets {
        if target.failed.load(std::sync::atomic::Ordering::Acquire) {
            continue;
        }
        let Ok(event_id) = i32::try_from(id) else {
            continue;
        };
        // moves the helper's latest published frame into the presentation planes
        render_callback(event_id);

        let Some(presented) = ffi::presented(id) else {
            continue;
        };
        if let Err(e) = check_size(presented.width, presented.height, max_dimension) {
            warn!("uuav player {id}: frames not presented: {e}");
            target
                .failed
                .store(true, std::sync::atomic::Ordering::Release);
            planes.0.remove(&id);
            continue;
        }

        target.presented_size.store(
            super::pack_size(presented.width, presented.height),
            std::sync::atomic::Ordering::Release,
        );

        let Some(gpu_image) = images.get_mut(target.image) else {
            trace!("uuav player {id}: no target image yet");
            continue;
        };
        let size = wgpu::Extent3d {
            width: presented.width,
            height: presented.height,
            depth_or_array_layers: 1,
        };
        // the main world sizes the image from `Target::presented_size`; a texture of another
        // size is the one the materials are bound to and this pass must not swap it
        if gpu_image.size != size
            || !gpu_image
                .texture_descriptor
                .usage
                .contains(TextureUsages::RENDER_ATTACHMENT)
        {
            trace!(
                "uuav player {id}: target image {:?} is not the presented {size:?} yet",
                gpu_image.size
            );
            continue;
        }

        if !planes
            .0
            .get(&id)
            .is_some_and(|planes| planes.holds(&presented))
        {
            let Some(textures) = platform::make_planes(
                &render_device,
                presented.y,
                presented.uv,
                presented.width,
                presented.height,
            ) else {
                continue;
            };
            let bind_group = pipeline.bind_group(device, &textures);
            planes.0.insert(
                id,
                Planes {
                    y: presented.y as usize,
                    uv: presented.uv as usize,
                    width: presented.width,
                    height: presented.height,
                    textures,
                    bind_group,
                    frames: None,
                    drawn_into: None,
                },
            );
        }
        let Some(planes) = planes.0.get_mut(&id) else {
            continue;
        };

        // nothing new to show and the target still holds the last conversion
        let target_texture = gpu_image.texture.id();
        if planes.frames == Some(presented.frames) && planes.drawn_into == Some(target_texture) {
            continue;
        }
        if planes.frames != Some(presented.frames) {
            platform::upload(
                &render_queue,
                (&planes.textures.0, &planes.textures.1),
                presented.y,
                presented.uv,
                presented.width,
                presented.height,
            );
            planes.frames = Some(presented.frames);
        }

        let render_pipeline = pipeline.pipeline(device, gpu_image.texture_format);
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("uuav nv12"),
        });
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("uuav nv12"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &gpu_image.texture_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            pass.set_pipeline(render_pipeline);
            pass.set_bind_group(0, &planes.bind_group, &[]);
            pass.draw(0..3, 0..1);
        }
        render_queue.submit(std::iter::once(encoder.finish()));
        planes.drawn_into = Some(target_texture);
        target
            .presented
            .fetch_add(1, std::sync::atomic::Ordering::Release);
    }
}

#[cfg(test)]
mod tests {
    use super::{check_bounds, check_size};

    #[test]
    fn an_nv12_size_within_the_limits_passes() {
        assert_eq!(check_size(852, 480, 8192), Ok(()));
        assert_eq!(check_size(4096, 4096, 8192), Ok(()));
        assert_eq!(check_size(16, 16, 8192), Ok(()));
    }

    #[test]
    fn an_odd_size_is_refused_as_planes_but_passes_the_bounds() {
        assert!(check_size(853, 480, 8192).is_err());
        assert!(check_size(852, 481, 8192).is_err());
        assert_eq!(check_bounds(853, 480, 8192), Ok(()));
    }

    #[test]
    fn an_edge_under_16_px_is_refused() {
        assert!(
            check_bounds(8, 8, 8192)
                .unwrap_err()
                .contains("under 16 px")
        );
        assert!(check_bounds(1920, 8, 8192).is_err());
        assert!(check_size(0, 480, 8192).is_err());
    }

    #[test]
    fn an_unknown_device_limit_keeps_the_pixel_cap() {
        assert_eq!(check_bounds(4096, 4096, 0), Ok(()));
        assert!(
            check_bounds(8192, 4096, 0)
                .unwrap_err()
                .contains("pixel cap")
        );
    }

    #[test]
    fn a_size_past_the_device_or_the_pixel_cap_is_refused() {
        assert!(
            check_size(8194, 16, 8192)
                .unwrap_err()
                .contains("texture edge")
        );
        assert!(
            check_size(8192, 4096, 8192)
                .unwrap_err()
                .contains("pixel cap")
        );
    }
}

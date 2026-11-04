//! The render-world half: each frame, every video player's latest presented NV12 planes are
//! converted into its target image, sized to the video (the main world's image only follows
//! suit through `VideoData::Info`, it never uploads a video image itself).

use std::{collections::HashMap, sync::Arc};

use bevy::{
    prelude::*,
    render::{
        Render, RenderSet,
        render_asset::{RenderAssets, prepare_assets},
        render_resource::{TextureFormat, TextureUsages},
        renderer::{RenderDevice, RenderQueue},
        texture::GpuImage,
    },
};
use common::structs::SnapshotInProgress;

use super::{Shared, ffi, ffi::PlayerId, platform};

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

/// Runs once the render device exists.
pub fn finish(render_app: &mut SubApp) {
    if render_app.world().contains_resource::<RenderDevice>() {
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
}

#[derive(Resource, Default)]
struct PlayerPlanes(HashMap<PlayerId, Planes>);

fn present_uuav_frames(
    shared: Res<UuavRenderShared>,
    pipeline: Option<ResMut<Nv12Pipeline>>,
    mut planes: ResMut<PlayerPlanes>,
    mut images: ResMut<RenderAssets<GpuImage>>,
    render_device: Res<RenderDevice>,
    render_queue: Res<RenderQueue>,
    snapshot: Option<Res<SnapshotInProgress>>,
) {
    let Some(mut pipeline) = pipeline else {
        return;
    };
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
    planes
        .0
        .retain(|id, _| targets.iter().any(|(target, _)| target == id));

    let device = render_device.wgpu_device();
    let render_callback = uuav::uuav_get_render_callback();

    for (id, target) in targets {
        let Ok(event_id) = i32::try_from(id) else {
            continue;
        };
        // moves the helper's latest published frame into the presentation planes
        render_callback(event_id);

        let Some((width, height)) = ffi::video_size(id) else {
            continue;
        };
        let (Some(y), Some(uv)) = (ffi::video_texture(id, 0), ffi::video_texture(id, 1)) else {
            continue;
        };
        if width < 2 || height < 2 {
            continue;
        }

        let Some(gpu_image) = images.get_mut(target.image) else {
            trace!("uuav player {id}: no target image yet");
            continue;
        };

        let current = planes.0.get(&id).filter(|planes| {
            planes.y == y as usize
                && planes.uv == uv as usize
                && planes.width == width
                && planes.height == height
        });
        let planes = match current {
            Some(_) => planes.0.get(&id).unwrap(),
            None => {
                let Some(textures) = platform::make_planes(&render_device, y, uv, width, height)
                else {
                    continue;
                };
                let bind_group = pipeline.bind_group(device, &textures);
                planes.0.insert(
                    id,
                    Planes {
                        y: y as usize,
                        uv: uv as usize,
                        width,
                        height,
                        textures,
                        bind_group,
                    },
                );
                planes.0.get(&id).unwrap()
            }
        };
        platform::upload(
            &render_queue,
            (&planes.textures.0, &planes.textures.1),
            y,
            uv,
            width,
            height,
        );

        let size = wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        };
        if gpu_image.size != size
            || !gpu_image
                .texture_descriptor
                .usage
                .contains(TextureUsages::RENDER_ATTACHMENT)
        {
            debug!("uuav player {id}: resize {:?} -> {size:?}", gpu_image.size);
            let mut descriptor = gpu_image.texture_descriptor.clone();
            descriptor.size = size;
            descriptor.usage |= TextureUsages::COPY_DST | TextureUsages::RENDER_ATTACHMENT;
            let texture = render_device.create_texture(&descriptor);
            gpu_image.texture_view = match &gpu_image.texture_view_descriptor {
                Some(view_descriptor) => texture.create_view(view_descriptor),
                None => texture.create_view(&Default::default()),
            };
            gpu_image.texture = texture;
            gpu_image.texture_descriptor = descriptor;
            gpu_image.size = size;
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
        target
            .presented
            .fetch_add(1, std::sync::atomic::Ordering::Release);
    }
}

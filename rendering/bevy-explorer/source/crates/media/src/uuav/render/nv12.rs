//! The NV12 conversion: the pipeline that draws a player's Y and CbCr planes into its target
//! image, and the plane textures it samples.

use std::collections::HashMap;

use bevy::{
    prelude::*,
    render::{render_resource::TextureFormat, renderer::RenderDevice},
};

#[derive(Resource)]
pub(super) struct Nv12Pipeline {
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
    /// The conversion pipeline for a target format, created on first use.
    pub(super) fn get_or_create_pipeline(
        &mut self,
        device: &wgpu::Device,
        format: TextureFormat,
    ) -> &wgpu::RenderPipeline {
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

    pub(super) fn new_bind_group(
        &self,
        device: &wgpu::Device,
        planes: &PlaneTextures,
    ) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("uuav nv12"),
            layout: &self.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&planes.y),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&planes.uv),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
            ],
        })
    }
}

/// The two plane views the conversion samples (Y as `R8Unorm`, CbCr as `Rg8Unorm`) and the
/// textures behind them: two (one per plane) or one `NV12` texture viewed per plane.
pub(crate) struct PlaneTextures {
    pub y: wgpu::TextureView,
    pub uv: wgpu::TextureView,
    /// The Y and CbCr textures behind the views, which the upload writes into.
    #[cfg(target_os = "linux")]
    textures: [wgpu::Texture; 2],
}

impl PlaneTextures {
    /// One texture per plane.
    #[cfg(not(windows))]
    pub fn pair(y: wgpu::Texture, uv: wgpu::Texture) -> Self {
        Self {
            y: y.create_view(&Default::default()),
            uv: uv.create_view(&Default::default()),
            #[cfg(target_os = "linux")]
            textures: [y, uv],
        }
    }

    /// One `NV12` texture, each plane viewed in its own format.
    #[cfg(windows)]
    pub fn nv12(texture: wgpu::Texture) -> Self {
        let plane = |aspect, format| {
            texture.create_view(&wgpu::TextureViewDescriptor {
                label: Some("uuav plane"),
                format: Some(format),
                aspect,
                ..Default::default()
            })
        };
        Self {
            y: plane(wgpu::TextureAspect::Plane0, TextureFormat::R8Unorm),
            uv: plane(wgpu::TextureAspect::Plane1, TextureFormat::Rg8Unorm),
        }
    }

    #[cfg(target_os = "linux")]
    pub fn textures(&self) -> &[wgpu::Texture; 2] {
        &self.textures
    }
}

//! The render-world half: each frame, every video player's latest presented NV12 planes are
//! converted into its target image once the image has the planes' size (this side publishes
//! the size through [`Target`](super::presentation::Target); it never replaces the image's texture or uploads pixels
//! through the main world).
//! Players the main world closed are freed here, once their planes are no longer read.

use std::{
    collections::HashMap,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

use bevy::{
    prelude::*,
    render::{
        Render, RenderSet,
        render_asset::{RenderAssets, prepare_assets},
        render_resource::{TextureId, TextureUsages},
        renderer::{RenderDevice, RenderQueue},
        texture::GpuImage,
    },
};

mod nv12;

pub(super) use nv12::PlaneTextures;

use super::{
    ffi,
    ffi::PlayerId,
    platform,
    presentation::{Target, VideoPresentation},
    size::{check_size, pack_size},
};
use nv12::Nv12Pipeline;

/// Whether a player's next frame waits until the conversion that read its planes has executed.
/// Only the Windows platform module leaves the copy into the planes unordered against the read,
/// and nothing on the GPU stops the next copy from overwriting planes still being read; a fence
/// the client's device waits on would replace this gate (#438).
const GATE_COPY_ON_READ: bool = cfg!(windows);

/// The main world's [`VideoPresentation`], read here.
#[derive(Resource)]
struct Presentation(Arc<VideoPresentation>);

pub fn build(render_app: &mut SubApp, presentation: Arc<VideoPresentation>) {
    render_app
        .insert_resource(Presentation(presentation))
        .init_resource::<ConversionSources>()
        .add_systems(
            Render,
            // before the gpu images are (re)prepared, so a material rebind the same frame
            // binds the texture the conversion resized
            present_uuav_frames
                .in_set(RenderSet::PrepareAssets)
                .before(prepare_assets::<GpuImage>),
        );
}

/// Runs once the render device exists; publishes the device's texture edge limit through
/// [`VideoPresentation`].
pub fn finish(render_app: &mut SubApp) {
    if let Some(device) = render_app.world().get_resource::<RenderDevice>() {
        let max_dimension = device.limits().max_texture_dimension_2d;
        render_app
            .world()
            .resource::<Presentation>()
            .0
            .max_dimension
            .store(max_dimension, std::sync::atomic::Ordering::Release);
        render_app.init_resource::<Nv12Pipeline>();
    } else {
        warn!("no render device: uuav video frames will not be presented");
    }
}

/// What the NV12 conversion reads for one player: the client's presentation planes as wgpu
/// textures, rebuilt when the client hands out new plane pointers or a new size. The pointers
/// are kept as addresses: they are only compared.
struct ConversionSource {
    y: usize,
    uv: usize,
    width: u32,
    height: u32,
    #[cfg(target_os = "linux")]
    textures: PlaneTextures,
    bind_group: wgpu::BindGroup,
    /// The client's frame count at the last upload; `None` before the first.
    frames: Option<u64>,
    /// The target texture the last conversion drew into: a new one needs the frame again.
    drawn_into: Option<TextureId>,
    /// Whether the last conversion submitted has executed; set by the queue's completion
    /// callback. Read before the next copy when [`GATE_COPY_ON_READ`].
    read_done: Arc<AtomicBool>,
}

impl ConversionSource {
    fn holds(&self, presented: &ffi::Presented) -> bool {
        self.y == presented.y as usize
            && self.uv == presented.uv as usize
            && self.width == presented.width
            && self.height == presented.height
    }
}

#[derive(Resource, Default)]
struct ConversionSources(HashMap<PlayerId, ConversionSource>);

fn present_uuav_frames(
    presentation: Res<Presentation>,
    pipeline: Option<ResMut<Nv12Pipeline>>,
    mut sources: ResMut<ConversionSources>,
    mut images: ResMut<RenderAssets<GpuImage>>,
    render_device: Res<RenderDevice>,
    render_queue: Res<RenderQueue>,
) {
    // Players the main world closed: their textures go first, then the client may release the
    // planes behind them. Nothing below reads a retired player.
    for id in presentation.0.take_retired() {
        sources.0.remove(&id);
        uuav::uuav_player_free(id);
    }

    let Some(mut pipeline) = pipeline else {
        return;
    };

    let targets: Vec<_> = presentation
        .0
        .targets
        .lock()
        .iter()
        .map(|(id, target)| (*id, target.clone()))
        .collect();

    let max_dimension = render_device.limits().max_texture_dimension_2d;
    if GATE_COPY_ON_READ {
        // runs the completion callbacks of the conversions that have executed since
        render_device.wgpu_device().poll(wgpu::Maintain::Poll);
    }

    for (id, target) in targets {
        if target.failed.load(Ordering::Acquire) {
            continue;
        }
        let Some(presented) = take_presented(id, &target, &mut sources, max_dimension) else {
            continue;
        };

        let Some(gpu_image) = images.get_mut(target.image) else {
            trace!("uuav player {id}: no target image yet");
            continue;
        };
        if !holds_presented_size(gpu_image, &presented) {
            trace!(
                "uuav player {id}: target image {:?} is not the presented {}x{} yet",
                gpu_image.size, presented.width, presented.height
            );
            continue;
        }
        let Some(source) = refresh_source(&mut sources, id, &presented, &render_device, &pipeline)
        else {
            continue;
        };

        // nothing new to show and the target still holds the last conversion
        let target_texture = gpu_image.texture.id();
        if source.frames == Some(presented.frames) && source.drawn_into == Some(target_texture) {
            continue;
        }
        if source.frames != Some(presented.frames) {
            #[cfg(target_os = "linux")]
            let in_planes = platform::upload(
                &render_queue,
                &source.textures,
                presented.y,
                presented.uv,
                presented.width,
                presented.height,
            );
            #[cfg(windows)]
            let in_planes = platform::wait_for_client_copy();
            // the planes are the client's textures, filled by `render_callback`
            #[cfg(target_os = "macos")]
            let in_planes = true;
            if !in_planes {
                // the frame is not in the planes yet; it is tried again next poll
                continue;
            }
            source.frames = Some(presented.frames);
        }

        convert(
            &mut pipeline,
            &render_device,
            &render_queue,
            gpu_image,
            source,
        );
        source.drawn_into = Some(target_texture);
        target.presented.fetch_add(1, Ordering::Release);
    }
}

/// The frame the player's planes hold after the copy, its size published through the target;
/// `None` when there is none, or its size cannot be presented (the player then fails).
fn take_presented(
    id: PlayerId,
    target: &Target,
    sources: &mut ConversionSources,
    max_dimension: u32,
) -> Option<ffi::Presented> {
    let presented = copy_frame(id, sources)?;
    if let Err(e) = check_size(presented.width, presented.height, max_dimension) {
        warn!("uuav player {id}: frames not presented: {e}");
        target.failed.store(true, Ordering::Release);
        sources.0.remove(&id);
        return None;
    }
    target.presented_size.store(
        pack_size(presented.width, presented.height),
        Ordering::Release,
    );
    Some(presented)
}

/// Moves the helper's latest published frame into the player's presentation planes and returns
/// what they hold; `None` while [`GATE_COPY_ON_READ`] holds the copy back.
fn copy_frame(id: PlayerId, sources: &ConversionSources) -> Option<ffi::Presented> {
    let event_id = i32::try_from(id).ok()?;
    if GATE_COPY_ON_READ
        && sources
            .0
            .get(&id)
            .is_some_and(|source| !source.read_done.load(Ordering::Acquire))
    {
        // the renderer may still be reading the planes the copy would overwrite
        return None;
    }
    uuav::uuav_get_render_callback()(event_id);
    ffi::presented(id)
}

/// Whether the target image is a render target of the planes' size. One of another size is the
/// texture the materials are bound to, and this pass must not swap it; the planes' size went
/// out through `Target::presented_size`.
fn holds_presented_size(gpu_image: &GpuImage, presented: &ffi::Presented) -> bool {
    gpu_image.size
        == wgpu::Extent3d {
            width: presented.width,
            height: presented.height,
            depth_or_array_layers: 1,
        }
        && gpu_image
            .texture_descriptor
            .usage
            .contains(TextureUsages::RENDER_ATTACHMENT)
}

/// The player's conversion source, rebuilt when the client handed out new planes.
fn refresh_source<'a>(
    sources: &'a mut ConversionSources,
    id: PlayerId,
    presented: &ffi::Presented,
    render_device: &RenderDevice,
    pipeline: &Nv12Pipeline,
) -> Option<&'a mut ConversionSource> {
    if !sources
        .0
        .get(&id)
        .is_some_and(|source| source.holds(presented))
    {
        let textures = platform::make_planes(
            render_device,
            presented.y,
            presented.uv,
            presented.width,
            presented.height,
        )?;
        let bind_group = pipeline.new_bind_group(render_device.wgpu_device(), &textures);
        sources.0.insert(
            id,
            ConversionSource {
                y: presented.y as usize,
                uv: presented.uv as usize,
                width: presented.width,
                height: presented.height,
                #[cfg(target_os = "linux")]
                textures,
                bind_group,
                frames: None,
                drawn_into: None,
                read_done: Arc::new(AtomicBool::new(true)),
            },
        );
    }
    sources.0.get_mut(&id)
}

/// Draws the player's planes into its target image.
fn convert(
    pipeline: &mut Nv12Pipeline,
    render_device: &RenderDevice,
    render_queue: &RenderQueue,
    gpu_image: &GpuImage,
    source: &ConversionSource,
) {
    let device = render_device.wgpu_device();
    let render_pipeline = pipeline.get_or_create_pipeline(device, gpu_image.texture_format);
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
        pass.set_bind_group(0, &source.bind_group, &[]);
        pass.draw(0..3, 0..1);
    }
    render_queue.submit(std::iter::once(encoder.finish()));
    if GATE_COPY_ON_READ {
        source.read_done.store(false, Ordering::Release);
        let read_done = source.read_done.clone();
        render_queue.on_submitted_work_done(move || read_done.store(true, Ordering::Release));
    }
}

//! The render-world half: each frame, every presented player's latest NV12 planes are converted
//! into its target image once the image has the planes' size. The planes' size goes to the main
//! world, which resizes the image; this side never replaces the image's texture or uploads
//! pixels through the main world. Players the main world frees are freed here, once their
//! planes are no longer read.

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
    link::{FromRender, RenderLink, ToRender},
    platform,
    size::check_size,
};
use nv12::Nv12Pipeline;

/// Whether a player's next frame waits until the conversion that read its planes has executed.
/// Only the Windows platform module leaves the copy into the planes unordered against the read,
/// and nothing on the GPU stops the next copy from overwriting planes still being read; a fence
/// the client's device waits on would replace this gate (#438).
const GATE_COPY_ON_READ: bool = cfg!(windows);

pub fn build(render_app: &mut SubApp, link: RenderLink) {
    render_app
        .insert_resource(link)
        .init_resource::<RenderPlayers>()
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
    fn new(
        presented: &ffi::Presented,
        textures: PlaneTextures,
        render_device: &RenderDevice,
        pipeline: &Nv12Pipeline,
    ) -> Self {
        Self {
            y: presented.y as usize,
            uv: presented.uv as usize,
            width: presented.width,
            height: presented.height,
            bind_group: pipeline.new_bind_group(render_device.wgpu_device(), &textures),
            #[cfg(target_os = "linux")]
            textures,
            frames: None,
            drawn_into: None,
            read_done: Arc::new(AtomicBool::new(true)),
        }
    }

    fn holds(&self, presented: &ffi::Presented) -> bool {
        (self.y, self.uv, self.width, self.height) == planes_key(presented)
    }
}

/// What the render world presents one player with.
struct RenderPlayer {
    image: AssetId<Image>,
    /// The planes' size last sent to the main world.
    size: Option<(u32, u32)>,
    /// `None` until the client hands out planes this side can read.
    source: Option<ConversionSource>,
    /// The planes that last failed to become textures, not tried again until the client hands
    /// out others.
    refused: Option<PlanesKey>,
}

/// Which planes the client handed out: their addresses and size.
type PlanesKey = (usize, usize, u32, u32);

fn planes_key(presented: &ffi::Presented) -> PlanesKey {
    (
        presented.y as usize,
        presented.uv as usize,
        presented.width,
        presented.height,
    )
}

/// The players the main world asked to present.
#[derive(Resource, Default)]
struct RenderPlayers {
    players: HashMap<PlayerId, RenderPlayer>,
    /// The decode device of the running runtime, which the planes come from.
    #[cfg(windows)]
    decode_device: Option<platform::DecodeDevice>,
}

fn present_uuav_frames(
    mut link: ResMut<RenderLink>,
    mut render_players: ResMut<RenderPlayers>,
    pipeline: Option<ResMut<Nv12Pipeline>>,
    mut images: ResMut<RenderAssets<GpuImage>>,
    render_device: Res<RenderDevice>,
    render_queue: Res<RenderQueue>,
) {
    let RenderPlayers {
        players,
        #[cfg(windows)]
        decode_device,
    } = &mut *render_players;
    while let Ok(message) = link.from_main.try_recv() {
        match message {
            ToRender::Present { id, image } => {
                players.insert(
                    id,
                    RenderPlayer {
                        image,
                        size: None,
                        source: None,
                        refused: None,
                    },
                );
            }
            ToRender::Stop(id) => {
                players.remove(&id);
            }
            ToRender::Free(id) => {
                // the player's textures go first, then the client may release the planes
                // behind them
                players.remove(&id);
                uuav::uuav_player_free(id);
            }
            #[cfg(windows)]
            ToRender::DecodeDevice(device) => *decode_device = Some(device),
        }
    }

    let Some(mut pipeline) = pipeline else {
        return;
    };
    let max_dimension = render_device.limits().max_texture_dimension_2d;
    if GATE_COPY_ON_READ {
        // runs the completion callbacks of the conversions that have executed since
        render_device.wgpu_device().poll(wgpu::Maintain::Poll);
    }

    // every player's copy goes first, so one fence wait covers them all
    let mut frames = Vec::new();
    let mut failed = Vec::new();
    for (&id, player) in players.iter_mut() {
        match take_presented(id, player, max_dimension) {
            Ok(Some(presented)) => {
                if player.size != Some((presented.width, presented.height)) {
                    player.size = Some((presented.width, presented.height));
                    _ = link.to_main.send(FromRender::Size {
                        id,
                        width: presented.width,
                        height: presented.height,
                    });
                }
                frames.push((id, presented));
            }
            Ok(None) => {}
            Err(e) => {
                warn!("uuav player {id}: frames not presented: {e}");
                failed.push(id);
            }
        }
    }
    for id in failed {
        players.remove(&id);
        _ = link.to_main.send(FromRender::Failed(id));
    }

    // whether the copies above have landed, waited for once and only when a frame needs it
    #[cfg(windows)]
    let mut copies_landed = None;
    for (id, presented) in frames {
        let Some(player) = players.get_mut(&id) else {
            continue;
        };
        let Some(gpu_image) = images.get_mut(player.image) else {
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
        if !player
            .source
            .as_ref()
            .is_some_and(|source| source.holds(&presented))
        {
            let key = planes_key(&presented);
            if player.refused == Some(key) {
                continue;
            }
            #[cfg(windows)]
            let Some(decode_device) = decode_device.as_ref() else {
                continue;
            };
            #[cfg(not(windows))]
            let textures = platform::make_planes(
                &render_device,
                presented.y,
                presented.uv,
                presented.width,
                presented.height,
            );
            #[cfg(windows)]
            let textures = platform::make_planes(
                &render_device,
                decode_device,
                presented.y,
                presented.uv,
                presented.width,
                presented.height,
            );
            // a failed import keeps the previous source and its read gate
            let Some(textures) = textures else {
                player.refused = Some(key);
                continue;
            };
            player.refused = None;
            player.source = Some(ConversionSource::new(
                &presented,
                textures,
                &render_device,
                &pipeline,
            ));
        }
        let Some(source) = &mut player.source else {
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
            let in_planes = *copies_landed.get_or_insert_with(|| {
                decode_device
                    .as_mut()
                    .is_some_and(platform::DecodeDevice::wait_for_client_copy)
            });
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
        _ = link.to_main.send(FromRender::Presented(id));
    }
}

/// The frame the player's planes hold after the copy; `Ok(None)` when there is none, `Err` when
/// its size cannot be presented.
fn take_presented(
    id: PlayerId,
    player: &RenderPlayer,
    max_dimension: u32,
) -> Result<Option<ffi::Presented>, String> {
    let Some(presented) = copy_frame(id, player.source.as_ref()) else {
        return Ok(None);
    };
    check_size(presented.width, presented.height, max_dimension)?;
    Ok(Some(presented))
}

/// Moves the helper's latest published frame into the player's presentation planes and returns
/// what they hold; `None` while [`GATE_COPY_ON_READ`] holds the copy back.
fn copy_frame(id: PlayerId, source: Option<&ConversionSource>) -> Option<ffi::Presented> {
    let event_id = i32::try_from(id).ok()?;
    if GATE_COPY_ON_READ && source.is_some_and(|source| !source.read_done.load(Ordering::Acquire)) {
        // the renderer may still be reading the planes the copy would overwrite
        return None;
    }
    uuav::uuav_get_render_callback()(event_id);
    ffi::presented(id)
}

/// Whether the target image is a render target of the planes' size. One of another size is the
/// texture the materials are bound to, and this pass must not swap it; the planes' size went
/// out to the main world.
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

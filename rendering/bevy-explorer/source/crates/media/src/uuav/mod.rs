//! Native media on the UUAV player (`library/uuav`): the sandboxed `uuav-helper` process
//! demuxes and decodes every scene video and audio stream (hardware first, software fallback)
//! and shares its frames with this process through the `uuav` client crate, which this module
//! drives as plain Rust calls. The engine side is the [`AVCommand`]s and [`VideoData`] the
//! `av` crate speaks ([`spawn_av`]): the main world polls each player's state and clock
//! ([`serve_uuav_players`]), kira pulls each player's audio through [`audio::UuavKiraDecoder`],
//! and the render world converts each player's presented NV12 planes into its target image
//! ([`render`]), so decoded pixels never cross the main world.

mod audio;
mod ffi;
#[cfg_attr(target_os = "macos", path = "platform_macos.rs")]
#[cfg_attr(target_os = "linux", path = "platform_linux.rs")]
#[cfg_attr(windows, path = "platform_windows/mod.rs")]
mod platform;
mod player;
mod presentation;
mod render;
mod runtime;
mod size;
mod source;

use std::sync::{Arc, atomic::Ordering};

#[cfg(windows)]
use bevy::render::renderer::RenderAdapter;
use bevy::{
    prelude::*,
    render::{RenderApp, renderer::RenderDevice},
};
use common::structs::AudioDecoderError;
use dcl_component::proto_components::sdk::components::VideoState;
use kira::sound::streaming::StreamingSoundData;
use parking_lot::Mutex;
use tokio::sync::{mpsc, oneshot, watch};

pub use audio::AudioHandle;
pub use source::resolve_source;

use crate::{AVCommand, VideoData};
use presentation::VideoPresentation;
use runtime::UuavRuntime;

pub struct UuavPlugin;

impl Plugin for UuavPlugin {
    fn build(&self, app: &mut App) {
        let presentation = Arc::new(VideoPresentation::default());
        app.insert_resource(UuavRuntime::new(presentation.clone()))
            .add_systems(PreUpdate, serve_uuav_players);
        if let Some(render_app) = app.get_sub_app_mut(RenderApp) {
            presentation.render_world.store(true, Ordering::Release);
            render::build(render_app, presentation);
        } else {
            warn!("no render app: uuav video frames will not be presented");
        }
    }

    fn finish(&self, app: &mut App) {
        if let Some(render_app) = app.get_sub_app_mut(RenderApp) {
            render::finish(render_app);
        }
    }
}

/// Stops the helper `UuavRuntime::initialize` started; nothing in this module may run afterwards.
pub fn uuav_shutdown() {
    uuav::uuav_deinit();
}

/// A source [`spawn_av`] took, waiting for the runtime's next look.
struct NewSource {
    commands: mpsc::UnboundedReceiver<AVCommand>,
    video: Option<mpsc::Sender<VideoData>>,
    state: watch::Sender<VideoState>,
    url: String,
    image: Option<AssetId<Image>>,
    audio: mpsc::Sender<StreamingSoundData<AudioDecoderError>>,
    audio_handle: oneshot::Receiver<AudioHandle>,
}

static NEW_SOURCES: Mutex<Vec<NewSource>> = Mutex::new(Vec::new());

/// Plays `url` on a uuav player, rendering its frames into `image`, driven by `commands`,
/// reporting its playback state on `state` and its frames on `video` (`None` for an audio
/// stream). The player's sound goes out on `audio`; the playing sound's handle, awaited on
/// `audio_handle`, is where the volume commands land.
pub fn spawn_av(
    commands: mpsc::UnboundedReceiver<AVCommand>,
    video: Option<mpsc::Sender<VideoData>>,
    state: watch::Sender<VideoState>,
    url: String,
    image: Option<&Handle<Image>>,
    audio: mpsc::Sender<StreamingSoundData<AudioDecoderError>>,
    audio_handle: oneshot::Receiver<AudioHandle>,
) {
    NEW_SOURCES.lock().push(NewSource {
        commands,
        video,
        state,
        url,
        image: image.map(Handle::id),
        audio,
        audio_handle,
    });
}

fn serve_uuav_players(
    mut runtime: ResMut<UuavRuntime>,
    device: Option<Res<RenderDevice>>,
    #[cfg(windows)] adapter: Option<Res<RenderAdapter>>,
) {
    runtime.initialize(
        device.as_deref(),
        #[cfg(windows)]
        adapter.as_deref(),
    );

    let new_sources = std::mem::take(&mut *NEW_SOURCES.lock());
    for source in new_sources {
        runtime.adopt(source, device.is_some());
    }
    runtime.poll_creating();

    let runtime = &mut *runtime;
    let presentation = &runtime.presentation;
    let max_dimension = presentation.max_dimension.load(Ordering::Acquire);
    runtime.players.retain_mut(|player| {
        let keep = player.serve(max_dimension);
        if !keep {
            presentation.retire(player.id);
        }
        keep
    });
}

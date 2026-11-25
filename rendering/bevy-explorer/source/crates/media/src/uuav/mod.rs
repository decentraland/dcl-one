//! Native media on the UUAV player (`library/uuav`): the sandboxed `uuav-helper` process
//! demuxes and decodes every scene video and audio stream (hardware first, software fallback)
//! and shares its frames with this process through the `uuav` client crate, which this module
//! drives as plain Rust calls. The engine side is the [`AVCommand`]s and [`VideoData`] the
//! `av` crate speaks through [`UuavPlayers::open`]: the main world serves each player's
//! commands, state and clock and plays its sound on kira ([`audio::UuavKiraDecoder`]); the
//! render world converts each player's presented NV12 planes into its target image
//! ([`render`]), so decoded pixels never cross the main world. The two worlds talk only through
//! the messages in [`link`].
//!
//! [`AVCommand`]: crate::AVCommand
//! [`VideoData`]: crate::VideoData

mod audio;
mod ffi;
mod link;
#[cfg_attr(target_os = "macos", path = "platform_macos.rs")]
#[cfg_attr(target_os = "linux", path = "platform_linux.rs")]
#[cfg_attr(windows, path = "platform_windows/mod.rs")]
mod platform;
mod player;
mod render;
mod runtime;
mod service;
mod size;
mod source;

#[cfg(windows)]
use bevy::render::renderer::RenderAdapter;
use bevy::{
    prelude::*,
    render::{RenderApp, renderer::RenderDevice},
};
use bevy_kira_audio::audio_output::AudioOutput;
use kira::manager::backend::DefaultBackend;

pub use service::UuavPlayers;
pub use source::resolve_source;

pub struct UuavPlugin;

impl Plugin for UuavPlugin {
    fn build(&self, app: &mut App) {
        let render_app = app.get_sub_app_mut(RenderApp);
        let (main_link, render_link) = link::link(render_app.is_some());
        match (render_app, render_link) {
            (Some(render_app), Some(render_link)) => render::build(render_app, render_link),
            _ => warn!("no render app: uuav video frames will not be presented"),
        }
        app.insert_resource(UuavPlayers::new(main_link))
            .add_systems(PreUpdate, serve_uuav_players);
    }

    fn finish(&self, app: &mut App) {
        if let Some(render_app) = app.get_sub_app_mut(RenderApp) {
            render::finish(render_app);
        }
    }
}

/// Stops the helper the runtime started; nothing in this module may run afterwards.
pub fn uuav_shutdown() {
    uuav::uuav_deinit();
}

fn serve_uuav_players(
    mut players: ResMut<UuavPlayers>,
    device: Option<Res<RenderDevice>>,
    #[cfg(windows)] adapter: Option<Res<RenderAdapter>>,
    mut audio: Option<NonSendMut<AudioOutput<DefaultBackend>>>,
) {
    players.serve(
        device.as_deref(),
        #[cfg(windows)]
        adapter.as_deref(),
        audio.as_mut().and_then(|audio| audio.manager.as_mut()),
    );
}

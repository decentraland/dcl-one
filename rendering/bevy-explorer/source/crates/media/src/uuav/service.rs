//! [`UuavPlayers`]: the app's uuav players, from the request to play a source until the player
//! is freed.

#[cfg(windows)]
use bevy::render::renderer::RenderAdapter;
use bevy::{prelude::*, render::renderer::RenderDevice};
use dcl_component::proto_components::sdk::components::VideoState;
use kira::{
    manager::{AudioManager, backend::DefaultBackend},
    sound::streaming::StreamingSoundData,
};
use tokio::sync::{mpsc, oneshot, watch};

use super::{
    audio::{AudioHandle, UuavKiraDecoder},
    ffi,
    ffi::PlayerId,
    link::{FromRender, MainLink},
    player::{Channels, Player, Presentation},
    runtime::{Admission, HelperRuntime},
};
use crate::{AVCommand, VideoData};

/// A source to play, as [`UuavPlayers::open`] took it.
struct Request {
    url: String,
    image: Option<AssetId<Image>>,
    channels: Channels,
}

/// A request whose player is being created on another thread (`uuav_player_new` round-trips
/// to the helper).
struct Creating {
    request: Request,
    done: oneshot::Receiver<Result<PlayerId, String>>,
}

/// The app's uuav players. [`open`](Self::open) asks for one; it lives until its command channel
/// delivers [`AVCommand::Dispose`] or closes, and is then freed.
#[derive(Resource, Default)]
pub struct UuavPlayers {
    runtime: HelperRuntime,
    /// Requests waiting for the runtime to come up.
    waiting: Vec<Request>,
    creating: Vec<Creating>,
    players: Vec<Player>,
    link: MainLink,
}

impl UuavPlayers {
    pub(super) fn new(link: MainLink) -> Self {
        Self {
            link,
            ..Default::default()
        }
    }

    /// Plays `url` on a new player, rendering its frames into `image`, driven by `commands` and
    /// reporting its playback state on `state` and its frames on `video` (`None` for an audio
    /// stream). Its sound plays on kira's default output.
    pub fn open(
        &mut self,
        url: String,
        image: Option<&Handle<Image>>,
        commands: mpsc::UnboundedReceiver<AVCommand>,
        video: Option<mpsc::Sender<VideoData>>,
        state: watch::Sender<VideoState>,
    ) {
        self.waiting.push(Request {
            url,
            image: image.map(Handle::id),
            channels: Channels {
                commands,
                video,
                state,
            },
        });
    }

    /// One frame of service: starts the runtime, creates the requested players and serves the
    /// live ones.
    pub(super) fn serve(
        &mut self,
        device: Option<&RenderDevice>,
        #[cfg(windows)] adapter: Option<&RenderAdapter>,
        mut audio: Option<&mut AudioManager<DefaultBackend>>,
    ) {
        self.runtime.initialize(
            device,
            #[cfg(windows)]
            adapter,
            #[cfg(windows)]
            &self.link,
        );

        for request in std::mem::take(&mut self.waiting) {
            self.create(request, device.is_some());
        }
        self.poll_creating(&mut audio);

        while let Some(message) = self.link.try_recv() {
            let id = match &message {
                FromRender::Size { id, .. }
                | FromRender::Presented(id)
                | FromRender::Failed(id) => *id,
            };
            if let Some(player) = self.players.iter_mut().find(|player| player.id == id) {
                player.on_render(message);
            }
        }

        // 0 before the device exists: no edge limit yet
        let max_dimension = device.map_or(0, |device| device.limits().max_texture_dimension_2d);
        let link = &self.link;
        for mut player in std::mem::take(&mut self.players) {
            if player.serve(link, max_dimension) {
                self.players.push(player);
            } else {
                player.close(link);
            }
        }
    }

    /// Begins creating a player for the request; [`poll_creating`](Self::poll_creating) starts
    /// it once the helper has answered.
    fn create(&mut self, request: Request, has_renderer: bool) {
        match self.runtime.admit(has_renderer, &request.url) {
            Admission::Open => {}
            Admission::Wait => {
                self.waiting.push(request);
                return;
            }
            Admission::Refuse => {
                debug!("uuav: no player runtime, `{}` will not play", request.url);
                request.channels.state.send_replace(VideoState::VsError);
                return;
            }
        }

        let (done_tx, done) = oneshot::channel();
        let spawned = std::thread::Builder::new()
            .name("uuav-player".to_owned())
            .spawn(move || {
                _ = done_tx.send(ffi::player_new());
            });
        match spawned {
            Ok(_) => self.creating.push(Creating { request, done }),
            Err(e) => {
                warn!("uuav: player for `{}` not created: {e}", request.url);
                request.channels.state.send_replace(VideoState::VsError);
            }
        }
    }

    fn poll_creating(&mut self, audio: &mut Option<&mut AudioManager<DefaultBackend>>) {
        for mut creating in std::mem::take(&mut self.creating) {
            let created = match creating.done.try_recv() {
                Ok(created) => created,
                Err(oneshot::error::TryRecvError::Empty) => {
                    self.creating.push(creating);
                    continue;
                }
                Err(oneshot::error::TryRecvError::Closed) => {
                    Err("the creation thread ended".to_owned())
                }
            };
            match created {
                Ok(id) => self.start(id, creating.request, audio.as_deref_mut()),
                Err(e) => {
                    warn!(
                        "uuav: player for `{}` not created: {e}",
                        creating.request.url
                    );
                    creating
                        .request
                        .channels
                        .state
                        .send_replace(VideoState::VsError);
                }
            }
        }
    }

    /// Opens the source on its new player, starts its sound and its presentation, and begins
    /// serving it.
    fn start(
        &mut self,
        id: PlayerId,
        request: Request,
        audio: Option<&mut AudioManager<DefaultBackend>>,
    ) {
        debug!("uuav player {id}: open `{}`", request.url);
        if let Err(e) = ffi::open_media(id, &request.url) {
            warn!("uuav player {id}: open `{}` failed: {e}", request.url);
            self.link.free(id, false);
            request.channels.state.send_replace(VideoState::VsError);
            return;
        }

        let sound = self.play_sound(id, audio);
        let presentation = request
            .channels
            .video
            .as_ref()
            .and(request.image)
            .map(|image| Presentation {
                render_world: self.link.present(id, image),
                ..Default::default()
            });
        self.players
            .push(Player::new(id, request.channels, sound, presentation));
    }

    fn play_sound(
        &self,
        id: PlayerId,
        audio: Option<&mut AudioManager<DefaultBackend>>,
    ) -> Option<AudioHandle> {
        let Some(audio) = audio else {
            debug!("uuav player {id}: no audio output, the sound does not play");
            return None;
        };
        let decoder = UuavKiraDecoder::new(id, self.runtime.sample_rate);
        match audio.play(StreamingSoundData::from_decoder(decoder)) {
            Ok(sound) => Some(sound),
            Err(e) => {
                warn!("uuav player {id}: sound not started: {e}");
                None
            }
        }
    }
}

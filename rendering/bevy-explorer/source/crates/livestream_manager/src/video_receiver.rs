use bevy::prelude::*;
use dcl_component::proto_components::sdk::components::VideoState;

/// An active receiver entity
#[derive(Component)]
#[require(ReceiverVolume)]
pub struct ActiveReceiver;

/// Audio volume of the receiver
#[derive(Component, Deref, DerefMut)]
pub struct ReceiverVolume(pub f32);

impl Default for ReceiverVolume {
    fn default() -> Self {
        Self(1.)
    }
}

/// Component with the handle to the image that the receiver
/// should read from
#[derive(Component)]
pub struct ReceiverImage {
    pub image: Handle<Image>,
    pub state: tokio::sync::watch::Receiver<VideoState>,
    pub sent_event: f64,
    pub sent_state: VideoState,
}

/// The scheme of every video source served from the scene room's LiveKit tracks.
const LIVEKIT_VIDEO_SCHEME: &str = "livekit-video://";

/// The automatic showcase: the scene room's highest-priority video track.
pub const LIVEKIT_CURRENT_STREAM: &str = "livekit-video://current-stream";

pub fn is_livekit_video(src: &str) -> bool {
    src.starts_with(LIVEKIT_VIDEO_SCHEME)
}

/// The source naming one track, as `getActiveVideoStreams` lists it.
pub fn livekit_track_url(
    identity: impl std::fmt::Display,
    track_sid: impl std::fmt::Display,
) -> String {
    format!("{LIVEKIT_VIDEO_SCHEME}{identity}/{track_sid}")
}

/// A published video track, scoped to the scene room that exposes it.
#[derive(Component, Clone, Debug, PartialEq, Eq)]
pub struct VideoTrackSource {
    pub scene_hash: String,
    pub identity: String,
    pub track_sid: String,
}

impl VideoTrackSource {
    /// The track a [`livekit_track_url`] names in the scene `scene_hash`.
    pub fn from_url(scene_hash: &str, url: &str) -> Option<Self> {
        let (identity, track_sid) = url.strip_prefix(LIVEKIT_VIDEO_SCHEME)?.split_once('/')?;
        (!identity.is_empty() && !track_sid.is_empty() && !track_sid.contains('/')).then(|| Self {
            scene_hash: scene_hash.to_owned(),
            identity: identity.to_owned(),
            track_sid: track_sid.to_owned(),
        })
    }
}

/// A receiver choosing a particular track instead of the automatic showcase.
#[derive(Component)]
pub struct SelectedVideoReceiver(pub VideoTrackSource);

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

/// A published video track, scoped to the scene room that exposes it.
#[derive(Component, Clone, Debug, PartialEq, Eq)]
pub struct VideoTrackSource {
    pub scene_hash: String,
    pub identity: String,
    pub track_sid: String,
}

/// A receiver choosing a particular track instead of the automatic showcase.
#[derive(Component)]
pub struct SelectedVideoReceiver(pub VideoTrackSource);

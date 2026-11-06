//! A `VideoPlayer` pinned to one participant's track (`livekit-video://{identity}/{sid}`),
//! instead of the stream the manager picks: the track it is bound to, the image that track
//! writes while it is not the active transmitter, and the facts a player needs about it.

use bevy::prelude::*;

/// A video track a participant publishes, as the scene API advertises it: the room's scene
/// hash, the participant's identity and the track's sid.
#[derive(Component, Clone, Debug, PartialEq, Eq)]
pub struct PublishedVideoTrack {
    pub room: String,
    pub identity: String,
    pub sid: String,
}

/// The track entity a pinned player shows; despawning either side removes the link.
#[derive(Component)]
#[relationship(relationship_target = PinnedBy)]
pub struct PinnedTo(pub Entity);

/// The players pinned to this track.
#[derive(Component)]
#[relationship_target(relationship = PinnedTo)]
pub struct PinnedBy(Vec<Entity>);

/// The image a pinned track's frames go into while the track is not the active transmitter.
#[derive(Component, Deref)]
pub struct PinnedImage(pub(crate) Handle<Image>);

/// Frames are arriving on this track.
#[derive(Component)]
pub struct VideoTrackLive;

/// The size of this track's frames changed; an entity event on the track.
#[derive(Event)]
pub struct VideoTrackResized;

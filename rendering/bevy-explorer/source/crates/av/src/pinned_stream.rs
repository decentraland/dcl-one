//! A `VideoPlayer` whose source names one participant's track
//! (`livekit-video://{identity}/{sid}`), after unity-explorer's `LivekitPlayer` with a
//! `UserStream` target (`Explorer/Assets/DCL/SDKComponents/MediaStream/LivekitPlayer.cs`): the
//! player binds exactly that track inside its scene's room and never follows the active
//! speaker; while the track is gone it shows what the stream manager picks, and binds the
//! track again when it is back (Unity stays on the manager's pick until the source changes).

use bevy::{ecs::relationship::RelationshipTarget, prelude::*};
use common::sets::SceneSets;
use dcl_component::proto_components::sdk::components::VideoState;
use livestream_manager::{
    address::LivekitAddress, states::Transmitter, ActiveTransmitter, PinnedBy, PinnedImage,
    PinnedTo, PublishedVideoTrack, ReceiverImage, VideoTrackLive, VideoTrackResized,
};
use scene_runner::{
    renderer_context::RendererSceneContext, update_world::material::VideoTextureOutput,
    ContainerEntity,
};

use crate::{video_player_should_be_playing, SetState, ShouldBePlaying, Stream, VideoPlayer};

/// The address a player asked for; kept while the track is away so it is bound again.
#[derive(Component, Debug, Deref)]
#[component(immutable)]
pub struct PinnedStream(pub LivekitAddress);

pub struct PinnedStreamPlugin;

impl Plugin for PinnedStreamPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (bind_pinned_streams, sync_pinned_texture)
                .chain()
                .after(video_player_should_be_playing)
                .in_set(SceneSets::PostLoop),
        );
        app.add_observer(pinned_to_removed);
        app.add_observer(video_track_live_added);
        app.add_observer(video_track_live_removed);
        app.add_observer(video_track_resized);
    }
}

/// Binds each scheduled pinned player to the published track its address names, inside its
/// scene's room. A player whose track is not there keeps its link as it is (none, after the
/// track despawned), so the manager's image stands in.
#[expect(clippy::type_complexity)]
pub(crate) fn bind_pinned_streams(
    mut commands: Commands,
    players: Query<
        (Entity, &PinnedStream, &ContainerEntity, Option<&PinnedTo>),
        (With<Stream>, With<ShouldBePlaying<VideoPlayer>>),
    >,
    scenes: Query<&RendererSceneContext>,
    tracks: Query<(Entity, &PublishedVideoTrack)>,
) {
    for (entity, pinned, container, current) in players.iter() {
        let LivekitAddress::User { identity, sid } = &**pinned else {
            continue;
        };
        let Ok(scene) = scenes.get(container.root) else {
            continue;
        };
        let found = tracks
            .iter()
            .find(|(_, track)| {
                *track.room == *scene.hash && track.identity == *identity && track.sid == *sid
            })
            .map(|(track, _)| track);
        if let Some(track) = found {
            if current.is_none_or(|current| current.0 != track) {
                debug!("{entity} pinned to track {track} ({identity}/{sid})");
                commands.entity(entity).try_insert(PinnedTo(track));
            }
        }
    }
}

/// Points each bound player at the image its track writes (the manager's while the track is
/// the active transmitter, the track's own otherwise), reporting Playing or Loading with it.
// complex on 1.97, plain on 1.99: allow rather than expect
#[allow(clippy::type_complexity)]
pub(crate) fn sync_pinned_texture(
    mut commands: Commands,
    players: Query<(
        Entity,
        &PinnedTo,
        &ContainerEntity,
        Option<&VideoTextureOutput>,
    )>,
    tracks: Query<(
        Option<&ActiveTransmitter>,
        Option<&PinnedImage>,
        Has<VideoTrackLive>,
    )>,
) {
    for (entity, pinned_to, container, output) in players.iter() {
        let Ok((active, pinned, live)) = tracks.get(pinned_to.0) else {
            continue;
        };
        let Some(target) = active
            .map(|image| (**image).clone())
            .or_else(|| pinned.map(|image| (**image).clone()))
        else {
            continue;
        };
        if output.is_none_or(|output| output.0 != target) {
            commands
                .entity(entity)
                .try_insert(VideoTextureOutput(target));
            commands.trigger(SetState::<VideoPlayer> {
                entity: *container,
                state: if live {
                    VideoState::VsPlaying
                } else {
                    VideoState::VsLoading
                },
                _phantom: Default::default(),
            });
        }
    }
}

/// The link went (the track despawned, the player stopped or changed source): back to the
/// manager's image while the player is still a receiver.
fn pinned_to_removed(
    trigger: Trigger<OnRemove, PinnedTo>,
    mut commands: Commands,
    players: Query<(&ContainerEntity, Option<&ReceiverImage>), With<VideoPlayer>>,
    transmitter: Res<State<Transmitter>>,
) {
    let entity = trigger.target();
    let Ok((container, image)) = players.get(entity) else {
        return;
    };
    let Some(image) = image else {
        return;
    };
    debug!("{entity} falls back to the stream manager's image");
    commands
        .entity(entity)
        .try_insert(VideoTextureOutput((**image).clone()));
    commands.trigger(SetState::<VideoPlayer> {
        entity: *container,
        state: if *transmitter.get() == Transmitter::On {
            VideoState::VsPlaying
        } else {
            VideoState::VsLoading
        },
        _phantom: Default::default(),
    });
}

fn set_pinned_players_state(
    track: Entity,
    state: VideoState,
    commands: &mut Commands,
    tracks: &Query<&PinnedBy>,
    players: &Query<&ContainerEntity, With<PinnedTo>>,
) {
    let Ok(pinned_by) = tracks.get(track) else {
        return;
    };
    for player in pinned_by.iter() {
        if let Ok(container) = players.get(player) {
            commands.trigger(SetState::<VideoPlayer> {
                entity: *container,
                state,
                _phantom: Default::default(),
            });
        }
    }
}

fn video_track_live_added(
    trigger: Trigger<OnAdd, VideoTrackLive>,
    mut commands: Commands,
    tracks: Query<&PinnedBy>,
    players: Query<&ContainerEntity, With<PinnedTo>>,
) {
    set_pinned_players_state(
        trigger.target(),
        VideoState::VsPlaying,
        &mut commands,
        &tracks,
        &players,
    );
}

fn video_track_live_removed(
    trigger: Trigger<OnRemove, VideoTrackLive>,
    mut commands: Commands,
    tracks: Query<&PinnedBy>,
    players: Query<&ContainerEntity, With<PinnedTo>>,
) {
    set_pinned_players_state(
        trigger.target(),
        VideoState::VsLoading,
        &mut commands,
        &tracks,
        &players,
    );
}

/// The track's image changed size: the materials on it rebind.
fn video_track_resized(
    trigger: Trigger<VideoTrackResized>,
    tracks: Query<&PinnedBy>,
    mut players: Query<&mut VideoTextureOutput, With<PinnedTo>>,
) {
    let Ok(pinned_by) = tracks.get(trigger.target()) else {
        return;
    };
    for player in pinned_by.iter() {
        if let Ok(mut output) = players.get_mut(player) {
            output.set_changed();
        }
    }
}

use std::collections::{HashMap, VecDeque};

use bevy::{
    ecs::{entity::EntityHashSet, relationship::Relationship},
    platform::collections::HashSet,
    prelude::*,
};
use common::{
    debug_panic,
    util::{AsH160, ReportErr},
};
use dcl_component::proto_components::kernel::comms::rfc4;
#[cfg(not(target_arch = "wasm32"))]
use livekit::prelude::Participant;
use livestream_manager::ActiveVideoCast;
use prost::Message;
use system_bridge::VoiceMessage;

use crate::livekit::room::UnannouncedParticipants;
#[cfg(target_arch = "wasm32")]
use crate::livekit::web::Participant;
use crate::{
    global_crdt::{
        NetworkUpdate, NonPlayerUpdate, PlayerMessage, PlayerUpdate, VoiceMessageStreams,
    },
    livekit::{
        participant::{
            ActiveSpeaker, ActiveSpeakersChanged, HostedBy, HostingParticipants,
            LivekitParticipant, Local, ParticipantConnected, ParticipantConnectionQuality,
            ParticipantDisconnected, ParticipantIndex, ParticipantMetadataChanged,
            ParticipantPayload,
        },
        plugin::{PlayerUpdateTask, PlayerUpdateTasksMut},
        room::LivekitRoom,
        track::{Camera as CameraTrack, Publishing, Video},
        LivekitRuntimeRes,
    },
    private_chat::PrivateChatRoom,
    voice_chat::{VoiceRoom, VOICE_CHAT_CHANNEL},
    SceneRoom,
};

const INBOUND_RATE_WINDOW_SECS: f64 = 1.0;
pub(crate) const MAX_MESSAGES_PER_WINDOW: usize = 300;
const GRACE_PERIOD: f32 = 3.;

/// Windows beyond which idle ones are dropped on the next check: keys that no participant
/// entity owns (community authors, keyed `community:<wallet>`) are not evicted any other way.
const MAX_RATE_WINDOWS: usize = 1024;

// Per-peer sliding-window rate limit; entries are evicted when the participant entity is removed
// or an unannounced sender is retired, which bounds the map by the senders the rooms know about,
// and idle windows are swept once the map outgrows `MAX_RATE_WINDOWS`. Keyed per room as well as
// identity so the same identity string in two rooms (island + scene) can't share a window.
#[derive(Resource, Default)]
pub(crate) struct InboundRateLimiter {
    windows: HashMap<(Entity, String), VecDeque<f64>>,
}

impl InboundRateLimiter {
    pub(crate) fn allow(&mut self, room: Entity, identity: &str, now: f64) -> bool {
        let cutoff = now - INBOUND_RATE_WINDOW_SECS;

        if self.windows.len() >= MAX_RATE_WINDOWS {
            self.windows
                .retain(|_, times| times.back().is_some_and(|&t| t >= cutoff));
        }
        let times = self.windows.entry((room, identity.to_owned())).or_default();
        while times.front().is_some_and(|&t| t < cutoff) {
            times.pop_front();
        }
        if times.len() >= MAX_MESSAGES_PER_WINDOW {
            return false;
        }
        times.push_back(now);
        true
    }
}

pub struct LivekitParticipantPlugin;

impl Plugin for LivekitParticipantPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<InboundRateLimiter>();
        app.add_observer(participant_connected);
        app.add_observer(participant_disconnected);
        app.add_observer(participant_entity_removed::<LivekitParticipant>);
        app.add_observer(participant_connection_quality_changed);
        app.add_observer(participant_payload);
        app.add_observer(active_speakers_changed);
        app.add_observer(is_now_speaking);
        app.add_observer(is_no_longer_speaking);

        app.add_systems(
            Update,
            (
                verify_active_speaker_grace_period,
                retire_unannounced_participants,
            ),
        );
    }
}

#[derive(Component, Deref, DerefMut)]
struct ActiveSpeakerGracePeriod(Timer);

#[allow(clippy::too_many_arguments)]
fn participant_connected(
    trigger: Trigger<ParticipantConnected>,
    mut commands: Commands,
    rooms: Query<(&LivekitRoom, Option<&HostingParticipants>)>,
    locals: Query<&LivekitParticipant, With<Local>>,
    mut indexes: Query<&mut ParticipantIndex>,
    transport_senders: crate::global_crdt::TransportSenders,
    mut player_update_tasks: PlayerUpdateTasksMut,
    livekit_runtime: LivekitRuntimeRes,
) {
    let ParticipantConnected {
        participant,
        room: room_entity,
    } = trigger.event();
    let Ok((room, hosting)) = rooms.get(*room_entity) else {
        debug_panic!("Room {room_entity} given to ParticipantConnected was invalid.");
    };
    debug!(
        "Participant '{}' ({}) connected to room {}.",
        participant.sid(),
        participant.identity(),
        room.name()
    );

    let is_local = matches!(participant.participant, Participant::Local(_));
    if admit_member(
        &mut commands,
        *room_entity,
        hosting,
        &locals,
        &mut indexes,
        participant.clone(),
        is_local,
    )
    .is_none()
    {
        return;
    }

    // Register presence explicitly. Membership is otherwise implied by a `PlayerUpdate` arriving,
    // and a peer whose avatar state rides Pulse sends none here, so it could sit in the room without
    // ever being recorded as being in it. `PlayerLeft` on disconnect is the mirror.
    if !is_local {
        if let Some(address) = participant.identity().as_str().as_h160() {
            let transport_id = *room_entity;
            if let Some(sender) = transport_senders.get(transport_id) {
                let task = livekit_runtime.spawn(async move {
                    sender
                        .send(
                            PlayerUpdate {
                                transport_id,
                                message: PlayerMessage::Joined,
                                address,
                            }
                            .into(),
                        )
                        .await
                });
                player_update_tasks.push(PlayerUpdateTask {
                    runtime: livekit_runtime.clone(),
                    task,
                });
            }
        }
    }

    commands.trigger(ParticipantMetadataChanged {
        room: *room_entity,
        participant: participant.clone(),
    });
}

fn participant_disconnected(
    trigger: Trigger<ParticipantDisconnected>,
    mut commands: Commands,
    rooms: Query<(
        &LivekitRoom,
        &ParticipantIndex,
        Option<&HostingParticipants>,
    )>,
    participants: Query<&LivekitParticipant>,
    transport_senders: crate::global_crdt::TransportSenders,
    mut player_update_tasks: PlayerUpdateTasksMut,
    livekit_runtime: LivekitRuntimeRes,
) {
    let ParticipantDisconnected {
        participant,
        room: room_entity,
    } = trigger.event();
    let Ok((room, index, hosting)) = rooms.get(*room_entity) else {
        debug_panic!("Room {room_entity} given to ParticipantDisconnected was invalid.");
    };
    debug!(
        "Participant '{}' ({}) disconnected from room {}.",
        participant.sid(),
        participant.identity(),
        room.name()
    );

    let departure = depart_member(
        &mut commands,
        index,
        hosting,
        &participants,
        participant.member_sid(),
        &participant.member_identity(),
    );

    // a same-identity rejoin that already owns the index keeps its membership
    if departure.leaves {
        if let Some(address) = participant.identity().as_str().as_h160() {
            let transport_id = *room_entity;
            // the transport (or its context) may already be torn down; skip only the
            // notification — the participant entity below must still be despawned
            if let Some(sender) = transport_senders.get(transport_id) {
                let task = livekit_runtime.spawn(async move {
                    sender
                        .send(NetworkUpdate::PlayerLeft {
                            transport_id,
                            address,
                        })
                        .await
                });
                player_update_tasks.push(PlayerUpdateTask {
                    runtime: (*livekit_runtime).clone(),
                    task,
                });
            }
        }
    }

    if departure.despawn.is_none() {
        error!(
            "Disconnecting participant '{}' ({}) not found in participants.",
            participant.sid(),
            participant.identity()
        );
    }
}

/// The sid and identity the room bookkeeping keys on; tests supply a stand-in for the SDK type.
pub(super) trait RoomMember: Component {
    fn member_sid(&self) -> String;
    fn member_identity(&self) -> String;
}

impl RoomMember for LivekitParticipant {
    fn member_sid(&self) -> String {
        self.sid().to_string()
    }
    fn member_identity(&self) -> String {
        self.identity().as_str().to_owned()
    }
}

/// Spawns the member in the room and indexes it. Returns `None` for an SDK resume that
/// re-announces a local participant already present.
fn admit_member<M: RoomMember>(
    commands: &mut Commands,
    room: Entity,
    hosting: Option<&HostingParticipants>,
    locals: &Query<&M, With<Local>>,
    indexes: &mut Query<&mut ParticipantIndex>,
    member: M,
    local: bool,
) -> Option<Entity> {
    if local {
        let existing = hosting
            .into_iter()
            .flat_map(|hosting| hosting.collection().iter().copied())
            .filter_map(|entity| {
                locals
                    .get(entity)
                    .ok()
                    .map(|local| (entity, local.member_sid()))
            });
        // a restarted session has a new sid: drop the old entity and its publications
        for entity in stale_locals(existing, &member.member_sid())? {
            commands.entity(entity).despawn();
        }
    }
    let key = ParticipantIndex::key(&member.member_identity());
    let mut cmd = commands.spawn((member, <HostedBy as Relationship>::from(room)));
    if local {
        cmd.insert(Local);
    }
    let entity = cmd.id();
    if let Ok(mut index) = indexes.get_mut(room) {
        index.insert(key, entity);
    }
    Some(entity)
}

/// Despawns the departing member, found by sid. The index entry is cleared on removal, only
/// while it still names that entity.
fn depart_member<M: RoomMember>(
    commands: &mut Commands,
    index: &ParticipantIndex,
    hosting: Option<&HostingParticipants>,
    members: &Query<&M>,
    sid: String,
    identity: &str,
) -> Departure {
    let hosted = hosting
        .into_iter()
        .flat_map(|hosting| hosting.collection().iter().copied())
        .filter_map(|entity| {
            members
                .get(entity)
                .ok()
                .map(|member| (entity, member.member_sid()))
        });
    let indexed = index.get(&ParticipantIndex::key(identity)).copied();
    let departure = plan_departure(hosted, &sid, indexed);
    if let Some(entity) = departure.despawn {
        commands.entity(entity).despawn();
    }
    departure
}

/// Local participant entities superseded by `sid`, or `None` if `sid` is already present.
fn stale_locals<S: PartialEq>(
    existing: impl IntoIterator<Item = (Entity, S)>,
    sid: &S,
) -> Option<Vec<Entity>> {
    let mut stale = Vec::new();
    for (entity, existing) in existing {
        if existing == *sid {
            return None;
        }
        stale.push(entity);
    }
    Some(stale)
}

struct Departure {
    despawn: Option<Entity>,
    leaves: bool,
}

/// The leaving participant is found by sid among the room's participants. The player leaves the
/// transport only if no other entity (a same-identity rejoin) holds its index entry.
fn plan_departure<S: PartialEq>(
    hosted: impl IntoIterator<Item = (Entity, S)>,
    sid: &S,
    indexed: Option<Entity>,
) -> Departure {
    let despawn = hosted
        .into_iter()
        .find(|(_, hosted)| hosted == sid)
        .map(|(entity, _)| entity);
    Departure {
        despawn,
        leaves: indexed.is_none() || indexed == despawn,
    }
}

// Covers both explicit disconnects and relationship-cascade despawns on room teardown, so
// rate-limiter entries can't outlive their participant.
fn participant_entity_removed<M: RoomMember>(
    trigger: Trigger<OnRemove, M>,
    participants: Query<(&M, Option<&HostedBy>)>,
    mut indexes: Query<&mut ParticipantIndex>,
    mut rate_limiter: ResMut<InboundRateLimiter>,
) {
    let Ok((participant, maybe_hosted)) = participants.get(trigger.target()) else {
        return;
    };
    let identity = participant.member_identity();
    match maybe_hosted {
        Some(hosted) => {
            // a rejoin under the same identity may have replaced the entry already
            if let Ok(mut index) = indexes.get_mut(hosted.get()) {
                let key = ParticipantIndex::key(&identity);
                if index.get(&key) == Some(&trigger.target()) {
                    index.remove(&key);
                }
            }
            rate_limiter.windows.remove(&(hosted.get(), identity));
        }
        // no room to scope by; over-evicting just forgets ≤1s of history
        None => rate_limiter.windows.retain(|(_, id), _| *id != identity),
    }
}

fn participant_connection_quality_changed(
    trigger: Trigger<ParticipantConnectionQuality>,
    mut commands: Commands,
    rooms: Query<(&LivekitRoom, &ParticipantIndex)>,
    private_rooms: Query<(), With<PrivateChatRoom>>,
) {
    let ParticipantConnectionQuality {
        participant,
        room,
        connection_quality,
    } = trigger.event();
    // Only the local quality is inspected for the environment-wide private chat room.
    if private_rooms.contains(*room) && matches!(participant.participant, Participant::Remote(_)) {
        return;
    }
    let Ok((livekit_room, index)) = rooms.get(*room) else {
        debug_panic!("Room given to ParticipantConnectionQuality was invalid.");
    };

    debug!(
        "Participant '{}' ({}) connection quality with {} changed to {:?}.",
        participant.sid(),
        participant.identity(),
        livekit_room.name(),
        connection_quality
    );

    let Some(entity) = index
        .get(&ParticipantIndex::key(participant.identity().as_str()))
        .copied()
    else {
        error!(
            "No entity referent to '{}' ({}).",
            participant.sid(),
            participant.identity()
        );
        return;
    };

    commands.entity(entity).try_insert(*connection_quality);
}

fn participant_payload(
    trigger: Trigger<ParticipantPayload>,
    transport_senders: crate::global_crdt::TransportSenders,
    mut player_update_tasks: PlayerUpdateTasksMut,
    livekit_runtime: LivekitRuntimeRes,
    mut rate_limiter: ResMut<InboundRateLimiter>,
    time: Res<Time>,
    private_rooms: Query<(), With<PrivateChatRoom>>,
) {
    let ParticipantPayload {
        room: room_entity,
        identity,
        payload,
        topic: _,
    } = trigger.event();
    if private_rooms.contains(*room_entity) {
        return;
    }

    if !rate_limiter.allow(*room_entity, identity.as_str(), time.elapsed_secs_f64()) {
        trace!("rate-limited payload from participant {identity}.");
        return;
    }

    let packet = match rfc4::Packet::decode(payload.as_slice()) {
        Ok(packet) => packet,
        Err(_) => {
            warn!("Could not decode payload from participant {identity}.");
            return;
        }
    };
    let Some(message) = packet.message else {
        warn!("Payload from {identity} had empty body.");
        return;
    };

    let room = *room_entity;

    let Some(sender) = transport_senders.get(room) else {
        return;
    };

    let task = if let Some(address) = identity.as_str().as_h160() {
        trace!(
            "[{}] received [{}] packet {message:?} from {address}",
            room_entity,
            packet.protocol_version
        );
        livekit_runtime.spawn(async move {
            sender
                .send(
                    PlayerUpdate {
                        transport_id: room,
                        message: PlayerMessage::PlayerData(message),
                        address,
                    }
                    .into(),
                )
                .await
        })
    } else {
        let address = identity.to_string();
        livekit_runtime.spawn(async move {
            sender
                .send(
                    NonPlayerUpdate {
                        transport_id: room,
                        address,
                        message,
                    }
                    .into(),
                )
                .await
        })
    };

    player_update_tasks.push(PlayerUpdateTask {
        runtime: (*livekit_runtime).clone(),
        task,
    });
}

/// Withdraws the presence of senders that were never announced (see [`UnannouncedParticipants`]):
/// their players were created from data alone, so no `ParticipantDisconnected` will ever come.
fn retire_unannounced_participants(
    rooms: Query<(Entity, &mut UnannouncedParticipants)>,
    transport_senders: crate::global_crdt::TransportSenders,
    mut player_update_tasks: PlayerUpdateTasksMut,
    mut rate_limiter: ResMut<InboundRateLimiter>,
    livekit_runtime: LivekitRuntimeRes,
    time: Res<Time>,
) {
    let now = time.elapsed_secs_f64();
    for (room, mut unannounced) in rooms {
        if unannounced.values().all(|deadline| *deadline > now) {
            continue;
        }
        let sender = transport_senders.get(room);
        unannounced.retain(|identity, deadline| {
            if *deadline > now {
                return true;
            }
            rate_limiter.windows.remove(&(room, identity.clone()));
            if let (Some(sender), Some(address)) = (sender.clone(), identity.as_str().as_h160()) {
                let task = livekit_runtime.spawn(async move {
                    sender
                        .send(NetworkUpdate::PlayerLeft {
                            transport_id: room,
                            address,
                        })
                        .await
                });
                player_update_tasks.push(PlayerUpdateTask {
                    runtime: (*livekit_runtime).clone(),
                    task,
                });
            }
            false
        });
    }
}

fn active_speakers_changed(
    trigger: Trigger<ActiveSpeakersChanged>,
    mut commands: Commands,
    rooms: Query<&HostingParticipants>,
    participants: Query<(Entity, &LivekitParticipant)>,
    old_speakers: Query<Entity, With<ActiveSpeaker>>,
) {
    let ActiveSpeakersChanged { speakers } = trigger.event();
    let Ok(roster) = rooms.get(trigger.target()) else {
        return;
    };

    let active_speakers_sid = speakers
        .iter()
        .map(|participant| participant.sid())
        .collect::<HashSet<_>>();

    let active_speakers = participants
        .iter_many(roster.collection())
        .filter(|(_, livekit_participant)| active_speakers_sid.contains(&livekit_participant.sid()))
        .map(|(entity, _)| entity)
        .collect::<EntityHashSet>();

    let old_speakers = old_speakers
        .iter_many(roster.collection())
        .collect::<EntityHashSet>();

    let new_speakers = active_speakers.difference(&old_speakers);
    let no_longer_speakers = old_speakers.difference(&active_speakers);

    for new_speaker in new_speakers {
        commands.entity(*new_speaker).try_insert(ActiveSpeaker);
    }
    for no_longer_speaker in no_longer_speakers {
        commands
            .entity(*no_longer_speaker)
            .try_remove::<ActiveSpeaker>();
    }
}

fn is_now_speaking(
    trigger: Trigger<OnInsert, ActiveSpeaker>,
    mut commands: Commands,
    participants: Query<
        (&LivekitParticipant, Option<&HostedBy>, Option<&Publishing>),
        With<ActiveSpeaker>,
    >,
    tracks: Query<Has<ActiveVideoCast>, (With<Video>, With<CameraTrack>)>,
    scene_rooms: Query<&SceneRoom>,
    voice_rooms: Query<(), With<VoiceRoom>>,
    senders: Res<VoiceMessageStreams>,
) {
    let entity = trigger.target();

    let Ok((participant, maybe_hosted_by, maybe_publishing)) = participants.get(entity) else {
        unreachable!("Infallible Query");
    };
    debug!(
        "{} ({}) is now speaking.",
        participant.sid(),
        participant.identity()
    );

    let Some(room) = maybe_hosted_by else {
        debug_panic!(
            "{} ({}) is not hosted by a room.",
            participant.sid(),
            participant.identity()
        );
    };

    if let Some(sender_address) = participant.identity().as_str().as_h160() {
        let channel = speaking_channel(room.get(), &scene_rooms, &voice_rooms);
        for sender in senders.iter() {
            sender
                .send(VoiceMessage {
                    sender_address: format!("{:#x}", sender_address),
                    channel: channel.clone(),
                    active: true,
                })
                .report();
        }
    } else if let Some(publishing) = maybe_publishing {
        for published in publishing.collection() {
            if let Ok(has_active_video_cast) = tracks.get(*published) {
                if has_active_video_cast {
                    commands
                        .entity(*published)
                        .try_remove::<ActiveSpeakerGracePeriod>();
                } else {
                    commands.entity(*published).insert(ActiveVideoCast);
                }
            }
        }
    }
}

fn is_no_longer_speaking(
    trigger: Trigger<OnReplace, ActiveSpeaker>,
    mut commands: Commands,
    participants: Query<
        (&LivekitParticipant, Option<&HostedBy>, Option<&Publishing>),
        With<ActiveSpeaker>,
    >,
    tracks: Query<(), (With<Video>, With<CameraTrack>, With<ActiveVideoCast>)>,
    scene_rooms: Query<&SceneRoom>,
    voice_rooms: Query<(), With<VoiceRoom>>,
    senders: Res<VoiceMessageStreams>,
) {
    let entity = trigger.target();

    let Ok((participant, maybe_hosted_by, maybe_publishing)) = participants.get(entity) else {
        unreachable!("Infallible Query");
    };
    debug!(
        "{} ({}) is no longer speaking.",
        participant.sid(),
        participant.identity()
    );

    let Some(room) = maybe_hosted_by else {
        debug_panic!(
            "{} ({}) is not hosted by a room.",
            participant.sid(),
            participant.identity()
        );
    };

    if let Some(sender_address) = participant.identity().as_str().as_h160() {
        let channel = speaking_channel(room.get(), &scene_rooms, &voice_rooms);
        for sender in senders.iter() {
            sender
                .send(VoiceMessage {
                    sender_address: format!("{:#x}", sender_address),
                    channel: channel.clone(),
                    active: false,
                })
                .report();
        }
    } else if let Some(publishing) = maybe_publishing {
        for published in publishing.collection() {
            if tracks.contains(*published) {
                commands
                    .entity(*published)
                    .try_insert(ActiveSpeakerGracePeriod(Timer::from_seconds(
                        GRACE_PERIOD,
                        TimerMode::Once,
                    )));
            }
        }
    }
}

/// The channel a speaker's room names: the scene's hash, the voice room's name, else nearby.
fn speaking_channel(
    room: Entity,
    scene_rooms: &Query<&SceneRoom>,
    voice_rooms: &Query<(), With<VoiceRoom>>,
) -> String {
    if let Ok(scene) = scene_rooms.get(room) {
        scene.0.clone()
    } else if voice_rooms.contains(room) {
        VOICE_CHAT_CHANNEL.to_owned()
    } else {
        "Nearby".to_owned()
    }
}

fn verify_active_speaker_grace_period(
    mut commands: Commands,
    active_speakers: Populated<(Entity, &mut ActiveSpeakerGracePeriod)>,
    time: Res<Time<Real>>,
) {
    let delta = time.delta();
    for (entity, mut active_speaker_grace_period) in active_speakers.into_inner() {
        active_speaker_grace_period.tick(delta);
        if active_speaker_grace_period.finished() {
            debug!("Grace period for {} has passed.", entity);
            commands
                .entity(entity)
                .try_remove::<(ActiveSpeakerGracePeriod, ActiveVideoCast)>();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn idle_rate_windows_are_swept_once_the_map_is_full() {
        let mut limiter = InboundRateLimiter::default();
        let room = Entity::from_raw(1);
        for i in 0..MAX_RATE_WINDOWS {
            assert!(limiter.allow(room, &format!("community:0x{i:040x}"), 0.0));
        }
        assert_eq!(limiter.windows.len(), MAX_RATE_WINDOWS);
        // a message well after the window: every idle entry goes, the live one stays
        assert!(limiter.allow(room, "community:0xlive", 10.0));
        assert_eq!(limiter.windows.len(), 1);
        assert!(limiter.allow(room, "community:0xlive", 10.5));
        assert_eq!(limiter.windows.len(), 1);
    }

    #[test]
    fn an_unknown_sid_despawns_nothing_and_spares_a_live_rejoin() {
        let new = Entity::from_raw(2);
        let departure = plan_departure([(new, "PA_new")], &"PA_old", Some(new));
        assert_eq!(departure.despawn, None);
        assert!(!departure.leaves);
    }

    /// Stand-in for the SDK participant, which only a live LiveKit connection can construct.
    #[derive(Component, Clone)]
    struct Member {
        sid: &'static str,
        identity: &'static str,
    }

    impl RoomMember for Member {
        fn member_sid(&self) -> String {
            self.sid.to_owned()
        }
        fn member_identity(&self) -> String {
            self.identity.to_owned()
        }
    }

    #[derive(Event)]
    struct Joined {
        room: Entity,
        member: Member,
        local: bool,
    }

    #[derive(Event)]
    struct Left {
        room: Entity,
        member: Member,
    }

    /// `PlayerLeft` decisions, in order.
    #[derive(Resource, Default)]
    struct Leaves(Vec<bool>);

    /// Stands for a `MicrophonePublication`: revoked when its participant entity goes.
    #[derive(Component)]
    struct Publication(std::sync::Arc<std::sync::atomic::AtomicBool>);

    impl Drop for Publication {
        fn drop(&mut self) {
            self.0.store(true, std::sync::atomic::Ordering::SeqCst);
        }
    }

    // the same room bookkeeping as `participant_connected` / `participant_disconnected`
    fn on_joined(
        trigger: Trigger<Joined>,
        mut commands: Commands,
        rooms: Query<Option<&HostingParticipants>>,
        locals: Query<&Member, With<Local>>,
        mut indexes: Query<&mut ParticipantIndex>,
    ) {
        let Joined {
            room,
            member,
            local,
        } = trigger.event();
        let hosting = rooms.get(*room).unwrap();
        admit_member(
            &mut commands,
            *room,
            hosting,
            &locals,
            &mut indexes,
            member.clone(),
            *local,
        );
    }

    fn on_left(
        trigger: Trigger<Left>,
        mut commands: Commands,
        rooms: Query<(&ParticipantIndex, Option<&HostingParticipants>)>,
        members: Query<&Member>,
        mut leaves: ResMut<Leaves>,
    ) {
        let Left { room, member } = trigger.event();
        let (index, hosting) = rooms.get(*room).unwrap();
        let departure = depart_member(
            &mut commands,
            index,
            hosting,
            &members,
            member.member_sid(),
            &member.member_identity(),
        );
        leaves.0.push(departure.leaves);
    }

    fn room_world() -> (World, Entity) {
        let mut world = World::new();
        world.init_resource::<InboundRateLimiter>();
        world.init_resource::<Leaves>();
        world.add_observer(on_joined);
        world.add_observer(on_left);
        world.add_observer(participant_entity_removed::<Member>);
        let room = world.spawn(ParticipantIndex::default()).id();
        (world, room)
    }

    fn join(world: &mut World, room: Entity, sid: &'static str, local: bool) {
        world.trigger(Joined {
            room,
            member: Member {
                sid,
                identity: "0xAbC",
            },
            local,
        });
        world.flush();
    }

    fn leave(world: &mut World, room: Entity, sid: &'static str) {
        world.trigger(Left {
            room,
            member: Member {
                sid,
                identity: "0xAbC",
            },
        });
        world.flush();
    }

    fn members(world: &mut World) -> Vec<(&'static str, bool)> {
        let mut members = world
            .query::<(&Member, Has<Local>)>()
            .iter(world)
            .map(|(member, local)| (member.sid, local))
            .collect::<Vec<_>>();
        members.sort();
        members
    }

    fn indexed(world: &World, room: Entity) -> Option<&'static str> {
        let entity = *world
            .get::<ParticipantIndex>(room)?
            .get(&ParticipantIndex::key("0xabc"))?;
        Some(world.get::<Member>(entity)?.sid)
    }

    #[test]
    fn an_sdk_resume_keeps_the_one_local_participant_and_its_publication() {
        let (mut world, room) = room_world();
        join(&mut world, room, "PA_1", true);
        let local = world
            .query_filtered::<Entity, With<Local>>()
            .single(&world)
            .unwrap();
        let revoked = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        world.entity_mut(local).insert(Publication(revoked.clone()));

        join(&mut world, room, "PA_1", true);

        assert_eq!(members(&mut world), vec![("PA_1", true)]);
        assert!(world.get_entity(local).is_ok());
        assert!(!revoked.load(std::sync::atomic::Ordering::SeqCst));
    }

    #[test]
    fn a_restarted_session_replaces_the_local_participant_and_revokes_its_publication() {
        let (mut world, room) = room_world();
        join(&mut world, room, "PA_1", true);
        let old = world
            .query_filtered::<Entity, With<Local>>()
            .single(&world)
            .unwrap();
        let revoked = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        world.entity_mut(old).insert(Publication(revoked.clone()));

        join(&mut world, room, "PA_2", true);

        assert_eq!(members(&mut world), vec![("PA_2", true)]);
        assert!(world.get_entity(old).is_err());
        assert!(revoked.load(std::sync::atomic::Ordering::SeqCst));
        assert_eq!(indexed(&world, room), Some("PA_2"));
    }

    #[test]
    fn a_rejoin_announced_before_the_old_departure_keeps_the_live_peer() {
        let (mut world, room) = room_world();
        join(&mut world, room, "PA_old", false);
        join(&mut world, room, "PA_new", false);

        leave(&mut world, room, "PA_old");

        assert_eq!(members(&mut world), vec![("PA_new", false)]);
        assert_eq!(indexed(&world, room), Some("PA_new"));
        assert_eq!(world.resource::<Leaves>().0, vec![false]);
    }

    #[test]
    fn a_departure_before_the_rejoin_leaves_and_the_rejoin_is_indexed_afresh() {
        let (mut world, room) = room_world();
        join(&mut world, room, "PA_old", false);

        leave(&mut world, room, "PA_old");
        assert!(members(&mut world).is_empty());
        assert_eq!(indexed(&world, room), None);
        join(&mut world, room, "PA_new", false);

        assert_eq!(members(&mut world), vec![("PA_new", false)]);
        assert_eq!(indexed(&world, room), Some("PA_new"));
        assert_eq!(world.resource::<Leaves>().0, vec![true]);
    }

    #[test]
    fn a_room_falling_silent_preserves_other_rooms_speakers() {
        let mut world = World::new();
        world.add_observer(active_speakers_changed);
        let island = world.spawn_empty().id();
        let scene = world.spawn_empty().id();
        let island_speaker = world.spawn((HostedBy(island), ActiveSpeaker)).id();
        let scene_speaker = world.spawn((HostedBy(scene), ActiveSpeaker)).id();
        world.trigger_targets(ActiveSpeakersChanged { speakers: vec![] }, scene);
        world.flush();
        assert!(world.get::<ActiveSpeaker>(island_speaker).is_some());
        assert!(world.get::<ActiveSpeaker>(scene_speaker).is_none());
    }
}

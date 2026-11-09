//! Trusted, explicitly activated voice sessions. Their lifetime is independent of the realm.

// The routing policy is shared by native and browser publication.
pub(crate) struct VoiceRouting<T> {
    pub active: Option<T>,
    proximity_enabled: Option<bool>,
}

impl<T> Default for VoiceRouting<T> {
    fn default() -> Self {
        Self {
            active: None,
            proximity_enabled: None,
        }
    }
}

impl<T: Copy + PartialEq> VoiceRouting<T> {
    fn activate(&mut self, room: T, mic_enabled: bool) {
        self.proximity_enabled.get_or_insert(mic_enabled);
        self.active = Some(room);
    }

    fn leave(&mut self, mic_enabled: &mut bool) {
        self.active = None;
        if let Some(previous) = self.proximity_enabled.take() {
            *mic_enabled = previous;
        }
    }

    pub(crate) fn allows(&self, room: T, proximity: bool, private_chat: bool) -> bool {
        !private_chat && self.active.map_or(proximity, |active| active == room)
    }
}

fn valid_session(session: &str) -> bool {
    !session.is_empty()
        && session.len() <= 128
        && session
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'-' | b'_' | b'.'))
}

#[cfg(test)]
mod routing_tests {
    use super::*;

    #[test]
    fn activation_routes_only_to_voice_before_connection_and_during_failure() {
        let mut route = VoiceRouting::default();
        assert!(route.allows(1, true, false));
        route.activate(2, true);
        // Connection markers deliberately do not enter the routing decision.
        assert!(!route.allows(1, true, false));
        assert!(route.allows(2, false, false));
        assert!(!route.allows(3, false, false));
    }

    #[test]
    fn private_chat_never_publishes_even_if_mistagged_as_transport() {
        let route = VoiceRouting::<u64>::default();
        assert!(!route.allows(1, true, true));
    }

    #[test]
    fn replacement_revokes_old_room_without_losing_proximity_preference() {
        let mut route = VoiceRouting::default();
        route.activate(1, false);
        route.activate(2, true);
        assert!(!route.allows(1, false, false));
        assert!(route.allows(2, false, false));
        let mut microphone = true;
        route.leave(&mut microphone);
        assert!(!microphone);
        assert!(route.allows(3, true, false));
    }

    #[test]
    fn leave_restores_previous_preference_and_is_idempotent() {
        let mut route = VoiceRouting::default();
        route.activate(1, true);
        let mut microphone = false;
        route.leave(&mut microphone);
        assert!(microphone);
        microphone = false;
        route.leave(&mut microphone);
        assert!(!microphone);
    }

    #[test]
    fn session_names_are_bounded_and_safe_for_diagnostics() {
        assert!(valid_session("call-123"));
        for value in ["", " ", "call\nsecret", "https://room?access_token=secret"] {
            assert!(!valid_session(value));
        }
        assert!(!valid_session(&"x".repeat(129)));
    }
}

// ECS integration
use alloy_core::primitives::Address;
use bevy::{ecs::relationship::Relationship, prelude::*};
use bevy_console::ConsoleCommand;
use common::{
    access_status::{AccessScope, AccessStatus},
    structs::{MicState, NoRenderApp},
    util::AsH160,
};
use console::{AddDebugSection, DebugSection, DoAddConsoleCommand};
use system_bridge::SystemApi;
use wallet::Wallet;

use crate::{
    adapter::{parse_adapter, AdapterKind},
    livekit::{
        participant::{HostedBy, LivekitParticipant, Local},
        room::{Connected, Connecting, Reconnecting},
        track::{
            LivekitTrack, Microphone, PublishedBy, SubscribeToTrack, Subscribed, Subscribing,
            UnsubscribeToTrack,
        },
        StartLivekit,
    },
    room_connection::{RoomAccessScope, RoomRecovery},
};

pub struct VoiceRoomPlugin;

impl Plugin for VoiceRoomPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ActiveVoiceRoom>()
            .add_systems(Update, (update_voice_room, subscribe_voice_tracks).chain())
            .add_console_command::<VoiceRoomCommand, _>(voice_room_command)
            .add_debug_section(DebugSection {
                title: "On-demand voice",
                query: "/voice_room",
                fields: &[
                    ("Active", "active"),
                    ("Session", "session"),
                    ("Connection", "phase"),
                    ("Failure", "failure"),
                    ("Block status ready", "blockStatusReady"),
                    ("Proximity microphone suppressed", "proximitySuppressed"),
                    ("Microphone cleanup pending", "microphoneCleanupPending"),
                ],
                actions: &[("Leave voice session", "/voice_room leave")],
                inputs: &[],
            });
    }
}

#[derive(Component)]
pub struct VoiceRoom;

#[derive(Resource, Default)]
pub(crate) struct ActiveVoiceRoom {
    pub routing: VoiceRouting<Entity>,
    session: Option<String>,
    owner: Option<Address>,
}

impl ActiveVoiceRoom {
    pub(crate) fn is_owned_by(&self, room: Entity, owner: Option<Address>) -> bool {
        self.routing.active == Some(room) && owner.is_some() && self.owner == owner
    }

    fn leave(&mut self, commands: &mut Commands, mic: &mut MicState) {
        if let Some(room) = self.routing.active {
            commands.entity(room).try_despawn();
        }
        self.routing.leave(&mut mic.enabled);
        self.session = None;
        self.owner = None;
    }
}

/// Consumes only the trusted engine channel; regular scene RPCs cannot activate a session.
pub(crate) fn update_voice_room(
    mut commands: Commands,
    mut events: EventReader<SystemApi>,
    wallet: Res<Wallet>,
    no_render: Option<Res<NoRenderApp>>,
    mut active: ResMut<ActiveVoiceRoom>,
    mut mic: ResMut<MicState>,
    mut connect: EventWriter<StartLivekit>,
    publications: Query<&crate::livekit::mic::MicrophonePublication>,
) {
    if active.owner.is_some() && (active.owner != wallet.address() || no_render.is_some()) {
        for publication in &publications {
            publication.revoke();
        }
        active.leave(&mut commands, &mut mic);
    }
    for event in events.read() {
        match event {
            SystemApi::SetVoiceRoom {
                session,
                adapter,
                response,
            } => {
                let result = if no_render.is_some() {
                    Err("voice rooms require a rendered client".to_owned())
                } else if wallet.address().is_none() {
                    Err("voice rooms require a wallet".to_owned())
                } else if !valid_session(session) {
                    Err("invalid voice session identifier".to_owned())
                } else {
                    parse_adapter(adapter)
                        .map_err(|err| err.to_string())
                        .and_then(|(kind, address)| {
                            if kind != AdapterKind::Livekit {
                                return Err("voice rooms require a LiveKit adapter".to_owned());
                            }
                            // Revoke synchronously before acknowledging activation. Async SDK work
                            // retains this revocation and may only stop/unpublish after this point.
                            for publication in &publications {
                                publication.revoke();
                            }
                            let room = commands
                                .spawn((
                                    VoiceRoom,
                                    RoomAccessScope(AccessScope::Voice(session.clone())),
                                ))
                                .id();
                            if let Some(old) = active.routing.active {
                                commands.entity(old).try_despawn();
                            }
                            active.routing.activate(room, mic.enabled);
                            active.session = Some(session.clone());
                            active.owner = wallet.address();
                            connect.write(StartLivekit {
                                entity: room,
                                address,
                                context: None,
                            });
                            Ok(())
                        })
                };
                response.send(result);
            }
            SystemApi::LeaveVoiceRoom if active.routing.active.is_some() => {
                for publication in &publications {
                    publication.revoke();
                }
                active.leave(&mut commands, &mut mic);
            }
            _ => {}
        }
    }
}

/// Standalone voice playback does not require an avatar or a CRDT transport.
fn subscribe_voice_tracks(
    mut commands: Commands,
    active: Res<ActiveVoiceRoom>,
    wallet: Res<Wallet>,
    access: Res<AccessStatus>,
    participants: Query<(&LivekitParticipant, &HostedBy), Without<Local>>,
    tracks: Query<
        (
            Entity,
            &LivekitTrack,
            &PublishedBy,
            Has<Subscribed>,
            Has<Subscribing>,
        ),
        With<Microphone>,
    >,
) {
    let owner = wallet.address();
    for (entity, track, publisher, subscribed, subscribing) in &tracks {
        let Ok((participant, room)) = participants.get(publisher.get()) else {
            continue;
        };
        if Some(room.get()) != active.routing.active {
            continue;
        }
        let allowed = active.is_owned_by(room.get(), owner)
            && participant
                .identity()
                .as_str()
                .as_h160()
                .is_some_and(|address| access.allows_voice_from(owner, address));
        if allowed && !subscribed && !subscribing {
            commands.entity(entity).trigger(SubscribeToTrack);
        } else if !allowed && (subscribed || subscribing) {
            // Silence immediately while the unsubscribe acknowledgment is in flight.
            if let Some(remote) = track.track() {
                #[cfg(target_arch = "wasm32")]
                remote.pan_and_volume(0.0, 0.0);
                #[cfg(not(target_arch = "wasm32"))]
                let _ = remote;
            }
            commands.entity(entity).trigger(UnsubscribeToTrack);
        }
    }
}

/// Inspect or leave a session. Adapters enter through SystemApi, never a logged console line.
#[derive(clap::Parser, ConsoleCommand)]
#[command(name = "/voice_room")]
struct VoiceRoomCommand {
    #[arg(value_parser = ["status", "leave"], default_value = "status")]
    action: String,
}

fn voice_room_command(
    mut input: ConsoleCommand<VoiceRoomCommand>,
    active: Res<ActiveVoiceRoom>,
    wallet: Res<Wallet>,
    access: Res<AccessStatus>,
    rooms: Query<
        (
            Has<Connected>,
            Has<Connecting>,
            Has<Reconnecting>,
            &RoomRecovery,
        ),
        With<VoiceRoom>,
    >,
    drains: Option<Res<crate::livekit::mic::MicrophoneDrain>>,
    mut events: EventWriter<SystemApi>,
) {
    let Some(Ok(command)) = input.take() else {
        return;
    };
    if command.action == "leave" {
        events.write(SystemApi::LeaveVoiceRoom);
    }
    let room = active.routing.active.and_then(|room| rooms.get(room).ok());
    let phase = match room {
        Some((true, _, _, _)) => "connected",
        Some((_, _, true, _)) => "reconnecting",
        Some((_, _, _, recovery)) if recovery.failure.is_some() => "failed",
        Some((_, true, _, _)) => "connecting",
        _ if active.routing.active.is_some() => "pending",
        _ => "inactive",
    };
    input.reply_ok(
        serde_json::json!({"active": active.routing.active.is_some(), "session": active.session,
        "phase": phase, "failure": room.and_then(|(_, _, _, recovery)| recovery.failure),
        "blockStatusReady": access.blocks_ready_for(wallet.address()),
        "proximitySuppressed": active.routing.active.is_some(),
        "microphoneCleanupPending": drains.map_or(0, |drain| drain.pending())})
        .to_string(),
    );
}

#[cfg(test)]
mod lifecycle_tests {
    use super::*;
    use common::rpc::RpcResultSender;

    fn app() -> App {
        let mut app = App::new();
        let mut wallet = Wallet::default();
        wallet.finalize_as_guest_with_seed([42; 32]);
        app.insert_resource(wallet)
            .init_resource::<MicState>()
            .init_resource::<ActiveVoiceRoom>()
            .add_event::<SystemApi>()
            .add_event::<StartLivekit>()
            .add_systems(Update, update_voice_room);
        app
    }

    fn activate(app: &mut App, adapter: &str) -> Result<(), String> {
        let (response, mut result) = RpcResultSender::channel();
        app.world_mut().send_event(SystemApi::SetVoiceRoom {
            session: "test-call".to_owned(),
            adapter: adapter.to_owned(),
            response,
        });
        app.update();
        result.try_recv().unwrap()
    }

    #[test]
    fn invalid_replacement_preserves_active_session() {
        let mut app = app();
        activate(&mut app, "livekit:wss://voice.example/?access_token=test").unwrap();
        let room = app.world().resource::<ActiveVoiceRoom>().routing.active;
        assert!(activate(&mut app, "livekit:wss://voice.example/").is_err());
        assert_eq!(
            app.world().resource::<ActiveVoiceRoom>().routing.active,
            room
        );
        assert!(app.world().get_entity(room.unwrap()).is_ok());
    }

    #[test]
    fn replacement_and_leave_retire_pending_owners() {
        let mut app = app();
        activate(&mut app, "livekit:wss://voice.example/?access_token=first").unwrap();
        let old = app
            .world()
            .resource::<ActiveVoiceRoom>()
            .routing
            .active
            .unwrap();
        activate(
            &mut app,
            "livekit:wss://voice.example/?access_token=replacement",
        )
        .unwrap();
        let next = app
            .world()
            .resource::<ActiveVoiceRoom>()
            .routing
            .active
            .unwrap();
        assert_ne!(old, next);
        assert!(app.world().get_entity(old).is_err());
        app.world_mut().resource_mut::<MicState>().enabled = true;
        app.world_mut().send_event(SystemApi::LeaveVoiceRoom);
        app.update();
        assert!(app.world().get_entity(next).is_err());
        assert!(app
            .world()
            .resource::<ActiveVoiceRoom>()
            .routing
            .active
            .is_none());
        assert!(!app.world().resource::<MicState>().enabled);
        app.update();
        assert!(app
            .world()
            .resource::<ActiveVoiceRoom>()
            .routing
            .active
            .is_none());
    }

    #[test]
    fn wallet_change_cancels_voice_but_ordinary_updates_preserve_it() {
        let mut app = app();
        activate(&mut app, "livekit:wss://voice.example/?access_token=test").unwrap();
        let room = app.world().resource::<ActiveVoiceRoom>().routing.active;
        app.update();
        assert_eq!(
            app.world().resource::<ActiveVoiceRoom>().routing.active,
            room
        );
        app.world_mut()
            .resource_mut::<Wallet>()
            .finalize_as_guest_with_seed([43; 32]);
        app.update();
        assert!(app
            .world()
            .resource::<ActiveVoiceRoom>()
            .routing
            .active
            .is_none());
        assert!(app.world().get_entity(room.unwrap()).is_err());
    }

    #[test]
    fn headless_and_signed_out_clients_cannot_activate() {
        let mut app = app();
        app.insert_resource(NoRenderApp);
        assert!(activate(&mut app, "livekit:wss://voice.example/?access_token=test").is_err());
        app.world_mut().remove_resource::<NoRenderApp>();
        app.world_mut().resource_mut::<Wallet>().disconnect();
        assert!(activate(&mut app, "livekit:wss://voice.example/?access_token=test").is_err());
        assert!(app
            .world()
            .resource::<ActiveVoiceRoom>()
            .routing
            .active
            .is_none());
    }
}

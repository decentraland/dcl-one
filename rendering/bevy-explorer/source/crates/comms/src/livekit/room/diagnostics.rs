use std::collections::{BTreeSet, HashSet};

use bevy::prelude::*;
use bevy_console::ConsoleCommand;
use common::util::AsH160;
use console::{AddDebugSection, DebugInput, DebugInputKind, DebugSection, DoAddConsoleCommand};
use serde_json::json;

use super::{
    ConnectFailed, Connected, Connecting, Disconnected, LivekitRoom, Reconnecting, RoomTraffic,
};
use crate::{
    automation::RoomState,
    global_crdt::ForeignPlayer,
    livekit::{
        participant::{ConnectionQuality, HostedBy, LivekitParticipant, Local, ParticipantIndex},
        ConnectionAvailability, LivekitNetworkMessage, LivekitRuntimeRes, LivekitTransport,
    },
    private_chat::PrivateChatRoom,
    profile::ProfileMetaCache,
    SceneRoom, Transport, TransportType,
};
use bevy::ecs::relationship::Relationship;

pub(super) fn register(app: &mut App) {
    app.add_console_command::<RoomDiagnostics, _>(room_diagnostics);
    let client = !app
        .world()
        .contains_resource::<common::structs::NoRenderApp>()
        && !common::structs::server_mode();
    if client {
        app.add_console_command::<RoomConnectionCommand, _>(room_connection)
            .add_systems(Update, (close_paused_rooms, discard_paused_messages));
    }
    for (title, scope) in [
        ("Island room", "island"),
        ("Scene room", "scene"),
        ("Private chat room", "chat"),
    ] {
        let mut fields = vec![
            ("Connection policy", "policy"),
            ("Room", "room.name"),
            ("Connection state", "room.state"),
            #[cfg(not(target_arch = "wasm32"))]
            ("Room SID", "room.sid"),
            ("Dial attempts", "room.attempts"),
            ("Self SID", "room.selfSid"),
            ("Local connection quality", "room.quality"),
            ("Remote participants", "room.remoteCount"),
        ];
        if scope == "scene" {
            fields.extend([
                ("Scene ID", "room.scene"),
                ("Associated base parcel", "room.baseParcel"),
            ]);
        }
        let pause = format!("/room_connection {scope} disconnect");
        let resume = format!("/room_connection {scope} connect");
        let actions = [
            ("Disconnect current room", pause.as_str()),
            ("Reconnect current room", resume.as_str()),
        ];
        app.add_debug_section(DebugSection {
            title,
            query: &format!("/room_diagnostics {scope}"),
            fields: &fields,
            actions: if client { &actions } else { &[] },
            inputs: &[],
        });
    }
    let indicators = app.is_plugin_added::<bevy::gizmos::GizmoPlugin>()
        && !app
            .world()
            .contains_resource::<common::structs::NoRenderApp>();
    if indicators {
        app.init_resource::<RoomIndicators>()
            .add_console_command::<RoomIndicatorsCommand, _>(room_indicators)
            .add_systems(
                PostUpdate,
                draw_room_indicators.run_if(|enabled: Res<RoomIndicators>| enabled.0),
            );
    }
    app.add_debug_section(DebugSection {
        title: "Room info",
        query: "/room_diagnostics info",
        fields: &[
            ("Active remote players", "players"),
            ("Players on LiveKit", "onLivekit"),
            ("Players off LiveKit", "offLivekit"),
            ("Participants without players", "unmatched"),
            ("Cached remote profile endpoints", "metadata"),
        ],
        actions: &[],
        inputs: if indicators {
            &[DebugInput {
                label: "Show room markers (gold Pulse, green island, blue scene)",
                field: "indicators",
                command: "/room_indicators {value}",
                kind: DebugInputKind::Toggle,
            }]
        } else {
            &[]
        },
    })
    .add_debug_section(DebugSection {
        title: "Room throughput",
        query: "/room_diagnostics throughput",
        fields: &[
            ("Island received (payload B/s)", "island.received"),
            ("Island published (payload B/s)", "island.sent"),
            ("Island participant quality (first 32)", "island.peers"),
            ("Scene received (payload B/s)", "scene.received"),
            ("Scene published (payload B/s)", "scene.sent"),
            ("Scene participant quality (first 32)", "scene.peers"),
        ],
        actions: &[],
        inputs: &[],
    });
}

#[derive(clap::ValueEnum, Clone, Copy, PartialEq, Eq)]
enum Scope {
    Island,
    Scene,
    Chat,
    Info,
    Throughput,
}

/// Inspect LiveKit room lifecycle, participant membership and data-channel throughput.
#[derive(clap::Parser, ConsoleCommand)]
#[command(name = "/room_diagnostics")]
struct RoomDiagnostics {
    scope: Scope,
}

#[allow(clippy::type_complexity, clippy::too_many_arguments)]
fn room_diagnostics(
    mut input: ConsoleCommand<RoomDiagnostics>,
    policy: Res<State<ConnectionAvailability>>,
    rooms: Query<
        (
            Entity,
            Option<&SceneRoom>,
            Has<PrivateChatRoom>,
            Has<Transport>,
            Option<&LivekitRoom>,
            Has<Connected>,
            Has<Reconnecting>,
            Has<Disconnected>,
            Has<ConnectFailed>,
            Option<&RoomTraffic>,
            Has<RoomPaused>,
            &LivekitTransport,
        ),
        With<LivekitTransport>,
    >,
    participants: Query<(
        &LivekitParticipant,
        &HostedBy,
        Has<Local>,
        Option<&ConnectionQuality>,
    )>,
    players: Query<&ForeignPlayer>,
    metadata: Res<ProfileMetaCache>,
    indicators: Option<Res<RoomIndicators>>,
    definitions: Option<Res<Assets<ipfs::EntityDefinition>>>,
) {
    let Some(Ok(command)) = input.take() else {
        return;
    };
    if command.scope == Scope::Info {
        let players: HashSet<_> = players.iter().map(|player| player.address).collect();
        let members: HashSet<_> = participants
            .iter()
            .filter(|(_, hosted, local, _)| {
                !*local
                    && rooms
                        .get(hosted.get())
                        .is_ok_and(|(_, _, _, transport, ..)| transport)
            })
            .filter_map(|(p, ..)| p.identity().as_str().as_h160())
            .collect();
        input.reply_ok(json!({"players": players.len(), "onLivekit": players.intersection(&members).count(),
            "offLivekit": players.difference(&members).count(), "unmatched": members.difference(&players).count(),
            "metadata": metadata.0.len(), "indicators": indicators.map(|s| s.0)}).to_string());
        return;
    }

    let unavailable = match command.scope {
        Scope::Island => json!({"room": "No LiveKit island room assigned"}),
        Scope::Scene => json!({"room": "No LiveKit scene room assigned"}),
        Scope::Chat => json!({"room": "No private chat room assigned"}),
        Scope::Throughput => json!({
            "island": "No LiveKit island room assigned",
            "scene": "No LiveKit scene room assigned",
        }),
        Scope::Info => unreachable!(),
    };
    let mut result =
        json!({"policy": format!("{:?}", policy.get()), "room": null, "unavailable": unavailable});
    for (
        entity,
        scene,
        private,
        transport,
        room,
        connected,
        reconnecting,
        disconnected,
        failed,
        traffic,
        paused,
        livekit,
    ) in &rooms
    {
        let scope = if private {
            Scope::Chat
        } else if !transport {
            continue;
        } else if scene.is_some() {
            Scope::Scene
        } else {
            Scope::Island
        };
        if scope != command.scope && (command.scope != Scope::Throughput || private) {
            continue;
        }
        let mut remote_count = 0;
        let mut peer_quality = BTreeSet::new();
        let mut local_sid = None;
        let mut local_quality = None;
        for (participant, _, local, _quality) in participants
            .iter()
            .filter(|(_, hosted, ..)| hosted.get() == entity)
        {
            // The native SDK retains current quality even if it hasn't changed since joining.
            #[cfg(not(target_arch = "wasm32"))]
            let quality = Some(format!("{:?}", participant.connection_quality()));
            #[cfg(target_arch = "wasm32")]
            let quality = _quality.map(|q| format!("{q:?}"));
            if local {
                local_sid = Some(participant.sid().to_string());
                local_quality = quality;
            } else {
                remote_count += 1;
                if command.scope == Scope::Throughput {
                    peer_quality.insert(format!(
                        "{} ({})",
                        participant.sid(),
                        quality.as_deref().unwrap_or("unknown")
                    ));
                    if peer_quality.len() > 32 {
                        peer_quality.pop_last();
                    }
                }
            }
        }
        if command.scope == Scope::Throughput {
            let key = if scope == Scope::Scene {
                "scene"
            } else {
                "island"
            };
            result[key] = json!({"sent": traffic.map(|t| t.sent.last_second().bytes), "received": traffic.map(|t| t.received.last_second().bytes), "peers": peer_quality});
            if traffic.is_none() {
                result["unavailable"][key] = json!("Room has not connected");
            } else {
                result["unavailable"].as_object_mut().unwrap().remove(key);
            }
        } else {
            #[cfg(not(target_arch = "wasm32"))]
            let sid = room.and_then(|r| r.maybe_sid()).map(|sid| sid.to_string());
            #[cfg(target_arch = "wasm32")]
            let sid: Option<String> = None;
            let base = scene.and_then(|scene| {
                definitions
                    .as_deref()?
                    .iter()
                    .find(|(_, def)| def.id == scene.0)?
                    .1
                    .metadata
                    .as_ref()?
                    .pointer("/scene/base")?
                    .as_str()
            });
            result["room"] = json!({"state": if paused { "paused" } else { RoomState::from_markers(connected, reconnecting, disconnected, failed).name() },
                "name": room.map(|r| r.name().to_string()), "sid": sid, "scene": scene.map(|s| s.0.as_str()),
                "baseParcel": base, "attempts": livekit.retries,
                "remoteCount": remote_count, "selfSid": local_sid, "quality": local_quality});
            result["unavailable"] = if connected {
                json!({"room.quality": "Waiting for LiveKit to report connection quality",
                    "room.baseParcel": "Scene metadata has no loaded base parcel",
                    "room.sid": "LiveKit has not reported a room SID",
                    "room.selfSid": "Waiting for the local participant"})
            } else {
                json!({"room": "Room is not connected", "room.baseParcel": "Scene metadata has no loaded base parcel"})
            };
            break;
        }
    }
    input.reply_ok(result.to_string());
}

/// A pause belongs to the current credentialed room. A fresh assignment starts normally.
#[derive(Component)]
pub(super) struct RoomPaused;

#[derive(clap::ValueEnum, Clone, Copy)]
enum RoomAction {
    Connect,
    Disconnect,
}

/// Control a current client room without changing account or admission policy.
#[derive(clap::Parser, ConsoleCommand)]
#[command(name = "/room_connection")]
struct RoomConnectionCommand {
    scope: Scope,
    action: RoomAction,
}

#[allow(clippy::type_complexity)]
fn room_connection(
    mut input: ConsoleCommand<RoomConnectionCommand>,
    mut commands: Commands,
    policy: Res<State<ConnectionAvailability>>,
    rooms: Query<
        (
            Entity,
            Has<SceneRoom>,
            Has<PrivateChatRoom>,
            Has<Transport>,
            Has<RoomPaused>,
            Has<ConnectFailed>,
            Option<&LivekitRoom>,
        ),
        With<LivekitTransport>,
    >,
) {
    let Some(Ok(command)) = input.take() else {
        return;
    };
    if common::structs::server_mode() || *policy.get() == ConnectionAvailability::Unavailable {
        input.reply_failed("Room connections are controlled by session policy");
        return;
    }
    let mut matching =
        rooms
            .iter()
            .filter(|(_, scene, private, transport, ..)| match command.scope {
                Scope::Chat => *private,
                Scope::Scene => *scene && *transport,
                Scope::Island => !*scene && !*private && *transport,
                _ => false,
            });
    let Some((entity, _, _, _, paused, failed, room)) = matching.next() else {
        input.reply_failed("No current room");
        return;
    };
    if matching.next().is_some() {
        input.reply_failed("More than one room matches");
        return;
    }
    match command.action {
        RoomAction::Disconnect if !paused => {
            commands
                .entity(entity)
                .insert(RoomPaused)
                .remove::<Connecting>();
        }
        RoomAction::Connect if paused || failed => {
            let mut entity = commands.entity(entity);
            entity.remove::<RoomPaused>();
            // A refused dial retains Connecting. Its OnAdd observer dials again only
            // when the marker is removed and re-added.
            if failed {
                entity.remove::<Connecting>();
            }
            // While close is in flight the SDK will report Disconnected, invoking the
            // regular reconnect path once cleanup of the old participants has finished.
            if failed || room.is_none_or(|room| room.closed) {
                entity.insert(Connecting);
            }
        }
        _ => {}
    }
    input.reply_ok("Room connection request applied");
}

// A dial can finish in the same frame a pause is requested. Close both rooms that
// were already live at pause time and any room installed by that in-flight dial.
#[allow(clippy::type_complexity)]
fn close_paused_rooms(
    rooms: Query<
        &LivekitRoom,
        (
            With<RoomPaused>,
            Or<(Added<RoomPaused>, Added<LivekitRoom>)>,
        ),
    >,
    runtime: LivekitRuntimeRes,
) {
    for room in &rooms {
        if !room.closed {
            let room = room.room.clone();
            runtime.spawn(async move {
                if let Err(err) = room.close().await {
                    warn!("Unable to close paused room: {err}");
                }
            });
        }
    }
}

fn discard_paused_messages(mut rooms: Query<&mut LivekitNetworkMessage, With<RoomPaused>>) {
    for mut messages in &mut rooms {
        while messages.try_recv().is_ok() {}
    }
}

#[derive(Resource, Default)]
struct RoomIndicators(bool);

/// Show the Pulse carrier and the two LiveKit rosters above remote avatars.
#[derive(clap::Parser, ConsoleCommand)]
#[command(name = "/room_indicators")]
struct RoomIndicatorsCommand {
    #[arg(action = clap::ArgAction::Set)]
    show: bool,
}

fn room_indicators(
    mut input: ConsoleCommand<RoomIndicatorsCommand>,
    mut enabled: ResMut<RoomIndicators>,
) {
    if let Some(Ok(command)) = input.take() {
        enabled.0 = command.show;
        input.reply_ok("Room markers updated");
    }
}

fn draw_room_indicators(
    mut gizmos: Gizmos,
    players: Query<(&GlobalTransform, &ForeignPlayer)>,
    transports: Query<&Transport>,
    rooms: Query<(&ParticipantIndex, Has<SceneRoom>), With<Transport>>,
) {
    for (transform, player) in &players {
        let address = format!("{:#x}", player.address);
        let mut island = false;
        let mut scene = false;
        for (roster, is_scene) in &rooms {
            if roster.contains_key(&address) {
                if is_scene {
                    scene = true;
                } else {
                    island = true;
                }
            }
        }
        let pulse = player.transports.iter().any(|t| {
            transports
                .get(*t)
                .is_ok_and(|t| t.transport_type == TransportType::Pulse)
        });
        let top = transform.translation() + Vec3::Y * 2.35;
        for (i, (present, color)) in [
            (pulse, Color::srgb(1.0, 0.75, 0.0)),
            (island, Color::srgb(0.25, 1.0, 0.3)),
            (scene, Color::srgb(0.3, 0.65, 1.0)),
        ]
        .into_iter()
        .enumerate()
        {
            if present {
                gizmos.sphere(
                    Isometry3d::from_translation(top + Vec3::Y * (i as f32 * 0.12)),
                    0.055,
                    color,
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy_console::ConsoleCommandEntered;

    #[test]
    fn reconnect_restarts_a_refused_dial_once() {
        let mut app = App::new();
        app.add_plugins(console::ConsolePlugin {
            add_bevy_console: false,
        })
        .insert_resource(State::new(ConnectionAvailability::Available))
        .add_console_command::<RoomConnectionCommand, _>(room_connection)
        .add_observer(
            |trigger: Trigger<OnAdd, Connecting>, mut rooms: Query<&mut LivekitTransport>| {
                rooms.get_mut(trigger.target()).unwrap().retries += 1;
            },
        );
        let (components, sender, _) = crate::livekit::standalone_room_components(String::new());
        let room = app
            .world_mut()
            .spawn((
                components,
                Transport {
                    transport_type: TransportType::Livekit,
                    context: Entity::PLACEHOLDER,
                    sender,
                    control: None,
                },
                Connecting,
            ))
            .id();
        app.update();
        app.world_mut().entity_mut(room).insert(ConnectFailed);
        app.world_mut().send_event(ConsoleCommandEntered {
            command_name: "/room_connection".into(),
            args: vec!["island".into(), "connect".into()],
            responder: None,
        });
        app.update();
        assert_eq!(
            app.world().get::<LivekitTransport>(room).unwrap().retries,
            2
        );
    }
}

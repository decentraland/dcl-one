use super::*;
use alloy_core::primitives::Address;
use common::{
    rpc::{RpcStreamReceiver, RpcStreamSender},
    structs::{CurrentRealm, EmoteMask},
};
use comms::{
    chat_reaction::ChatReactionEvent,
    global_crdt::{
        process_transport_updates, ChatEvent, DiscardPlayerUpdates, GlobalCrdtState, PlayerMessage,
        PlayerSceneAnimEvent, PlayerUpdate, ProfileEvent,
    },
    scene_topics::SceneTopicTraffic,
    SceneRoomConnection, ServerSceneRooms, Transport, TransportType,
};
use dcl_component::transform_and_parent::{DclQuat, DclTranslation};

fn app() -> App {
    let mut app = App::new();
    app.init_resource::<Time>()
        .init_resource::<DiscardPlayerUpdates>()
        .init_resource::<SceneRoomConnection>()
        .init_resource::<ServerSceneRooms>()
        .init_resource::<CurrentRealm>()
        .init_resource::<SceneTopicTraffic>()
        .init_resource::<CrdtContexts>()
        .add_event::<RpcCallEvent>()
        .add_event::<ProfileEvent>()
        .add_event::<PlayerPositionEvent>()
        .add_event::<PlayerSceneAnimEvent>()
        .add_event::<EmoteLifecycleEvent>()
        .add_event::<ChatEvent>()
        .add_event::<ChatReactionEvent>()
        .add_systems(
            Update,
            process_transport_updates.before(SceneSets::RestrictedActions),
        )
        .add_plugins(LegacySocialPlugin);
    let shared = app.world_mut().spawn(GlobalCrdtState::new(None)).id();
    app.world_mut()
        .resource_mut::<CrdtContexts>()
        .0
        .insert(String::new(), shared);
    app
}

fn scene(app: &mut App, hash: &str) -> (Entity, Entity) {
    let context = app
        .world_mut()
        .spawn(GlobalCrdtState::new(Some(hash.to_owned())))
        .id();
    app.world_mut()
        .resource_mut::<CrdtContexts>()
        .0
        .insert(hash.to_owned(), context);
    let scene = app
        .world_mut()
        .spawn(RendererSceneContext::new(
            dcl_types::SceneId::DUMMY,
            hash.to_owned(),
            String::new(),
            false,
            0,
            String::new(),
            IVec2::ZERO,
            Default::default(),
            vec![],
            Default::default(),
            vec![],
            Entity::PLACEHOLDER,
            0.,
            false,
            "7",
            false,
            false,
        ))
        .id();
    (scene, context)
}

struct Subscription {
    scene: Entity,
    hash: String,
    liveness: RpcStreamReceiver<String>,
}

fn subscribe_to(app: &mut App, scene: Entity, hash: &str, capacity: usize) -> Subscription {
    let (sender, receiver) = RpcStreamSender::channel_with_capacity(capacity);
    app.world_mut().send_event(RpcCallEvent {
        origin: RpcOrigin::Scene {
            entity: scene,
            hash: hash.into(),
        },
        call: RpcCall::SubscribeLegacyAvatarEvents { sender },
    });
    Subscription {
        scene,
        hash: hash.to_owned(),
        liveness: receiver,
    }
}

fn peer(app: &mut App, context: Entity, address: Address) -> Entity {
    let (sender, _receiver) = tokio::sync::mpsc::channel(8);
    let transport = app
        .world_mut()
        .spawn(Transport {
            transport_type: TransportType::WebsocketRoom,
            sender,
            control: None,
            context,
        })
        .id();
    app.world()
        .get::<GlobalCrdtState>(context)
        .unwrap()
        .get_sender()
        .try_send(
            PlayerUpdate {
                transport_id: transport,
                message: PlayerMessage::Joined,
                address,
            }
            .into(),
        )
        .unwrap();
    app.update();
    let entity = app
        .world_mut()
        .query::<(Entity, &ForeignPlayer)>()
        .iter(app.world())
        .find(|(_, peer)| peer.context == context && peer.address == address)
        .unwrap()
        .0;
    let mut profile = UserProfile::default();
    profile.content.eth_address = format!("{address:#x}");
    profile.content.name = "Peer".into();
    profile.base_url = "https://peer.example/content/contents/".into();
    app.world_mut().entity_mut(entity).insert(profile);
    entity
}

fn drain(app: &mut App, subscription: &mut Subscription) -> Vec<Value> {
    let (response, mut receiver) = common::rpc::RpcResultSender::channel();
    app.world_mut().send_event(RpcCallEvent {
        origin: RpcOrigin::Scene {
            entity: subscription.scene,
            hash: subscription.hash.clone().into(),
        },
        call: RpcCall::PullLegacyAvatarEvents { response },
    });
    app.update();
    receiver
        .try_recv()
        .unwrap()
        .into_iter()
        .map(|value| {
            let envelope: Value = serde_json::from_str(&value).unwrap();
            let payload: Value =
                serde_json::from_str(envelope["payload"].as_str().unwrap()).unwrap();
            assert_eq!(envelope["event"], payload["type"]);
            payload
        })
        .collect()
}

fn expression(app: &mut App, avatar: Entity, urn: &str, source: EmoteLifecycleSource) {
    app.world_mut().send_event(EmoteLifecycleEvent {
        avatar,
        source,
        event: EmoteLifecycle::Started {
            urn: urn.into(),
            r#loop: false,
            mask: EmoteMask::FullBody,
        },
    });
}

#[test]
fn native_updates_are_scoped_ordered_and_drained_once() {
    let mut app = app();
    let (a, ca) = scene(&mut app, "a");
    let (b, cb) = scene(&mut app, "b");
    let address = Address::repeat_byte(1);
    let pa = peer(&mut app, ca, address);
    let pb = peer(&mut app, cb, address);
    let mut ra = subscribe_to(&mut app, a, "a", 1024);
    let mut rb = subscribe_to(&mut app, b, "b", 1024);
    app.update();
    for receiver in [&mut ra, &mut rb] {
        let initial = drain(&mut app, receiver);
        assert_eq!(
            initial
                .iter()
                .map(|v| v["type"].as_str().unwrap())
                .collect::<Vec<_>>(),
            ["USER_VISIBLE", "USER_DATA"]
        );
        assert_eq!(initial[1]["userId"], format!("{address:#x}"));
    }
    expression(&mut app, pa, "wave", EmoteLifecycleSource::Wire);
    expression(&mut app, pa, "dance", EmoteLifecycleSource::Wire);
    expression(
        &mut app,
        pa,
        "playback-duplicate",
        EmoteLifecycleSource::Playback,
    );
    expression(&mut app, pb, "other-room", EmoteLifecycleSource::Wire);
    app.update();
    let events = drain(&mut app, &mut ra);
    assert_eq!(
        events
            .iter()
            .map(|v| v["expressionId"].as_str().unwrap())
            .collect::<Vec<_>>(),
        ["wave", "dance"]
    );
    assert!(events.iter().all(|v| v["timestamp"].as_u64().unwrap() > 0));
    assert_eq!(drain(&mut app, &mut rb)[0]["expressionId"], "other-room");
    assert!(drain(&mut app, &mut ra).is_empty());
    app.update();
    assert!(drain(&mut app, &mut ra).is_empty());
    app.world_mut().despawn(pa);
    assert_eq!(drain(&mut app, &mut ra)[0]["type"], "USER_REMOVED");
    assert!(drain(&mut app, &mut rb).is_empty());
}

#[test]
fn visibility_and_profile_changes_preserve_renderer_payload() {
    let mut app = app();
    let (s, context) = scene(&mut app, "scene");
    let p = peer(&mut app, context, Address::repeat_byte(2));
    let mut receiver = subscribe_to(&mut app, s, "scene", 1024);
    app.update();
    let initial = drain(&mut app, &mut receiver);
    let profile = &initial[1]["profile"];
    assert_eq!(profile["name"], "Peer#0202");
    assert_eq!(profile["version"], -1);
    assert_eq!(profile["avatar"]["eyeColor"]["a"], 1);
    assert!(profile["avatar"].get("eyes").is_none());
    assert!(profile["avatar"].get("snapshots").is_none());
    assert_eq!(
        profile["snapshots"]["body"],
        "QmSav1o6QK37Jj1yhbmhYk9MJc6c2H5DWbWzPVsg9JLYfF"
    );
    app.world_mut().entity_mut(p).insert(Visibility::Hidden);
    app.update();
    let hidden = drain(&mut app, &mut receiver);
    assert_eq!(hidden.len(), 1);
    assert_eq!(hidden[0]["type"], "USER_VISIBLE");
    assert_eq!(hidden[0]["visible"], false);
    app.world_mut().entity_mut(p).insert(Visibility::Inherited);
    app.update();
    let shown = drain(&mut app, &mut receiver);
    assert_eq!(shown[0]["type"], "USER_VISIBLE");
    assert_eq!(shown[1]["type"], "USER_DATA");
    let unchanged = app.world().get::<UserProfile>(p).unwrap().clone();
    app.world_mut().entity_mut(p).insert(unchanged);
    app.update();
    assert!(drain(&mut app, &mut receiver).is_empty());
    app.world_mut()
        .get_mut::<UserProfile>(p)
        .unwrap()
        .content
        .name = "Changed".into();
    app.update();
    assert_eq!(
        drain(&mut app, &mut receiver)[0]["profile"]["name"],
        "Changed#0202"
    );
}

#[test]
fn native_positions_emit_without_render_interpolation_or_profile_identity_mixups() {
    let mut app = app();
    let (s, context) = scene(&mut app, "scene");
    let p = peer(&mut app, context, Address::repeat_byte(3));
    let mut receiver = subscribe_to(&mut app, s, "scene", 1024);
    app.update();
    drain(&mut app, &mut receiver);
    app.world_mut()
        .entity_mut(p)
        .insert(Transform::from_xyz(99., 99., 99.));
    app.update();
    assert!(drain(&mut app, &mut receiver).is_empty());
    app.world_mut().send_event(PlayerPositionEvent {
        player: p,
        time: 1.,
        timestamp: 1.,
        translation: DclTranslation([1., 2., 3.]),
        rotation: DclQuat([0., 0., 0., 1.]),
        velocity: Vec3::ZERO,
        grounded: None,
        remote_move_kind: None,
        teleport: false,
        precision: Vec3::ZERO,
    });
    app.update();
    let position = drain(&mut app, &mut receiver);
    assert_eq!(position.len(), 1);
    assert_eq!(position[0]["data"]["position"]["positionZ"], 3.);
    assert_eq!(position[0]["data"]["position"]["index"], 1);
    app.world_mut()
        .get_mut::<UserProfile>(p)
        .unwrap()
        .content
        .eth_address = format!("{:#x}", Address::repeat_byte(4));
    app.update();
    assert!(drain(&mut app, &mut receiver).is_empty());
    let sentinel = peer(&mut app, context, Address::ZERO);
    expression(&mut app, sentinel, "sentinel", EmoteLifecycleSource::Wire);
    app.update();
    assert!(drain(&mut app, &mut receiver).is_empty());
}

#[cfg(feature = "livekit")]
#[test]
fn public_voice_samples_authoritative_room_state_without_loss_or_cross_scene_leaks() {
    use bevy::ecs::relationship::Relationship;
    use comms::{
        livekit::participant::{ActiveSpeaker, HostedBy, ParticipantIndex},
        SceneRoom,
    };
    use system_bridge::VoiceMessage;
    let mut app = app();
    let (a, _) = scene(&mut app, "a");
    let (b, _) = scene(&mut app, "b");
    let context = app.world().resource::<CrdtContexts>().shared();
    app.world_mut()
        .resource_mut::<CrdtContexts>()
        .0
        .insert("a".into(), context);
    app.world_mut()
        .resource_mut::<CrdtContexts>()
        .0
        .insert("b".into(), context);
    let address = Address::repeat_byte(5);
    let player = peer(&mut app, context, address);
    let (sender, _) = tokio::sync::mpsc::channel(8);
    let room = app
        .world_mut()
        .spawn((
            ParticipantIndex::default(),
            SceneRoom("a".into()),
            Transport {
                transport_type: TransportType::WebsocketRoom,
                sender,
                control: None,
                context,
            },
        ))
        .id();
    let speaker = app
        .world_mut()
        .spawn((<HostedBy as Relationship>::from(room), ActiveSpeaker))
        .id();
    app.world_mut()
        .get_mut::<ParticipantIndex>(room)
        .unwrap()
        .insert(format!("{address:#x}"), speaker);
    // A speaking participant already exists when the scene subscribes.
    let mut ra = subscribe_to(&mut app, a, "a", 1);
    let mut rb = subscribe_to(&mut app, b, "b", 1);
    app.update();
    let first = drain(&mut app, &mut ra);
    assert!(first
        .iter()
        .any(|event| event["type"] == "USER_TALKING" && event["talking"] == true));
    assert!(!drain(&mut app, &mut rb)
        .iter()
        .any(|event| event["type"] == "USER_TALKING"));
    // The same address can leave and rejoin before the next sampling pass. Its new
    // incarnation needs a fresh speaking event even though the room never went silent.
    let peer = app
        .world_mut()
        .entity_mut(player)
        .take::<ForeignPlayer>()
        .unwrap();
    app.world_mut().entity_mut(player).insert(peer);
    app.update();
    let reincarnated = drain(&mut app, &mut ra);
    assert!(reincarnated
        .iter()
        .any(|event| event["type"] == "USER_REMOVED"));
    assert!(reincarnated
        .iter()
        .any(|event| event["type"] == "USER_TALKING" && event["talking"] == true));
    // Force B's recovery while A is speaking: a shared CRDT context does not share room audio.
    app.world_mut()
        .get_mut::<LegacySocialSubscription>(b)
        .unwrap()
        .resync = true;
    assert!(drain(&mut app, &mut rb)
        .iter()
        .filter(|event| event["type"] == "USER_TALKING")
        .all(|event| event["talking"] == false));
    // The old transition bridge drops the final stop after filling its input. Sampling
    // ActiveSpeaker must still report false, both normally and during overflow recovery.
    let (bridge, mut lost) = RpcStreamSender::channel_with_capacity(1);
    bridge
        .send(VoiceMessage {
            sender_address: format!("{address:#x}"),
            channel: "a".into(),
            active: true,
        })
        .unwrap();
    bridge
        .send(VoiceMessage {
            sender_address: format!("{address:#x}"),
            channel: "a".into(),
            active: false,
        })
        .unwrap();
    assert!(lost.try_recv().unwrap().active);
    assert!(lost.try_recv().is_err());
    app.world_mut()
        .entity_mut(speaker)
        .remove::<ActiveSpeaker>();
    app.update();
    assert!(drain(&mut app, &mut ra)
        .iter()
        .any(|event| event["type"] == "USER_TALKING" && event["talking"] == false));
    app.world_mut()
        .get_mut::<LegacySocialSubscription>(a)
        .unwrap()
        .resync = true;
    assert!(drain(&mut app, &mut ra)
        .iter()
        .filter(|event| event["type"] == "USER_TALKING")
        .all(|event| event["talking"] == false));
    // A public island transport without SceneRoom is Nearby for shared-context scenes.
    app.world_mut().entity_mut(speaker).insert(ActiveSpeaker);
    app.world_mut().entity_mut(room).remove::<SceneRoom>();
    app.update();
    for subscription in [&mut ra, &mut rb] {
        assert!(drain(&mut app, subscription)
            .iter()
            .any(|event| event["type"] == "USER_TALKING" && event["talking"] == true));
    }
    // Private calls have participants but no realm Transport. They cannot become Nearby.
    app.world_mut().entity_mut(room).remove::<Transport>();
    app.update();
    for subscription in [&mut ra, &mut rb] {
        let events = drain(&mut app, subscription);
        assert!(events
            .iter()
            .any(|event| event["type"] == "USER_TALKING" && event["talking"] == false));
        assert!(!events
            .iter()
            .any(|event| event["type"] == "USER_TALKING" && event["talking"] == true));
    }
}

#[test]
fn subscription_lifetime_follows_scene_and_receiver() {
    let mut app = app();
    let (a, _) = scene(&mut app, "a");
    let (b, _) = scene(&mut app, "b");
    let ra = subscribe_to(&mut app, a, "a", 1024);
    let mut rb = subscribe_to(&mut app, b, "b", 1024);
    app.update();
    assert!(app.world().get::<LegacySocialSubscription>(a).is_some());
    assert!(app.world().get::<LegacySocialSubscription>(b).is_some());
    drop(ra);
    app.update();
    assert!(app.world().get::<LegacySocialSubscription>(a).is_none());
    app.world_mut().despawn(b);
    assert!(app.world().get::<LegacySocialSubscription>(b).is_none());
    assert!(matches!(
        rb.liveness.try_recv(),
        Err(tokio::sync::mpsc::error::TryRecvError::Disconnected)
    ));
}

#[test]
fn subscription_rejects_non_scene_and_stale_scene_identity() {
    let mut app = app();
    let (scene, _) = scene(&mut app, "actual");
    let mut receiver = subscribe_to(&mut app, scene, "forged", 1024);
    app.update();
    assert!(app.world().get::<LegacySocialSubscription>(scene).is_none());
    assert!(drain(&mut app, &mut receiver).is_empty());
}

#[test]
fn overflow_recovers_removal_and_current_visibility_with_a_bounded_pull() {
    let mut app = app();
    let (s, context) = scene(&mut app, "scene");
    let departed = peer(&mut app, context, Address::repeat_byte(6));
    let remaining = peer(&mut app, context, Address::repeat_byte(7));
    let mut subscription = subscribe_to(&mut app, s, "scene", 1024);
    app.update();
    assert_eq!(drain(&mut app, &mut subscription).len(), 4);
    for _ in 0..=MAX_PENDING {
        expression(&mut app, remaining, "wave", EmoteLifecycleSource::Wire);
    }
    app.update();
    let state = app.world().get::<LegacySocialSubscription>(s).unwrap();
    assert!(state.resync);
    assert!(state.pending.is_empty());
    app.world_mut().despawn(departed);
    app.world_mut()
        .entity_mut(remaining)
        .insert(Visibility::Hidden);
    let recovered = drain(&mut app, &mut subscription);
    assert!(recovered.len() <= MAX_PENDING);
    assert_eq!(
        recovered
            .iter()
            .filter(|event| event["type"] == "USER_REMOVED")
            .count(),
        2
    );
    let visible = recovered
        .iter()
        .find(|event| event["type"] == "USER_VISIBLE")
        .unwrap();
    assert_eq!(visible["userId"], format!("{:#x}", Address::repeat_byte(7)));
    assert_eq!(visible["visible"], false);
    let state_events: Vec<_> = recovered
        .iter()
        .skip(2)
        .map(|event| event["type"].as_str().unwrap())
        .collect();
    assert_eq!(state_events, ["USER_VISIBLE", "USER_DATA", "USER_TALKING"]);
    assert_eq!(recovered.last().unwrap()["talking"], false);
    assert!(drain(&mut app, &mut subscription).is_empty());
}

#[test]
fn cancelled_pull_leaves_pending_events_for_the_next_pull() {
    let mut app = app();
    let (s, context) = scene(&mut app, "scene");
    peer(&mut app, context, Address::repeat_byte(8));
    let mut subscription = subscribe_to(&mut app, s, "scene", 1024);
    app.update();
    let (response, receiver) = common::rpc::RpcResultSender::channel();
    drop(receiver);
    app.world_mut().send_event(RpcCallEvent {
        origin: RpcOrigin::Scene {
            entity: s,
            hash: "scene".into(),
        },
        call: RpcCall::PullLegacyAvatarEvents { response },
    });
    app.update();
    assert_eq!(drain(&mut app, &mut subscription).len(), 2);
}

#[test]
fn rendered_child_visibility_tracks_culling_and_survives_avatar_rebuild() {
    let mut app = app();
    let (s, context) = scene(&mut app, "scene");
    let p = peer(&mut app, context, Address::repeat_byte(9));
    let rendered = app
        .world_mut()
        .spawn((avatar::AvatarProcessed, ChildOf(p), Visibility::Inherited))
        .id();
    let mut subscription = subscribe_to(&mut app, s, "scene", 1024);
    app.update();
    drain(&mut app, &mut subscription);
    app.world_mut()
        .entity_mut(rendered)
        .insert(Visibility::Hidden);
    let hidden = drain(&mut app, &mut subscription);
    assert_eq!(hidden.len(), 1);
    assert_eq!(hidden[0]["visible"], false);
    assert_eq!(
        *app.world().get::<Visibility>(p).unwrap(),
        Visibility::Inherited
    );
    let replacement = app
        .world_mut()
        .spawn((avatar::AvatarProcessed, ChildOf(p), Visibility::Inherited))
        .id();
    let shown = drain(&mut app, &mut subscription);
    assert_eq!(shown[0]["visible"], true);
    assert_eq!(shown[1]["type"], "USER_DATA");
    app.world_mut().despawn(rendered);
    assert!(drain(&mut app, &mut subscription).is_empty());
    app.world_mut().despawn(p);
    assert!(app.world().get_entity(replacement).is_err());
    assert_eq!(
        drain(&mut app, &mut subscription)[0]["type"],
        "USER_REMOVED"
    );
}

#[test]
fn subscribe_and_pull_share_a_frame_and_late_registration_preserves_pending_events() {
    let mut app = app();
    let (scene, context) = scene(&mut app, "scene");
    let avatar = peer(&mut app, context, Address::repeat_byte(9));
    let mut subscription = subscribe_to(&mut app, scene, "scene", 1);
    // No update between registration and pull: the ordered producer applies the subscription.
    assert_eq!(drain(&mut app, &mut subscription).len(), 2);
    app.world_mut()
        .entity_mut(avatar)
        .insert(Visibility::Hidden);
    app.update();
    let _late = subscribe_to(&mut app, scene, "scene", 1);
    let events = drain(&mut app, &mut subscription);
    assert_eq!(events.len(), 1);
    assert_eq!(events[0]["type"], "USER_VISIBLE");
    assert_eq!(events[0]["visible"], false);
}

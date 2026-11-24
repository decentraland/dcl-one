use bevy::{app::PropagateSet, platform::collections::HashMap, prelude::*};
use common::{
    dynamics::{PLAYER_COLLIDER_HEIGHT, PLAYER_COLLIDER_RADIUS},
    inputs::CommonInputAction,
    rpc::{RpcCall, RpcCallEvent, RpcEventSender},
    sets::SceneSets,
    structs::{PlayerModifiers, PointerTargetType, PrimaryCamera, PrimaryUser},
};
use comms::{global_crdt::ForeignPlayer, Transport};
use dcl_component::{proto_components::sdk::components::ColliderLayer, SceneEntityId};
use input_manager::{InputManager, InputPriority};
use rapier3d_f64::{
    na::Isometry,
    prelude::{ColliderBuilder, Group, InteractionGroups, SharedShape},
};
use scene_material::highlight::Highlight;
use scene_runner::{
    update_scene::pointer_results::{AvatarColliders, PointerTarget},
    update_world::mesh_collider::ColliderId,
};
use serde_json::json;
use system_bridge::{AvatarModifierState, SystemApi};

use crate::hover_glow::{apply_hover_glow, set_glow_target, HoverGlow};

pub struct AvatarColliderPlugin;

impl Plugin for AvatarColliderPlugin {
    fn build(&self, app: &mut App) {
        app.init_state::<AvatarHighlighted>();
        app.init_resource::<PlayerClickedSenders>();

        app.add_systems(
            Update,
            (
                update_avatar_colliders.in_set(SceneSets::PostInit),
                (
                    clean_player_clicked_senders,
                    collect_player_clicked_senders.run_if(on_event::<RpcCallEvent>),
                    update_avatar_collider_actions.in_set(SceneSets::Input),
                    send_message_to_scene.run_if(in_state(AvatarHighlighted(true))),
                )
                    .chain(),
                apply_hover_glow
                    .in_set(SceneSets::PostLoop)
                    .before(PropagateSet::<Highlight>::default()),
                handle_avatar_modifier_requests,
            ),
        );
    }
}

fn update_avatar_colliders(
    mut colliders: ResMut<AvatarColliders>,
    foreign_players: Query<(Entity, &ForeignPlayer, &GlobalTransform)>,
    primary_player: Query<(Entity, &GlobalTransform), With<PrimaryUser>>,
    transports: Query<&Transport>,
) {
    let mut positions = foreign_players
        .iter()
        .filter(|(_, player, _)| player.has_pulse_transport(&transports))
        .map(|(e, f, t)| (f.scene_id, (e, t)))
        .collect::<HashMap<_, _>>();
    if let Ok((e, gt)) = primary_player.single() {
        positions.insert(SceneEntityId::PLAYER, (e, gt));
    }

    let remove = colliders
        .collider_data
        .iter()
        .filter(|id| !positions.contains_key(&id.entity))
        .cloned()
        .collect::<Vec<_>>();
    for id in remove {
        colliders.collider_data.remove_collider(&id);
        colliders.lookup.remove(&id);
    }

    for (id, (ent, transform)) in positions {
        let id = ColliderId {
            entity: id,
            name: None,
            index: 0,
        };
        if colliders.lookup.contains_key(&id) {
            let transform = transform.mul_transform(Transform::from_translation(
                PLAYER_COLLIDER_HEIGHT * 0.5 * Vec3::Y,
            ));
            colliders
                .collider_data
                .update_collider_transform(&id, &transform);
        } else {
            let collision_layers = if id.entity == SceneEntityId::PLAYER {
                ColliderLayer::ClPlayer as u32 | ColliderLayer::ClMainPlayer as u32
            } else {
                ColliderLayer::ClPlayer as u32
            };

            // collider didn't exist, make a new one
            let collider = ColliderBuilder::new(SharedShape::capsule_y(
                (PLAYER_COLLIDER_HEIGHT * 0.5 - PLAYER_COLLIDER_RADIUS) as f64,
                PLAYER_COLLIDER_RADIUS as f64,
            ))
            .position(Isometry::from_parts(
                (transform.translation() + PLAYER_COLLIDER_HEIGHT * 0.5 * Vec3::Y)
                    .as_dvec3()
                    .into(),
                Default::default(),
            ))
            .collision_groups(InteractionGroups {
                memberships: Group::from_bits_truncate(collision_layers),
                filter: Group::from_bits_truncate(collision_layers),
                test_mode: rapier3d_f64::prelude::InteractionTestMode::And,
            })
            .build();
            colliders
                .collider_data
                .set_collider(&id, collider, Some(ent));
            colliders.lookup.insert(id, ent);
        }
    }
}

#[derive(Default, Resource, Deref, DerefMut)]
struct PlayerClickedSenders(Vec<RpcEventSender>);

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash, States)]
struct AvatarHighlighted(bool);

fn clean_player_clicked_senders(mut senders: ResMut<PlayerClickedSenders>) {
    senders.retain(|s| !s.is_closed());
}

fn collect_player_clicked_senders(
    mut senders: ResMut<PlayerClickedSenders>,
    mut subscribe_events: EventReader<RpcCallEvent>,
) {
    // gather any event receivers
    for sender in subscribe_events.read().filter_map(|ev| match &ev.call {
        RpcCall::SubscribePlayerClicked { sender } => Some(sender),
        _ => None,
    }) {
        senders.push(sender.clone());
    }
}

fn update_avatar_collider_actions(
    mut commands: Commands,
    pointer_target: Res<PointerTarget>,
    profiles: Query<&PlayerModifiers, With<ForeignPlayer>>,
    mut glows: Query<&mut HoverGlow>,
    mut previous_target: Local<Option<Entity>>,
) {
    if previous_target.as_ref() != pointer_target.0.as_ref().map(|target| &target.container) {
        debug!(
            "Pointer target changed from {:?} to {:?}",
            previous_target.as_ref(),
            pointer_target.0.as_ref().map(|target| &target.container)
        );
        let maybe_old_target = previous_target.take();
        *previous_target = pointer_target.0.as_ref().map(|target| target.container);

        if let Some(old_target) = maybe_old_target {
            debug!("Fading out the glow of {}", old_target);
            set_glow_target(&mut commands, &mut glows, old_target, 0.0);
        }

        if let Some(target) = pointer_target.0.as_ref() {
            if target.ty == PointerTargetType::Avatar {
                let Ok(modifiers) = profiles.get(target.container) else {
                    return;
                };

                // check modifier
                if modifiers.hide_profile {
                    return;
                }

                // hilight meshes of target container
                debug!("Highlighting avatar {}", target.container);
                commands.set_state(AvatarHighlighted(true));
                set_glow_target(&mut commands, &mut glows, target.container, 1.0);
                return;
            }
        }

        commands.set_state(AvatarHighlighted(false));
    }
}

fn send_message_to_scene(
    pointer_target: Res<PointerTarget>,
    profiles: Query<&ForeignPlayer>,
    camera: Single<(&Camera, &GlobalTransform), With<PrimaryCamera>>,
    senders: Res<PlayerClickedSenders>,
    input_manager: InputManager,
) {
    let Some(target) = pointer_target.0.as_ref() else {
        return;
    };

    let Ok(player) = profiles.get(target.container) else {
        return;
    };

    if input_manager.just_down(CommonInputAction::IaPointer, InputPriority::Scene) {
        let camera_position = camera.1.translation();
        let direction = (target.position.unwrap() - camera_position).normalize();

        // send event
        let event = json!({
            "userId": format!("{:#x}", player.address),
            "ray": {
                "origin": { "x": camera_position.x, "y": camera_position.y, "z": -camera_position.z },
                "direction": { "x": direction.x, "y": direction.y, "z": -direction.z },
                "distance": target.distance.0
            }
        }).to_string();
        for sender in senders.iter() {
            let _ = sender.send(event.clone());
        }
    }
}

fn handle_avatar_modifier_requests(
    mut events: EventReader<SystemApi>,
    players: Query<(&ForeignPlayer, Option<&PlayerModifiers>)>,
    transports: Query<&Transport>,
) {
    for ev in events.read() {
        if let SystemApi::GetAvatarModifiers(sender) = ev {
            let response: Vec<AvatarModifierState> = players
                .iter()
                .filter_map(|(player, modifiers)| {
                    let hide_avatar = modifiers.is_some_and(|modifiers| modifiers.hide);
                    let hide_profile = modifiers.is_some_and(|modifiers| modifiers.hide_profile);
                    let pulse_absent = !player.has_pulse_transport(&transports);
                    (hide_avatar || hide_profile || pulse_absent).then(|| AvatarModifierState {
                        user_id: format!("{:#x}", player.address),
                        hide_avatar,
                        hide_profile,
                        pulse_absent,
                    })
                })
                .collect();
            sender.send(response);
        }
    }
}

#[cfg(test)]
mod tests {
    use common::{
        rpc::RpcResultSender,
        structs::{CurrentRealm, NoRenderApp, PreviewMode},
    };
    use comms::{
        chat_reaction::ChatReactionEvent,
        global_crdt::{
            CrdtContexts, GlobalCrdtPlugin, GlobalCrdtState, NetworkUpdate, PlayerMessage,
            PlayerUpdate,
        },
        scene_messages::SceneMessageTraffic,
        SceneRoomConnection, ServerSceneRooms, TransportType,
    };

    use super::*;

    #[test]
    fn pulse_departure_removes_the_avatar_from_picking_and_reports_absence() {
        let mut app = App::new();
        app.insert_resource(NoRenderApp)
            .init_resource::<Time>()
            .init_resource::<CurrentRealm>()
            .init_resource::<PreviewMode>()
            .init_resource::<SceneRoomConnection>()
            .init_resource::<ServerSceneRooms>()
            .init_resource::<SceneMessageTraffic>()
            .init_resource::<AvatarColliders>()
            .add_event::<RpcCallEvent>()
            .add_event::<ChatReactionEvent>()
            .add_event::<SystemApi>()
            .add_plugins(GlobalCrdtPlugin)
            .add_systems(
                PostUpdate,
                (update_avatar_colliders, handle_avatar_modifier_requests),
            );
        let context = app.world().resource::<CrdtContexts>().shared();
        let updates = app
            .world()
            .get::<GlobalCrdtState>(context)
            .unwrap()
            .get_sender();
        let mut spawn_transport = |transport_type| {
            let (sender, _) = tokio::sync::mpsc::channel(1);
            app.world_mut()
                .spawn(Transport {
                    transport_type,
                    sender,
                    control: None,
                    context,
                })
                .id()
        };
        let room = spawn_transport(TransportType::Livekit);
        let pulse = spawn_transport(TransportType::Pulse);
        let address = "0x1111111111111111111111111111111111111111"
            .parse()
            .unwrap();
        let join = |transport_id| {
            updates
                .try_send(
                    PlayerUpdate {
                        transport_id,
                        address,
                        message: PlayerMessage::Joined,
                    }
                    .into(),
                )
                .unwrap();
        };
        join(room);
        join(pulse);
        app.update();
        let player = app
            .world_mut()
            .query_filtered::<Entity, With<ForeignPlayer>>()
            .single(app.world())
            .unwrap();
        app.world_mut()
            .entity_mut(player)
            .insert(GlobalTransform::IDENTITY);

        let check = |app: &mut App, pulse_present: bool| {
            let (reply, mut response) = RpcResultSender::channel();
            app.world_mut()
                .send_event(SystemApi::GetAvatarModifiers(reply));
            app.update();
            let hit = app
                .world_mut()
                .resource_mut::<AvatarColliders>()
                .collider_data
                .cast_ray_nearest(
                    Vec3::new(0.0, PLAYER_COLLIDER_HEIGHT * 0.5, -3.0),
                    Vec3::Z,
                    6.0,
                    u32::MAX,
                    true,
                    false,
                    None,
                );
            assert_eq!(hit.is_some(), pulse_present);
            let modifiers = response.try_recv().unwrap();
            if pulse_present {
                assert!(modifiers.is_empty());
            } else {
                let [state] = modifiers.as_slice() else {
                    panic!("expected one absent peer, got {modifiers:?}")
                };
                assert_eq!(state.user_id, format!("{address:#x}"));
                assert!(state.pulse_absent);
            }
        };
        check(&mut app, true);

        updates
            .try_send(NetworkUpdate::PlayerLeft {
                transport_id: pulse,
                address,
            })
            .unwrap();
        check(&mut app, false);
        let retained = app.world().get::<ForeignPlayer>(player).unwrap();
        assert!(retained.transports.contains(&room));

        join(pulse);
        check(&mut app, true);
    }
}

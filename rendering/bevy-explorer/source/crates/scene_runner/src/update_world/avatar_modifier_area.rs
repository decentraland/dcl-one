use bevy::{
    platform::collections::{HashMap, HashSet},
    prelude::*,
};

use common::{
    dynamics::{PLAYER_COLLIDER_HEIGHT, PLAYER_COLLIDER_RADIUS},
    sets::SceneSets,
    structs::{
        ActiveAvatarArea, AttachPoints, AvatarControl, PermissionState, PlayerAddress,
        PlayerModifiers, PrimaryPlayerRes, PrimaryUser,
    },
};
use comms::{global_crdt::ForeignPlayer, BlockedOrBanned, Transport};
use dcl_component::{
    proto_components::sdk::components::{
        pb_input_modifier, AvatarControlType, AvatarModifierType, PbAvatarModifierArea,
        PbInputModifier,
    },
    SceneComponentId, SceneEntityId,
};
use dcl_types::interface::ComponentPosition;

use crate::{
    permissions::Permission, renderer_context::RendererSceneContext, ContainingScene, SceneEntity,
};

use super::AddCrdtInterfaceExt;

pub struct AvatarModifierAreaPlugin;

#[derive(Component, Debug)]
pub struct AvatarModifierArea(pub PbAvatarModifierArea);

impl From<PbAvatarModifierArea> for AvatarModifierArea {
    fn from(value: PbAvatarModifierArea) -> Self {
        Self(value)
    }
}

#[derive(Component, Debug, Clone, Default)]
pub struct InputModifier(pub Option<PbInputModifier>);

impl From<PbInputModifier> for InputModifier {
    fn from(value: PbInputModifier) -> Self {
        Self(Some(value))
    }
}

impl Plugin for AvatarModifierAreaPlugin {
    fn build(&self, app: &mut App) {
        app.add_crdt_lww_component::<PbAvatarModifierArea, AvatarModifierArea>(
            SceneComponentId::AVATAR_MODIFIER_AREA,
            ComponentPosition::Any,
        );

        app.add_crdt_lww_component::<PbInputModifier, InputModifier>(
            SceneComponentId::INPUT_MODIFIER,
            ComponentPosition::EntityOnly,
        );

        app.add_systems(
            Update,
            (
                update_avatar_modifier_area,
                manage_foreign_player_visibility,
            )
                .chain()
                .in_set(SceneSets::PostLoop),
        );

        app.add_observer(hide_nametag);
        app.add_observer(show_nametag);
    }
}

/// On a player whose nametag a scene's modifier area hides.
#[derive(Component)]
pub struct HiddenNametag;

pub(crate) fn update_avatar_modifier_area(
    mut commands: Commands,
    mut players: Query<
        (
            Entity,
            &GlobalTransform,
            Option<&ForeignPlayer>,
            Option<&PlayerAddress>,
            Option<&mut PlayerModifiers>,
            Has<HiddenNametag>,
        ),
        Or<(With<PrimaryUser>, With<ForeignPlayer>)>,
    >,
    containing_scene: ContainingScene,
    player_res: Res<PrimaryPlayerRes>,
    areas: Query<
        (
            Entity,
            &SceneEntity,
            Option<&AvatarModifierArea>,
            Option<&InputModifier>,
            &GlobalTransform,
        ),
        Or<(With<AvatarModifierArea>, With<InputModifier>)>,
    >,
    mut perms: Permission<Entity>,
    mut active_hide_areas: Local<HashMap<Entity, PermissionState>>,
    contexts: Query<&RendererSceneContext>,
    input_modifiers: Query<&InputModifier>,
) {
    let scenes = containing_scene.get_area(player_res.0, PLAYER_COLLIDER_RADIUS);

    // for every player
    for (player, gt, maybe_foreign, maybe_address, maybe_modifiers, has_hidden_nametag) in
        players.iter_mut()
    {
        let Some(mut modifiers) = maybe_modifiers else {
            commands
                .entity(player)
                .try_insert(PlayerModifiers::default());
            continue;
        };

        // reset overrides
        *modifiers = PlayerModifiers {
            areas: std::mem::take(&mut modifiers.areas),
            ..PlayerModifiers::default()
        };

        let player_position = gt.translation();
        let player_id = format!(
            "{:#x}",
            maybe_address.map(|address| address.0).unwrap_or_default()
        );

        let mut needs_hidden_nametag = false;

        // utility to check if player is within a camera area
        let player_in_area = |area: &AvatarModifierArea, transform: &GlobalTransform| -> bool {
            // check exclusions
            if area.0.exclude_ids.contains(&player_id) {
                return false;
            }

            // check bounds
            let (_, rotation, translation) = transform.to_scale_rotation_translation();
            let player_relative_position = rotation.inverse() * (player_position - translation);
            let area = area.0.area.unwrap_or_default().abs_vec_to_vec3() * 0.5
                + Vec3::new(
                    PLAYER_COLLIDER_RADIUS,
                    PLAYER_COLLIDER_HEIGHT,
                    PLAYER_COLLIDER_RADIUS,
                ) * if area.0.use_collider_range.unwrap_or(true) {
                    1.0
                } else {
                    0.0
                };
            player_relative_position.clamp(-area, area) == player_relative_position
        };

        // gather areas
        for (ent, scene_ent, maybe_area, _, transform) in areas.iter() {
            let Some(area) = maybe_area else {
                continue;
            };
            let current_index = modifiers
                .areas
                .iter()
                .enumerate()
                .find(|(_, ActiveAvatarArea { entity, .. })| ent == *entity)
                .map(|(ix, _)| ix);
            let in_area = scenes.contains(&scene_ent.root) && player_in_area(area, transform);

            if in_area == current_index.is_some() {
                continue;
            }

            match current_index {
                // remove if no longer in area
                Some(index) => {
                    modifiers.areas.remove(index);
                }
                // add at end if newly entered
                None => modifiers.areas.push(ActiveAvatarArea {
                    entity: ent,
                    allow_locomotion: PermissionState::NotRequested,
                }),
            }
        }

        // lastly add input modifier
        if maybe_foreign.is_none() {
            let input_modifier = scenes
                .iter()
                .flat_map(|scene| {
                    let Ok(ctx) = contexts.get(*scene) else {
                        return None;
                    };

                    ctx.bevy_entity(SceneEntityId::PLAYER)
                        .filter(|player_ent| input_modifiers.get(*player_ent).is_ok())
                })
                .next();

            let modifier_present = input_modifier.as_ref().is_some_and(|ent| {
                modifiers
                    .areas
                    .iter()
                    .any(|ActiveAvatarArea { entity, .. }| ent == entity)
            });

            if !modifier_present {
                modifiers
                    .areas
                    .extend(input_modifier.map(|e| ActiveAvatarArea {
                        entity: e,
                        allow_locomotion: PermissionState::NotRequested,
                    }));
            }
        }

        // remove destroyed areas
        modifiers
            .areas
            .retain(|ActiveAvatarArea { entity, .. }| areas.get(*entity).is_ok());

        // for each modifier area the player is within (starting from oldest)
        let mut areas_clone = modifiers.areas.clone();
        for active_area in areas_clone.iter_mut() {
            let (_, scene_ent, maybe_area, maybe_modifier, _) =
                areas.get(active_area.entity).unwrap();

            if maybe_area.is_some_and(|area| !area.0.modifiers.is_empty()) {
                let area = maybe_area.unwrap();
                let permit = match active_hide_areas
                    .get(&scene_ent.root)
                    .unwrap_or(&PermissionState::NotRequested)
                {
                    PermissionState::Resolved(true) => true,
                    PermissionState::NotRequested => {
                        perms.check(
                            common::structs::PermissionType::HideAvatarsNametags,
                            scene_ent.root,
                            scene_ent.root,
                            None,
                            true,
                        );
                        active_hide_areas.insert(scene_ent.root, PermissionState::Pending);
                        false
                    }
                    _ => false,
                };

                if permit {
                    // apply modifiers
                    let acts = [
                        AvatarModifierType::AmtHideAvatars,
                        AvatarModifierType::AmtDisablePassports,
                        AvatarModifierType::AmtHideNametags,
                    ]
                    .iter()
                    .any(|modifier| area.0.modifiers.contains(&(*modifier as i32)));
                    if acts
                        && maybe_foreign.is_none()
                        && !modifiers.hide_scenes.contains(&scene_ent.root)
                    {
                        modifiers.hide_scenes.push(scene_ent.root);
                    }
                    modifiers.hide |= area
                        .0
                        .modifiers
                        .contains(&(AvatarModifierType::AmtHideAvatars as i32));
                    modifiers.hide_profile |= area
                        .0
                        .modifiers
                        .contains(&(AvatarModifierType::AmtDisablePassports as i32));
                    needs_hidden_nametag |= area
                        .0
                        .modifiers
                        .contains(&(AvatarModifierType::AmtHideNametags as i32));
                }
            }

            if let Some(movement) = maybe_area.and_then(|area| area.0.movement_settings.as_ref()) {
                let permit = maybe_foreign.is_some()
                    || match active_area.allow_locomotion {
                        PermissionState::Resolved(true) => true,
                        PermissionState::NotRequested => {
                            perms.check(
                                common::structs::PermissionType::SetLocomotion,
                                scene_ent.root,
                                active_area.entity,
                                None,
                                false,
                            );
                            active_area.allow_locomotion = PermissionState::Pending;
                            false
                        }
                        _ => false,
                    };

                if permit {
                    // listed only while it changes something: an area with nothing set does not
                    let changes = movement.control_mode.is_some()
                        || movement.run_speed.is_some()
                        || movement.friction.is_some()
                        || movement.gravity.is_some()
                        || movement.jump_height.is_some()
                        || movement.max_fall_speed.is_some()
                        || movement.turn_speed.is_some()
                        || movement.walk_speed.is_some()
                        || !movement.allow_weighted_movement.unwrap_or(true);
                    if changes
                        && maybe_foreign.is_none()
                        && !modifiers.locomotion_scenes.contains(&scene_ent.root)
                    {
                        modifiers.locomotion_scenes.push(scene_ent.root);
                    }
                    if movement.control_mode.is_some() {
                        modifiers.control_type = Some(match movement.control_mode() {
                            AvatarControlType::CctNone => AvatarControl::None,
                            AvatarControlType::CctRelative => AvatarControl::Relative,
                            AvatarControlType::CctTank => AvatarControl::Tank,
                        })
                    }

                    modifiers.run_speed = movement.run_speed.or(modifiers.run_speed);
                    modifiers.friction = movement.friction.or(modifiers.friction);
                    modifiers.gravity = movement.gravity.or(modifiers.gravity);
                    modifiers.jump_height = movement.jump_height.or(modifiers.jump_height);
                    modifiers.fall_speed = movement.max_fall_speed.or(modifiers.fall_speed);
                    modifiers.turn_speed = movement.turn_speed.or(modifiers.turn_speed);
                    modifiers.walk_speed = movement.walk_speed.or(modifiers.walk_speed);
                    modifiers.block_run |= !(movement.allow_weighted_movement.unwrap_or(true));
                    modifiers.block_walk |= !(movement.allow_weighted_movement.unwrap_or(true));
                }
            }

            if let Some(pb_input_modifier::Mode::Standard(modifier)) = maybe_modifier
                .and_then(|m| m.0.as_ref())
                .and_then(|m| m.mode.as_ref())
            {
                let permit = maybe_foreign.is_some()
                    || match active_area.allow_locomotion {
                        PermissionState::Resolved(true) => true,
                        PermissionState::NotRequested => {
                            perms.check(
                                common::structs::PermissionType::SetLocomotion,
                                scene_ent.root,
                                active_area.entity,
                                None,
                                false,
                            );
                            active_area.allow_locomotion = PermissionState::Pending;
                            false
                        }
                        _ => false,
                    };

                if permit {
                    // a scene lifts its block by setting every flag off, which keeps the
                    // component: listed only while one is on
                    let blocks = modifier.disable_all()
                        || modifier.disable_walk()
                        || modifier.disable_run()
                        || modifier.disable_jump()
                        || modifier.disable_emote();
                    if blocks
                        && maybe_foreign.is_none()
                        && !modifiers.locomotion_scenes.contains(&scene_ent.root)
                    {
                        modifiers.locomotion_scenes.push(scene_ent.root);
                    }
                    modifiers.block_all |= modifier.disable_all();
                    modifiers.block_run |= modifier.disable_all() || modifier.disable_run();
                    modifiers.block_walk |= modifier.disable_all() || modifier.disable_walk();
                    modifiers.block_jog |= modifier.disable_all() || modifier.disable_jog();
                    modifiers.block_jump |= modifier.disable_all() || modifier.disable_jump();
                    modifiers.block_double_jump |=
                        modifier.disable_all() || modifier.disable_double_jump();
                    modifiers.block_gliding |= modifier.disable_all() || modifier.disable_gliding();
                    modifiers.block_emote |= modifier.disable_all() || modifier.disable_emote();
                }
            }
        }
        if maybe_foreign.is_none() {
            let allowed_areas = perms
                .drain_success(common::structs::PermissionType::SetLocomotion)
                .collect::<HashSet<_>>();
            if !allowed_areas.is_empty() {
                for area in areas_clone.iter_mut() {
                    if allowed_areas.contains(&area.entity) {
                        area.allow_locomotion = PermissionState::Resolved(true);
                    }
                }
            }

            let denied_areas = perms
                .drain_fail(common::structs::PermissionType::SetLocomotion)
                .collect::<HashSet<_>>();
            if !denied_areas.is_empty() {
                for area in areas_clone.iter_mut() {
                    if denied_areas.contains(&area.entity) {
                        area.allow_locomotion = PermissionState::Resolved(false);
                    }
                }
            }
        }

        modifiers.areas = areas_clone;

        if has_hidden_nametag != needs_hidden_nametag {
            let mut cmds = commands.entity(player);
            if has_hidden_nametag {
                cmds.try_remove::<HiddenNametag>();
            } else {
                cmds.try_insert(HiddenNametag);
            }
        }

        debug!("modifiers: {modifiers:?}");
    }

    for allowed in perms
        .drain_success(common::structs::PermissionType::HideAvatarsNametags)
        .collect::<HashSet<_>>()
    {
        active_hide_areas.insert(allowed, PermissionState::Resolved(true));
    }
    for allowed in perms
        .drain_fail(common::structs::PermissionType::HideAvatarsNametags)
        .collect::<HashSet<_>>()
    {
        active_hide_areas.insert(allowed, PermissionState::Resolved(false));
    }
    active_hide_areas.retain(|ent, _| scenes.contains(ent));
}

// The primary user's visibility is managed by user_input::manage_player_visibility.
// Pulse supplies the avatar pose. Other transports can keep the player in their rosters
// for voice and room messages while the avatar and its nametag are hidden.
fn manage_foreign_player_visibility(
    mut players: Query<(
        &mut Visibility,
        &ForeignPlayer,
        &PlayerModifiers,
        Has<BlockedOrBanned>,
    )>,
    transports: Query<&Transport>,
) {
    for (mut visibility, player, modifiers, moderated) in &mut players {
        let visible = !moderated && !modifiers.hide && player.has_pulse_transport(&transports);
        visibility.set_if_neq(if visible {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        });
    }
}

fn hide_nametag(
    trigger: Trigger<OnAdd, HiddenNametag>,
    attach_points: Query<&AttachPoints, Or<(With<PrimaryUser>, With<ForeignPlayer>)>>,
    mut visibilities: Query<&mut Visibility>,
) {
    let entity = trigger.target();
    let Ok(attach_points) = attach_points.get(entity) else {
        return;
    };

    let Ok(mut visibility) = visibilities.get_mut(attach_points.nametag) else {
        return;
    };
    *visibility = Visibility::Hidden;
}

fn show_nametag(
    trigger: Trigger<OnRemove, HiddenNametag>,
    attach_points: Query<&AttachPoints, Or<(With<PrimaryUser>, With<ForeignPlayer>)>>,
    mut visibilities: Query<&mut Visibility>,
) {
    let entity = trigger.target();
    let Ok(attach_points) = attach_points.get(entity) else {
        return;
    };

    let Ok(mut visibility) = visibilities.get_mut(attach_points.nametag) else {
        return;
    };
    *visibility = Visibility::Inherited;
}

#[cfg(test)]
mod tests {
    use common::{
        rpc::RpcCallEvent,
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
    use dcl_component::proto_components::kernel::comms::rfc4;
    use system_bridge::SystemApi;

    use super::*;

    #[test]
    fn a_pulse_departure_hides_the_avatar_until_pulse_presence_returns() {
        let mut app = App::new();
        app.insert_resource(NoRenderApp)
            .init_resource::<Time>()
            .init_resource::<CurrentRealm>()
            .init_resource::<PreviewMode>()
            .init_resource::<SceneRoomConnection>()
            .init_resource::<ServerSceneRooms>()
            .init_resource::<SceneMessageTraffic>()
            .add_event::<RpcCallEvent>()
            .add_event::<ChatReactionEvent>()
            .add_event::<SystemApi>()
            .add_plugins(GlobalCrdtPlugin)
            .add_systems(PostUpdate, manage_foreign_player_visibility);
        let context = app.world().resource::<CrdtContexts>().shared();
        let sender = app
            .world()
            .get::<GlobalCrdtState>(context)
            .unwrap()
            .get_sender();
        let mut transport = |transport_type| {
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
        let livekit = transport(TransportType::Livekit);
        let websocket = transport(TransportType::WebsocketRoom);
        let pulse = transport(TransportType::Pulse);
        let address = "0x1111111111111111111111111111111111111111"
            .parse()
            .unwrap();
        let send = |transport_id, message| {
            sender
                .try_send(
                    PlayerUpdate {
                        transport_id,
                        address,
                        message,
                    }
                    .into(),
                )
                .unwrap();
        };

        let join_pulse = |position_x, timestamp| {
            send(pulse, PlayerMessage::Joined);
            send(
                pulse,
                PlayerMessage::Movement {
                    movement: Box::new(rfc4::Movement {
                        position_x,
                        ..Default::default()
                    }),
                    teleport: true,
                    timestamp,
                },
            );
        };

        send(livekit, PlayerMessage::Joined);
        send(websocket, PlayerMessage::Joined);
        join_pulse(1.0, 1.0);
        app.update();
        let player = app
            .world_mut()
            .query_filtered::<Entity, With<ForeignPlayer>>()
            .single(app.world())
            .unwrap();
        app.world_mut()
            .entity_mut(player)
            .insert(PlayerModifiers::default());
        app.update();
        let visibility = |app: &App| *app.world().get::<Visibility>(player).unwrap();
        assert_eq!(visibility(&app), Visibility::Inherited);
        let initial = app.world().get::<ForeignPlayer>(player).unwrap();
        assert!(initial.transports.contains(&pulse));
        assert!(initial.transports.contains(&livekit));
        assert!(initial.transports.contains(&websocket));

        sender
            .try_send(NetworkUpdate::PlayerLeft {
                transport_id: pulse,
                address,
            })
            .unwrap();
        app.update();
        let retained = app.world().get::<ForeignPlayer>(player).unwrap();
        assert!(!retained.transports.contains(&pulse));
        assert!(retained.transports.contains(&livekit));
        assert!(retained.transports.contains(&websocket));
        assert_eq!(visibility(&app), Visibility::Hidden);

        app.world_mut().entity_mut(player).insert(BlockedOrBanned);
        join_pulse(7.0, 2.0);
        app.update();
        assert_eq!(visibility(&app), Visibility::Hidden);

        app.world_mut()
            .entity_mut(player)
            .remove::<BlockedOrBanned>();
        app.world_mut()
            .get_mut::<PlayerModifiers>(player)
            .unwrap()
            .hide = true;
        app.update();
        assert_eq!(visibility(&app), Visibility::Hidden);

        app.world_mut()
            .get_mut::<PlayerModifiers>(player)
            .unwrap()
            .hide = false;
        app.update();
        assert_eq!(visibility(&app), Visibility::Inherited);
    }

    /// The two systems on a player standing in the parcel scene, a step from the neighbour's
    /// parcel, with `area` belonging to the neighbour and reaching over the border. Returns
    /// what is in force after a few frames, and whether the player's nametag is hidden.
    fn in_force_with_a_neighbours_area(
        area: PbAvatarModifierArea,
    ) -> (Vec<(String, common::structs::PermissionType)>, bool) {
        use crate::{
            initialize_scene::{LiveScenes, PointerResult, PortableScenes, ScenePointers},
            permissions::{publish_permissions_in_force, PermissionManager, PermissionsInForce},
        };
        use common::structs::{AppConfig, PermissionUsed, PrimaryCamera, PrimaryUser};
        use dcl_types::SceneId;

        let mut app = App::new();
        app.init_resource::<ScenePointers>()
            .init_resource::<LiveScenes>()
            .init_resource::<PortableScenes>()
            .init_resource::<AppConfig>()
            .init_resource::<CurrentRealm>()
            .init_resource::<PermissionManager>()
            .init_resource::<PermissionsInForce>()
            .add_event::<PermissionUsed>()
            .add_systems(
                Update,
                (update_avatar_modifier_area, publish_permissions_in_force).chain(),
            );
        let at = GlobalTransform::from_xyz(15.9, 0.0, -8.0);
        let player = app.world_mut().spawn((PrimaryUser::default(), at)).id();
        app.insert_resource(PrimaryPlayerRes(player));
        app.world_mut().spawn(PrimaryCamera::default());

        let mut scene = |hash: &str, parcel: IVec2| {
            let scene = app.world_mut().spawn_empty().id();
            let context = RendererSceneContext::new(
                SceneId::DUMMY,
                hash.to_owned(),
                "storage_root".to_owned(),
                false,
                0,
                "title".to_owned(),
                parcel,
                HashSet::from_iter([parcel]),
                vec![],
                Default::default(),
                vec![],
                scene,
                0.0,
                false,
                "sdk_version",
                false,
                false,
            );
            app.world_mut().entity_mut(scene).insert(context);
            app.world_mut()
                .resource_mut::<LiveScenes>()
                .scenes
                .insert(hash.to_owned(), scene);
            app.world_mut().resource_mut::<ScenePointers>().insert(
                parcel,
                PointerResult::Exists {
                    hash: hash.to_owned(),
                    urn: None,
                },
            );
            scene
        };
        scene("parcel", IVec2::ZERO);
        let neighbour = scene("neighbour", IVec2::new(1, 0));
        app.world_mut().spawn((
            SceneEntity {
                root: neighbour,
                scene_id: SceneId::DUMMY,
                id: SceneEntityId::from_proto_u32(600),
            },
            AvatarModifierArea(PbAvatarModifierArea {
                area: Some(dcl_component::proto_components::common::Vector3 {
                    x: 4.0,
                    y: 4.0,
                    z: 4.0,
                }),
                ..area
            }),
            at,
        ));

        for _ in 0..4 {
            app.update();
        }
        (
            app.world().resource::<PermissionsInForce>().0.clone(),
            app.world().entity(player).contains::<HiddenNametag>(),
        )
    }

    #[test]
    fn a_scenes_input_modifier_blocks_jogging_air_jumps_and_gliding() {
        use crate::{
            initialize_scene::{LiveScenes, PointerResult, PortableScenes, ScenePointers},
            permissions::PermissionManager,
        };
        use common::structs::{AppConfig, PermissionUsed};
        use dcl_component::proto_components::sdk::components::pb_input_modifier::StandardInput;
        use dcl_types::SceneId;

        let mut app = App::new();
        app.init_resource::<ScenePointers>()
            .init_resource::<LiveScenes>()
            .init_resource::<PortableScenes>()
            .init_resource::<AppConfig>()
            .init_resource::<CurrentRealm>()
            .init_resource::<PermissionManager>()
            .add_event::<PermissionUsed>()
            .add_systems(Update, update_avatar_modifier_area);
        let player = app
            .world_mut()
            .spawn((
                PrimaryUser::default(),
                GlobalTransform::from_xyz(8.0, 0.0, -8.0),
            ))
            .id();
        app.insert_resource(PrimaryPlayerRes(player));

        // the scene the player stands in, with an input modifier on its player entity
        let scene = app.world_mut().spawn_empty().id();
        let scene_player = app
            .world_mut()
            .spawn((
                SceneEntity {
                    root: scene,
                    scene_id: SceneId::DUMMY,
                    id: SceneEntityId::PLAYER,
                },
                InputModifier(Some(PbInputModifier {
                    mode: Some(pb_input_modifier::Mode::Standard(StandardInput {
                        disable_jog: Some(true),
                        disable_double_jump: Some(true),
                        disable_gliding: Some(true),
                        ..Default::default()
                    })),
                })),
                GlobalTransform::default(),
            ))
            .id();
        let mut context = RendererSceneContext::new(
            SceneId::DUMMY,
            "parcel".to_owned(),
            "storage_root".to_owned(),
            false,
            0,
            "title".to_owned(),
            IVec2::ZERO,
            HashSet::from_iter([IVec2::ZERO]),
            vec![],
            Default::default(),
            vec![],
            scene,
            0.0,
            false,
            "sdk_version",
            false,
            false,
        );
        context.associate_bevy_entity(SceneEntityId::PLAYER, scene_player);
        app.world_mut().entity_mut(scene).insert(context);
        app.world_mut()
            .resource_mut::<LiveScenes>()
            .scenes
            .insert("parcel".to_owned(), scene);
        app.world_mut().resource_mut::<ScenePointers>().insert(
            IVec2::ZERO,
            PointerResult::Exists {
                hash: "parcel".to_owned(),
                urn: None,
            },
        );

        for _ in 0..4 {
            app.update();
        }
        let modifiers = app.world().get::<PlayerModifiers>(player).unwrap();
        assert!(modifiers.block_jog && modifiers.block_double_jump && modifiers.block_gliding);
        assert!(!(modifiers.block_walk || modifiers.block_run || modifiers.block_jump));
    }

    #[test]
    fn a_neighbours_nametag_area_is_in_force_under_the_neighbour() {
        use common::structs::PermissionType;
        // the neighbour's area hides nametags only
        let (in_force, nametag_hidden) = in_force_with_a_neighbours_area(PbAvatarModifierArea {
            modifiers: vec![AvatarModifierType::AmtHideNametags as i32],
            ..Default::default()
        });
        assert!(nametag_hidden);
        assert_eq!(
            in_force,
            [("neighbour".to_owned(), PermissionType::HideAvatarsNametags)]
        );
    }

    #[test]
    fn movement_settings_are_in_force_only_while_they_change_something() {
        use common::structs::PermissionType;
        use dcl_component::proto_components::sdk::components::AvatarMovementSettings;
        // granted, and nothing set: nothing acts on the player
        let (in_force, _) = in_force_with_a_neighbours_area(PbAvatarModifierArea {
            movement_settings: Some(AvatarMovementSettings::default()),
            ..Default::default()
        });
        assert_eq!(in_force, []);

        let (in_force, _) = in_force_with_a_neighbours_area(PbAvatarModifierArea {
            movement_settings: Some(AvatarMovementSettings {
                run_speed: Some(1.0),
                ..Default::default()
            }),
            ..Default::default()
        });
        assert_eq!(
            in_force,
            [("neighbour".to_owned(), PermissionType::SetLocomotion)]
        );
    }
}

pub(crate) mod diagnostics;

use bevy::{
    ecs::{schedule::ScheduleConfigs, system::ScheduleSystem},
    math::Vec3Swizzles,
    prelude::*,
};
use common::{
    rpc::{RpcCall, RpcCallEvent, RpcResultSender},
    structs::{AvatarDynamicState, PermissionType, PrimaryUser, ReturnedToWorld, SpawnPoint},
};
use comms::global_crdt::ForeignPlayer;
use ipfs::{ChangeRealmEvent, RealmInitialLocation};
use rand::{seq::SliceRandom, Rng};
use scene_runner::{
    initialize_scene::{
        LiveScenes, PointerResult, SceneHash, SceneLoading, ScenePointers, SuperUserScene,
        PARCEL_SIZE,
    },
    load_timing::scene_has_loaded,
    permissions::Permission,
    renderer_context::RendererSceneContext,
    update_world::mesh_collider::SceneColliderData,
    OutOfWorld,
};
use wallet::Wallet;

// (parcel, realm, spawn point name, response, report a realm change's outcome)
type TeleportAction = (
    Option<IVec2>,
    Option<String>,
    Option<String>,
    RpcResultSender<Result<(), String>>,
    bool,
);

/// The longest spawn point name a teleport may ask for, in bytes.
const MAX_SPAWN_POINT_NAME: usize = 128;

fn spawn_point_name_too_long() -> String {
    format!("spawn point name is longer than {MAX_SPAWN_POINT_NAME} bytes")
}

/// On the player: the last landing of theirs that looked for a spawn point by name, and whether
/// its scene had one of that name. The debug panel's Teleport section shows it.
#[derive(Component, Debug, PartialEq)]
pub(crate) struct LastNamedLanding {
    name: String,
    found: bool,
}

/// The teleport and the landing, in the order they need. A landing is placed before a new
/// teleport moves the player: the move is immediate and the new landing's `OutOfWorld` is
/// deferred, so the other order would place the old landing, by its spawn point, at the new
/// position.
pub(crate) fn landing_systems() -> ScheduleConfigs<ScheduleSystem> {
    (
        teleport_player.after(super::change_realm),
        handle_out_of_world.before(teleport_player),
    )
        .into_configs()
}

pub fn teleport_player(
    mut commands: Commands,
    mut events: EventReader<RpcCallEvent>,
    mut player: Query<(Entity, &mut Transform, &mut AvatarDynamicState), With<PrimaryUser>>,
    mut perms: Permission<TeleportAction>,
    mut realm_target: ResMut<RealmInitialLocation>,
    super_user: Query<(), With<SuperUserScene>>,
) {
    let mut actions: Vec<TeleportAction> = Vec::new();

    for (scene, to, realm, spawn_point, response) in events.read().filter_map(|ev| match &ev.call {
        RpcCall::TeleportPlayer {
            to,
            realm,
            spawn_point,
            response,
        } => Some((
            ev.origin.scene(),
            *to,
            realm.clone(),
            spawn_point.clone(),
            response.clone(),
        )),
        _ => None,
    }) {
        // the name comes from scenes, and is kept on the landing and logged: it is bounded
        // here, where every request arrives, before anyone is asked for a permission
        if spawn_point
            .as_ref()
            .is_some_and(|name| name.len() > MAX_SPAWN_POINT_NAME)
        {
            response.send(Err(spawn_point_name_too_long()));
            continue;
        }
        let parcel = to.map(|to| format!("({},{})", to.x, to.y));
        let (ty, detail) = match (&realm, parcel) {
            (Some(realm), Some(parcel)) => {
                (PermissionType::ChangeRealm, format!("{realm} {parcel}"))
            }
            (Some(realm), None) => (PermissionType::ChangeRealm, realm.clone()),
            (None, Some(parcel)) => (PermissionType::Teleport, parcel),
            (None, None) => {
                response.send(Err("teleport needs a parcel or a realm".to_owned()));
                continue;
            }
        };
        // system requests reply to their requester; a scene is gone once its realm change lands,
        // so its player is told in the console, unless it is the HUD's own bridge scene
        let Some(scene) = scene else {
            actions.push((to, realm, spawn_point, response, false));
            continue;
        };
        let report = !super_user.contains(scene);
        perms.check(
            ty,
            scene,
            (to, realm, spawn_point, response, report),
            Some(detail),
            false,
        );
    }

    actions.extend(perms.drain_success(PermissionType::Teleport));
    actions.extend(perms.drain_success(PermissionType::ChangeRealm));

    for (to, realm, spawn_point, response, report) in actions {
        if let Some(realm) = realm {
            // A realm change (a full reconnect, even to the realm we are in — same as changeRealm),
            // landing on the parcel once it is live, or on the realm's default spawn without one.
            // The parcel can't be applied now: it would resolve against the parcel grid of the
            // realm being left.
            debug!("teleport -> {to:?} in {realm}");
            *realm_target = match to {
                Some(parcel) => RealmInitialLocation::Parcel {
                    parcel,
                    spawn_point,
                },
                None => RealmInitialLocation::Base { spawn_point },
            };
            // answered once the realm is actually set (or failed, keeping the player where they are)
            commands.send_event(ChangeRealmEvent {
                new_realm: realm,
                content_server_override: None,
                response,
                report,
            });
            continue;
        }

        // Neither: already answered above (a system request without a scene is never built that way).
        let Some(to) = to else {
            response.send(Err("teleport needs a parcel or a realm".to_owned()));
            continue;
        };

        let Ok((ent, mut transform, mut dynamic_state)) = player.single_mut() else {
            warn!("player doesn't exist?!");
            response.send(Err("Something went wrong".into()));
            continue;
        };

        transform.translation.x = to.x as f32 * 16.0 + 8.0;
        transform.translation.z = -to.y as f32 * 16.0 - 8.0;
        dynamic_state.velocity = Vec3::ZERO;
        if let Ok(mut commands) = commands.get_entity(ent) {
            commands.try_insert(OutOfWorld { spawn_point });
        }

        debug!("teleport -> none");
        *realm_target = RealmInitialLocation::None;

        response.send(Ok(()));
        info!("teleported to {to}");
    }

    for (_, _, _, response, _) in perms.drain_fail(PermissionType::Teleport) {
        response.send(Err("User declined".to_owned()))
    }
    for (_, _, _, response, _) in perms.drain_fail(PermissionType::ChangeRealm) {
        response.send(Err("User declined".to_owned()))
    }
}

/// The scene's first spawn point of this name, both compared in lowercase.
fn named_spawn_point<'a>(spawn_points: &'a [SpawnPoint], name: &str) -> Option<&'a SpawnPoint> {
    let name = name.to_lowercase();
    spawn_points.iter().find(|spawn_point| {
        spawn_point
            .name
            .as_deref()
            .is_some_and(|own| own.to_lowercase() == name)
    })
}

fn default_spawn_points(spawn_points: &[SpawnPoint]) -> Vec<&SpawnPoint> {
    let defaults = spawn_points
        .iter()
        .filter(|spawn_point| spawn_point.default)
        .collect::<Vec<_>>();
    if defaults.is_empty() {
        spawn_points.iter().collect()
    } else {
        defaults
    }
}

/// Lets the player into the world. A landing that named a spawn point keeps whether its scene
/// had it.
fn end_landing(commands: &mut Commands, player: Entity, out_of_world: &OutOfWorld, found: bool) {
    let mut player = commands.entity(player);
    player.remove::<OutOfWorld>();
    if let Some(name) = &out_of_world.spawn_point {
        player.insert(LastNamedLanding {
            name: name.clone(),
            found,
        });
    }
}

pub(crate) fn handle_out_of_world(
    mut commands: Commands,
    mut scenes: Query<
        (
            Option<&RendererSceneContext>,
            Option<&SceneLoading>,
            Option<&mut SceneColliderData>,
        ),
        With<SceneHash>,
    >,
    mut player: Query<(Entity, &mut Transform, &OutOfWorld), With<PrimaryUser>>,
    pointers: Res<ScenePointers>,
    live_scenes: Res<LiveScenes>,
    foreign_players: Query<&GlobalTransform, With<ForeignPlayer>>,
    wallet: Res<Wallet>,
    bans: comms::SceneBanGate,
    mut returned: EventWriter<ReturnedToWorld>,
) {
    let Ok((player, mut t, out_of_world)) = player.single_mut() else {
        return;
    };

    debug!("out of world!");

    if wallet.address().is_none() {
        debug!("waiting for connection");
        return;
    }

    let parcel = (t.translation.xz() * Vec2::new(1.0, -1.0) / PARCEL_SIZE)
        .floor()
        .as_ivec2();

    let hash = match pointers.get(parcel) {
        Some(PointerResult::Exists { hash, .. }) if !bans.is_banned(hash) => hash,
        Some(PointerResult::Exists { .. }) => {
            // A refused scene deliberately has no live root. Arrival must not wait for it.
            end_landing(&mut commands, player, out_of_world, false);
            returned.write(ReturnedToWorld::SceneReady);
            return;
        }
        Some(PointerResult::Nothing) => {
            debug!("scene {parcel} doesn't exist, returning to world");
            debug!("everything: {:?}", pointers);
            end_landing(&mut commands, player, out_of_world, false);
            returned.write(ReturnedToWorld::SceneReady);
            return;
        }
        None => {
            // we don't know yet, the scene isn't loaded
            debug!("waiting for scene to resolve");
            return;
        }
    };

    let Some(scene) = live_scenes.scenes.get(hash) else {
        debug!("scene resolved but not spawned");
        return;
    };

    let (maybe_context, maybe_loadstate, maybe_collider_data) = scenes.get_mut(*scene).unwrap();

    // A failed load is terminal, with or without a context: a scene whose main js could not be
    // fetched has one and never ticks past its spawn tick.
    if matches!(maybe_loadstate, Some(SceneLoading::Failed)) {
        debug!("failed, returning to world");
        end_landing(&mut commands, player, out_of_world, false);
        returned.write(ReturnedToWorld::SceneLoadFailed);
        return;
    }

    if let Some(context) = maybe_context {
        // A frozen scene (inspector /freeze_scene, or the editor-mode auto-freeze at tick 3) is as
        // loaded as it's going to get — it never advances its tick or clears `blocked` to become
        // "ready" on its own, so don't keep the player out-of-world behind the loading screen. Do
        // still wait for gltfs though: they keep processing renderer-side while the scene is frozen,
        // and releasing early would drop the player in before the ground colliders exist.
        // A broken scene (hung past the not-responding timeout, errored, or unreachable) will
        // never become ready; stop holding the player behind the loading screen and let them
        // into the world. `scene_has_loaded` is that rule, the one the settling commands stop
        // waiting on. An inspected scene never gates the player at all: it's a debugging
        // session, and it may sit at a breakpoint (or paused before its first tick) forever.
        if !scene_has_loaded(context) && !context.inspected {
            debug!("scene not ready");
        } else {
            if context.broken() {
                debug!("scene broken, returning to world");
            }
            debug!(
                "ready, returning to world (set spawn here) tick: {}",
                context.tick_number
            );

            let other_positions = foreign_players
                .iter()
                .map(|gt| gt.translation())
                .collect::<Vec<_>>();
            let base_position =
                Vec3::new(context.base.x as f32, 0.0, -context.base.y as f32) * PARCEL_SIZE;

            let rng = &mut rand::thread_rng();
            let mut best_distance = 0.0;
            let mut best_position = Vec3::new(
                rng.gen_range(0.0..PARCEL_SIZE),
                1000.0,
                -rng.gen_range(0.0..PARCEL_SIZE),
            ) + base_position;
            best_position.y = 1000.0
                - maybe_collider_data
                    .and_then(|mut cd| cd.get_ground(best_position))
                    .map(|(h, _)| h)
                    .unwrap_or(1000.0);
            let mut count = 100;

            // the spawn point this landing names, when the scene has it; without one, spawn
            // points marked `default` take precedence, and a scene without any uses them all
            let named = out_of_world.spawn_point.as_ref().and_then(|name| {
                let found = named_spawn_point(&context.spawn_points, name);
                if found.is_none() {
                    debug!(
                        "scene {} has no spawn point named {name:?}, using its default",
                        context.hash
                    );
                }
                found
            });
            let spawn_points = match named {
                Some(named) => vec![named],
                None => default_spawn_points(&context.spawn_points),
            };

            if !spawn_points.is_empty() {
                while best_distance < 0.75 && count > 0 {
                    let spawn_point = spawn_points.choose(rng).unwrap();
                    let aabb = spawn_point.position.bounding_box();
                    let position = base_position
                        + Vec3::new(
                            rng.gen_range(aabb.0.x..=aabb.1.x),
                            rng.gen_range(aabb.0.y..=aabb.1.y),
                            -rng.gen_range(aabb.0.z..=aabb.1.z),
                        );
                    let distance = other_positions
                        .iter()
                        .fold(0.75, |d, other| f32::min(d, (position - *other).length()));
                    if distance > best_distance {
                        best_distance = distance;
                        best_position = position;
                    }

                    count -= 1;
                }
            }

            debug!("chose {best_position}");
            t.translation = best_position;
            end_landing(&mut commands, player, out_of_world, named.is_some());
            returned.write(if context.broken() {
                ReturnedToWorld::SceneBroken
            } else {
                ReturnedToWorld::SceneReady
            });
        }
        return;
    }

    match maybe_loadstate {
        Some(_) => {
            debug!("scene not loaded");
        }
        None => {
            panic!("no context or loadstate?!");
        }
    }
}

#[cfg(test)]
mod tests {
    use bevy::{ecs::event::Events, ecs::system::RunSystemOnce, platform::collections::HashSet};
    use common::{
        rpc::RpcOrigin,
        structs::{
            AppConfig, CurrentRealm, PermissionUsed, PrimaryPlayerRes, StartupScene, StartupScenes,
        },
    };
    use dcl_types::SceneId;
    use scene_runner::{initialize_scene::PortableScenes, permissions::PermissionManager};

    use super::*;

    #[test]
    fn arrival_on_a_refused_parcel_does_not_wait_for_a_live_scene() {
        for scenario in [
            "banned",
            "unknown",
            "other_wallet",
            "other_realm",
            "preview",
            "local",
        ] {
            let mut world = World::new();
            let mut wallet = Wallet::default();
            wallet.finalize_as_guest_with_seed([1; 32]);
            let address = wallet.address().unwrap();
            world.insert_resource(wallet);
            let mut realm = common::structs::CurrentRealm {
                about_url: "realm".into(),
                ..default()
            };
            if scenario == "local" {
                realm.config.local_scene_parcels = Some(vec!["0,0".into()]);
            }
            world.insert_resource(realm);
            let preview = common::structs::PreviewMode {
                is_preview: scenario == "preview",
                ..default()
            };
            world.insert_resource(preview);
            world.insert_resource(if scenario == "unknown" {
                comms::SceneAdmission::unknown()
            } else {
                comms::SceneAdmission::from_refusal(
                    403,
                    Some("User is banned from this scene"),
                    false,
                    "scene",
                )
                .for_session(
                    if scenario == "other_realm" {
                        "elsewhere"
                    } else {
                        "realm"
                    },
                    if scenario == "other_wallet" {
                        alloy_core::primitives::Address::ZERO
                    } else {
                        address
                    },
                )
            });
            let mut pointers = ScenePointers::default();
            pointers.insert(
                IVec2::ZERO,
                PointerResult::Exists {
                    hash: "scene".into(),
                    urn: None,
                },
            );
            world.insert_resource(pointers);
            world.init_resource::<LiveScenes>();
            world.init_resource::<Events<ReturnedToWorld>>();
            let player = world
                .spawn((
                    PrimaryUser::default(),
                    OutOfWorld::default(),
                    Transform::from_xyz(8.0, 0.0, -8.0),
                ))
                .id();

            world.run_system_once(handle_out_of_world).unwrap();

            let released = scenario == "banned";
            assert_eq!(
                !world.entity(player).contains::<OutOfWorld>(),
                released,
                "{scenario}"
            );
            assert_eq!(
                world.get::<Transform>(player).unwrap().translation,
                Vec3::new(8.0, 0.0, -8.0)
            );
            let events = world.resource::<Events<ReturnedToWorld>>();
            let returned: Vec<_> = events.iter_current_update_events().copied().collect();
            assert_eq!(
                returned,
                if released {
                    vec![ReturnedToWorld::SceneReady]
                } else {
                    vec![]
                },
                "{scenario}"
            );
        }
    }

    fn spawn_points(json: serde_json::Value) -> Vec<SpawnPoint> {
        serde_json::from_value(json).unwrap()
    }

    /// A world whose player is out of world on parcel 0,0, looking for `spawn_point`, and what
    /// the parcel holds: nothing (`None`), or a scene in this load state with these spawn points.
    fn landing(
        spawn_point: Option<&str>,
        scene: Option<(Option<SceneLoading>, Vec<SpawnPoint>)>,
    ) -> (World, Entity) {
        let mut world = World::new();
        let mut pointers = ScenePointers::default();
        pointers.set_realm(IVec2::splat(-1), IVec2::splat(1));
        let mut live_scenes = LiveScenes::default();
        match scene {
            None => {
                pointers.insert(IVec2::ZERO, PointerResult::Nothing);
            }
            Some((loading, spawn_points)) => {
                pointers.insert(
                    IVec2::ZERO,
                    PointerResult::Exists {
                        hash: "scene".to_owned(),
                        urn: None,
                    },
                );
                let mut context = RendererSceneContext::new(
                    SceneId::DUMMY,
                    "scene".to_owned(),
                    String::new(),
                    false,
                    0,
                    String::new(),
                    IVec2::ZERO,
                    HashSet::from_iter([IVec2::ZERO]),
                    vec![],
                    Default::default(),
                    spawn_points,
                    Entity::PLACEHOLDER,
                    0.0,
                    false,
                    "",
                    false,
                    false,
                );
                // past its first ticks with nothing loading: ready for the player
                context.tick_number = 6;
                let mut scene = world.spawn((SceneHash("scene".to_owned()), context));
                if let Some(loading) = loading {
                    scene.insert(loading);
                }
                live_scenes.scenes.insert("scene".to_owned(), scene.id());
            }
        }
        world.insert_resource(pointers);
        world.insert_resource(live_scenes);
        let mut wallet = Wallet::default();
        wallet.finalize_as_guest();
        world.insert_resource(wallet);
        world.init_resource::<comms::SceneAdmission>();
        world.init_resource::<common::structs::CurrentRealm>();
        world.init_resource::<common::structs::PreviewMode>();
        world.init_resource::<Events<ReturnedToWorld>>();
        let player = world
            .spawn((
                PrimaryUser::default(),
                OutOfWorld {
                    spawn_point: spawn_point.map(str::to_owned),
                },
                Transform::from_xyz(8.0, 0.0, -8.0),
            ))
            .id();
        (world, player)
    }

    fn returned(world: &World) -> Vec<ReturnedToWorld> {
        world
            .resource::<Events<ReturnedToWorld>>()
            .iter_current_update_events()
            .copied()
            .collect()
    }

    /// What the player's last landing by name looked for, and whether its scene had it.
    fn last_named(world: &World, player: Entity) -> Option<(&str, bool)> {
        world
            .entity(player)
            .get::<LastNamedLanding>()
            .map(|last| (last.name.as_str(), last.found))
    }

    fn two_spawn_points() -> Vec<SpawnPoint> {
        spawn_points(serde_json::json!([
            { "name": "Lobby", "default": true, "position": { "x": 1, "y": 0, "z": 1 } },
            { "name": "Roof", "position": { "x": 2, "y": 9, "z": 3 } },
        ]))
    }

    #[test]
    fn a_named_spawn_point_is_found_in_lowercase() {
        let spawn_points = spawn_points(serde_json::json!([
            { "name": "Lobby", "default": true, "position": { "x": 1, "y": 0, "z": 1 } },
            { "name": "roof", "position": { "x": 2, "y": 9, "z": 2 } },
            { "name": "ROOF", "position": { "x": 3, "y": 9, "z": 3 } },
            { "position": { "x": 4, "y": 0, "z": 4 } },
        ]));
        // each one's x tells them apart
        let named = |name| {
            named_spawn_point(&spawn_points, name).map(|found| found.position.bounding_box().0.x)
        };
        assert_eq!(named("lobby"), Some(1.0));
        // the first of two alike
        assert_eq!(named("Roof"), Some(2.0));
        assert_eq!(named("cellar"), None);
        assert_eq!(named(""), None);
        // beyond ASCII too
        let accented = super::tests::spawn_points(serde_json::json!([
            { "name": "Entrée", "position": { "x": 7, "y": 0, "z": 7 } },
        ]));
        assert!(named_spawn_point(&accented, "ENTRÉE").is_some());
        // lowercase, not case folding: a letter whose capital is two letters keeps its own
        let sharp = super::tests::spawn_points(serde_json::json!([
            { "name": "Straße", "position": { "x": 8, "y": 0, "z": 8 } },
        ]));
        assert!(named_spawn_point(&sharp, "STRAßE").is_some());
        assert!(named_spawn_point(&sharp, "STRASSE").is_none());
        // without a name, the scene's default ones
        assert_eq!(default_spawn_points(&spawn_points).len(), 1);
        assert_eq!(default_spawn_points(&spawn_points[1..]).len(), 3);
    }

    #[test]
    fn a_landing_uses_the_spawn_point_it_names() {
        let (mut world, player) = landing(Some("roof"), Some((None, two_spawn_points())));

        world.run_system_once(handle_out_of_world).unwrap();

        assert!(!world.entity(player).contains::<OutOfWorld>());
        assert_eq!(
            world.entity(player).get::<Transform>().unwrap().translation,
            Vec3::new(2.0, 9.0, -3.0)
        );
        assert_eq!(returned(&world), vec![ReturnedToWorld::SceneReady]);
        assert_eq!(last_named(&world, player), Some(("roof", true)));
    }

    #[test]
    fn a_landing_naming_a_spawn_point_the_scene_lacks_uses_its_default() {
        let (mut world, player) = landing(Some("cellar"), Some((None, two_spawn_points())));

        world.run_system_once(handle_out_of_world).unwrap();

        assert!(!world.entity(player).contains::<OutOfWorld>());
        assert_eq!(
            world.entity(player).get::<Transform>().unwrap().translation,
            Vec3::new(1.0, 0.0, -1.0)
        );
        assert_eq!(last_named(&world, player), Some(("cellar", false)));
    }

    #[test]
    fn a_landing_naming_no_spawn_point_uses_the_default_ones() {
        let (mut world, player) = landing(None, Some((None, two_spawn_points())));

        world.run_system_once(handle_out_of_world).unwrap();

        assert_eq!(
            world.entity(player).get::<Transform>().unwrap().translation,
            Vec3::new(1.0, 0.0, -1.0)
        );
        assert_eq!(last_named(&world, player), None);
    }

    #[test]
    fn a_scene_still_loading_keeps_the_landing_and_its_spawn_point() {
        let (mut world, player) = landing(Some("roof"), Some((None, two_spawn_points())));
        let scene = world.resource::<LiveScenes>().scenes["scene"];
        world
            .entity_mut(scene)
            .get_mut::<RendererSceneContext>()
            .unwrap()
            .tick_number = 0;

        world.run_system_once(handle_out_of_world).unwrap();

        assert_eq!(
            world
                .entity(player)
                .get::<OutOfWorld>()
                .unwrap()
                .spawn_point
                .as_deref(),
            Some("roof")
        );
        assert_eq!(returned(&world), vec![]);
    }

    #[test]
    fn an_empty_parcel_lets_the_player_in_without_the_named_spawn_point() {
        let (mut world, player) = landing(Some("roof"), None);

        world.run_system_once(handle_out_of_world).unwrap();

        assert!(!world.entity(player).contains::<OutOfWorld>());
        assert_eq!(returned(&world), vec![ReturnedToWorld::SceneReady]);
        assert_eq!(last_named(&world, player), Some(("roof", false)));
    }

    // A scene whose main js could not be fetched fails after it has a context, still at its first
    // tick: the player waiting outside it goes in, as for a scene that failed before it had one.
    #[test]
    fn a_scene_that_failed_with_a_context_lets_the_player_in() {
        let (mut world, player) = landing(
            Some("roof"),
            Some((Some(SceneLoading::Failed), two_spawn_points())),
        );
        let scene = world.resource::<LiveScenes>().scenes["scene"];
        world
            .entity_mut(scene)
            .get_mut::<RendererSceneContext>()
            .unwrap()
            .tick_number = 0;

        world.run_system_once(handle_out_of_world).unwrap();

        assert!(!world.entity(player).contains::<OutOfWorld>());
        assert_eq!(returned(&world), vec![ReturnedToWorld::SceneLoadFailed]);
        // not placed on the spawn point of a scene that never ran
        assert_eq!(
            world.entity(player).get::<Transform>().unwrap().translation,
            Vec3::new(8.0, 0.0, -8.0)
        );
        assert_eq!(last_named(&world, player), Some(("roof", false)));
    }

    /// An app running `teleport_player`, with a player in the world.
    fn teleports() -> (App, Entity) {
        let mut app = App::new();
        app.add_event::<RpcCallEvent>()
            .add_event::<PermissionUsed>()
            .add_event::<ChangeRealmEvent>()
            .init_resource::<PermissionManager>()
            .init_resource::<AppConfig>()
            .init_resource::<CurrentRealm>()
            .init_resource::<ScenePointers>()
            .init_resource::<LiveScenes>()
            .init_resource::<PortableScenes>()
            .insert_resource(RealmInitialLocation::None)
            .add_systems(Update, teleport_player);
        let player = app
            .world_mut()
            .spawn((
                PrimaryUser::default(),
                Transform::default(),
                GlobalTransform::default(),
                AvatarDynamicState::default(),
            ))
            .id();
        app.insert_resource(PrimaryPlayerRes(player));
        (app, player)
    }

    /// A teleport the engine itself asks for, as the console's `/teleport` does.
    fn teleport(app: &mut App, to: (i32, i32), realm: Option<&str>, spawn_point: Option<&str>) {
        app.world_mut()
            .send_event(RpcCallEvent::engine(RpcCall::TeleportPlayer {
                to: Some(IVec2::new(to.0, to.1)),
                realm: realm.map(str::to_owned),
                spawn_point: spawn_point.map(str::to_owned),
                response: RpcResultSender::default(),
            }));
        app.update();
    }

    fn landing_spawn_point(app: &App, player: Entity) -> Option<Option<String>> {
        app.world()
            .entity(player)
            .get::<OutOfWorld>()
            .map(|out_of_world| out_of_world.spawn_point.clone())
    }

    fn realm_changes(app: &App) -> usize {
        app.world().resource::<Events<ChangeRealmEvent>>().len()
    }

    #[test]
    fn a_second_teleport_before_the_first_lands_takes_its_own_spawn_point() {
        let (mut app, player) = teleports();

        teleport(&mut app, (1, 1), None, Some("roof"));
        assert_eq!(
            landing_spawn_point(&app, player),
            Some(Some("roof".to_owned()))
        );

        // still out of world: the second landing names none, and lands where it asked
        teleport(&mut app, (2, 2), None, None);
        assert_eq!(landing_spawn_point(&app, player), Some(None));
        let position = app.world().entity(player).get::<Transform>().unwrap();
        assert_eq!(position.translation.xz(), Vec2::new(40.0, -40.0));
    }

    #[test]
    fn a_realm_teleport_keeps_its_spawn_point_for_its_own_landing() {
        let (mut app, player) = teleports();

        // a teleport inside the realm, still loading its scene
        teleport(&mut app, (10, 20), None, None);
        // then one to another realm: the name waits with that realm's landing
        teleport(&mut app, (3, 4), Some("mana.dcl.eth"), Some("door"));

        assert_eq!(landing_spawn_point(&app, player), Some(None));
        assert_eq!(
            *app.world().resource::<RealmInitialLocation>(),
            RealmInitialLocation::Parcel {
                parcel: IVec2::new(3, 4),
                spawn_point: Some("door".to_owned()),
            }
        );
        assert_eq!(realm_changes(&app), 1);
    }

    #[test]
    fn a_teleport_inside_the_realm_does_not_share_a_realm_changes_spawn_point() {
        let (mut app, player) = teleports();

        teleport(&mut app, (3, 4), Some("mana.dcl.eth"), Some("door"));
        // the realm change is still in flight
        teleport(&mut app, (1, 2), None, Some("roof"));

        assert_eq!(
            landing_spawn_point(&app, player),
            Some(Some("roof".to_owned()))
        );
        // the realm change's landing is given up with its name, so its failing later has
        // nothing of the local landing to clear
        assert_eq!(
            *app.world().resource::<RealmInitialLocation>(),
            RealmInitialLocation::None
        );
    }

    #[test]
    fn a_realm_change_without_a_parcel_carries_the_spawn_point_to_the_realms_spawn() {
        let (mut app, _) = teleports();
        app.world_mut()
            .send_event(RpcCallEvent::engine(RpcCall::TeleportPlayer {
                to: None,
                realm: Some("mana.dcl.eth".to_owned()),
                spawn_point: Some("door".to_owned()),
                response: RpcResultSender::default(),
            }));
        app.update();

        assert_eq!(
            *app.world().resource::<RealmInitialLocation>(),
            RealmInitialLocation::Base {
                spawn_point: Some("door".to_owned()),
            }
        );
    }

    #[test]
    fn a_spawn_point_name_too_long_is_refused() {
        let (mut app, player) = teleports();
        let (response, mut answer) = RpcResultSender::channel();
        app.world_mut()
            .send_event(RpcCallEvent::engine(RpcCall::TeleportPlayer {
                to: Some(IVec2::new(1, 1)),
                realm: None,
                spawn_point: Some("x".repeat(MAX_SPAWN_POINT_NAME + 1)),
                response,
            }));
        app.update();

        assert!(
            matches!(answer.try_recv(), Ok(Err(reason)) if reason == spawn_point_name_too_long())
        );
        // refused before anything moved
        assert_eq!(landing_spawn_point(&app, player), None);
        let position = app.world().entity(player).get::<Transform>().unwrap();
        assert_eq!(position.translation, Vec3::ZERO);
        // the longest one allowed goes through
        teleport(
            &mut app,
            (1, 1),
            None,
            Some(&"x".repeat(MAX_SPAWN_POINT_NAME)),
        );
        assert!(landing_spawn_point(&app, player).is_some());
    }

    /// A scene running in `app`, as the HUD's bridge scene when `super_user`, and a teleport of
    /// its own naming `spawn_point`. Returns what the scene is answered.
    fn scene_teleport(
        app: &mut App,
        super_user: bool,
        spawn_point: String,
    ) -> common::rpc::RpcResultReceiver<Result<(), String>> {
        let context = RendererSceneContext::new(
            SceneId::DUMMY,
            "scene".to_owned(),
            String::new(),
            false,
            0,
            String::new(),
            IVec2::ZERO,
            HashSet::from_iter([IVec2::ZERO]),
            vec![],
            Default::default(),
            vec![],
            Entity::PLACEHOLDER,
            0.0,
            false,
            "",
            false,
            false,
        );
        let scene = app.world_mut().spawn(context).id();
        app.insert_resource(StartupScenes {
            scenes: vec![StartupScene {
                source: String::new(),
                super_user,
                preview: false,
                hot_reload: None,
                hash: Some("scene".to_owned()),
                failed: false,
            }],
        });
        let (response, answer) = RpcResultSender::channel();
        app.world_mut().send_event(RpcCallEvent {
            origin: RpcOrigin::Scene {
                entity: scene,
                hash: "scene".into(),
            },
            call: RpcCall::TeleportPlayer {
                to: Some(IVec2::new(1, 1)),
                realm: None,
                spawn_point: Some(spawn_point),
                response,
            },
        });
        app.update();
        answer
    }

    // The HUD's bridge scene asks through the scene API: its permission is granted without
    // asking, and the name stays with its request through the permission check.
    #[test]
    fn a_scenes_teleport_carries_the_spawn_point_it_names() {
        let (mut app, player) = teleports();

        let mut answer = scene_teleport(&mut app, true, "roof".to_owned());

        assert!(matches!(answer.try_recv(), Ok(Ok(()))));
        assert_eq!(
            landing_spawn_point(&app, player),
            Some(Some("roof".to_owned()))
        );
    }

    #[test]
    fn a_scenes_spawn_point_name_too_long_is_refused_before_a_permission_is_asked() {
        let (mut app, player) = teleports();

        // a scene whose teleport the player would be asked about
        let mut answer = scene_teleport(&mut app, false, "x".repeat(MAX_SPAWN_POINT_NAME + 1));

        assert!(
            matches!(answer.try_recv(), Ok(Err(reason)) if reason == spawn_point_name_too_long())
        );
        assert!(app
            .world()
            .resource::<PermissionManager>()
            .pending
            .is_empty());
        assert_eq!(landing_spawn_point(&app, player), None);
    }

    // The schedule the plugin runs: a landing whose scene is ready in the frame a new teleport
    // arrives is placed first, by its own spawn point, and the teleport then moves the player.
    #[test]
    fn a_landing_is_placed_before_a_teleport_arriving_in_the_same_frame() {
        let (mut landed, _) = landing(Some("roof"), Some((None, two_spawn_points())));
        // the scene also covers the parcel the teleport goes to: placed after the move, the old
        // landing would find its spawn point there
        let mut pointers = landed.remove_resource::<ScenePointers>().unwrap();
        pointers.insert(
            IVec2::new(1, 1),
            PointerResult::Exists {
                hash: "scene".to_owned(),
                urn: None,
            },
        );
        let live_scenes = landed.remove_resource::<LiveScenes>().unwrap();
        let wallet = landed.remove_resource::<Wallet>().unwrap();
        let scene = live_scenes.scenes["scene"];
        let context = landed
            .entity_mut(scene)
            .take::<RendererSceneContext>()
            .unwrap();

        let mut app = App::new();
        app.add_event::<RpcCallEvent>()
            .add_event::<PermissionUsed>()
            .add_event::<ChangeRealmEvent>()
            .add_event::<ReturnedToWorld>()
            .init_resource::<PermissionManager>()
            .init_resource::<AppConfig>()
            .init_resource::<CurrentRealm>()
            .init_resource::<comms::SceneAdmission>()
            .init_resource::<common::structs::PreviewMode>()
            .init_resource::<PortableScenes>()
            .insert_resource(RealmInitialLocation::None)
            .insert_resource(wallet)
            .add_systems(Update, landing_systems());
        let scene = app
            .world_mut()
            .spawn((SceneHash("scene".to_owned()), context))
            .id();
        let mut live_scenes = LiveScenes::default();
        live_scenes.scenes.insert("scene".to_owned(), scene);
        app.insert_resource(live_scenes).insert_resource(pointers);
        let player = app
            .world_mut()
            .spawn((
                PrimaryUser::default(),
                OutOfWorld {
                    spawn_point: Some("roof".to_owned()),
                },
                Transform::from_xyz(8.0, 0.0, -8.0),
                GlobalTransform::default(),
                AvatarDynamicState::default(),
            ))
            .id();
        app.insert_resource(PrimaryPlayerRes(player));

        teleport(&mut app, (1, 1), None, None);

        // the old landing found its spawn point, and is over
        assert_eq!(last_named(app.world(), player), Some(("roof", true)));
        // the new one names none, and starts from the parcel it asked for
        assert_eq!(landing_spawn_point(&app, player), Some(None));
        let position = app.world().entity(player).get::<Transform>().unwrap();
        assert_eq!(position.translation.xz(), Vec2::new(24.0, -24.0));
    }
}

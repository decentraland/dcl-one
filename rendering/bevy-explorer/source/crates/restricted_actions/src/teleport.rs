use bevy::{math::Vec3Swizzles, prelude::*};
use bevy_console::ConsoleCommand;
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
    permissions::Permission,
    renderer_context::{RendererSceneContext, FROZEN_BLOCK},
    update_world::{gltf_container::GLTF_LOADING, mesh_collider::SceneColliderData},
    OutOfWorld,
};
use system_bridge::SystemApi;
use wallet::Wallet;

// (parcel, realm, response, report a realm change's outcome)
type TeleportAction = (
    Option<IVec2>,
    Option<String>,
    RpcResultSender<Result<(), String>>,
    bool,
);

/// The last landing that looked for a spawn point by name, and whether its scene had one of
/// that name: the debug panel's Teleport section shows it.
#[derive(Resource, Default)]
pub struct LastNamedLanding(Option<(String, bool)>);

pub fn teleport_player(
    mut commands: Commands,
    mut events: EventReader<RpcCallEvent>,
    mut player: Query<(Entity, &mut Transform, &mut AvatarDynamicState), With<PrimaryUser>>,
    mut perms: Permission<TeleportAction>,
    mut realm_target: ResMut<RealmInitialLocation>,
    super_user: Query<(), With<SuperUserScene>>,
    mut hud_requests: EventReader<SystemApi>,
) {
    // (the action, the spawn point it names): a scene's teleport names none
    let mut actions: Vec<(TeleportAction, Option<String>)> = Vec::new();

    for (scene, to, realm, response) in events.read().filter_map(|ev| match &ev.call {
        RpcCall::TeleportPlayer {
            to,
            realm,
            response,
        } => Some((ev.origin.scene(), *to, realm.clone(), response.clone())),
        _ => None,
    }) {
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
            actions.push(((to, realm, response, false), None));
            continue;
        };
        let report = !super_user.contains(scene);
        perms.check(
            ty,
            scene,
            (to, realm, response, report),
            Some(detail),
            false,
        );
    }

    let unnamed = |action| (action, None);
    actions.extend(perms.drain_success(PermissionType::Teleport).map(unnamed));
    actions.extend(
        perms
            .drain_success(PermissionType::ChangeRealm)
            .map(unnamed),
    );
    // the HUD's own, after the scenes': the latest request is the one the player made
    for request in hud_requests.read() {
        if let SystemApi::Teleport {
            to,
            realm,
            spawn_point,
            response,
        } = request
        {
            actions.push((
                (*to, realm.clone(), response.clone(), false),
                spawn_point.clone(),
            ));
        }
    }

    for ((to, realm, response, report), spawn_point) in actions {
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

    for (_, _, response, _) in perms.drain_fail(PermissionType::Teleport) {
        response.send(Err("User declined".to_owned()))
    }
    for (_, _, response, _) in perms.drain_fail(PermissionType::ChangeRealm) {
        response.send(Err("User declined".to_owned()))
    }
}

/// The scene's first spawn point of this name, whatever the case of either.
fn named_spawn_point<'a>(spawn_points: &'a [SpawnPoint], name: &str) -> Option<&'a SpawnPoint> {
    spawn_points.iter().find(|spawn_point| {
        spawn_point
            .name
            .as_deref()
            .is_some_and(|own| own.eq_ignore_ascii_case(name))
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

/// The spawn point a landing names, for the debug panel's Teleport section.
#[derive(clap::Parser, ConsoleCommand)]
#[command(name = "/teleport_state")]
pub(crate) struct TeleportState;

pub(crate) fn teleport_state(
    mut input: ConsoleCommand<TeleportState>,
    player: Query<&OutOfWorld, With<PrimaryUser>>,
    realm_target: Res<RealmInitialLocation>,
    last_named: Res<LastNamedLanding>,
) {
    if input.take().is_none() {
        return;
    }
    // of the landing under way, else of the realm change that has yet to start one
    let pending = player
        .single()
        .ok()
        .and_then(|out_of_world| out_of_world.spawn_point.clone())
        .or_else(|| match &*realm_target {
            RealmInitialLocation::Base { spawn_point }
            | RealmInitialLocation::Parcel { spawn_point, .. } => spawn_point.clone(),
            RealmInitialLocation::None => None,
        });
    input.reply_ok(
        serde_json::json!({
            "pending": pending,
            "last": last_named.0.as_ref().map(|(name, found)| {
                serde_json::json!({ "name": name, "found": found })
            }),
            "unavailable": {
                "pending": "No landing under way names a spawn point.",
                "last": "No landing has looked for a named spawn point.",
            },
        })
        .to_string(),
    );
}

pub fn handle_out_of_world(
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
    mut returned: EventWriter<ReturnedToWorld>,
    mut last_named: ResMut<LastNamedLanding>,
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
        Some(PointerResult::Exists { hash, .. }) => hash,
        Some(PointerResult::Nothing) => {
            debug!("scene {parcel} doesn't exist, returning to world");
            debug!("everything: {:?}", pointers);
            if let Some(name) = &out_of_world.spawn_point {
                last_named.0 = Some((name.clone(), false));
            }
            commands.entity(player).remove::<OutOfWorld>();
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
        if let Some(name) = &out_of_world.spawn_point {
            last_named.0 = Some((name.clone(), false));
        }
        commands.entity(player).remove::<OutOfWorld>();
        returned.write(ReturnedToWorld::SceneLoadFailed);
        return;
    }

    if let Some(context) = maybe_context {
        // A frozen scene (inspector /freeze_scene, or the editor-mode auto-freeze at tick 3) is as
        // loaded as it's going to get — it never advances its tick or clears `blocked` to become
        // "ready" on its own, so don't keep the player out-of-world behind the loading screen. Do
        // still wait for gltfs though: they keep processing renderer-side while the scene is frozen,
        // and releasing early would drop the player in before the ground colliders exist.
        let frozen =
            context.blocked.contains(FROZEN_BLOCK) && !context.blocked.contains(GLTF_LOADING);
        // A broken scene (hung past the not-responding timeout, errored, or unreachable) will
        // never become ready; stop holding the player behind the loading screen and let them
        // into the world. An inspected scene never gates the player at all: it's a debugging
        // session, and it may sit at a breakpoint (or paused before its first tick) forever.
        if !context.broken()
            && !frozen
            && !context.inspected
            && (context.tick_number <= 5 || !context.blocked.is_empty())
        {
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
                    warn!(
                        "scene {} has no spawn point named {name:?}, using its default",
                        context.hash
                    );
                }
                last_named.0 = Some((name.clone(), found.is_some()));
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
            commands.entity(player).remove::<OutOfWorld>();
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
    use common::structs::{AppConfig, CurrentRealm, PermissionUsed, PrimaryPlayerRes};
    use dcl_types::SceneId;
    use scene_runner::{initialize_scene::PortableScenes, permissions::PermissionManager};

    use super::*;

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
        world.init_resource::<Events<ReturnedToWorld>>();
        world.init_resource::<LastNamedLanding>();
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

    fn two_spawn_points() -> Vec<SpawnPoint> {
        spawn_points(serde_json::json!([
            { "name": "Lobby", "default": true, "position": { "x": 1, "y": 0, "z": 1 } },
            { "name": "Roof", "position": { "x": 2, "y": 9, "z": 3 } },
        ]))
    }

    #[test]
    fn a_named_spawn_point_is_found_whatever_its_case() {
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
        assert_eq!(
            world.resource::<LastNamedLanding>().0,
            Some(("roof".to_owned(), true))
        );
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
        assert_eq!(
            world.resource::<LastNamedLanding>().0,
            Some(("cellar".to_owned(), false))
        );
    }

    #[test]
    fn a_landing_naming_no_spawn_point_uses_the_default_ones() {
        let (mut world, player) = landing(None, Some((None, two_spawn_points())));

        world.run_system_once(handle_out_of_world).unwrap();

        assert_eq!(
            world.entity(player).get::<Transform>().unwrap().translation,
            Vec3::new(1.0, 0.0, -1.0)
        );
        assert_eq!(world.resource::<LastNamedLanding>().0, None);
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
        assert_eq!(
            world.resource::<LastNamedLanding>().0,
            Some(("roof".to_owned(), false))
        );
    }

    // A scene whose main js could not be fetched fails after it has a context, still at its first
    // tick: the player waiting outside it goes in, as for a scene that failed before it had one.
    #[test]
    fn a_scene_that_failed_with_a_context_lets_the_player_in() {
        let (mut world, player) = landing(
            Some("roof"),
            Some((Some(SceneLoading::Failed), two_spawn_points())),
        );

        world.run_system_once(handle_out_of_world).unwrap();

        assert!(!world.entity(player).contains::<OutOfWorld>());
        assert_eq!(returned(&world), vec![ReturnedToWorld::SceneLoadFailed]);
        // not placed on the spawn point of a scene that never ran
        assert_eq!(
            world.entity(player).get::<Transform>().unwrap().translation,
            Vec3::new(8.0, 0.0, -8.0)
        );
    }

    /// An app running `teleport_player`, with a player in the world.
    fn teleports() -> (App, Entity) {
        let mut app = App::new();
        app.add_event::<RpcCallEvent>()
            .add_event::<SystemApi>()
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

    fn hud_teleport(app: &mut App, to: (i32, i32), realm: Option<&str>, spawn_point: Option<&str>) {
        app.world_mut().send_event(SystemApi::Teleport {
            to: Some(IVec2::new(to.0, to.1)),
            realm: realm.map(str::to_owned),
            spawn_point: spawn_point.map(str::to_owned),
            response: RpcResultSender::default(),
        });
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

        hud_teleport(&mut app, (1, 1), None, Some("roof"));
        assert_eq!(
            landing_spawn_point(&app, player),
            Some(Some("roof".to_owned()))
        );

        // still out of world: the second landing names none, and lands where it asked
        hud_teleport(&mut app, (2, 2), None, None);
        assert_eq!(landing_spawn_point(&app, player), Some(None));
        let position = app.world().entity(player).get::<Transform>().unwrap();
        assert_eq!(position.translation.xz(), Vec2::new(40.0, -40.0));
    }

    #[test]
    fn a_realm_teleport_keeps_its_spawn_point_for_its_own_landing() {
        let (mut app, player) = teleports();

        // a teleport inside the realm, still loading its scene
        hud_teleport(&mut app, (10, 20), None, None);
        // then one to another realm: the name waits with that realm's landing
        hud_teleport(&mut app, (3, 4), Some("mana.dcl.eth"), Some("door"));

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

        hud_teleport(&mut app, (3, 4), Some("mana.dcl.eth"), Some("door"));
        // the realm change is still in flight
        hud_teleport(&mut app, (1, 2), None, Some("roof"));

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
        app.world_mut().send_event(SystemApi::Teleport {
            to: None,
            realm: Some("mana.dcl.eth".to_owned()),
            spawn_point: Some("door".to_owned()),
            response: RpcResultSender::default(),
        });
        app.update();

        assert_eq!(
            *app.world().resource::<RealmInitialLocation>(),
            RealmInitialLocation::Base {
                spawn_point: Some("door".to_owned()),
            }
        );
    }
}

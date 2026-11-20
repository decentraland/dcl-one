use bevy::{math::Vec3Swizzles, prelude::*};
use bevy_console::ConsoleCommand;
use common::{
    rpc::{RpcCall, RpcCallEvent, RpcResultSender},
    structs::{
        AvatarDynamicState, LandingSpawnPoint, PermissionType, PrimaryUser, ReturnedToWorld,
        SpawnPoint,
    },
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
use wallet::Wallet;

// (parcel, realm, spawn point name, response, report a realm change's outcome)
type TeleportAction = (
    Option<IVec2>,
    Option<String>,
    Option<String>,
    RpcResultSender<Result<(), String>>,
    bool,
);

pub fn teleport_player(
    mut commands: Commands,
    mut events: EventReader<RpcCallEvent>,
    mut player: Query<(Entity, &mut Transform, &mut AvatarDynamicState), With<PrimaryUser>>,
    mut perms: Permission<TeleportAction>,
    mut realm_target: ResMut<RealmInitialLocation>,
    mut landing_spawn_point: ResMut<LandingSpawnPoint>,
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
        // each teleport names its own landing; a realm change that fails clears it again
        landing_spawn_point.pending = spawn_point;
        if let Some(realm) = realm {
            // A realm change (a full reconnect, even to the realm we are in — same as changeRealm),
            // landing on the parcel once it is live, or on the realm's default spawn without one.
            // The parcel can't be applied now: it would resolve against the parcel grid of the
            // realm being left.
            debug!("teleport -> {to:?} in {realm}");
            *realm_target = match to {
                Some(to) => RealmInitialLocation::Parcel(to),
                None => RealmInitialLocation::Base,
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
            commands.try_insert(OutOfWorld);
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

/// The spawn point a teleport named for its landing, for the debug panel's Teleport section.
#[derive(clap::Parser, ConsoleCommand)]
#[command(name = "/teleport_state")]
pub struct TeleportState;

pub fn teleport_state(
    mut input: ConsoleCommand<TeleportState>,
    landing_spawn_point: Res<LandingSpawnPoint>,
) {
    if input.take().is_none() {
        return;
    }
    input.reply_ok(
        serde_json::json!({
            "pending": landing_spawn_point.pending,
            "last": landing_spawn_point.last.as_ref().map(|(name, found)| {
                serde_json::json!({ "name": name, "found": found })
            }),
            "unavailable": {
                "pending": "No teleport waiting to land named a spawn point.",
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
    mut player: Query<(Entity, &mut Transform), (With<PrimaryUser>, With<OutOfWorld>)>,
    pointers: Res<ScenePointers>,
    live_scenes: Res<LiveScenes>,
    foreign_players: Query<&GlobalTransform, With<ForeignPlayer>>,
    wallet: Res<Wallet>,
    mut returned: EventWriter<ReturnedToWorld>,
    mut landing_spawn_point: ResMut<LandingSpawnPoint>,
) {
    let Ok((player, mut t)) = player.single_mut() else {
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
            if let Some(name) = landing_spawn_point.pending.take() {
                landing_spawn_point.last = Some((name, false));
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
        if let Some(name) = landing_spawn_point.pending.take() {
            landing_spawn_point.last = Some((name, false));
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

            // the spawn point the teleport named, when the scene has it; without one, spawn
            // points marked `default` take precedence, and a scene without any uses them all
            let named = landing_spawn_point.pending.take().and_then(|name| {
                let found = named_spawn_point(&context.spawn_points, &name);
                if found.is_none() {
                    warn!(
                        "scene {} has no spawn point named `{name}`, using its default",
                        context.hash
                    );
                }
                landing_spawn_point.last = Some((name, found.is_some()));
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
    use dcl_types::SceneId;

    use super::*;

    #[test]
    fn a_named_spawn_point_is_found_whatever_its_case() {
        let spawn_points: Vec<SpawnPoint> = serde_json::from_value(serde_json::json!([
            { "name": "Lobby", "default": true, "position": { "x": 1, "y": 0, "z": 1 } },
            { "name": "roof", "position": { "x": 2, "y": 9, "z": 2 } },
            { "name": "ROOF", "position": { "x": 3, "y": 9, "z": 3 } },
            { "position": { "x": 4, "y": 0, "z": 4 } },
        ]))
        .unwrap();
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

    // A scene whose main js could not be fetched fails after it has a context, still at its first
    // tick: the player waiting outside it goes in, as for a scene that failed before it had one.
    #[test]
    fn a_scene_that_failed_with_a_context_lets_the_player_in() {
        let mut world = World::new();
        let mut pointers = ScenePointers::default();
        pointers.set_realm(IVec2::splat(-1), IVec2::splat(1));
        pointers.insert(
            IVec2::ZERO,
            PointerResult::Exists {
                hash: "scene".to_owned(),
                urn: None,
            },
        );
        world.insert_resource(pointers);
        let scene = world
            .spawn((
                SceneHash("scene".to_owned()),
                SceneLoading::Failed,
                RendererSceneContext::new(
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
                ),
            ))
            .id();
        let mut live_scenes = LiveScenes::default();
        live_scenes.scenes.insert("scene".to_owned(), scene);
        world.insert_resource(live_scenes);
        let mut wallet = Wallet::default();
        wallet.finalize_as_guest();
        world.insert_resource(wallet);
        world.init_resource::<Events<ReturnedToWorld>>();
        world.init_resource::<LandingSpawnPoint>();
        let player = world
            .spawn((
                PrimaryUser::default(),
                OutOfWorld,
                Transform::from_xyz(8.0, 0.0, -8.0),
            ))
            .id();

        world.run_system_once(handle_out_of_world).unwrap();

        assert!(!world.entity(player).contains::<OutOfWorld>());
        let returned = world
            .resource::<Events<ReturnedToWorld>>()
            .iter_current_update_events()
            .copied()
            .collect::<Vec<_>>();
        assert_eq!(returned, vec![ReturnedToWorld::SceneLoadFailed]);
    }
}

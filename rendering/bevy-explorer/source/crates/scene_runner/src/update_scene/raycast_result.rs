// TODO
// [x] - handle continuous properly
// [x] - don't run every renderer frame
// [/] - then prevent scene execution until raycasts are run (not required now, we run raycasts once on first frame after request arrives, required for ponter events anyway)
// [x] - probably change renderer context to contain frame number as well as dt so we can track precisely track run state
// [ ] - move into scene loop
// [/] - consider how global raycasts interact with this setup (it works, pointer events use a global raycast already. but need to optimise by ordering scenes based on ray)

use bevy::{
    color::palettes::basic,
    prelude::*,
    tasks::{ComputeTaskPool, ParallelSliceMut},
};
use bevy_console::ConsoleCommand;
use common::{
    dynamics::{PLAYER_COLLIDER_OVERLAP, PLAYER_COLLIDER_RADIUS},
    structs::PrimaryUser,
};
use web_time::Instant;

use crate::{
    initialize_scene::SuperUserScene,
    update_scene::raycast_budget::{
        budget, candidate_order, raycast_budget, raycast_stats, run_within_budget, Candidate,
        RaycastBudgetCommand, RaycastBudgetOverride, RaycastStats, RaycastStatsCommand, TierStats,
    },
    update_world::{
        gltf_container::GLTF_LOADING,
        mesh_collider::{GlobalGroundCollider, RaycastResult, SceneColliderData},
        raycast::Raycast,
    },
    ContainingScene, RendererSceneContext, SceneEntity, SceneSets, SceneUpdates,
};
use console::{AddDebugSection, DebugInput, DebugInputKind, DebugSection, DoAddConsoleCommand};
use dcl_component::{
    proto_components::{
        common::Vector3,
        sdk::components::{
            common::RaycastHit, pb_raycast::Direction, ColliderLayer, PbRaycastResult,
            RaycastQueryType, RaycastShape,
        },
        RoughRoundExt,
    },
    SceneComponentId, SceneEntityId,
};
use dcl_types::interface::CrdtType;

pub struct RaycastResultPlugin;

impl Plugin for RaycastResultPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, run_raycasts.in_set(SceneSets::Input));
        app.init_resource::<DebugRaycast>();
        app.init_resource::<RaycastStats>();
        app.init_resource::<RaycastBudgetOverride>();
        app.init_resource::<SuperUserRaycastScene>();
        app.add_preview_console_command::<DebugRaycastCommand, _>(debug_raycast);
        app.add_console_command::<RaycastStatsCommand, _>(raycast_stats);
        app.add_console_command::<RaycastBudgetCommand, _>(raycast_budget);
        app.add_debug_section(DebugSection {
            title: "Raycasts",
            query: "/raycast_stats",
            fields: &[
                ("Capped budget (ms)", "budgetMs"),
                ("Capped ran (last pass)", "ran"),
                ("Capped deferred (last pass)", "deferred"),
                ("Capped spent (last pass, ms)", "spentMs"),
                ("Uncapped ran (last pass)", "uncappedRan"),
                ("Uncapped spent (last pass, ms)", "uncappedSpentMs"),
            ],
            inputs: &[DebugInput {
                label: "Capped budget override (ms)",
                field: "budgetMs",
                command: "/raycast_budget {value}",
                kind: DebugInputKind::Number {
                    min: 0.0,
                    max: 10.0,
                    step: 0.1,
                },
            }],
            actions: &[("Automatic budget", "/raycast_budget auto")],
        });
    }
}

#[derive(Resource, Default)]
struct DebugRaycast(bool);

/// The scene a super-user scene has pinned (via the inspector's `/set_scene`). When set, that
/// super-user scene's raycasts hit only its own scene and this target, and receive entity ids for
/// both — letting the editor identify what it hit in the scene it's inspecting (cross-scene ids are
/// otherwise stripped). None = unpinned; super-user raycasts then behave normally.
#[derive(Resource, Default)]
pub struct SuperUserRaycastScene(pub Option<Entity>);

/// Toggle debug lines for raycasts
#[derive(clap::Parser, ConsoleCommand)]
#[command(name = "/debug_raycast")]
struct DebugRaycastCommand {
    show: Option<bool>,
}

fn debug_raycast(mut input: ConsoleCommand<DebugRaycastCommand>, mut debug: ResMut<DebugRaycast>) {
    if let Some(Ok(command)) = input.take() {
        let new_state = command.show.unwrap_or(!debug.0);
        debug.0 = new_state;
        input.reply_ok(format!("showing debug raycast lines: {new_state}"));
    }
}

/// Below this many due raycasts, dispatching to the pool costs more than it saves.
const MIN_PARALLEL_RAYCASTS: usize = 8;
/// Tasks per pool thread when casting in parallel, so one expensive cast only holds up its own
/// chunk while idle threads take the others.
const RAYCAST_TASKS_PER_THREAD: usize = 4;

type RaycastRequests<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static SceneEntity,
        &'static mut Raycast,
        &'static GlobalTransform,
    ),
>;

type SceneContexts<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static mut RendererSceneContext,
        &'static mut SceneColliderData,
        &'static GlobalTransform,
        Has<SuperUserScene>,
    ),
>;

type GlobalGround<'w, 's> = Query<
    'w,
    's,
    (Entity, &'static mut SceneColliderData),
    (With<GlobalGroundCollider>, Without<RendererSceneContext>),
>;

/// Which colliders a raycast runs against.
enum RaycastTargets {
    /// the caster's own scene
    Own,
    /// A super-user scene that has pinned a target (/set_scene) raycasts only its own scene and
    /// that target scene, returning entity ids for both — a known, unambiguous pair, so it can
    /// identify what it hit in the scene it's operating on.
    Pinned(Entity),
    /// The scenes the cast meets, nearest first, with the distance at which it first touches each,
    /// then global ground: the scenes on a ray's line, and for the avatar's shape those its
    /// capsule touches as well. Cross-scene ids are stripped.
    World(Vec<(Entity, f32)>),
}

/// A due raycast, resolved from the ecs so that it can cast on any thread.
struct RaycastJob {
    scene_root: Entity,
    scene_entity: SceneEntityId,
    scene_translation: Vec3,
    timestamp: Option<u32>,
    continuous: bool,
    origin: Vec3,
    direction: Vec3,
    max_distance: f32,
    mask: u32,
    shape: RaycastShape,
    query: RaycastQueryType,
    targets: RaycastTargets,
    results: Vec<(Entity, RaycastResult)>,
}

impl RaycastJob {
    fn nearest(&self, colliders: &SceneColliderData) -> Option<RaycastResult> {
        match self.shape {
            RaycastShape::RsRay => colliders.cast_ray_nearest_flushed(
                self.origin,
                self.direction,
                self.max_distance,
                self.mask,
                true,
                false,
                None,
            ),
            RaycastShape::RsAvatar => colliders.cast_avatar_nearest_flushed(
                self.origin.as_dvec3(),
                self.direction.as_dvec3(),
                self.max_distance as f64,
                self.mask,
                false,
                false,
                Default::default(),
                -PLAYER_COLLIDER_OVERLAP,
            ),
        }
    }

    fn all(&self, colliders: &SceneColliderData) -> Vec<RaycastResult> {
        match self.shape {
            RaycastShape::RsRay => colliders.cast_ray_all_flushed(
                self.origin,
                self.direction,
                self.max_distance,
                self.mask,
                true,
            ),
            RaycastShape::RsAvatar => colliders.cast_avatar_all_flushed(
                self.origin.as_dvec3(),
                self.direction.as_dvec3(),
                self.max_distance as f64,
                self.mask,
                false,
                false,
                -PLAYER_COLLIDER_OVERLAP,
            ),
        }
    }

    /// [`Self::nearest`] for a scene of a world cast: another scene's colliders answer where
    /// that scene is drawn, the caster's own wherever they are, as in a cast at its own scene.
    fn nearest_in_world(
        &self,
        scene: Entity,
        colliders: &SceneColliderData,
    ) -> Option<RaycastResult> {
        if scene == self.scene_root {
            return self.nearest(colliders);
        }

        match self.shape {
            RaycastShape::RsRay => colliders.cast_ray_nearest_in_scene_flushed(
                self.origin,
                self.direction,
                self.max_distance,
                self.mask,
                true,
                false,
                None,
            ),
            RaycastShape::RsAvatar => colliders.cast_avatar_nearest_in_scene_flushed(
                self.origin.as_dvec3(),
                self.direction.as_dvec3(),
                self.max_distance as f64,
                self.mask,
                false,
                false,
                Default::default(),
                -PLAYER_COLLIDER_OVERLAP,
            ),
        }
    }

    /// [`Self::all`] for a scene of a world cast, as [`Self::nearest_in_world`].
    fn all_in_world(&self, scene: Entity, colliders: &SceneColliderData) -> Vec<RaycastResult> {
        if scene == self.scene_root {
            return self.all(colliders);
        }

        match self.shape {
            RaycastShape::RsRay => colliders.cast_ray_all_in_scene_flushed(
                self.origin,
                self.direction,
                self.max_distance,
                self.mask,
                true,
            ),
            RaycastShape::RsAvatar => colliders.cast_avatar_all_in_scene_flushed(
                self.origin.as_dvec3(),
                self.direction.as_dvec3(),
                self.max_distance as f64,
                self.mask,
                false,
                false,
                -PLAYER_COLLIDER_OVERLAP,
            ),
        }
    }

    /// Runs against the scenes' colliders, which must have been flushed for this job's targets.
    fn cast<'a>(
        &self,
        scene_colliders: &impl Fn(Entity) -> Option<&'a SceneColliderData>,
        global_ground: &[(Entity, &SceneColliderData)],
    ) -> Vec<(Entity, RaycastResult)> {
        match &self.targets {
            RaycastTargets::Pinned(target) => {
                let mut hits: Vec<(Entity, RaycastResult)> = Vec::new();
                // the caster's own scene, then the pinned target scene
                let mut scenes = vec![self.scene_root];
                if *target != self.scene_root {
                    scenes.push(*target);
                }
                for scene in scenes {
                    let Some(colliders) = scene_colliders(scene) else {
                        continue;
                    };
                    match self.query {
                        RaycastQueryType::RqtHitFirst => {
                            if let Some(hit) = self.nearest(colliders) {
                                hits.push((scene, hit));
                            }
                        }
                        RaycastQueryType::RqtQueryAll => {
                            hits.extend(self.all(colliders).into_iter().map(|hit| (scene, hit)))
                        }
                        RaycastQueryType::RqtNone => {}
                    }
                }
                // hit-first: across the two scenes, keep the nearest
                if self.query == RaycastQueryType::RqtHitFirst && hits.len() > 1 {
                    hits.sort_by(|a, b| {
                        a.1.toi
                            .partial_cmp(&b.1.toi)
                            .unwrap_or(std::cmp::Ordering::Equal)
                    });
                    hits.truncate(1);
                }
                hits
            }
            RaycastTargets::Own => {
                let Some(colliders) = scene_colliders(self.scene_root) else {
                    return Vec::new();
                };
                match self.query {
                    RaycastQueryType::RqtHitFirst => self
                        .nearest(colliders)
                        .map(|hit| vec![(self.scene_root, hit)])
                        .unwrap_or_default(),
                    RaycastQueryType::RqtQueryAll => self
                        .all(colliders)
                        .into_iter()
                        .map(|hit| (self.scene_root, hit))
                        .collect(),
                    RaycastQueryType::RqtNone => Vec::new(),
                }
            }
            RaycastTargets::World(scenes) => match self.query {
                RaycastQueryType::RqtHitFirst => {
                    let mut scenes = scenes.iter().peekable();
                    let mut best_result: Option<(Entity, RaycastResult)> = None;
                    while scenes.peek().is_some_and(|(_, closest)| {
                        best_result.as_ref().is_none_or(|(_, br)| br.toi > *closest)
                    }) {
                        let scene = scenes.next().unwrap().0;
                        let Some(colliders) = scene_colliders(scene) else {
                            continue;
                        };
                        if let Some(result) = self.nearest_in_world(scene, colliders) {
                            if best_result.as_ref().is_none_or(|(_, b)| b.toi > result.toi) {
                                best_result = Some((scene, result));
                            }
                        }
                    }

                    for (ground, colliders) in global_ground {
                        if let Some(result) = self.nearest(colliders) {
                            if best_result.as_ref().is_none_or(|(_, b)| b.toi > result.toi) {
                                best_result = Some((*ground, result));
                            }
                        }
                    }

                    best_result.into_iter().collect()
                }
                RaycastQueryType::RqtQueryAll => {
                    let mut results = Vec::new();
                    for (scene, _) in scenes {
                        let Some(colliders) = scene_colliders(*scene) else {
                            continue;
                        };
                        for result in self.all_in_world(*scene, colliders) {
                            results.push((*scene, result));
                        }
                    }
                    for (ground, colliders) in global_ground {
                        for result in self.all(colliders) {
                            results.push((*ground, result));
                        }
                    }

                    results
                }
                RaycastQueryType::RqtNone => Vec::new(),
            },
        }
    }
}

/// The scenes a world cast asks, with the distance at which it first meets each: those on its
/// ray, and for a cast of the avatar's shape those the capsule touches beside the ray as well.
fn world_scenes(
    containing_scene: &ContainingScene,
    origin: Vec3,
    full_ray: Vec3,
    shape: RaycastShape,
) -> Vec<(Entity, f32)> {
    match shape {
        RaycastShape::RsRay => containing_scene.get_ray(origin, full_ray),
        RaycastShape::RsAvatar => {
            containing_scene.get_swept(origin, full_ray, PLAYER_COLLIDER_RADIUS)
        }
    }
}

/// What preparing a due raycast request found.
enum Prepared {
    /// The request or its scene is gone, so it counts as neither ran nor deferred.
    Gone,
    /// The request has no direction: it ran, with no result.
    NoDirection,
    Job(RaycastJob),
}

/// Marks a due request as run, resolves it into a job and flushes the colliders the job will cast
/// against.
fn prepare(
    e: Entity,
    raycast_requests: &mut RaycastRequests,
    scene_context: &mut SceneContexts,
    target_positions: &Query<&GlobalTransform>,
    containing_scene: &ContainingScene,
    su_target: Option<Entity>,
    global_ground: &mut GlobalGround,
    ground_flushed: &mut bool,
) -> Prepared {
    let Ok((_, scene_ent, mut raycast, transform)) = raycast_requests.get_mut(e) else {
        return Prepared::Gone;
    };
    debug!("{e:?} has raycast request: {raycast:?}");
    let Ok((_, context, _, scene_transform, super_user)) = scene_context.get(scene_ent.root) else {
        return Prepared::Gone;
    };
    let continuous = raycast.raycast.continuous.unwrap_or(false);
    raycast.last_run = context.last_update_frame;
    debug!("running raycast");

    // execute the raycast
    let raycast = &raycast.raycast;

    let (_, local_rotation, _) = transform.to_scale_rotation_translation();
    let scene_translation = scene_transform.translation();

    let offset = raycast
        .origin_offset
        .as_ref()
        .map(Vector3::world_vec_to_vec3)
        .unwrap_or(Vec3::ZERO);
    let origin = transform.transform_point(offset);
    let direction = match &raycast.direction {
        Some(Direction::LocalDirection(dir)) => local_rotation * dir.world_vec_to_vec3(),
        Some(Direction::GlobalDirection(dir)) => dir.world_vec_to_vec3(),
        Some(Direction::GlobalTarget(point)) => {
            point.world_vec_to_vec3() + scene_translation - origin
        }
        Some(Direction::TargetEntity(id)) => {
            let target_position = context
                .bevy_entity(SceneEntityId::from_proto_u32(*id))
                .and_then(|entity| target_positions.get(entity).ok())
                .map(|gt| gt.translation())
                .unwrap_or(origin);
            target_position - origin
        }
        None => {
            warn!("no direction on raycast");
            return Prepared::NoDirection;
        }
    }
    .normalize();

    let mask = raycast
        .collision_mask
        .unwrap_or(ColliderLayer::ClPointer as u32 | ColliderLayer::ClPhysics as u32);

    // Everything else (normal scenes, and super-user scenes with no pinned target) is own
    // scene, or the world via `include_world`.
    let su_target = if super_user { su_target } else { None };
    let targets = if let Some(target) = su_target {
        RaycastTargets::Pinned(target)
    } else if context.is_portable || raycast.include_world() {
        let full_ray = direction * raycast.max_distance;
        RaycastTargets::World(world_scenes(
            containing_scene,
            origin,
            full_ray,
            raycast.shape(),
        ))
    } else {
        RaycastTargets::Own
    };

    let mut flush = |scene: Entity| {
        if let Ok((_, _, mut colliders, _, _)) = scene_context.get_mut(scene) {
            colliders.update_bvh();
        }
    };
    match &targets {
        RaycastTargets::Own => flush(scene_ent.root),
        RaycastTargets::Pinned(target) => {
            flush(scene_ent.root);
            flush(*target);
        }
        RaycastTargets::World(scenes) => {
            scenes.iter().for_each(|(scene, _)| flush(*scene));
            if !*ground_flushed {
                for (_, mut colliders) in global_ground.iter_mut() {
                    colliders.update_bvh();
                }
                *ground_flushed = true;
            }
        }
    }

    Prepared::Job(RaycastJob {
        scene_root: scene_ent.root,
        scene_entity: scene_ent.id,
        scene_translation,
        timestamp: raycast.timestamp,
        continuous,
        origin,
        direction,
        max_distance: raycast.max_distance,
        mask,
        shape: raycast.shape(),
        query: raycast.query_type(),
        targets,
        results: Vec::new(),
    })
}

/// Casts prepared jobs, on the compute pool once there are enough of them. Reads colliders only;
/// each job writes only its own results.
fn cast_jobs(
    mut jobs: &mut [RaycastJob],
    scene_context: &SceneContexts,
    global_ground: &GlobalGround,
) {
    let scenes = scene_context.as_readonly();
    let scene_colliders = |scene: Entity| scenes.get(scene).ok().map(|item| item.2);
    let ground = global_ground.iter().collect::<Vec<_>>();
    let cast = |jobs: &mut [RaycastJob]| {
        for job in jobs {
            job.results = job.cast(&scene_colliders, &ground);
        }
    };

    if jobs.len() < MIN_PARALLEL_RAYCASTS {
        cast(jobs);
    } else {
        let pool = ComputeTaskPool::get();
        let tasks = pool.thread_num().max(1) * RAYCAST_TASKS_PER_THREAD;
        let chunk_size = jobs.len().div_ceil(tasks);
        jobs.par_chunk_map_mut(pool, chunk_size, |_, chunk| cast(chunk));
    }
}

/// Writes a cast job's result to its scene, and draws its debug line.
fn write_result(
    job: RaycastJob,
    scene_context: &mut SceneContexts,
    debug: bool,
    gizmos: &mut Gizmos,
    gizmo_cache: &mut Vec<(f64, Vec3, Vec3)>,
    elapsed_secs: f64,
) {
    // debug line showing raycast
    if debug {
        let end = job.origin + job.direction * job.max_distance;
        gizmos.line(job.origin, end, basic::BLUE);
        if !job.continuous {
            gizmo_cache.push((elapsed_secs + 1.0, job.origin, end));
        }
    }

    // output
    let scene_origin = job.origin - job.scene_translation;
    let direction = job.direction;
    let scene_root = job.scene_root;
    let scoped = matches!(job.targets, RaycastTargets::Pinned(_));

    let make_hit = |(scene, result): (Entity, RaycastResult)| -> RaycastHit {
        RaycastHit {
            position: Some(Vector3::world_vec_from_vec3(
                &(scene_origin + direction * result.toi).round_at_pow2(-14),
            )),
            global_origin: Some(Vector3::world_vec_from_vec3(
                &scene_origin.round_at_pow2(-14),
            )),
            direction: Some(Vector3::world_vec_from_vec3(&direction.round_at_pow2(-14))),
            normal_hit: Some(Vector3::world_vec_from_vec3(
                &result.normal.round_at_pow2(-14),
            )),
            length: result.toi,
            // Only pass entity details for hits on the casting scene's own entities — except a
            // super-user scene's *scoped* raycast (routed to one target scene above), where the
            // ids are meaningful and unambiguous. World raycasts keep cross-scene ids stripped.
            mesh_name: if scene == scene_root || scoped {
                result.id.name
            } else {
                None
            },
            entity_id: if scene == scene_root || scoped {
                result.id.entity.as_proto_u32()
            } else {
                None
            },
        }
    };

    let mut context = scene_context.get_mut(scene_root).unwrap().1;

    let result = PbRaycastResult {
        timestamp: job.timestamp,
        global_origin: Some(Vector3::world_vec_from_vec3(&scene_origin)),
        direction: Some(Vector3::world_vec_from_vec3(&direction.round_at_pow2(-14))),
        hits: job.results.into_iter().map(make_hit).collect(),
        tick_number: context.tick_number,
    };

    context.update_crdt(
        SceneComponentId::RAYCAST_RESULT,
        CrdtType::LWW_ENT,
        job.scene_entity,
        &result,
    );
}

fn run_raycasts(
    mut raycast_requests: RaycastRequests,
    target_positions: Query<&GlobalTransform>,
    mut scene_context: SceneContexts,
    debug: Res<DebugRaycast>,
    mut gizmos: Gizmos,
    time: Res<Time>,
    mut gizmo_cache: Local<Vec<(f64, Vec3, Vec3)>>,
    containing_scene: ContainingScene,
    su_target_res: Res<SuperUserRaycastScene>,
    mut global_ground: GlobalGround,
    updates: Res<SceneUpdates>,
    budget_override: Res<RaycastBudgetOverride>,
    mut stats: ResMut<RaycastStats>,
    player: Query<Entity, With<PrimaryUser>>,
) {
    // redraw non-continuous gizmos for 1 sec
    gizmo_cache.retain(|(until, origin, end)| {
        gizmos.line(*origin, *end, basic::BLUE);
        time.elapsed_secs_f64() > *until
    });

    // the scenes the player stands in, plus every portable
    let player_scenes = player
        .single()
        .map(|player| containing_scene.get_area(player, PLAYER_COLLIDER_RADIUS))
        .unwrap_or_default();

    // collect the requests that are due this frame: uncapped for the system scene, portables and
    // the player's scenes, capped (in the order they should run) for every other scene
    let mut uncapped: Vec<Entity> = Vec::new();
    let mut capped: Vec<(Entity, Candidate)> = Vec::new();
    for (e, scene_ent, raycast, _) in raycast_requests.iter() {
        let Ok((_, context, _, _, super_user)) = scene_context.get(scene_ent.root) else {
            continue;
        };

        // check if we can run
        if context.blocked.contains(GLTF_LOADING) {
            debug!("raycast skipped, waiting for gltfs");
            continue;
        }

        // check if we need to run
        let continuous = raycast.raycast.continuous.unwrap_or(false);
        if !continuous && raycast.last_run > 0 {
            continue;
        }
        if continuous && raycast.last_run >= context.last_update_frame {
            continue;
        }

        if super_user || player_scenes.contains(&scene_ent.root) {
            uncapped.push(e);
        } else {
            capped.push((
                e,
                Candidate {
                    priority: context.priority,
                    last_run: raycast.last_run,
                    entity_id: e.index(),
                },
            ));
        }
    }
    capped.sort_by(|(_, a), (_, b)| candidate_order(a, b));
    let capped: Vec<Entity> = capped.into_iter().map(|(e, _)| e).collect();

    let elapsed_secs = time.elapsed_secs_f64();
    let mut ground_flushed = false;

    // each tier's stats describe its last frame that had something to cast
    if !uncapped.is_empty() {
        // prepare every request, cast them together, then write in request order
        let start = Instant::now();
        let mut ran = 0;
        let mut jobs = Vec::new();
        for e in uncapped {
            match prepare(
                e,
                &mut raycast_requests,
                &mut scene_context,
                &target_positions,
                &containing_scene,
                su_target_res.0,
                &mut global_ground,
                &mut ground_flushed,
            ) {
                Prepared::Gone => {}
                Prepared::NoDirection => ran += 1,
                Prepared::Job(job) => {
                    ran += 1;
                    jobs.push(job);
                }
            }
        }
        cast_jobs(&mut jobs, &scene_context, &global_ground);
        for job in jobs {
            write_result(
                job,
                &mut scene_context,
                debug.0,
                &mut gizmos,
                &mut gizmo_cache,
                elapsed_secs,
            );
        }
        stats.uncapped = TierStats {
            ran,
            deferred: 0,
            spent: start.elapsed(),
        };
    }
    // the frame target is the previous scene loop's, as raycasts run before the loop
    stats.budget = budget(updates.frame_target, *budget_override);
    if !capped.is_empty() {
        // one at a time, so the budget can stop between casts. returns false when the request or
        // its scene is gone, so it counts as neither ran nor deferred
        let mut cast_one = |e: Entity| -> bool {
            match prepare(
                e,
                &mut raycast_requests,
                &mut scene_context,
                &target_positions,
                &containing_scene,
                su_target_res.0,
                &mut global_ground,
                &mut ground_flushed,
            ) {
                Prepared::Gone => false,
                Prepared::NoDirection => true,
                Prepared::Job(mut job) => {
                    cast_jobs(
                        std::slice::from_mut(&mut job),
                        &scene_context,
                        &global_ground,
                    );
                    write_result(
                        job,
                        &mut scene_context,
                        debug.0,
                        &mut gizmos,
                        &mut gizmo_cache,
                        elapsed_secs,
                    );
                    true
                }
            }
        };
        stats.capped = run_within_budget(capped, stats.budget, &mut Instant::now, &mut cast_one);
    }
}

#[cfg(test)]
mod world_target_tests {
    use super::*;
    use crate::{
        get_ray_tests::world_with_scenes,
        update_world::mesh_collider::{regions_of_parcels, ColliderId},
    };
    use bevy::ecs::system::RunSystemOnce;
    use rapier3d_f64::prelude::{ColliderBuilder, Group, InteractionGroups, InteractionTestMode};

    const MASK: u32 = ColliderLayer::ClPointer as u32 | ColliderLayer::ClPhysics as u32;

    /// A box of `size` centred on `centre`.
    fn add_box(
        colliders: &mut SceneColliderData,
        index: u16,
        centre: Vec3,
        size: Vec3,
    ) -> ColliderId {
        let id = ColliderId::new(
            SceneEntityId {
                id: 512 + index,
                generation: 0,
            },
            None,
            0,
        );
        let collider = ColliderBuilder::cuboid(0.5, 0.5, 0.5)
            .collision_groups(InteractionGroups::new(
                Group::from_bits_truncate(MASK),
                Group::from_bits_truncate(MASK),
                InteractionTestMode::And,
            ))
            .build();
        colliders.set_collider(&id, collider, None);
        colliders.update_collider_transform(
            &id,
            &GlobalTransform::from(Transform::from_translation(centre).with_scale(size)),
        );
        id
    }

    /// A scene around a notch: parcels (0, 0) and (2, 0), joined by the row (0, 1), (1, 1),
    /// (2, 1). The notch at (1, 0), which is not the scene's, spans world x 16 to 32 and z -16
    /// to 0. The scene has a post over the notch at x = 24 and one past it at x = 40, both on.
    fn scene_with_posts() -> (SceneColliderData, [ColliderId; 2]) {
        let parcels = [(0, 0), (2, 0), (0, 1), (1, 1), (2, 1)];
        let mut colliders =
            SceneColliderData::for_regions(&regions_of_parcels(&parcels), parcels.len());

        let ids = [24.0, 40.0].map(|x| {
            let id = ColliderId::new(
                SceneEntityId {
                    id: 512 + x as u16,
                    generation: 0,
                },
                None,
                0,
            );
            let collider = ColliderBuilder::cuboid(0.5, 0.5, 0.5)
                .collision_groups(InteractionGroups::new(
                    Group::from_bits_truncate(MASK),
                    Group::from_bits_truncate(MASK),
                    InteractionTestMode::And,
                ))
                .build();
            colliders.set_collider(&id, collider, None);
            colliders.update_collider_transform(
                &id,
                &GlobalTransform::from(
                    Transform::from_xyz(x, 2.0, -8.0).with_scale(Vec3::new(1.0, 4.0, 1.0)),
                ),
            );
            id
        });
        colliders.update_bvh();
        (colliders, ids)
    }

    /// The scene with the posts.
    fn scene() -> Entity {
        Entity::from_raw(1)
    }

    fn another_scene() -> Entity {
        Entity::from_raw(2)
    }

    /// The hits of a cast by `caster` along +x from parcel (0, 0), through the notch, as
    /// (collider, distance).
    fn cast(
        caster: Entity,
        shape: RaycastShape,
        query: RaycastQueryType,
        targets: RaycastTargets,
    ) -> (Vec<(ColliderId, f32)>, [ColliderId; 2]) {
        let scene = scene();
        let (colliders, ids) = scene_with_posts();
        let job = RaycastJob {
            scene_root: caster,
            scene_entity: SceneEntityId::ROOT,
            scene_translation: Vec3::ZERO,
            timestamp: None,
            continuous: false,
            origin: Vec3::new(2.0, 0.0, -8.0),
            direction: Vec3::X,
            max_distance: 100.0,
            mask: MASK,
            shape,
            query,
            targets,
            results: Vec::new(),
        };
        let hits = job
            .cast(&|entity| (entity == scene).then_some(&colliders), &[])
            .into_iter()
            .map(|(_, hit)| (hit.id, (hit.toi * 100.0).round() / 100.0))
            .collect();
        (hits, ids)
    }

    fn world() -> RaycastTargets {
        RaycastTargets::World(vec![(scene(), 0.0)])
    }

    #[test]
    fn a_world_ray_hits_another_scene_only_where_that_scene_is() {
        let (hits, [_, past_the_notch]) = cast(
            another_scene(),
            RaycastShape::RsRay,
            RaycastQueryType::RqtHitFirst,
            world(),
        );
        assert_eq!(hits, vec![(past_the_notch.clone(), 37.5)]);

        let (hits, _) = cast(
            another_scene(),
            RaycastShape::RsRay,
            RaycastQueryType::RqtQueryAll,
            world(),
        );
        assert_eq!(hits, vec![(past_the_notch, 37.5)]);
    }

    #[test]
    fn a_world_avatar_cast_hits_another_scene_only_where_that_scene_is() {
        let (hits, [_, past_the_notch]) = cast(
            another_scene(),
            RaycastShape::RsAvatar,
            RaycastQueryType::RqtHitFirst,
            world(),
        );
        assert_eq!(hits, vec![(past_the_notch.clone(), 37.2)]);

        let (hits, _) = cast(
            another_scene(),
            RaycastShape::RsAvatar,
            RaycastQueryType::RqtQueryAll,
            world(),
        );
        assert_eq!(hits, vec![(past_the_notch, 37.2)]);
    }

    #[test]
    fn a_scenes_own_ray_hits_its_colliders_wherever_they_are() {
        let (hits, [in_the_notch, past_the_notch]) = cast(
            scene(),
            RaycastShape::RsRay,
            RaycastQueryType::RqtHitFirst,
            RaycastTargets::Own,
        );
        assert_eq!(hits, vec![(in_the_notch.clone(), 21.5)]);

        let (mut hits, _) = cast(
            scene(),
            RaycastShape::RsRay,
            RaycastQueryType::RqtQueryAll,
            RaycastTargets::Own,
        );
        hits.sort_by(|a, b| a.1.total_cmp(&b.1));
        assert_eq!(hits, vec![(in_the_notch, 21.5), (past_the_notch, 37.5)]);
    }

    #[test]
    fn a_world_cast_hits_the_casters_own_colliders_wherever_they_are() {
        let (hits, [in_the_notch, past_the_notch]) = cast(
            scene(),
            RaycastShape::RsRay,
            RaycastQueryType::RqtHitFirst,
            world(),
        );
        assert_eq!(hits, vec![(in_the_notch.clone(), 21.5)]);

        let (mut hits, _) = cast(
            scene(),
            RaycastShape::RsRay,
            RaycastQueryType::RqtQueryAll,
            world(),
        );
        hits.sort_by(|a, b| a.1.total_cmp(&b.1));
        assert_eq!(
            hits,
            vec![(in_the_notch.clone(), 21.5), (past_the_notch, 37.5)]
        );

        let (hits, _) = cast(
            scene(),
            RaycastShape::RsAvatar,
            RaycastQueryType::RqtHitFirst,
            world(),
        );
        assert_eq!(hits, vec![(in_the_notch, 21.2)]);
    }

    #[test]
    fn a_world_avatar_cast_hits_a_step_of_the_scene_beside_its_ray() {
        // the caster's scene is parcel (0, 0); east of it, parcel (1, 0) starts at world x = 16
        let (mut world, [caster, east]) = world_with_scenes([(0, 0), (1, 0)]);

        // a step of east along the edge, x from 16 to 18, its top at 0.5 m
        let mut colliders = SceneColliderData::for_regions(&regions_of_parcels(&[(1, 0)]), 1);
        let step = add_box(
            &mut colliders,
            0,
            Vec3::new(17.0, 0.25, -8.0),
            Vec3::new(2.0, 0.5, 4.0),
        );
        colliders.update_bvh();

        // straight down inside the caster's scene, 0.2 m from the edge: the capsule's 0.3 m
        // reaches over the step, the ray down its middle does not
        let origin = Vec3::new(15.8, 2.0, -8.0);
        let mut cast = |shape: RaycastShape| {
            let scenes = world
                .run_system_once(move |containing: ContainingScene| {
                    world_scenes(&containing, origin, Vec3::NEG_Y * 5.0, shape)
                })
                .unwrap();
            let asked = scenes.iter().map(|(scene, _)| *scene).collect::<Vec<_>>();
            let job = RaycastJob {
                scene_root: caster,
                scene_entity: SceneEntityId::ROOT,
                scene_translation: Vec3::ZERO,
                timestamp: None,
                continuous: false,
                origin,
                direction: Vec3::NEG_Y,
                max_distance: 5.0,
                mask: MASK,
                shape,
                query: RaycastQueryType::RqtHitFirst,
                targets: RaycastTargets::World(scenes),
                results: Vec::new(),
            };
            let hits = job.cast(&|scene| (scene == east).then_some(&colliders), &[]);
            (asked, hits)
        };

        let (asked, hits) = cast(RaycastShape::RsRay);
        assert_eq!(asked, vec![caster]);
        assert!(hits.is_empty());

        let (asked, hits) = cast(RaycastShape::RsAvatar);
        assert_eq!(asked.len(), 2);
        assert!(asked.contains(&east));
        assert_eq!(hits.len(), 1, "{} hits", hits.len());
        let (scene, hit) = &hits[0];
        assert_eq!((*scene, &hit.id), (east, &step));
        // the capsule's rounded foot lands on the step's edge, the feet 0.42 m up
        assert!((hit.toi - 1.576).abs() < 2e-3, "{}", hit.toi);
    }

    #[test]
    fn a_world_avatar_cast_returns_a_stop_at_a_neighbours_edge_that_comes_before_a_hit() {
        // west is parcel (0, 0) and east is (1, 0), the edge between them at world x = 16; the
        // caster's scene is elsewhere
        let (mut world, [west, east, caster]) = world_with_scenes([(0, 0), (1, 0), (5, 5)]);

        // a wall of west across the path, its near face at z = -7.25
        let mut west_colliders = SceneColliderData::for_regions(&regions_of_parcels(&[(0, 0)]), 1);
        add_box(
            &mut west_colliders,
            0,
            Vec3::new(8.0, 2.0, -7.75),
            Vec3::new(16.0, 4.0, 1.0),
        );
        west_colliders.update_bvh();
        // a wall of east along the edge, from x = 15.85: on, and 0.1 m past what east is drawn to
        let mut east_colliders = SceneColliderData::for_regions(&regions_of_parcels(&[(1, 0)]), 1);
        let along_the_edge = add_box(
            &mut east_colliders,
            1,
            Vec3::new(16.925, 2.0, -7.0),
            Vec3::new(2.15, 4.0, 6.0),
        );
        east_colliders.update_bvh();

        // along -z inside west, closing on east by a centimetre a metre. The capsule touches
        // east's space 5 m along, overlapping the wall there, so it is stopped at east's edge:
        // a millimetre clear of it, which is 0.1 m earlier on this path. West's wall is 4.95 m
        // along.
        let origin = Vec3::new(15.6, 0.0, -2.0);
        let direction = Vec3::new(0.01, 0.0, -1.0).normalize();
        let scenes = world
            .run_system_once(move |containing: ContainingScene| {
                world_scenes(
                    &containing,
                    origin,
                    direction * 10.0,
                    RaycastShape::RsAvatar,
                )
            })
            .unwrap();
        let job = RaycastJob {
            scene_root: caster,
            scene_entity: SceneEntityId::ROOT,
            scene_translation: Vec3::ZERO,
            timestamp: None,
            continuous: false,
            origin,
            direction,
            max_distance: 10.0,
            mask: MASK,
            shape: RaycastShape::RsAvatar,
            query: RaycastQueryType::RqtHitFirst,
            targets: RaycastTargets::World(scenes),
            results: Vec::new(),
        };
        let hits = job.cast(
            &|scene| {
                if scene == west {
                    Some(&west_colliders)
                } else if scene == east {
                    Some(&east_colliders)
                } else {
                    None
                }
            },
            &[],
        );

        assert_eq!(hits.len(), 1, "{} hits", hits.len());
        let (scene, hit) = &hits[0];
        assert_eq!(
            (*scene, &hit.id),
            (east, &along_the_edge),
            "the hit is {} along",
            hit.toi
        );
        assert!((hit.toi - 4.9).abs() < 2e-3, "{}", hit.toi);
    }
}

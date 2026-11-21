//! `/visible_entities`: the scene entities whose meshes fall in the primary camera's view, with the
//! screen rect their bounds project to, largest first. Frustum and bounds only: an entity behind
//! another is still listed.

use bevy::{
    math::Affine3A,
    prelude::*,
    render::primitives::{Aabb, Frustum},
    window::PrimaryWindow,
};
use bevy_console::ConsoleCommand;
use camera_reel::metadata::{in_frame, screen_rect};
use common::structs::{PrimaryCamera, PrimaryUser};
use console::{
    reply::{round, round_fine, Screen},
    DoAddConsoleCommand, PendingConsoleResponses,
};
use dcl_component::{transform_and_parent::DclTranslation, SceneEntityId};
use scene_runner::{update_scene::synthetic_pointer::world_bounds, SceneEntity};
use serde_json::json;

use crate::{active_scene::SceneResolver, labels, snapshot::PendingSnapshotRequests};

const DEFAULT_LIMIT: usize = 25;
const MAX_LIMIT: usize = 200;

pub(crate) struct VisibleEntitiesPlugin;

impl Plugin for VisibleEntitiesPlugin {
    fn build(&self, app: &mut App) {
        app.add_console_command::<VisibleEntitiesCommand, _>(visible_entities_cmd);
    }
}

/// List, as JSON, the parcel scene's entities that draw a primitive mesh or a glTF and whose
/// bounds meet the camera's view, sorted by `coverage` (the share of the screen their projected
/// bounds cover), largest first: `crdtEntityId`, `label` (as `/scene_entities --labels`),
/// `screenRect` (logical pixels from the top left, clamped to the view), `center` (normalized),
/// `coverage`, `distance` (from the player), `distanceToCamera`, `renderers`. With `screen`,
/// `cameraPosition`, `candidates` (entities that draw), `inFrustum`, `returned`, `truncated`.
/// Occlusion is not evaluated: an entity behind a wall is listed, and a bounds box is larger than
/// the mesh it wraps; a screenshot shows what is visible.
#[derive(clap::Parser, ConsoleCommand)]
#[command(name = "/visible_entities")]
struct VisibleEntitiesCommand {
    /// most entities to list, largest coverage first (1 to 200, default 25)
    #[arg(long, value_parser = clap::value_parser!(u32).range(1..=200))]
    limit: Option<u32>,
    /// least share of the screen (0 to 1) an entity must cover to be listed
    #[arg(long, default_value_t = 0.0)]
    min_coverage: f32,
}

/// What one entity's meshes project to.
struct Placed {
    entity: SceneEntityId,
    rect: Option<Rect>,
    centre: Vec3,
    renderers: usize,
}

#[allow(clippy::too_many_arguments)]
fn visible_entities_cmd(
    mut input: ConsoleCommand<VisibleEntitiesCommand>,
    resolver: SceneResolver,
    camera: Query<(&Camera, &GlobalTransform, &Frustum, &Projection), With<PrimaryCamera>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    player: Query<&GlobalTransform, With<PrimaryUser>>,
    meshes: Query<(&Aabb, &GlobalTransform, &InheritedVisibility)>,
    children: Query<&Children>,
    scene_entities: Query<(), With<SceneEntity>>,
    mut pending: ResMut<PendingSnapshotRequests>,
    mut console_responses: ResMut<PendingConsoleResponses>,
) {
    let Some(Ok(cmd)) = input.take() else {
        return;
    };
    if !(0.0..=1.0).contains(&cmd.min_coverage) {
        input.reply_failed(format!(
            "--min-coverage runs from 0 to 1; got {}",
            cmd.min_coverage
        ));
        return;
    }
    let (_, ctx) = match resolver.resolve() {
        Ok(resolved) => resolved,
        Err(e) => {
            input.reply_failed(e);
            return;
        }
    };
    let Ok((camera, camera_transform, frustum, projection)) = camera.single() else {
        input.reply_failed("no primary camera");
        return;
    };
    let Some(screen) = Screen::primary(&windows) else {
        input.reply_failed("no primary window");
        return;
    };
    let Some(viewport) = camera.logical_viewport_rect() else {
        input.reply_failed("the camera has no viewport yet");
        return;
    };
    let near = match projection {
        Projection::Perspective(p) => p.near,
        Projection::Orthographic(p) => p.near,
        _ => 0.1,
    };
    let player = player.single().ok().map(|t| t.translation());
    let clip_from_view = camera.clip_from_view();

    let mut placed = Vec::new();
    for entity in ctx.live_scene_entities() {
        if entity.id <= SceneEntityId::CAMERA.id {
            continue;
        }
        let Some(target) = ctx.bevy_entity(entity) else {
            continue;
        };
        let boxes = own_meshes(target, &meshes, &children, &scene_entities);
        let renderers = boxes.len();
        let Some((min, max)) = world_bounds(boxes) else {
            continue;
        };
        let world = Aabb::from_min_max(min, max);
        if !in_frame(frustum, &world) {
            continue;
        }
        // the share of the whole view the box covers, from its top left
        let share = screen_rect(&world, camera_transform, clip_from_view, near, Vec2::ONE);
        let rect = (share.width > 0.0 && share.height > 0.0).then(|| {
            let min = viewport.min + Vec2::new(share.x, share.y) * viewport.size();
            Rect::from_corners(
                min,
                min + Vec2::new(share.width, share.height) * viewport.size(),
            )
        });
        placed.push(Placed {
            entity,
            rect,
            centre: (min + max) * 0.5,
            renderers,
        });
    }

    let camera_position = camera_transform.translation();
    let limit = cmd
        .limit
        .map_or(DEFAULT_LIMIT, |l| l as usize)
        .min(MAX_LIMIT);
    let min_coverage = cmd.min_coverage;
    let scene = ctx.hash.clone();
    let (tx, rx) = tokio::sync::oneshot::channel();
    match resolver.request_snapshot(&mut pending, move |crdt| {
        let candidates = ctx_candidates(crdt);
        let viewport_area = viewport.width() * viewport.height();
        let mut listed: Vec<_> = placed
            .into_iter()
            .filter(|p| labels::renders(crdt, p.entity))
            .map(|p| {
                let coverage = p
                    .rect
                    .map_or(0.0, |r| r.width() * r.height() / viewport_area.max(1.0));
                (p, coverage)
            })
            .collect();
        let in_frustum = listed.len();
        listed.retain(|(_, coverage)| *coverage >= min_coverage);
        listed.sort_by(|a, b| b.1.total_cmp(&a.1));
        let truncated = listed.len() > limit;
        listed.truncate(limit);
        let entities: Vec<_> = listed
            .iter()
            .map(|(p, coverage)| {
                json!({
                    "crdtEntityId": p.entity.as_proto_u32().unwrap_or(p.entity.id as u32),
                    "label": labels::label(crdt, p.entity),
                    "screenRect": p.rect.map(|r| json!({
                        "x": round(r.min.x),
                        "y": round(r.min.y),
                        "width": round(r.width()),
                        "height": round(r.height()),
                    })),
                    "center": p.rect.map(|r| {
                        let c = screen.to_normalized(r.center());
                        json!({ "x": round_fine(c.x), "y": round_fine(c.y) })
                    }),
                    "coverage": round_fine(*coverage),
                    "distance": player.map(|player| round(player.distance(p.centre))),
                    "distanceToCamera": round(camera_position.distance(p.centre)),
                    "renderers": p.renderers,
                })
            })
            .collect();
        let camera = DclTranslation::from_bevy_translation(camera_position).0;
        let _ = tx.send(Ok(json!({
            "scene": scene,
            "screen": screen.to_json(),
            "cameraPosition": { "x": round(camera[0]), "y": round(camera[1]), "z": round(camera[2]) },
            "candidates": candidates,
            "inFrustum": in_frustum,
            "returned": entities.len(),
            "truncated": truncated,
            "occlusion": "not evaluated: frustum and bounds only, so an entity behind another is still listed",
            "entities": entities,
        })
        .to_string()));
    }) {
        Ok(()) => console_responses.push_oneshot(rx, |r| r, input.take_responder()),
        Err(e) => input.reply_failed(e),
    }
}

/// The entities of the scene that draw a primitive mesh or a glTF.
fn ctx_candidates(crdt: &dcl_types::interface::CrdtStore) -> usize {
    let mut entities = std::collections::HashSet::new();
    for component in [
        dcl_component::SceneComponentId::MESH_RENDERER,
        dcl_component::SceneComponentId::GLTF_CONTAINER,
    ] {
        if let Some(state) = crdt.lww.get(&component) {
            entities.extend(
                state
                    .last_write
                    .iter()
                    .filter(|(_, entry)| entry.is_some)
                    .map(|(entity, _)| *entity),
            );
        }
    }
    entities.len()
}

/// The visible meshes of `target` and of its descendants that are not themselves scene entities
/// (a child entity is listed on its own), each with where it is placed.
fn own_meshes(
    target: Entity,
    meshes: &Query<(&Aabb, &GlobalTransform, &InheritedVisibility)>,
    children: &Query<&Children>,
    scene_entities: &Query<(), With<SceneEntity>>,
) -> Vec<(Aabb, Affine3A)> {
    let mut boxes = Vec::new();
    let mut stack = vec![target];
    while let Some(entity) = stack.pop() {
        if let Ok((aabb, transform, visible)) = meshes.get(entity) {
            if visible.get() {
                boxes.push((*aabb, transform.affine()));
            }
        }
        if let Ok(kids) = children.get(entity) {
            stack.extend(kids.iter().filter(|kid| !scene_entities.contains(*kid)));
        }
    }
    boxes
}

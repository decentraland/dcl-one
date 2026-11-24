//! `/scene_entities --visible`: the scene entities whose meshes fall in the primary camera's
//! view, with the screen rect their bounds project to, largest first. Frustum and bounds only: an
//! entity behind another is still listed.

use bevy::{
    ecs::system::SystemParam,
    math::Affine3A,
    prelude::*,
    render::primitives::{Aabb, Frustum},
    window::PrimaryWindow,
};
use camera_reel::metadata::{in_frame, screen_rect};
use common::structs::{PrimaryCamera, PrimaryUser};
use console::reply::{round, round_fine, Screen};
use dcl_component::{transform_and_parent::DclTranslation, SceneEntityId};
use scene_runner::{update_scene::synthetic_pointer::world_bounds, SceneEntity};
use serde_json::json;

use crate::{active_scene::SceneResolver, labels, snapshot::PendingSnapshotRequests};

const DEFAULT_LIMIT: usize = 25;
const MAX_LIMIT: u32 = 200;

/// What the listing reads besides the scene: the camera, the window and the meshes.
#[derive(SystemParam)]
pub(crate) struct View<'w, 's> {
    camera: Query<
        'w,
        's,
        (
            &'static Camera,
            &'static GlobalTransform,
            &'static Frustum,
            &'static Projection,
        ),
        With<PrimaryCamera>,
    >,
    windows: Query<'w, 's, &'static Window, With<PrimaryWindow>>,
    player: Query<'w, 's, &'static GlobalTransform, With<PrimaryUser>>,
    meshes: Query<
        'w,
        's,
        (
            &'static Aabb,
            &'static GlobalTransform,
            &'static InheritedVisibility,
        ),
    >,
    children: Query<'w, 's, &'static Children>,
    scene_entities: Query<'w, 's, (), With<SceneEntity>>,
}

/// The reply of a listing, once the scene's state has been read.
pub(crate) type Listing = tokio::sync::oneshot::Receiver<Result<String, String>>;

/// What one entity's meshes project to.
struct Placed {
    entity: SceneEntityId,
    rect: Option<Rect>,
    centre: Vec3,
    renderers: usize,
}

impl View<'_, '_> {
    /// List, as JSON, the active scene's entities that draw a primitive mesh or a glTF and whose
    /// bounds meet the camera's view, sorted by `coverage` (the share of the screen their projected
    /// bounds cover), largest first: `crdtEntityId`, `label` (as `/scene_entities --labels`),
    /// `screenRect` (logical pixels from the top left, clamped to the view), `center` (normalized),
    /// `coverage`, `distance` (from the player), `distanceToCamera`, `renderers`. With `screen`,
    /// `cameraPosition`, `candidates` (entities that draw), `inFrustum`, `returned`, `truncated`.
    /// Occlusion is not evaluated: an entity behind a wall is listed, and a bounds box is larger
    /// than the mesh it wraps; a screenshot shows what is visible.
    pub(crate) fn list(
        &self,
        resolver: &SceneResolver,
        pending: &mut PendingSnapshotRequests,
        limit: Option<u32>,
        min_coverage: f32,
    ) -> Result<Listing, String> {
        if !(0.0..=1.0).contains(&min_coverage) {
            return Err(format!(
                "--min-coverage runs from 0 to 1; got {min_coverage}"
            ));
        }
        if let Some(limit) = limit.filter(|limit| *limit > MAX_LIMIT) {
            return Err(format!(
                "with --visible, --limit runs from 1 to {MAX_LIMIT}; got {limit}"
            ));
        }
        let (_, ctx) = resolver.resolve()?;
        let Ok((camera, camera_transform, frustum, projection)) = self.camera.single() else {
            return Err("no primary camera".to_owned());
        };
        let Some(screen) = Screen::primary(&self.windows) else {
            return Err("no primary window".to_owned());
        };
        let Some(viewport) = camera.logical_viewport_rect() else {
            return Err("the camera has no viewport yet".to_owned());
        };
        let near = match projection {
            Projection::Perspective(p) => p.near,
            Projection::Orthographic(p) => p.near,
            _ => 0.1,
        };
        let player = self.player.single().ok().map(|t| t.translation());
        let clip_from_view = camera.clip_from_view();

        let mut placed = Vec::new();
        for entity in ctx.live_scene_entities() {
            if entity.id <= SceneEntityId::CAMERA.id {
                continue;
            }
            let Some(target) = ctx.bevy_entity(entity) else {
                continue;
            };
            let boxes = own_meshes(target, &self.meshes, &self.children, &self.scene_entities);
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
        let limit = limit.map_or(DEFAULT_LIMIT, |limit| limit as usize);
        let scene = ctx.hash.clone();
        let (tx, rx) = tokio::sync::oneshot::channel();
        resolver.request_snapshot(pending, move |crdt| {
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
        })?;
        Ok(rx)
    }
}

/// The entities of the scene that draw a primitive mesh or a glTF.
fn ctx_candidates(crdt: &dcl_types::interface::CrdtStore) -> usize {
    [
        dcl_component::SceneComponentId::MESH_RENDERER,
        dcl_component::SceneComponentId::GLTF_CONTAINER,
    ]
    .iter()
    .filter_map(|component| crdt.lww.get(component))
    .flat_map(|state| state.last_write.keys().copied())
    .filter(|entity| labels::renders(crdt, *entity))
    .collect::<std::collections::HashSet<_>>()
    .len()
}

/// The visible meshes of `target` and of its descendants that are not themselves scene entities
/// (a child entity is listed on its own), each with where it is placed. The bounds anchor of
/// `/entity_screen_position --bounds` and the entity pointer commands walks child entities too, so
/// a parent's rect here can be smaller than the box those aim at the centre of.
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

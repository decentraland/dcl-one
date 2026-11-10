//! A synthetic pointer: an aim at one scene entity that the pointer pipeline follows instead of
//! the cursor, so a console command can hover and click an entity without moving the mouse. The
//! ray is cast from the camera to the entity's anchor point and then runs through the same casts,
//! distance checks and result writers as a real cursor.

use bevy::{
    diagnostic::FrameCount,
    math::{Affine3A, Mat3A, Vec3A},
    prelude::*,
    render::primitives::Aabb,
};
use dcl_component::{
    proto_components::sdk::components::{
        common::{InputAction, PointerEventType},
        pb_pointer_events::Entry,
    },
    SceneEntityId,
};
use serde::Serialize;

use crate::update_world::pointer_events::PointerEvents;

use super::pointer_results::{passes_distance_check, ActionCandidateMode, PointerTargetInfo};

/// The active aim, if any. While it holds one the pointer ray goes from the camera to the aimed
/// entity, whatever the cursor does, and the UI gate is open.
#[derive(Resource, Default)]
pub struct SyntheticPointer(pub Option<SyntheticAim>);

#[derive(Debug, Clone)]
pub struct SyntheticAim {
    /// the scene the entity belongs to
    pub scene: Entity,
    /// the entity as the scene names it
    pub entity: SceneEntityId,
    /// the bevy entity aimed at
    pub target: Entity,
    pub anchor: Anchor,
    pub hold: Hold,
    /// the frame the aim was set on
    pub started_frame: u32,
}

/// Where on the entity the aim points
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Anchor {
    /// the entity's own position
    Origin,
    /// the centre of the bounds of the visible meshes of the entity and its descendants
    Bounds,
}

/// How long the aim lasts
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Hold {
    /// this many frames from `started_frame`
    Frames(u32),
    /// until it is replaced or cleared
    Open,
}

/// The pointer results written while an aim is active, oldest first
#[derive(Resource, Default)]
pub struct PointerEventLog(pub Vec<LoggedPointerEvent>);

#[derive(Debug, Clone)]
pub struct LoggedPointerEvent {
    pub container: Entity,
    pub entity: SceneEntityId,
    pub event: PointerEventType,
    pub button: InputAction,
    /// the scene tick the result was written for
    pub tick: u32,
    pub frame: u32,
}

/// One cursor pointer-event entry of the hovered entity, as the hover feedback would show it
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HoverEntryReport {
    pub button: String,
    pub event: String,
    pub hover_text: Option<String>,
    pub enabled: bool,
    pub too_far: bool,
}

/// The ray from the camera through the anchor point; `None` when the two coincide
pub fn aim_ray(camera: Vec3, anchor_point: Vec3) -> Option<Ray3d> {
    Dir3::new(anchor_point - camera)
        .ok()
        .map(|direction| Ray3d::new(camera, direction))
}

/// The world point the anchor names on `target`: its translation for `Origin`, the centre of the
/// bounds of its visible meshes and its descendants' for `Bounds`. `None` when the entity has no
/// position, or for `Bounds` no visible mesh with bounds (invisible, or still loading).
pub fn anchor_point(
    target: Entity,
    anchor: Anchor,
    transforms: &Query<&GlobalTransform>,
    meshes: &Query<(&Aabb, &GlobalTransform, &InheritedVisibility)>,
    children: &Query<&Children>,
) -> Option<Vec3> {
    match anchor {
        Anchor::Origin => transforms.get(target).ok().map(|t| t.translation()),
        Anchor::Bounds => world_bounds_centre(
            std::iter::once(target)
                .chain(children.iter_descendants(target))
                .filter_map(|e| meshes.get(e).ok())
                .filter(|(_, _, visible)| visible.get())
                .map(|(aabb, transform, _)| (*aabb, transform.affine())),
        ),
    }
}

/// The centre of the box around every local-space `Aabb`, each placed in the world by its transform
fn world_bounds_centre(boxes: impl IntoIterator<Item = (Aabb, Affine3A)>) -> Option<Vec3> {
    let mut bounds: Option<(Vec3A, Vec3A)> = None;
    for (aabb, placement) in boxes {
        let centre = placement.transform_point3a(aabb.center);
        // the half extents of the placed box: its axes' extents, summed absolute
        let axes = placement.matrix3;
        let abs = Mat3A::from_cols(axes.x_axis.abs(), axes.y_axis.abs(), axes.z_axis.abs());
        let half = abs * aabb.half_extents;
        if !(centre.is_finite() && half.is_finite()) {
            continue;
        }
        let (min, max) = (centre - half, centre + half);
        bounds = Some(match bounds {
            Some((lo, hi)) => (lo.min(min), hi.max(max)),
            None => (min, max),
        });
    }
    bounds.map(|(min, max)| Vec3::from((min + max) * 0.5))
}

/// Whether the pointer target may be taken from the world: the cursor is over the 3D view (a UI
/// root is interacting), or a synthetic aim is active, which has no cursor to gate
pub fn ui_gate_passes(any_root_interacting: bool, synthetic_active: bool) -> bool {
    synthetic_active || any_root_interacting
}

/// The cursor entries of `pe` (proximity entries are left out), each with whether the target's
/// distances pass its distance check
pub fn cursor_entries<'a>(
    info: &PointerTargetInfo,
    pe: &'a PointerEvents,
) -> impl Iterator<Item = (&'a Entry, bool)> {
    let (camera_distance, distance) = (info.camera_distance.0, info.distance.0);
    pe.iter()
        .filter(|entry| ActionCandidateMode::Cursor.matches_entry(entry))
        .map(move |entry| {
            let in_reach =
                passes_distance_check(entry.event_info.as_ref(), camera_distance, distance);
            (entry, in_reach)
        })
}

/// The hovered entity's cursor entries: enabled when in the scene and in reach, too far when in
/// the scene and out of reach
pub fn hover_entries(info: &PointerTargetInfo, pe: &PointerEvents) -> Vec<HoverEntryReport> {
    cursor_entries(info, pe)
        .map(|(entry, in_reach)| {
            let event_info = entry.event_info.as_ref();
            let button = event_info
                .and_then(|i| i.button.map(|_| i.button()))
                .unwrap_or(InputAction::IaAny);
            HoverEntryReport {
                button: format!("{button:?}"),
                event: format!("{:?}", entry.event_type()),
                hover_text: event_info.and_then(|i| i.hover_text.clone()),
                enabled: info.in_scene && in_reach,
                too_far: info.in_scene && !in_reach,
            }
        })
        .collect()
}

/// Ends a `Hold::Frames` aim once its frames have passed, and clears the log with it
pub fn expire_synthetic_aim(
    frame: Res<FrameCount>,
    mut synthetic: ResMut<SyntheticPointer>,
    mut log: ResMut<PointerEventLog>,
) {
    let expired = matches!(
        synthetic.0,
        Some(SyntheticAim { hold: Hold::Frames(n), started_frame, .. })
            if frame.0 >= started_frame.saturating_add(n)
    );
    if expired {
        synthetic.0 = None;
        log.0.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::{ecs::system::SystemState, math::FloatOrd};
    use common::structs::PointerTargetType;
    use dcl_component::proto_components::sdk::components::{
        common::InteractionType, pb_pointer_events::Info, PbPointerEvents,
    };

    const EPSILON: f32 = 1e-3;

    fn entry(event: PointerEventType, max_distance: Option<f32>, proximity: bool) -> Entry {
        Entry {
            event_type: event as i32,
            interaction_type: proximity.then_some(InteractionType::Proximity as i32),
            event_info: Some(Info {
                button: Some(InputAction::IaPointer as i32),
                max_distance,
                ..Default::default()
            }),
        }
    }

    fn dummy_info() -> PointerTargetInfo {
        PointerTargetInfo {
            container: Entity::from_raw(1),
            mesh_name: None,
            distance: FloatOrd(0.0),
            camera_distance: FloatOrd(0.0),
            in_scene: false,
            position: None,
            normal: None,
            face: None,
            ty: PointerTargetType::World,
        }
    }
    #[test]
    fn aim_ray_points_from_the_camera_to_the_anchor() {
        let ray = aim_ray(Vec3::new(0.0, 2.0, 0.0), Vec3::new(10.0, 2.0, 0.0)).unwrap();
        assert_eq!(ray.origin, Vec3::new(0.0, 2.0, 0.0));
        assert!((Vec3::from(ray.direction) - Vec3::X).length() < 1e-6);
        assert!(aim_ray(Vec3::ONE, Vec3::ONE).is_none());
    }
    #[test]
    fn aim_ray_ignores_the_cursor() {
        // the gate and the ray are decided by the aim alone
        assert!(ui_gate_passes(false, true));
        assert!(!ui_gate_passes(false, false));
        assert!(ui_gate_passes(true, false));
    }
    #[test]
    fn hover_entries_report_distance_and_too_far() {
        // one cursor entry with max_distance 5, one proximity entry; player 8 m away, in scene
        let pe = PointerEvents::from(PbPointerEvents {
            pointer_events: vec![
                entry(PointerEventType::PetDown, Some(5.0), false),
                entry(PointerEventType::PetDown, None, true),
            ],
        });
        let info = PointerTargetInfo {
            distance: FloatOrd(8.0),
            camera_distance: FloatOrd(9.0),
            in_scene: true,
            ..dummy_info()
        };
        let entries = hover_entries(&info, &pe);
        assert_eq!(entries.len(), 1);
        assert_eq!((entries[0].enabled, entries[0].too_far), (false, true));
    }

    #[test]
    fn hover_entries_name_the_button_and_event() {
        let pe = PointerEvents::from(PbPointerEvents {
            pointer_events: vec![entry(PointerEventType::PetDown, None, false)],
        });
        let info = PointerTargetInfo {
            distance: FloatOrd(2.0),
            in_scene: true,
            ..dummy_info()
        };
        let entries = hover_entries(&info, &pe);
        assert_eq!(entries[0].button, "IaPointer");
        assert_eq!(entries[0].event, "PetDown");
        assert_eq!((entries[0].enabled, entries[0].too_far), (true, false));
        // out of the scene, neither enabled nor too far
        let outside = hover_entries(&dummy_info(), &pe);
        assert_eq!((outside[0].enabled, outside[0].too_far), (false, false));
    }

    #[test]
    fn an_aim_held_for_frames_expires_and_clears_the_log() {
        let aim = |hold, started_frame| SyntheticAim {
            scene: Entity::from_raw(1),
            entity: SceneEntityId::from_proto_u32(512),
            target: Entity::from_raw(2),
            anchor: Anchor::Origin,
            hold,
            started_frame,
        };
        let mut app = App::new();
        app.init_resource::<FrameCount>()
            .init_resource::<PointerEventLog>()
            .insert_resource(SyntheticPointer(Some(aim(Hold::Frames(2), 10))))
            .add_systems(Update, expire_synthetic_aim);
        app.world_mut()
            .resource_mut::<PointerEventLog>()
            .0
            .push(LoggedPointerEvent {
                container: Entity::from_raw(2),
                entity: SceneEntityId::from_proto_u32(512),
                event: PointerEventType::PetDown,
                button: InputAction::IaPointer,
                tick: 0,
                frame: 10,
            });
        let run_at = |app: &mut App, frame: u32| {
            app.world_mut().resource_mut::<FrameCount>().0 = frame;
            app.update();
            app.world().resource::<SyntheticPointer>().0.is_some()
        };
        assert!(run_at(&mut app, 11));
        assert_eq!(app.world().resource::<PointerEventLog>().0.len(), 1);
        assert!(!run_at(&mut app, 12));
        assert!(app.world().resource::<PointerEventLog>().0.is_empty());

        // an open aim stays
        app.world_mut().resource_mut::<SyntheticPointer>().0 = Some(aim(Hold::Open, 0));
        assert!(run_at(&mut app, 1000));
    }

    #[test]
    fn anchor_points_follow_the_origin_and_the_visible_bounds() {
        let mut world = World::new();
        let child = world
            .spawn((
                Aabb::from_min_max(Vec3::splat(-1.0), Vec3::splat(1.0)),
                GlobalTransform::from_xyz(10.0, 0.0, 0.0),
                InheritedVisibility::VISIBLE,
            ))
            .id();
        let parent = world
            .spawn(GlobalTransform::from_xyz(0.0, 5.0, 0.0))
            .add_child(child)
            .id();
        let bare = world.spawn(GlobalTransform::from_xyz(1.0, 2.0, 3.0)).id();
        let mut state = SystemState::<(
            Query<&GlobalTransform>,
            Query<(&Aabb, &GlobalTransform, &InheritedVisibility)>,
            Query<&Children>,
        )>::new(&mut world);
        let (transforms, meshes, children) = state.get(&world);
        let point = |e, a| anchor_point(e, a, &transforms, &meshes, &children);
        assert_eq!(
            point(parent, Anchor::Origin),
            Some(Vec3::new(0.0, 5.0, 0.0))
        );
        assert_eq!(
            point(parent, Anchor::Bounds),
            Some(Vec3::new(10.0, 0.0, 0.0))
        );
        // no visible mesh: no bounds
        assert_eq!(point(bare, Anchor::Bounds), None);
        assert_eq!(point(bare, Anchor::Origin), Some(Vec3::new(1.0, 2.0, 3.0)));
    }

    #[test]
    fn bounds_centre_is_the_centre_of_the_placed_boxes() {
        let unit = Aabb::from_min_max(Vec3::splat(-1.0), Vec3::splat(1.0));
        let shifted = Affine3A::from_translation(Vec3::new(10.0, 0.0, 0.0));
        let centre = world_bounds_centre([(unit, Affine3A::IDENTITY), (unit, shifted)]).unwrap();
        // the boxes span x from -1 to 11
        assert!(
            centre.distance(Vec3::new(5.0, 0.0, 0.0)) < EPSILON,
            "{centre}"
        );
    }

    #[test]
    fn bounds_follow_a_box_through_its_scale_and_translation() {
        let placement = Affine3A::from_scale_rotation_translation(
            Vec3::splat(2.0),
            Quat::IDENTITY,
            Vec3::new(0.0, 3.0, 0.0),
        );
        let aabb = Aabb::from_min_max(Vec3::ZERO, Vec3::splat(2.0));
        // the box spans (0, 3, 0) to (4, 7, 4)
        let centre = world_bounds_centre([(aabb, placement)]).unwrap();
        assert!(
            centre.distance(Vec3::new(2.0, 5.0, 2.0)) < EPSILON,
            "{centre}"
        );
    }

    #[test]
    fn bounds_widen_for_a_box_turned_a_quarter() {
        // 4 long on x, turned about Y so it is 4 long on z: it spans z from -2 to 2
        let turned = Affine3A::from_rotation_translation(
            Quat::from_rotation_y(std::f32::consts::FRAC_PI_2),
            Vec3::ZERO,
        );
        let long = Aabb::from_min_max(Vec3::new(-2.0, -1.0, -1.0), Vec3::new(2.0, 1.0, 1.0));
        let far = Aabb::from_min_max(Vec3::new(-0.5, -0.5, 4.5), Vec3::new(0.5, 0.5, 5.5));
        let centre = world_bounds_centre([(long, turned), (far, Affine3A::IDENTITY)]).unwrap();
        // z spans -2 to 5.5; unturned it would span -1 to 5.5
        assert!(
            centre.distance(Vec3::new(0.0, 0.0, 1.75)) < EPSILON,
            "{centre}"
        );
    }

    #[test]
    fn bounds_skip_boxes_that_are_not_finite_and_need_one_box() {
        let broken = Affine3A::from_scale(Vec3::splat(f32::NAN));
        let unit = Aabb::from_min_max(Vec3::splat(-1.0), Vec3::splat(1.0));
        let centre = world_bounds_centre([(unit, broken), (unit, Affine3A::IDENTITY)]).unwrap();
        assert!(centre.length() < EPSILON);
        assert_eq!(world_bounds_centre([]), None);
        assert_eq!(world_bounds_centre([(unit, broken)]), None);
    }
}

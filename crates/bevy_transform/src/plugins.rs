use crate::systems::{mark_dirty_trees, propagate_parent_transforms, sync_simple_transforms};
use bevy_app::{App, Plugin, PostStartup, PostUpdate};
use bevy_ecs::{
    schedule::{IntoScheduleConfigs, ScheduleLabel, SystemSet},
    world::World,
};

/// Set enum for the systems relating to transform propagation
#[derive(Debug, Hash, PartialEq, Eq, Clone, SystemSet)]
pub enum TransformSystem {
    /// Propagates changes in transform to children's [`GlobalTransform`](crate::components::GlobalTransform)
    TransformPropagate,
}

/// The schedule that holds the transform propagation systems.
/// [`TransformSystem::TransformPropagate`] runs it in `PostStartup` and `PostUpdate`, and an app
/// that needs current globals earlier in a frame runs it there too, with
/// [`propagate_transforms`]. The systems are the same instances at every point, so each run
/// propagates only what changed since the previous run, whichever point ran it.
#[derive(ScheduleLabel, Debug, Hash, PartialEq, Eq, Clone)]
pub struct PropagateTransforms;

/// Runs the [`PropagateTransforms`] schedule.
pub fn propagate_transforms(world: &mut World) {
    world.run_schedule(PropagateTransforms);
}

/// The base plugin for handling [`Transform`](crate::components::Transform) components
#[derive(Default)]
pub struct TransformPlugin;

impl Plugin for TransformPlugin {
    fn build(&self, app: &mut App) {
        #[cfg(feature = "bevy_reflect")]
        app.register_type::<crate::components::Transform>()
            .register_type::<crate::components::TransformTreeChanged>()
            .register_type::<crate::components::GlobalTransform>();

        app.add_systems(
            PropagateTransforms,
            (
                mark_dirty_trees,
                propagate_parent_transforms,
                // TODO: Adjust the internal parallel queries to make this system more efficiently share and fill CPU time.
                sync_simple_transforms,
            )
                .chain(),
        )
        // add transform systems to startup so the first update is "correct"
        .add_systems(
            PostStartup,
            propagate_transforms.in_set(TransformSystem::TransformPropagate),
        )
        .add_systems(
            PostUpdate,
            propagate_transforms.in_set(TransformSystem::TransformPropagate),
        );
    }
}

#[cfg(test)]
mod tests {
    //! Transform propagation run twice a frame, as the explorer does: an early pass of
    //! [`PropagateTransforms`] ahead of the app's own systems, then
    //! [`TransformSystem::TransformPropagate`]. Each frame writes to different parts of a few
    //! trees before the early pass, between the passes and after the late pass, and checks after
    //! each pass that
    //! - every `GlobalTransform` matches its `Transform` chain, and
    //! - the pass wrote `GlobalTransform` only in trees written since the other pass ran, and
    //! - in a hierarchy, the pass wrote no `GlobalTransform` whose value stayed the same: a root
    //!   marked changed with its old value makes every direct child recompute.
    use super::*;
    use crate::{
        components::{GlobalTransform, Transform},
        systems::{mark_dirty_trees, propagate_parent_transforms, sync_simple_transforms},
    };
    use alloc::{format, string::String, vec::Vec};
    use bevy_app::{App, Last, Update};
    use bevy_ecs::{component::Tick, prelude::*};
    use bevy_tasks::{ComputeTaskPool, TaskPool};
    use std::collections::{HashMap, HashSet};

    #[derive(SystemSet, Debug, Hash, PartialEq, Eq, Clone)]
    struct EarlyPropagate;

    #[derive(SystemSet, Debug, Hash, PartialEq, Eq, Clone)]
    struct BetweenPasses;

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    enum Stage {
        BeforeEarly,
        Between,
        AfterLate,
    }

    #[derive(Clone, Copy)]
    enum Op {
        Move(Entity),
        Reparent(Entity, Entity),
        Orphan(Entity),
        Spawn(Entity),
    }

    /// Tree `a`: `a -> a1 -> a2`, `a -> a1b`. Tree `b`: `b -> b1 -> b2`. Tree `c`: `c -> c1`.
    /// `s` has no hierarchy.
    #[derive(Resource, Clone, Copy)]
    struct Trees {
        a: Entity,
        a1: Entity,
        a2: Entity,
        b: Entity,
        b1: Entity,
        b2: Entity,
        c: Entity,
        c1: Entity,
        s: Entity,
    }

    #[derive(Resource, Default)]
    struct Frame(u32);

    #[derive(Resource, Default)]
    struct Touched {
        since_late: HashSet<Entity>,
        since_early: HashSet<Entity>,
    }

    #[derive(Resource, Default)]
    struct Ticks(Option<HashMap<Entity, (Tick, GlobalTransform)>>);

    #[derive(Resource, Default)]
    struct Checked {
        failures: Vec<String>,
        frames: u32,
        written_by_early: usize,
        written_by_late: usize,
    }

    #[expect(
        clippy::match_same_arms,
        reason = "one arm per step of the script, each with its own comment"
    )]
    fn script(frame: u32, stage: Stage, t: &Trees) -> Vec<Op> {
        use Op::*;
        use Stage::*;
        match (frame, stage) {
            // a leaf, before the early pass: its root `a` and sibling `a1b` keep their globals
            (2, BeforeEarly) => [Move(t.a2)].into(),
            // a mid node, between the passes
            (3, Between) => [Move(t.b1)].into(),
            // a root, after the late pass: the next early pass picks it up
            (4, AfterLate) => [Move(t.c)].into(),
            // the same tree written outside and between the passes, and the simple entity too
            (5, BeforeEarly) => [Move(t.a1), Move(t.s)].into(),
            (5, Between) => [Move(t.a2), Move(t.s)].into(),
            // a subtree moved to another tree between the passes
            (6, Between) => [Reparent(t.b2, t.a2)].into(),
            // an orphan between the passes, then its old root moves after the late pass
            (7, Between) => [Orphan(t.c1)].into(),
            (7, AfterLate) => [Move(t.c)].into(),
            // new children before and between the passes
            (8, BeforeEarly) => [Spawn(t.b)].into(),
            (8, Between) => [Spawn(t.c)].into(),
            // frame 9 changes nothing: neither pass may write
            // an old leaf of `b`, now under `a`, written before the early pass and its new
            // parent between the passes
            (10, BeforeEarly) => [Move(t.b2)].into(),
            (10, Between) => [Move(t.a2)].into(),
            // an orphan with descendants: a new root whose global changes
            (11, BeforeEarly) => [Orphan(t.a1)].into(),
            _ => Vec::new(),
        }
    }

    fn root(world: &World, mut entity: Entity) -> Entity {
        while let Some(child_of) = world.get::<ChildOf>(entity) {
            entity = child_of.parent();
        }
        entity
    }

    fn expected(world: &World, entity: Entity) -> GlobalTransform {
        let transform = *world.get::<Transform>(entity).unwrap();
        match world.get::<ChildOf>(entity) {
            Some(child_of) => expected(world, child_of.parent()).mul_transform(transform),
            None => GlobalTransform::from(transform),
        }
    }

    fn apply(world: &mut World, stage: Stage) {
        let frame = world.resource::<Frame>().0;
        let trees = *world.resource::<Trees>();
        let mut touched = Vec::new();
        for op in script(frame, stage, &trees) {
            match op {
                Op::Move(entity) => {
                    let k = frame as f32 + 0.25;
                    *world.get_mut::<Transform>(entity).unwrap() = Transform::from_xyz(k, -k, 0.5)
                        .with_rotation(bevy_math::Quat::from_rotation_y(k))
                        .with_scale(bevy_math::Vec3::splat(1.25));
                    touched.push(root(world, entity));
                }
                Op::Reparent(entity, parent) => {
                    touched.push(root(world, entity));
                    world.entity_mut(entity).insert(ChildOf(parent));
                    touched.push(root(world, entity));
                }
                Op::Orphan(entity) => {
                    touched.push(root(world, entity));
                    world.entity_mut(entity).remove::<ChildOf>();
                    touched.push(entity);
                }
                Op::Spawn(parent) => {
                    world.spawn((Transform::from_xyz(3.0, 2.0, 1.0), ChildOf(parent)));
                    touched.push(root(world, parent));
                }
            }
        }
        let mut set = world.resource_mut::<Touched>();
        let set = match stage {
            Stage::Between => &mut set.since_early,
            Stage::BeforeEarly | Stage::AfterLate => &mut set.since_late,
        };
        set.extend(touched);
    }

    /// Checks the globals and the writes of the pass that just ran, recording what is wrong.
    fn check(world: &mut World, early: bool) {
        let frame = world.resource::<Frame>().0;
        let pass = if early { "early" } else { "late" };
        let entities: Vec<Entity> = world
            .query_filtered::<Entity, With<GlobalTransform>>()
            .iter(world)
            .collect();
        let mut failures = Vec::new();
        let mut ticks = HashMap::new();
        for &entity in &entities {
            let global = world.entity(entity).get_ref::<GlobalTransform>().unwrap();
            let want = expected(world, entity);
            if !global.affine().abs_diff_eq(want.affine(), 1e-4) {
                failures.push(format!(
                    "frame {frame}, after the {pass} pass: {entity} is stale at {}, expected {}",
                    global.translation(),
                    want.translation(),
                ));
            }
            ticks.insert(entity, (global.last_changed(), *global));
        }
        let touched = {
            let mut touched = world.resource_mut::<Touched>();
            if early {
                core::mem::take(&mut touched.since_late)
            } else {
                core::mem::take(&mut touched.since_early)
            }
        };
        let previous = world.resource_mut::<Ticks>().0.replace(ticks.clone());
        let (written, unchanged): (Vec<Entity>, Vec<Entity>) = match previous {
            Some(previous) => {
                let written: Vec<Entity> = ticks
                    .iter()
                    .filter(|(entity, (tick, _))| {
                        previous.get(entity).map(|(tick, _)| tick) != Some(tick)
                    })
                    .map(|(entity, _)| *entity)
                    .collect();
                let unchanged = written
                    .iter()
                    .copied()
                    .filter(|entity| {
                        let in_hierarchy = world.get::<ChildOf>(*entity).is_some()
                            || world.get::<Children>(*entity).is_some();
                        in_hierarchy
                            && previous.get(entity).map(|(_, global)| global)
                                == ticks.get(entity).map(|(_, global)| global)
                    })
                    .collect();
                (written, unchanged)
            }
            None => (Vec::new(), Vec::new()),
        };
        if !unchanged.is_empty() {
            failures.push(format!(
                "frame {frame}: the {pass} pass marked {} hierarchy globals changed without \
                 changing their value: {unchanged:?}",
                unchanged.len(),
            ));
        }
        let outside: Vec<Entity> = written
            .iter()
            .copied()
            .filter(|entity| !touched.contains(&root(world, *entity)))
            .collect();
        if !outside.is_empty() {
            failures.push(format!(
                "frame {frame}: the {pass} pass rewrote {} globals in trees nothing wrote since \
                 the other pass ran: {outside:?}",
                outside.len(),
            ));
        }
        let mut checked = world.resource_mut::<Checked>();
        checked.failures.extend(failures);
        if early {
            checked.written_by_early += written.len();
        } else {
            checked.written_by_late += written.len();
            checked.frames += 1;
        }
    }

    fn setup(world: &mut World) {
        let spawn = |world: &mut World, parent: Option<Entity>, x: f32| {
            let mut entity = world.spawn(Transform::from_xyz(x, 1.0, 0.0));
            if let Some(parent) = parent {
                entity.insert(ChildOf(parent));
            }
            entity.id()
        };
        let a = spawn(world, None, 1.0);
        let a1 = spawn(world, Some(a), 2.0);
        let a2 = spawn(world, Some(a1), 3.0);
        spawn(world, Some(a), 4.0);
        let b = spawn(world, None, 5.0);
        let b1 = spawn(world, Some(b), 6.0);
        let b2 = spawn(world, Some(b1), 7.0);
        let c = spawn(world, None, 8.0);
        let c1 = spawn(world, Some(c), 9.0);
        let s = spawn(world, None, 10.0);
        world.insert_resource(Trees {
            a,
            a1,
            a2,
            b,
            b1,
            b2,
            c,
            c1,
            s,
        });
    }

    fn run(add_early_pass: impl FnOnce(&mut App)) -> Checked {
        ComputeTaskPool::get_or_init(TaskPool::default);
        let mut app = App::new();
        app.add_plugins(TransformPlugin)
            .init_resource::<Frame>()
            .init_resource::<Touched>()
            .init_resource::<Ticks>()
            .init_resource::<Checked>()
            .configure_sets(
                PostUpdate,
                (
                    EarlyPropagate,
                    BetweenPasses,
                    TransformSystem::TransformPropagate,
                )
                    .chain(),
            )
            .add_systems(
                Update,
                (
                    |mut frame: ResMut<Frame>| frame.0 += 1,
                    |world: &mut World| apply(world, Stage::BeforeEarly),
                )
                    .chain(),
            )
            .add_systems(
                PostUpdate,
                (
                    (|world: &mut World| check(world, true)).after(EarlyPropagate),
                    |world: &mut World| apply(world, Stage::Between),
                )
                    .chain()
                    .in_set(BetweenPasses),
            )
            .add_systems(
                PostUpdate,
                (|world: &mut World| check(world, false))
                    .after(TransformSystem::TransformPropagate),
            )
            .add_systems(Last, |world: &mut World| apply(world, Stage::AfterLate));
        add_early_pass(&mut app);
        setup(app.world_mut());
        for _ in 0..12 {
            app.update();
        }
        app.world_mut().remove_resource::<Checked>().unwrap()
    }

    #[test]
    fn two_runs_a_frame_propagate_every_change_once() {
        let checked = run(|app| {
            app.add_systems(PostUpdate, propagate_transforms.in_set(EarlyPropagate));
        });
        assert!(checked.failures.is_empty(), "{:#?}", checked.failures);
        // both passes of every frame were checked, and both propagated something
        assert_eq!(checked.frames, 12);
        assert!(checked.written_by_early > 0 && checked.written_by_late > 0);
    }

    /// The setup [`PropagateTransforms`] replaces: a second instance of the systems for the early
    /// pass. Its globals are right, but its change detection is relative to its own previous run,
    /// a frame ago, so each pass rewalks what the other has just propagated.
    #[test]
    fn a_second_instance_of_the_systems_rewalks_the_other_passes_work() {
        let checked = run(|app| {
            app.add_systems(
                PostUpdate,
                (
                    mark_dirty_trees,
                    propagate_parent_transforms,
                    sync_simple_transforms,
                )
                    .chain()
                    .in_set(EarlyPropagate),
            );
        });
        assert!(
            !checked
                .failures
                .iter()
                .any(|failure| failure.contains("stale")),
            "{:#?}",
            checked.failures
        );
        assert!(
            checked
                .failures
                .iter()
                .any(|failure| failure.contains("rewrote")),
            "expected rewalks, got none"
        );
    }
}

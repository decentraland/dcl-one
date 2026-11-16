//! The spring-bone solver: plain data and maths, with no entities, so it runs the same in a test
//! as in the frame.
//!
//! A chain is a list of joints hanging off one anchor bone. Each joint keeps a tail point in
//! world space; a step moves the tail by its own inertia, a pull towards the rest direction and
//! a constant force, then puts it back at the bone's length from the joint. The joint turns to
//! point at its tail. Only rotations come out: bones never stretch or move.
//!
//! Steps have a fixed length, so the motion does not depend on the frame rate. A frame runs as
//! many as fit the time that passed, and the rotation shown is blended between the last two.

use bevy::math::{Mat4, Quat, Vec3};

/// Length of one step, in seconds.
pub const FIXED_STEP: f32 = 1.0 / 60.0;
/// Most steps one frame runs. A frame longer than this many steps is not simulated at all.
pub const MAX_SUBSTEPS: usize = 4;
/// Most joints one chain simulates. Whoever builds a chain keeps it within this; the solver
/// checks it in debug builds and does not shorten a chain itself.
pub const MAX_CHAIN_JOINTS: usize = 12;
/// Length given to a chain's last joint, which has no next joint to measure to.
pub const TIP_LENGTH: f32 = 0.1;
/// Below this a length or a scale is treated as zero.
const EPSILON: f32 = 1e-4;
/// Below this a squared length has no direction. The value is compared with squared lengths on
/// purpose, so the cut-off is a length of 0.01: a bone shorter than 1 cm, after scaling, keeps
/// its rest rotation and never swings.
const DIRECTION_EPSILON_SQ: f32 = 1e-4;

/// What a joint is measured as when its chain is snapped. The solver reads it on every step and
/// never changes it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct JointConfig {
    pub stiffness: f32,
    pub drag: f32,
    /// world-space direction times power; not normalised
    pub gravity: Vec3,
    /// the joint's local rotation at rest
    pub rest_rotation: Quat,
    /// direction to the next joint, in the joint's own space
    pub bone_axis: Vec3,
    /// distance to the next joint, in world units at the rest scale
    pub length: f32,
    /// the joint's position and scale under the joint before it (the anchor, for the first).
    /// Each step places the joint from these, so a joint whose own position or scale is
    /// animated is followed only as often as the caller measures them again.
    pub local_translation: Vec3,
    pub local_scale: Vec3,
}

impl JointConfig {
    /// The axis and length of a joint from the next joint's local position and world scale.
    pub fn axis_and_length(next_local_translation: Vec3, next_world_scale: Vec3) -> (Vec3, f32) {
        let to_next = next_local_translation * next_world_scale;
        let length = to_next.length();
        if length > EPSILON {
            (to_next / length, length)
        } else {
            (Vec3::Y, length)
        }
    }
}

/// The anchor bone's world pose for one frame.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AnchorPose {
    pub translation: Vec3,
    pub rotation: Quat,
    pub scale: Vec3,
    /// how much the anchor has grown since the chain's lengths were measured
    pub scale_factor: f32,
}

impl AnchorPose {
    fn lerp(&self, to: &Self, t: f32) -> Self {
        Self {
            translation: self.translation.lerp(to.translation, t),
            rotation: self.rotation.slerp(to.rotation, t),
            scale: self.scale.lerp(to.scale, t),
            scale_factor: self.scale_factor + (to.scale_factor - self.scale_factor) * t,
        }
    }

    fn matrix(&self) -> Mat4 {
        Mat4::from_scale_rotation_translation(self.scale, self.rotation, self.translation)
    }
}

/// Mean over the axes of the anchor's scale against its scale at rest. An axis with no rest
/// scale counts as unchanged.
pub fn scale_factor(scale: Vec3, rest_scale: Vec3) -> f32 {
    let ratio = |now: f32, rest: f32| if rest > EPSILON { now / rest } else { 1.0 };
    (ratio(scale.x, rest_scale.x) + ratio(scale.y, rest_scale.y) + ratio(scale.z, rest_scale.z))
        / 3.0
}

/// The shortest rotation turning `from` onto `to`. Identity when either has no direction, which
/// is any vector shorter than 0.01 (see [`DIRECTION_EPSILON_SQ`]).
pub fn from_to(from: Vec3, to: Vec3) -> Quat {
    if from.length_squared() < DIRECTION_EPSILON_SQ || to.length_squared() < DIRECTION_EPSILON_SQ {
        return Quat::IDENTITY;
    }
    let from = from.normalize();
    let to = to.normalize();
    let dot = from.dot(to);

    // opposite directions have no shortest turn: take a half turn about any perpendicular
    if dot <= -0.999999 {
        let mut axis = from.cross(Vec3::X);
        if axis.length_squared() < DIRECTION_EPSILON_SQ {
            axis = from.cross(Vec3::Y);
        }
        return Quat::from_axis_angle(axis.normalize(), std::f32::consts::PI);
    }

    let cross = from.cross(to);
    Quat::from_xyzw(cross.x, cross.y, cross.z, 1.0 + dot).normalize()
}

/// What moves for a joint, all in world space.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct JointState {
    pub prev_tail: Vec3,
    pub tail: Vec3,
    /// the joint's rotation after the step before last, and after the last
    pub prev_rotation: Quat,
    pub rotation: Quat,
}

impl JointState {
    /// A joint at rest where it stands: no velocity, and nothing to blend from.
    pub fn snapped(translation: Vec3, rotation: Quat) -> Self {
        Self {
            prev_tail: translation,
            tail: translation,
            prev_rotation: rotation,
            rotation,
        }
    }

    /// The rotation to show, `blend` of the way from the step before last to the last.
    pub fn blended(&self, blend: f32) -> Quat {
        self.prev_rotation.slerp(self.rotation, blend)
    }
}

/// Runs one step of a chain, from the anchor to the tip. `configs` and `states` are the chain's
/// joints in order, one state for each config; each joint is solved under the one before it.
pub fn step_chain(configs: &[JointConfig], states: &mut [JointState], anchor: &AnchorPose, h: f32) {
    debug_assert_eq!(
        configs.len(),
        states.len(),
        "a chain has one state for each joint"
    );
    debug_assert!(
        configs.len() <= MAX_CHAIN_JOINTS,
        "a chain has at most {MAX_CHAIN_JOINTS} joints"
    );

    let mut parent_rotation = anchor.rotation;
    let mut parent_matrix = anchor.matrix();

    for (config, state) in configs.iter().zip(states.iter_mut()) {
        let head = parent_matrix.transform_point3(config.local_translation);
        let rest_rotation = parent_rotation * config.rest_rotation;
        let rest_direction = rest_rotation * config.bone_axis;

        // drag is the share of velocity lost over one 60 Hz step, whatever `h` is
        let retention = (1.0 - config.drag).max(1e-6).powf(60.0 * h);
        let moved = state.tail
            + (state.tail - state.prev_tail) * retention
            + rest_direction * config.stiffness * h
            + config.gravity * h;

        let to_tail = moved - head;
        let distance = to_tail.length();
        let direction = if distance > EPSILON {
            to_tail / distance
        } else {
            rest_direction
        };
        let tail = head + direction * config.length * anchor.scale_factor;

        let rotation = from_to(rest_direction, tail - head) * rest_rotation;

        state.prev_tail = state.tail;
        state.tail = tail;
        state.prev_rotation = state.rotation;
        state.rotation = rotation;

        let local_rotation = (parent_rotation.inverse() * rotation).normalize();
        parent_matrix *= Mat4::from_scale_rotation_translation(
            config.local_scale,
            local_rotation,
            config.local_translation,
        );
        parent_rotation = rotation;
    }
}

/// What one frame does, the same for every chain.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FramePlan {
    /// Too much time passed to simulate. No chain steps, and each chain's state is stale from
    /// here on: [`ChainState::simulate`] answers [`Shown::Skipped`], and the caller replaces the
    /// chain with a new [`ChainState::snapped`] before it steps again.
    Skip,
    Run {
        /// For each step, how far the anchor is from last frame's pose to this frame's.
        anchor_alphas: [f32; MAX_SUBSTEPS],
        substeps: usize,
        /// How far the shown rotation is from the step before last to the last.
        blend: f32,
        /// False until a step has run since the last skip: there is nothing to show yet.
        write: bool,
    },
}

/// Turns frame times into fixed steps.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Stepper {
    accumulated: f32,
    has_stepped: bool,
}

impl Stepper {
    pub fn advance(&mut self, dt: f32) -> FramePlan {
        if dt > MAX_SUBSTEPS as f32 * FIXED_STEP {
            *self = Self::default();
            return FramePlan::Skip;
        }

        self.accumulated += dt;
        let substeps = ((self.accumulated / FIXED_STEP) as usize).min(MAX_SUBSTEPS);
        let mut anchor_alphas = [1.0; MAX_SUBSTEPS];
        for alpha in anchor_alphas.iter_mut().take(substeps) {
            self.accumulated -= FIXED_STEP;
            if dt > 0.0 {
                *alpha = ((dt - self.accumulated) / dt).clamp(0.0, 1.0);
            }
            self.has_stepped = true;
        }
        // a frame that ran every step it may run does not owe the rest to the next one
        if substeps == MAX_SUBSTEPS {
            self.accumulated = 0.0;
        }

        FramePlan::Run {
            anchor_alphas,
            substeps,
            blend: (self.accumulated / FIXED_STEP).clamp(0.0, 1.0),
            write: self.has_stepped,
        }
    }
}

/// What a chain has to show after one frame.
#[derive(Debug, Clone, Copy, PartialEq)]
#[must_use = "a skipped chain must be snapped again before its next step"]
pub enum Shown {
    /// The frame was skipped and the chain left as it was. Its tails, rotations and anchor are
    /// from before the gap, so stepping them again would swing the chain towards where it stood
    /// then. Snap the chain again where it stands before its next step.
    Skipped,
    /// No step has run since the chain was last snapped: keep the pose already on the bones.
    Hold,
    /// Show each joint this far from the step before last to the last
    /// ([`JointState::blended`]).
    Blend(f32),
}

/// One chain's moving state across frames.
#[derive(Debug, Clone, PartialEq)]
pub struct ChainState {
    pub joints: Vec<JointState>,
    prev_anchor: AnchorPose,
    anchor: AnchorPose,
}

impl ChainState {
    /// A chain at rest where it stands. `joints` are the joints' current world positions and
    /// rotations.
    pub fn snapped(anchor: AnchorPose, joints: impl IntoIterator<Item = (Vec3, Quat)>) -> Self {
        Self {
            joints: joints
                .into_iter()
                .map(|(translation, rotation)| JointState::snapped(translation, rotation))
                .collect(),
            prev_anchor: anchor,
            anchor,
        }
    }

    /// Records this frame's anchor pose; the steps of the frame move between the last and this.
    pub fn set_anchor(&mut self, anchor: AnchorPose) {
        self.prev_anchor = self.anchor;
        self.anchor = anchor;
    }

    /// Runs the frame's steps and says what to show. `configs` are the joints the chain was
    /// snapped with, in the same order. A skipped frame changes nothing here: the caller snaps
    /// the chain again, as [`Shown::Skipped`] says.
    pub fn simulate(&mut self, configs: &[JointConfig], plan: &FramePlan) -> Shown {
        let FramePlan::Run {
            anchor_alphas,
            substeps,
            blend,
            write,
        } = plan
        else {
            return Shown::Skipped;
        };

        for alpha in anchor_alphas.iter().take(*substeps) {
            let anchor = self.prev_anchor.lerp(&self.anchor, *alpha);
            step_chain(configs, &mut self.joints, &anchor, FIXED_STEP);
        }

        if *write {
            Shown::Blend(*blend)
        } else {
            Shown::Hold
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DEFAULT_STIFFNESS: f32 = 0.5;
    const DEFAULT_DRAG: f32 = 0.4;
    const BONE: f32 = 0.2;

    fn anchor_at(translation: Vec3) -> AnchorPose {
        AnchorPose {
            translation,
            rotation: Quat::IDENTITY,
            scale: Vec3::ONE,
            scale_factor: 1.0,
        }
    }

    /// `count` joints stacked along +Y, each `BONE` above the one before.
    fn chain(count: usize, gravity: Vec3) -> Vec<JointConfig> {
        (0..count)
            .map(|ix| JointConfig {
                stiffness: DEFAULT_STIFFNESS,
                drag: DEFAULT_DRAG,
                gravity,
                rest_rotation: Quat::IDENTITY,
                bone_axis: Vec3::Y,
                length: if ix + 1 == count { TIP_LENGTH } else { BONE },
                local_translation: Vec3::Y * BONE,
                local_scale: Vec3::ONE,
            })
            .collect()
    }

    fn snapped(configs: &[JointConfig], anchor: AnchorPose) -> ChainState {
        let joints = (0..configs.len())
            .map(|ix| {
                (
                    anchor.translation + Vec3::Y * BONE * (ix + 1) as f32,
                    Quat::IDENTITY,
                )
            })
            .collect::<Vec<_>>();
        ChainState::snapped(anchor, joints)
    }

    fn assert_close(a: Vec3, b: Vec3, tolerance: f32) {
        assert!(
            (a - b).length() < tolerance,
            "{a} is not within {tolerance} of {b}"
        );
    }

    /// Moves the anchor 1 m along +X over the first 10/60 s, then holds it for 3 s, in frames of
    /// `dt`. Returns the direction each joint ends up pointing.
    fn settle(dt: f32, gravity: Vec3) -> Vec<Vec3> {
        let configs = chain(3, gravity);
        let mut state = snapped(&configs, anchor_at(Vec3::ZERO));
        let mut stepper = Stepper::default();
        let mut time = 0.0;
        while time < 3.0 {
            time += dt;
            let travelled = (time / (10.0 / 60.0)).min(1.0);
            state.set_anchor(anchor_at(Vec3::X * travelled));
            let _ = state.simulate(&configs, &stepper.advance(dt));
        }
        state
            .joints
            .iter()
            .map(|joint| joint.rotation * Vec3::Y)
            .collect()
    }

    #[test]
    fn from_to_turns_one_direction_onto_another() {
        let rotation = from_to(Vec3::Y, Vec3::new(3.0, 0.0, 0.0));
        assert_close(rotation * Vec3::Y, Vec3::X, 1e-6);
    }

    #[test]
    fn from_to_is_identity_without_a_direction() {
        assert_eq!(from_to(Vec3::ZERO, Vec3::X), Quat::IDENTITY);
        assert_eq!(from_to(Vec3::X, Vec3::splat(1e-3)), Quat::IDENTITY);

        // the cut-off is a length of 0.01, not of 1e-4
        assert_eq!(from_to(Vec3::X, Vec3::Y * 0.009), Quat::IDENTITY);
        assert_close(from_to(Vec3::X, Vec3::Y * 0.011) * Vec3::X, Vec3::Y, 1e-6);
    }

    #[test]
    fn from_to_half_turns_between_opposite_directions() {
        for from in [Vec3::X, Vec3::Y, Vec3::Z, Vec3::new(1.0, 2.0, -3.0)] {
            let rotation = from_to(from, -from);
            assert_close(rotation * from, -from, 1e-5);
        }
    }

    #[test]
    fn axis_and_length_follow_the_next_joint_scaled() {
        let (axis, length) =
            JointConfig::axis_and_length(Vec3::new(0.0, 0.0, 0.5), Vec3::new(1.0, 1.0, 2.0));
        assert_close(axis, Vec3::Z, 1e-6);
        assert!((length - 1.0).abs() < 1e-6);

        let (axis, _) = JointConfig::axis_and_length(Vec3::ZERO, Vec3::ONE);
        assert_eq!(axis, Vec3::Y);
    }

    #[test]
    fn scale_factor_is_the_mean_growth_and_ignores_an_axis_without_rest_scale() {
        assert!((scale_factor(Vec3::splat(2.0), Vec3::ONE) - 2.0).abs() < 1e-6);
        assert!(
            (scale_factor(Vec3::new(2.0, 1.0, 3.0), Vec3::new(1.0, 1.0, 0.0)) - 4.0 / 3.0).abs()
                < 1e-6
        );
    }

    #[test]
    fn a_still_chain_stays_at_rest() {
        let configs = chain(3, Vec3::ZERO);
        let anchor = anchor_at(Vec3::ZERO);
        let mut state = snapped(&configs, anchor);
        for _ in 0..30 {
            step_chain(&configs, &mut state.joints, &anchor, FIXED_STEP);
        }
        for joint in &state.joints {
            assert_close(joint.rotation * Vec3::Y, Vec3::Y, 1e-5);
        }
    }

    #[test]
    fn the_first_step_points_between_rest_and_gravity() {
        // from a snap the tail sits on the joint, so the step's pull alone gives the direction
        let gravity = Vec3::X * 0.5;
        let configs = chain(1, gravity);
        let anchor = anchor_at(Vec3::ZERO);
        let mut state = snapped(&configs, anchor);

        step_chain(&configs, &mut state.joints, &anchor, FIXED_STEP);

        let expected = (Vec3::Y * DEFAULT_STIFFNESS + gravity).normalize();
        let head = Vec3::Y * BONE;
        assert_close(state.joints[0].tail, head + expected * TIP_LENGTH, 1e-6);
        assert_close(state.joints[0].rotation * Vec3::Y, expected, 1e-6);
    }

    #[test]
    fn every_step_keeps_each_tail_at_the_bone_length() {
        let configs = chain(3, Vec3::new(0.3, -1.0, 0.2));
        let mut state = snapped(&configs, anchor_at(Vec3::ZERO));
        let mut stepper = Stepper::default();
        for frame in 1..=40 {
            state.set_anchor(anchor_at(Vec3::X * (frame as f32 * 0.05).sin()));
            let _ = state.simulate(&configs, &stepper.advance(FIXED_STEP));

            let mut parent = state.anchor.matrix();
            let mut parent_rotation = Quat::IDENTITY;
            for (config, joint) in configs.iter().zip(&state.joints) {
                let head = parent.transform_point3(config.local_translation);
                assert!(((joint.tail - head).length() - config.length).abs() < 1e-5);
                parent *= Mat4::from_rotation_translation(
                    parent_rotation.inverse() * joint.rotation,
                    config.local_translation,
                );
                parent_rotation = joint.rotation;
            }
        }
    }

    #[test]
    fn a_moved_anchor_swings_the_chain_and_it_settles_back_to_rest() {
        let configs = chain(3, Vec3::ZERO);
        let mut state = snapped(&configs, anchor_at(Vec3::ZERO));
        let mut stepper = Stepper::default();
        let mut widest: f32 = 0.0;
        for frame in 1..=10 {
            state.set_anchor(anchor_at(Vec3::X * frame as f32 / 10.0));
            let _ = state.simulate(&configs, &stepper.advance(FIXED_STEP));
            widest = widest.max((state.joints[0].rotation * Vec3::Y).angle_between(Vec3::Y));
        }
        // the tails lag behind an anchor moving +X
        assert!(widest > 0.3, "{widest}");
        assert!((state.joints[0].rotation * Vec3::Y).x < 0.0);

        for direction in settle(FIXED_STEP, Vec3::ZERO) {
            assert_close(direction, Vec3::Y, 1e-3);
        }
    }

    #[test]
    fn the_chain_swings_the_same_way_at_any_frame_rate() {
        // the anchor moves 1 m along +X over the first 10/60 s, then holds
        let anchor_at_time = |time: f32| anchor_at(Vec3::X * (time / (10.0 / 60.0)).min(1.0));
        let direction = |rotation: Quat| rotation * Vec3::Y;

        for gravity in [Vec3::ZERO, Vec3::new(0.4, -1.0, 0.0)] {
            let configs = chain(3, gravity);

            // the swing itself: each joint's rotation after every step, one step a frame
            let rotations = |state: &ChainState| {
                state
                    .joints
                    .iter()
                    .map(|joint| joint.rotation)
                    .collect::<Vec<_>>()
            };
            let mut state = snapped(&configs, anchor_at(Vec3::ZERO));
            let mut after_step = vec![rotations(&state)];
            for step in 1..=180 {
                let anchor = anchor_at_time(step as f32 * FIXED_STEP);
                step_chain(&configs, &mut state.joints, &anchor, FIXED_STEP);
                after_step.push(rotations(&state));
            }
            let widest = after_step
                .iter()
                .map(|joints| direction(joints[0]).angle_between(Vec3::Y))
                .fold(0.0, f32::max);
            assert!(widest > 0.3, "{widest}");

            // every frame of every rate shows a point of that swing: one step behind the time
            // that passed, part way between the two steps around it
            for fps in [30.0, 60.0, 144.0] {
                let dt = 1.0 / fps;
                let mut state = snapped(&configs, anchor_at(Vec3::ZERO));
                let mut stepper = Stepper::default();
                let mut compared = 0;
                for frame in 1..(2.9 * fps) as usize {
                    let time = frame as f32 * dt;
                    state.set_anchor(anchor_at_time(time));
                    let Shown::Blend(blend) = state.simulate(&configs, &stepper.advance(dt)) else {
                        continue;
                    };

                    let steps = time / FIXED_STEP - 1.0;
                    let before = steps.floor().max(0.0) as usize;
                    let part = (steps - before as f32).clamp(0.0, 1.0);
                    for (ix, joint) in state.joints.iter().enumerate() {
                        let expected =
                            after_step[before][ix].slerp(after_step[before + 1][ix], part);
                        assert_close(direction(joint.blended(blend)), direction(expected), 1e-4);
                    }
                    compared += 1;
                }
                assert!(
                    compared as f32 > 2.5 * fps,
                    "{compared} frames at {fps} fps"
                );
            }
        }
    }

    #[test]
    fn drag_bounds_how_much_velocity_a_step_keeps() {
        let moving = JointState {
            prev_tail: Vec3::new(-0.01, BONE + TIP_LENGTH, 0.0),
            tail: Vec3::new(0.0, BONE + TIP_LENGTH, 0.0),
            prev_rotation: Quat::IDENTITY,
            rotation: Quat::IDENTITY,
        };
        let step = |drag: f32| {
            let mut configs = chain(1, Vec3::ZERO);
            configs[0].drag = drag;
            configs[0].stiffness = 0.0;
            let mut states = [moving];
            step_chain(&configs, &mut states, &anchor_at(Vec3::ZERO), FIXED_STEP);
            states[0].tail.x
        };

        let free = step(0.0);
        let half = step(0.5);
        assert!(free > half && half > 0.0, "{free} {half}");
        // full drag, or more, keeps no velocity
        assert!(step(1.0).abs() < 1e-6);
        assert!(step(3.0).abs() < 1e-6);
    }

    #[test]
    fn a_frame_runs_the_steps_that_fit_and_carries_the_rest() {
        let mut stepper = Stepper::default();

        // 144 fps: no step fits the first two frames, so there is nothing to show yet
        let dt = 1.0 / 144.0;
        for _ in 0..2 {
            assert!(matches!(
                stepper.advance(dt),
                FramePlan::Run {
                    substeps: 0,
                    write: false,
                    ..
                }
            ));
        }
        let FramePlan::Run {
            substeps,
            blend,
            write,
            ..
        } = stepper.advance(dt)
        else {
            panic!("skipped");
        };
        assert_eq!(substeps, 1);
        assert!(write);
        assert!((blend - (3.0 * dt - FIXED_STEP) / FIXED_STEP).abs() < 1e-4);

        // 30 fps: two steps, the anchor half way then all the way
        let mut stepper = Stepper::default();
        let FramePlan::Run {
            substeps,
            anchor_alphas,
            ..
        } = stepper.advance(2.0 * FIXED_STEP + 1e-5)
        else {
            panic!("skipped");
        };
        assert_eq!(substeps, 2);
        assert!((anchor_alphas[0] - 0.5).abs() < 1e-3);
        assert!((anchor_alphas[1] - 1.0).abs() < 1e-3);
    }

    #[test]
    fn a_frame_with_no_time_runs_no_step() {
        let mut stepper = Stepper::default();
        stepper.advance(FIXED_STEP * 1.5);
        let before = stepper;
        assert!(matches!(
            stepper.advance(0.0),
            FramePlan::Run {
                substeps: 0,
                write: true,
                ..
            }
        ));
        assert_eq!(stepper, before);
    }

    #[test]
    fn a_frame_at_the_step_limit_drops_what_is_left_over() {
        let mut stepper = Stepper::default();
        let FramePlan::Run {
            substeps, blend, ..
        } = stepper.advance(3.9 * FIXED_STEP)
        else {
            panic!("skipped");
        };
        assert_eq!(substeps, 3);
        assert!(blend > 0.8);

        let FramePlan::Run {
            substeps, blend, ..
        } = stepper.advance(3.9 * FIXED_STEP)
        else {
            panic!("skipped");
        };
        assert_eq!(substeps, MAX_SUBSTEPS);
        assert_eq!(blend, 0.0);
    }

    #[test]
    fn a_frame_too_long_is_skipped_and_leaves_the_chain_as_it_was() {
        let configs = chain(2, Vec3::ZERO);
        let mut state = snapped(&configs, anchor_at(Vec3::ZERO));
        let mut stepper = Stepper::default();
        stepper.advance(FIXED_STEP);

        let plan = stepper.advance(4.01 * FIXED_STEP);
        assert_eq!(plan, FramePlan::Skip);
        let before = state.joints.clone();
        state.set_anchor(anchor_at(Vec3::X * 50.0));
        assert_eq!(state.simulate(&configs, &plan), Shown::Skipped);
        assert_eq!(state.joints, before);

        // and the stepper starts over: nothing to show until a step runs again
        assert_eq!(stepper, Stepper::default());
    }

    #[test]
    fn a_chain_snapped_again_after_a_skipped_jump_does_not_swing_back() {
        let configs = chain(3, Vec3::ZERO);
        let far = anchor_at(Vec3::new(500.0, 0.0, -300.0));

        // a chain in mid swing when the avatar jumps away in one frame too long to simulate
        let swinging = || {
            let mut state = snapped(&configs, anchor_at(Vec3::ZERO));
            let mut stepper = Stepper::default();
            for frame in 1..=5 {
                state.set_anchor(anchor_at(Vec3::X * frame as f32 / 10.0));
                let _ = state.simulate(&configs, &stepper.advance(FIXED_STEP));
            }
            state.set_anchor(far);
            let shown = state.simulate(&configs, &stepper.advance(5.0 * FIXED_STEP));
            assert_eq!(shown, Shown::Skipped);
            (state, stepper)
        };
        let off_rest = |state: &ChainState| {
            state
                .joints
                .iter()
                .map(|joint| (joint.rotation * Vec3::Y).angle_between(Vec3::Y))
                .fold(0.0, f32::max)
        };

        // snapped again where it now stands, it steps from rest
        let (_, mut stepper) = swinging();
        let mut state = snapped(&configs, far);
        for _ in 0..3 {
            state.set_anchor(far);
            let shown = state.simulate(&configs, &stepper.advance(FIXED_STEP));
            assert!(matches!(shown, Shown::Blend(_)));
            assert!(off_rest(&state) < 1e-4);
        }

        // left as it was, it steps from the old tails and swings towards where it stood
        let (mut state, mut stepper) = swinging();
        state.set_anchor(far);
        let _ = state.simulate(&configs, &stepper.advance(FIXED_STEP));
        assert!(off_rest(&state) > 1.0, "{}", off_rest(&state));
    }

    #[test]
    fn a_grown_anchor_lengthens_the_bones() {
        let configs = chain(1, Vec3::ZERO);
        let mut anchor = anchor_at(Vec3::ZERO);
        anchor.scale = Vec3::splat(2.0);
        anchor.scale_factor = scale_factor(anchor.scale, Vec3::ONE);
        let mut states = [JointState::snapped(Vec3::Y * BONE * 2.0, Quat::IDENTITY)];

        step_chain(&configs, &mut states, &anchor, FIXED_STEP);

        assert_close(states[0].tail, Vec3::Y * (BONE + TIP_LENGTH) * 2.0, 1e-5);
    }
}

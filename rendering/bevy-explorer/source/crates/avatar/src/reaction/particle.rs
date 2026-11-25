//! A reaction in flight: one emoji rising over the player it plays for, apart from how it is
//! drawn. It starts small at the player's anchor, rises and slows, keeps over the player as
//! they move, sways, swells once near its end and is gone.

use std::{f32::consts::TAU, ops::RangeInclusive};

use bevy::math::Vec3;
use rand::Rng;

/// Seconds it lasts.
const LIFETIME: RangeInclusive<f32> = 0.75..=1.0;
/// Its speed upward at the start, in metres a second.
const SPEED: RangeInclusive<f32> = 1.0..=1.5;
/// Its speed sideways at the start, either way along each axis, in metres a second.
const DRIFT: f32 = 0.1;
/// How far from the anchor it starts, either way: across, along each axis, and up, in metres.
const START_ACROSS: f32 = 0.05;
const START_UP: f32 = 0.02;
/// Its width once grown, in metres, and the share of that it starts at.
const SIZE: RangeInclusive<f32> = 0.15..=0.2;
const START_SIZE: RangeInclusive<f32> = 0.2..=0.5;
/// What slows it: its velocity falls by `e` to the power of this every second.
const DRAG: f32 = 0.4;
/// How fast it closes on the anchor across the ground at the start of its life, per second,
/// and how much of that is gone by its end.
const FOLLOW_RATE: f32 = 5.0;
const FOLLOW_LOSS: f32 = 0.65139425;
/// The sway: metres either side, and times a second.
const SWAY: f32 = 0.1;
const SWAY_FREQUENCY: f32 = 0.4;
/// The share of its life it has faded in by, and the share it starts to fade out at.
const FADE_IN: f32 = 0.1;
const FADE_OUT: f32 = 0.9;

/// A point of the curve that scales a particle's size over its life: the share of its life,
/// the scale there, and the curve's slope arriving and leaving.
struct SizeKey {
    at: f32,
    scale: f32,
    slope_in: f32,
    slope_out: f32,
}

const fn key(at: f32, scale: f32, slope_in: f32, slope_out: f32) -> SizeKey {
    SizeKey {
        at,
        scale,
        slope_in,
        slope_out,
    }
}

/// It pops in, holds, swells once and shrinks to nothing.
const SIZE_OVER_LIFE: [SizeKey; 6] = [
    key(0.0, 0.2, 8.250152, 8.250152),
    key(0.09696792, 1.0, 0.0, 0.0),
    key(0.7680084, 1.0, 0.0, 0.0),
    key(0.84144276, 1.7107952, 0.7490359, 0.7490359),
    key(0.896026, 1.000315, -11.318636, -11.318636),
    key(1.0, 0.0, 0.0, 0.0),
];

/// The size's scale at `t` of a particle's life: a cubic through the keys either side of `t`
/// with their slopes.
fn size_over_life(t: f32) -> f32 {
    let after = SIZE_OVER_LIFE
        .iter()
        .position(|key| key.at > t)
        .unwrap_or(SIZE_OVER_LIFE.len() - 1)
        .max(1);
    let (from, to) = (&SIZE_OVER_LIFE[after - 1], &SIZE_OVER_LIFE[after]);
    let span = to.at - from.at;
    let s = ((t - from.at) / span).clamp(0.0, 1.0);
    let (s2, s3) = (s * s, s * s * s);
    (2.0 * s3 - 3.0 * s2 + 1.0) * from.scale
        + (s3 - 2.0 * s2 + s) * from.slope_out * span
        + (3.0 * s2 - 2.0 * s3) * to.scale
        + (s3 - s2) * to.slope_in * span
}

fn smoothstep(from: f32, to: f32, x: f32) -> f32 {
    let t = ((x - from) / (to - from)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Where and how a particle is drawn.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Pose {
    /// Its centre, in the world.
    pub position: Vec3,
    /// Its width, in metres.
    pub size: f32,
    /// 0 to 1.
    pub opacity: f32,
}

#[derive(Debug, Clone)]
pub struct Particle {
    /// Where it is, in the world, before its sway. `None` until its player's anchor places it.
    position: Option<Vec3>,
    /// From the anchor to where it starts.
    start: Vec3,
    velocity: Vec3,
    age: f32,
    lifetime: f32,
    size: f32,
    start_size: f32,
    /// The direction it sways along, as an angle on the ground.
    sway_heading: f32,
}

impl Particle {
    pub fn new(rng: &mut impl Rng) -> Self {
        let mut either_way = |reach: f32| rng.gen_range(-reach..=reach);
        let start = Vec3::new(
            either_way(START_ACROSS),
            either_way(START_UP),
            either_way(START_ACROSS),
        );
        let drift = (either_way(DRIFT), either_way(DRIFT));
        let size = rng.gen_range(SIZE);
        Self {
            position: None,
            start,
            velocity: Vec3::new(drift.0, rng.gen_range(SPEED), drift.1),
            age: 0.0,
            lifetime: rng.gen_range(LIFETIME),
            size,
            start_size: size * rng.gen_range(START_SIZE),
            sway_heading: rng.gen_range(0.0..TAU),
        }
    }

    /// Advance by `dt` seconds. `anchor` is where its player's reactions rise from now; with
    /// none it keeps its course, or waits to be placed. Returns whether it is still alive.
    pub fn tick(&mut self, dt: f32, anchor: Option<Vec3>) -> bool {
        let Some(position) = self.position.as_mut() else {
            match anchor {
                // placed in the frame it was made: no time has passed for it
                Some(anchor) if self.age == 0.0 => self.position = Some(anchor + self.start),
                Some(anchor) => {
                    self.position = Some(anchor + self.start);
                    self.age += dt;
                }
                None => self.age += dt,
            }
            return self.age < self.lifetime;
        };
        if let Some(anchor) = anchor {
            let rate = FOLLOW_RATE * (1.0 - FOLLOW_LOSS * self.age / self.lifetime);
            let blend = 1.0 - (-rate * dt).exp();
            position.x += (anchor.x - position.x) * blend;
            position.z += (anchor.z - position.z) * blend;
        }
        self.age += dt;
        if self.age >= self.lifetime {
            return false;
        }
        self.velocity *= (-DRAG * dt).exp();
        *position += self.velocity * dt;
        true
    }

    /// `None` while it has no place.
    pub fn pose(&self) -> Option<Pose> {
        let t = (self.age / self.lifetime).clamp(0.0, 1.0);
        let sway = SWAY * (self.age * SWAY_FREQUENCY * TAU).sin();
        let grown = 1.0 - (1.0 - t).powi(3);
        Some(Pose {
            position: self.position?
                + Vec3::new(self.sway_heading.cos(), 0.0, self.sway_heading.sin()) * sway,
            size: size_over_life(t) * (self.start_size + (self.size - self.start_size) * grown),
            opacity: smoothstep(0.0, FADE_IN, t) * (1.0 - smoothstep(FADE_OUT, 1.0, t)),
        })
    }
}

#[cfg(test)]
mod tests {
    use rand::{rngs::StdRng, SeedableRng};

    use super::*;

    const ANCHOR: Vec3 = Vec3::new(10.0, 2.0, -4.0);

    fn particle(seed: u64) -> Particle {
        Particle::new(&mut StdRng::seed_from_u64(seed))
    }

    /// A particle placed at the anchor, its first frame behind it.
    fn placed(seed: u64) -> Particle {
        let mut particle = particle(seed);
        assert!(particle.tick(0.016, Some(ANCHOR)));
        particle
    }

    #[test]
    fn a_particle_is_made_within_its_ranges() {
        for seed in 0..500 {
            let particle = particle(seed);
            assert!(LIFETIME.contains(&particle.lifetime));
            assert!(SPEED.contains(&particle.velocity.y));
            assert!(particle.velocity.x.abs() <= DRIFT && particle.velocity.z.abs() <= DRIFT);
            assert!(
                particle.start.x.abs() <= START_ACROSS && particle.start.z.abs() <= START_ACROSS
            );
            assert!(particle.start.y.abs() <= START_UP);
            assert!(SIZE.contains(&particle.size));
            let share = particle.start_size / particle.size;
            assert!((0.2 - 1e-5..=0.5 + 1e-5).contains(&share), "{share}");
            assert!((0.0..TAU).contains(&particle.sway_heading));
        }
    }

    #[test]
    fn it_has_no_place_until_an_anchor_gives_it_one() {
        let mut particle = particle(1);
        assert_eq!(particle.pose(), None);
        // placed in the frame it was made, where it starts, with no time gone
        assert!(particle.tick(0.016, Some(ANCHOR)));
        let pose = particle.pose().unwrap();
        assert_eq!(pose.position, ANCHOR + particle.start);
        assert_eq!(pose.opacity, 0.0);
        assert_eq!(particle.age, 0.0);
    }

    #[test]
    fn it_ages_while_it_waits_for_a_place_and_takes_one_late() {
        let mut waiting = particle(2);
        assert!(waiting.tick(0.3, None));
        assert_eq!(waiting.pose(), None);
        assert!(waiting.tick(0.1, Some(ANCHOR)));
        assert!(waiting.pose().is_some());
        assert!((waiting.age - 0.4).abs() < 1e-6);

        // one that never gets a place lasts no longer for it
        let mut never = particle(2);
        let mut frames = 0;
        while never.tick(0.05, None) {
            frames += 1;
            assert!(frames < 100);
        }
        assert_eq!(never.pose(), None);
        assert!(never.age >= never.lifetime);
    }

    #[test]
    fn it_rises_and_slows_and_is_gone_when_its_life_is() {
        let mut particle = placed(3);
        let lifetime = particle.lifetime;
        let (mut last_height, mut last_step) = (particle.position.unwrap().y, f32::MAX);
        let mut age = 0.0;
        while particle.tick(0.02, Some(ANCHOR)) {
            age += 0.02;
            let height = particle.position.unwrap().y;
            let step = height - last_height;
            assert!(step > 0.0 && step < last_step, "{step} after {last_step}");
            (last_height, last_step) = (height, step);
        }
        assert!((age + 0.02 - lifetime).abs() <= 0.02, "{age} of {lifetime}");
        // slowed by its drag alone: what a second takes off a speed
        let risen = last_height - (ANCHOR.y + particle.start.y);
        assert!(risen < SPEED.end() * lifetime && risen > SPEED.start() * lifetime * 0.75);
    }

    #[test]
    fn it_keeps_over_a_player_who_moves_and_not_up_or_down_with_them() {
        let moved = ANCHOR + Vec3::new(3.0, 50.0, -2.0);
        let mut followed = placed(4);
        let mut left = followed.clone();
        for _ in 0..20 {
            followed.tick(0.02, Some(moved));
            left.tick(0.02, None);
        }
        let (followed, left) = (followed.position.unwrap(), left.position.unwrap());
        // it has closed most of the way across the ground, and not passed the player
        let gap = |position: Vec3| (position - moved).with_y(0.0).length();
        assert!(
            gap(followed) < gap(left) * 0.35,
            "{} of {}",
            gap(followed),
            gap(left)
        );
        assert!(followed.x < moved.x && followed.z > moved.z);
        assert_eq!(followed.y, left.y);
    }

    #[test]
    fn it_follows_the_same_however_long_the_frames_are() {
        let moved = ANCHOR + Vec3::new(10.0, 0.0, 0.0);
        let after = |frames: u32| {
            let mut particle = placed(5);
            for _ in 0..frames {
                particle.tick(0.4 / frames as f32, Some(moved));
            }
            particle.position.unwrap()
        };
        // a blend in step with the frame rate would put these well over half a metre apart
        assert!(after(4).distance(after(40)) < 0.25);
    }

    #[test]
    fn a_player_who_jumps_away_is_caught_up_with_not_jumped_to() {
        let far = ANCHOR + Vec3::new(100.0, 0.0, 0.0);
        let mut particle = placed(6);
        particle.tick(0.016, Some(far));
        let position = particle.position.unwrap();
        assert!(position.x > ANCHOR.x + 1.0 && position.x < far.x - 50.0);
        assert!(position.is_finite());
    }

    #[test]
    fn the_size_curve_passes_through_its_keys() {
        for key in &SIZE_OVER_LIFE {
            assert!(
                (size_over_life(key.at) - key.scale).abs() < 1e-5,
                "{} at {}",
                size_over_life(key.at),
                key.at
            );
        }
        // held between the two keys that hold it, and swollen only near the end
        assert!((size_over_life(0.4) - 1.0).abs() < 1e-6);
        let largest = (0..=1000)
            .map(|step| size_over_life(step as f32 / 1000.0))
            .fold(0.0, f32::max);
        assert!((1.7..1.75).contains(&largest), "{largest}");
        // out of range it holds its ends
        assert_eq!(size_over_life(-1.0), 0.2);
        assert_eq!(size_over_life(2.0), 0.0);
    }

    #[test]
    fn it_grows_from_its_start_size_fades_in_and_fades_out() {
        let mut particle = placed(7);
        let lifetime = particle.lifetime;
        let at = |particle: &mut Particle, t: f32| {
            particle.age = t * lifetime;
            particle.pose().unwrap()
        };
        // the start size, at the curve's first key
        assert!((at(&mut particle, 0.0).size - particle.start_size * 0.2).abs() < 1e-6);
        assert_eq!(at(&mut particle, 0.0).opacity, 0.0);
        assert!((at(&mut particle, 0.05).opacity - 0.5).abs() < 1e-5);
        for t in [0.1, 0.5, 0.9] {
            assert!((at(&mut particle, t).opacity - 1.0).abs() < 1e-5, "{t}");
        }
        assert!((at(&mut particle, 0.95).opacity - 0.5).abs() < 1e-5);
        assert_eq!(at(&mut particle, 1.0).opacity, 0.0);
        // nearly its full size by the middle of its life
        let middle = at(&mut particle, 0.5).size;
        assert!(middle > particle.size * 0.89 && middle <= particle.size);
        assert_eq!(at(&mut particle, 1.0).size, 0.0);
    }

    #[test]
    fn the_sway_moves_where_it_is_drawn_and_not_where_it_is() {
        let mut particle = placed(8);
        // a quarter of a sway in
        particle.age = 0.25 / SWAY_FREQUENCY;
        particle.lifetime = 10.0;
        let offset = particle.pose().unwrap().position - particle.position.unwrap();
        assert!((offset.length() - SWAY).abs() < 1e-5);
        assert_eq!(offset.y, 0.0);
    }
}

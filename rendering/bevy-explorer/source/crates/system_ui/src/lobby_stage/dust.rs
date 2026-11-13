//! The floating dust motes: a small particle simulation in the stage's frame, drawn as one mesh
//! of camera-facing quads rebuilt each frame.

use bevy::{
    prelude::*,
    render::{
        mesh::{Indices, PrimitiveTopology},
        render_asset::RenderAssetUsages,
    },
};

/// The motes' emitter, in the stage's frame.
pub const EMITTER_CENTRE: Vec3 = Vec3::new(0.0, 1.9, -3.0);
pub const EMITTER_SIZE: Vec3 = Vec3::new(26.0, 6.0, 16.0);
const MAX_MOTES: usize = 300;
/// Motes per second, picked anew for each second between these.
const RATE: (f32, f32) = (20.0, 35.0);
/// Seconds.
const LIFETIME: (f32, f32) = (6.0, 10.0);
/// Metres per second, along +z.
const SPEED: (f32, f32) = (0.02, 0.06);
/// Metres across.
const SIZE: (f32, f32) = (0.015, 0.045);
/// Seconds simulated up front, so the motes are already spread when the stage appears.
const PREWARM: f32 = 10.0;
const PREWARM_STEP: f32 = 0.1;
/// The drift: metres per second at most, the noise's cells per metre, and how fast it scrolls.
const NOISE_STRENGTH: f32 = 0.05;
const NOISE_FREQUENCY: f32 = 0.25;
const NOISE_SCROLL: f32 = 0.05;

#[derive(Clone, Copy, Debug)]
struct Mote {
    position: Vec3,
    velocity: Vec3,
    size: f32,
    age: f32,
    lifetime: f32,
}

#[derive(Component)]
pub struct Dust {
    motes: Vec<Mote>,
    seed: u32,
    time: f32,
    pending: f32,
    rate: f32,
    color: LinearRgba,
}

impl Dust {
    pub fn new(color: LinearRgba) -> Self {
        let mut dust = Self {
            motes: Vec::with_capacity(MAX_MOTES),
            seed: 0x9e37_79b9,
            time: 0.0,
            pending: 0.0,
            rate: RATE.0,
            color,
        };
        dust.rate = dust.between(RATE);
        let mut warmed = 0.0;
        while warmed < PREWARM {
            dust.tick(PREWARM_STEP);
            warmed += PREWARM_STEP;
        }
        dust
    }

    fn random(&mut self) -> f32 {
        // xorshift32
        let mut x = self.seed;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.seed = x;
        (x >> 8) as f32 / (1u32 << 24) as f32
    }

    fn between(&mut self, (from, to): (f32, f32)) -> f32 {
        from + (to - from) * self.random()
    }

    #[cfg(test)]
    pub fn len(&self) -> usize {
        self.motes.len()
    }

    pub fn tick(&mut self, dt: f32) {
        let whole_second = self.time.floor();
        self.time += dt;
        if self.time.floor() != whole_second {
            self.rate = self.between(RATE);
        }

        let time = self.time;
        self.motes.retain_mut(|mote| {
            mote.age += dt;
            let drift = noise_drift(mote.position, time);
            mote.position += (mote.velocity + drift) * dt;
            mote.age < mote.lifetime
        });

        self.pending += self.rate * dt;
        while self.pending >= 1.0 {
            self.pending -= 1.0;
            if self.motes.len() >= MAX_MOTES {
                continue;
            }
            let offset = Vec3::new(
                self.random() - 0.5,
                self.random() - 0.5,
                self.random() - 0.5,
            );
            let mote = Mote {
                position: EMITTER_CENTRE + offset * EMITTER_SIZE,
                velocity: Vec3::Z * self.between(SPEED),
                size: self.between(SIZE),
                age: 0.0,
                lifetime: self.between(LIFETIME),
            };
            self.motes.push(mote);
        }
    }

    /// The motes as camera-facing quads; `right` and `up` are the camera's axes in the stage's
    /// frame.
    pub fn build_mesh(&self, mesh: &mut Mesh, right: Vec3, up: Vec3) {
        let mut positions = Vec::with_capacity(self.motes.len() * 4);
        let mut colors = Vec::with_capacity(self.motes.len() * 4);
        let mut uvs = Vec::with_capacity(self.motes.len() * 4);
        let mut indices = Vec::with_capacity(self.motes.len() * 6);
        for mote in &self.motes {
            let half = mote.size * 0.5;
            let r = right * half;
            let u = up * half;
            let base = positions.len() as u32;
            positions.extend([
                (mote.position - r - u).to_array(),
                (mote.position + r - u).to_array(),
                (mote.position + r + u).to_array(),
                (mote.position - r + u).to_array(),
            ]);
            uvs.extend([[0.0, 1.0], [1.0, 1.0], [1.0, 0.0], [0.0, 0.0]]);
            let alpha = self.color.alpha * fade(mote.age / mote.lifetime);
            let color = [self.color.red, self.color.green, self.color.blue, alpha];
            colors.extend([color; 4]);
            indices.extend([base, base + 1, base + 2, base, base + 2, base + 3]);
        }
        let normals = vec![[0.0, 0.0, 1.0]; positions.len()];
        mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
        mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
        mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
        mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, colors);
        mesh.insert_indices(Indices::U32(indices));
    }
}

/// An empty mesh for `Dust::build_mesh` to fill, kept on the CPU to be rebuilt every frame.
pub fn empty_mesh() -> Mesh {
    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::RENDER_WORLD | RenderAssetUsages::MAIN_WORLD,
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, Vec::<[f32; 3]>::new());
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, Vec::<[f32; 3]>::new());
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, Vec::<[f32; 2]>::new());
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, Vec::<[f32; 4]>::new());
    mesh.insert_indices(Indices::U32(Vec::new()));
    mesh
}

/// Opacity over a mote's life: in from nothing, fully there at the middle, out again.
fn fade(life: f32) -> f32 {
    let life = life.clamp(0.0, 1.0);
    if life < 0.5 {
        life * 2.0
    } else {
        (1.0 - life) * 2.0
    }
}

fn hash(p: IVec3) -> f32 {
    let mut h = (p.x as u32).wrapping_mul(0x8da6_b343)
        ^ (p.y as u32).wrapping_mul(0xd816_3841)
        ^ (p.z as u32).wrapping_mul(0xcb1a_b31f);
    h ^= h >> 13;
    h = h.wrapping_mul(0x5bd1_e995);
    h ^= h >> 15;
    (h >> 8) as f32 / (1u32 << 24) as f32
}

fn value_noise(p: Vec3) -> f32 {
    let i = p.floor();
    let f = p - i;
    let f = f * f * (Vec3::splat(3.0) - 2.0 * f);
    let i = i.as_ivec3();
    let corner = |x, y, z| hash(i + IVec3::new(x, y, z));
    let x00 = corner(0, 0, 0) + (corner(1, 0, 0) - corner(0, 0, 0)) * f.x;
    let x10 = corner(0, 1, 0) + (corner(1, 1, 0) - corner(0, 1, 0)) * f.x;
    let x01 = corner(0, 0, 1) + (corner(1, 0, 1) - corner(0, 0, 1)) * f.x;
    let x11 = corner(0, 1, 1) + (corner(1, 1, 1) - corner(0, 1, 1)) * f.x;
    let y0 = x00 + (x10 - x00) * f.y;
    let y1 = x01 + (x11 - x01) * f.y;
    y0 + (y1 - y0) * f.z
}

/// A slow, smooth wander: each axis its own noise field, scrolling over time.
fn noise_drift(position: Vec3, time: f32) -> Vec3 {
    let p = position * NOISE_FREQUENCY + Vec3::splat(time * NOISE_SCROLL);
    let axis = |offset: f32| value_noise(p + Vec3::splat(offset)) * 2.0 - 1.0;
    Vec3::new(axis(0.0), axis(31.7), axis(73.1)) * NOISE_STRENGTH
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prewarm_fills_the_emitter() {
        let dust = Dust::new(LinearRgba::WHITE);
        // ~27 motes a second living ~8 s: well over a hundred, never past the cap
        assert!(dust.len() > 100, "{}", dust.len());
        assert!(dust.len() <= MAX_MOTES);
    }

    #[test]
    fn motes_expire() {
        let mut dust = Dust::new(LinearRgba::WHITE);
        let before: Vec<f32> = dust.motes.iter().map(|m| m.lifetime - m.age).collect();
        let longest = before.iter().cloned().fold(0.0, f32::max);
        // stop emitting and outlive every mote
        dust.rate = 0.0;
        dust.pending = 0.0;
        let mut time = 0.0;
        while time <= longest + 0.1 {
            // never cross a whole second, so the rate stays 0
            dust.time = 0.5;
            dust.tick(0.1);
            time += 0.1;
        }
        assert_eq!(dust.len(), 0);
    }

    #[test]
    fn fade_peaks_mid_life() {
        assert_eq!(fade(0.0), 0.0);
        assert_eq!(fade(0.5), 1.0);
        assert_eq!(fade(1.0), 0.0);
    }

    #[test]
    fn mesh_has_a_quad_per_mote() {
        let dust = Dust::new(LinearRgba::WHITE);
        let mut mesh = empty_mesh();
        dust.build_mesh(&mut mesh, Vec3::X, Vec3::Y);
        assert_eq!(mesh.count_vertices(), dust.len() * 4);
    }
}

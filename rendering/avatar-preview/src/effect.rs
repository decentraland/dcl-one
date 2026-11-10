//! The jesus mode's particles: glowing specks rising off the avatar's surface, as the Unity
//! renderer's `Character_Effect` visual effect draws them on the download page. It is a preview
//! feature with no Explorer counterpart, so it lives here: the particles start on random vertices
//! of the posed avatar (as the graph places them, so the dense hands, feet and face spark most) (`host::Surfaces::sample`, the CPU skinning the framing uses), move on the
//! CPU, and are drawn as one mesh of camera-facing quads through the shared camera, so its bloom
//! and tonemapping light them as they do the rest of the view.

use bevy::{
    asset::RenderAssetUsages,
    pbr::NotShadowCaster,
    prelude::*,
    render::{
        mesh::{Indices, PrimitiveTopology},
        render_resource::{Extent3d, TextureDimension, TextureFormat},
        view::NoFrustumCulling,
    },
};

use crate::{
    host::{PreviewCamera, Surfaces},
    PreviewAvatar,
};

/// The effect's settings, from the Unity graph's values; the page tunes them with `effect`.
#[derive(Resource, Debug, Clone)]
pub struct EffectSettings {
    pub shown: bool,
    /// particles a second
    pub rate: f32,
    /// seconds each lives, picked between these
    pub life_min: f32,
    pub life_max: f32,
    /// metres a second, each axis picked between these (x across, y up, z towards the camera)
    pub velocity_min: Vec3,
    pub velocity_max: Vec3,
    /// metres across, picked between these
    pub size_min: f32,
    pub size_max: f32,
    /// the colour's hue (linear), and how bright: the graph's HDR (1.36, 42.5, 95.9)
    pub color: Vec3,
    pub intensity: f32,
}

impl Default for EffectSettings {
    fn default() -> Self {
        let unity = Vec3::new(1.356_712_5, 42.544_434, 95.874_53);
        Self {
            shown: false,
            // tuned by eye through this camera from the graph's (1000 a second, 0.2 to 0.8 s,
            // 1 to 30 mm, the colour at 96)
            rate: 650.0,
            life_min: 0.2,
            life_max: 1.0,
            velocity_min: Vec3::new(-0.5, 0.1, -0.2),
            velocity_max: Vec3::new(0.5, 0.4, 0.2),
            size_min: 0.019,
            size_max: 0.04,
            color: unity / unity.max_element(),
            intensity: 100.0,
        }
    }
}

pub struct EffectPlugin;

impl Plugin for EffectPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<EffectSettings>()
            .add_systems(Startup, spawn_effect)
            .add_systems(Update, (simulate, draw).chain());
    }
}

struct Particle {
    position: Vec3,
    velocity: Vec3,
    age: f32,
    life: f32,
    size: f32,
}

#[derive(Component, Default)]
struct Particles {
    live: Vec<Particle>,
    /// the fraction of a particle carried to the next frame
    owed: f32,
    seed: u32,
}

impl Particles {
    /// 0..1, xorshift: the effect only needs noise
    fn random(&mut self) -> f32 {
        let mut x = self.seed;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.seed = x;
        (x >> 8) as f32 / (1u32 << 24) as f32
    }

    fn between(&mut self, a: f32, b: f32) -> f32 {
        a + (b - a) * self.random()
    }
}

fn spawn_effect(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
    settings: Res<EffectSettings>,
) {
    commands.spawn((
        Mesh3d(meshes.add(quads(&[], Vec3::X, Vec3::Y))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: color(&settings),
            base_color_texture: Some(images.add(soft_disc())),
            unlit: true,
            alpha_mode: AlphaMode::Add,
            ..default()
        })),
        Transform::default(),
        Visibility::Hidden,
        NotShadowCaster,
        NoFrustumCulling,
        Particles {
            seed: 0x9e37_79b9,
            ..default()
        },
    ));
}

fn color(settings: &EffectSettings) -> Color {
    let c = settings.color * settings.intensity;
    Color::linear_rgb(c.x, c.y, c.z)
}

/// Ages and moves the particles, and starts new ones on the avatar's surface.
fn simulate(
    time: Res<Time>,
    settings: Res<EffectSettings>,
    avatars: Query<Entity, With<PreviewAvatar>>,
    surfaces: Surfaces,
    mut effects: Query<(&mut Particles, &mut Visibility)>,
) {
    let dt = time.delta_secs();
    for (mut particles, mut visibility) in effects.iter_mut() {
        visibility.set_if_neq(if settings.shown {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        });
        if !settings.shown {
            particles.live.clear();
            particles.owed = 0.0;
            continue;
        }
        particles.live.retain_mut(|p| {
            p.age += dt;
            p.position += p.velocity * dt;
            p.age < p.life
        });
        let Ok(root) = avatars.single() else {
            continue;
        };
        // a frame's share of the rate; a long frame starts no more than a tenth of a second's
        let owed = particles.owed + settings.rate * dt.min(0.1);
        let count = owed.floor() as usize;
        particles.owed = owed - count as f32;
        let mut points = Vec::with_capacity(count);
        let particles = &mut *particles;
        surfaces.sample(root, count, &mut || particles.random(), &mut |p| {
            points.push(p)
        });
        let (lo, hi) = (settings.velocity_min, settings.velocity_max);
        for position in points {
            let velocity = Vec3::new(
                particles.between(lo.x, hi.x),
                particles.between(lo.y, hi.y),
                particles.between(lo.z, hi.z),
            );
            let life = particles
                .between(settings.life_min, settings.life_max)
                .max(0.01);
            let size = particles.between(settings.size_min, settings.size_max);
            particles.live.push(Particle {
                position,
                velocity,
                age: 0.0,
                life,
                size,
            });
        }
    }
}

/// Rebuilds the quads facing the camera, each shrinking and fading over its life.
fn draw(
    settings: Res<EffectSettings>,
    cameras: Query<&GlobalTransform, With<PreviewCamera>>,
    effects: Query<(&Particles, &Mesh3d, &MeshMaterial3d<StandardMaterial>)>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let Some(camera) = cameras.iter().next() else {
        return;
    };
    for (particles, mesh, material) in effects.iter() {
        if settings.is_changed() {
            if let Some(material) = materials.get_mut(&material.0) {
                material.base_color = color(&settings);
            }
        }
        if !settings.shown && particles.live.is_empty() {
            continue;
        }
        if let Some(mesh) = meshes.get_mut(&mesh.0) {
            *mesh = quads(&particles.live, camera.right().into(), camera.up().into());
        }
    }
}

/// One quad per particle, `right` and `up` its axes; the vertex colour carries the fade.
fn quads(particles: &[Particle], right: Vec3, up: Vec3) -> Mesh {
    let mut positions = Vec::with_capacity(particles.len() * 4);
    let mut uvs = Vec::with_capacity(particles.len() * 4);
    let mut colors = Vec::with_capacity(particles.len() * 4);
    let mut indices = Vec::with_capacity(particles.len() * 6);
    for p in particles {
        // the graph's size and colour over life: both fall to nothing by the end
        let left = 1.0 - (p.age / p.life).clamp(0.0, 1.0);
        let half = p.size * left * 0.5;
        let base = positions.len() as u32;
        for (u, v) in [(0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0)] {
            let offset = right * (u * 2.0 - 1.0) * half + up * (1.0 - v * 2.0) * half;
            positions.push((p.position + offset).to_array());
            uvs.push([u, v]);
            colors.push([left, left, left, left]);
        }
        indices.extend([base, base + 2, base + 1, base, base + 3, base + 2]);
    }
    // never an empty mesh: WebGL2 gives one a zero-sized buffer and panics slicing it. With no
    // particles, a single triangle of no area, which draws nothing
    if positions.is_empty() {
        positions.extend([[0.0; 3]; 3]);
        uvs.extend([[0.0; 2]; 3]);
        colors.extend([[0.0; 4]; 3]);
        indices.extend([0, 1, 2]);
    }
    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
    .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uvs)
    .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, colors)
    .with_inserted_indices(Indices::U32(indices))
}

/// A round speck, opaque in the middle and fading to its edge, as Unity's default particle.
fn soft_disc() -> Image {
    const SIZE: u32 = 32;
    let mut data = Vec::with_capacity((SIZE * SIZE * 4) as usize);
    for y in 0..SIZE {
        for x in 0..SIZE {
            let d = Vec2::new(x as f32 + 0.5, y as f32 + 0.5) / SIZE as f32 * 2.0 - Vec2::ONE;
            let a = (1.0 - d.length()).clamp(0.0, 1.0);
            let a = (a * a * 255.0) as u8;
            data.extend([a, a, a, a]);
        }
    }
    Image::new(
        Extent3d {
            width: SIZE,
            height: SIZE,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        data,
        TextureFormat::Rgba8Unorm,
        RenderAssetUsages::default(),
    )
}

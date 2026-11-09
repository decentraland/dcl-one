//! The dithered swap between a scene's stand-in and what replaces it: the live scene over its
//! near LOD, the near LOD over the far one. Both roots fade at once with complementary dither
//! patterns, so every pixel shows exactly one of them at every step and the swap never shows
//! what lies behind both. The fade is carried in each mesh's `MeshTag` (`scene_material`'s fade
//! bits), so it costs no material and no extra draw.

use bevy::{prelude::*, render::mesh::MeshTag};
use scene_material::{fade_mesh_tag, unfaded_mesh_tag};

/// How long a stand-in and what replaces it take to swap.
pub const FADE_DURATION: f32 = 0.4;

/// A root whose meshes dither in, or out for a stand-in on its way out, over
/// [`FADE_DURATION`] from the first frame the fade runs.
#[derive(Component)]
pub struct SceneFade {
    started: Option<f32>,
    out: bool,
}

impl SceneFade {
    pub fn fade_in() -> Self {
        Self {
            started: None,
            out: false,
        }
    }

    pub fn fade_out() -> Self {
        Self {
            started: None,
            out: true,
        }
    }
}

/// A fade-out has run its course: the root shows nothing and can go.
#[derive(Component)]
pub struct FadedOut;

/// Steps every fading root's meshes, those spawned since the fade began included, and ends the
/// fade: a fade-in leaves the tags clean, a fade-out hides the root and marks it [`FadedOut`].
pub fn animate_scene_fades(
    mut commands: Commands,
    time: Res<Time>,
    mut fades: Query<(Entity, &mut SceneFade)>,
    children: Query<&Children>,
    mut meshes: Query<(&mut Mesh3d, &mut MeshTag)>,
) {
    let now = time.elapsed_secs();
    for (root, mut fade) in fades.iter_mut() {
        let started = *fade.started.get_or_insert(now);
        let progress = ((now - started) / FADE_DURATION).clamp(0.0, 1.0);
        let done = progress >= 1.0;
        let coverage = if fade.out { 1.0 - progress } else { progress };

        for entity in children.iter_descendants(root) {
            let Ok((mut mesh, mut tag)) = meshes.get_mut(entity) else {
                continue;
            };
            let next = if done {
                unfaded_mesh_tag(tag.0)
            } else {
                fade_mesh_tag(tag.0, coverage, fade.out)
            };
            if tag.0 != next {
                tag.0 = next;
                // bevy's mesh extraction doesn't watch MeshTag (0.16); Mesh3d is on its list
                mesh.set_changed();
            }
        }

        if done {
            let mut root = commands.entity(root);
            root.remove::<SceneFade>();
            if fade.out {
                root.insert((FadedOut, Visibility::Hidden));
            }
        }
    }
}

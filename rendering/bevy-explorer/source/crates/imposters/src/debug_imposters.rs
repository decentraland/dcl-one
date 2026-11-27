use bevy::{
    asset::weak_handle,
    prelude::*,
    render::{mesh::MeshTag, render_resource::ShaderImport},
};
use boimp::{
    render::{FRAGMENT_HANDLE, VERTEX_HANDLE},
    Imposter,
};

use crate::render::{DebugImpostersEnabled, SceneImposter};

const BOIMP_VERTEX: Handle<Shader> = weak_handle!("ff86185c-bf78-46e3-b706-c21f8b22a581");
const BOIMP_FRAGMENT: Handle<Shader> = weak_handle!("20036dff-2ed9-4b19-b763-e96500175228");
const VERTEX_IMPORT: &str = "dcl_imposters/boimp_vertex.wgsl";
const FRAGMENT_IMPORT: &str = "dcl_imposters/boimp_fragment.wgsl";

pub(super) fn install_debug_shaders(mut shaders: ResMut<Assets<Shader>>) {
    for (handle, original_handle, import_path, wrapper, path) in [
        (
            VERTEX_HANDLE,
            BOIMP_VERTEX,
            VERTEX_IMPORT,
            include_str!("shaders/debug_vertex.wgsl"),
            "dcl_imposters/debug_vertex.wgsl",
        ),
        (
            FRAGMENT_HANDLE,
            BOIMP_FRAGMENT,
            FRAGMENT_IMPORT,
            include_str!("shaders/debug_fragment.wgsl"),
            "dcl_imposters/debug_fragment.wgsl",
        ),
    ] {
        // Keep boimp's existing shader as an import. Naga Oil exposes the original entry
        // point as a function so the wrapper can reuse sampling, lighting and depth tests.
        let original = shaders
            .get_mut(handle.id())
            .expect("boimp shaders must be registered before the imposter debug panel");
        let mut original = std::mem::replace(original, Shader::from_wgsl(wrapper, path));
        original.import_path = ShaderImport::AssetPath(import_path.to_owned());
        shaders.insert(original_handle.id(), original);
    }
}

#[derive(Component)]
pub(super) struct TileDebugColor(u32);

pub(super) fn tag_new_tiles(
    mut commands: Commands,
    debug: Res<DebugImpostersEnabled>,
    tiles: Query<&SceneImposter>,
    meshes: Query<(Entity, &ChildOf), Added<MeshMaterial3d<Imposter>>>,
) {
    for (entity, parent) in &meshes {
        let Ok(tile) = tiles.get(parent.parent()) else {
            continue;
        };
        if tile.as_ingredient {
            continue;
        }
        let color = tile_debug_color(tile);
        // Use the tile that owns this mesh at spawn. During a pending LOD switch its
        // child may temporarily be reparented to a different tile.
        commands.entity(entity).insert((
            TileDebugColor(color),
            MeshTag(if debug.0 { color } else { 0 }),
        ));
    }
}

pub(super) fn update_tile_colors(
    debug: Res<DebugImpostersEnabled>,
    mut meshes: Query<(&TileDebugColor, &mut MeshTag, &mut Mesh3d)>,
) {
    for (color, mut tag, mut mesh) in &mut meshes {
        let next = if debug.0 { color.0 } else { 0 };
        if tag.0 != next {
            tag.0 = next;
            // The GPU mesh extractor reads MeshTag when Mesh3d changes.
            mesh.set_changed();
        }
    }
}

fn tile_debug_color(tile: &SceneImposter) -> u32 {
    // Coordinates are the lower corner in parcels. A one-tile step in either axis
    // produces a substantial hue change, including when adjacent tiles borrow one bake.
    let size = 1_i64 << tile.level;
    let x = i64::from(tile.parcel.x).div_euclid(size);
    let y = i64::from(tile.parcel.y).div_euclid(size);
    let hue = (5 * x + 9 * y + 11 * tile.level as i64).rem_euclid(23) as f32 * (360.0 / 23.0);
    let [r, g, b, _] = Color::hsl(hue, 0.9, 0.6).to_linear().to_u8_array();
    u32::from_le_bytes([r, g, b, 0x80])
}

#[cfg(test)]
mod tests {
    use std::{
        path::PathBuf,
        sync::mpsc,
        time::{Duration, Instant},
    };

    use bevy::{
        asset::RenderAssetUsages,
        core_pipeline::{
            dof::{DepthOfField, DepthOfFieldMode},
            prepass::DepthPrepass,
        },
        pbr::NotShadowCaster,
        render::{
            camera::{RenderTarget, ScalingMode},
            gpu_readback::{Readback, ReadbackComplete},
            render_resource::{Extent3d, TextureDimension, TextureFormat, TextureUsages},
        },
    };
    use boimp::{GridMode, ImposterData, ImposterRenderPlugin};
    use pipeline_test::{compile_pipelines, headless_app, render_target};

    use super::*;

    /// Exercise the real forward shader and mesh extraction with two requests sharing one bake.
    #[test]
    #[ignore = "needs a GPU; run with --ignored"]
    fn debug_imposter_colors_render() {
        let mut app = headless_app();
        app.add_plugins(ImposterRenderPlugin)
            .init_resource::<DebugImpostersEnabled>()
            .add_systems(Startup, install_debug_shaders)
            .add_systems(
                PostUpdate,
                (
                    tag_new_tiles,
                    update_tile_colors.run_if(resource_changed::<DebugImpostersEnabled>),
                )
                    .chain(),
            );
        app.finish();
        app.cleanup();

        let target = render_target(&mut app);
        let world = app.world_mut();
        let mut images = world.resource_mut::<Assets<Image>>();
        images.get_mut(&target).unwrap().texture_descriptor.usage |= TextureUsages::COPY_SRC;
        let pixels = images.add(Image::new_fill(
            Extent3d {
                width: 256,
                height: 256,
                depth_or_array_layers: 1,
            },
            TextureDimension::D2,
            &[255, 255, 15, 16, 128, 128, 128, 128], // opaque white unlit texel
            TextureFormat::Rg32Uint,
            RenderAssetUsages::RENDER_WORLD,
        ));
        let mesh = world.resource_mut::<Assets<Mesh>>().add(Cuboid::default());
        let material = world.resource_mut::<Assets<Imposter>>().add(Imposter {
            data: ImposterData::new(
                Vec3::ZERO,
                1.0,
                4,
                64,
                UVec2::ZERO,
                UVec2::splat(64),
                GridMode::Hemispherical,
                false,
                false,
                0.5,
                0.99,
            ),
            pixels: pixels.clone(),
            indices: pixels.clone(),
            indices_hi: pixels.clone(),
            depth_palette: pixels,
            alpha_mode: AlphaMode::Blend,
            vram_bytes: 0,
        });
        for (parcel_x, world_x) in [(-36, -1.0), (-35, 1.0)] {
            let parent = world
                .spawn((
                    SceneImposter {
                        parcel: IVec2::new(parcel_x, 2),
                        level: 0,
                        as_ingredient: false,
                    },
                    Transform::default(),
                    Visibility::default(),
                ))
                .id();
            world.spawn((
                ChildOf(parent),
                Mesh3d(mesh.clone()),
                MeshMaterial3d(material.clone()),
                NotShadowCaster,
                Transform::from_xyz(world_x, 0.0, 0.0),
            ));
        }
        world.spawn((
            Camera3d::default(),
            Camera {
                target: RenderTarget::Image(target.clone().into()),
                hdr: true,
                clear_color: ClearColorConfig::Custom(Color::NONE),
                ..default()
            },
            DepthPrepass,
            DepthOfField {
                mode: DepthOfFieldMode::Disabled,
                ..default()
            },
            Msaa::Off,
            Projection::Orthographic(OrthographicProjection {
                scaling_mode: ScalingMode::Fixed {
                    width: 4.0,
                    height: 4.0,
                },
                ..OrthographicProjection::default_3d()
            }),
            Transform::from_xyz(0.0, 0.0, 4.0),
        ));

        compile_pipelines(&mut app);
        let original = read_colors(&mut app, &target, "disabled");
        assert_eq!(original[0], original[1], "both proxies sample one material");
        assert_ne!(original[0], original[2], "an imposter drew on the target");
        assert!(
            (1..255).contains(&original[0][3]),
            "the proxy is translucent"
        );

        app.world_mut().resource_mut::<DebugImpostersEnabled>().0 = true;
        let debug = read_colors(&mut app, &target, "enabled");
        assert_ne!(
            debug[0], debug[1],
            "neighboring requests have distinct tints"
        );
        assert_eq!(debug[2], original[2], "the background retains its color");
        assert_eq!(debug[0][3], original[0][3], "tint does not change alpha");
        assert_eq!(debug[1][3], original[1][3], "tint does not change alpha");

        app.world_mut().resource_mut::<DebugImpostersEnabled>().0 = false;
        assert_eq!(read_colors(&mut app, &target, "restored"), original);
    }

    fn read_colors(app: &mut App, target: &Handle<Image>, frame: &str) -> [[u8; 4]; 3] {
        // Let the render schedule apply any ECS tag changes before requesting the next read.
        app.update();
        app.update();
        let (tx, rx) = mpsc::sync_channel(1);
        let readback = app
            .world_mut()
            .spawn(Readback::texture(target.clone()))
            .observe(move |event: Trigger<ReadbackComplete>| {
                let _ = tx.try_send(event.event().0.clone());
            })
            .id();
        let start = Instant::now();
        let pixels = loop {
            app.update();
            if let Ok(pixels) = rx.try_recv() {
                break pixels;
            }
            assert!(
                start.elapsed() < Duration::from_secs(5),
                "timed out reading the GPU render target"
            );
        };
        app.world_mut().despawn(readback);
        if let Some(dir) = std::env::var_os("DCL_IMPOSTER_TEST_CAPTURE_DIR") {
            let mut ppm = b"P6\n64 64\n255\n".to_vec();
            ppm.extend(
                pixels
                    .chunks_exact(4)
                    .flat_map(|bgra| [bgra[2], bgra[1], bgra[0]]),
            );
            std::fs::write(
                PathBuf::from(dir).join(format!("imposter-debug-{frame}.ppm")),
                ppm,
            )
            .unwrap();
        }
        [16, 48, 32].map(|x| {
            let offset = (32 * 64 + x) * 4;
            pixels[offset..offset + 4].try_into().unwrap()
        })
    }
}

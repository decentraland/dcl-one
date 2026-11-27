use bevy::{
    asset::weak_handle,
    prelude::*,
    render::{
        mesh::MeshTag,
        render_resource::{ShaderImport, Source},
    },
};
use boimp::{
    render::{FRAGMENT_HANDLE, VERTEX_HANDLE},
    Imposter,
};

use crate::render::{DebugImpostersEnabled, SceneImposter};

const BOIMP_VERTEX: Handle<Shader> = weak_handle!("ff86185c-bf78-46e3-b706-c21f8b22a581");
const BOIMP_FRAGMENT: Handle<Shader> = weak_handle!("20036dff-2ed9-4b19-b763-e96500175228");
const VERTEX_IMPORT: &str = "dcl_imposters::boimp_vertex";
const FRAGMENT_IMPORT: &str = "dcl_imposters::boimp_fragment";

pub(super) struct ImposterDebugTintPlugin;

impl Plugin for ImposterDebugTintPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<DebugImpostersEnabled>().add_systems(
            PostUpdate,
            (
                update_debug_shaders.run_if(resource_changed::<DebugImpostersEnabled>),
                tag_new_tiles,
                update_tile_colors.run_if(resource_changed::<DebugImpostersEnabled>),
            )
                .chain(),
        );
    }
}

fn update_debug_shaders(
    debug: Res<DebugImpostersEnabled>,
    mut shaders: ResMut<Assets<Shader>>,
    mut originals: Local<Option<[Shader; 2]>>,
) {
    let handles = [
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
    ];
    if !debug.0 {
        if let Some(originals) = originals.take() {
            for ((handle, original_handle, ..), original) in handles.into_iter().zip(originals) {
                shaders.insert(handle.id(), original);
                shaders.remove(original_handle.id());
            }
        }
        return;
    }
    if originals.is_some() {
        return;
    }

    let [Some((vertex, vertex_import)), Some((fragment, fragment_import))] =
        handles.each_ref().map(|(handle, _, import_path, ..)| {
            let original = shaders.get(handle.id())?;
            let Source::Wgsl(source) = &original.source else {
                return None;
            };
            // The pipeline cache retains resolved asset-path imports when a shader
            // is replaced. A named module lets the original shaders be restored.
            let mut import = original.clone();
            import.import_path = ShaderImport::Custom((*import_path).to_owned());
            import.source =
                Source::Wgsl(format!("#define_import_path {import_path}\n{source}").into());
            Some((original.clone(), import))
        })
    else {
        warn!("imposter debug tint needs boimp's WGSL render shaders");
        return;
    };
    for ((handle, original_handle, _, wrapper, path), import) in
        handles.into_iter().zip([vertex_import, fragment_import])
    {
        // Naga Oil exposes the original entry point as a function so the debug
        // shader reuses its sampling, lighting and depth tests.
        shaders.insert(original_handle.id(), import);
        shaders.insert(handle.id(), Shader::from_wgsl(wrapper, path));
    }
    *originals = Some([vertex, fragment]);
}

#[derive(Component)]
pub(super) struct TileDebugColor(u32);

fn tag_new_tiles(
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

fn update_tile_colors(
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
    u32::from_le_bytes([r, g, b, 0])
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
            prepass::{DepthPrepass, NormalPrepass},
        },
        pbr::NotShadowCaster,
        render::{
            camera::{RenderTarget, ScalingMode},
            gpu_readback::{Readback, ReadbackComplete},
            mesh::MeshBuilder,
            render_resource::{Extent3d, TextureDimension, TextureFormat, TextureUsages},
        },
    };
    use boimp::{GridMode, ImposterData, ImposterRenderPlugin};
    use pipeline_test::{compile_pipelines, headless_app, render_target};

    use super::*;
    use crate::{imposter_mesh::ImposterMesh, imposter_spec::ImposterSpec};

    /// Exercise the real forward shader and mesh extraction with two requests sharing one bake.
    #[test]
    #[ignore = "needs a GPU; run with --ignored"]
    fn debug_imposter_colors_render() {
        let mut app = headless_app();
        app.add_plugins((ImposterRenderPlugin, ImposterDebugTintPlugin));
        app.finish();
        app.cleanup();

        let original_shader = app
            .world()
            .resource::<Assets<Shader>>()
            .get(FRAGMENT_HANDLE.id())
            .unwrap()
            .path
            .clone();

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
        let spec = ImposterSpec {
            scale: 64.0,
            region_min: Vec3::new(-64.0, 0.0, -64.0),
            region_max: Vec3::new(0.0, 12.0, 0.0),
            overhang: 0.0,
        };
        let blend = Imposter {
            data: ImposterData::new(
                Vec3::ZERO,
                spec.scale,
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
        };
        let mut materials = world.resource_mut::<Assets<Imposter>>();
        let masked = materials.add(Imposter {
            alpha_mode: AlphaMode::Mask(0.4),
            ..blend.clone()
        });
        let material = materials.add(blend);
        for parcel_x in [-3, -2] {
            let tile = SceneImposter {
                parcel: IVec2::new(parcel_x, 1),
                level: 0,
                as_ingredient: false,
            };
            let (transform, proxy) = ImposterMesh::render_proxy(&spec, 2, &tile).unwrap();
            let mesh = world.resource_mut::<Assets<Mesh>>().add(proxy.build());
            let parent = world
                .spawn((tile, Transform::default(), Visibility::default()))
                .id();
            world.spawn((
                ChildOf(parent),
                Mesh3d(mesh),
                MeshMaterial3d(material.clone()),
                NotShadowCaster,
                transform,
            ));
        }
        // An untagged, masked imposter exercises the normal prepass without
        // affecting the two samples from the transparent fallback proxies.
        let mesh = world.resource_mut::<Assets<Mesh>>().add(Cuboid::default());
        world.spawn((
            Mesh3d(mesh),
            MeshMaterial3d(masked),
            NotShadowCaster,
            Transform::from_xyz(-32.0, 18.0, -20.0).with_scale(Vec3::splat(4.0)),
        ));
        world.spawn((
            Camera3d::default(),
            Camera {
                target: RenderTarget::Image(target.clone().into()),
                hdr: true,
                clear_color: ClearColorConfig::Custom(Color::NONE),
                ..default()
            },
            DepthPrepass,
            NormalPrepass,
            DepthOfField {
                mode: DepthOfFieldMode::Disabled,
                ..default()
            },
            Msaa::Off,
            Projection::Orthographic(OrthographicProjection {
                scaling_mode: ScalingMode::Fixed {
                    width: 32.0,
                    height: 32.0,
                },
                ..OrthographicProjection::default_3d()
            }),
            Transform::from_xyz(-32.0, 6.0, 4.0),
        ));

        compile_pipelines(&mut app);
        assert_eq!(
            app.world()
                .resource::<Assets<Shader>>()
                .get(FRAGMENT_HANDLE.id())
                .unwrap()
                .path,
            original_shader,
            "debug off uses the original fragment shader"
        );
        let original = read_colors(&mut app, &target, "disabled");
        assert_eq!(original[0], original[1], "both proxies sample one material");
        assert_ne!(original[0], original[2], "an imposter drew on the target");
        assert_ne!(original[3], original[2], "the masked imposter was drawn");
        assert!(
            (1..255).contains(&original[0][3]),
            "the proxy is translucent"
        );

        for _ in 0..2 {
            app.world_mut().resource_mut::<DebugImpostersEnabled>().0 = true;
            compile_pipelines(&mut app);
            let debug = read_colors(&mut app, &target, "enabled");
            assert_ne!(
                debug[0], debug[1],
                "neighboring requests have distinct tints"
            );
            assert_eq!(debug[2], original[2], "the background retains its color");
            assert_eq!(
                debug[3], original[3],
                "the untagged imposter stays uncolored"
            );
            assert_eq!(debug[0][3], original[0][3], "tint does not change alpha");
            assert_eq!(debug[1][3], original[1][3], "tint does not change alpha");

            app.world_mut().resource_mut::<DebugImpostersEnabled>().0 = false;
            compile_pipelines(&mut app);
            assert_eq!(read_colors(&mut app, &target, "restored"), original);
            assert_eq!(
                app.world()
                    .resource::<Assets<Shader>>()
                    .get(FRAGMENT_HANDLE.id())
                    .unwrap()
                    .path,
                original_shader,
                "turning debug off restores the original fragment shader"
            );
        }
    }

    fn read_colors(app: &mut App, target: &Handle<Image>, frame: &str) -> [[u8; 4]; 4] {
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
                    .as_chunks::<4>()
                    .0
                    .iter()
                    .flat_map(|bgra| [bgra[2], bgra[1], bgra[0]]),
            );
            std::fs::write(
                PathBuf::from(dir).join(format!("imposter-debug-{frame}.ppm")),
                ppm,
            )
            .unwrap();
        }
        [(16, 32), (48, 32), (32, 54), (32, 8)].map(|(x, y)| {
            let offset = (y * 64 + x) * 4;
            pixels[offset..offset + 4].try_into().unwrap()
        })
    }
}

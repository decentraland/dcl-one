//! GPU tests for the avatar branches of `bound_material.wgsl`. The avatar outline darkens a
//! mesh's lit surface but never its emission: a glowing particle a few pixels across, the Epic
//! Arrival Shield's, all of whose pixels are edge pixels, keeps its glow.
use std::sync::{Arc, Mutex};

use bevy::{
    asset::RenderAssetUsages,
    core_pipeline::tonemapping::{DebandDither, Tonemapping},
    prelude::*,
    render::mesh::{MeshTag, VertexAttributeValues},
    render::{
        camera::RenderTarget,
        gpu_readback::{Readback, ReadbackComplete},
        render_resource::{Extent3d, Face, TextureDimension, TextureFormat, TextureUsages},
        storage::ShaderStorageBuffer,
    },
};
use pipeline_test::{compile_pipelines, fragment_pipelines, headless_app, render_target};
use scene_material::{
    SceneBound, SceneMaterial, EMPTY_SCENE_CELLS, SCENE_MATERIAL_CONE_ONLY_DITHER_MESH_TAG,
    SCENE_MATERIAL_FACIAL_FEATURE_MESH_TAG, SCENE_MATERIAL_NO_DITHERING_MESH_TAG,
    SCENE_MATERIAL_OUTLINE_MESH_TAG, SCENE_MATERIAL_TOON_MESH_TAG,
};

fn f16_to_f32(h: u16) -> f32 {
    let s = if h & 0x8000 != 0 { -1.0 } else { 1.0 };
    let e = ((h >> 10) & 0x1f) as i32;
    let m = (h & 0x3ff) as f32;
    if e == 0 {
        s * m * 2f32.powi(-24)
    } else if e == 31 {
        f32::INFINITY
    } else {
        s * (1.0 + m / 1024.0) * 2f32.powi(e - 15)
    }
}

/// Renders the shield's particle material (toon, undithered, as on the player's avatar) on a 4 mm
/// quad 15 mm in front of an outlined backdrop, with and without the outline, and reads back the
/// HDR pixels. Needs a GPU, so `#[ignore]`d and skipped by CI:
///
/// ```text
/// cargo test -p avatar outlined_particle_keeps_its_glow -- --ignored --nocapture
/// ```
#[test]
#[ignore = "needs a GPU; run with --ignored"]
fn outlined_particle_keeps_its_glow() {
    let mut app = headless_app();
    app.add_plugins(MaterialPlugin::<SceneMaterial>::default());
    app.insert_resource(common::structs::AppConfig::default());
    app.finish();
    app.cleanup();

    let w = 1024u32;
    let mut target = Image::new_fill(
        Extent3d {
            width: w,
            height: w,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        &[0; 8],
        TextureFormat::Rgba16Float,
        RenderAssetUsages::all(),
    );
    target.texture_descriptor.usage |= TextureUsages::RENDER_ATTACHMENT | TextureUsages::COPY_SRC;
    let world = app.world_mut();
    let target = world.resource_mut::<Assets<Image>>().add(target);
    world.resource_mut::<Assets<ShaderStorageBuffer>>().insert(
        EMPTY_SCENE_CELLS.id(),
        ShaderStorageBuffer::new(&[0; 8], RenderAssetUsages::RENDER_WORLD),
    );

    // the glTF's particle material, derived as derived_scene_material does
    let factor = [0.11824828f32, 0.760_310_2, 1.0];
    let base = Color::linear_rgba(0.11696858, 0.76052517, 1.0, 1.0);
    let emissive = LinearRgba::rgb(
        crate::avatar_emission(factor[0]),
        crate::avatar_emission(factor[1]),
        crate::avatar_emission(factor[2]),
    );
    let backdrop = world
        .resource_mut::<Assets<Mesh>>()
        .add(Rectangle::new(1.0, 1.0));
    let particle = world
        .resource_mut::<Assets<Mesh>>()
        .add(Rectangle::new(0.004, 0.004));
    let mut mat = |emissive: LinearRgba| {
        world
            .resource_mut::<Assets<SceneMaterial>>()
            .add(SceneMaterial {
                base: StandardMaterial {
                    base_color: base,
                    emissive,
                    metallic: 0.0,
                    ..default()
                },
                extension: SceneBound::new_unbounded(),
            })
    };
    let glowing = mat(emissive);
    let plain = mat(LinearRgba::BLACK);
    let tag = SCENE_MATERIAL_TOON_MESH_TAG
        | SCENE_MATERIAL_NO_DITHERING_MESH_TAG
        | SCENE_MATERIAL_CONE_ONLY_DITHER_MESH_TAG;
    // the shield behind, outlined as an avatar mesh
    world.spawn((
        Mesh3d(backdrop),
        MeshMaterial3d(plain),
        MeshTag(tag | SCENE_MATERIAL_OUTLINE_MESH_TAG),
        Transform::default(),
    ));
    // a 4 mm particle 15 mm in front of it, with and without the outline
    for (x, outline) in [(-0.1, SCENE_MATERIAL_OUTLINE_MESH_TAG), (0.1, 0)] {
        world.spawn((
            Mesh3d(particle.clone()),
            MeshMaterial3d(glowing.clone()),
            MeshTag(tag | outline),
            Transform::from_xyz(x, 0.0, 0.015),
        ));
    }
    world.spawn((
        Camera3d::default(),
        Camera {
            hdr: true,
            target: RenderTarget::Image(target.clone().into()),
            ..default()
        },
        Msaa::Off,
        bevy::core_pipeline::prepass::DepthPrepass,
        bevy::core_pipeline::prepass::NormalPrepass,
        Tonemapping::None,
        DebandDither::Disabled,
        Transform::from_xyz(0.0, 0.0, 1.5).looking_at(Vec3::ZERO, Vec3::Y),
    ));
    world.spawn((
        DirectionalLight::default(),
        Transform::from_xyz(0.0, 1.0, 3.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));

    compile_pipelines(&mut app);
    for _ in 0..5 {
        app.update();
    }
    let data: Arc<Mutex<Option<Vec<u8>>>> = default();
    let sink = data.clone();
    app.world_mut().spawn(Readback::texture(target)).observe(
        move |trigger: Trigger<ReadbackComplete>| {
            *sink.lock().unwrap() = Some(trigger.event().0.clone());
        },
    );
    for _ in 0..20 {
        app.update();
        if data.lock().unwrap().is_some() {
            break;
        }
    }
    let bytes = data.lock().unwrap().clone().expect("no readback");
    let px = |x: u32, y: u32| {
        let i = ((y * w + x) * 8) as usize;
        (0..4)
            .map(|c| f16_to_f32(u16::from_le_bytes([bytes[i + c * 2], bytes[i + c * 2 + 1]])))
            .collect::<Vec<_>>()
    };
    // the particles stand 0.1 m either side of the centre, 1.485 m from the camera; the default
    // vertical field of view is 45 degrees
    let off = (0.1 / 1.485 / (std::f32::consts::FRAC_PI_8).tan() * 512.0) as u32;
    let outlined = px(512 - off, 512);
    let plain = px(512 + off, 512);
    println!("outlined {outlined:?}, not outlined {plain:?}");
    // the emission alone is 12.5 in blue; the lit part is ~1
    assert!(plain[2] > 12.0, "the particle does not glow: {plain:?}");
    assert!(
        outlined[2] > 0.95 * plain[2],
        "the outline took the glow: {outlined:?} against {plain:?}"
    );
}

/// Compiles the pipeline variants the avatar branches of `bound_material.wgsl` take: opaque,
/// alpha-tested and blended avatar materials with base colour and emissive textures read from the
/// first or the second UV set, on meshes with and without a second UV set and vertex colours,
/// outlined, plus a face feature. Needs a GPU, so `#[ignore]`d and skipped by CI:
///
/// ```text
/// cargo test -p avatar avatar_material_variants_compile -- --ignored --nocapture
/// ```
#[test]
#[ignore = "needs a GPU; run with --ignored"]
fn avatar_material_variants_compile() {
    let mut app = headless_app();
    app.add_plugins(MaterialPlugin::<SceneMaterial>::default());
    app.insert_resource(common::structs::AppConfig::default());
    app.finish();
    app.cleanup();

    let target = render_target(&mut app);
    let world = app.world_mut();
    world.resource_mut::<Assets<ShaderStorageBuffer>>().insert(
        EMPTY_SCENE_CELLS.id(),
        ShaderStorageBuffer::new(&[0; 8], RenderAssetUsages::RENDER_WORLD),
    );
    let texture = world.resource_mut::<Assets<Image>>().add(Image::new_fill(
        Extent3d::default(),
        TextureDimension::D2,
        &[255, 255, 255, 128],
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::RENDER_WORLD,
    ));

    // a quad, and the same quad with a second UV set and vertex colours
    let plain = Mesh::from(Rectangle::new(0.2, 0.2));
    let Some(VertexAttributeValues::Float32x2(uvs)) = plain.attribute(Mesh::ATTRIBUTE_UV_0) else {
        panic!("a rectangle has UVs");
    };
    let vertices = uvs.len();
    let extra = plain
        .clone()
        .with_inserted_attribute(Mesh::ATTRIBUTE_UV_1, uvs.clone())
        .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, vec![[1.0f32; 4]; vertices]);
    let meshes = {
        let mut meshes = world.resource_mut::<Assets<Mesh>>();
        [meshes.add(plain), meshes.add(extra)]
    };

    let mut materials = Vec::new();
    for alpha_mode in [AlphaMode::Opaque, AlphaMode::Mask(0.5), AlphaMode::Blend] {
        for channel in [bevy::pbr::UvChannel::Uv0, bevy::pbr::UvChannel::Uv1] {
            // as derived_scene_material builds an avatar material
            materials.push(SceneMaterial {
                base: StandardMaterial {
                    base_color_texture: Some(texture.clone()),
                    base_color_channel: channel.clone(),
                    emissive: LinearRgba::rgb(12.5, 12.5, 12.5),
                    emissive_texture: Some(texture.clone()),
                    emissive_channel: channel,
                    alpha_mode,
                    cull_mode: Some(Face::Back),
                    ..default()
                },
                extension: SceneBound::new_unbounded(),
            });
        }
    }
    let materials: Vec<_> = materials
        .into_iter()
        .map(|material| world.resource_mut::<Assets<SceneMaterial>>().add(material))
        .collect();

    let toon = SCENE_MATERIAL_TOON_MESH_TAG
        | SCENE_MATERIAL_NO_DITHERING_MESH_TAG
        | SCENE_MATERIAL_CONE_ONLY_DITHER_MESH_TAG
        | SCENE_MATERIAL_OUTLINE_MESH_TAG;
    for (i, material) in materials.iter().enumerate() {
        for (j, mesh) in meshes.iter().enumerate() {
            world.spawn((
                Mesh3d(mesh.clone()),
                MeshMaterial3d(material.clone()),
                MeshTag(toon),
                Transform::from_xyz(i as f32 * 0.25 - 0.6, j as f32 * 0.25 - 0.1, 0.0),
            ));
        }
    }
    world.spawn((
        Mesh3d(meshes[0].clone()),
        MeshMaterial3d(materials[0].clone()),
        MeshTag(SCENE_MATERIAL_FACIAL_FEATURE_MESH_TAG | SCENE_MATERIAL_OUTLINE_MESH_TAG),
        Transform::from_xyz(0.0, 0.4, 0.0),
    ));

    world.spawn((
        Camera3d::default(),
        Camera {
            hdr: true,
            target: RenderTarget::Image(target.into()),
            ..default()
        },
        Msaa::Off,
        bevy::core_pipeline::prepass::DepthPrepass,
        bevy::core_pipeline::prepass::NormalPrepass,
        Transform::from_xyz(0.0, 0.0, 2.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
    world.spawn((
        DirectionalLight {
            shadows_enabled: true,
            ..default()
        },
        Transform::from_xyz(0.0, 1.0, 3.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));

    compile_pipelines(&mut app);
    // at least one per alpha mode; a variant that fails to compile panics above
    let compiled = fragment_pipelines(&app, "embedded://shaders/bound_material.wgsl");
    assert!(compiled >= 3, "{compiled} bound_material.wgsl pipelines");
}

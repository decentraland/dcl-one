//! Skinned meshes draw with the skin matrices the compute pass writes. The pass and the skinned
//! bind group layouts are the Bevy fork's (`bevy_pbr::render::skin`), whose own workspace has no
//! renderer to test them with; the test is here because CI runs this crate's GPU tests.

use bevy::{
    asset::{RenderAssetTransferPriority, RenderAssetUsages},
    core_pipeline::prepass::{DepthPrepass, MotionVectorPrepass, NormalPrepass},
    pbr::SkinUniforms,
    prelude::*,
    render::{
        camera::RenderTarget,
        mesh::{
            morph::{MeshMorphWeights, MorphAttributes, MorphTargetImage},
            skinning::{SkinnedMesh, SkinnedMeshInverseBindposes},
            VertexAttributeValues,
        },
        render_resource::{CachedPipelineState, PipelineCache, PipelineDescriptor, ShaderDefVal},
        RenderApp,
    },
};
use pipeline_test::{compile_pipelines, gpu_errors, headless_app, render_target};

/// Joints of each skin of the second wave.
const WAVE_JOINTS: usize = 100;
/// Skins of the second wave: together they outgrow the skin buffers' first size of 256 joints.
const WAVE_SKINS: usize = 4;

/// Spawns `count` joints, the second one raised, and returns them.
fn spawn_joints(world: &mut World, count: usize) -> Vec<Entity> {
    (0..count)
        .map(|joint| {
            let height = if joint == 1 { 0.2 } else { 0.0 };
            world.spawn(Transform::from_xyz(0.0, height, 0.0)).id()
        })
        .collect()
}

/// The skin matrices compute pipeline and every pipeline that draws a skinned mesh compile on this
/// machine's adapter, and frames draw with them without a device error, in a renderer with no
/// window: a skinned cube and a cube that is both morphed and skinned, in front of a camera with
/// the depth, normal and motion vector prepasses and a shadowing light. A second wave of skins
/// then outgrows the skin buffers, so the frames after it bind the new buffers of the current and
/// the previous frame. Needs a GPU, so `#[ignore]`d:
///
/// ```text
/// cargo test -p visuals skinned_meshes_draw_with_computed_skin_matrices -- --ignored --nocapture
/// ```
#[test]
#[ignore = "needs a GPU; run with --ignored"]
fn skinned_meshes_draw_with_computed_skin_matrices() {
    let mut app = headless_app();
    app.finish();
    app.cleanup();
    let errors = gpu_errors(&app);

    let target = render_target(&mut app);
    let world = app.world_mut();

    let cube = Mesh::from(Cuboid::new(0.5, 0.5, 0.5));
    let vertex_count = cube.count_vertices();
    let mut skinned = cube.clone();
    skinned.insert_attribute(
        Mesh::ATTRIBUTE_JOINT_INDEX,
        VertexAttributeValues::Uint16x4(vec![[0, 1, 0, 0]; vertex_count]),
    );
    skinned.insert_attribute(
        Mesh::ATTRIBUTE_JOINT_WEIGHT,
        vec![[0.5f32, 0.5, 0.0, 0.0]; vertex_count],
    );
    let morph_targets = MorphTargetImage::new(
        [(0..vertex_count).map(|_| MorphAttributes::new(Vec3::Y * 0.1, Vec3::ZERO, Vec3::ZERO))]
            .into_iter(),
        vertex_count,
        RenderAssetUsages::RENDER_WORLD,
        RenderAssetTransferPriority::default(),
    )
    .unwrap();
    let morph_targets = world.resource_mut::<Assets<Image>>().add(morph_targets.0);
    let mut morphed_skinned = skinned.clone();
    morphed_skinned.set_morph_targets(morph_targets);

    let mut meshes = world.resource_mut::<Assets<Mesh>>();
    let (skinned, morphed_skinned) = (meshes.add(skinned), meshes.add(morphed_skinned));
    let material = world
        .resource_mut::<Assets<StandardMaterial>>()
        .add(StandardMaterial::default());
    let mut bindposes = world.resource_mut::<Assets<SkinnedMeshInverseBindposes>>();
    let two_bindposes = bindposes.add(SkinnedMeshInverseBindposes::from(vec![Mat4::IDENTITY; 2]));
    let wave_bindposes = bindposes.add(SkinnedMeshInverseBindposes::from(vec![
        Mat4::IDENTITY;
        WAVE_JOINTS
    ]));

    world.spawn((
        Camera3d::default(),
        Camera {
            target: RenderTarget::Image(target.into()),
            ..default()
        },
        DepthPrepass,
        NormalPrepass,
        MotionVectorPrepass,
        Transform::from_xyz(0.0, 0.0, 4.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
    world.spawn((
        DirectionalLight {
            shadows_enabled: true,
            ..default()
        },
        Transform::from_xyz(1.0, 3.0, 2.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
    let joints = spawn_joints(world, 2);
    let moving_joint = joints[1];
    world.spawn((
        Mesh3d(skinned.clone()),
        MeshMaterial3d(material.clone()),
        SkinnedMesh {
            inverse_bindposes: two_bindposes.clone(),
            joints,
        },
        Transform::from_xyz(-1.0, 0.0, 0.0),
    ));
    let joints = spawn_joints(world, 2);
    world.spawn((
        Mesh3d(morphed_skinned),
        MeshMaterial3d(material.clone()),
        SkinnedMesh {
            inverse_bindposes: two_bindposes,
            joints,
        },
        MeshMorphWeights::new(vec![0.5]).unwrap(),
        Transform::from_xyz(1.0, 0.0, 0.0),
    ));

    // A joint that moves every frame, so the previous frame's matrices differ from the current.
    let mut frame = 0;
    let mut draw_frames = |app: &mut App, frames: usize| {
        for _ in 0..frames {
            frame += 1;
            app.world_mut()
                .entity_mut(moving_joint)
                .insert(Transform::from_xyz(0.0, 0.2 + 0.01 * frame as f32, 0.0));
            app.update();
        }
    };

    compile_pipelines(&mut app);
    draw_frames(&mut app, 5);
    let skin_buffer_sizes = |app: &App| {
        let uniforms = app.sub_app(RenderApp).world().resource::<SkinUniforms>();
        (
            uniforms.skin_matrices_buffer.size(),
            uniforms.prev_skin_matrices_buffer.size(),
        )
    };
    let first_sizes = skin_buffer_sizes(&app);

    let world = app.world_mut();
    for skin in 0..WAVE_SKINS {
        let joints = spawn_joints(world, WAVE_JOINTS);
        world.spawn((
            Mesh3d(skinned.clone()),
            MeshMaterial3d(material.clone()),
            SkinnedMesh {
                inverse_bindposes: wave_bindposes.clone(),
                joints,
            },
            Transform::from_xyz(skin as f32 - 1.5, 1.0, 0.0),
        ));
    }
    compile_pipelines(&mut app);
    draw_frames(&mut app, 5);

    let errors = errors.lock().unwrap();
    assert!(errors.is_empty(), "GPU errors:\n{}", errors.join("\n\n"));

    let grown_sizes = skin_buffer_sizes(&app);
    let wave_bytes = (WAVE_SKINS * WAVE_JOINTS * size_of::<Mat4>()) as u64;
    assert!(
        grown_sizes.0 > first_sizes.0 && grown_sizes.0 >= wave_bytes,
        "the skin matrices buffer did not grow: {first_sizes:?} then {grown_sizes:?}"
    );
    assert_eq!(grown_sizes.0, grown_sizes.1);

    let cache = app.sub_app(RenderApp).world().resource::<PipelineCache>();
    assert!(
        cache.pipelines().any(|pipeline| {
            matches!(pipeline.state, CachedPipelineState::Ok(_))
                && matches!(
                    &pipeline.descriptor,
                    PipelineDescriptor::ComputePipelineDescriptor(descriptor)
                        if descriptor.label.as_deref() == Some("skin matrices pipeline")
                )
        }),
        "the skin matrices pipeline did not compile in the pipeline cache"
    );
    let compiled_with = |defs: &[&str]| {
        cache
            .pipelines()
            .filter(|pipeline| {
                matches!(pipeline.state, CachedPipelineState::Ok(_))
                    && matches!(
                        &pipeline.descriptor,
                        PipelineDescriptor::RenderPipelineDescriptor(descriptor)
                            if defs.iter().all(|def| {
                                descriptor.vertex.shader_defs.contains(&ShaderDefVal::from(*def))
                            })
                    )
            })
            .count()
    };
    for defs in [
        &["SKINNED"][..],
        &["SKINNED", "MOTION_VECTOR_PREPASS"],
        &["SKINNED", "MORPH_TARGETS"],
        &["SKINNED", "MORPH_TARGETS", "MOTION_VECTOR_PREPASS"],
    ] {
        assert!(compiled_with(defs) >= 1, "no pipeline with {defs:?}");
    }
}

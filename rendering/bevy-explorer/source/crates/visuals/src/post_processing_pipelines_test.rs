//! The tonemapping and depth of field variants that scenes' `PBPostProcessing` uses compile and
//! draw. Their shaders come from the Bevy fork's patches (grading without a curve and its
//! `untonemapped_mix`, the depth of field's `linear_ramp`, `rgb_to_hsv`), whose own workspace has
//! no renderer to test them with; the test is here because CI runs this crate's GPU tests.

use bevy::{
    core_pipeline::{
        dof::{DepthOfField, DepthOfFieldMode},
        prepass::DepthPrepass,
        tonemapping::Tonemapping,
    },
    prelude::*,
    render::{
        camera::RenderTarget,
        render_resource::{CachedPipelineState, PipelineCache, PipelineDescriptor, ShaderDefVal},
        view::{ColorGrading, ColorGradingGlobal, Msaa},
        RenderApp,
    },
};
use pipeline_test::{compile_pipelines, gpu_errors, headless_app, render_target};

/// Three HDR cameras draw a lit cube without a device error: one with no curve and an exposure,
/// whose grading pass runs without a curve; one with a curve faded halfway out, a hue rotation
/// and a white balance; and one with a scene's linear depth of field ramp. Needs a GPU, so
/// `#[ignore]`d:
///
/// ```text
/// cargo test -p visuals post_processing_variants_compile_and_draw -- --ignored --nocapture
/// ```
#[test]
#[ignore = "needs a GPU; run with --ignored"]
fn post_processing_variants_compile_and_draw() {
    let mut app = headless_app();
    app.finish();
    app.cleanup();
    let errors = gpu_errors(&app);

    let graded = |global: ColorGradingGlobal| ColorGrading {
        global,
        ..default()
    };
    let cameras = [
        (
            Tonemapping::None,
            graded(ColorGradingGlobal {
                exposure: 1.0,
                ..default()
            }),
            None,
        ),
        (
            Tonemapping::AcesFitted,
            graded(ColorGradingGlobal {
                untonemapped_mix: 0.5,
                hue: 1.0,
                temperature: 0.1,
                tint: 0.05,
                ..default()
            }),
            None,
        ),
        (
            Tonemapping::AcesFitted,
            ColorGrading::default(),
            Some(DepthOfField {
                mode: DepthOfFieldMode::Gaussian,
                linear_ramp: 1.0,
                ..default()
            }),
        ),
    ];
    for (order, (tonemapping, grading, dof)) in cameras.into_iter().enumerate() {
        let target = render_target(&mut app);
        let mut camera = app.world_mut().spawn((
            Camera3d::default(),
            Camera {
                hdr: true,
                order: order as isize,
                target: RenderTarget::Image(target.into()),
                ..default()
            },
            tonemapping,
            grading,
            Msaa::Off,
            DepthPrepass,
            Transform::from_xyz(0.0, 0.0, 4.0).looking_at(Vec3::ZERO, Vec3::Y),
        ));
        if let Some(dof) = dof {
            camera.insert(dof);
        }
    }

    let world = app.world_mut();
    let mesh = world
        .resource_mut::<Assets<Mesh>>()
        .add(Cuboid::new(1.0, 1.0, 1.0));
    let material = world
        .resource_mut::<Assets<StandardMaterial>>()
        .add(StandardMaterial::default());
    world.spawn((Mesh3d(mesh), MeshMaterial3d(material)));
    world.spawn((
        DirectionalLight::default(),
        Transform::from_xyz(1.0, 3.0, 2.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));

    compile_pipelines(&mut app);
    for _ in 0..5 {
        app.update();
    }

    let errors = errors.lock().unwrap();
    assert!(errors.is_empty(), "GPU errors:\n{}", errors.join("\n\n"));

    let cache = app.sub_app(RenderApp).world().resource::<PipelineCache>();
    let compiled = |label: &str, defs: &[&str]| {
        cache.pipelines().any(|pipeline| {
            matches!(pipeline.state, CachedPipelineState::Ok(_))
                && matches!(
                    &pipeline.descriptor,
                    PipelineDescriptor::RenderPipelineDescriptor(descriptor)
                        if descriptor.label.as_deref() == Some(label)
                            && descriptor.fragment.as_ref().is_some_and(|fragment| {
                                defs.iter().all(|def| {
                                    fragment.shader_defs.contains(&ShaderDefVal::from(*def))
                                })
                            })
                )
        })
    };
    for defs in [
        &["TONEMAP_METHOD_NONE"][..],
        &[
            "TONEMAP_METHOD_ACES_FITTED",
            "UNTONEMAPPED_MIX",
            "HUE_ROTATE",
            "WHITE_BALANCE",
        ],
    ] {
        assert!(
            compiled("tonemapping pipeline", defs),
            "no tonemapping pipeline with {defs:?}"
        );
    }
    assert!(
        compiled("depth of field pipeline", &[]),
        "the depth of field pipeline did not compile"
    );
}

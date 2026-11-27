//! The tonemapping and depth of field variants that scenes' `PBPostProcessing` uses compile and
//! draw. Their shaders come from the Bevy fork's patches (grading without a curve and its
//! `untonemapped_mix`, the depth of field's `linear_ramp`, `rgb_to_hsv`), whose own workspace has
//! no renderer to test them with; the test is here because CI runs this crate's GPU tests.

use std::sync::{Arc, Mutex};

use bevy::{
    core_pipeline::{
        dof::{DepthOfField, DepthOfFieldMode},
        prepass::DepthPrepass,
        tonemapping::{DebandDither, Tonemapping},
    },
    prelude::*,
    render::{
        camera::RenderTarget,
        gpu_readback::{Readback, ReadbackComplete},
        render_resource::{
            CachedPipelineState, PipelineCache, PipelineDescriptor, ShaderDefVal, TextureUsages,
        },
        view::{ColorGrading, ColorGradingGlobal, Msaa},
        RenderApp,
    },
};
use common::structs::default_color_grading;
use pipeline_test::{compile_pipelines, gpu_errors, headless_app, render_target};

/// Five HDR cameras draw a lit cube without a device error: one with no curve and an exposure,
/// whose grading pass runs without a curve; one with the explorer's own grading and dither, its
/// curve faded halfway out, a hue rotation and a white balance; two with a scene's linear depth
/// of field ramp, Gaussian and bokeh; and one with no curve and no grading, which the first must
/// come out brighter than (bevy skips the pass for no curve). Needs a GPU, so `#[ignore]`d:
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

    let mut shipped = default_color_grading();
    shipped.global = ColorGradingGlobal {
        untonemapped_mix: 0.5,
        hue: 1.0,
        temperature: 0.1,
        tint: 0.05,
        ..shipped.global
    };
    let linear_dof = |mode| {
        Some(DepthOfField {
            mode,
            linear_ramp: 1.0,
            ..default()
        })
    };
    let cameras = [
        (
            Tonemapping::None,
            ColorGrading {
                global: ColorGradingGlobal {
                    exposure: 1.0,
                    ..default()
                },
                ..default()
            },
            DebandDither::Disabled,
            None,
        ),
        (
            Tonemapping::AcesFitted,
            shipped,
            DebandDither::Enabled,
            None,
        ),
        (
            Tonemapping::AcesFitted,
            ColorGrading::default(),
            DebandDither::Disabled,
            linear_dof(DepthOfFieldMode::Gaussian),
        ),
        (
            Tonemapping::AcesFitted,
            ColorGrading::default(),
            DebandDither::Disabled,
            linear_dof(DepthOfFieldMode::Bokeh),
        ),
        (
            Tonemapping::None,
            ColorGrading::default(),
            DebandDither::Disabled,
            None,
        ),
    ];
    let mut targets = Vec::new();
    for (order, (tonemapping, grading, dither, dof)) in cameras.into_iter().enumerate() {
        let target = render_target(&mut app);
        if let Some(image) = app
            .world_mut()
            .resource_mut::<Assets<Image>>()
            .get_mut(&target)
        {
            image.texture_descriptor.usage |= TextureUsages::COPY_SRC;
        }
        targets.push(target.clone());
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
            dither,
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

    // the no-curve cameras' targets, the one with an exposure and the one with none
    let exposed = brightness(&mut app, &targets[0]);
    let plain = brightness(&mut app, &targets[4]);
    assert!(
        exposed > plain * 1.2,
        "exposure +1 without a curve: {exposed} against {plain}"
    );

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
            "SECTIONAL_COLOR_GRADING",
            "UNTONEMAPPED_MIX",
            "HUE_ROTATE",
            "WHITE_BALANCE",
            "DEBAND_DITHER",
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
    // bokeh's second pass reads both of the first pass's outputs
    assert!(
        compiled("depth of field pipeline", &["DUAL_INPUT"]),
        "the bokeh depth of field pipeline did not compile"
    );
}

/// The mean of the colour channels of an 8-bit target, read back from the GPU.
fn brightness(app: &mut App, target: &Handle<Image>) -> f32 {
    let read = Arc::new(Mutex::new(None));
    let sink = read.clone();
    let entity = app
        .world_mut()
        .spawn(Readback::texture(target.clone()))
        .observe(move |event: Trigger<ReadbackComplete>| {
            *sink.lock().unwrap() = Some(event.event().0.clone());
        })
        .id();
    let start = std::time::Instant::now();
    let bytes = loop {
        app.update();
        if let Some(bytes) = read.lock().unwrap().take() {
            break bytes;
        }
        assert!(start.elapsed().as_secs() < 30, "GPU readback timed out");
    };
    app.world_mut().despawn(entity);
    // 64 texels of 4 bytes fill each 256-byte row; the alpha byte is left out
    let colour: Vec<f32> = bytes
        .chunks_exact(4)
        .flat_map(|texel| texel[..3].iter().map(|&byte| f32::from(byte)))
        .collect();
    colour.iter().sum::<f32>() / colour.len() as f32
}

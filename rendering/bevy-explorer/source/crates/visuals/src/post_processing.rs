//! Applies scenes' `PBPostProcessing` to the player's camera and to texture cameras.
//!
//! The explorer's writers (a camera's spawn, the player's settings, the console and the
//! auto-focus) edit a camera's [`CameraLook`]. In `PostUpdateSets::PostProcessing`, after all of
//! them, a camera's `Tonemapping`, `ColorGrading`, `Bloom` and `DepthOfField` are written from
//! its look with a scene's settings over it: on the player's camera, those of the scene the
//! player's look comes from (and a virtual camera's own, while it is active), blended as the
//! camera moves; on any other camera, those on its parent, the scene entity of a texture camera.

use bevy::{
    core_pipeline::{
        bloom::Bloom,
        dof::{DepthOfField, DepthOfFieldMode},
        tonemapping::Tonemapping,
    },
    prelude::*,
    render::view::ColorGrading,
};
use bevy_console::ConsoleCommand;
use common::{
    sets::PostUpdateSets,
    structs::{scaled_bloom, CameraLook, CameraOverride, PrimaryCamera},
};
use console::{AddDebugSection, DebugSection, DoAddConsoleCommand};
use dcl_component::SceneEntityId;
use scene_runner::{
    renderer_context::RendererSceneContext,
    update_world::{
        camera_mode_area::VirtualCamera,
        post_processing::{PostProcessing, TonemapCurve},
    },
};

use crate::day_night::SceneSource;

/// seconds to blend between scenes' settings, as the global light does
const SCENE_TRANSITION_TIME: f32 = 1.0;
/// CIE 1931 x of the D65 white point
const D65_X: f32 = 0.31271;

pub struct PostProcessingPlugin;

impl Plugin for PostProcessingPlugin {
    fn build(&self, app: &mut App) {
        app.register_required_components::<PrimaryCamera, PrimaryBlend>()
            .add_systems(
                PostUpdate,
                (show_primary, show_other_cameras).in_set(PostUpdateSets::PostProcessing),
            )
            .add_console_command::<PostProcessingCommand, _>(post_processing_command)
            .add_debug_section(DebugSection {
                title: "Post processing",
                query: "/post_processing",
                fields: &[
                    ("Scene", "scene"),
                    ("Virtual camera", "virtualCamera"),
                    ("Blend progress", "blend"),
                    ("Tonemapping", "tonemapping"),
                    ("Exposure (stops)", "exposure"),
                    ("Saturation", "saturation"),
                    ("Contrast", "contrast"),
                    ("Hue shift (radians)", "hueShift"),
                    ("Temperature", "temperature"),
                    ("Tint", "tint"),
                    ("Bloom multiplier", "bloomIntensity"),
                    ("Depth of field intensity", "dofIntensity"),
                    ("Focus start (m)", "dofFocusStart"),
                    ("Focus end (m)", "dofFocusEnd"),
                    ("Applied focus (m)", "appliedFocus"),
                    ("Applied blur at focus end (px)", "appliedBlur"),
                    ("Texture cameras with settings", "textureCameras"),
                ],
                actions: &[],
                inputs: &[],
            });
    }
}

/// What a camera shows: a curve, the grading (whose `untonemapped_mix` fades the curve out), a
/// multiplier of the camera's bloom and, when the settings change it, the depth of field.
#[derive(Clone)]
struct Output {
    curve: Tonemapping,
    grading: ColorGrading,
    bloom_scale: f32,
    dof: Option<Focus>,
}

/// Depth of field as it blends, in what the eye sees: `strength` is the circle of confusion at
/// `end` in pixels, 0 for none. `ramp` is bevy's `DepthOfField::linear_ramp`: 0 for the lens's
/// blur (the explorer's own), 1 for a scene's, which rises evenly from `start` to `end`. The
/// aperture for the lens's part is worked out each frame, since the blur an aperture gives changes
/// steeply with the focus distance.
#[derive(Clone, Copy, PartialEq, Debug)]
struct Focus {
    start: f32,
    end: f32,
    strength: f32,
    ramp: f32,
}

impl Focus {
    /// The camera's own depth of field, for a perspective camera.
    fn of(dof: &DepthOfField, lens: &Lens) -> Option<Self> {
        let fov = lens.fov?;
        let strength = if dof.mode == DepthOfFieldMode::Disabled {
            0.0
        } else {
            let (start, end) = (dof.focal_distance, dof.max_depth);
            circle_of_confusion(
                dof.aperture_f_stops.recip(),
                start,
                end,
                dof,
                fov,
                lens.height,
            )
        };
        Some(Self {
            start: dof.focal_distance,
            end: dof.max_depth,
            strength,
            ramp: dof.linear_ramp,
        })
    }

    /// Distances blend geometrically, so the focus moves at an even pace from far to near.
    fn lerp(self, other: Self, t: f32) -> Self {
        fn geometric(a: f32, b: f32, t: f32) -> f32 {
            let (a, b) = (a.max(0.01), b.max(0.01));
            a * (b / a).powf(t)
        }
        Self {
            start: geometric(self.start, other.start, t),
            end: geometric(self.end, other.end, t),
            strength: self.strength.lerp(other.strength, t),
            ramp: self.ramp.lerp(other.ramp, t),
        }
    }
}

/// Where the camera is, for the focus distances.
struct Lens {
    /// vertical field of view; `None` for an orthographic camera, which keeps its depth of field
    fov: Option<f32>,
    /// render target height in pixels
    height: f32,
}

/// The output of `settings` over a camera whose own values are `base`. `texture_camera` turns
/// bloom off by default under `TonemapCurve::None`.
fn compose(
    settings: &PostProcessing,
    base: &CameraLook,
    lens: &Lens,
    texture_camera: bool,
) -> Output {
    let no_curve = settings.tonemapping == Some(TonemapCurve::None);

    let mut grading = if no_curve {
        ColorGrading::default()
    } else {
        base.grading.clone()
    };
    let global = &mut grading.global;
    global.exposure += settings.exposure.unwrap_or(0.0);
    global.hue += settings.hue_shift.unwrap_or(0.0);
    let (temperature, tint) = white_balance(
        settings.temperature.unwrap_or(0.0),
        settings.tint.unwrap_or(0.0),
    );
    global.temperature += temperature;
    global.tint += tint;
    global.untonemapped_mix = if no_curve { 1.0 } else { 0.0 };
    for section in grading.all_sections_mut() {
        section.saturation *= settings.saturation.unwrap_or(1.0);
        section.contrast *= settings.contrast.unwrap_or(1.0);
    }

    let curve = match settings.tonemapping {
        Some(TonemapCurve::Aces) => Tonemapping::AcesFitted,
        // with no curve the camera's curve stays selected, faded out by the mix
        Some(TonemapCurve::Default | TonemapCurve::None) | None => base.tonemapping,
    };

    let bloom_scale = settings
        .bloom_intensity
        .unwrap_or(if no_curve && texture_camera { 0.0 } else { 1.0 });

    Output {
        curve,
        grading,
        bloom_scale,
        dof: focus(settings, base.dof.as_ref(), lens),
    }
}

/// bevy's white point offsets (`ColorGradingGlobal::temperature` and `tint`, CIE 1931 xy) for a
/// scene's temperature and tint. Temperature moves the white point along the daylight locus, so
/// warm is orange rather than magenta; tint moves it off the locus.
fn white_balance(temperature: f32, tint: f32) -> (f32, f32) {
    fn locus_y(x: f32) -> f32 {
        2.87 * x - 3.0 * x * x - 0.275_095_07
    }
    let t = temperature / 0.65;
    let x = D65_X - t * if t < 0.0 { 0.1 } else { 0.05 };
    let y = locus_y(x) + tint / 0.65 * 0.05;
    (D65_X - x, y - locus_y(D65_X))
}

/// The depth of field the settings give, or `None` to keep the camera's own.
fn focus(settings: &PostProcessing, base: Option<&DepthOfField>, lens: &Lens) -> Option<Focus> {
    if !focus_set(settings) {
        return None;
    }
    let base = base?;
    // the player's setting is off: a scene can't turn it on
    if base.mode == DepthOfFieldMode::Disabled {
        return None;
    }
    let own = Focus::of(base, lens)?;

    // distances a scene sets are absolute; unset, the start follows the camera's own focus
    let start = settings.dof_focus_start.unwrap_or(base.focal_distance);
    let end = settings
        .dof_focus_end
        .unwrap_or(base.max_depth)
        .max(start + 0.1);

    let strength = match settings.dof_intensity {
        // the player's setting: its blur at its own focus end
        None => own.strength,
        Some(intensity) => intensity * base.max_circle_of_confusion_diameter,
    };

    Some(Focus {
        start,
        end,
        strength,
        ramp: 1.0,
    })
}

/// `f^2 / (sensor_height (start - f)) * height`: by bevy's thin-lens formula (`dof.wgsl`,
/// `calculate_circle_of_confusion`) the circle of confusion at `end` is this times
/// `(end - start) / end / aperture_f_stops`. `None` when nothing beyond `start` can blur.
fn lens_scale(start: f32, end: f32, dof: &DepthOfField, fov: f32, height: f32) -> Option<f32> {
    let focal_length = 0.5 * dof.sensor_height / (0.5 * fov).tan();
    (height > 0.0 && start > focal_length && end > start).then(|| {
        focal_length * focal_length * (end - start) * height
            / (dof.sensor_height * end * (start - focal_length))
    })
}

/// The circle of confusion at `end`, in pixels, that the aperture `1 / blur` gives.
fn circle_of_confusion(
    blur: f32,
    start: f32,
    end: f32,
    dof: &DepthOfField,
    fov: f32,
    height: f32,
) -> f32 {
    lens_scale(start, end, dof, fov, height).map_or(0.0, |scale| {
        (scale * blur).min(dof.max_circle_of_confusion_diameter)
    })
}

/// `1 / aperture_f_stops` that gives a circle of confusion of `strength` pixels at `end`.
fn blur_for(strength: f32, start: f32, end: f32, dof: &DepthOfField, fov: f32, height: f32) -> f32 {
    match lens_scale(start, end, dof, fov, height) {
        Some(scale) if strength > 0.0 => strength / scale,
        _ => 0.0,
    }
}

fn lerp_grading(a: &ColorGrading, b: &ColorGrading, t: f32) -> ColorGrading {
    let mut out = b.clone();
    let (ga, gb, go) = (&a.global, &b.global, &mut out.global);
    go.exposure = ga.exposure.lerp(gb.exposure, t);
    go.temperature = ga.temperature.lerp(gb.temperature, t);
    go.tint = ga.tint.lerp(gb.tint, t);
    go.hue = ga.hue.lerp(gb.hue, t);
    go.post_saturation = ga.post_saturation.lerp(gb.post_saturation, t);
    go.untonemapped_mix = ga.untonemapped_mix.lerp(gb.untonemapped_mix, t);
    go.midtones_range = ga.midtones_range.start.lerp(gb.midtones_range.start, t)
        ..ga.midtones_range.end.lerp(gb.midtones_range.end, t);
    for ((sa, sb), so) in a.all_sections().zip(b.all_sections()).zip([
        &mut out.shadows,
        &mut out.midtones,
        &mut out.highlights,
    ]) {
        so.saturation = sa.saturation.lerp(sb.saturation, t);
        so.contrast = sa.contrast.lerp(sb.contrast, t);
        so.gamma = sa.gamma.lerp(sb.gamma, t);
        so.gain = sa.gain.lerp(sb.gain, t);
        so.lift = sa.lift.lerp(sb.lift, t);
    }
    out
}

fn lerp_output(from: &Output, to: &Output, base: &CameraLook, lens: &Lens, t: f32) -> Output {
    let own = base.dof.as_ref().and_then(|dof| Focus::of(dof, lens));
    Output {
        curve: if t < 0.5 { from.curve } else { to.curve },
        grading: lerp_grading(&from.grading, &to.grading, t),
        bloom_scale: from.bloom_scale.lerp(to.bloom_scale, t),
        dof: match (from.dof.or(own), to.dof.or(own)) {
            (Some(a), Some(b)) => Some(a.lerp(b, t)),
            _ => None,
        },
    }
}

/// What a camera with `base` as its own look shows for `output`.
fn shown(output: &Output, base: &CameraLook, lens: &Lens) -> CameraLook {
    let mut grading = output.grading.clone();
    let tonemapping = if grading.global.untonemapped_mix >= 1.0 {
        grading.global.untonemapped_mix = 0.0;
        Tonemapping::None
    } else {
        output.curve
    };

    let bloom = base
        .bloom
        .as_ref()
        .map(|own| scaled_bloom(own, output.bloom_scale));

    let dof = base.dof.map(|own| {
        let (Some(focus), Some(fov)) = (output.dof, lens.fov) else {
            return own;
        };
        let mut dof = own;
        dof.focal_distance = focus.start;
        dof.max_depth = focus.end;
        dof.linear_ramp = focus.ramp;
        // the lens's blur never passes its value at `end`, so this caps only the ramp
        dof.max_circle_of_confusion_diameter = focus.strength;
        if focus.strength > 1e-3 {
            let blur = blur_for(
                focus.strength,
                focus.start,
                focus.end,
                &own,
                fov,
                lens.height,
            );
            if blur > 1e-6 {
                dof.aperture_f_stops = blur.recip();
            }
        } else {
            // keeps the component (and its focus target, which the stars read)
            dof.mode = DepthOfFieldMode::Disabled;
        }
        dof
    });

    CameraLook {
        tonemapping,
        grading,
        bloom,
        dof,
    }
}

type LookComponents = (
    &'static mut Tonemapping,
    &'static mut ColorGrading,
    Option<&'static mut Bloom>,
    Option<&'static mut DepthOfField>,
);

/// Writes `look` to a camera's components, adding or removing its bloom and depth of field.
fn show(
    commands: &mut Commands,
    entity: Entity,
    look: CameraLook,
    (mut tonemapping, mut grading, bloom, dof): (
        Mut<Tonemapping>,
        Mut<ColorGrading>,
        Option<Mut<Bloom>>,
        Option<Mut<DepthOfField>>,
    ),
) {
    tonemapping.set_if_neq(look.tonemapping);
    *grading = look.grading;
    match (look.bloom, bloom) {
        (Some(look), Some(mut bloom)) => *bloom = look,
        (Some(look), None) => {
            commands.entity(entity).try_insert(look);
        }
        (None, Some(_)) => {
            commands.entity(entity).try_remove::<Bloom>();
        }
        (None, None) => (),
    }
    match (look.dof, dof) {
        (Some(look), Some(mut dof)) => *dof = look,
        (Some(look), None) => {
            commands.entity(entity).try_insert(look);
        }
        (None, Some(_)) => {
            commands.entity(entity).try_remove::<DepthOfField>();
        }
        (None, None) => (),
    }
}

fn lens(camera: &Camera, projection: &Projection) -> Lens {
    Lens {
        fov: match projection {
            Projection::Perspective(p) => Some(p.fov),
            _ => None,
        },
        height: camera
            .physical_target_size()
            .map_or(0.0, |size| size.y as f32),
    }
}

/// On the player's camera: which scene and virtual camera its settings come from, and the blend.
#[derive(Component, Default)]
struct PrimaryBlend {
    scene: Option<Entity>,
    virtual_camera: Option<Entity>,
    /// the start of the last camera move seen
    seen_move: Option<f64>,
    settings: PostProcessing,
    from: Option<Output>,
    last: Option<Output>,
    start: f64,
    duration: f32,
    progress: f32,
    /// a scene's settings applied last frame
    active: bool,
}

#[allow(clippy::too_many_arguments)]
fn show_primary(
    mut commands: Commands,
    time: Res<Time>,
    mut source: SceneSource,
    mut picked: Local<Option<Entity>>,
    changed_settings: Query<(), Changed<PostProcessing>>,
    mut removed_settings: RemovedComponents<PostProcessing>,
    contexts: Query<&RendererSceneContext>,
    settings: Query<&PostProcessing>,
    virtual_cameras: Query<(), With<VirtualCamera>>,
    mut camera: Query<(
        Entity,
        &PrimaryCamera,
        &mut PrimaryBlend,
        Ref<CameraLook>,
        &Camera,
        &Projection,
        LookComponents,
    )>,
) {
    // the settings on a scene's `engine.CameraEntity`
    let on_camera = |scene: Entity| {
        let camera = contexts
            .get(scene)
            .ok()?
            .bevy_entity(SceneEntityId::CAMERA)?;
        settings.get(camera).ok().copied()
    };
    let changed = !changed_settings.is_empty() || removed_settings.read().next().is_some();
    removed_settings.clear();
    if let Some(pick) = source.changed_pick(changed, |scene| on_camera(scene).is_some()) {
        *picked = pick;
    }
    let Ok((entity, primary, mut state, look, camera, projection, components)) =
        camera.single_mut()
    else {
        return;
    };

    let scene = *picked;
    let base_settings = scene.and_then(on_camera).unwrap_or_default();

    // a virtual camera's own settings, while the camera's last move went to it
    let camera_move = primary.last_move.as_ref();
    let virtual_camera = match camera_move.and_then(|camera_move| camera_move.to.as_ref()) {
        Some(CameraOverride::Cinematic(cinematic))
            if virtual_cameras.contains(cinematic.origin)
                && settings.contains(cinematic.origin) =>
        {
            Some(cinematic.origin)
        }
        _ => None,
    };
    let new_move = camera_move.filter(|camera_move| state.seen_move != Some(camera_move.started));
    state.seen_move = camera_move.map(|camera_move| camera_move.started);

    let effective = match virtual_camera.and_then(|e| settings.get(e).ok()) {
        Some(layer) => layer.over(base_settings),
        None => base_settings,
    };

    let camera_lens = lens(camera, projection);
    let target = compose(&effective, &look, &camera_lens, false);

    let now = time.elapsed_secs_f64();
    if scene != state.scene || virtual_camera != state.virtual_camera {
        state.duration = if scene != state.scene {
            SCENE_TRANSITION_TIME
        } else {
            // over the camera's move to or from the virtual camera; other changes apply at once
            new_move.map_or(0.0, |camera_move| camera_move.time)
        };
        state.from = state.last.take();
        state.start = now;
        state.scene = scene;
        state.virtual_camera = virtual_camera;
    }

    let progress = if state.duration <= 0.0 {
        1.0
    } else {
        ((now - state.start) as f32 / state.duration).clamp(0.0, 1.0)
    };
    let output = match (&state.from, progress < 1.0) {
        (Some(from), true) => lerp_output(from, &target, &look, &camera_lens, progress),
        _ => target,
    };

    let active = effective != PostProcessing::default() || progress < 1.0;
    if active || state.active || look.is_changed() {
        show(
            &mut commands,
            entity,
            shown(&output, &look, &camera_lens),
            components,
        );
    }

    state.active = active;
    state.settings = effective;
    state.progress = progress;
    state.last = Some(output);
}

/// On a camera other than the player's that shows its parent's settings.
#[derive(Component)]
struct ShowsSceneSettings;

/// Cameras other than the player's: their parent's settings (a texture camera's scene entity),
/// at once, while the camera is active.
fn show_other_cameras(
    mut commands: Commands,
    settings: Query<Ref<PostProcessing>>,
    mut cameras: Query<
        (
            Entity,
            Ref<CameraLook>,
            &Camera,
            &Projection,
            Option<&ChildOf>,
            Has<ShowsSceneSettings>,
            LookComponents,
        ),
        Without<PrimaryCamera>,
    >,
) {
    for (entity, look, camera, projection, parent, was_shown, components) in &mut cameras {
        if !camera.is_active {
            continue;
        }
        let found = parent.and_then(|parent| settings.get(parent.parent()).ok());
        let changed = look.is_changed() || found.as_ref().is_some_and(Ref::is_changed);
        // depth of field follows the target's size, so it is worked out each frame
        let follows_lens = found.as_ref().is_some_and(|found| focus_set(found));
        if !(changed || follows_lens || was_shown != found.is_some()) {
            continue;
        }

        let scene_settings = found.as_deref().copied().unwrap_or_default();
        let camera_lens = lens(camera, projection);
        let output = compose(&scene_settings, &look, &camera_lens, true);
        show(
            &mut commands,
            entity,
            shown(&output, &look, &camera_lens),
            components,
        );
        match (found.is_some(), was_shown) {
            (true, false) => {
                commands.entity(entity).try_insert(ShowsSceneSettings);
            }
            (false, true) => {
                commands.entity(entity).try_remove::<ShowsSceneSettings>();
            }
            _ => (),
        }
    }
}

/// Settings that set depth of field.
fn focus_set(settings: &PostProcessing) -> bool {
    settings.dof_intensity.is_some()
        || settings.dof_focus_start.is_some()
        || settings.dof_focus_end.is_some()
}

#[derive(clap::Parser, ConsoleCommand)]
#[command(name = "/post_processing")]
struct PostProcessingCommand;

fn post_processing_command(
    mut command: ConsoleCommand<PostProcessingCommand>,
    primary: Query<&PrimaryBlend>,
    texture_cameras: Query<(), With<ShowsSceneSettings>>,
) {
    let Some(Ok(_)) = command.take() else { return };
    let Ok(state) = primary.single() else {
        command.reply_failed("no player camera");
        return;
    };
    let s = &state.settings;
    // the depth of field the player's camera shows this frame, blends included
    let applied = state.last.as_ref().and_then(|output| output.dof);
    let reading = serde_json::json!({
        "scene": state.scene.map(|e| e.to_bits().to_string()),
        "virtualCamera": state.virtual_camera.map(|e| e.to_bits().to_string()),
        "blend": state.progress,
        "tonemapping": s.tonemapping.map(|curve| format!("{curve:?}")),
        "exposure": s.exposure,
        "saturation": s.saturation,
        "contrast": s.contrast,
        "hueShift": s.hue_shift,
        "temperature": s.temperature,
        "tint": s.tint,
        "bloomIntensity": s.bloom_intensity,
        "dofIntensity": s.dof_intensity,
        "dofFocusStart": s.dof_focus_start,
        "dofFocusEnd": s.dof_focus_end,
        "appliedFocus": applied.map(|f| format!("{:.1} to {:.1}", f.start, f.end)),
        "appliedBlur": applied.map(|f| f.strength),
        "textureCameras": texture_cameras.iter().count(),
        "unavailable": { "": "No setting for this field; using the explorer's default." },
    });
    command.reply_ok(reading.to_string());
}

#[cfg(test)]
mod tests {
    use super::*;

    fn look() -> CameraLook {
        CameraLook {
            tonemapping: Tonemapping::AcesFitted,
            grading: ColorGrading::default(),
            bloom: Some(Bloom::default()),
            dof: Some(DepthOfField {
                mode: DepthOfFieldMode::Gaussian,
                focal_distance: 60.0,
                aperture_f_stops: 0.05,
                max_depth: 250.0,
                max_circle_of_confusion_diameter: 20.0,
                sensor_height: 0.06,
                linear_ramp: 0.0,
            }),
        }
    }

    fn lens() -> Lens {
        Lens {
            fov: Some(std::f32::consts::FRAC_PI_4),
            height: 1080.0,
        }
    }

    #[test]
    fn no_settings_keep_the_camera_look() {
        let base = look();
        let out = compose(&PostProcessing::default(), &base, &lens(), false);
        assert_eq!(out.curve, Tonemapping::AcesFitted);
        assert_eq!(out.grading.global.untonemapped_mix, 0.0);
        assert_eq!(out.bloom_scale, 1.0);
        assert_eq!(out.dof, None);
    }

    #[test]
    fn no_curve_drops_the_explorer_grading_and_texture_bloom() {
        let mut base = look();
        base.grading.shadows.gain = 1.4;
        let settings = PostProcessing {
            tonemapping: Some(TonemapCurve::None),
            exposure: Some(1.0),
            ..default()
        };
        let out = compose(&settings, &base, &lens(), true);
        assert_eq!(out.grading.shadows.gain, 1.0);
        assert_eq!(out.grading.global.exposure, 1.0);
        assert_eq!(out.grading.global.untonemapped_mix, 1.0);
        assert_eq!(out.bloom_scale, 0.0);
        // the player's camera keeps its bloom
        assert_eq!(compose(&settings, &base, &lens(), false).bloom_scale, 1.0);
    }

    #[test]
    fn focus_distances_a_scene_sets_are_absolute() {
        let at = |start: Option<f32>, end: Option<f32>| {
            let settings = PostProcessing {
                dof_focus_start: start,
                dof_focus_end: end,
                ..default()
            };
            compose(&settings, &look(), &lens(), false).dof.unwrap()
        };
        // in front of the player is allowed
        let focus = at(Some(1.0), None);
        assert_eq!((focus.start, focus.end), (1.0, 250.0));
        let focus = at(Some(0.0), Some(0.0));
        assert_eq!(focus.start, 0.0);
        assert!((focus.end - 0.1).abs() < 1e-6);
        // unset, the start follows the camera's own focus
        assert_eq!(at(None, Some(30.0)).start, 60.0);
    }

    #[test]
    fn player_setting_off_wins_over_scene_depth_of_field() {
        let mut base = look();
        base.dof.as_mut().unwrap().mode = DepthOfFieldMode::Disabled;
        let settings = PostProcessing {
            dof_intensity: Some(1.0),
            ..default()
        };
        assert_eq!(compose(&settings, &base, &lens(), false).dof, None);
    }

    #[test]
    fn temperature_follows_the_daylight_locus() {
        assert_eq!(white_balance(0.0, 0.0), (0.0, 0.0));
        // warm: redder, and greener (lower y) to stay on the locus instead of turning magenta
        let (x, y) = white_balance(1.0, 0.0);
        assert!(x > 0.0 && y < 0.0, "{x} {y}");
        // cool: bluer, and higher y
        let (x, y) = white_balance(-1.0, 0.0);
        assert!(x < 0.0 && y > 0.0, "{x} {y}");
        // tint alone moves only y: positive is magenta
        let (x, y) = white_balance(0.0, 1.0);
        assert!(x == 0.0 && y > 0.0, "{x} {y}");
    }

    #[test]
    fn scene_focus_ramps_to_the_player_setting_unless_it_sets_an_intensity() {
        let base = look();
        let lens = lens();
        let own = Focus::of(base.dof.as_ref().unwrap(), &lens).unwrap();
        let at = |start: f32, end: f32, intensity: Option<f32>| {
            let settings = PostProcessing {
                dof_intensity: intensity,
                dof_focus_start: Some(start),
                dof_focus_end: Some(end),
                ..default()
            };
            compose(&settings, &base, &lens, false).dof.unwrap()
        };
        // the end only says where the blur is reached, not how strong it is
        let near = at(6.0, 10.0, None);
        let far = at(6.0, 250.0, None);
        assert_eq!((near.ramp, far.ramp), (1.0, 1.0));
        assert_eq!(near.strength, own.strength);
        assert_eq!(far.strength, own.strength);
        assert_eq!(at(6.0, 10.0, Some(0.5)).strength, 10.0);

        // an end before the start blurs everything past the start, rather than nothing
        let step = at(6.0, 4.0, None);
        assert!(
            step.end > step.start && step.end - step.start < 0.2,
            "{step:?}"
        );
        let output = Output {
            dof: Some(step),
            ..compose(&PostProcessing::default(), &base, &lens, false)
        };
        let dof = shown(&output, &base, &lens).dof.unwrap();
        assert_eq!(dof.mode, DepthOfFieldMode::Gaussian);
        assert_eq!(dof.linear_ramp, 1.0);
        assert_eq!(dof.max_circle_of_confusion_diameter, own.strength);
    }

    #[test]
    fn depth_of_field_blends_the_blur_seen_and_the_focus_evenly() {
        let base = look();
        let lens = lens();
        let near = compose(
            &PostProcessing {
                dof_intensity: Some(1.0),
                dof_focus_start: Some(5.0),
                dof_focus_end: Some(20.0),
                ..default()
            },
            &base,
            &lens,
            false,
        );
        let far = compose(
            &PostProcessing {
                dof_intensity: Some(0.5),
                dof_focus_start: Some(80.0),
                dof_focus_end: Some(320.0),
                ..default()
            },
            &base,
            &lens,
            false,
        );
        let half = lerp_output(&near, &far, &base, &lens, 0.5).dof.unwrap();
        assert!((half.start - 20.0).abs() < 1e-3, "{half:?}");
        assert!((half.end - 80.0).abs() < 1e-3, "{half:?}");
        assert!((half.strength - 15.0).abs() < 1e-3, "{half:?}");

        // the aperture written gives that blur at the blended end
        let fov = lens.fov.unwrap();
        let output = Output {
            dof: Some(half),
            ..near.clone()
        };
        let dof = shown(&output, &base, &lens).dof.unwrap();
        let seen = circle_of_confusion(
            dof.aperture_f_stops.recip(),
            dof.focal_distance,
            dof.max_depth,
            &dof,
            fov,
            lens.height,
        );
        assert!((seen - 15.0).abs() < 1e-2, "{seen}");
    }

    #[test]
    fn blend_fades_the_curve_out_and_switches_at_the_end() {
        let base = look();
        let lens = lens();
        let from = compose(&PostProcessing::default(), &base, &lens, false);
        let to = compose(
            &PostProcessing {
                tonemapping: Some(TonemapCurve::None),
                ..default()
            },
            &base,
            &lens,
            false,
        );
        let half = lerp_output(&from, &to, &base, &lens, 0.5);
        assert_eq!(half.grading.global.untonemapped_mix, 0.5);

        assert_eq!(
            shown(&half, &base, &lens).tonemapping,
            Tonemapping::AcesFitted
        );
        let end = shown(&to, &base, &lens);
        assert_eq!(end.tonemapping, Tonemapping::None);
        assert_eq!(end.grading.global.untonemapped_mix, 0.0);
    }
}

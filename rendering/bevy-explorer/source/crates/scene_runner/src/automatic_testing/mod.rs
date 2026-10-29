use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc,
};

use bevy::{
    asset::LoadedFolder,
    platform::collections::{HashMap, HashSet},
    prelude::*,
    render::{
        camera::RenderTarget,
        render_resource::{PipelineCache, TextureFormat},
        view::screenshot::{Screenshot, ScreenshotCaptured},
        Render, RenderApp, RenderSet,
    },
    window::{EnabledButtons, WindowLevel, WindowRef, WindowResolution},
};
use common::{
    profile::SerializedProfile,
    rpc::{
        CompareSnapshot, CompareSnapshotResult, RpcCall, RpcCallEvent, RpcOrigin, RpcResultSender,
    },
    sets::SceneSets,
    structs::PrimaryUser,
};
use comms::profile::{CurrentUserProfile, UserProfile};
use dcl_component::transform_and_parent::DclTranslation;
use ipfs::IpfsAssetServer;
use wallet::Wallet;

use crate::{
    initialize_scene::{TestingData, PARCEL_SIZE},
    renderer_context::RendererSceneContext,
    ContainingScene, OutOfWorld, Toaster,
};

pub struct AutomaticTestingPlugin;

impl Plugin for AutomaticTestingPlugin {
    fn build(&self, app: &mut App) {
        let ready = SnapshotRenderReadiness::default();
        app.insert_resource(ready.clone());
        app.init_resource::<SnapshotInProgress>();
        app.sub_app_mut(RenderApp)
            .insert_resource(ready)
            .add_systems(Render, record_ready_frame.in_set(RenderSet::Cleanup));
        app.add_systems(Update, automatic_testing.in_set(SceneSets::PostLoop));
    }
}

/// Hold native video textures while a requested snapshot waits for render readiness
/// and GPU readback. The resource exists only when the scene test harness is enabled.
#[derive(Resource, Default)]
pub struct SnapshotInProgress(pub bool);

// The render thread signals completed frames with no outstanding pipelines.
// Three such frames account for pipelined rendering and allow UI layout to settle.
#[derive(Resource, Clone, Default)]
struct SnapshotRenderReadiness(Arc<AtomicU64>);

fn record_ready_frame(cache: Res<PipelineCache>, ready: Res<SnapshotRenderReadiness>) {
    if cache.waiting_pipelines().next().is_none() {
        ready.0.fetch_add(1, Ordering::Release);
    }
}

struct PendingCapture {
    request: CompareSnapshot,
    window: Entity,
    camera: Entity,
    ready_after: u64,
}

struct SnapshotResult {
    request: CompareSnapshot,
    image: Image,
    window: Entity,
    camera: Entity,
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn automatic_testing(
    mut commands: Commands,
    mut testing_data: ResMut<TestingData>,
    player: Query<(Entity, Option<&OutOfWorld>), With<PrimaryUser>>,
    containing_scene: ContainingScene,
    mut toaster: Toaster,
    mut current_profile: ResMut<CurrentUserProfile>,
    ipfas: IpfsAssetServer,
    scenes: Query<&RendererSceneContext>,
    mut fails: Local<Vec<(String, String, bool)>>,
    mut rpcs: EventReader<RpcCallEvent>,
    mut plans: Local<HashMap<Entity, HashSet<String>>>,
    mut snapshot_in_progress: Local<Option<(Entity, CompareSnapshot, Entity)>>,
    (mut pending_capture, render_ready): (
        Local<Option<PendingCapture>>,
        Res<SnapshotRenderReadiness>,
    ),
    (mut local_sender, mut local_receiver, mut screenshots, mut screenshot_in_progress): (
        Local<Option<tokio::sync::mpsc::Sender<SnapshotResult>>>,
        Local<Option<tokio::sync::mpsc::Receiver<SnapshotResult>>>,
        Local<Handle<LoadedFolder>>,
        ResMut<SnapshotInProgress>,
    ),
    (mut wallet, folders, images): (
        ResMut<Wallet>,
        Res<Assets<LoadedFolder>>,
        Res<Assets<Image>>,
    ),
    ui_roots: Query<(Entity, Option<&mut UiTargetCamera>), (With<ComputedNode>, Without<ChildOf>)>,
) {
    // load screenshots before entering any scenes (to ensure we don't have to async wait later)
    if screenshots.is_weak() {
        *screenshots = ipfas
            .asset_server()
            .load_folder("embedded://images/screenshots");
    }

    match ipfas.asset_server().load_state(screenshots.id()) {
        bevy::asset::LoadState::Loaded => (),
        bevy::asset::LoadState::Loading => {
            debug!("waiting for screenshots");
            return;
        }
        bevy::asset::LoadState::NotLoaded | bevy::asset::LoadState::Failed(_) => {
            panic!("failed to load screenshots");
        }
    }

    // init channels
    if local_sender.is_none() {
        let (sx, rx) = tokio::sync::mpsc::channel(10);
        *local_sender = Some(sx);
        *local_receiver = Some(rx);
    }

    if pending_capture
        .as_ref()
        .is_some_and(|pending| render_ready.0.load(Ordering::Acquire) >= pending.ready_after)
    {
        let pending = pending_capture.take().unwrap();
        let sender = local_sender.as_ref().unwrap().clone();
        commands.spawn(Screenshot::window(pending.window)).observe(
            move |mut trigger: Trigger<ScreenshotCaptured>| {
                let _ = sender.blocking_send(SnapshotResult {
                    request: pending.request.clone(),
                    image: std::mem::take(&mut trigger.0),
                    window: pending.window,
                    camera: pending.camera,
                });
            },
        );
    }

    // Create the camera after the window, then wait for complete render frames
    // before requesting a screenshot. Capturing immediately races UI layout and
    // asynchronous shader compilation, producing black or partially drawn frames.
    if let Some((scene, snapshot, window)) = snapshot_in_progress.take() {
        if let Ok(context) = scenes.get(scene) {
            let base_position =
                Vec3::new(context.base.x as f32, 0.0, -context.base.y as f32) * PARCEL_SIZE;

            let mut cam = |window: Entity, transform: Transform| {
                commands
                    .spawn((
                        Camera3d::default(),
                        transform,
                        Projection::Perspective(PerspectiveProjection {
                            fov: std::f32::consts::PI / 2.0,
                            aspect_ratio: 1.0,
                            near: 0.1,
                            far: 1000.0,
                        }),
                        Camera {
                            target: RenderTarget::Window(WindowRef::Entity(window)),
                            clear_color: ClearColorConfig::Custom(Color::NONE),
                            ..default()
                        },
                    ))
                    .id()
            };

            let snapshot_cam = cam(
                window,
                Transform::from_translation(
                    DclTranslation(snapshot.camera_position).to_bevy_translation() + base_position,
                )
                .looking_at(
                    DclTranslation(snapshot.camera_target).to_bevy_translation() + base_position,
                    Vec3::Y,
                ),
            );

            // set ui to render to the snapshot camera
            for (ent, target) in ui_roots.iter() {
                if target.is_none() {
                    debug!("added {snapshot_cam:?} on {ent:?}");
                    commands
                        .entity(ent)
                        .try_insert(UiTargetCamera(snapshot_cam));
                }
            }

            *pending_capture = Some(PendingCapture {
                request: snapshot,
                window,
                camera: snapshot_cam,
                ready_after: render_ready.0.load(Ordering::Acquire) + 3,
            });
        } else {
            warn!("scene not found for snapshot");
        };
    }

    // process received snapshots
    if let Ok(result) = local_receiver.as_mut().unwrap().try_recv() {
        let name = urlencoding::encode(&result.request.name);
        let screenshots: &LoadedFolder = folders.get(screenshots.id()).unwrap();
        let existing_image = screenshots
            .handles
            .iter()
            .find(|h| {
                let p = h
                    .path()
                    .unwrap()
                    .path()
                    .file_name()
                    .unwrap()
                    .to_string_lossy();
                p == format!("{name}.png")
            })
            .and_then(|h| {
                let img = images.get(h.id().typed());
                img
            });

        let comparison = (|| {
            let captured = snapshot_rgb(&result.image)?;
            let suffix = if existing_image.is_some() {
                "_output"
            } else {
                ""
            };
            captured
                .save_with_format(
                    format!("assets/images/screenshots/{name}{suffix}.png"),
                    image::ImageFormat::Png,
                )
                .map_err(|e| format!("failed to save screenshot: {e}"))?;
            let reference =
                existing_image.ok_or_else(|| "image not found (it has been created)".to_owned())?;
            compute_image_similarity(reference, &result.image)
        })();
        let (similarity, error) = match comparison {
            Ok(similarity) => (similarity, None),
            Err(error) => (0.0, Some(error)),
        };

        result.request.response.send(CompareSnapshotResult {
            error,
            found: existing_image.is_some(),
            similarity,
        });

        commands.entity(result.window).despawn();
        commands.entity(result.camera).despawn();

        // set ui to render to the snapshot camera
        for (ent, target) in ui_roots.iter() {
            if target == Some(&UiTargetCamera(result.camera)) {
                debug!("removed {:?} from {ent:?}", result.camera);
                commands.entity(ent).remove::<UiTargetCamera>();
            } else {
                debug!(
                    "skipping remove from {ent:?}, {:?} != {:?}",
                    result.camera, target
                );
            }
        }

        screenshot_in_progress.0 = false;
    }

    // process events
    for event in rpcs.read() {
        match (&event.origin, &event.call) {
            (RpcOrigin::Scene { entity: scene, .. }, RpcCall::TestPlan { plan }) => {
                plans.insert(*scene, HashSet::from_iter(plan.iter().cloned()));
            }
            (
                RpcOrigin::Scene { entity: scene, .. },
                RpcCall::TestResult {
                    name,
                    success,
                    error,
                },
            ) => {
                let Some(plan) = plans.get_mut(scene) else {
                    warn!("unregistered plan for scene {:?}", scene);
                    continue;
                };

                if !plan.remove(name) {
                    warn!("test {} not registered for scene {:?}", name, scene);
                };

                info!("test {}: {} [{} remaining]", name, success, plan.len());

                if !success {
                    if let Some(location) = scenes.get(*scene).ok().map(|ctx| ctx.base) {
                        if let Some(scene) = testing_data
                            .test_scenes
                            .as_ref()
                            .unwrap()
                            .0
                            .iter()
                            .find(|ts| ts.location == location)
                        {
                            let expected = scene.allow_failures.contains(name);
                            let location = format!("({},{})", location.x, location.y);
                            fails.push((
                                format!("[{location} : {name}]"),
                                error.clone().unwrap_or_default(),
                                expected,
                            ));
                        } else {
                            warn!("location {location} wasn't part of the required set, ignoring this failure");
                        }
                    } else {
                        warn!("scene entity {scene:?} not found(?), ignoring this failure");
                    }
                }
            }
            (RpcOrigin::Scene { entity: scene, .. }, RpcCall::TestSnapshot(snapshot)) => {
                if screenshot_in_progress.0 {
                    snapshot.response.send(CompareSnapshotResult {
                        error: Some("snapshot already in progress".to_owned()),
                        found: false,
                        similarity: 0.0,
                    });
                    continue;
                }
                screenshot_in_progress.0 = true;
                let snapshot_window = commands
                    .spawn(Window {
                        title: "snapshot window".to_owned(),
                        resolution: WindowResolution::new(256.0, 256.0)
                            .with_scale_factor_override(1.0),
                        resizable: false,
                        enabled_buttons: EnabledButtons {
                            minimize: false,
                            maximize: false,
                            close: false,
                        },
                        decorations: false,
                        focused: false,
                        prevent_default_event_handling: true,
                        ime_enabled: false,
                        visible: false,
                        window_level: WindowLevel::AlwaysOnBottom,
                        ..Default::default()
                    })
                    .id();

                *snapshot_in_progress = Some((*scene, snapshot.clone(), snapshot_window));
            }
            _ => (),
        }
    }

    if wallet.address().is_none() {
        wallet.finalize_as_guest();
        current_profile.profile = Some(UserProfile {
            version: 0,
            content: SerializedProfile {
                eth_address: format!("{:#x}", wallet.address().unwrap()),
                user_id: Some(format!("{:#x}", wallet.address().unwrap())),
                ..Default::default()
            },
            base_url: ipfas.ipfs().contents_endpoint().unwrap_or_default(),
        });
        current_profile.is_deployed = true;
        toaster.add_toast(
            "testing login",
            "AUTO TESTING: Automatically logged in as guest",
        );
        return;
    }

    let (player_ent, oow) = player.single().unwrap();

    if oow.is_some() {
        return;
    }

    let Some(next_test_scene) = testing_data.test_scenes.as_ref().unwrap().0.front() else {
        if fails.is_empty() {
            info!("all tests passed!");
            std::process::exit(0);
        } else {
            info!("some tests failed:\n {:#?}", *fails);

            if fails.iter().all(|(_, _, expected)| *expected) {
                info!("all failures were allowed");
                std::process::exit(0);
            } else {
                std::process::exit(1);
            }
        }
    };

    let Some(current_scene) = containing_scene.get(player_ent).into_iter().find(|scene| {
        scenes
            .get(*scene)
            .map(|ctx| ctx.base == next_test_scene.location)
            .unwrap_or(false)
    }) else {
        info!("moving to next scene {:?}", next_test_scene.location);
        let to = next_test_scene.location;
        commands.queue(move |w: &mut World| {
            w.send_event(RpcCallEvent::engine(RpcCall::TeleportPlayer {
                to: Some(to),
                realm: None,
                response: RpcResultSender::default(),
            }));
        });
        return;
    };
    let context = scenes.get(current_scene).unwrap();

    let Some(plan) = plans.get(&current_scene) else {
        warn!("waiting for plan for scene @ {:?}", context.base);
        return;
    };

    if plan.is_empty() {
        info!("plan completed for scene @ {:?}", context.base);
        testing_data.test_scenes.as_mut().unwrap().0.pop_front();
    }
}

fn snapshot_rgb(image: &Image) -> Result<image::RgbImage, String> {
    let channels = match image.texture_descriptor.format {
        TextureFormat::R8Unorm => 1,
        TextureFormat::Rg8Unorm => 2,
        TextureFormat::Rgba8Unorm
        | TextureFormat::Rgba8UnormSrgb
        | TextureFormat::Bgra8Unorm
        | TextureFormat::Bgra8UnormSrgb => 4,
        format => return Err(format!("unsupported screenshot format: {format:?}")),
    };
    let size = image.texture_descriptor.size;
    if size.width == 0 || size.height == 0 || size.depth_or_array_layers != 1 {
        return Err("screenshot must be a nonempty 2D image".to_owned());
    }
    let expected = (size.width as usize)
        .checked_mul(size.height as usize)
        .and_then(|pixels| pixels.checked_mul(channels));
    let data = image
        .data
        .as_ref()
        .ok_or("screenshot has no CPU pixel data")?;
    if expected != Some(data.len()) {
        return Err("screenshot pixel buffer does not match its dimensions".to_owned());
    }
    let mut image = image.clone();
    // Compare encoded channel bytes, as with PNGs; this changes no pixel values.
    if image.texture_descriptor.format == TextureFormat::Rgba8Unorm {
        image.texture_descriptor.format = TextureFormat::Rgba8UnormSrgb;
    }
    image
        .try_into_dynamic()
        .map(|image| image.into_rgb8())
        .map_err(|e| e.to_string())
}

fn compute_image_similarity(img_a: &Image, img_b: &Image) -> Result<f64, String> {
    if img_a.texture_descriptor.size != img_b.texture_descriptor.size {
        return Err("screenshot dimensions differ from the reference".to_owned());
    }
    let a = snapshot_rgb(img_a)?;
    let b = snapshot_rgb(img_b)?;
    // RGB channels only. RGBA buffers must not be walked with a three-byte stride:
    // that mixes alpha into the score and never compares the last quarter of pixels.
    let squared_error: f64 = a
        .as_raw()
        .iter()
        .zip(b.as_raw())
        .map(|(&a, &b)| (f64::from(a) - f64::from(b)).powi(2))
        .sum();
    Ok(
        (1.0 - squared_error / (a.as_raw().len() as f64 * 255.0_f64.powi(2)))
            .max(0.0)
            .sqrt(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::{
        asset::RenderAssetUsages,
        render::render_resource::{Extent3d, TextureDimension},
    };

    fn rgba(pixels: &[u8]) -> Image {
        Image::new(
            Extent3d {
                width: (pixels.len() / 4) as u32,
                height: 1,
                depth_or_array_layers: 1,
            },
            TextureDimension::D2,
            pixels.to_vec(),
            TextureFormat::Rgba8UnormSrgb,
            RenderAssetUsages::all(),
        )
    }

    #[test]
    fn compares_all_pixels_and_ignores_alpha() {
        let black = rgba(&[0, 0, 0, 255, 0, 0, 0, 255]);
        let transparent = rgba(&[0, 0, 0, 0, 0, 0, 0, 0]);
        let last_white = rgba(&[0, 0, 0, 255, 255, 255, 255, 255]);
        assert_eq!(compute_image_similarity(&black, &transparent).unwrap(), 1.0);
        assert!(
            (compute_image_similarity(&black, &last_white).unwrap() - 0.5_f64.sqrt()).abs() < 1e-12
        );
        assert_eq!(
            compute_image_similarity(&rgba(&[0, 0, 0, 255]), &rgba(&[255, 255, 255, 255])).unwrap(),
            0.0
        );
    }

    #[test]
    fn normalizes_bgra_and_rgba_capture_formats() {
        let red = rgba(&[255, 0, 0, 255]);
        let mut bgra = rgba(&[0, 0, 255, 255]);
        bgra.texture_descriptor.format = TextureFormat::Bgra8Unorm;
        assert_eq!(compute_image_similarity(&red, &bgra).unwrap(), 1.0);
        let mut linear = red.clone();
        linear.texture_descriptor.format = TextureFormat::Rgba8Unorm;
        assert_eq!(compute_image_similarity(&red, &linear).unwrap(), 1.0);
    }

    #[test]
    fn rejects_missing_or_malformed_pixels_without_panicking() {
        let valid = rgba(&[255, 0, 0, 255]);
        let mut missing = valid.clone();
        missing.data = None;
        assert!(compute_image_similarity(&missing, &valid)
            .unwrap_err()
            .contains("CPU"));
        missing.data = Some(vec![255]);
        assert!(compute_image_similarity(&valid, &missing)
            .unwrap_err()
            .contains("buffer"));
    }

    #[test]
    fn rejects_different_dimensions_and_empty_images() {
        let valid = rgba(&[0, 0, 0, 255]);
        assert!(compute_image_similarity(&valid, &rgba(&[0; 8])).is_err());
        assert!(compute_image_similarity(&rgba(&[]), &rgba(&[])).is_err());
    }
}

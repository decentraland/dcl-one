//! Baking the landscape's impostors: each prototype that has one is dressed in its surfaces,
//! held still, and photographed from around the upper hemisphere into a `.boimp` file.
//!
//! `/bake_landscape_impostors <out_dir>` runs it in the client; the `bake_landscape_impostors`
//! test runs it in a renderer with no window, which is how the embedded files are made.

use std::{
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::{Duration, Instant, SystemTime},
};

use bevy::prelude::*;
use bevy_console::ConsoleCommand;
use boimp::{
    bake::{BakeState, ImposterBakeBundle, ImposterBakeCamera},
    GridMode,
};
use common::rpc::RpcResultSender;
use console::{DoAddConsoleCommand, PendingConsoleResponses};
use serde_json::json;

use super::{
    add_texture_mips, apply_tree_wind, material::TreeMaterial, prototypes::PROTOTYPES,
    LandscapeTreeAssets, TreeMaterials, TreeWind,
};

pub(crate) const GRID_MODE: GridMode = GridMode::Hemispherical;
/// 8 by 8 views of 64 by 64 texels: a 512 by 512 atlas.
pub(crate) const GRID_SIZE: u32 = 8;
pub(crate) const TILE_SIZE: u32 = 64;
/// Samples per texel along each axis.
pub(crate) const MULTISAMPLE: u32 = 8;
/// The palette's quality budget (RGB error on a 0 to 255 scale), as the scene impostors save
/// theirs.
pub(crate) const PALETTE_RMSE: f32 = 10.0;

/// Where the models stand while they are baked: far below the landscape, apart enough that no
/// view of one reaches another.
const ORIGIN: Vec3 = Vec3::new(0.0, -5000.0, 0.0);
const SPACING: f32 = 200.0;
/// How long a bake may take before it gives up (a headless bake of all five takes seconds).
const DEADLINE: Duration = Duration::from_secs(300);

/// The radius a bake frames: the model's bounds seen from any side.
pub(crate) fn bake_radius(prototype: usize) -> f32 {
    (PROTOTYPES[prototype].size() * 0.5).length()
}

/// The prototypes with an impostor, in prototype order.
pub(crate) fn baked_prototypes() -> impl Iterator<Item = usize> {
    (0..PROTOTYPES.len()).filter(|index| PROTOTYPES[*index].lod.impostor.is_some())
}

/// A bake under way. Insert it to start one; it ends `Finished`.
#[derive(Resource)]
pub(crate) struct ImpostorBake {
    out_dir: PathBuf,
    stage: Stage,
    /// The prototypes and their models, in baking order.
    subjects: Vec<(usize, Entity)>,
    /// The wind as it was before the bake held the trees still.
    wind: bool,
    reply: Option<RpcResultSender<Result<String, String>>>,
    /// When the bake started, on the clock and on the file system's.
    started: Option<(Instant, SystemTime)>,
    /// Why the bake failed.
    error: Option<String>,
    /// Per baked prototype, the height in metres its silhouette covers in the side views.
    pub(crate) heights: Arc<Mutex<Vec<(usize, f32)>>>,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Stage {
    Start,
    Dressing,
    Baking { next: usize, camera: Option<Entity> },
    Finished,
}

impl ImpostorBake {
    pub(crate) fn new(out_dir: PathBuf) -> Self {
        Self {
            out_dir,
            stage: Stage::Start,
            subjects: Vec::new(),
            wind: true,
            reply: None,
            started: None,
            error: None,
            heights: Default::default(),
        }
    }

    #[cfg(test)]
    pub(crate) fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }

    /// End the bake: the models and any bake camera go, the wind comes back, and the reply says
    /// what was written or why nothing was.
    fn finish(
        &mut self,
        commands: &mut Commands,
        wind: &mut TreeWind,
        result: Result<Vec<String>, String>,
    ) {
        for (_, root) in self.subjects.drain(..) {
            commands.entity(root).despawn();
        }
        if let Stage::Baking {
            camera: Some(camera),
            ..
        } = self.stage
        {
            commands.entity(camera).despawn();
        }
        wind.0 = self.wind;
        let reply = match result {
            Ok(written) => {
                info!("landscape impostors baked: {written:?}");
                Ok(json!({ "written": written }).to_string())
            }
            Err(error) => {
                warn!("landscape impostor bake failed: {error}");
                self.error = Some(error.clone());
                Err(error)
            }
        };
        if let Some(sender) = self.reply.take() {
            sender.send(reply);
        }
        self.stage = Stage::Finished;
    }

    /// The files this bake wrote: each must exist, be newer than the bake's start and read as an
    /// archive (boimp's writer only logs its errors).
    fn written(&self) -> Result<Vec<String>, String> {
        let since = self
            .started
            .map_or(SystemTime::UNIX_EPOCH, |(_, since)| since);
        baked_prototypes()
            .map(|prototype| {
                let path = self.path(prototype);
                check_written(&path, since).map(|()| path.display().to_string())
            })
            .collect()
    }

    #[cfg(test)]
    pub(crate) fn stage(&self) -> Stage {
        self.stage
    }

    fn path(&self, prototype: usize) -> PathBuf {
        self.out_dir
            .join(format!("{}.boimp", PROTOTYPES[prototype].asset))
    }
}

/// A file written at or after `since` that reads as a zip archive, as boimp stores an impostor.
pub(crate) fn check_written(path: &Path, since: SystemTime) -> Result<(), String> {
    let fail = |why: String| format!("{}: {why}", path.display());
    let modified = std::fs::metadata(path)
        .and_then(|metadata| metadata.modified())
        .map_err(|error| fail(error.to_string()))?;
    if modified < since {
        return Err(fail("not written by this bake".to_owned()));
    }
    let bytes = std::fs::read(path).map_err(|error| fail(error.to_string()))?;
    if !bytes.starts_with(b"PK\x03\x04") {
        return Err(fail("not an impostor archive".to_owned()));
    }
    Ok(())
}

pub(crate) fn add_bake_command(app: &mut App) {
    app.add_console_command::<BakeCommand, _>(bake_command);
    add_bake_systems(app);
}

pub(crate) fn add_bake_systems(app: &mut App) {
    app.add_systems(
        Update,
        drive_bake.after(add_texture_mips).before(apply_tree_wind),
    );
}

/// Bake the impostors of the landscape's trees and bushes into `<out_dir>/<model>.boimp`.
#[derive(clap::Parser, ConsoleCommand)]
#[command(name = "/bake_landscape_impostors")]
struct BakeCommand {
    out_dir: PathBuf,
}

fn bake_command(
    mut cmd: ConsoleCommand<BakeCommand>,
    mut commands: Commands,
    bake: Option<Res<ImpostorBake>>,
    mut pending: ResMut<PendingConsoleResponses>,
) {
    let Some(Ok(args)) = cmd.take() else { return };
    if bake.is_some_and(|bake| bake.stage != Stage::Finished) {
        cmd.reply_failed("a bake is under way");
        return;
    }
    if let Err(error) = std::fs::create_dir_all(&args.out_dir) {
        cmd.reply_failed(format!("{}: {error}", args.out_dir.display()));
        return;
    }
    let (sender, receiver) = RpcResultSender::channel();
    let mut bake = ImpostorBake::new(args.out_dir);
    bake.reply = Some(sender);
    commands.insert_resource(bake);
    pending.push_receiver(receiver, |result| result, cmd.take_responder());
}

/// The silhouette's height in metres in the views from the side: the rows with any coverage in
/// the tiles whose view is level.
fn side_height(image: &Image, radius: f32) -> f32 {
    let Some(data) = image.data.as_ref() else {
        return 0.0;
    };
    let width = (GRID_SIZE * TILE_SIZE) as usize;
    let mut tallest = 0;
    for y in 0..GRID_SIZE {
        for x in 0..GRID_SIZE {
            let (normal, _) =
                boimp::oct_coords::normal_from_grid(UVec2::new(x, y), GRID_MODE, GRID_SIZE);
            if normal.y.abs() > 1e-3 {
                continue;
            }
            let rows = (0..TILE_SIZE as usize).filter(|row| {
                (0..TILE_SIZE as usize).any(|column| {
                    let texel = (y as usize * TILE_SIZE as usize + row) * width
                        + x as usize * TILE_SIZE as usize
                        + column;
                    let bytes = &data[texel * 8..texel * 8 + 4];
                    let packed = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
                    // alpha: five bits from bit 15
                    (packed >> 15) & 31 > 0
                })
            });
            let (first, last) = rows.fold((usize::MAX, 0), |(first, last), row| {
                (first.min(row), last.max(row))
            });
            if first <= last {
                tallest = tallest.max(last + 1 - first);
            }
        }
    }
    tallest as f32 * radius * 2.0 / TILE_SIZE as f32
}

/// Whether a model's mesh wears its surface's material, and whether it still wears its own.
type Dressing = (
    Has<MeshMaterial3d<TreeMaterial>>,
    Has<MeshMaterial3d<StandardMaterial>>,
);

fn drive_bake(
    mut commands: Commands,
    bake: Option<ResMut<ImpostorBake>>,
    (assets, materials, asset_server): (
        Option<Res<LandscapeTreeAssets>>,
        Option<Res<TreeMaterials>>,
        Res<AssetServer>,
    ),
    mut wind: ResMut<TreeWind>,
    children: Query<&Children>,
    meshes: Query<Dressing, With<Mesh3d>>,
    cameras: Query<&ImposterBakeCamera>,
) {
    let Some(mut bake) = bake else { return };
    let (Some(assets), Some(materials)) = (assets, materials) else {
        return;
    };
    if let Some((started, _)) = bake.started {
        if bake.stage != Stage::Finished && started.elapsed() > DEADLINE {
            let stage = bake.stage;
            bake.finish(
                &mut commands,
                &mut wind,
                Err(format!("no bake after {DEADLINE:?}, at {stage:?}")),
            );
            return;
        }
    }
    match bake.stage {
        Stage::Start => {
            bake.started = Some((Instant::now(), SystemTime::now()));
            // held before anything can end the bake, which puts it back
            bake.wind = wind.0;
            // the files this bake writes replace the old ones, so none can pass for new
            for prototype in baked_prototypes() {
                let path = bake.path(prototype);
                if let Err(error) = std::fs::remove_file(&path) {
                    if error.kind() != std::io::ErrorKind::NotFound {
                        let error = format!("{}: {error}", path.display());
                        bake.finish(&mut commands, &mut wind, Err(error));
                        return;
                    }
                }
            }
            wind.0 = false;
            bake.subjects = baked_prototypes()
                .enumerate()
                .map(|(slot, prototype)| {
                    let entity = commands
                        .spawn((
                            Name::new(format!("impostor bake {}", PROTOTYPES[prototype].name)),
                            SceneRoot(assets.scenes[prototype].clone()),
                            Transform::from_translation(ORIGIN + Vec3::X * SPACING * slot as f32),
                        ))
                        .id();
                    (prototype, entity)
                })
                .collect();
            bake.stage = Stage::Dressing;
        }
        Stage::Dressing => {
            // every model wears its surfaces, and every texture is in with its mips
            let textures = materials
                .textures
                .iter()
                .all(|texture| asset_server.is_loaded_with_dependencies(texture));
            let dressed = bake.subjects.iter().all(|(prototype, root)| {
                let (mut worn, mut bare) = (0, 0);
                for (tree, standard) in meshes.iter_many(children.iter_descendants(*root)) {
                    worn += usize::from(tree);
                    bare += usize::from(standard);
                }
                worn == PROTOTYPES[*prototype].surfaces.len() && bare == 0
            });
            if textures && dressed {
                bake.stage = Stage::Baking {
                    next: 0,
                    camera: None,
                };
            }
        }
        Stage::Baking { next, camera } => {
            if let Some(camera) = camera {
                if cameras
                    .get(camera)
                    .is_ok_and(|camera| camera.state == BakeState::Finished)
                {
                    commands.entity(camera).despawn();
                    bake.stage = Stage::Baking {
                        next: next + 1,
                        camera: None,
                    };
                }
                return;
            }
            let Some(&(prototype, _)) = bake.subjects.get(next) else {
                let written = bake.written();
                bake.finish(&mut commands, &mut wind, written);
                return;
            };
            let slot = Vec3::X * SPACING * next as f32;
            let radius = bake_radius(prototype);
            let mut camera = ImposterBakeCamera {
                radius,
                grid_size: GRID_SIZE,
                tile_size: TILE_SIZE,
                multisample: MULTISAMPLE,
                grid_mode: GRID_MODE,
                ..default()
            };
            let save = camera.save_asset_callback_v2(bake.path(prototype), true, PALETTE_RMSE);
            let heights = bake.heights.clone();
            camera.set_callback(move |image| {
                heights
                    .lock()
                    .unwrap()
                    .push((prototype, side_height(&image, radius)));
                save(image);
            });
            let entity = commands
                .spawn(ImposterBakeBundle {
                    camera,
                    transform: Transform::from_translation(
                        ORIGIN + slot + PROTOTYPES[prototype].centre(),
                    ),
                    ..default()
                })
                .id();
            bake.stage = Stage::Baking {
                next,
                camera: Some(entity),
            };
        }
        Stage::Finished => {}
    }
}

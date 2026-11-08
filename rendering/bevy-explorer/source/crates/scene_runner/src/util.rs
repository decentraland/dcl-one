use std::{
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::Duration,
};

use base64::{prelude::BASE64_URL_SAFE_NO_PAD, Engine};
use bevy::{
    prelude::*,
    tasks::{IoTaskPool, Task},
};
use bevy_console::{ConsoleCommand, PrintConsoleLine};
use common::{
    backoff::Backoff,
    structs::{PreviewCommand, PrimaryUser},
    util::{JoinRelativeExt, TaskExt},
};
use console::DoAddConsoleCommand;
use futures_lite::AsyncReadExt;
use ipfs::{
    ipfs_path::{IpfsPath, IpfsType},
    EntityDefinition, IpfsAssetServer,
};
use multihash_codetable::MultihashDigest;

use crate::{
    initialize_scene::{LiveScenes, PointerResult, ScenePointers, SuperUserScene},
    renderer_context::RendererSceneContext,
    ContainingScene, Toaster,
};

pub struct SceneUtilPlugin;
mod rapid_reload;

impl Plugin for SceneUtilPlugin {
    fn build(&self, app: &mut App) {
        let (send, recv) = tokio::sync::mpsc::unbounded_channel();
        app.insert_resource(ConsoleRelay { send, recv });
        app.add_preview_console_command::<DebugDumpScene, _>(debug_dump_scene);
        app.add_console_command::<ReloadCommand, _>(reload_command);
        rapid_reload::register(app);
        app.add_preview_console_command::<ClearStoreCommand, _>(clear_store_command);
        app.add_systems(Update, (console_relay, handle_preview_command));
    }
}

#[derive(Resource)]
pub struct ConsoleRelay {
    pub send: tokio::sync::mpsc::UnboundedSender<String>,
    recv: tokio::sync::mpsc::UnboundedReceiver<String>,
}

fn console_relay(mut write: EventWriter<PrintConsoleLine>, mut relay: ResMut<ConsoleRelay>) {
    while let Ok(line) = relay.recv.try_recv() {
        write.write(PrintConsoleLine { line });
    }
}

#[derive(clap::Parser, ConsoleCommand)]
#[command(name = "/debug_dump_scene")]
struct DebugDumpScene;

#[allow(clippy::too_many_arguments)]
fn debug_dump_scene(
    mut input: ConsoleCommand<DebugDumpScene>,
    containing_scene: ContainingScene,
    player: Query<Entity, With<PrimaryUser>>,
    scene: Query<&RendererSceneContext>,
    ipfas: IpfsAssetServer,
    scene_definitions: Res<Assets<EntityDefinition>>,
    mut tasks: Local<Vec<Task<()>>>,
    console_relay: Res<ConsoleRelay>,
) {
    if let Some(Ok(_)) = input.take() {
        let scenes = player
            .single()
            .ok()
            .map(|p| containing_scene.get(p))
            .unwrap_or_default()
            .into_iter()
            .flat_map(|s| scene.get(s).ok())
            .collect::<Vec<_>>();

        if scenes.is_empty() {
            input.reply_failed("no scenes");
            return;
        };

        for scene in scenes {
            let h_scene = ipfas.load_hash::<EntityDefinition>(&scene.hash);
            let Some(def) = scene_definitions.get(&h_scene) else {
                input.reply_failed("can't resolve scene handle");
                return;
            };

            if ipfas.ipfs().cache_path().is_none() {
                warn!("no cache");
                return;
            }

            let dump_folder = ipfas
                .ipfs()
                .cache_path()
                .unwrap()
                .to_owned()
                .join("scene_dump")
                .join(&*scene.hash);
            std::fs::create_dir_all(&dump_folder).unwrap();

            // total / succeed / fail
            let count = Arc::new(Mutex::new((0, 0, 0)));

            for content_file in def.content.files() {
                count.lock().unwrap().0 += 1;
                let ipfs_path = IpfsPath::new(IpfsType::new_content_file(
                    scene.hash.to_string(),
                    content_file.to_owned(),
                ));

                let path = PathBuf::from(&ipfs_path);

                let ipfs = ipfas.ipfs().clone();
                let content_file = content_file.clone();
                let dump_folder = dump_folder.clone();
                let count = count.clone();
                let send = console_relay.send.clone();
                tasks.push(IoTaskPool::get().spawn(async move {
                    let report = |fail: Option<String>| {
                        let mut count = count.lock().unwrap();
                        if let Some(fail) = fail {
                            count.2 += 1;
                            let _ = send.send(fail);
                        } else {
                            count.1 += 1;
                        }
                        if count.0 == count.1 + count.2 {
                            if count.2 == 0 {
                                let _ = send.send(format!("[ok] {} files downloaded", count.0));
                            } else {
                                let _ = send.send(format!(
                                    "[failed] {}/{} files downloaded",
                                    count.1, count.0
                                ));
                            }
                        }
                    };

                    let Ok(mut reader) = ipfs.read(&path).await else {
                        report(Some(format!(
                            "{content_file} failed: couldn't load bytes\n"
                        )));
                        return;
                    };
                    let mut bytes = Vec::default();
                    if let Err(e) = reader.read_to_end(&mut bytes).await {
                        report(Some(format!("{content_file} failed: {e}")));
                        return;
                    }

                    // the key is the deployer's string and may still carry `..`
                    let Some(file) = dump_folder.join_relative(&content_file) else {
                        report(Some(format!(
                            "{content_file} failed: escapes the dump folder"
                        )));
                        return;
                    };
                    if let Some(parent) = file.parent() {
                        if let Err(e) = std::fs::create_dir_all(parent) {
                            report(Some(format!(
                                "{content_file} failed: couldn't create parent: {e}"
                            )));
                            return;
                        }
                    }
                    if let Err(e) = std::fs::write(file, bytes) {
                        report(Some(format!("{content_file} failed: {e}")));
                        return;
                    }

                    report(None);
                }));
            }

            input.reply(format!(
                "scene hash {}, downloading {} files",
                scene.hash,
                tasks.len()
            ));
        }
    }

    tasks.retain_mut(|t| t.complete().is_none());
}

#[derive(clap::Parser, ConsoleCommand)]
#[command(name = "/reload")]
struct ReloadCommand {
    hash: Option<String>,
    /// Reload only the non-portable parcel scene under the player.
    #[arg(long, conflicts_with = "hash")]
    current: bool,
}

fn reload_command(
    mut input: ConsoleCommand<ReloadCommand>,
    mut live_scenes: ResMut<LiveScenes>,
    targets: ParcelReloadTarget,
) {
    if let Some(Ok(ReloadCommand { mut hash, current })) = input.take() {
        if current {
            let Some(current) = targets.ready_hash(&live_scenes) else {
                input.reply_failed("No running parcel scene under the player");
                return;
            };
            hash = Some(current.to_owned());
        }
        match hash {
            Some(hash) => {
                if reload_hash(&mut live_scenes, &hash) {
                    input.reply_ok(format!("{hash} reloaded"));
                } else {
                    input.reply_failed(format!("{hash} not found"));
                }
            }
            None => {
                live_scenes.scenes.clear();
                input.reply_ok("all scenes reloaded");
            }
        }
    }
}

#[derive(bevy::ecs::system::SystemParam)]
struct ParcelReloadTarget<'w, 's> {
    pointers: Option<Res<'w, ScenePointers>>,
    player: Query<'w, 's, &'static GlobalTransform, With<PrimaryUser>>,
    scenes: Query<'w, 's, &'static RendererSceneContext, Without<SuperUserScene>>,
}

impl ParcelReloadTarget<'_, '_> {
    fn parcel_hash(&self) -> Option<&str> {
        let player = self.player.single().ok()?;
        match self
            .pointers
            .as_ref()?
            .get(crate::vec3_to_parcel(player.translation()))?
        {
            PointerResult::Exists { hash, .. } => Some(hash),
            PointerResult::Nothing => None,
        }
    }

    fn ready_hash(&self, live: &LiveScenes) -> Option<&str> {
        let hash = self.parcel_hash()?;
        let scene = self.scenes.get(*live.scenes.get(hash)?).ok()?;
        (!scene.is_portable && scene.sender().is_some() && scene.hash.as_ref() == hash)
            .then_some(hash)
    }
}

fn reload_hash(scenes: &mut LiveScenes, hash: &str) -> bool {
    scenes.scenes.remove(hash).is_some()
}

#[derive(clap::Parser, ConsoleCommand)]
#[command(name = "/clear_store")]
struct ClearStoreCommand {
    hash: Option<String>,
}

/// Retries `operation`, waiting 25 ms doubling to 500 ms between attempts, until it
/// succeeds or `deadline` passes.
async fn retry_storage_operation(
    what: &str,
    mut operation: impl FnMut() -> std::io::Result<()>,
    deadline: web_time::Instant,
) -> std::io::Result<()> {
    let mut backoff = Backoff::new(Duration::from_millis(25), Duration::from_millis(500));
    loop {
        match operation() {
            Ok(()) => return Ok(()),
            Err(error) => {
                let remaining = deadline.saturating_duration_since(web_time::Instant::now());
                if remaining.is_zero() {
                    return Err(error);
                }
                debug!("{what} failed, retrying: {error}");
                async_std::task::sleep(backoff.delay().min(remaining)).await;
            }
        }
    }
}

const DELETE_MARK: &str = "_delete_";

fn unix_millis() -> u128 {
    web_time::SystemTime::now()
        .duration_since(web_time::UNIX_EPOCH)
        .map(|since| since.as_millis())
        .unwrap_or_default()
}

fn remove_dir_if_present(path: &Path) -> std::io::Result<()> {
    match std::fs::remove_dir_all(path) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        result => result,
    }
}

/// Removes the renamed stores of earlier runs under `parent`, except `current`, within 10 s.
async fn remove_stale_stores(parent: &Path, file: &str, current: &Path) {
    let Ok(entries) = std::fs::read_dir(parent) else {
        return;
    };
    let prefix = format!("{file}{DELETE_MARK}");
    let deadline = web_time::Instant::now() + Duration::from_secs(10);
    for path in entries.flatten().map(|entry| entry.path()) {
        let is_stale = path != current
            && path
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with(&prefix));
        if !is_stale {
            continue;
        }
        if let Err(error) =
            retry_storage_operation("stale delete", || remove_dir_if_present(&path), deadline).await
        {
            warn!("can't delete stale {path:?}: {error}");
        }
    }
}

fn clear_store_command(
    mut input: ConsoleCommand<ClearStoreCommand>,
    mut live_scenes: ResMut<LiveScenes>,
    contexts: Query<&RendererSceneContext>,
    mut clear_task: Local<Option<Task<()>>>,
) {
    if let Some(mut task) = clear_task.take() {
        if task.complete().is_some() {
            live_scenes.block_new_scenes = false;
        } else {
            *clear_task = Some(task);
            return;
        }
    }

    let mut remove = |parent: &Path, file: &str| {
        let storage_folder = parent.join(file);

        if std::fs::exists(&storage_folder).unwrap_or_default() {
            // a name of this run's own, so a leftover of an earlier run cannot block the clear
            let temp = parent.join(format!("{file}{DELETE_MARK}{}", unix_millis()));
            let parent = parent.to_path_buf();
            let file = file.to_string();
            *clear_task = Some(IoTaskPool::get().spawn(async move {
                let deadline = web_time::Instant::now() + Duration::from_secs(60);
                if let Err(error) = retry_storage_operation(
                    "rename",
                    || std::fs::rename(&storage_folder, &temp),
                    deadline,
                )
                .await
                {
                    error!("can't rename {storage_folder:?} to {temp:?}: {error}");
                    return;
                }
                if let Err(error) =
                    retry_storage_operation("delete", || remove_dir_if_present(&temp), deadline)
                        .await
                {
                    error!("can't delete {temp:?}: {error}");
                }
                // leftovers of earlier runs that failed to delete: best effort, on its own budget
                remove_stale_stores(&parent, &file, &temp).await;
            }));
        }
    };

    if let Some(Ok(ClearStoreCommand { hash })) = input.take() {
        let Some(project_directories) = platform::project_directories() else {
            warn!("not implemented");
            return;
        };

        if let Some(hash) = hash {
            if let Some(ctx) = live_scenes
                .scenes
                .remove(&hash)
                .and_then(|e| contexts.get(e).ok())
            {
                let storage_digest =
                    multihash_codetable::Code::Sha2_256.digest(ctx.storage_root.as_bytes());
                let storage_hash = BASE64_URL_SAFE_NO_PAD.encode(storage_digest.digest());
                remove(
                    &project_directories.data_local_dir().join("LocalStorage"),
                    &storage_hash,
                );
            }
            live_scenes.block_new_scenes = clear_task.is_some();
        } else {
            // all
            live_scenes.scenes.clear();
            remove(project_directories.data_local_dir(), "LocalStorage");
            live_scenes.block_new_scenes = clear_task.is_some();
        }
    }
}

fn handle_preview_command(
    mut events: EventReader<PreviewCommand>,
    mut live_scenes: ResMut<LiveScenes>,
    scenes: Query<&RendererSceneContext>,
    mut toaster: Toaster,
) {
    for command in events.read() {
        match command {
            PreviewCommand::ReloadScene { hash }
            // TODO properly handle model commands
            | PreviewCommand::ReloadModel {
                hash: _,
                src: _,
                scene_id: hash,
            }
            | PreviewCommand::RemoveModel {
                hash: _,
                src: _,
                scene_id: hash,
            } => {
                if let Some(ctx) = live_scenes
                    .scenes
                    .get(hash)
                    .and_then(|e| scenes.get(*e).ok())
                {
                    if ctx.inspected {
                        toaster.add_toast("reload-inspected", "Scene has updated but an inspector is attached. To force the reload type \"/reload\" in the chat window");
                        continue;
                    }
                };
                live_scenes.scenes.remove(hash);
            }
        }
    }
}

#[cfg(test)]
mod storage_retry_tests {
    use std::io::{Error, ErrorKind};

    use super::*;

    fn locked() -> Error {
        Error::new(ErrorKind::PermissionDenied, "locked")
    }

    #[test]
    fn retries_until_the_operation_succeeds() {
        let mut attempts = 0;
        let deadline = web_time::Instant::now() + Duration::from_secs(5);
        let result = async_std::task::block_on(retry_storage_operation(
            "test",
            || {
                attempts += 1;
                if attempts < 3 {
                    Err(locked())
                } else {
                    Ok(())
                }
            },
            deadline,
        ));
        assert!(result.is_ok());
        assert_eq!(attempts, 3);
    }

    #[test]
    fn returns_the_last_error_once_the_deadline_passes() {
        let mut attempts = 0;
        let started = web_time::Instant::now();
        let deadline = started + Duration::from_millis(80);
        let result = async_std::task::block_on(retry_storage_operation(
            "test",
            || {
                attempts += 1;
                Err(locked())
            },
            deadline,
        ));
        assert_eq!(result.unwrap_err().kind(), ErrorKind::PermissionDenied);
        // waits of 25 and 50 ms fit the budget, the next is clipped to what is left
        assert!(attempts >= 3, "{attempts} attempts");
        assert!(started.elapsed() >= Duration::from_millis(80));
    }
}

use std::{future::Future, sync::Arc, time::Duration};

// `ContentMap`'s inner map is bevy_platform's HashMap — use the same type for the map we pass into
// `ContentMap(..)` (the scene-collection merge in /init_asset).
use bevy::platform::collections::HashMap;
use bevy::platform::time::Instant;
use bevy::prelude::*;
use bevy::tasks::IoTaskPool;
use bevy_console::ConsoleCommand;
use common::memory_reclaim::{AddMemoryReclaimer, ReclaimBudget, ReclaimDomains, ReclaimSpec};
use console::{DoAddConsoleCommand, PendingConsoleResponses};
use ipfs::{ContentMap, IpfsIo, IpfsResource};
use platform::AsyncRwLock;

use crate::active_scene::SceneResolver;

// deliberately not base_domain-derived: builder-items is only deployed on decentraland.org
const CATALOG_URL: &str = "https://builder-items.decentraland.org/asset-packs/latest/catalog.json";
const CONTENTS_BASE: &str = "https://builder-items.decentraland.org/contents/";

/// One catalog asset: its slim metadata plus the raw composite and the path→hash content map.
#[derive(Clone)]
pub struct CatalogAsset {
    pub id: String,
    pub name: String,
    pub category: String,
    pub tags: Vec<String>,
    pub pack: String,
    pub composite: serde_json::Value,
    pub contents: HashMap<String, String>,
}

/// Commands hold independent snapshots while the optional cache pin can be reclaimed.
#[derive(Resource, Clone, Default)]
pub struct AssetCatalog(Arc<AsyncRwLock<CatalogCache>>);

#[derive(Default)]
struct CatalogCache {
    assets: Option<Arc<Vec<CatalogAsset>>>,
    last_access: Option<Instant>,
    generation: u64,
}

impl AssetCatalog {
    async fn load(
        &self,
        fetch: impl Future<Output = Result<Vec<CatalogAsset>, String>>,
        refresh: bool,
    ) -> Result<Arc<Vec<CatalogAsset>>, String> {
        let generation = {
            let mut cache = self.0.write().await;
            if !refresh {
                if let Some(assets) = cache.assets.clone() {
                    cache.last_access = Some(Instant::now());
                    return Ok(assets);
                }
            }
            cache.generation += 1;
            cache.generation
        };
        // The browser lock cannot be held over an await. A late fetch must not replace
        // the snapshot from a newer request.
        let assets = Arc::new(fetch.await?);
        let mut cache = self.0.write().await;
        if cache.generation == generation {
            cache.assets = Some(assets.clone());
            cache.last_access = Some(Instant::now());
        }
        Ok(assets)
    }
}

fn reclaim_asset_catalog(catalog: Res<AssetCatalog>, mut budget: ResMut<ReclaimBudget>) {
    let Ok(mut cache) = catalog.0.try_write() else {
        return;
    };
    let entries = usize::from(cache.assets.is_some());
    if let Some(mut turn) = budget.begin("editor_catalog", entries) {
        if turn.inspect() {
            let expired = cache.last_access.is_some_and(|access| {
                Instant::now().saturating_duration_since(access) >= Duration::from_secs(5)
            });
            let released = expired && cache.assets.take().is_some();
            if released {
                turn.released();
            }
            turn.complete_scan(u64::from(released));
        }
        budget.finish_turn(turn, usize::from(cache.assets.is_some()));
    }
}

pub fn add_asset_commands(app: &mut App) {
    app.init_resource::<AssetCatalog>();
    app.add_memory_reclaimer(
        ReclaimSpec::new("editor_catalog", 30, ReclaimDomains::Ram),
        reclaim_asset_catalog,
    );
    app.add_console_command::<AssetCatalogCommand, _>(asset_catalog_cmd);
    app.add_console_command::<InitAssetCommand, _>(init_asset_cmd);
    app.add_console_command::<SceneContentCommand, _>(scene_content_cmd);
}

// --- /scene_content ---

/// List the current scene's content files (content-map paths, sorted) as a JSON array of strings,
/// for the editor's content-file pickers (gltf/audio/video/texture). Includes imported assets
/// (merged into the scene collection). Paths are lowercased, as stored; the editor filters by
/// extension per the field's semantic.
#[derive(clap::Parser, ConsoleCommand)]
#[command(name = "/scene_content")]
struct SceneContentCommand;

fn scene_content_cmd(
    mut input: ConsoleCommand<SceneContentCommand>,
    ipfs: Res<IpfsResource>,
    resolver: SceneResolver,
    mut console_responses: ResMut<PendingConsoleResponses>,
) {
    if let Some(Ok(_)) = input.take() {
        let scene_hash = match resolver.resolve() {
            Ok((_, ctx)) => ctx.hash.clone(),
            Err(e) => {
                input.reply_failed(e);
                return;
            }
        };
        let io = ipfs.inner.clone();
        let (tx, rx) = tokio::sync::oneshot::channel();
        IoTaskPool::get()
            .spawn(async move {
                // refresh from the dev server first so files added to the project outside the
                // editor (the dev server's content map is the whole project glob) are picked up.
                io.refresh_scene_collection(&scene_hash).await;
                let files = io.scene_content_files(&scene_hash).await;
                let _ = tx.send(serde_json::to_string(&files).map_err(|e| e.to_string()));
            })
            .detach();
        console_responses.push_oneshot(rx, |r| r, input.take_responder());
    }
}

async fn fetch_catalog(io: &IpfsIo) -> Result<Vec<CatalogAsset>, String> {
    let resp = io
        .client()
        .get(CATALOG_URL)
        .timeout(std::time::Duration::from_secs(30))
        .send()
        .await
        .map_err(|e| e.to_string())?;
    let json: serde_json::Value = resp
        .error_for_status()
        .map_err(|e| e.to_string())?
        .json()
        .await
        .map_err(|e| e.to_string())?;
    let packs = json
        .get("assetPacks")
        .and_then(|v| v.as_array())
        .ok_or_else(|| "catalog has no assetPacks".to_string())?;
    let mut assets: Vec<CatalogAsset> = Vec::new();
    for pack in packs {
        let pack_name = pack
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        for a in pack
            .get("assets")
            .and_then(|v| v.as_array())
            .into_iter()
            .flatten()
        {
            let id = a
                .get("id")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let contents: HashMap<String, String> = a
                .get("contents")
                .and_then(|v| v.as_object())
                .map(|o| {
                    o.iter()
                        .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string())))
                        .collect()
                })
                .unwrap_or_default();
            assets.push(CatalogAsset {
                id,
                name: a
                    .get("name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
                category: a
                    .get("category")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
                tags: a
                    .get("tags")
                    .and_then(|v| v.as_array())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|t| t.as_str().map(String::from))
                            .collect()
                    })
                    .unwrap_or_default(),
                pack: pack_name.clone(),
                composite: a
                    .get("composite")
                    .cloned()
                    .unwrap_or(serde_json::Value::Null),
                contents,
            });
        }
    }
    Ok(assets)
}

// --- /asset_catalog ---

/// Fetch the asset-packs catalog and return a slim asset index
/// (`[{id,name,category,tags,pack,thumbnail}]`, thumbnail = a CDN url). The full catalog
/// is cached for `/init_asset` and can be reclaimed under RAM pressure.
#[derive(clap::Parser, ConsoleCommand)]
#[command(name = "/asset_catalog")]
struct AssetCatalogCommand;

fn asset_catalog_cmd(
    mut input: ConsoleCommand<AssetCatalogCommand>,
    ipfs: Res<IpfsResource>,
    catalog: Res<AssetCatalog>,
    mut console_responses: ResMut<PendingConsoleResponses>,
) {
    if let Some(Ok(_)) = input.take() {
        let io = ipfs.inner.clone();
        let catalog = catalog.clone();
        let (tx, rx) = tokio::sync::oneshot::channel();
        IoTaskPool::get()
            .spawn(async move {
                let result: Result<String, String> = async {
                    let assets = catalog.load(fetch_catalog(&io), true).await?;
                    let index: Vec<serde_json::Value> = assets
                        .iter()
                        .map(|a| {
                            let thumbnail = a
                                .contents
                                .get("thumbnail.png")
                                .map(|h| format!("{CONTENTS_BASE}{h}"));
                            serde_json::json!({
                                "id": a.id, "name": a.name, "category": a.category,
                                "tags": a.tags, "pack": a.pack, "thumbnail": thumbnail,
                            })
                        })
                        .collect();
                    let reply = serde_json::to_string(&index).map_err(|e| e.to_string())?;
                    Ok(reply)
                }
                .await;
                let _ = tx.send(result);
            })
            .detach();
        console_responses.push_oneshot(rx, |r| r, input.take_responder());
    }
}

// --- /init_asset ---

/// Import a catalog asset into the current scene: fetch+cache each of its files, register them in
/// the scene's content map under `base_dir`, and return the (path-substituted) composite for the
/// editor to instance. Reloads the catalog when its cache has been reclaimed.
///
/// Collision-safety: `base_dir` defaults to `assets/imported/<asset_id>`, so every asset's files
/// occupy a disjoint, per-asset path namespace within the scene's collection — distinct assets
/// never share a path key, and re-importing the same asset re-inserts identical path→hash entries
/// (idempotent). The on-disk byte cache is keyed by content hash, so files shared between assets
/// dedupe rather than collide. (The merge into the scene collection itself is `extend`; the
/// namespace is what keeps it from shadowing the scene's own files.)
#[derive(clap::Parser, ConsoleCommand)]
#[command(name = "/init_asset")]
struct InitAssetCommand {
    /// Catalog asset id
    asset_id: String,
    /// Destination base dir within the scene (default: assets/imported/<asset_id>)
    base_dir: Option<String>,
}

fn init_asset_cmd(
    mut input: ConsoleCommand<InitAssetCommand>,
    ipfs: Res<IpfsResource>,
    catalog: Res<AssetCatalog>,
    resolver: SceneResolver,
    mut console_responses: ResMut<PendingConsoleResponses>,
) {
    if let Some(Ok(cmd)) = input.take() {
        let scene_hash = match resolver.resolve() {
            Ok((_, ctx)) => ctx.hash.clone(),
            Err(e) => {
                input.reply_failed(e);
                return;
            }
        };
        let base_dir = cmd
            .base_dir
            .clone()
            .unwrap_or_else(|| format!("assets/imported/{}", cmd.asset_id));
        let asset_id = cmd.asset_id.clone();
        let client = ipfs.client();
        let io = ipfs.inner.clone();
        let catalog = catalog.clone();
        let (tx, rx) = tokio::sync::oneshot::channel();
        IoTaskPool::get()
            .spawn(async move {
                let result: Result<String, String> = async {
                    let asset = catalog
                        .load(fetch_catalog(&io), false)
                        .await?
                        .iter()
                        .find(|a| a.id == asset_id)
                        .cloned()
                        .ok_or_else(|| format!("asset '{asset_id}' not in catalog"))?;
                    let mut merge: HashMap<String, String> = HashMap::new();
                    let mut written_rels: Vec<String> = Vec::new();
                    let mut errors: Vec<String> = Vec::new();
                    // Scene identity for the web save's project-folder matching (ignored on native).
                    let target = scene_target_json(&io, &scene_hash).await;
                    for (path, hash) in &asset.contents {
                        let rel = format!("{base_dir}/{path}");
                        // Fetch the bytes (the service worker serves them from cache on web) so we
                        // can both seed the native live-render cache AND write the file into the
                        // scene folder right now — the immediate push, so the asset renders without
                        // waiting for a save (and the dev server can serve it on the live merge).
                        let url = format!("{CONTENTS_BASE}{hash}");
                        let fetched = match client
                            .get(&url)
                            .timeout(std::time::Duration::from_secs(60))
                            .build()
                        {
                            Ok(request) => match io
                                .fetch_request(
                                    request,
                                    client.clone(),
                                    std::time::Duration::from_secs(30),
                                    std::time::Duration::from_secs(10),
                                    Some(scene_hash.to_string()),
                                )
                                .await
                            {
                                Ok(response) => Ok(response.body),
                                Err(platform::FetchError::Headers) => {
                                    Err("timed out awaiting headers".to_owned())
                                }
                                Err(platform::FetchError::Stalled) => {
                                    Err("body transfer stalled".to_owned())
                                }
                                Err(platform::FetchError::Status(status, _)) => {
                                    Err(format!("status {status}"))
                                }
                                Err(platform::FetchError::Send(error)) => Err(error.to_string()),
                                Err(platform::FetchError::Body(error)) => Err(error.to_string()),
                            },
                            Err(error) => Err(error.to_string()),
                        };
                        match fetched {
                            Ok(bytes) => {
                                if io.cache_path().is_some()
                                    && ipfs::content_hash::matches_content_hash(hash, &bytes)
                                {
                                    let _ = io.cache_bytes(hash, &bytes).await;
                                }
                                if let Err(e) =
                                    platform::write_scene_file(&scene_hash, &rel, &bytes, &target)
                                        .await
                                {
                                    if errors.len() < 5 {
                                        errors.push(format!("{rel}: {e}"));
                                    }
                                } else {
                                    written_rels.push(rel.clone());
                                }
                            }
                            Err(error) => {
                                if errors.len() < 5 {
                                    errors.push(format!("{rel}: fetch: {error}"));
                                }
                            }
                        }
                        // On a `dcl start` scene the dev server addresses files by a `b64-<path>`
                        // hash, not the source CID, and serves them by that — so map the path to the
                        // hash the dev server will use (else the live load 404s until a reload). On
                        // native/deployed scenes this is None and the source CID (served from the
                        // local cache we seeded above) is used.
                        let merge_hash = io
                            .local_b64_hash_for(&scene_hash, &rel)
                            .await
                            .unwrap_or_else(|| hash.clone());
                        merge.insert(rel.to_lowercase(), merge_hash);
                    }
                    io.merge_collection(&scene_hash, ContentMap(merge)).await;
                    // Wait for the dev server to index the freshly-written files before returning —
                    // the renderer's first load isn't retried on failure, so loading before the file
                    // is served 404s permanently (until a reload). Bounded so a genuinely-unservable
                    // file can't hang the import.
                    if !written_rels.is_empty() {
                        io.await_contents_available(
                            &written_rels,
                            &scene_hash,
                            std::time::Duration::from_secs(10),
                        )
                        .await;
                    }
                    let composite_str =
                        serde_json::to_string(&asset.composite).map_err(|e| e.to_string())?;
                    let composite_str = composite_str.replace("{assetPath}", &base_dir);
                    let composite: serde_json::Value =
                        serde_json::from_str(&composite_str).map_err(|e| e.to_string())?;
                    let reply = serde_json::json!({
                        "baseDir": base_dir, "composite": composite,
                        "written": written_rels.len(), "errors": errors,
                    });
                    serde_json::to_string(&reply).map_err(|e| e.to_string())
                }
                .await;
                let _ = tx.send(result);
            })
            .detach();
        console_responses.push_oneshot(rx, |r| r, input.take_responder());
    }
}

// --- scene target (web save folder matching) ---

/// Build the scene-identity JSON the web save uses to locate + verify the scene's project folder
/// under a granted directory handle: the clean project root (for navigation) plus a fingerprint —
/// `source.projectId` (authoritative), else `scene.parcels` + `display.title` — read from the
/// scene's scene.json metadata. Fields are null/empty on native or when the scene lacks them.
pub async fn scene_target_json(io: &Arc<IpfsIo>, scene_hash: &str) -> String {
    let root = io.local_project_root(scene_hash).await;
    let meta: Option<serde_json::Value> = io
        .scene_metadata(scene_hash)
        .await
        .and_then(|m| serde_json::from_str(&m).ok());
    let str_at = |ptr: &str| {
        meta.as_ref()
            .and_then(|m| m.pointer(ptr))
            .and_then(|v| v.as_str())
            .map(String::from)
    };
    let parcels: Vec<String> = meta
        .as_ref()
        .and_then(|m| m.pointer("/scene/parcels"))
        .and_then(|v| v.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();
    serde_json::json!({
        "hash": scene_hash,
        "root": root,
        "projectId": str_at("/source/projectId"),
        "parcels": parcels,
        "title": str_at("/display/title"),
    })
    .to_string()
}

#[cfg(test)]
mod cache_tests {
    use super::*;
    use common::{
        memory_reclaim::{MemoryReclaimPlugin, MemoryReclaimSettings, ReclaimSimulation},
        structs::PreviewMode,
    };

    fn assets(name: &str) -> Vec<CatalogAsset> {
        vec![CatalogAsset {
            id: "chair".into(),
            name: name.into(),
            category: "furniture".into(),
            tags: vec!["seat".into()],
            pack: "test".into(),
            composite: serde_json::json!({"model": "{assetPath}/chair.glb"}),
            contents: HashMap::from([("chair.glb".into(), "chair-hash".into())]),
        }]
    }

    #[tokio::test]
    async fn pressure_reclaims_catalog_and_import_lookup_reloads_without_invalidating_readers() {
        let catalog = AssetCatalog::default();
        let first = catalog
            .load(async { Ok(assets("first")) }, false)
            .await
            .unwrap();
        let mut app = App::new();
        app.add_plugins(MemoryReclaimPlugin)
            .insert_resource(catalog.clone())
            .insert_resource(PreviewMode {
                is_preview: true,
                ..default()
            })
            .insert_resource(MemoryReclaimSettings {
                frame_budget: Duration::from_secs(10),
                simulation: ReclaimSimulation::Ram,
            })
            .add_memory_reclaimer(
                ReclaimSpec::new("editor_catalog", 30, ReclaimDomains::Ram),
                reclaim_asset_catalog,
            );
        app.finish();
        app.cleanup();
        app.update();
        assert!(
            catalog.0.read().await.assets.is_some(),
            "recent access has a grace period"
        );
        catalog.0.write().await.last_access = Some(Instant::now() - Duration::from_secs(6));
        app.world_mut()
            .resource_mut::<MemoryReclaimSettings>()
            .simulation = ReclaimSimulation::Gpu;
        app.update();
        assert!(
            catalog.0.read().await.assets.is_some(),
            "GPU pressure cannot release RAM metadata"
        );
        app.world_mut()
            .resource_mut::<MemoryReclaimSettings>()
            .simulation = ReclaimSimulation::Ram;
        app.update();
        assert!(catalog.0.read().await.assets.is_none());
        assert_eq!(
            app.world().resource::<ReclaimBudget>().caches()[0].released,
            1
        );
        assert_eq!(
            first[0].contents["chair.glb"], "chair-hash",
            "an in-flight import still owns its data"
        );
        assert!(catalog
            .load(async { Err("offline".into()) }, false)
            .await
            .is_err());
        assert!(
            catalog.0.read().await.assets.is_none(),
            "failed reload remains retryable"
        );
        let reloaded = catalog
            .load(async { Ok(assets("reloaded")) }, false)
            .await
            .unwrap();
        assert_eq!(reloaded[0].name, "reloaded");
        let cached = catalog
            .load(async { panic!("cache hit must not fetch") }, false)
            .await
            .unwrap();
        assert!(Arc::ptr_eq(&cached, &reloaded));
        assert_eq!(first[0].name, "first");
    }

    #[tokio::test]
    async fn late_catalog_refresh_cannot_replace_a_newer_snapshot() {
        let catalog = AssetCatalog::default();
        let (started, ready) = tokio::sync::oneshot::channel();
        let (finish, pending) = tokio::sync::oneshot::channel();
        let older = catalog.clone();
        let task = tokio::spawn(async move {
            older
                .load(
                    async {
                        started.send(()).unwrap();
                        pending.await.unwrap();
                        Ok(assets("older"))
                    },
                    true,
                )
                .await
                .unwrap()
        });
        ready.await.unwrap();
        let newest = catalog
            .load(async { Ok(assets("newest")) }, true)
            .await
            .unwrap();
        finish.send(()).unwrap();
        assert_eq!(task.await.unwrap()[0].name, "older");
        let cached = catalog
            .load(async { panic!("latest snapshot is cached") }, false)
            .await
            .unwrap();
        assert!(Arc::ptr_eq(&cached, &newest));
        assert!(catalog
            .load(async { Err("failed refresh".into()) }, true)
            .await
            .is_err());
        assert_eq!(
            catalog.0.read().await.assets.as_ref().unwrap()[0].name,
            "newest"
        );
    }
}

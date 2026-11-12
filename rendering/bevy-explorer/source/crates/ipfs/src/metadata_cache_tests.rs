use bevy::asset::io::memory::{Dir, MemoryAssetReader};
use common::memory_reclaim::{MemoryReclaimPlugin, MemoryReclaimSettings, ReclaimSimulation};

use super::*;

fn fixture() -> (Arc<IpfsIo>, Dir) {
    let dir = Dir::default();
    let ipfs = Arc::new(IpfsIo::new(
        false,
        Box::new(MemoryAssetReader { root: dir.clone() }),
        Some(PathBuf::from("metadata-fixture-cache")),
        Default::default(),
        1,
        #[cfg(feature = "ipfs_debug")]
        tokio::sync::mpsc::unbounded_channel().0,
    ));
    (ipfs, dir)
}

fn register(ipfs: &IpfsIo, hash: &str, metadata: Option<&str>) {
    ipfs.add_collection(
        hash.to_owned(),
        ContentMap::new_single("model.glb".to_owned(), "model-content".to_owned()),
        Some(IpfsModifier {
            base_url: Some("https://metadata.invalid/contents/".to_owned()),
        }),
        metadata.map(str::to_owned),
    );
}

fn store(dir: &Dir, hash: &str, metadata: serde_json::Value) {
    dir.insert_asset_text(
        &PathBuf::from("metadata-fixture-cache").join(hash),
        &serde_json::json!({
            "id": hash, "pointers": [], "content": [], "metadata": metadata,
        })
        .to_string(),
    );
}

fn expire(ipfs: &IpfsIo) {
    let mut context = ipfs.context.blocking_write();
    for entry in context.entities.values_mut() {
        entry.metadata_accessed =
            MetadataAccess::new(Instant::now() - IDLE_GRACE - Duration::from_secs(1));
    }
    context.reclaim_metadata(Instant::now(), &mut ReclaimTurn::bounded(10, 64));
}

#[test]
fn metadata_reclamation_is_ram_only_and_preserves_routing_and_absence() {
    let (ipfs, _) = fixture();
    register(&ipfs, "stale", Some("{\"name\":\"old\"}"));
    register(&ipfs, "recent", Some("{\"name\":\"active\"}"));
    register(&ipfs, "missing", None);
    ipfs.context
        .blocking_write()
        .entities
        .get_mut("stale")
        .unwrap()
        .metadata_accessed =
        MetadataAccess::new(Instant::now() - IDLE_GRACE - Duration::from_secs(1));
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, MemoryReclaimPlugin))
        .insert_resource(IpfsResource {
            inner: ipfs.clone(),
        })
        .insert_resource(MemoryReclaimSettings {
            simulation: ReclaimSimulation::Gpu,
            frame_budget: Duration::from_secs(1),
        })
        .add_memory_reclaimer(
            ReclaimSpec::new("entity_metadata", 25, ReclaimDomains::Ram),
            reclaim_metadata,
        );
    app.finish();
    app.cleanup();
    app.update();
    assert!(ipfs.context.blocking_read().entities["stale"]
        .metadata
        .is_some());
    app.world_mut()
        .resource_mut::<MemoryReclaimSettings>()
        .simulation = ReclaimSimulation::Ram;
    app.update();
    let context = ipfs.context.blocking_read();
    assert!(context.entities["stale"].metadata.is_none());
    assert!(context.entities["stale"].metadata_evicted);
    assert!(context.entities["recent"].metadata.is_some());
    assert!(
        !context.entities["missing"].metadata_evicted,
        "genuinely absent metadata must never start a reload"
    );
    let path = IpfsPath::new(IpfsType::new_content_file(
        "stale".to_owned(),
        "model.glb".to_owned(),
    ));
    assert_eq!(
        path.to_url(&context).unwrap(),
        "https://metadata.invalid/contents/model-content"
    );
    assert_eq!(
        app.world().resource::<ReclaimBudget>().caches()[0].released,
        1
    );
}

#[test]
fn both_metadata_readers_reload_evicted_json_without_replacing_content_maps() {
    let (ipfs, dir) = fixture();
    let metadata = serde_json::json!({ "name": "fixture", "scene": { "base": "0,0" } });
    register(&ipfs, "scene", Some(&metadata.to_string()));
    store(&dir, "scene", metadata.clone());
    expire(&ipfs);
    let (definition, base) = bevy::tasks::block_on(ipfs.entity_definition("scene")).unwrap();
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&definition.metadata.unwrap()).unwrap(),
        metadata
    );
    assert_eq!(
        definition.collection.hash("model.glb").as_deref(),
        Some("model-content")
    );
    assert_eq!(base, "https://metadata.invalid/contents/");
    expire(&ipfs);
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(
            &bevy::tasks::block_on(ipfs.scene_metadata("scene")).unwrap()
        )
        .unwrap(),
        metadata
    );
    register(&ipfs, "missing", None);
    assert!(bevy::tasks::block_on(ipfs.entity_definition("missing"))
        .unwrap()
        .0
        .metadata
        .is_none());
    assert!(bevy::tasks::block_on(ipfs.scene_metadata("unknown")).is_none());
}

#[cfg(not(target_arch = "wasm32"))]
#[tokio::test]
async fn failed_metadata_reload_remains_retryable() {
    rejected_payload_remains_retryable("invalid-json".to_owned()).await;
}

#[cfg(not(target_arch = "wasm32"))]
async fn rejected_payload_remains_retryable(payload: String) {
    use tokio::{
        io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
        net::TcpListener,
        time::timeout,
    };

    timeout(Duration::from_secs(10), async {
        IoTaskPool::get_or_init(bevy::tasks::TaskPool::new);
        let (ipfs, dir) = fixture();
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let public_url = format!("http://{}", listener.local_addr().unwrap());
        let base_url = format!("{public_url}/contents/");
        let about = ServerAbout {
            content: Some(EndpointConfig {
                healthy: true,
                public_url: public_url.clone(),
            }),
            ..Default::default()
        };
        {
            let mut context = ipfs.context.write().await;
            context.about = Some(about.clone());
            context.register_entity(
                "scene".to_owned(),
                IpfsEntity {
                    collection: ContentMap::new_single(
                        "model.glb".to_owned(),
                        "model-content".to_owned(),
                    ),
                    metadata: Some("{\"name\":\"original\"}".to_owned()),
                },
            );
            context.modifiers.insert(
                "scene".to_owned(),
                IpfsModifier {
                    base_url: Some(base_url.clone()),
                },
            );
            context.entities.get_mut("scene").unwrap().metadata_accessed =
                MetadataAccess::new(Instant::now() - IDLE_GRACE - Duration::from_secs(1));
            context.reclaim_metadata(Instant::now(), &mut ReclaimTurn::bounded(10, 64));
        }
        ipfs.realm_config_sender.send_replace(Some(RealmConfig {
            about_url: format!("{public_url}/about"),
            address: public_url,
            about,
            connected: true,
            secret: None,
            spawn_parcel: None,
        }));
        dir.insert_asset_text(
            &PathBuf::from("metadata-fixture-cache").join("scene"),
            &payload,
        );
        let rejected = payload.clone();
        let server = tokio::spawn(async move {
            let recovered = serde_json::json!({
                "id": "scene", "pointers": [], "content": [],
                "metadata": {"name": "recovered"}
            })
            .to_string();
            for response in [rejected, recovered] {
                let (stream, _) = listener.accept().await.unwrap();
                let mut request = BufReader::new(stream);
                let mut line = String::new();
                request.read_line(&mut line).await.unwrap();
                assert_eq!(line, "GET /contents/scene HTTP/1.1\r\n");
                let mut bypass_cache = false;
                loop {
                    line.clear();
                    assert!(request.read_line(&mut line).await.unwrap() > 0);
                    if line == "\r\n" {
                        break;
                    }
                    if let Some((name, value)) = line.split_once(':') {
                        if name.eq_ignore_ascii_case("cache-control") {
                            bypass_cache = value.trim() == "no-cache";
                        }
                    }
                }
                assert!(bypass_cache, "rejected metadata must bypass HTTP caches");
                request
                    .into_inner()
                    .write_all(
                        format!(
                            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{response}",
                            response.len()
                        )
                        .as_bytes(),
                    )
                    .await
                    .unwrap();
            }
        });

        let (definition, base) = ipfs.entity_definition("scene").await.unwrap();
        assert!(definition.metadata.is_none());
        assert_eq!(definition.collection.0.len(), 1);
        assert_eq!(definition.collection.0["model.glb"], "model-content");
        assert_eq!(base, base_url);
        {
            let context = ipfs.context.read().await;
            assert!(context.entities["scene"].metadata_evicted);
            assert_eq!(context.metadata_retention.resident, 0);
        }

        let metadata = ipfs.scene_metadata("scene").await.unwrap();
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&metadata).unwrap(),
            serde_json::json!({"name": "recovered"})
        );
        server.await.unwrap();
        // Recovery must not depend on rewriting or deleting rejected cached bytes.
        assert_eq!(
            ipfs.read_disk_cache(&ipfs_path::CacheKey::Content("scene".to_owned()))
                .await
                .unwrap()
                .unwrap(),
            payload.as_bytes()
        );
        assert_eq!(ipfs.request_metrics.attempts.load(Ordering::Relaxed), 2);
        let context = ipfs.context.read().await;
        assert!(!context.entities["scene"].metadata_evicted);
        assert_eq!(context.metadata_retention.resident, 1);
        assert_eq!(context.entities["scene"].collection.0["model.glb"], "model-content");
    })
    .await
    .unwrap();
}

#[cfg(not(target_arch = "wasm32"))]
#[tokio::test]
async fn metadata_reload_rejects_another_entity_and_accepts_a_matching_retry() {
    let other = serde_json::json!({
        "id": "another-scene",
        "pointers": [],
        "content": [{"file": "other.glb", "hash": "other-content"}],
        "metadata": {"name": "wrong scene"}
    });
    for payload in [other.clone(), serde_json::json!([other])] {
        rejected_payload_remains_retryable(payload.to_string()).await;
    }
}

#[cfg(not(target_arch = "wasm32"))]
#[tokio::test]
async fn metadata_reload_keeps_empty_null_and_missing_payloads_retryable() {
    for payload in [
        serde_json::json!([]),
        serde_json::json!({"id": "scene", "pointers": [], "content": [], "metadata": null}),
        serde_json::json!({"id": "scene", "pointers": [], "content": []}),
    ] {
        rejected_payload_remains_retryable(payload.to_string()).await;
    }
}

#[test]
fn metadata_reload_accepts_a_hash_addressed_payload_without_an_embedded_id() {
    let (ipfs, dir) = fixture();
    register(&ipfs, "scene", Some("{}"));
    expire(&ipfs);
    dir.insert_asset_text(
        &PathBuf::from("metadata-fixture-cache").join("scene"),
        &serde_json::json!({
            "pointers": [], "content": [], "metadata": {"name": "hash addressed"}
        })
        .to_string(),
    );
    let metadata = bevy::tasks::block_on(ipfs.scene_metadata("scene")).unwrap();
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&metadata).unwrap()["name"],
        "hash addressed"
    );
    assert!(!ipfs.context.blocking_read().entities["scene"].metadata_evicted);
}

#[test]
fn a_resident_metadata_read_does_not_exclude_other_content_readers() {
    use std::{
        future::Future,
        task::{Context, Poll, Waker},
    };
    let (ipfs, _) = fixture();
    register(&ipfs, "scene", Some("{}"));
    let read = ipfs.context.blocking_read();
    let mut lookup = Box::pin(ipfs.entity_definition("scene"));
    assert!(matches!(
        lookup
            .as_mut()
            .poll(&mut Context::from_waker(Waker::noop())),
        Poll::Ready(Some(_))
    ));
    drop(read);
}

#[test]
fn bounded_metadata_scans_reach_idle_entries_while_recent_entries_keep_being_read() {
    let (ipfs, _) = fixture();
    for index in 0..128 {
        register(&ipfs, &format!("{index:03}"), Some("{}"));
    }
    let mut context = ipfs.context.blocking_write();
    context.entities.get_mut("127").unwrap().metadata_accessed =
        MetadataAccess::new(Instant::now() - IDLE_GRACE - Duration::from_secs(1));
    for _ in 0..128 {
        context.entities["000"].metadata_accessed.touch();
        context.reclaim_metadata(Instant::now(), &mut ReclaimTurn::bounded(10, 1));
    }
    assert!(context.entities["127"].metadata_evicted);
    assert!(context.entities["000"].metadata.is_some());
    assert_eq!(context.metadata_retention.resident, 127);
}

#[test]
fn metadata_reclaimer_yields_when_async_content_reader_holds_the_context() {
    let (ipfs, _) = fixture();
    register(&ipfs, "scene", Some("{}"));
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, MemoryReclaimPlugin))
        .insert_resource(IpfsResource {
            inner: ipfs.clone(),
        })
        .insert_resource(MemoryReclaimSettings {
            simulation: ReclaimSimulation::Ram,
            frame_budget: Duration::from_secs(1),
        })
        .add_memory_reclaimer(
            ReclaimSpec::new("entity_metadata", 25, ReclaimDomains::Ram),
            reclaim_metadata,
        );
    app.finish();
    app.cleanup();
    let read = ipfs.context.blocking_read();
    app.update();
    assert_eq!(
        app.world().resource::<ReclaimBudget>().caches()[0].released,
        0
    );
    drop(read);
}

struct ReRegisteringReader {
    inner: MemoryAssetReader,
    replacement: Arc<std::sync::Mutex<Option<Weak<IpfsIo>>>>,
    register: bool,
    pending: bool,
}

impl AssetReader for ReRegisteringReader {
    async fn read<'a>(&'a self, path: &'a Path) -> Result<impl Reader + 'a, AssetReaderError> {
        if self.pending {
            std::future::pending::<()>().await;
        }
        let previous = AssetReader::read(&self.inner, path).await?;
        let replacement = self
            .replacement
            .lock()
            .unwrap()
            .take()
            .and_then(|ipfs| ipfs.upgrade());
        if let Some(ipfs) = replacement {
            let mut context = ipfs.context.write().await;
            if self.register {
                context.register_entity(
                    "scene".to_owned(),
                    IpfsEntity {
                        collection: ContentMap::new_single(
                            "model.glb".to_owned(),
                            "new-model".to_owned(),
                        ),
                        metadata: Some("{\"name\":\"replacement\"}".to_owned()),
                    },
                );
            }
            context.modifiers.insert(
                "scene".to_owned(),
                IpfsModifier {
                    base_url: Some("https://replacement.invalid/".to_owned()),
                },
            );
            store(
                &self.inner.root,
                "scene",
                serde_json::json!({ "name": "replacement" }),
            );
        }
        Ok(previous)
    }

    async fn read_meta<'a>(&'a self, path: &'a Path) -> Result<impl Reader + 'a, AssetReaderError> {
        AssetReader::read_meta(&self.inner, path).await
    }

    async fn read_directory<'a>(
        &'a self,
        path: &'a Path,
    ) -> Result<Box<bevy::asset::io::PathStream>, AssetReaderError> {
        AssetReader::read_directory(&self.inner, path).await
    }

    async fn is_directory<'a>(&'a self, path: &'a Path) -> Result<bool, AssetReaderError> {
        AssetReader::is_directory(&self.inner, path).await
    }
}

#[test]
fn a_late_metadata_reload_cannot_return_an_older_registration_or_source() {
    for register_entity in [true, false] {
        let dir = Dir::default();
        store(&dir, "scene", serde_json::json!({ "name": "outdated" }));
        let replacement = Arc::new(std::sync::Mutex::new(None));
        let ipfs = Arc::new(IpfsIo::new(
            false,
            Box::new(ReRegisteringReader {
                inner: MemoryAssetReader { root: dir },
                replacement: replacement.clone(),
                register: register_entity,
                pending: false,
            }),
            Some(PathBuf::from("metadata-fixture-cache")),
            Default::default(),
            1,
            #[cfg(feature = "ipfs_debug")]
            tokio::sync::mpsc::unbounded_channel().0,
        ));
        register(&ipfs, "scene", Some("{\"name\":\"original\"}"));
        expire(&ipfs);
        *replacement.lock().unwrap() = Some(Arc::downgrade(&ipfs));
        let (definition, base) = bevy::tasks::block_on(ipfs.entity_definition("scene")).unwrap();
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&definition.metadata.unwrap()).unwrap()
                ["name"],
            "replacement"
        );
        assert_eq!(
            definition.collection.hash("model.glb").as_deref(),
            Some(if register_entity {
                "new-model"
            } else {
                "model-content"
            })
        );
        assert_eq!(base, "https://replacement.invalid/");
        assert_eq!(ipfs.context.blocking_read().metadata_retention.resident, 1);
    }
}

#[test]
fn metadata_reload_deadline_returns_routing_and_keeps_the_payload_retryable() {
    let ipfs = Arc::new(IpfsIo::new(
        false,
        Box::new(ReRegisteringReader {
            inner: MemoryAssetReader {
                root: Dir::default(),
            },
            replacement: Arc::new(std::sync::Mutex::new(None)),
            register: false,
            pending: true,
        }),
        Some(PathBuf::from("metadata-fixture-cache")),
        Default::default(),
        1,
        #[cfg(feature = "ipfs_debug")]
        tokio::sync::mpsc::unbounded_channel().0,
    ));
    register(&ipfs, "scene", Some("{}"));
    expire(&ipfs);
    let (definition, base) = bevy::tasks::block_on(entity_definition_with_timeout(
        &ipfs,
        "scene",
        Duration::from_millis(1),
    ))
    .unwrap();
    assert!(definition.metadata.is_none());
    assert_eq!(
        definition.collection.hash("model.glb").as_deref(),
        Some("model-content")
    );
    assert_eq!(base, "https://metadata.invalid/contents/");
    assert!(ipfs.context.blocking_read().entities["scene"].metadata_evicted);
}

use std::{sync::Arc, time::Duration};

use bevy::{
    asset::{
        io::{
            memory::{Dir, MemoryAssetReader},
            AssetSource, AssetSourceId,
        },
        AssetLoader,
    },
    ecs::system::RunSystemOnce,
};
use common::memory_reclaim::{MemoryReclaimPlugin, MemoryReclaimSettings, ReclaimSimulation};
use ipfs::{IpfsIo, IpfsResource};

use super::*;

pub(super) fn metadata_expires_and_reloads<T: CollectibleType>(
    loader: impl AssetLoader<Asset = CollectibleData<T>>,
    metadata: serde_json::Value,
) {
    let urn = CollectibleUrn::<T>::new("urn:decentraland:matic:collections-v2:0xabc:0").unwrap();
    let path = PathBuf::from(&IpfsPath::new(IpfsType::new_content_file(
        "metadata-fixture".to_owned(),
        format!("collectible.{}", T::data_extension()),
    )));
    let dir = Dir::default();
    dir.insert_asset_text(
        &path,
        &serde_json::json!({
            "id": "metadata-fixture",
            "pointers": [urn.as_str()],
            "content": [],
            "metadata": metadata,
        })
        .to_string(),
    );
    let source = dir.clone();
    let mut app = App::new();
    app.register_asset_source(
        AssetSourceId::Default,
        AssetSource::build().with_reader(move || {
            Box::new(MemoryAssetReader {
                root: source.clone(),
            })
        }),
    )
    .add_plugins((
        MinimalPlugins,
        AssetPlugin::default(),
        MemoryReclaimPlugin,
        CollectiblesTypePlugin::<T>::default(),
    ))
    .register_asset_loader(loader)
    .insert_resource(MemoryReclaimSettings {
        frame_budget: Duration::from_secs(1),
        simulation: ReclaimSimulation::Gpu,
    })
    .insert_resource(IpfsResource {
        inner: Arc::new(IpfsIo::new(
            false,
            Box::new(MemoryAssetReader { root: dir }),
            None,
            Default::default(),
            1,
            tokio::sync::mpsc::unbounded_channel().0,
        )),
    });
    app.world_mut()
        .resource_mut::<Collectibles<T>>()
        .record_remote(
            urn.clone(),
            PointerResult::Hash("metadata-fixture".to_owned()),
        );
    app.finish();
    app.cleanup();

    let read = move |mut manager: CollectibleManager<T>| {
        manager.get_data(&urn).map(|data| data.name.clone())
    };
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    loop {
        if let Ok(name) = app.world_mut().run_system_once(read.clone()).unwrap() {
            assert_eq!(name, "Fixture metadata");
            break;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "metadata did not load"
        );
        app.update();
        std::thread::sleep(Duration::from_millis(1));
    }
    let original = app
        .world()
        .resource::<AssetServer>()
        .get_handle::<CollectibleData<T>>(path.clone())
        .unwrap()
        .id();
    for _ in 0..10 {
        app.update();
    }
    assert!(
        app.world()
            .resource::<Assets<CollectibleData<T>>>()
            .contains(original),
        "GPU-only pressure must not evict RAM metadata"
    );

    app.world_mut()
        .resource_mut::<MemoryReclaimSettings>()
        .simulation = ReclaimSimulation::Ram;
    for _ in 0..10 {
        app.update();
    }
    assert!(
        !app.world()
            .resource::<Assets<CollectibleData<T>>>()
            .contains(original),
        "idle metadata must actually leave Bevy Assets after its pin is dropped"
    );
    assert!(matches!(
        app.world_mut().run_system_once(read.clone()).unwrap(),
        Err(CollectibleError::Loading)
    ));
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    loop {
        app.update();
        if let Ok(name) = app.world_mut().run_system_once(read.clone()).unwrap() {
            assert_eq!(name, "Fixture metadata");
            break;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "evicted metadata did not reload"
        );
        std::thread::sleep(Duration::from_millis(1));
    }
    let reloaded = app
        .world()
        .resource::<AssetServer>()
        .get_handle::<CollectibleData<T>>(path)
        .unwrap()
        .id();
    assert_ne!(original, reloaded);
    assert_eq!(
        app.world().resource::<Collectibles<T>>().data_cache.len(),
        1
    );
}

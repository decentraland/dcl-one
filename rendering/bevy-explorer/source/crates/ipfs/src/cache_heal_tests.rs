use super::*;
use crate::{
    content_hash::raw_cid,
    ipfs_path::{IpfsPath, IpfsType},
    ContentMap, EndpointConfig, IpfsEntity, PassThroughReader, RealmConfig, ServerAbout,
};
use bevy::asset::{
    io::{AssetReaderError, AssetSource, AssetSourceId, Reader},
    AssetLoader, LoadContext, UnapprovedPathMode,
};
use std::{path::PathBuf, sync::Arc};

const SCENE: &str = "bafkreiscene";

fn temp_dir(label: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "cache-heal-{label}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir(&dir).unwrap();
    dir
}

fn io_with_cache(label: &str) -> (Arc<IpfsIo>, PathBuf) {
    IoTaskPool::get_or_init(bevy::tasks::TaskPool::new);
    let dir = temp_dir(label);
    let mut io = Arc::new(IpfsIo::new(
        false,
        Box::new(bevy::asset::io::file::FileAssetReader::new("assets")),
        None,
        Default::default(),
        4,
        #[cfg(feature = "ipfs_debug")]
        tokio::sync::mpsc::unbounded_channel().0,
    ));
    Arc::get_mut(&mut io).unwrap().default_fs_path = Some(dir.clone());
    (io, dir)
}

fn content_path(file: &str) -> PathBuf {
    PathBuf::from(&IpfsPath::new(IpfsType::new_content_file(
        SCENE.to_owned(),
        file.to_owned(),
    )))
}

/// A scene with one content file `file` = `hash`, whose content server is `base`.
async fn scene_with(io: &Arc<IpfsIo>, base: &str, file: &str, hash: &str) {
    let about = ServerAbout {
        content: Some(EndpointConfig {
            healthy: true,
            public_url: base.to_owned(),
        }),
        ..Default::default()
    };
    {
        let mut context = io.context.write().await;
        context.about = Some(about.clone());
        context.entities.insert(
            SCENE.to_owned(),
            IpfsEntity {
                collection: ContentMap::new_single(file.to_owned(), hash.to_owned()),
                metadata: None,
            },
        );
    }
    io.realm_config_sender.send_replace(Some(RealmConfig {
        about_url: String::new(),
        address: String::new(),
        about,
        connected: true,
        secret: None,
        spawn_parcel: None,
    }));
}

/// The path of the asset at `url`, and the name of its cache entry.
async fn url_asset(io: &Arc<IpfsIo>, url: &str, ext: &str) -> (PathBuf, String) {
    let path = PathBuf::from(&IpfsPath::new_from_url(url, ext));
    // the engine reads through the path, which names the url as it is written there
    let ipfs_path = IpfsPath::new_from_path(&path).unwrap().unwrap();
    let Some(CacheKey::Url(name)) = ipfs_path.cache_key(&*io.context.read().await) else {
        panic!("`{url}` has no url cache key");
    };
    (path, name)
}

const NOT_FOUND: &[u8] =
    b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n";

/// Serves `routes` (path -> 200 body; anything else 404) and logs every requested path.
async fn serve(routes: Vec<(String, Vec<u8>)>) -> (String, Arc<std::sync::Mutex<Vec<String>>>) {
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let log = Arc::new(std::sync::Mutex::new(Vec::new()));
    let seen = log.clone();
    tokio::spawn(async move {
        while let Ok((socket, _)) = listener.accept().await {
            let routes = routes.clone();
            let seen = seen.clone();
            tokio::spawn(async move {
                let mut request = BufReader::new(socket);
                let mut line = String::new();
                request.read_line(&mut line).await.unwrap();
                let path = line
                    .split_whitespace()
                    .nth(1)
                    .unwrap_or_default()
                    .to_owned();
                loop {
                    let mut header = String::new();
                    if request.read_line(&mut header).await.unwrap() == 0 || header == "\r\n" {
                        break;
                    }
                }
                seen.lock().unwrap().push(path.clone());
                let response = match routes.iter().find(|(route, _)| *route == path) {
                    Some((_, body)) => [
                        format!(
                            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                            body.len()
                        )
                        .into_bytes(),
                        body.clone(),
                    ]
                    .concat(),
                    None => NOT_FOUND.to_vec(),
                };
                request.into_inner().write_all(&response).await.unwrap();
            });
        }
    });
    (base, log)
}

async fn read_all(io: &Arc<IpfsIo>, path: &Path) -> Vec<u8> {
    let mut reader = io.read(path).await.unwrap();
    let mut bytes = Vec::new();
    reader.read_to_end(&mut bytes).await.unwrap();
    bytes
}

#[test]
fn only_a_loaders_error_or_panic_says_the_bytes_are_bad() {
    assert!(says_bad_bytes(&AssetLoadError::AssetLoaderPanic {
        path: "x.png".into(),
        loader_name: "test",
    }));
    assert!(!says_bad_bytes(&AssetLoadError::AssetReaderError(
        AssetReaderError::NotFound("x".into())
    )));
    assert!(!says_bad_bytes(&AssetLoadError::AssetReaderError(
        AssetReaderError::Io(Arc::new(std::io::Error::from(
            std::io::ErrorKind::Interrupted
        )))
    )));
    assert!(!says_bad_bytes(&AssetLoadError::WasmThreadLoaderError));
    assert!(!says_bad_bytes(&AssetLoadError::MissingLabel {
        base_path: "x.glb".into(),
        label: "Scene0".into(),
        all_labels: vec![],
    }));
}

#[derive(Asset, TypePath)]
struct HealTest;

struct FailingLoader;

impl AssetLoader for FailingLoader {
    type Asset = HealTest;
    type Settings = ();
    type Error = std::io::Error;

    async fn load(
        &self,
        reader: &mut dyn Reader,
        _settings: &Self::Settings,
        _load_context: &mut LoadContext<'_>,
    ) -> Result<Self::Asset, Self::Error> {
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes).await?;
        Err(std::io::Error::other(
            String::from_utf8_lossy(&bytes).into_owned(),
        ))
    }

    fn extensions(&self) -> &[&str] {
        &["healtest"]
    }
}

#[derive(Resource, Default)]
struct Verdicts(Vec<(String, bool)>);

fn collect_verdicts(
    mut failed: EventReader<UntypedAssetLoadFailedEvent>,
    mut verdicts: ResMut<Verdicts>,
) {
    for event in failed.read() {
        verdicts
            .0
            .push((event.path.to_string(), says_bad_bytes(&event.error)));
    }
}

#[test]
fn a_loader_rejecting_the_bytes_says_they_are_bad_unless_the_processor_repairs_it() {
    let dir = temp_dir("loader");
    std::fs::write(dir.join("bad.healtest"), b"not a picture").unwrap();
    std::fs::write(
        dir.join("draco.healtest"),
        b"requires KHR_draco_mesh_compression",
    )
    .unwrap();

    let mut app = App::new();
    app.add_plugins((
        MinimalPlugins,
        AssetPlugin {
            file_path: dir.to_string_lossy().into_owned(),
            ..default()
        },
    ))
    .init_asset::<HealTest>()
    .register_asset_loader(FailingLoader)
    .init_resource::<Verdicts>()
    .add_systems(Update, collect_verdicts);
    let server = app.world().resource::<AssetServer>().clone();
    let _handles = [
        server.load::<HealTest>("bad.healtest"),
        server.load::<HealTest>("draco.healtest"),
        server.load::<HealTest>("missing.healtest"),
    ];

    let start = std::time::Instant::now();
    while app.world().resource::<Verdicts>().0.len() < 3 && start.elapsed().as_secs() < 5 {
        app.update();
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    let verdicts = app.world().resource::<Verdicts>().0.clone();
    let verdict_of = |name: &str| {
        verdicts
            .iter()
            .find(|(path, _)| path == name)
            .unwrap_or_else(|| panic!("no failure for {name}: {verdicts:?}"))
            .1
    };
    assert!(verdict_of("bad.healtest"));
    assert!(!verdict_of("draco.healtest"));
    assert!(!verdict_of("missing.healtest"));
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn each_entry_is_checked_once_and_the_session_holds_at_most_1024() {
    let heals = CacheHeals::default();
    assert!(heals.begin("a"));
    assert!(!heals.begin("a"));
    heals.finish("a", true);
    assert_eq!(heals.state("a"), Some(Check::Dropped));
    assert_eq!(heals.state("b"), None);
    assert_eq!(heals.counts(), (1, 1));
    for n in 0..1023 {
        assert!(heals.begin(&format!("name-{n}")));
    }
    assert!(!heals.begin("one-too-many"));
    assert!(!heals.begin("a"));
}

#[tokio::test]
async fn an_entry_under_a_content_hash_is_never_checked() {
    let (io, dir) = io_with_cache("hash-keyed");
    let hash = raw_cid(b"original");
    scene_with(&io, "https://content.invalid", "tex.png", &hash).await;
    std::fs::write(dir.join(&hash), b"processed bytes").unwrap();

    assert!(io
        .drop_bad_cached_entry(&content_path("tex.png"))
        .await
        .is_none());
    assert_eq!(std::fs::read(dir.join(&hash)).unwrap(), b"processed bytes");

    // a content-map name that is no hash names no file the cache wrote, and is left alone too
    let name = "imposters/realms/x/0/0,0.boimp";
    scene_with(&io, "https://content.invalid", "a.png", name).await;
    std::fs::create_dir_all(dir.join("imposters/realms/x/0")).unwrap();
    std::fs::write(dir.join(name), b"imposter bytes").unwrap();
    assert!(io
        .drop_bad_cached_entry(&content_path("a.png"))
        .await
        .is_none());
    assert_eq!(std::fs::read(dir.join(name)).unwrap(), b"imposter bytes");

    assert_eq!(io.cache_heals.counts(), (0, 0));
    std::fs::remove_dir_all(dir).unwrap();
}

#[tokio::test]
async fn a_url_keyed_copy_is_dropped_once_and_downloaded_again() {
    let (io, dir) = io_with_cache("url");
    let (base, log) = serve(vec![("/pic.png".to_owned(), b"a picture".to_vec())]).await;
    // a read waits for a connected realm
    scene_with(&io, &base, "unused.png", "bafkreiunused").await;
    let (path, name) = url_asset(&io, &format!("{base}/pic.png"), "png").await;
    std::fs::write(dir.join(&name), b"<html>not a picture</html>").unwrap();

    assert_eq!(read_all(&io, &path).await, b"<html>not a picture</html>");
    assert!(log.lock().unwrap().is_empty());

    assert_eq!(
        io.drop_bad_cached_entry(&path).await,
        Some(Checked::Dropped(name.clone()))
    );
    assert!(!dir.join(&name).exists());
    assert_eq!(read_all(&io, &path).await, b"a picture");
    let requests = log.lock().unwrap().clone();
    assert_eq!(requests, vec!["/pic.png".to_owned()]);

    // the entry had its check: a later failure on it is that check's
    std::fs::write(dir.join(&name), b"<html>not a picture</html>").unwrap();
    assert_eq!(
        io.drop_bad_cached_entry(&path).await,
        Some(Checked::Elsewhere(name.clone()))
    );
    assert!(dir.join(&name).exists());
    assert_eq!(io.cache_heals.counts(), (1, 1));
    std::fs::remove_dir_all(dir).unwrap();
}

#[tokio::test]
async fn a_url_with_nothing_cached_drops_nothing() {
    let (io, dir) = io_with_cache("empty");
    scene_with(
        &io,
        "https://content.invalid",
        "unused.png",
        "bafkreiunused",
    )
    .await;
    let (path, name) = url_asset(&io, "https://content.invalid/pic.png", "png").await;

    assert_eq!(
        io.drop_bad_cached_entry(&path).await,
        Some(Checked::Kept(name))
    );
    assert_eq!(io.cache_heals.counts(), (1, 0));
    std::fs::remove_dir_all(dir).unwrap();
}

#[tokio::test]
async fn a_loopback_remote_is_never_looked_up_in_the_web_cache() {
    let (io, dir) = io_with_cache("loopback");
    let hash = raw_cid(b"original");
    scene_with(&io, "http://127.0.0.1:9", "tex.png", &hash).await;
    let ipfs_path = IpfsPath::new_from_path(&content_path("tex.png"))
        .unwrap()
        .unwrap();
    assert_eq!(web_lookup_url(&ipfs_path, &*io.context.read().await), None);

    scene_with(&io, "https://content.invalid", "tex.png", &hash).await;
    assert_eq!(
        web_lookup_url(&ipfs_path, &*io.context.read().await),
        Some(format!("https://content.invalid/contents/{hash}"))
    );
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn a_failed_load_drops_a_bad_url_entry_and_a_reader_error_does_not() {
    let (io, dir) = io_with_cache("system");
    bevy::tasks::block_on(scene_with(
        &io,
        "https://content.invalid",
        "unused.png",
        "bafkreiunused",
    ));
    let (path, name) =
        bevy::tasks::block_on(url_asset(&io, "https://content.invalid/pic.png", "png"));
    std::fs::write(dir.join(&name), b"<html>not a picture</html>").unwrap();

    let mut app = App::new();
    app.add_plugins((MinimalPlugins, AssetPlugin::default()))
        .insert_resource(IpfsResource { inner: io.clone() });
    super::CacheHealPlugin.finish(&mut app);

    let event = |error| UntypedAssetLoadFailedEvent {
        id: AssetId::<crate::EntityDefinition>::default().untyped(),
        path: AssetPath::from(path.clone()),
        error,
    };
    app.world_mut()
        .send_event(event(AssetLoadError::AssetReaderError(
            AssetReaderError::NotFound(path.clone()),
        )));
    for _ in 0..20 {
        app.update();
    }
    assert!(dir.join(&name).exists());
    assert_eq!(io.cache_heals.counts(), (0, 0));

    app.world_mut()
        .send_event(event(AssetLoadError::AssetLoaderPanic {
            path: AssetPath::from(path.clone()),
            loader_name: "test",
        }));
    let start = std::time::Instant::now();
    while (dir.join(&name).exists()
        || !app
            .world()
            .resource::<PendingCacheHeals>()
            .checks
            .is_empty())
        && start.elapsed().as_secs() < 5
    {
        app.update();
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    assert!(!dir.join(&name).exists());
    assert_eq!(io.cache_heals.counts(), (1, 1));
    assert!(app
        .world()
        .resource::<PendingCacheHeals>()
        .checks
        .is_empty());
    std::fs::remove_dir_all(dir).unwrap();
}

#[derive(Asset, TypePath)]
struct SettingsTest;

/// Loader settings with a non-default value to look for.
#[derive(Clone, Copy, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
struct Level(u32);

/// Records the bytes and settings of every load; bytes other than `good` fail.
struct RecordingLoader(Arc<std::sync::Mutex<Vec<(Vec<u8>, Level)>>>);

impl AssetLoader for RecordingLoader {
    type Asset = SettingsTest;
    type Settings = Level;
    type Error = std::io::Error;

    async fn load(
        &self,
        reader: &mut dyn Reader,
        settings: &Self::Settings,
        _load_context: &mut LoadContext<'_>,
    ) -> Result<Self::Asset, Self::Error> {
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes).await?;
        self.0.lock().unwrap().push((bytes.clone(), *settings));
        if bytes == b"good" {
            Ok(SettingsTest)
        } else {
            Err(std::io::Error::other("bad bytes"))
        }
    }

    fn extensions(&self) -> &[&str] {
        &["settingstest"]
    }
}

#[tokio::test]
async fn a_healed_asset_loads_again_with_the_settings_it_was_first_loaded_with() {
    let (io, dir) = io_with_cache("settings");
    let (base, log) = serve(vec![("/model.settingstest".to_owned(), b"good".to_vec())]).await;
    scene_with(&io, &base, "unused.png", "bafkreiunused").await;
    let (path, name) = url_asset(&io, &format!("{base}/model.settingstest"), "settingstest").await;
    std::fs::write(dir.join(&name), b"bad").unwrap();

    let loads = Arc::new(std::sync::Mutex::new(Vec::new()));
    let reader = PassThroughReader { inner: io.clone() };
    let mut app = App::new();
    app.register_asset_source(
        AssetSourceId::Default,
        AssetSource::build().with_reader(move || Box::new(reader.clone())),
    )
    .add_plugins((
        MinimalPlugins,
        AssetPlugin {
            unapproved_path_mode: UnapprovedPathMode::Allow,
            ..default()
        },
    ))
    .init_asset::<SettingsTest>()
    .register_asset_loader(RecordingLoader(loads.clone()))
    .insert_resource(IpfsResource { inner: io.clone() });
    super::CacheHealPlugin.finish(&mut app);

    // the requester holds its handle, as a scene's glTF container or material does
    let handle = app
        .world()
        .resource::<AssetServer>()
        .load_with_settings::<SettingsTest, Level>(AssetPath::from(path), |level| {
            *level = Level(7)
        });
    let start = std::time::Instant::now();
    while app
        .world()
        .resource::<Assets<SettingsTest>>()
        .get(&handle)
        .is_none()
        && start.elapsed().as_secs() < 10
    {
        app.update();
        // lets the test server answer
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }

    assert_eq!(
        *loads.lock().unwrap(),
        vec![(b"bad".to_vec(), Level(7)), (b"good".to_vec(), Level(7))]
    );
    assert_eq!(io.cache_heals.counts(), (1, 1));
    assert_eq!(*log.lock().unwrap(), vec!["/model.settingstest".to_owned()]);
    std::fs::remove_dir_all(dir).unwrap();
}

fn sorted(paths: Vec<AssetPath<'static>>) -> Vec<String> {
    let mut names: Vec<String> = paths.iter().map(|path| path.to_string()).collect();
    names.sort();
    names
}

fn asset(file: &str) -> AssetPath<'static> {
    AssetPath::from(content_path(file)).into_owned()
}

fn dropped(name: &str) -> Option<Checked> {
    Some(Checked::Dropped(name.to_owned()))
}

fn elsewhere(name: &str) -> Option<Checked> {
    Some(Checked::Elsewhere(name.to_owned()))
}

#[test]
fn every_asset_that_failed_on_a_dropped_entry_is_reloaded_once() {
    let heals = CacheHeals::default();
    let mut pending = PendingCacheHeals::default();
    let (a, b, c) = (asset("a.png"), asset("b.png"), asset("c.png"));
    assert!(heals.begin("h"));
    // b's failure found a's check under way: it waits for it
    assert!(pending.settle(&heals, b.clone(), elsewhere("h")).is_empty());
    heals.finish("h", true);
    assert_eq!(
        sorted(pending.settle(&heals, a.clone(), dropped("h"))),
        sorted(vec![a.clone(), b.clone()])
    );
    // c's failure found the check finished: the map says it dropped the entry
    assert_eq!(pending.settle(&heals, c.clone(), elsewhere("h")), vec![c]);
    assert!(pending.settle(&heals, a, elsewhere("h")).is_empty());
    assert!(pending.settle(&heals, b, None).is_empty());
}

#[test]
fn assets_that_failed_on_a_kept_entry_are_not_reloaded() {
    let heals = CacheHeals::default();
    let mut pending = PendingCacheHeals::default();
    let (a, b) = (asset("a.png"), asset("b.png"));
    assert!(heals.begin("k"));
    assert!(pending.settle(&heals, b.clone(), elsewhere("k")).is_empty());
    heals.finish("k", false);
    assert!(pending
        .settle(&heals, a, Some(Checked::Kept("k".to_owned())))
        .is_empty());
    assert!(pending.waiting.is_empty());
    assert!(pending.settle(&heals, b, elsewhere("k")).is_empty());
    assert!(pending.waiting.is_empty());
}

#[test]
fn a_session_reloads_at_most_1024_assets() {
    let heals = CacheHeals::default();
    let mut pending = PendingCacheHeals::default();
    for n in 0..MAX_CACHE_HEALS {
        let path = asset(&format!("f{n}.png"));
        assert_eq!(
            pending.settle(&heals, path.clone(), dropped("h")),
            vec![path]
        );
    }
    assert!(pending
        .settle(&heals, asset("one-too-many.png"), dropped("h"))
        .is_empty());
}

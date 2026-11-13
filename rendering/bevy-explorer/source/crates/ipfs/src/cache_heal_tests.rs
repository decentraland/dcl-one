use super::*;
use crate::{
    content_hash::raw_cid,
    ipfs_path::{IpfsPath, IpfsType},
    ContentMap, EndpointConfig, IpfsEntity, RealmConfig, ServerAbout,
};
use bevy::asset::{
    io::{AssetReaderError, Reader},
    AssetLoader, LoadContext,
};
use std::{path::PathBuf, sync::Arc};

const SCENE: &str = "bafkreiscene";

fn io_with_cache(label: &str) -> (Arc<IpfsIo>, PathBuf) {
    IoTaskPool::get_or_init(bevy::tasks::TaskPool::new);
    let dir = std::env::temp_dir().join(format!(
        "cache-heal-{label}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir(&dir).unwrap();
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
    let dir = std::env::temp_dir().join(format!(
        "cache-heal-loader-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir(&dir).unwrap();
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
fn an_entry_filed_under_a_hash_is_kept_while_its_bytes_match() {
    assert!(!holds_other_bytes(
        FiledUnder::Hash(&raw_cid(b"png")),
        b"png"
    ));
    assert!(holds_other_bytes(
        FiledUnder::Hash(&raw_cid(b"png")),
        b"DDS processed"
    ));
    assert!(holds_other_bytes(FiledUnder::Hash("not-a-hash"), b"png"));
    assert!(holds_other_bytes(FiledUnder::Url, b"png"));
}

#[test]
fn each_entry_is_checked_once_and_the_session_holds_at_most_1024() {
    let heals = CacheHeals::default();
    assert!(heals.begin("a"));
    assert!(!heals.begin("a"));
    heals.dropped("a");
    assert!(heals.was_dropped("a"));
    assert!(!heals.was_dropped("b"));
    assert_eq!(heals.counts(), (1, 1));
    for n in 0..1023 {
        assert!(heals.begin(&format!("name-{n}")));
    }
    assert!(!heals.begin("one-too-many"));
    assert!(!heals.begin("a"));
}

#[tokio::test]
async fn a_cached_copy_that_is_not_its_hash_is_dropped_once() {
    let (io, dir) = io_with_cache("processed");
    let hash = raw_cid(b"original");
    scene_with(&io, "https://content.invalid", "tex.png", &hash).await;
    std::fs::write(dir.join(&hash), b"processed bytes").unwrap();

    assert!(io.drop_bad_cached_entry(&content_path("tex.png")).await);
    assert!(!dir.join(&hash).exists());
    assert!(io.cache_entry_dropped(&hash));

    std::fs::write(dir.join(&hash), b"processed bytes").unwrap();
    assert!(!io.drop_bad_cached_entry(&content_path("tex.png")).await);
    assert!(dir.join(&hash).exists());
    assert_eq!(io.cache_heals.counts(), (1, 1));
    std::fs::remove_dir_all(dir).unwrap();
}

#[tokio::test]
async fn a_cached_copy_that_matches_its_hash_is_kept() {
    let (io, dir) = io_with_cache("verified");
    let hash = raw_cid(b"original");
    scene_with(&io, "https://content.invalid", "tex.png", &hash).await;
    std::fs::write(dir.join(&hash), b"original").unwrap();

    assert!(!io.drop_bad_cached_entry(&content_path("tex.png")).await);
    assert_eq!(std::fs::read(dir.join(&hash)).unwrap(), b"original");
    assert!(!io.cache_entry_dropped(&hash));
    assert_eq!(io.cache_heals.counts(), (1, 0));
    std::fs::remove_dir_all(dir).unwrap();
}

#[tokio::test]
async fn a_url_keyed_copy_is_dropped() {
    let (io, dir) = io_with_cache("url");
    let path = PathBuf::from(&IpfsPath::new_from_url(
        "https://example.invalid/pic.png",
        "png",
    ));
    // the engine reads through the path, which names the url as it is written there
    let ipfs_path = IpfsPath::new_from_path(&path).unwrap().unwrap();
    let name = ipfs_path
        .cache_key(&*io.context.read().await)
        .unwrap()
        .name()
        .to_owned();
    std::fs::write(dir.join(&name), b"<html>not a picture</html>").unwrap();

    assert!(io.drop_bad_cached_entry(&path).await);
    assert!(!dir.join(&name).exists());
    std::fs::remove_dir_all(dir).unwrap();
}

#[tokio::test]
async fn nothing_cached_nothing_dropped() {
    let (io, dir) = io_with_cache("empty");
    let hash = raw_cid(b"original");
    scene_with(&io, "https://content.invalid", "tex.png", &hash).await;

    assert!(!io.drop_bad_cached_entry(&content_path("tex.png")).await);
    assert!(!io.cache_entry_dropped(&hash));
    std::fs::remove_dir_all(dir).unwrap();
}

#[tokio::test]
async fn after_a_drop_the_next_read_downloads_the_original() {
    let (io, dir) = io_with_cache("redownload");
    let hash = raw_cid(b"original");
    let route = format!("/contents/{hash}");
    let (base, log) = serve(vec![(route.clone(), b"original".to_vec())]).await;
    scene_with(&io, &base, "tex.png", &hash).await;
    std::fs::write(dir.join(&hash), b"processed bytes").unwrap();
    let path = content_path("tex.png");

    assert_eq!(read_all(&io, &path).await, b"processed bytes");
    assert!(log.lock().unwrap().is_empty());

    assert!(io.drop_bad_cached_entry(&path).await);
    assert_eq!(read_all(&io, &path).await, b"original");
    let requests = log.lock().unwrap().clone();
    assert_eq!(requests, vec![route]);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn a_failed_load_drops_a_bad_cached_copy_and_a_reader_error_does_not() {
    let (io, dir) = io_with_cache("system");
    let hash = raw_cid(b"original");
    bevy::tasks::block_on(scene_with(&io, "https://content.invalid", "tex.png", &hash));
    std::fs::write(dir.join(&hash), b"processed bytes").unwrap();

    let mut app = App::new();
    app.add_plugins((MinimalPlugins, AssetPlugin::default()))
        .insert_resource(IpfsResource { inner: io.clone() });
    super::finish(&mut app);

    let event = |error| UntypedAssetLoadFailedEvent {
        id: AssetId::<crate::EntityDefinition>::default().untyped(),
        path: AssetPath::from(content_path("tex.png")),
        error,
    };
    app.world_mut()
        .send_event(event(AssetLoadError::AssetReaderError(
            AssetReaderError::NotFound(content_path("tex.png")),
        )));
    for _ in 0..20 {
        app.update();
    }
    assert!(dir.join(&hash).exists());
    assert_eq!(io.cache_heals.counts(), (0, 0));

    app.world_mut()
        .send_event(event(AssetLoadError::AssetLoaderPanic {
            path: AssetPath::from(content_path("tex.png")),
            loader_name: "test",
        }));
    let start = std::time::Instant::now();
    while dir.join(&hash).exists() && start.elapsed().as_secs() < 5 {
        app.update();
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    app.update();
    assert!(!dir.join(&hash).exists());
    assert_eq!(io.cache_heals.counts(), (1, 1));
    assert!(app.world().resource::<PendingCacheHeals>().0.is_empty());
    std::fs::remove_dir_all(dir).unwrap();
}

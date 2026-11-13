use super::*;
use crate::{
    content_hash::{raw_cid, sha256_cid_v0},
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
fn an_entry_is_kept_while_its_bytes_match_its_hash_or_its_name_is_no_hash() {
    assert!(!holds_other_bytes(
        FiledUnder::Hash(&raw_cid(b"png")),
        b"png"
    ));
    assert!(holds_other_bytes(
        FiledUnder::Hash(&raw_cid(b"png")),
        b"DDS processed"
    ));
    assert!(!holds_other_bytes(FiledUnder::Hash("not-a-hash"), b"png"));
    assert!(!holds_other_bytes(FiledUnder::Hash("bafkrei/sub"), b"png"));
    assert!(!holds_other_bytes(
        FiledUnder::Hash("imposters/realms/x/0/0,0.boimp"),
        b"png"
    ));
    assert!(holds_other_bytes(FiledUnder::Url, b"png"));
}

#[test]
fn a_hash_in_another_letter_case_never_drops_the_verified_entry() {
    let raw = raw_cid(b"png");
    let flipped = flip_case_after(&raw, 7);
    assert_ne!(flipped, raw);
    assert!(!holds_other_bytes(FiledUnder::Hash(&flipped), b"png"));
    let file_cid_spelling = raw.replacen("bafkrei", "bafKrei", 1);
    assert!(!holds_other_bytes(
        FiledUnder::Hash(&file_cid_spelling),
        b"png"
    ));
    assert!(holds_other_bytes(FiledUnder::Hash(&raw), b"DDS processed"));

    let qm = sha256_cid_v0(b"png");
    let flipped = flip_case_after(&qm, 2);
    assert_ne!(flipped, qm);
    assert!(!holds_other_bytes(FiledUnder::Hash(&flipped), b"png"));
    assert!(!holds_other_bytes(FiledUnder::Hash(&qm), b"png"));
    assert!(holds_other_bytes(FiledUnder::Hash(&qm), b"DDS processed"));
    assert!(holds_other_bytes(
        FiledUnder::Hash(&flipped),
        b"DDS processed"
    ));
}

// flips the case of the first letter at or after `from`
fn flip_case_after(name: &str, from: usize) -> String {
    let at = name
        .char_indices()
        .find(|(i, c)| *i >= from && c.is_ascii_alphabetic())
        .map(|(i, _)| i)
        .unwrap();
    let mut flipped = name.to_owned();
    flipped[at..=at].make_ascii_uppercase();
    if flipped == name {
        flipped[at..=at].make_ascii_lowercase();
    }
    flipped
}

#[test]
fn each_entry_is_checked_once_and_the_session_holds_at_most_1024() {
    let heals = CacheHeals::default();
    assert!(heals.begin("a"));
    assert!(!heals.begin("a"));
    heals.finish("a", true);
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

    assert_eq!(
        io.drop_bad_cached_entry(&content_path("tex.png")).await,
        Some(hash.clone())
    );
    assert!(!dir.join(&hash).exists());
    assert!(io.cache_entry_dropped(&hash));

    std::fs::write(dir.join(&hash), b"processed bytes").unwrap();
    assert_eq!(
        io.drop_bad_cached_entry(&content_path("tex.png")).await,
        Some(hash.clone())
    );
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

    assert_eq!(
        io.drop_bad_cached_entry(&content_path("tex.png")).await,
        Some(hash.clone())
    );
    assert_eq!(std::fs::read(dir.join(&hash)).unwrap(), b"original");
    assert!(!io.cache_entry_dropped(&hash));
    assert_eq!(io.cache_heals.counts(), (1, 0));
    std::fs::remove_dir_all(dir).unwrap();
}

#[tokio::test]
async fn a_name_that_is_no_hash_never_drops_a_cache_file() {
    let (io, dir) = io_with_cache("not-a-hash");
    let name = "imposters/realms/x/0/0,0.boimp";
    scene_with(&io, "https://content.invalid", "a.png", name).await;
    std::fs::create_dir_all(dir.join("imposters/realms/x/0")).unwrap();
    std::fs::write(dir.join(name), b"imposter bytes").unwrap();

    assert_eq!(
        io.drop_bad_cached_entry(&content_path("a.png")).await,
        Some(name.to_owned())
    );
    assert_eq!(std::fs::read(dir.join(name)).unwrap(), b"imposter bytes");
    assert!(!io.cache_entry_dropped(name));
    std::fs::remove_dir_all(dir).unwrap();
}

#[tokio::test]
async fn a_verified_entry_survives_a_hash_spelled_in_another_case() {
    let (io, dir) = io_with_cache("other-case");
    let hash = sha256_cid_v0(b"original");
    let flipped = flip_case_after(&hash, 2);
    assert_ne!(flipped, hash);
    scene_with(&io, "https://content.invalid", "a.png", &flipped).await;
    std::fs::write(dir.join(&flipped), b"original").unwrap();

    io.drop_bad_cached_entry(&content_path("a.png")).await;
    assert_eq!(std::fs::read(dir.join(&flipped)).unwrap(), b"original");
    assert!(!io.cache_entry_dropped(&hash));
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn every_letter_case_of_a_name_shares_one_check() {
    let heals = CacheHeals::default();
    assert!(heals.begin("QmAbC"));
    assert!(!heals.begin("qmabc"));
    heals.finish("QMABC", true);
    assert!(heals.was_dropped("Qmabc"));
    assert_eq!(heals.counts(), (1, 1));
}

#[tokio::test]
async fn a_bad_entry_named_in_two_letter_cases_is_checked_once() {
    let (io, dir) = io_with_cache("two-cases");
    let hash = sha256_cid_v0(b"original");
    let flipped = flip_case_after(&hash, 2);
    let lower = hash.to_ascii_lowercase();
    scene_with(&io, "https://content.invalid", "a.png", &hash).await;
    std::fs::write(dir.join(&hash), b"processed bytes").unwrap();
    assert_eq!(
        io.drop_bad_cached_entry(&content_path("a.png")).await,
        Some(lower.clone())
    );
    assert!(!dir.join(&hash).exists());

    scene_with(&io, "https://content.invalid", "b.png", &flipped).await;
    std::fs::write(dir.join(&flipped), b"processed bytes").unwrap();
    assert_eq!(
        io.drop_bad_cached_entry(&content_path("b.png")).await,
        Some(lower)
    );
    assert!(dir.join(&flipped).exists());
    assert!(io.cache_entry_dropped(&hash));
    assert!(io.cache_entry_dropped(&flipped));
    assert_eq!(io.cache_heals.counts(), (1, 1));
    std::fs::remove_dir_all(dir).unwrap();
}

#[tokio::test]
async fn a_url_keyed_copy_is_dropped() {
    let (io, dir) = io_with_cache("url");
    let (base, log) = serve(vec![("/pic.png".to_owned(), b"a picture".to_vec())]).await;
    // a read waits for a connected realm
    scene_with(&io, &base, "unused.png", "bafkreiunused").await;
    let path = PathBuf::from(&IpfsPath::new_from_url(&format!("{base}/pic.png"), "png"));
    // the engine reads through the path, which names the url as it is written there
    let ipfs_path = IpfsPath::new_from_path(&path).unwrap().unwrap();
    let name = ipfs_path
        .cache_key(&*io.context.read().await)
        .unwrap()
        .name()
        .to_owned();
    std::fs::write(dir.join(&name), b"<html>not a picture</html>").unwrap();

    assert_eq!(read_all(&io, &path).await, b"<html>not a picture</html>");
    assert!(log.lock().unwrap().is_empty());

    assert_eq!(
        io.drop_bad_cached_entry(&path).await,
        Some(name.to_ascii_lowercase())
    );
    assert!(!dir.join(&name).exists());
    assert_eq!(read_all(&io, &path).await, b"a picture");
    let requests = log.lock().unwrap().clone();
    assert_eq!(requests, vec!["/pic.png".to_owned()]);
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

#[tokio::test]
async fn nothing_cached_nothing_dropped() {
    let (io, dir) = io_with_cache("empty");
    let hash = raw_cid(b"original");
    scene_with(&io, "https://content.invalid", "tex.png", &hash).await;

    assert_eq!(
        io.drop_bad_cached_entry(&content_path("tex.png")).await,
        Some(hash.clone())
    );
    assert!(!io.cache_entry_dropped(&hash));
    assert_eq!(io.cache_heals.counts(), (1, 0));
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

    assert!(io.drop_bad_cached_entry(&path).await.is_some());
    assert!(io.cache_entry_dropped(&hash));
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
    while (dir.join(&hash).exists()
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
    assert!(!dir.join(&hash).exists());
    assert_eq!(io.cache_heals.counts(), (1, 1));
    assert!(app
        .world()
        .resource::<PendingCacheHeals>()
        .checks
        .is_empty());
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

#[test]
fn every_asset_that_failed_on_a_dropped_entry_is_reloaded_once() {
    let heals = CacheHeals::default();
    let mut pending = PendingCacheHeals::default();
    let (a, b, c) = (asset("a.png"), asset("b.png"), asset("c.png"));
    assert!(heals.begin("h"));
    assert!(pending
        .settle(&heals, b.clone(), Some("h".to_owned()))
        .is_empty());
    heals.finish("h", true);
    assert_eq!(
        sorted(pending.settle(&heals, a.clone(), Some("h".to_owned()))),
        sorted(vec![a.clone(), b.clone()])
    );
    assert_eq!(
        pending.settle(&heals, c.clone(), Some("h".to_owned())),
        vec![c]
    );
    assert!(pending.settle(&heals, a, Some("h".to_owned())).is_empty());
    assert!(pending.settle(&heals, b, None).is_empty());
}

#[test]
fn assets_that_failed_on_a_kept_entry_are_not_reloaded() {
    let heals = CacheHeals::default();
    let mut pending = PendingCacheHeals::default();
    let (a, b) = (asset("a.png"), asset("b.png"));
    assert!(heals.begin("k"));
    assert!(pending
        .settle(&heals, b.clone(), Some("k".to_owned()))
        .is_empty());
    heals.finish("k", false);
    assert!(pending.settle(&heals, a, Some("k".to_owned())).is_empty());
    assert!(pending.settle(&heals, b, Some("k".to_owned())).is_empty());
    assert!(pending.waiting.is_empty());
}

#[test]
fn a_session_reloads_at_most_1024_assets() {
    let heals = CacheHeals::default();
    let mut pending = PendingCacheHeals::default();
    assert!(heals.begin("h"));
    heals.finish("h", true);
    for n in 0..MAX_CACHE_HEALS {
        let path = asset(&format!("f{n}.png"));
        assert_eq!(
            pending.settle(&heals, path.clone(), Some("h".to_owned())),
            vec![path]
        );
    }
    assert!(pending
        .settle(&heals, asset("one-too-many.png"), Some("h".to_owned()))
        .is_empty());
}

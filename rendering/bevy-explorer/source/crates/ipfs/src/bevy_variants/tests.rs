use super::*;
use crate::{
    ipfs_path::IpfsType, ContentMap, EndpointConfig, IpfsEntity, RealmConfig, ServerAbout,
};
use bevy::asset::io::Reader;

// `i % 251` for 1000 bytes, and its raw-leaf CIDv1 (see `content_hash` tests)
const VARIANT_CID: &str = "bafkreicojquuwmy7piqjti3zx3buxh47ya64i2vumxmzr5gwqpnfgsd6nu";
const OTHER_CID: &str = "bafkreihdwdcefgh4dqkjv67uzcmw7ojee6xedzdetojuzjevtenxquvyku";
const ORIGINAL_CID: &str = "bafkreioriginaltexture";
const SCENE: &str = "bafkreiscene";

fn variant_bytes() -> Vec<u8> {
    (0..1000).map(|i| (i % 251) as u8).collect()
}

fn variant(cid: &str, kind: &str, format: &str) -> Variant {
    Variant {
        cid: cid.to_owned(),
        kind: kind.to_owned(),
        format: format.to_owned(),
        width: None,
        height: None,
        mips: None,
        size: None,
    }
}

#[test]
fn parses_a_v1_manifest() {
    let json = format!(
        r#"{{
            "v": 1,
            "entity": "{SCENE}",
            "generator": "abgen 0.1",
            "unknown": true,
            "files": {{
                "{ORIGINAL_CID}": [
                    {{"cid": "{VARIANT_CID}", "kind": "texture", "format": "dds-bc7",
                      "width": 1024, "height": 1024, "mips": 11, "size": 1000}},
                    {{"cid": "../../etc/passwd", "kind": "texture", "format": "dds-bc1"}},
                    {{"cid": "QmNotRawLeaf", "kind": "texture", "format": "dds-bc1"}}
                ],
                "bafkreionlybadvariants": [
                    {{"cid": "bafkrei/../x", "kind": "model", "format": "glb"}}
                ]
            }}
        }}"#
    );
    let manifest = parse_manifest(json.as_bytes(), SCENE).unwrap();
    assert_eq!(manifest.generator.as_deref(), Some("abgen 0.1"));
    // only the well-formed variant survives, and an original left with none is dropped
    assert_eq!(manifest.files.len(), 1);
    let variants = &manifest.files[ORIGINAL_CID];
    assert_eq!(variants.len(), 1);
    assert_eq!(variants[0].cid, VARIANT_CID);
    assert_eq!(variants[0].width, Some(1024));
    assert_eq!(variants[0].mips, Some(11));
}

#[test]
fn rejects_other_versions_entities_and_garbage() {
    let manifest = |v: u32, entity: &str| {
        format!(r#"{{"v": {v}, "entity": "{entity}", "files": {{}}}}"#)
    };
    assert!(parse_manifest(manifest(1, SCENE).as_bytes(), SCENE).is_ok());
    assert!(parse_manifest(manifest(2, SCENE).as_bytes(), SCENE)
        .unwrap_err()
        .contains("version 2"));
    assert!(parse_manifest(manifest(1, "bafkreiother").as_bytes(), SCENE).is_err());
    assert!(parse_manifest(b"<html>not found</html>", SCENE).is_err());
    assert!(parse_manifest(br#"{"entity": "x"}"#, SCENE).is_err());
}

#[test]
fn selects_the_first_variant_this_client_takes() {
    let variants = [
        variant(OTHER_CID, "texture", "astc-4x4"),
        variant(VARIANT_CID, "texture", "dds-bc7"),
        variant(OTHER_CID, "texture", "dds-bc1"),
    ];
    let none_bad = |_: &str| false;
    assert_eq!(
        select_variant(&variants, "images/a.png", true, none_bad).map(|v| v.cid.as_str()),
        Some(VARIANT_CID)
    );
    // the extension is matched case-insensitively
    assert!(select_variant(&variants, "images/A.JPG", true, none_bad).is_some());
    // no BC on the GPU: nothing
    assert!(select_variant(&variants, "images/a.png", false, none_bad).is_none());
    // a texture never replaces a model, nor a file whose loader would not guess the format
    assert!(select_variant(&variants, "models/a.glb", true, none_bad).is_none());
    assert!(select_variant(&variants, "sounds/a.mp3", true, none_bad).is_none());
    assert!(select_variant(&variants, "noextension", true, none_bad).is_none());
    // a variant that failed this session is passed over for the next supported one
    let fallback = select_variant(&variants, "a.png", true, |cid| cid == VARIANT_CID);
    assert_eq!(fallback.map(|v| v.format.as_str()), Some("dds-bc1"));

    let models = [variant(VARIANT_CID, "model", "glb")];
    assert!(select_variant(&models, "models/a.glb", true, none_bad).is_some());
    assert!(select_variant(&models, "models/a.gltf", true, none_bad).is_some());
    assert!(select_variant(&models, "images/a.png", true, none_bad).is_none());
    // a kind/format pair the schema does not have
    assert!(select_variant(
        &[variant(VARIANT_CID, "texture", "glb")],
        "a.glb",
        true,
        none_bad
    )
    .is_none());
    assert!(could_have_variant("a.KTX2") && !could_have_variant("main.crdt"));
}

#[test]
fn preview_and_local_scenes_are_not_eligible() {
    let remote = Some("https://peer.decentraland.org/content/contents/");
    assert_eq!(scene_eligibility(SCENE, remote, false), Ok(()));
    assert_eq!(scene_eligibility(SCENE, remote, true), Err("preview mode"));
    assert_eq!(
        scene_eligibility("b64-L1VzZXJzL3NjZW5l", remote, false),
        Err("local scene (b64- id)")
    );
    for local in [
        "http://localhost:8000/content/contents/",
        "http://127.0.0.1:8000/contents/",
        "http://[::1]:8000/contents/",
    ] {
        assert_eq!(
            scene_eligibility(SCENE, Some(local), false),
            Err("loopback content server")
        );
    }
    assert_eq!(
        scene_eligibility(SCENE, Some("file:///scenes/x/"), false),
        Err("not an http content server")
    );
    assert_eq!(scene_eligibility(SCENE, None, false), Err("no content server"));
    assert!(scene_eligibility("bafk/../x", remote, false).is_err());
}

fn io_with_cache(label: &str) -> (Arc<IpfsIo>, PathBuf) {
    IoTaskPool::get_or_init(bevy::tasks::TaskPool::new);
    let dir = std::env::temp_dir().join(format!(
        "bevy-variants-{label}-{}-{}",
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

#[tokio::test]
async fn variants_are_off_by_default() {
    let (io, dir) = io_with_cache("off");
    io.request_bevy_variants(SCENE);
    assert!(io.bevy_variants.manifests.lock().unwrap().is_empty());
    let path = content_path("tex.png");
    let ipfs_path = IpfsPath::new_from_path(&path).unwrap().unwrap();
    assert!(io.read_bevy_variant(&ipfs_path, &path).await.is_none());
    std::fs::remove_dir_all(dir).unwrap();
}

#[tokio::test]
async fn preview_and_local_scenes_ask_for_no_manifest() {
    let (mut io, dir) = io_with_cache("preview");
    Arc::get_mut(&mut io).unwrap().is_preview = true;
    io.set_bevy_variants_base("https://abgen.example");
    io.request_bevy_variants(SCENE);
    let mut state = io.bevy_variants.manifest(SCENE).unwrap();
    let state = state
        .wait_for(|state| !matches!(state, ManifestState::Pending))
        .await
        .unwrap()
        .clone();
    assert!(matches!(state, ManifestState::Ineligible("preview mode")));
    std::fs::remove_dir_all(dir).unwrap();
}

fn content_path(file: &str) -> PathBuf {
    PathBuf::from(&IpfsPath::new(IpfsType::new_content_file(
        SCENE.to_owned(),
        file.to_owned(),
    )))
}

const NOT_FOUND: &[u8] =
    b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n";

/// Serves `routes` (path → 200 body; anything else 404) and logs every requested path.
async fn serve(routes: Vec<(String, Vec<u8>)>) -> (String, Arc<Mutex<Vec<String>>>) {
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let log = Arc::new(Mutex::new(Vec::new()));
    let seen = log.clone();
    tokio::spawn(async move {
        while let Ok((socket, _)) = listener.accept().await {
            let routes = routes.clone();
            let seen = seen.clone();
            tokio::spawn(async move {
                let mut request = BufReader::new(socket);
                let mut line = String::new();
                request.read_line(&mut line).await.unwrap();
                let path = line.split_whitespace().nth(1).unwrap_or_default().to_owned();
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

/// A scene with `tex.png` = [`ORIGINAL_CID`], whose manifest offers [`VARIANT_CID`] as BC7,
/// served from `base` (which is also the realm's content server).
async fn scene_with_variant(io: &Arc<IpfsIo>, base: &str) {
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
                collection: ContentMap::new_single("tex.png".to_owned(), ORIGINAL_CID.to_owned()),
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
    io.set_bevy_variants_base(base);
    let _ = io.bevy_variants.bc.set(true);
    let manifest = format!(
        r#"{{"v": 1, "entity": "{SCENE}", "files": {{"{ORIGINAL_CID}": [
            {{"cid": "{VARIANT_CID}", "kind": "texture", "format": "dds-bc7"}}
        ]}}}}"#
    );
    // the scene's own content server is loopback here, which `request_bevy_variants` refuses
    let (sender, receiver) = watch::channel(ManifestState::Loaded(Arc::new(
        parse_manifest(manifest.as_bytes(), SCENE).unwrap(),
    )));
    drop(sender);
    io.bevy_variants
        .manifests
        .lock()
        .unwrap()
        .insert(SCENE.to_owned(), receiver);
}

async fn read_all(io: &Arc<IpfsIo>, path: &Path) -> Vec<u8> {
    let mut reader = io.read(path).await.unwrap();
    let mut bytes = Vec::new();
    reader.read_to_end(&mut bytes).await.unwrap();
    bytes
}

fn requests_for(log: &Mutex<Vec<String>>, path: &str) -> usize {
    log.lock().unwrap().iter().filter(|p| *p == path).count()
}

#[tokio::test]
async fn a_verified_variant_is_served_and_cached_under_its_own_cid() {
    let (io, dir) = io_with_cache("served");
    let variant_route = format!("/bevy/contents/{VARIANT_CID}");
    let original_route = format!("/contents/{ORIGINAL_CID}");
    let (base, log) = serve(vec![
        (variant_route.clone(), variant_bytes()),
        (original_route.clone(), b"original png".to_vec()),
    ])
    .await;
    scene_with_variant(&io, &base).await;
    let path = content_path("tex.png");

    assert_eq!(read_all(&io, &path).await, variant_bytes());
    assert!(io.served_bevy_variant(&path));
    // cached under the variant cid, and nothing under the original's
    assert_eq!(std::fs::read(dir.join(VARIANT_CID)).unwrap(), variant_bytes());
    assert!(!dir.join(ORIGINAL_CID).exists());

    // the second read is the cache's
    assert_eq!(read_all(&io, &path).await, variant_bytes());
    assert_eq!(requests_for(&log, &variant_route), 1);
    assert_eq!(requests_for(&log, &original_route), 0);
    let session = io.bevy_variants.session.lock().unwrap();
    assert_eq!((session.downloaded, session.cached, session.fallbacks), (1, 1, 0));
    drop(session);
    std::fs::remove_dir_all(dir).unwrap();
}

#[tokio::test]
async fn a_variant_that_fails_its_cid_check_falls_back_to_the_original() {
    let (io, dir) = io_with_cache("mismatch");
    let variant_route = format!("/bevy/contents/{VARIANT_CID}");
    let original_route = format!("/contents/{ORIGINAL_CID}");
    let (base, log) = serve(vec![
        (variant_route.clone(), b"tampered".to_vec()),
        (original_route.clone(), b"original png".to_vec()),
    ])
    .await;
    scene_with_variant(&io, &base).await;
    let path = content_path("tex.png");

    assert_eq!(read_all(&io, &path).await, b"original png");
    assert!(!io.served_bevy_variant(&path));
    // the tampered bytes are cached under no key
    assert!(!dir.join(VARIANT_CID).exists());
    assert!(!dir.join(ORIGINAL_CID).exists());
    {
        let session = io.bevy_variants.session.lock().unwrap();
        assert_eq!(session.fallbacks, 1);
        assert!(session.recent[0].reason.contains("do not hash"));
        assert!(session.bad.contains(VARIANT_CID));
    }

    // the bad variant is not asked for again this session
    assert_eq!(read_all(&io, &path).await, b"original png");
    assert_eq!(requests_for(&log, &variant_route), 1);
    assert_eq!(requests_for(&log, &original_route), 2);
    std::fs::remove_dir_all(dir).unwrap();
}

#[tokio::test]
async fn a_missing_variant_falls_back_to_the_original() {
    let (io, dir) = io_with_cache("missing");
    let original_route = format!("/contents/{ORIGINAL_CID}");
    let (base, _log) = serve(vec![(original_route, b"original png".to_vec())]).await;
    scene_with_variant(&io, &base).await;
    let path = content_path("tex.png");
    assert_eq!(read_all(&io, &path).await, b"original png");
    let session = io.bevy_variants.session.lock().unwrap();
    assert_eq!(session.fallbacks, 1);
    assert!(session.recent[0].reason.starts_with("fetch:"));
    drop(session);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn a_variant_that_fails_to_decode_is_marked_bad_once() {
    let variants = BevyVariants::default();
    let path = content_path("tex.png");
    variants.served(
        &path,
        ServedVariant {
            scene: SCENE.to_owned(),
            file: "tex.png".to_owned(),
            cid: VARIANT_CID.to_owned(),
            format: "dds-bc7".to_owned(),
        },
        false,
    );
    assert!(variants.is_served(&path));
    assert!(variants.decode_failed(&path, "unsupported texture format"));
    assert!(!variants.is_served(&path));
    assert!(variants.is_bad(VARIANT_CID));
    // an asset that was never a variant is not reloaded
    assert!(!variants.decode_failed(&path, "again"));
    assert!(!variants.decode_failed(&content_path("other.png"), "x"));
}

#[test]
fn repeated_transport_failures_turn_variants_off_for_the_session() {
    let variants = BevyVariants::default();
    for _ in 1..BREAKER_FAILURES {
        variants.transport_failed("connection refused");
    }
    variants.transport_ok();
    for _ in 1..BREAKER_FAILURES {
        variants.transport_failed("connection refused");
    }
    assert!(variants.disabled().is_none());
    variants.transport_failed("connection refused");
    assert!(variants.disabled().unwrap().contains("connection refused"));
}

#[test]
fn the_debug_snapshot_reports_state_and_fallbacks() {
    let variants = BevyVariants::default();
    assert_eq!(snapshot_of(&variants)["state"], "off");
    variants.set_base("http://localhost:5147/");
    let _ = variants.bc.set(true);
    variants.fail(SCENE, "tex.png", Some(VARIANT_CID), "fetch: refused".to_owned());
    let snapshot = snapshot_of(&variants);
    assert_eq!(snapshot["base"], "http://localhost:5147");
    assert_eq!(snapshot["state"], "on");
    assert_eq!(snapshot["fallbacks"], 1);
    assert!(snapshot["recentFallbacks"]
        .as_str()
        .unwrap()
        .contains("tex.png"));
}

fn snapshot_of(variants: &BevyVariants) -> serde_json::Value {
    super::debug_panel::snapshot(variants)
}

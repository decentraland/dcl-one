use super::*;
use crate::{
    ipfs_path::IpfsType, ContentMap, EndpointConfig, IpfsEntity, RealmConfig, ServerAbout,
};
use bevy::asset::io::Reader;
use common::sync::MutexExt;

/// A 2-level BC7 KTX2 variant and its CID.
fn bc7_variant() -> (Vec<u8>, String) {
    let (file, _, _) = super::ktx2::tests::bc7_two_levels();
    let cid = crate::content_hash::raw_cid(&file);
    (file, cid)
}

// `i % 251` for 1000 bytes (no KTX2), and its raw-leaf CIDv1 (see `content_hash` tests)
const VARIANT_CID: &str = "bafkreicojquuwmy7piqjti3zx3buxh47ya64i2vumxmzr5gwqpnfgsd6nu";
const OTHER_CID: &str = "bafkreihdwdcefgh4dqkjv67uzcmw7ojee6xedzdetojuzjevtenxquvyku";
// a name with the form of a raw-leaf CIDv1
const ORIGINAL_CID: &str = "bafkreioriginaltextureaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const SCENE: &str = "bafkreiscene";

fn variant_bytes() -> Vec<u8> {
    (0..1000).map(|i| (i % 251) as u8).collect()
}

/// The path abgen would serve a variant of `format` at, from an id (any valid one will do here);
/// its prefix letter is the class.
fn served_path_of(format: &str, id: &str) -> String {
    let prefix = match format {
        "glb-br" => "m2",
        "glb" => "g4",
        _ => "t2",
    };
    format!("bevy/{prefix}/{ORIGINAL_CID}_{}", &id[id.len() - 6..])
}

fn served_path(id: &str) -> String {
    served_path_of("ktx2-bc", id)
}

fn variant(id: &str, kind: &str, format: &str) -> ContentFileVariant {
    ContentFileVariant {
        path: served_path_of(format, id),
        kind: kind.to_owned(),
        format: format.to_owned(),
    }
}

/// The cache file name of a variant fetched from `base`: a digest of its url.
fn cache_name(base: &str, path: &str) -> String {
    crate::content_hash::url_cache_name(&format!("{base}/{path}"))
}

#[test]
fn parses_a_v3_manifest() {
    let json = format!(
        r#"{{
            "v": 3,
            "lanes": {{"texture": "t2", "mesh": "m1"}},
            "unknown": true,
            "files": {{
                "{ORIGINAL_CID}": [
                    "bevy/t2/{ORIGINAL_CID}_srgb",
                    "bevy/m1/{ORIGINAL_CID}",
                    "bevy/g4/{ORIGINAL_CID}",
                    "bevy/g4z9e12/{ORIGINAL_CID}_0123456789abcdef",
                    "bevy/x1/{ORIGINAL_CID}",
                    "bevy/../etc/passwd",
                    "bevy/t2/a.br",
                    "bevy/manifest/x",
                    "https://elsewhere/bevy/t2/x",
                    "bevy/t2/x/y",
                    ""
                ],
                "bafkreionlybadvariants": ["bevy/q1/x", "../x"]
            }}
        }}"#
    );
    let manifest = parse_manifest(json.as_bytes()).unwrap();
    assert_eq!(manifest.lanes["texture"], "t2");
    // only the well-formed paths of a known class survive, in order, and an original left with
    // none is dropped
    assert_eq!(manifest.files.len(), 1);
    let variants: Vec<_> = manifest.files[ORIGINAL_CID]
        .iter()
        .map(|v| (v.path.as_str(), v.kind.as_str(), v.format.as_str()))
        .collect();
    assert_eq!(
        variants,
        [
            (
                &*format!("bevy/t2/{ORIGINAL_CID}_srgb"),
                "texture",
                "ktx2-bc"
            ),
            (&*format!("bevy/m1/{ORIGINAL_CID}"), "model", "glb-br"),
            (&*format!("bevy/g4/{ORIGINAL_CID}"), "model", "glb"),
            (
                &*format!("bevy/g4z9e12/{ORIGINAL_CID}_0123456789abcdef"),
                "model",
                "glb"
            ),
        ]
    );
}

#[test]
fn reads_any_version_and_rejects_garbage() {
    // `v` is abgen's rebuild reference: the client reads the paths whatever it says, or
    // without it
    let listed = format!(r#""files": {{"{ORIGINAL_CID}": ["bevy/t4/{VARIANT_CID}"]}}"#);
    for manifest in [
        format!(r#"{{"v": 3, {listed}}}"#),
        format!(r#"{{"v": 4, "added": true, {listed}}}"#),
        format!(r#"{{{listed}}}"#),
        format!(r#"{{"lanes": null, {listed}}}"#),
        format!(r#"{{"lanes": ["t4"], {listed}}}"#),
    ] {
        let manifest = parse_manifest(manifest.as_bytes()).unwrap();
        assert_eq!(
            manifest.files[ORIGINAL_CID][0].path,
            format!("bevy/t4/{VARIANT_CID}")
        );
    }
    // `lanes` is only shown: an entry that is not a string is left out, the rest stay
    let lanes = format!(r#"{{"lanes": {{"texture": 4, "mesh": "m2"}}, {listed}}}"#);
    let manifest = parse_manifest(lanes.as_bytes()).unwrap();
    assert_eq!(manifest.lanes.len(), 1);
    assert_eq!(manifest.lanes["mesh"], "m2");
    assert!(manifest.files.contains_key(ORIGINAL_CID));
    // the entries of the first manifests were objects, not paths: not a manifest
    let objects =
        format!(r#"{{"v": 2, "files": {{"{ORIGINAL_CID}": [{{"cid": "{VARIANT_CID}"}}]}}}}"#);
    assert!(parse_manifest(objects.as_bytes()).is_err());
    assert!(parse_manifest(b"<html>not found</html>").is_err());
    assert!(parse_manifest(br#"{"error": "not found"}"#).is_err());
}

#[test]
fn selects_the_first_variant_this_client_takes() {
    let variants = [
        variant(OTHER_CID, "texture", "astc-4x4"),
        // the per-format strings of v2 manifests are no longer taken
        variant(OTHER_CID, "texture", "ktx2-bc7"),
        variant(VARIANT_CID, "texture", "ktx2-bc"),
        variant(OTHER_CID, "texture", "ktx2-bc"),
    ];
    let none_bad = |_: &str| false;
    assert_eq!(
        ContentFileVariant::preferred(
            &variants,
            ContentFilePathView("images/a.png"),
            true,
            none_bad
        )
        .map(|v| v.path.clone()),
        Some(served_path(VARIANT_CID))
    );
    // the extension is matched case-insensitively, and a path without one is an image too
    assert!(ContentFileVariant::preferred(
        &variants,
        ContentFilePathView("images/A.JPG"),
        true,
        none_bad
    )
    .is_some());
    assert!(ContentFileVariant::preferred(
        &variants,
        ContentFilePathView("images/a.ktx2"),
        true,
        none_bad
    )
    .is_some());
    assert!(ContentFileVariant::preferred(
        &variants,
        ContentFilePathView("noextension"),
        true,
        none_bad
    )
    .is_some());
    // no BC on the GPU: nothing
    assert!(ContentFileVariant::preferred(
        &variants,
        ContentFilePathView("images/a.png"),
        false,
        none_bad
    )
    .is_none());
    // a texture never replaces a model, nor a file another loader reads
    assert!(ContentFileVariant::preferred(
        &variants,
        ContentFilePathView("models/a.glb"),
        true,
        none_bad
    )
    .is_none());
    assert!(ContentFileVariant::preferred(
        &variants,
        ContentFilePathView("sounds/a.mp3"),
        true,
        none_bad
    )
    .is_none());
    // a variant that failed this session is passed over for the next supported one
    let bad = served_path(VARIANT_CID);
    let fallback =
        ContentFileVariant::preferred(&variants, ContentFilePathView("a.png"), true, |path| {
            path == bad
        });
    assert_eq!(
        fallback.map(|v| v.path.clone()),
        Some(served_path(OTHER_CID))
    );

    // `glb` (its embedded images BC KTX2, unwrapped by the glTF loader) is taken for a `.glb`,
    // on a GPU with BC textures
    let glb = [variant(VARIANT_CID, "model", "glb")];
    assert!(supported_format(&glb[0]).is_some());
    assert!(ContentFileVariant::preferred(
        &glb,
        ContentFilePathView("models/a.glb"),
        true,
        none_bad
    )
    .is_some());
    assert!(ContentFileVariant::preferred(
        &glb,
        ContentFilePathView("models/A.GLB"),
        true,
        none_bad
    )
    .is_some());
    assert!(ContentFileVariant::preferred(
        &glb,
        ContentFilePathView("models/a.glb"),
        false,
        none_bad
    )
    .is_none());
    assert!(ContentFileVariant::preferred(
        &glb,
        ContentFilePathView("models/a.gltf"),
        true,
        none_bad
    )
    .is_none());
    assert!(ContentFileVariant::preferred(
        &glb,
        ContentFilePathView("images/a.png"),
        true,
        none_bad
    )
    .is_none());
    // `glb-br` is fetched as a Brotli copy only a browser decodes: native leaves it untaken and
    // loads the original model (the web build takes it for `.glb` files)
    let brotli = [variant(VARIANT_CID, "model", "glb-br")];
    assert!(supported_format(&brotli[0]).is_none());
    assert!(ContentFileVariant::preferred(
        &brotli,
        ContentFilePathView("models/a.glb"),
        true,
        none_bad
    )
    .is_none());
    assert_eq!(
        taken_formats(),
        "ktx2-bc (BC1/BC5/BC7, unwrapped to DDS by the image loaders), \
         glb (embedded BC1/BC5/BC7, unwrapped to DDS by the glTF loader); not glb-br"
    );
    assert!(could_have_variant(ContentFilePathView("a.glb")));
    // the earlier mesh format is not in the schema any more
    assert!(supported_format(&variant(VARIANT_CID, "model", "glb-meshopt")).is_none());
    // a kind/format pair the schema does not have
    assert!(ContentFileVariant::preferred(
        &[variant(VARIANT_CID, "model", "ktx2-bc")],
        ContentFilePathView("a.png"),
        true,
        none_bad
    )
    .is_none());
    assert!(
        could_have_variant(ContentFilePathView("a.KTX2"))
            && could_have_variant(ContentFilePathView("a"))
    );
    assert!(
        !could_have_variant(ContentFilePathView("main.crdt"))
            && !could_have_variant(ContentFilePathView("a.mp3"))
    );
    // a url a scene names waits on no manifest
    assert!(!could_have_variant(ContentFilePathView(
        "https://abgen.example/LOD/lods-bevy/lods/bafkreiscene_1.glb"
    )));
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
    assert_eq!(
        scene_eligibility(SCENE, None, false),
        Err("no content server")
    );
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
    assert!(io
        .content_file_variant_manifests
        .0
        .lock_ignore_poison()
        .states
        .is_empty());
    let path = content_path("tex.png");
    let ipfs_path = IpfsPath::new_from_path(&path).unwrap().unwrap();
    assert!(io.read_bevy_variant(&ipfs_path, &path).await.is_none());
    std::fs::remove_dir_all(dir).unwrap();
}

#[tokio::test]
async fn preview_and_local_scenes_ask_for_no_manifest() {
    let (mut io, dir) = io_with_cache("preview");
    Arc::get_mut(&mut io).unwrap().is_preview = true;
    io.init_bevy_variants_base("https://abgen.example");
    io.request_bevy_variants(SCENE);
    let mut state = io.content_file_variant_manifests.get(SCENE).unwrap();
    let state = state
        .wait_for(|state| !matches!(state, ManifestState::Pending))
        .await
        .unwrap()
        .clone();
    assert!(matches!(state, ManifestState::Ineligible("preview mode")));
    std::fs::remove_dir_all(dir).unwrap();
}

#[tokio::test]
async fn a_near_lod_descriptor_comes_from_the_variants_base() {
    let (io, dir) = io_with_cache("lod-descriptor");
    assert_eq!(io.bevy_lod_descriptor(SCENE), None);
    io.init_bevy_variants_base("https://abgen.example/");
    assert_eq!(
        io.bevy_lod_descriptor(SCENE),
        Some(format!(
            "https://abgen.example/LOD/lods-unity/manifests/{}_InitialSceneState.json",
            SCENE.to_lowercase()
        ))
    );
    std::fs::remove_dir_all(dir).unwrap();
}

#[tokio::test]
async fn a_far_lod_is_offered_on_the_terms_of_a_manifest_and_bc() {
    let (mut io, dir) = io_with_cache("far-lod");
    assert_eq!(
        io.bevy_far_lod(SCENE),
        FarLodSource::Unavailable("bevy variants off".to_owned())
    );
    io.init_bevy_variants_base("https://abgen.example/");
    assert_eq!(io.bevy_far_lod(SCENE), FarLodSource::NotYet);
    io.context.write().await.about = Some(ServerAbout {
        content: Some(EndpointConfig {
            healthy: true,
            public_url: "https://peer.example/content".to_owned(),
        }),
        ..Default::default()
    });
    io.content_file_variants_config.init_bc(true);
    assert_eq!(
        io.bevy_far_lod(SCENE),
        FarLodSource::Url(format!(
            "https://abgen.example/LOD/lods-bevy/lods/{}_1.glb",
            SCENE.to_lowercase()
        ))
    );
    assert!(matches!(
        io.bevy_far_lod("b64-abc"),
        FarLodSource::Unavailable(_)
    ));
    Arc::get_mut(&mut io).unwrap().is_preview = true;
    assert_eq!(
        io.bevy_far_lod(SCENE),
        FarLodSource::Unavailable("preview mode".to_owned())
    );
    std::fs::remove_dir_all(dir).unwrap();
}

#[tokio::test]
async fn a_missing_far_lod_is_remembered_and_failures_pause_far_lods() {
    let (io, dir) = io_with_cache("far-lod-health");
    io.init_bevy_variants_base("https://abgen.example");
    io.context.write().await.about = Some(ServerAbout {
        content: Some(EndpointConfig {
            healthy: true,
            public_url: "https://peer.example/content".to_owned(),
        }),
        ..Default::default()
    });
    io.content_file_variants_config.init_bc(true);
    let url = |scene: &str| match io.bevy_far_lod(scene) {
        FarLodSource::Url(url) => url,
        other => panic!("{scene}: {other:?}"),
    };
    let status = reqwest::StatusCode::from_u16;

    // a 404 is remembered for the scene, by its lower-case id
    let missing = url("bafkreiMissing");
    io.far_lod_fetched(&missing, Err(Some(status(404).unwrap())));
    assert_eq!(
        io.bevy_far_lod("bafkreiMissing"),
        FarLodSource::Unavailable("the server has none for the scene".to_owned())
    );
    assert_eq!(io.far_lod_status().missing, 1);

    // a far LOD url the client never named counts for nothing, whatever it answers
    let made_up = "https://abgen.example/LOD/lods-bevy/lods/bafkreimadeup_1.glb";
    io.far_lod_fetched(made_up, Err(Some(status(404).unwrap())));
    for _ in 0..10 {
        io.far_lod_fetched(made_up, Err(None));
    }
    assert_eq!(io.far_lod_status().missing, 1);
    assert_eq!(io.far_lod_status().failures, 0);
    assert!(matches!(
        io.bevy_far_lod("bafkreimadeup"),
        FarLodSource::Url(_)
    ));
    assert!(io.is_named_far_lod(made_up));

    // other urls are not far LODs and count for nothing
    for _ in 0..10 {
        io.far_lod_fetched("https://abgen.example/bevy/manifest/x.json", Err(None));
    }
    assert_eq!(io.far_lod_status().failures, 0);

    // errors, timeouts and statuses that may change pause far LODs once enough are in a row
    for (n, result) in [
        Err(None),
        Err(Some(status(503).unwrap())),
        Err(Some(status(403).unwrap())),
    ]
    .into_iter()
    .cycle()
    .take(far_lod::FAR_LOD_BREAKER_FAILURES as usize)
    .enumerate()
    {
        io.far_lod_fetched(&url(&format!("bafkrei{n}")), result);
    }
    // naming a url spends no probe: the download is what the breaker gates
    assert!(matches!(io.bevy_far_lod(SCENE), FarLodSource::Url(_)));
    assert!(io.far_lod_status().paused_for.is_some());
    assert!(!io.far_lod_may_request());
    // a remembered scene stays unavailable
    assert!(matches!(
        io.bevy_far_lod("bafkreimissing"),
        FarLodSource::Unavailable(_)
    ));
    std::fs::remove_dir_all(dir).unwrap();
}

#[tokio::test]
async fn far_lods_outlive_the_variants_session_switch() {
    let (io, dir) = io_with_cache("far-lod-session-switch");
    io.init_bevy_variants_base("https://abgen.example");
    io.context.write().await.about = Some(ServerAbout {
        content: Some(EndpointConfig {
            healthy: true,
            public_url: "https://peer.example/content".to_owned(),
        }),
        ..Default::default()
    });
    io.content_file_variants_config.init_bc(true);
    let url = |scene: &str| match io.bevy_far_lod(scene) {
        FarLodSource::Url(url) => url,
        other => panic!("{scene}: {other:?}"),
    };

    // manifest and texture failures turn the variants client off for the session
    for _ in 0..BREAKER_FAILURES {
        io.content_file_variant_session
            .transport_failed("connection refused");
    }
    assert!(io.content_file_variant_session.disabled().is_some());

    // far LODs are still offered, and their own breaker sees the outage
    assert!(matches!(io.bevy_far_lod(SCENE), FarLodSource::Url(_)));
    assert!(!io.far_lods_off());
    assert!(io.far_lod_may_request());
    for n in 0..far_lod::FAR_LOD_BREAKER_FAILURES {
        io.far_lod_fetched(&url(&format!("bafkrei{n}")), Err(None));
    }
    let status = io.far_lod_status();
    assert_eq!(status.failures, far_lod::FAR_LOD_BREAKER_FAILURES);
    assert!(status.paused_for.is_some());
    assert!(!io.far_lod_may_request());
    assert!(!io.far_lods_off());

    // an answer resumes them
    io.far_lod_fetched(&url("bafkrei0"), Ok(()));
    assert!(io.far_lod_status().paused_for.is_none());
    assert!(io.far_lod_may_request());
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
                seen.lock_ignore_poison().push(path.clone());
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

/// A scene with `tex.png` = [`ORIGINAL_CID`], whose manifest offers `variant_cid` as BC7 KTX2,
/// served from `base` (which is also the realm's content server).
async fn scene_with_variant(io: &Arc<IpfsIo>, base: &str, variant_cid: &str) {
    scene_with_variant_of(io, base, "tex.png", "texture", "ktx2-bc7", variant_cid).await
}

/// A scene with one content file whose original has one variant, with a loaded manifest.
async fn scene_with_variant_of(
    io: &Arc<IpfsIo>,
    base: &str,
    file: &str,
    kind: &str,
    format: &str,
    variant_cid: &str,
) {
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
                collection: ContentMap::new_single(file.to_owned(), ORIGINAL_CID.to_owned()),
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
    io.init_bevy_variants_base(base);
    io.content_file_variants_config.init_bc(true);
    let manifest = format!(
        r#"{{"v": 3, "lanes": {{"texture": "t2", "mesh": "m1"}}, "files": {{"{ORIGINAL_CID}": ["{}"]}}}}"#,
        served_path_of(format, variant_cid)
    );
    let variants = parse_manifest(manifest.as_bytes()).unwrap();
    assert_eq!(variants.files[ORIGINAL_CID][0].kind, kind);
    // the scene's own content server is loopback here, which `request_bevy_variants` refuses
    let (sender, receiver) = watch::channel(ManifestState::Loaded(Arc::new(variants)));
    drop(sender);
    io.content_file_variant_manifests
        .0
        .lock_ignore_poison()
        .states
        .insert(SCENE.to_owned(), receiver);
}

async fn read_all(io: &Arc<IpfsIo>, path: &Path) -> Vec<u8> {
    let mut reader = io.read(path).await.unwrap();
    let mut bytes = Vec::new();
    reader.read_to_end(&mut bytes).await.unwrap();
    bytes
}

fn requests_for(log: &Mutex<Vec<String>>, path: &str) -> usize {
    log.lock_ignore_poison()
        .iter()
        .filter(|p| *p == path)
        .count()
}

#[tokio::test]
async fn a_texture_variant_is_served_as_its_ktx2_and_cached_under_its_url() {
    let (io, dir) = io_with_cache("served");
    let (ktx2, cid) = bc7_variant();
    let variant_route = format!("/{}", served_path(&cid));
    let original_route = format!("/contents/{ORIGINAL_CID}");
    let (base, log) = serve(vec![
        (variant_route.clone(), ktx2.clone()),
        (original_route.clone(), b"original png".to_vec()),
    ])
    .await;
    scene_with_variant(&io, &base, &cid).await;
    let path = content_path("tex.png");

    // the reader hands the KTX2 over as it is: the image loaders unwrap it
    assert_eq!(read_all(&io, &path).await, ktx2);
    assert!(io.served_bevy_variant(&path));
    // the KTX2 under a digest of its url, and nothing beside it, under the original's hash or
    // under the variant's path
    let name = cache_name(&base, &served_path(&cid));
    assert_eq!(std::fs::read(dir.join(&name)).unwrap(), ktx2);
    assert!(!dir.join(format!("{name}.dds")).exists());
    assert!(!dir.join(ORIGINAL_CID).exists());
    assert!(!dir.join("bevy").exists());

    // a warm read is the cached KTX2
    assert_eq!(read_all(&io, &path).await, ktx2);

    assert_eq!(requests_for(&log, &variant_route), 1);
    assert_eq!(requests_for(&log, &original_route), 0);
    let session = io.content_file_variant_session.lock();
    assert_eq!(
        (session.downloaded, session.cached, session.fallbacks),
        (1, 1, 0)
    );
    assert_eq!(session.served_by_format.get("ktx2-bc"), Some(&[1, 1]));
    drop(session);
    std::fs::remove_dir_all(dir).unwrap();
}

/// Wearables and emotes register their entity with the catalyst's content url as its own base
/// (`collectibles::request_collectibles`) and request its manifest as a scene does: the
/// manifest is fetched from the variants base and the entity's files read as their variants.
#[tokio::test]
async fn a_wearable_entity_fetches_its_manifest_and_reads_its_variants() {
    const WEARABLE: &str = "bafkreiwearable";
    let (io, dir) = io_with_cache("wearable");
    let (ktx2, cid) = bc7_variant();
    let manifest = format!(
        r#"{{"v": 3, "lanes": {{"texture": "t2"}}, "files": {{"{ORIGINAL_CID}": ["{}"]}}}}"#,
        served_path(&cid)
    );
    let manifest_route = format!("/bevy/manifest/{WEARABLE}.json");
    let variant_route = format!("/{}", served_path(&cid));
    let (base, log) = serve(vec![
        (manifest_route.clone(), manifest.into_bytes()),
        (variant_route.clone(), ktx2.clone()),
    ])
    .await;
    let about = ServerAbout {
        content: Some(EndpointConfig {
            healthy: true,
            public_url: base.clone(),
        }),
        ..Default::default()
    };
    {
        let mut context = io.context.write().await;
        context.about = Some(about.clone());
        context.entities.insert(
            WEARABLE.to_owned(),
            IpfsEntity {
                collection: ContentMap::new_single(
                    "male/tex.png".to_owned(),
                    ORIGINAL_CID.to_owned(),
                ),
                metadata: None,
            },
        );
        // as `request_collectibles` registers it: the catalyst's contents, not the realm's. The
        // name is reserved and never resolves, so a read of the original fails the test and
        // reaches no host.
        context.modifiers.insert(
            WEARABLE.to_owned(),
            crate::IpfsModifier {
                base_url: Some("https://catalyst.invalid/content/contents/".to_owned()),
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
    io.init_bevy_variants_base(&base);
    io.content_file_variants_config.init_bc(true);

    io.request_bevy_variants(WEARABLE);
    // A read waits for the manifest only until its deadline, then reads the original. The
    // deadline has its own test; this one reads after the manifest is there.
    let mut manifest = io.content_file_variant_manifests.get(WEARABLE).unwrap();
    let state = manifest
        .wait_for(|state| !matches!(state, ManifestState::Pending))
        .await
        .unwrap()
        .clone();
    assert!(matches!(state, ManifestState::Loaded(_)), "{state:?}");
    let path = PathBuf::from(&IpfsPath::new(IpfsType::new_content_file(
        WEARABLE.to_owned(),
        "male/tex.png".to_owned(),
    )));
    assert_eq!(read_all(&io, &path).await, ktx2);
    assert!(io.served_bevy_variant(&path));
    assert_eq!(requests_for(&log, &manifest_route), 1);
    assert_eq!(requests_for(&log, &variant_route), 1);
    std::fs::remove_dir_all(dir).unwrap();
}

/// The manifest names no hash for a variant, so its header decides: a KTX2 that holds no BC
/// format this client unwraps (here sRGB BC7) falls back, and is not asked for again.
#[tokio::test]
async fn a_texture_variant_whose_header_names_no_bc_format_falls_back_to_the_original() {
    let (io, dir) = io_with_cache("not-bc");
    let (_, level0, level1) = super::ktx2::tests::bc7_two_levels();
    let srgb = super::ktx2::tests::ktx2(146, 8, 4, &[level0, level1]);
    let variant_route = format!("/{}", served_path(VARIANT_CID));
    let original_route = format!("/contents/{ORIGINAL_CID}");
    let (base, log) = serve(vec![
        (variant_route.clone(), srgb),
        (original_route.clone(), b"original png".to_vec()),
    ])
    .await;
    scene_with_variant(&io, &base, VARIANT_CID).await;
    let path = content_path("tex.png");

    assert_eq!(read_all(&io, &path).await, b"original png");
    assert!(!io.served_bevy_variant(&path));
    // the rejected copy is dropped from the cache, so a later session does not read it again
    let name = cache_name(&base, &served_path(VARIANT_CID));
    assert!(!dir.join(&name).exists());
    assert!(!dir.join(ORIGINAL_CID).exists());
    {
        let session = io.content_file_variant_session.lock();
        assert_eq!(session.fallbacks, 1);
        assert!(session.recent[0].reason.contains("vkFormat 146"));
        assert!(session.bad.contains(&served_path(VARIANT_CID)));
    }

    // the bad variant is not asked for again this session
    assert_eq!(read_all(&io, &path).await, b"original png");
    assert_eq!(requests_for(&log, &variant_route), 1);
    assert_eq!(requests_for(&log, &original_route), 2);
    std::fs::remove_dir_all(dir).unwrap();
}

#[tokio::test]
async fn a_variant_that_is_no_ktx2_falls_back_to_the_original() {
    let (io, dir) = io_with_cache("not-ktx2");
    let variant_route = format!("/{}", served_path(VARIANT_CID));
    let original_route = format!("/contents/{ORIGINAL_CID}");
    let (base, log) = serve(vec![
        (variant_route.clone(), variant_bytes()),
        (original_route, b"original png".to_vec()),
    ])
    .await;
    scene_with_variant(&io, &base, VARIANT_CID).await;
    let path = content_path("tex.png");

    assert_eq!(read_all(&io, &path).await, b"original png");
    let name = cache_name(&base, &served_path(VARIANT_CID));
    assert!(!dir.join(&name).exists());
    {
        let session = io.content_file_variant_session.lock();
        assert!(session.recent[0].reason.starts_with("invalid KTX2"));
        assert!(session.bad.contains(&served_path(VARIANT_CID)));
    }
    assert_eq!(read_all(&io, &path).await, b"original png");
    assert_eq!(requests_for(&log, &variant_route), 1);
    std::fs::remove_dir_all(dir).unwrap();
}

#[tokio::test]
async fn a_missing_variant_falls_back_to_the_original() {
    let (io, dir) = io_with_cache("missing");
    let original_route = format!("/contents/{ORIGINAL_CID}");
    let (base, _log) = serve(vec![(original_route, b"original png".to_vec())]).await;
    scene_with_variant(&io, &base, VARIANT_CID).await;
    let path = content_path("tex.png");
    assert_eq!(read_all(&io, &path).await, b"original png");
    let session = io.content_file_variant_session.lock();
    assert_eq!(session.fallbacks, 1);
    assert!(session.recent[0].reason.starts_with("fetch:"));
    drop(session);
    std::fs::remove_dir_all(dir).unwrap();
}

/// A `g` variant: a GLB (its embedded images BC KTX2), served at its own path (native has no
/// Brotli decoder). The reader checks only the GLB magic; the glTF loader decodes the images.
fn glb_variant() -> (Vec<u8>, String) {
    let mut file = b"glTF".to_vec();
    file.extend_from_slice(&2u32.to_le_bytes());
    file.extend_from_slice(&12u32.to_le_bytes());
    let cid = crate::content_hash::raw_cid(&file);
    (file, cid)
}

#[tokio::test]
async fn a_glb_variant_is_read_as_served() {
    let (io, dir) = io_with_cache("glb");
    let (served, cid) = glb_variant();
    let variant_route = format!("/{}", served_path_of("glb", &cid));
    let original_route = format!("/contents/{ORIGINAL_CID}");
    let (base, log) = serve(vec![
        (variant_route.clone(), served.clone()),
        (original_route.clone(), b"original glb".to_vec()),
    ])
    .await;
    scene_with_variant_of(&io, &base, "models/a.glb", "model", "glb", &cid).await;
    let path = content_path("models/a.glb");

    // the loader reads the GLB as served, and the cache holds it under a digest of its url
    assert_eq!(read_all(&io, &path).await, served);
    assert!(io.served_bevy_variant(&path));
    let name = cache_name(&base, &served_path_of("glb", &cid));
    assert_eq!(std::fs::read(dir.join(&name)).unwrap(), served);
    assert!(!dir.join(ORIGINAL_CID).exists());
    assert_eq!(read_all(&io, &path).await, served);

    assert_eq!(requests_for(&log, &variant_route), 1);
    assert_eq!(requests_for(&log, &original_route), 0);
    let session = io.content_file_variant_session.lock();
    assert_eq!(session.served_by_format.get("glb"), Some(&[1, 1]));
    drop(session);
    std::fs::remove_dir_all(dir).unwrap();
}

/// A `g` variant that is not a binary glTF at all falls back before the loader sees it: the
/// original is read instead, the copy dropped and the variant not asked for again.
#[tokio::test]
async fn a_glb_variant_that_is_not_a_glb_falls_back_to_the_original() {
    let (io, dir) = io_with_cache("glb-not-glb");
    let served = b"<html>not found</html>".to_vec();
    let cid = crate::content_hash::raw_cid(&served);
    let variant_route = format!("/{}", served_path_of("glb", &cid));
    let original_route = format!("/contents/{ORIGINAL_CID}");
    let (base, log) = serve(vec![
        (variant_route.clone(), served),
        (original_route.clone(), b"original glb".to_vec()),
    ])
    .await;
    scene_with_variant_of(&io, &base, "models/a.glb", "model", "glb", &cid).await;
    let path = content_path("models/a.glb");

    assert_eq!(read_all(&io, &path).await, b"original glb");
    assert!(!io.served_bevy_variant(&path));
    let name = cache_name(&base, &served_path_of("glb", &cid));
    assert!(!dir.join(&name).exists());
    {
        let session = io.content_file_variant_session.lock();
        assert!(session.recent[0].reason.starts_with("invalid glb variant"));
        assert_eq!(session.fallbacks_by_format.get("glb"), Some(&1));
        assert!(session.bad.contains(&served_path_of("glb", &cid)));
    }
    assert_eq!(read_all(&io, &path).await, b"original glb");
    assert_eq!(requests_for(&log, &variant_route), 1);
    assert_eq!(requests_for(&log, &original_route), 2);
    std::fs::remove_dir_all(dir).unwrap();
}

/// A `glb` variant whose embedded KTX2 does not unwrap fails the `Gltf` load, not the read: the
/// `Gltf` fallback marks the variant bad and hands back its cached copy to drop, so the model
/// reloads from its original.
#[test]
fn a_glb_model_that_fails_to_load_is_reloaded_from_the_original() {
    let variants = ContentFileVariantSession::default();
    let path = content_path("models/a.glb");
    variants.served(
        &path,
        ServedVariant {
            scene: SCENE.to_owned(),
            file: "models/a.glb".to_owned(),
            variant: served_path_of("glb", VARIANT_CID),
            format: "glb".to_owned(),
            url: "https://abgen.example/bevy/g4/variant".to_owned(),
        },
        false,
    );
    // an image fallback does not take a model
    assert!(variants
        .decode_failed(&path, "x", <Image as VariantAsset>::FORMATS)
        .is_none());
    assert!(variants.is_served(&path));
    assert_eq!(
        variants
            .decode_failed(
                &path,
                "unsupported texture format",
                <Gltf as VariantAsset>::FORMATS
            )
            .as_deref(),
        Some("https://abgen.example/bevy/g4/variant")
    );
    assert!(!variants.is_served(&path));
    assert!(variants.is_bad(&served_path_of("glb", VARIANT_CID)));
    let session = variants.lock();
    assert_eq!(session.fallbacks_by_format.get("glb"), Some(&1));
    assert!(session.recent[0]
        .reason
        .starts_with("glb variant failed to load"));
}

/// An avatar waits on a model whose `glb` variant failed: from the failure until the
/// original's reload is over, `model_reload_pending` holds. A `glb-br` copy's failure never
/// reloads, so nothing waits on it.
#[test]
fn a_failed_glb_model_is_pending_until_its_original_reloads() {
    let variants = ContentFileVariantSession::default();
    let path = content_path("wearable.glb");
    variants.served(
        &path,
        ServedVariant {
            scene: SCENE.to_owned(),
            file: "wearable.glb".to_owned(),
            variant: served_path_of("glb", VARIANT_CID),
            format: "glb".to_owned(),
            url: "https://abgen.example/bevy/g4/variant".to_owned(),
        },
        false,
    );
    // failed, the fallback not yet run: still served from the variant
    assert!(variants.model_reload_pending(&path));
    variants.decode_failed(&path, "x", <Gltf as VariantAsset>::FORMATS);
    variants.reload_started(&path);
    assert!(variants.model_reload_pending(&path));
    variants.reload_finished(&path);
    assert!(!variants.model_reload_pending(&path));

    let br = content_path("draco.glb");
    variants.served(
        &br,
        ServedVariant {
            scene: SCENE.to_owned(),
            file: "draco.glb".to_owned(),
            variant: served_path_of("glb-br", VARIANT_CID),
            format: "glb-br".to_owned(),
            url: "https://abgen.example/bevy/m2/variant.br".to_owned(),
        },
        false,
    );
    assert!(!variants.model_reload_pending(&br));
    assert!(!variants.model_reload_pending(&content_path("never-a-variant.glb")));
}

#[test]
fn a_variant_that_fails_to_decode_is_marked_bad_once() {
    let variants = ContentFileVariantSession::default();
    let path = content_path("tex.png");
    variants.served(
        &path,
        ServedVariant {
            scene: SCENE.to_owned(),
            file: "tex.png".to_owned(),
            variant: served_path(VARIANT_CID),
            format: "ktx2-bc".to_owned(),
            url: "https://abgen.example/bevy/t3/variant".to_owned(),
        },
        false,
    );
    assert!(variants.is_served(&path));
    // reloaded from the original, and the url of the copy to drop handed back
    assert_eq!(
        variants
            .decode_failed(&path, "unsupported texture format", &["ktx2-bc"])
            .as_deref(),
        Some("https://abgen.example/bevy/t3/variant")
    );
    assert!(!variants.is_served(&path));
    assert!(variants.is_bad(&served_path(VARIANT_CID)));
    // an asset that was never a variant is not reloaded
    assert!(variants
        .decode_failed(&path, "again", &["ktx2-bc"])
        .is_none());
    assert!(variants
        .decode_failed(&content_path("other.png"), "x", &["ktx2-bc"])
        .is_none());
}

#[test]
fn a_glb_br_model_that_fails_to_load_is_not_reloaded() {
    let variants = ContentFileVariantSession::default();
    let path = content_path("models/a.glb");
    variants.served(
        &path,
        ServedVariant {
            scene: SCENE.to_owned(),
            file: "models/a.glb".to_owned(),
            variant: served_path_of("glb-br", VARIANT_CID),
            format: "glb-br".to_owned(),
            url: "https://abgen.example/bevy/m2/variant.br".to_owned(),
        },
        false,
    );
    // the glTF fallback takes only `glb`: a `glb-br` copy is the original, its failure the
    // original's own
    assert!(variants.decode_failed(&path, "draco", &["glb"]).is_none());
    assert!(variants.is_served(&path));
    assert!(!variants.is_bad(&served_path_of("glb-br", VARIANT_CID)));
}

#[test]
fn repeated_transport_failures_turn_variants_off_for_the_session() {
    let variants = ContentFileVariantSession::default();
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
    let config = ContentFileVariantsConfig::default();
    let manifests = ContentFileVariantManifests::default();
    let session = ContentFileVariantSession::default();
    assert_eq!(snapshot(&config, &manifests, &session)["state"], "off");
    config.init_base_url("http://localhost:5147/");
    config.init_bc(true);
    session.fail(
        SCENE,
        "tex.png",
        Some(&variant(VARIANT_CID, "texture", "ktx2-bc")),
        "fetch: refused".to_owned(),
    );
    let snapshot = snapshot(&config, &manifests, &session);
    assert_eq!(snapshot["base"], "http://localhost:5147");
    assert_eq!(snapshot["state"], "on");
    assert_eq!(snapshot["fallbacks"], 1);
    assert_eq!(snapshot["fallbacksByFormat"], "ktx2-bc: 1");
    assert!(snapshot["recentFallbacks"]
        .as_str()
        .unwrap()
        .contains("tex.png"));
}

fn snapshot(
    config: &ContentFileVariantsConfig,
    manifests: &ContentFileVariantManifests,
    session: &ContentFileVariantSession,
) -> serde_json::Value {
    super::debug_panel::snapshot(config, manifests, session)
}

/// A manifest that has not answered holds an entity's files only until its deadline: a read
/// near the deadline waits out what is left, a read after it goes to the original at once, and
/// a manifest that arrives late still serves the files read after it.
#[tokio::test]
async fn files_wait_for_a_pending_manifest_only_until_its_deadline() {
    let (io, dir) = io_with_cache("deadline");
    let (ktx2, cid) = bc7_variant();
    let variant_route = format!("/{}", served_path(&cid));
    let (base, log) = serve(vec![(variant_route.clone(), ktx2.clone())]).await;
    // a loaded manifest first, for the entity and realm setup, then put back to pending
    scene_with_variant(&io, &base, &cid).await;
    let loaded = io
        .content_file_variant_manifests
        .get(SCENE)
        .unwrap()
        .borrow()
        .clone();
    let (sender, receiver) = watch::channel(ManifestState::Pending);
    io.content_file_variant_manifests
        .0
        .lock_ignore_poison()
        .states
        .insert(SCENE.to_owned(), receiver);
    let requested = |ago: Duration| {
        io.content_file_variant_manifests
            .0
            .lock_ignore_poison()
            .requested_at
            .insert(SCENE.to_owned(), web_time::Instant::now() - ago);
    };
    let path = content_path("tex.png");
    let ipfs_path = IpfsPath::new_from_path(&path).unwrap().unwrap();

    // requested just short of the deadline: the read waits out the rest, then gives up
    requested(MANIFEST_DEADLINE - Duration::from_millis(150));
    let began = web_time::Instant::now();
    assert!(io.read_bevy_variant(&ipfs_path, &path).await.is_none());
    let waited = began.elapsed();
    assert!(
        waited >= Duration::from_millis(100) && waited < Duration::from_millis(1500),
        "waited {waited:?}"
    );
    // past the deadline: no wait at all
    let began = web_time::Instant::now();
    assert!(io.read_bevy_variant(&ipfs_path, &path).await.is_none());
    assert!(began.elapsed() < Duration::from_millis(100));
    {
        let session = io.content_file_variant_session.lock();
        assert_eq!(session.fallbacks, 2);
        assert!(session.recent[0].reason.contains("manifest was not there"));
        assert!(session.bad.is_empty());
    }
    assert_eq!(requests_for(&log, &variant_route), 0);

    // the manifest arrives late: the next read is served from it
    sender.send_replace(loaded);
    assert_eq!(read_all(&io, &path).await, ktx2);
    assert_eq!(requests_for(&log, &variant_route), 1);

    // a read that is waiting when the manifest arrives in time is served too
    let (sender, receiver) = watch::channel(ManifestState::Pending);
    io.content_file_variant_manifests
        .0
        .lock_ignore_poison()
        .states
        .insert(SCENE.to_owned(), receiver);
    requested(Duration::ZERO);
    let io2 = io.clone();
    let path2 = path.clone();
    let reading = tokio::spawn(async move { read_all(&io2, &path2).await });
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    let manifest = format!(
        r#"{{"v": 3, "files": {{"{ORIGINAL_CID}": ["{}"]}}}}"#,
        served_path(&cid)
    );
    sender.send_replace(ManifestState::Loaded(Arc::new(
        parse_manifest(manifest.as_bytes()).unwrap(),
    )));
    assert_eq!(reading.await.unwrap(), ktx2);
    std::fs::remove_dir_all(dir).unwrap();
}

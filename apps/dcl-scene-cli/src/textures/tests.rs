use super::*;
use crate::deploy::TempTree;
use crate::scene::Project;

/// A stand-in for basisu: a KTX2 magic, the mode and the PNG's digest, so
/// equal inputs give equal bytes and different ones differ; `bloat` makes
/// every result larger than any source.
struct FakeEncoder {
    bloat: bool,
}

impl Encoder for FakeEncoder {
    fn id(&self) -> &str {
        "fake"
    }

    fn encode(&self, png: &[u8], mode: ColorMode) -> anyhow::Result<Vec<u8>> {
        let mut out = b"\xABKTX 20\xBB\r\n\x1A\n".to_vec();
        out.extend_from_slice(mode.as_str().as_bytes());
        out.extend_from_slice(encode::sha256_hex(png).as_bytes());
        if self.bloat {
            out.resize(png.len() * 4 + 4096, 0);
        }
        Ok(out)
    }
}

/// A noisy image, so its PNG is far larger than the fake KTX2.
fn noise_png(w: u32, h: u32, seed: u32) -> Vec<u8> {
    let img = image::RgbImage::from_fn(w, h, |x, y| {
        let v = (x.wrapping_mul(7919) ^ y.wrapping_mul(104_729) ^ seed).wrapping_mul(2_654_435_761);
        image::Rgb([(v >> 8) as u8, (v >> 16) as u8, (v >> 24) as u8])
    });
    let mut out = std::io::Cursor::new(Vec::new());
    image::DynamicImage::ImageRgb8(img)
        .write_to(&mut out, image::ImageFormat::Png)
        .unwrap();
    out.into_inner()
}

fn write(root: &Path, rel: &str, bytes: &[u8]) {
    let p = root.join(rel);
    std::fs::create_dir_all(p.parent().unwrap()).unwrap();
    std::fs::write(p, bytes).unwrap();
}

/// models/a.glb: an embedded base colour, `images/wall.png` as its normal
/// map, `images/ui.png` as emissive. models/b.gltf names `images/logo.png`
/// without sampling it. bin/index.js names `images/ui.png`.
fn scene(tag: &str) -> (TempTree, PathBuf, Vec<u8>) {
    let t = TempTree::new(tag);
    let root = dunce::canonicalize(&t.0).unwrap();
    t.write(
        "scene.json",
        "{\"main\":\"bin/index.js\",\"runtimeVersion\":\"7\",\"scene\":{\"parcels\":[\"0,0\"],\"base\":\"0,0\"}}",
    );
    t.write(
        "bin/index.js",
        "engine.addEntity(); Material.setPbrMaterial(e, { texture: Material.Texture.Common({ src: 'images/ui.png' }) })",
    );
    write(&root, "images/wall.png", &noise_png(64, 64, 1));
    write(&root, "images/ui.png", &noise_png(64, 64, 2));
    write(&root, "images/logo.png", &noise_png(64, 64, 3));
    let embedded = noise_png(64, 64, 4);
    let mut bin = vec![7u8; 12];
    bin.extend_from_slice(&embedded);
    let glb = gltf::Model {
        json: json!({
            "asset": {"version": "2.0"},
            "buffers": [{"byteLength": bin.len()}],
            "bufferViews": [
                {"buffer": 0, "byteOffset": 0, "byteLength": 12},
                {"buffer": 0, "byteOffset": 12, "byteLength": embedded.len()}
            ],
            "accessors": [{"bufferView": 0, "componentType": 5126, "count": 1, "type": "VEC3"}],
            "images": [
                {"bufferView": 1, "mimeType": "image/png"},
                {"uri": "../images/wall.png"},
                {"uri": "../images/ui.png"}
            ],
            "textures": [{"source": 0}, {"source": 1}, {"source": 2}],
            "materials": [{
                "pbrMetallicRoughness": {"baseColorTexture": {"index": 0}},
                "normalTexture": {"index": 1},
                "emissiveTexture": {"index": 2}
            }]
        }),
        bin: Some(bin),
        glb: true,
    }
    .to_bytes();
    write(&root, "models/a.glb", &glb);
    t.write(
        "models/b.gltf",
        "{\"asset\":{\"version\":\"2.0\"},\"images\":[{\"uri\":\"../images/logo.png\"}]}",
    );
    (t, root, glb)
}

fn run(root: &Path, settings: Settings, encoder: &dyn Encoder) -> Outcome {
    let rels = crate::deploy::collect_publishable_files(root).unwrap();
    let run = Run {
        workers: 4,
        progress: &|_| {},
    };
    optimize(root, &rels, settings, encoder, &|rel| root.join(rel), &run).unwrap()
}

#[test]
fn ktx2_is_published_beside_its_model() {
    assert_eq!(published_ktx2("m.glb", "ab"), "ab.ktx2");
    assert_eq!(published_ktx2("models/m.glb", "ab"), "models/ab.ktx2");
    assert_eq!(published_ktx2("a/b/m.gltf", "ab"), "a/b/ab.ktx2");
    for model in ["m.glb", "models/m.glb", "a/b/m.gltf"] {
        let uri = gltf::relative_uri(model, &published_ktx2(model, "ab"));
        assert_eq!(uri, "ab.ktx2");
    }
}

#[test]
fn models_are_rewritten_staged_and_originals_ignored_unless_named() {
    let (_t, root, glb) = scene("textures-rewrite");
    let out = run(&root, Settings::default(), &FakeEncoder { bloat: false });
    let o = &out.overlay;

    // three distinct textures, one KTX2 each, published beside the model
    assert_eq!(o.added.len(), 3);
    assert!(
        o.added
            .iter()
            .all(|a| a.starts_with("models/") && a.ends_with(".ktx2"))
    );
    let staged_model = o.source("models/a.glb").unwrap();
    assert!(staged_model.starts_with(root.join(STAGE)));
    for added in &o.added {
        let staged = o.source(added).unwrap();
        let bytes = std::fs::read(staged).unwrap();
        assert_eq!(
            format!("models/{}.ktx2", encode::sha256_hex(&bytes)),
            *added
        );
    }

    // the creator's files are untouched
    assert_eq!(std::fs::read(root.join("models/a.glb")).unwrap(), glb);
    assert!(root.join("images/wall.png").is_file());

    let model = gltf::Model::parse(&std::fs::read(staged_model).unwrap()).unwrap();
    for (i, image) in model.json["images"].as_array().unwrap().iter().enumerate() {
        let uri = image["uri"].as_str().unwrap();
        // beside the model: no `..` for any client to resolve
        assert!(!uri.contains('/') && uri.ends_with(".ktx2"), "{uri}");
        assert_eq!(image["mimeType"], json!("image/ktx2"));
        assert_eq!(
            model.json["textures"][i],
            json!({"extensions": {"KHR_texture_basisu": {"source": i}}})
        );
    }
    assert_eq!(model.json["bufferViews"].as_array().unwrap().len(), 1);
    assert_eq!(model.bin.as_ref().unwrap().len(), 12);

    // wall.png only feeds the model; ui.png is named by the bundle; logo.png
    // is named by a model that keeps it
    assert_eq!(o.removed, BTreeSet::from(["images/wall.png".to_string()]));
    assert_eq!(out.summary.kept_originals.len(), 1);
    assert_eq!(out.summary.kept_originals[0].0, "images/ui.png");
    assert!(out.summary.kept_originals[0].1.contains("bin/index.js"));
    assert_eq!(out.summary.models, 1);
    assert_eq!(out.summary.encoded, 3);
    assert!(out.summary.bytes_after < out.summary.bytes_before);

    // the normal map was encoded as one
    let modes: BTreeSet<&str> = out.manifest["textures"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["colorMode"].as_str().unwrap())
        .collect();
    assert_eq!(modes, BTreeSet::from(["normal", "srgb"]));
    assert_eq!(out.summary.total, 3);
    assert_eq!(out.summary.failed, 0);
    assert!(out.summary.ktx2_bytes > 0 && out.summary.ktx2_bytes < out.summary.converted_bytes);
}

#[test]
fn a_second_run_is_served_from_the_cache_with_identical_bytes() {
    let (_t, root, _) = scene("textures-cache");
    let first = run(&root, Settings::default(), &FakeEncoder { bloat: false });
    let second = run(&root, Settings::default(), &FakeEncoder { bloat: false });
    assert_eq!(second.summary.encoded, 0);
    assert_eq!(second.summary.cached, 3);
    assert_eq!(first.overlay, second.overlay);
}

#[test]
fn keep_source_textures_ignores_nothing() {
    let (_t, root, _) = scene("textures-keep");
    let settings = Settings {
        keep_source_textures: true,
        ..Settings::default()
    };
    let out = run(&root, settings, &FakeEncoder { bloat: false });
    assert!(out.overlay.removed.is_empty());
    assert_eq!(out.overlay.added.len(), 3);
}

#[test]
fn a_ktx2_no_smaller_than_its_source_keeps_the_image() {
    let (_t, root, _) = scene("textures-bloat");
    let out = run(&root, Settings::default(), &FakeEncoder { bloat: true });
    assert!(out.overlay.sources.is_empty());
    assert!(out.overlay.added.is_empty());
    assert!(out.overlay.removed.is_empty());
    assert_eq!(out.summary.kept.len(), 3);
    assert!(
        out.summary
            .kept
            .iter()
            .all(|(_, why)| why.contains("not smaller"))
    );
}

#[test]
fn the_overlay_edits_the_list_and_nothing_else() {
    let overlay = Overlay {
        sources: BTreeMap::from([
            ("m.glb".to_string(), PathBuf::from("/stage/m.glb")),
            (
                "optimized-textures/a.ktx2".to_string(),
                PathBuf::from("/stage/a.ktx2"),
            ),
        ]),
        added: BTreeSet::from(["optimized-textures/a.ktx2".to_string()]),
        removed: BTreeSet::from(["t.png".to_string()]),
    };
    let rels = vec![
        "scene.json".to_string(),
        "m.glb".to_string(),
        "t.png".to_string(),
    ];
    assert_eq!(
        overlay.apply(rels),
        ["scene.json", "m.glb", "optimized-textures/a.ktx2"]
    );
    assert_eq!(overlay.source("m.glb"), Some(Path::new("/stage/m.glb")));
    assert_eq!(overlay.source("scene.json"), None);
}

#[test]
fn an_unregistered_project_is_untouched() {
    let root = Path::new("/nowhere/registered");
    let rels = vec!["a.glb".to_string(), "t.png".to_string()];
    assert_eq!(publishable(root, rels.clone()), rels);
    assert_eq!(staged_source(root, "a.glb"), None);
}

#[test]
fn a_disabled_overlay_is_not_applied() {
    let root = Path::new("/nowhere/disabled");
    let overlay = Overlay {
        sources: BTreeMap::from([("a.glb".to_string(), PathBuf::from("/stage/a.glb"))]),
        added: BTreeSet::new(),
        removed: BTreeSet::from(["t.png".to_string()]),
    };
    let entry = |enabled| Entry {
        enabled,
        settings: Settings::default(),
        overlay: Some(Arc::new(overlay.clone())),
        stamps: None,
    };
    let rels = vec!["a.glb".to_string(), "t.png".to_string()];
    assert!(
        swap(root, entry(false)),
        "a new overlay is a change, applied or not"
    );
    assert_eq!(publishable(root, rels.clone()), rels);
    assert_eq!(staged_source(root, "a.glb"), None);
    assert!(
        swap(root, entry(true)),
        "turning it on changes what is served"
    );
    assert_eq!(publishable(root, rels.clone()), ["a.glb"]);
    assert!(
        !swap(root, entry(true)),
        "the same overlay again changes nothing"
    );
    registry().remove(root);
}

#[test]
fn a_quoted_stem_keeps_the_original() {
    let (t, root, _) = scene("textures-stem");
    // code that appends the extension itself: `${name}.png`
    t.write(
        "bin/index.js",
        "const names = ['wall', \"other\"]; names.map(n => `images/${n}.png`); 'images/ui.png'",
    );
    let out = run(&root, Settings::default(), &FakeEncoder { bloat: false });
    assert!(out.overlay.removed.is_empty(), "{:?}", out.overlay.removed);
    let kept: BTreeSet<&str> = out
        .summary
        .kept_originals
        .iter()
        .map(|(p, _)| p.as_str())
        .collect();
    assert_eq!(kept, BTreeSet::from(["images/ui.png", "images/wall.png"]));

    // an unquoted or partial stem is not a reference
    assert!(names_image("const x = 'wall'", "images/wall.png"));
    assert!(names_image("load(`wall`)", "images/wall.png"));
    assert!(names_image("{\"src\":\"wall\"}", "images/wall.png"));
    assert!(names_image("models/wall.png", "images/wall.png"));
    assert!(!names_image("const wall = 1", "images/wall.png"));
    assert!(!names_image("'wallpaper'", "images/wall.png"));
    assert!(!names_image("''", "images/.png"), "no empty stem");
    // a folder before the stem, the extension appended
    assert!(names_image("'images/wall' + '.png'", "images/wall.png"));
    assert!(names_image("`${dir}/wall`", "images/wall.png"));
    assert!(!names_image("'images/wallpaper'", "images/wall.png"));
}

/// A deploy searches what it uploads: a file only the release build wrote
/// names the image, so the image is published.
#[test]
fn a_deploy_searches_the_release_only_files() {
    let (_t, root, _) = scene("textures-release");
    write(
        &root,
        &format!("{}/bin/chunk.js", crate::build::RELEASE_OUT),
        b"load('images/wall.png')",
    );
    let rels = crate::deploy::tree_payload_files(
        &root,
        crate::deploy::collect_publishable_files(&root).unwrap(),
    );
    assert!(rels.iter().any(|r| r == "bin/chunk.js"));
    let run = Run {
        workers: 2,
        progress: &|_| {},
    };
    let out = optimize(
        &root,
        &rels,
        Settings::default(),
        &FakeEncoder { bloat: false },
        &|rel| crate::deploy::tree_payload_path(&root, rel),
        &run,
    )
    .unwrap();
    assert!(out.overlay.removed.is_empty(), "{:?}", out.overlay.removed);
    let wall = out
        .summary
        .kept_originals
        .iter()
        .find(|(p, _)| p == "images/wall.png")
        .unwrap();
    assert!(wall.1.contains("bin/chunk.js"), "{}", wall.1);
}

/// An image URI no scene file answers (a Windows path) is still searched for
/// names: a client that reads it would 404 if the original were left out.
#[test]
fn an_unresolved_image_uri_keeps_the_original() {
    let (t, root, _) = scene("textures-backslash");
    t.write(
        "models/c.gltf",
        r#"{"asset":{"version":"2.0"},"images":[{"uri":"..\\images\\wall.png"}],"textures":[{"source":0}],"materials":[{"pbrMetallicRoughness":{"baseColorTexture":{"index":0}}}]}"#,
    );
    let out = run(&root, Settings::default(), &FakeEncoder { bloat: false });
    assert!(out.overlay.removed.is_empty(), "{:?}", out.overlay.removed);
    let wall = out
        .summary
        .kept_originals
        .iter()
        .find(|(p, _)| p == "images/wall.png")
        .unwrap();
    assert!(wall.1.contains("models/c.gltf"), "{}", wall.1);
}

#[test]
fn the_ignore_difference_names_both_directions() {
    let preview = json!({
        "ignoredOriginals": [
            {"path": "images/a.png", "reason": "x"},
            {"path": "images/b.png", "reason": "x"}
        ]
    });
    let same = BTreeSet::from(["images/a.png".to_string(), "images/b.png".to_string()]);
    assert_eq!(ignore_difference_warning(&preview, &same), None);

    let deployed = BTreeSet::from(["images/b.png".to_string(), "images/c.png".to_string()]);
    let warning = ignore_difference_warning(&preview, &deployed).unwrap();
    assert!(
        warning.contains("left out in the preview: images/a.png;"),
        "{warning}"
    );
    assert!(
        warning.contains("served in the preview: images/c.png"),
        "{warning}"
    );

    let none = BTreeSet::new();
    let warning = ignore_difference_warning(&preview, &none).unwrap();
    assert!(
        warning.ends_with("served in the preview: none"),
        "{warning}"
    );
    // a manifest without the list (or none at all) compares as empty
    assert_eq!(ignore_difference_warning(&json!({}), &none), None);
    assert!(ignored_warning(3).starts_with("3 original image(s) left out of the upload"));
    assert!(ignored_warning(3).contains("--keep-source-textures"));
}

#[test]
fn progress_is_reported_only_while_encoding() {
    let (_t, root, _) = scene("textures-progress");
    let rels = crate::deploy::collect_publishable_files(&root).unwrap();
    let seen = Mutex::new(Vec::new());
    let progress = |p: Progress| seen.lock().unwrap().push(p);
    let run = Run {
        workers: 2,
        progress: &progress,
    };
    let encoder = FakeEncoder { bloat: false };
    optimize(
        &root,
        &rels,
        Settings::default(),
        &encoder,
        &|r| root.join(r),
        &run,
    )
    .unwrap();
    let first = std::mem::take(&mut *seen.lock().unwrap());
    assert_eq!(first.first().map(|p| p.done), Some(0));
    assert_eq!(first.iter().map(|p| p.done).max(), Some(3));
    assert!(first.iter().all(|p| p.total == 3 && p.failed == 0));
    assert_eq!(first.len(), 4, "one at the start, one per encode");

    // everything cached: nothing to report
    optimize(
        &root,
        &rels,
        Settings::default(),
        &encoder,
        &|r| root.join(r),
        &run,
    )
    .unwrap();
    assert!(seen.lock().unwrap().is_empty());
}

fn enable_in_settings(root: &Path) {
    settings::apply(
        root,
        Overrides {
            optimize_textures: Some(true),
            keep_source_textures: None,
        },
    )
    .unwrap();
}

static FAKE: FakeEncoder = FakeEncoder { bloat: false };

fn fake() -> anyhow::Result<&'static dyn Encoder> {
    Ok(&FAKE)
}

#[test]
fn a_run_with_unchanged_inputs_sends_nothing() {
    let (_t, root, _) = scene("textures-noop");
    enable_in_settings(&root);
    let sent: Mutex<Vec<import::ImportStatus>> = Mutex::new(Vec::new());
    let send = |s: import::ImportStatus| sent.lock().unwrap().push(s);
    assert!(import::import_once(&root, &fake, 2, &send, false));
    let first = std::mem::take(&mut *sent.lock().unwrap());
    let last = first.last().unwrap();
    assert_eq!(last.state, import::ImportState::Ready);
    assert_eq!((last.done, last.total, last.failed), (3, 3, 0));
    assert!(last.enabled);
    assert!(last.bytes_after > 0 && last.bytes_after < last.bytes_before);
    assert!(root.join(STAGE).join(PREVIEW_MANIFEST).is_file());

    // nothing moved: no run, nothing sent
    assert!(!import::import_once(&root, &fake, 2, &send, false));
    assert!(sent.lock().unwrap().is_empty());

    // code changed, every texture cached and the overlay the same: still silent
    std::fs::write(root.join("bin/index.js"), "'images/ui.png'; // edited").unwrap();
    assert!(!import::import_once(&root, &fake, 2, &send, false));
    assert!(sent.lock().unwrap().is_empty());
    registry().remove(&root);
}

/// An encoder that runs a check before each encode, mid-run.
struct Probe<'a>(&'a (dyn Fn() + Sync));

impl Encoder for Probe<'_> {
    fn id(&self) -> &str {
        "fake"
    }

    fn encode(&self, png: &[u8], mode: ColorMode) -> anyhow::Result<Vec<u8>> {
        (self.0)();
        FAKE.encode(png, mode)
    }
}

/// A run in progress leaves the overlay in use whole: the old staged model
/// is still served and names only KTX2 files that overlay publishes. The new
/// one replaces it in one swap, under a new staged name, and the old file
/// goes once nothing names it.
#[test]
fn the_overlay_swaps_whole_when_a_run_ends() {
    let (_t, root, _) = scene("textures-swap");
    enable_in_settings(&root);
    let sent: Mutex<Vec<import::ImportStatus>> = Mutex::new(Vec::new());
    let send = |s: import::ImportStatus| sent.lock().unwrap().push(s);
    assert!(import::import_once(&root, &fake, 2, &send, false));
    let before = overlay(&root).unwrap();
    let old_model = staged_source(&root, "models/a.glb").unwrap();
    sent.lock().unwrap().clear();

    write(&root, "images/wall.png", &noise_png(64, 64, 9));
    let checks = std::sync::atomic::AtomicUsize::new(0);
    let check = || {
        checks.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let current = overlay(&root).unwrap();
        assert!(
            Arc::ptr_eq(&current, &before),
            "no part of the new run is applied"
        );
        let served = staged_source(&root, "models/a.glb").unwrap();
        assert_eq!(served, old_model);
        let model = gltf::Model::parse(&std::fs::read(&served).unwrap()).unwrap();
        for image in model.json["images"].as_array().unwrap() {
            let ktx2 = format!("models/{}", image["uri"].as_str().unwrap());
            assert!(current.added.contains(&ktx2), "{ktx2} is published");
            assert!(current.source(&ktx2).unwrap().is_file());
        }
    };
    let probe = Probe(&check);
    let encoder = || Ok::<&dyn Encoder, anyhow::Error>(&probe);
    assert!(import::import_once(&root, &encoder, 2, &send, false));
    assert_eq!(
        checks.load(std::sync::atomic::Ordering::Relaxed),
        1,
        "one texture changed"
    );

    let after = overlay(&root).unwrap();
    assert_ne!(*after, *before);
    let new_model = staged_source(&root, "models/a.glb").unwrap();
    assert_ne!(
        new_model, old_model,
        "a new run never rewrites the served file"
    );
    assert!(new_model.is_file());
    assert!(!old_model.exists(), "pruned after the swap");
    let sent = sent.lock().unwrap();
    assert_eq!(sent.first().unwrap().state, import::ImportState::Running);
    assert_eq!(sent.last().unwrap().state, import::ImportState::Ready);
    assert_eq!(
        sent.iter()
            .filter(|s| s.state != import::ImportState::Running)
            .count(),
        1,
        "one READY per run"
    );
    registry().remove(&root);
}

#[test]
fn the_import_encodes_nothing_with_the_setting_off() {
    let (_t, root, _) = scene("textures-off");
    let sent: Mutex<Vec<import::ImportStatus>> = Mutex::new(Vec::new());
    let send = |s: import::ImportStatus| sent.lock().unwrap().push(s);
    let untouched = || -> anyhow::Result<&'static dyn Encoder> {
        panic!("no encoder is asked for with the setting off")
    };
    assert!(!import::import_once(&root, &untouched, 2, &send, false));
    assert!(sent.lock().unwrap().is_empty(), "nothing to tell");
    assert!(!root.join(STAGE).join("cache").exists(), "nothing encoded");
    let rels = crate::deploy::collect_publishable_files(&root).unwrap();
    assert_eq!(publishable(&root, rels.clone()), rels);

    // turned on: the next run encodes and applies it
    enable_in_settings(&root);
    assert!(import::import_once(&root, &fake, 2, &send, false));
    let last = *sent.lock().unwrap().last().unwrap();
    assert!(last.enabled && last.state == import::ImportState::Ready);
    assert!(staged_source(&root, "models/a.glb").is_some());

    // turned off again: the overlay goes and clients are told once
    settings::apply(
        &root,
        Overrides {
            optimize_textures: Some(false),
            keep_source_textures: None,
        },
    )
    .unwrap();
    sent.lock().unwrap().clear();
    assert!(import::import_once(&root, &untouched, 2, &send, false));
    assert!(!import::import_once(&root, &untouched, 2, &send, false));
    let sent = sent.lock().unwrap();
    assert_eq!(sent.len(), 1);
    assert!(!sent[0].enabled && sent[0].state == import::ImportState::Ready);
    assert_eq!(staged_source(&root, "models/a.glb"), None);
    registry().remove(&root);
}

#[test]
fn an_import_that_cannot_start_fails_loudly() {
    let (_t, root, _) = scene("textures-fail");
    enable_in_settings(&root);
    let sent: Mutex<Vec<import::ImportStatus>> = Mutex::new(Vec::new());
    let send = |s: import::ImportStatus| sent.lock().unwrap().push(s);
    let broken = || -> anyhow::Result<&'static dyn Encoder> { anyhow::bail!("no encoder") };
    assert!(!import::import_once(&root, &broken, 2, &send, false));
    let sent = sent.lock().unwrap();
    assert_eq!(sent.len(), 1);
    assert_eq!(sent[0].state, import::ImportState::Failed);
    assert!(registry().get(&root).is_none(), "nothing swapped in");
}

/// The overlay as `deploy` sees it: flag off, the payload is the tree's;
/// flag on, the rewritten model's bytes, the KTX2 files, and no ignored
/// original — through the same `prepare` and `preview` every publish uses.
#[test]
fn deploy_payload_follows_the_overlay() {
    let (_t, root, glb) = scene("textures-deploy");
    let project = Project::load(&root).unwrap();
    let off = crate::deploy::prepare(&project).unwrap();
    let off_files: Vec<&str> = off.files.iter().map(|(f, _, _)| f.as_str()).collect();
    assert!(off_files.contains(&"images/wall.png"));
    assert!(!off_files.iter().any(|f| f.ends_with(".ktx2")));
    let model_off = off
        .files
        .iter()
        .find(|(f, _, _)| f == "models/a.glb")
        .unwrap();
    assert_eq!(model_off.2, glb);

    let out = run(
        &project.root,
        Settings::default(),
        &FakeEncoder { bloat: false },
    );
    swap(
        &project.root,
        Entry {
            enabled: true,
            settings: Settings::default(),
            overlay: Some(Arc::new(out.overlay.clone())),
            stamps: None,
        },
    );

    let on = crate::deploy::prepare(&project).unwrap();
    let on_files: BTreeSet<&str> = on.files.iter().map(|(f, _, _)| f.as_str()).collect();
    assert!(!on_files.contains("images/wall.png"));
    assert!(on_files.contains("images/ui.png"));
    for added in &out.overlay.added {
        assert!(on_files.contains(added.as_str()));
    }
    let model_on = on
        .files
        .iter()
        .find(|(f, _, _)| f == "models/a.glb")
        .unwrap();
    assert_eq!(
        model_on.2,
        std::fs::read(out.overlay.source("models/a.glb").unwrap()).unwrap()
    );
    let preview = crate::deploy::preview(&project).unwrap();
    let listed: BTreeSet<&str> = preview.files.iter().map(|(f, _)| f.as_str()).collect();
    assert_eq!(listed, on_files);

    registry().remove(&project.root);
    let again = crate::deploy::prepare(&project).unwrap();
    assert_eq!(
        again
            .files
            .iter()
            .map(|(f, h, _)| (f, h))
            .collect::<Vec<_>>(),
        off.files.iter().map(|(f, h, _)| (f, h)).collect::<Vec<_>>(),
        "with the project unregistered the payload is the tree's again"
    );
}

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
        "{\"main\":\"bin/index.js\",\"scene\":{\"parcels\":[\"0,0\"],\"base\":\"0,0\"}}",
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
    optimize(root, &rels, settings, encoder, &|rel| root.join(rel)).unwrap()
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
    assert!(o.added.iter().all(|a| a.starts_with("models/") && a.ends_with(".ktx2")));
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
    assert_eq!(
        o.removed,
        BTreeSet::from(["images/wall.png".to_string()])
    );
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
    assert!(root.join(STAGE).join("manifest.json").is_file());
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
    assert!(out.summary.kept.iter().all(|(_, why)| why.contains("not smaller")));
}

#[test]
fn the_overlay_edits_the_list_and_nothing_else() {
    let overlay = Overlay {
        sources: BTreeMap::from([
            ("m.glb".to_string(), PathBuf::from("/stage/m.glb")),
            ("optimized-textures/a.ktx2".to_string(), PathBuf::from("/stage/a.ktx2")),
        ]),
        added: BTreeSet::from(["optimized-textures/a.ktx2".to_string()]),
        removed: BTreeSet::from(["t.png".to_string()]),
    };
    let rels = vec!["scene.json".to_string(), "m.glb".to_string(), "t.png".to_string()];
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
    assert_eq!(settings(root), None);
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
    let model_off = off.files.iter().find(|(f, _, _)| f == "models/a.glb").unwrap();
    assert_eq!(model_off.2, glb);

    let out = run(&project.root, Settings::default(), &FakeEncoder { bloat: false });
    enable(&project.root, Settings::default());
    registry().get_mut(&project.root).unwrap().overlay = Some(Arc::new(out.overlay.clone()));

    let on = crate::deploy::prepare(&project).unwrap();
    let on_files: BTreeSet<&str> = on.files.iter().map(|(f, _, _)| f.as_str()).collect();
    assert!(!on_files.contains("images/wall.png"));
    assert!(on_files.contains("images/ui.png"));
    for added in &out.overlay.added {
        assert!(on_files.contains(added.as_str()));
    }
    let model_on = on.files.iter().find(|(f, _, _)| f == "models/a.glb").unwrap();
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
        again.files.iter().map(|(f, h, _)| (f, h)).collect::<Vec<_>>(),
        off.files.iter().map(|(f, h, _)| (f, h)).collect::<Vec<_>>(),
        "with the project unregistered the payload is the tree's again"
    );
}

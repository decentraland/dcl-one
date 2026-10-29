//! This crate's build against dcl-scene-cli's own (`build --built-in
//! --skip-type-check`), over dcl-scene-cli's golden fixtures: every file either
//! writes must match byte for byte once each scene's root path is masked. A
//! development build inlines the generated entrypoint, which imports the
//! scene's `src/index.ts` by absolute path, into its base64 sourcemap, so that
//! is decoded before masking.
//!
//! dcl-scene-cli gets the full vendored node_modules; this crate gets only the
//! `@dcl/sdk` it embeds, which is all the browser will ever give it.
//!
//! `testdata/sdk7-scene-template` holds the build inputs of
//! decentraland/sdk7-scene-template at 7c2a0285, the scene every new project
//! starts from; it is built in both modes.

use dcl_scene_cli::build::BuildOptions;
use dcl_scene_build::{MainCrdt, Options, build, install_sdk};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

const GENERATED: &[&str] = &[
    ".dcl-one/entrypoint.ts",
    ".dcl-one/all-composites.js",
    ".dcl-one/sdk-boot.js",
];

fn manifest_dir() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
}

fn golden(name: &str) -> PathBuf {
    PathBuf::from(env!("DCL_ONE_ROOT"))
        .join("apps/dcl-scene-cli/testdata/golden")
        .join(name)
}

/// Copy a scene's sources, skipping what a build writes or installs.
fn copy_sources(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for entry in std::fs::read_dir(from).unwrap().flatten() {
        let name = entry.file_name();
        if matches!(
            name.to_str(),
            Some("node_modules" | "bin" | ".dcl-one" | ".git" | "main.crdt")
        ) {
            continue;
        }
        let src = entry.path();
        let dst = to.join(&name);
        if src.is_dir() {
            copy_sources(&src, &dst);
        } else {
            std::fs::copy(&src, &dst).unwrap();
        }
    }
}

fn fresh(dir: &Path) -> PathBuf {
    let _ = std::fs::remove_dir_all(dir);
    std::fs::create_dir_all(dir).unwrap();
    dir.canonicalize().unwrap()
}

const INLINE_MAP: &str = "\n//# sourceMappingURL=data:application/json;charset=utf-8;base64,";

/// A development chunk's inline sourcemap, decoded so the root mask reaches it.
fn decode_inline_map(text: String) -> String {
    use base64::Engine;
    let Some(at) = text.rfind(INLINE_MAP) else {
        return text;
    };
    let encoded = text[at + INLINE_MAP.len()..].trim_end();
    let map = base64::engine::general_purpose::STANDARD
        .decode(encoded)
        .expect("inline sourcemap is base64");
    format!(
        "{}\n//# sourceMappingURL (decoded) {}",
        &text[..at],
        String::from_utf8(map).expect("inline sourcemap is UTF-8")
    )
}

/// Every file under `root` but node_modules, masked for the root path.
fn snapshot(root: &Path) -> BTreeMap<String, Vec<u8>> {
    fn walk(root: &Path, dir: &Path, out: &mut BTreeMap<String, Vec<u8>>) {
        for entry in std::fs::read_dir(dir).unwrap().flatten() {
            let path = entry.path();
            if entry.file_name() == "node_modules" {
                continue;
            }
            if path.is_dir() {
                walk(root, &path, out);
                continue;
            }
            let rel = path
                .strip_prefix(root)
                .unwrap()
                .to_string_lossy()
                .replace('\\', "/");
            out.insert(rel, std::fs::read(&path).unwrap());
        }
    }
    let mut out = BTreeMap::new();
    walk(root, root, &mut out);
    let needle = root.display().to_string();
    out.into_iter()
        .map(|(rel, bytes)| match String::from_utf8(bytes) {
            Ok(text) => (
                rel,
                decode_inline_map(text)
                    .replace(&needle, "<root>")
                    .into_bytes(),
            ),
            Err(e) => (rel, e.into_bytes()),
        })
        .collect()
}

fn check(name: &str, sources: &Path, staged: &[(PathBuf, &str)], production: bool) {
    let mode = if production {
        "production"
    } else {
        "development"
    };
    let work = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join("parity")
        .join(format!("{name}.{mode}"));
    let native = fresh(&work.join("dcl-scene-cli"));
    let ours = fresh(&work.join("scene-build"));
    for root in [&native, &ours] {
        copy_sources(sources, root);
        for (from, to) in staged {
            let dst = root.join(to);
            std::fs::create_dir_all(dst.parent().unwrap()).unwrap();
            std::fs::copy(from, dst).unwrap();
        }
    }

    dcl_scene_cli::init::init(&dcl_scene_cli::init::InitOptions {
        dir: native.clone(),
        project: None,
        yes: true,
        node_modules_only: true,
    })
    .expect("extracting the vendored node_modules");
    tokio::runtime::Runtime::new()
        .unwrap()
        .block_on(dcl_scene_cli::build::build(&BuildOptions {
            built_in: true,
            dir: native.clone(),
            production,
            ignore_composite: false,
            custom_entry_point: false,
            skip_type_check: true,
            out_root: None,
            quiet: true,
        }))
        .unwrap_or_else(|e| panic!("{name}.{mode}: dcl-scene-cli failed: {e:#}"));

    assert!(install_sdk(&ours).unwrap());
    let report = build(
        &ours,
        &Options {
            production,
            ignore_composite: false,
        },
    )
    .unwrap_or_else(|e| panic!("{name}.{mode}: scene-build failed: {e:#}"));
    assert!(
        report.unresolved.is_empty(),
        "{name}.{mode}: {:?}",
        report.unresolved
    );
    assert!(
        report.warnings.is_empty(),
        "{name}.{mode}: {:?}",
        report.warnings
    );

    let mut want = snapshot(&native);
    let got = snapshot(&ours);
    // dcl-scene-cli's own bookkeeping, and the files only its source-path SDK
    // chunk reads.
    for rel in [
        ".dcl-one/split",
        ".dcl-one/script-utils.js",
        ".dcl-one/composite-slot.js",
        ".dcl-one/sdk-runtime-entry.js",
    ] {
        want.remove(rel);
    }
    if let MainCrdt::Unsupported(why) = &report.main_crdt {
        panic!("{name}.{mode}: dcl-scene-cli needed its node data-layer for main.crdt: {why}");
    }
    let differ: Vec<&String> = want
        .keys()
        .chain(got.keys())
        .filter(|rel| want.get(*rel) != got.get(*rel))
        .collect();
    assert!(differ.is_empty(), "{name}.{mode}: differs in {differ:?}");
    for rel in &report.outputs {
        assert!(
            got.contains_key(rel),
            "{name}.{mode}: reported {rel} was not written"
        );
    }
    for rel in GENERATED {
        assert!(got.contains_key(*rel), "{name}.{mode}: {rel} missing");
    }
}

fn fixture(name: &str, production: bool) {
    check(name, &golden(name), &[], production);
}

#[test]
fn cube_production() {
    fixture("cube", true);
}

#[test]
fn cube_development() {
    fixture("cube", false);
}

#[test]
fn composite_production() {
    let opera = PathBuf::from(env!("DCL_ONE_ROOT"))
        .join("library/dcl-scene-authoring/testdata/opera-main.composite");
    check(
        "composite",
        &golden("composite"),
        &[(opera, "assets/scene/main.composite")],
        true,
    );
}

#[test]
fn dynamic_import_production() {
    fixture("dynamic-import", true);
}

#[test]
fn ui_production() {
    fixture("ui", true);
}

#[test]
fn smart_item_production() {
    fixture("smart-item", true);
}

#[test]
fn smart_item_development() {
    fixture("smart-item", false);
}

fn template(production: bool) {
    let sources = manifest_dir().join("testdata/sdk7-scene-template");
    check("sdk7-scene-template", &sources, &[], production);
}

#[test]
fn template_production() {
    template(true);
}

#[test]
fn template_development() {
    template(false);
}

use std::process::Command;

#[test]
fn split_loader_decodes_utf8_in_explorer_sandboxes() {
    if Command::new("node").arg("--version").output().is_err() {
        eprintln!("skipping: node is not on PATH");
        return;
    }
    let output = Command::new("node")
        .args(["--test", "scripts/loader-utf8.test.mjs"])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .env_remove("LOADER_TEMPLATE")
        .output()
        .expect("run the split-loader UTF-8 regression tests");
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

/// A production build writes the loader through `compact_loader`: the same
/// suites must pass on that text.
#[test]
fn the_compact_loader_passes_the_loader_suites() {
    if Command::new("node").arg("--version").output().is_err() {
        eprintln!("skipping: node is not on PATH");
        return;
    }
    let template = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/src/templates/split-loader.js"
    ))
    .unwrap();
    let dir = std::env::temp_dir().join(format!(
        "dcl-scene-cli-compact-loader-{}",
        std::process::id()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let compact = dir.join("split-loader.js");
    std::fs::write(&compact, dcl_scene_cli::split::compact_loader(&template)).unwrap();
    let output = Command::new("node")
        .args([
            "--test",
            "scripts/loader-utf8.test.mjs",
            "scripts/loader-mp.test.mjs",
        ])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .env("LOADER_TEMPLATE", &compact)
        .output()
        .expect("run the split-loader suites on the compact loader");
    let _ = std::fs::remove_dir_all(&dir);
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

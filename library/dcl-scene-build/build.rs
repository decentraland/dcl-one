//! Embeds `node_modules/@dcl/sdk` from dcl-scene-cli's vendored node_modules:
//! the prebuilt runtime chunks a build installs and the tsconfig a scene
//! extends. The wasm build then carries the exact SDK it was built against, so
//! a browser feeds it only the project's own files.

use std::io::Read;
use std::path::PathBuf;

const VENDORED: &str = "apps/dcl-scene-cli/src/vendor/node_modules.zip";
const PREFIX: &str = "node_modules/@dcl/sdk/";

fn main() {
    // The repository root, from the [env] table of the root .cargo/config.toml.
    let root = PathBuf::from(std::env::var("DCL_ONE_ROOT").expect("DCL_ONE_ROOT"));
    println!("cargo:rerun-if-changed={}", root.join(VENDORED).display());
    let out_dir = PathBuf::from(std::env::var("OUT_DIR").expect("OUT_DIR"));
    let zip =
        std::fs::File::open(root.join(VENDORED)).expect("opening the vendored node_modules.zip");
    let mut archive = zip::ZipArchive::new(zip).expect("reading the vendored node_modules.zip");
    let mut entries = Vec::new();
    for i in 0..archive.len() {
        let mut file = archive.by_index(i).expect("reading a zip entry");
        let name = file.name().expect("a UTF-8 zip entry name").to_string();
        if !file.is_file() || !name.starts_with(PREFIX) {
            continue;
        }
        let mut bytes = Vec::new();
        file.read_to_end(&mut bytes).expect("inflating a zip entry");
        let dst = out_dir.join("sdk").join(&name);
        std::fs::create_dir_all(dst.parent().expect("entry parent")).expect("creating OUT_DIR/sdk");
        std::fs::write(&dst, bytes).expect("writing an embedded SDK file");
        entries.push(name);
    }
    assert!(
        entries.iter().any(|n| n.ends_with("/prebuilt/core.js")),
        "the vendored node_modules.zip has no prebuilt @dcl/sdk core chunk"
    );
    entries.sort();
    let table: String = entries
        .iter()
        .map(|name| {
            format!(
                "    ({name:?}, include_bytes!(concat!(env!(\"OUT_DIR\"), \"/sdk/{name}\"))),\n"
            )
        })
        .collect();
    std::fs::write(
        out_dir.join("sdk_files.rs"),
        format!("pub const SDK_FILES: &[(&str, &[u8])] = &[\n{table}];\n"),
    )
    .expect("writing sdk_files.rs");
}

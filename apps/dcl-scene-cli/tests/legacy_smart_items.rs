//! The loader's `@dcl/asset-packs` specifier mapping (equal to
//! `split::smart_items_spec`): a Creator Hub scene's legacy import resolves to
//! the `@dcl/smart-items` keys the smart chunk publishes.

use dcl_scene_cli::split;
use std::path::{Path, PathBuf};
use std::process::Command;

fn src(rel: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("src").join(rel)
}

struct Tmp(PathBuf);

impl Tmp {
    fn new(tag: &str) -> Self {
        let dir = std::env::temp_dir().join(format!(
            "dcl-scene-cli-legacy-smart-items-{tag}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        Tmp(dir)
    }

    fn write(&self, rel: &str, contents: impl AsRef<[u8]>) -> PathBuf {
        let path = self.0.join(rel);
        std::fs::write(&path, contents).unwrap();
        path
    }
}

impl Drop for Tmp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn node() -> Option<Command> {
    if Command::new("node").arg("--version").output().is_err() {
        eprintln!("skipping: node is not on PATH");
        return None;
    }
    Some(Command::new("node"))
}

fn run(mut node: Command, script: &Path, args: &[&Path]) -> String {
    let output = node
        .arg(script)
        .args(args)
        .output()
        .unwrap_or_else(|e| panic!("running {}: {e}", script.display()));
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap()
}

const RUN_LOADER: &str = r#"
import vm from 'node:vm'
import { readFileSync } from 'node:fs'
const template = readFileSync(process.argv[2], 'utf8')
const context = vm.createContext({ module: { exports: {} }, TextDecoder })
vm.runInContext(
  template
    .replaceAll('__DCL_SCENE_CLI_CHUNK__', 'sdk.js')
    .replaceAll('__DCL_ONE_SCENE_CHUNK__', 'scene.js')
    .replaceAll('__DCL_ONE_SMART_CHUNK__', '')
    .replaceAll('__DCL_ONE_MAX_COMPOSITE_ENTITY__', '0')
    .replaceAll('__DCL_ONE_MP__', 'false'),
  context
)
const specs = JSON.parse(readFileSync(process.argv[3], 'utf8'))
const mapped = {}
for (const spec of specs) mapped[spec] = context.__dclOneSmartItemsSpec(spec)
const registry = {
  '@dcl/smart-items': 'root',
  '@dcl/smart-items/helpers': 'helpers',
  '@dcl/smart-items/admin-toolkit-ui/index': 'ui'
}
const req = context.__dclOneMakeRequire(registry, (spec) => 'host:' + spec)
const required = {
  '@dcl/asset-packs': req('@dcl/asset-packs'),
  '@dcl/asset-packs/dist/scene-entrypoint.js': req('@dcl/asset-packs/dist/scene-entrypoint.js'),
  '@dcl/asset-packs/dist/helpers': req('@dcl/asset-packs/dist/helpers'),
  '@dcl/asset-packs/dist/admin-toolkit-ui/index.js': req('@dcl/asset-packs/dist/admin-toolkit-ui/index.js'),
  '~system/Runtime': req('~system/Runtime')
}
let missing = 'resolved'
try {
  req('@dcl/asset-packs/dist/nope')
} catch (e) {
  missing = e.message
}
console.log(JSON.stringify({ mapped, required, missing }))
"#;

const SPECS: [&str; 12] = [
    "@dcl/asset-packs",
    "@dcl/asset-packs/dist/scene-entrypoint",
    "@dcl/asset-packs/dist/scene-entrypoint.js",
    "@dcl/asset-packs/dist/",
    "@dcl/asset-packs/dist/helpers",
    "@dcl/asset-packs/dist/helpers.js",
    "@dcl/asset-packs/dist/admin-toolkit-ui/ModerationControl/api",
    "@dcl/asset-packs/dist/index",
    "@dcl/asset-packs/package.json",
    "@dcl/smart-items/actions",
    "@dcl/sdk",
    "~system/Runtime",
];

/// The loader maps `@dcl/asset-packs` specifiers the way `split::smart_items_spec`
/// does, and serves them from the smart chunk's registry.
#[test]
fn the_loader_maps_legacy_smart_items_specifiers_like_the_cli() {
    let Some(node) = node() else { return };
    let tmp = Tmp::new("loader");
    let script = tmp.write("run.mjs", RUN_LOADER);
    let specs = tmp.write("specs.json", serde_json::to_string(&SPECS).unwrap());
    let out = run(node, &script, &[&src("templates/split-loader.js"), &specs]);
    let js: serde_json::Value = serde_json::from_str(&out).unwrap();
    for spec in SPECS {
        assert_eq!(
            js["mapped"][spec],
            split::smart_items_spec(spec).as_ref(),
            "{spec}"
        );
    }
    let required = &js["required"];
    assert_eq!(required["@dcl/asset-packs"], "root");
    assert_eq!(
        required["@dcl/asset-packs/dist/scene-entrypoint.js"],
        "root"
    );
    assert_eq!(required["@dcl/asset-packs/dist/helpers"], "helpers");
    assert_eq!(
        required["@dcl/asset-packs/dist/admin-toolkit-ui/index.js"],
        "ui"
    );
    assert_eq!(required["~system/Runtime"], "host:~system/Runtime");
    let missing = js["missing"].as_str().unwrap();
    assert!(
        missing.contains("\"@dcl/smart-items/nope\" is not in the sdk runtime registry"),
        "{missing}"
    );
}

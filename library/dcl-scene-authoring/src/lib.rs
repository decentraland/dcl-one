//! The filesystem-free core of the SDK7 scene toolchain: composite
//! normalization, jsonSchema CRDT encoding, native `main.crdt` and
//! `entity-names.ts` generation, the production form of the loader stub.
//! Everything here reads scene files through [`SceneFiles`] and touches
//! neither `std::fs` nor the network, so the same code serves `dcl-scene-cli`
//! on the desktop and a browser web worker on `wasm32-unknown-unknown`.

pub mod composite_norm;
pub mod crdt_gen;
pub mod entity_names;
pub mod jsjson;
pub mod schema_crdt;

use std::collections::BTreeMap;

/// A composite larger than this is refused rather than parsed; the native
/// walker applies the cap before a path ever reaches [`SceneFiles::list`].
pub const COMPOSITE_FILE_MAX_BYTES: u64 = 16 * 1024 * 1024;

/// A scene's files as the authoring core sees them: relative, forward-slash
/// paths under the project root, and their bytes. Implementations prune what
/// the native walker prunes (see [`is_pruned_dir`]) and may drop files they
/// refuse to load; `find_composites` re-applies the name rules so an
/// in-memory map that ships `node_modules` still behaves like the disk walk.
pub trait SceneFiles {
    /// Every file under the root, in any order.
    fn list(&self) -> Vec<String>;
    /// The bytes of one listed file; None when it is missing or unreadable.
    fn read(&self, rel: &str) -> Option<Vec<u8>>;
}

impl SceneFiles for BTreeMap<String, Vec<u8>> {
    fn list(&self) -> Vec<String> {
        self.keys().cloned().collect()
    }

    fn read(&self, rel: &str) -> Option<Vec<u8>> {
        self.get(rel).cloned()
    }
}

/// Directories the composite walk never enters: dot-entries and the three
/// build/dependency trees.
pub fn is_pruned_dir(name: &str) -> bool {
    name.starts_with('.') || matches!(name, "node_modules" | "bin" | "dist")
}

/// Is `rel` a composite the walk would have found: a `.composite` file, not
/// dot-prefixed, with no pruned directory on its path?
pub fn is_composite_path(rel: &str) -> bool {
    let mut segments = rel.split('/').peekable();
    while let Some(segment) = segments.next() {
        let is_file = segments.peek().is_none();
        if is_file {
            return segment.ends_with(".composite") && !segment.starts_with('.');
        }
        if is_pruned_dir(segment) {
            return false;
        }
    }
    false
}

/// Every composite the scene ships, ordered like the native walk sorted its
/// paths (component by component, so `a/x` sorts before `a-b/x`); later
/// entries override earlier ones wherever the core instances them.
pub fn find_composites(files: &dyn SceneFiles) -> Vec<String> {
    let mut out: Vec<String> = files
        .list()
        .into_iter()
        .filter(|rel| is_composite_path(rel))
        .collect();
    out.sort_by(|a, b| a.split('/').cmp(b.split('/')));
    out
}

/// A composite's text, or why it could not be loaded.
pub(crate) fn read_composite_text(files: &dyn SceneFiles, rel: &str) -> Result<String, String> {
    let bytes = files
        .read(rel)
        .ok_or_else(|| format!("reading {rel}: not readable"))?;
    String::from_utf8(bytes).map_err(|e| format!("reading {rel}: {e}"))
}

/// `.dcl-one/sdk-boot.js`, which the generated entrypoint imports before the scene.
pub const SDK_BOOT: &str = "import { engine, setCompositeProvider } from '@dcl/sdk/ecs'\n\
                        import { compositeProvider } from '@dcl/sdk/composite-provider'\n\
                        setCompositeProvider(engine, compositeProvider)\n";

/// What the generated entrypoint needs to start the scene's scripts.
#[derive(Default)]
pub struct Scripts {
    /// `import * as script_<path> from "<root>/<path>"`, one line for each script file.
    imports: String,
    /// The second argument of `_initializeScripts`, with its comma: the
    /// instances to start. Empty when the scene has no script.
    argument: String,
    /// The composites that cannot be instanced. Their components are ignored.
    pub skipped: Vec<String>,
}

impl Scripts {
    /// Whether the scene has no script to start.
    pub fn is_empty(&self) -> bool {
        self.imports.is_empty()
    }

    /// The generated entrypoint of a split build: the scene's `main`, after
    /// its scripts. Its comments are part of the output: a development build
    /// inlines this file's source into the scene chunk's sourcemap.
    pub fn entrypoint(&self, safe_entry: &str, editor_scene: bool) -> String {
        // The scene chunk holds the script modules: `~sdk/script-utils` is in a
        // chunk that no scene owns, so the scene gives it the instances to start.
        let composite_fill = format!(
            "import {{ compositeFromLoader as __sceneComposites }} from './all-composites.js'\nObject.assign(compositeFromLoader, __sceneComposites)\n{}",
            self.imports
        );
        let script_instances = &self.argument;
        let editor_block = if editor_scene {
            "\nimport { syncEntity } from '@dcl/sdk/network'\nimport players from '@dcl/sdk/players'\nimport { setCompositeProvider } from '@dcl/sdk/ecs'\nimport { initAssetPacks } from '@dcl/asset-packs/dist/scene-entrypoint'\nsetCompositeProvider(engine, compositeProvider)\ninitAssetPacks(engine, { syncEntity }, players)\n"
                .to_string()
        } else {
            "false".to_string()
        };
        format!(
            r#"// BEGIN AUTO GENERATED CODE "~sdk/scene-entrypoint"
"use strict";
import {{ engine, NetworkEntity }} from '@dcl/sdk/ecs'
import * as sdk from '@dcl/sdk'
import {{ compositeProvider }} from '@dcl/sdk/composite-provider'
import {{ compositeFromLoader }} from '~sdk/all-composites'
import {{ _initializeScripts }} from '~sdk/script-utils'
// Registers the composite provider, and must be imported BEFORE the scene.
// A scene that calls initAssetPacks() at module scope -- as the Creator Hub
// templates do -- reads that provider while its own module body runs, and until
// this existed the only thing registering it was `@dcl/sdk`'s body, which the
// bundler emits at the END of the chunk. So the editor logged "[asset-packs] No
// SDK composite provider registered; SPAWN_ENTITY cannot resolve composites"
// and placing a smart item resolved nothing. Upstream sdk-commands has the same
// ordering (logic/bundle.ts) and gets away with it only because upstream scenes
// never call initAssetPacks themselves.
//
// A separate module because neither cheaper fix works: statements here run after
// every import including the scene's, and the bundler does not honour moving
// `@dcl/sdk` earlier -- a side-effect import is shaken to a bare require and the
// re-export below then reads a `_dcl_sdk` binding that was never declared, while
// `export * from` is emitted at the end wherever it is written.
import './sdk-boot.js'
import * as entrypoint from {safe_entry}
{composite_fill}
{editor_block}

// `console.error(e)` drops the frames, and the host captures its own stack at
// the call site — this catch block. `e.stack` carries the throw site, so it is
// a superset of what upstream prints.
function __reportSceneError(e: any) {{
  console.error(e && e.stack ? e.stack : e)
}}

if ((entrypoint as any).main !== undefined) {{
  function _INTERNAL_startup_system() {{
    try {{
      _initializeScripts(engine{script_instances})

      const maybePromise = (entrypoint as any).main()
      if (maybePromise && typeof maybePromise === 'object' && typeof (maybePromise as unknown as Promise<unknown>).then === 'function') {{
        maybePromise.catch(__reportSceneError)
      }}
    }} catch (e) {{
      __reportSceneError(e)
    }} finally {{
      engine.removeSystem(_INTERNAL_startup_system)
    }}
  }}
  engine.addSystem(_INTERNAL_startup_system, Infinity)
}}

export * from '@dcl/sdk'
export * from {safe_entry}
export * from '~sdk/script-utils'
"#
        )
    }
}

/// `@dcl/sdk-commands` `getAllComposites` and `collectScriptData`: each entry
/// of an `asset-packs::Script` value is one instance of that file's module on
/// its entity, and the instances of one file stay together. `root` is the
/// scene folder as the bundler sees it.
pub fn scripts(files: &dyn SceneFiles, root: &str) -> Scripts {
    use serde_json::Value;
    let mut normalizer = composite_norm::CompositeNormalizer::new();
    let mut out = Scripts::default();
    let mut modules: Vec<(String, Vec<Value>)> = Vec::new();
    for rel in find_composites(files) {
        let composite = read_composite_text(files, &rel)
            .map_err(anyhow::Error::msg)
            .and_then(|raw| normalizer.normalize(&raw))
            .and_then(|json| Ok(serde_json::from_str::<Value>(&json)?));
        let composite = match composite {
            Ok(composite) => composite,
            Err(err) => {
                out.skipped.push(format!(
                    "composite {rel} can't be instanced, so its components are ignored: {err:#}"
                ));
                continue;
            }
        };
        let components = composite["components"].as_array().into_iter().flatten();
        for component in components.filter(|c| c["name"] == "asset-packs::Script") {
            for (entity, entry) in component["data"].as_object().into_iter().flatten() {
                for script in entry["json"]["value"].as_array().into_iter().flatten() {
                    let (Some(path), Ok(entity)) = (script["path"].as_str(), entity.parse::<u64>())
                    else {
                        continue;
                    };
                    let mut instance = script.clone();
                    instance["entity"] = entity.into();
                    match modules.iter_mut().find(|(file, _)| file == path) {
                        Some((_, instances)) => instances.push(instance),
                        None => modules.push((path.to_owned(), vec![instance])),
                    }
                }
            }
        }
    }
    let mut lines = Vec::new();
    for (path, instances) in &modules {
        let stem = path.strip_suffix(".tsx").or(path.strip_suffix(".ts"));
        let name = stem
            .unwrap_or(path)
            .replace(|c: char| !c.is_ascii_alphanumeric(), "_");
        let file = Value::from(format!("{root}/{path}").replace('\\', "/"));
        out.imports += &format!("import * as script_{name} from {file}\n");
        for instance in instances {
            lines.push(format!("  {{ ...{instance}, module: script_{name} }}"));
        }
    }
    if !lines.is_empty() {
        out.argument = format!(", [\n{}\n]", lines.join(",\n"));
    }
    out
}

/// The loader stub (`main` of a built scene) without its comment lines,
/// blank lines and indentation, for a production build. `dcl-scene-cli` and
/// `dcl-scene-build` both write it, so their outputs agree. Nothing is
/// renamed and no line is joined: the chunks are direct-eval'd inside this
/// scope, so its identifiers are part of the contract, and the statements
/// rely on their line ends.
pub fn compact_loader(stub: &str) -> String {
    let mut out = String::with_capacity(stub.len());
    for line in stub.lines().map(str::trim) {
        if line.is_empty() || line.starts_with("//") {
            continue;
        }
        out.push_str(line);
        out.push('\n');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scene(paths: &[&str]) -> BTreeMap<String, Vec<u8>> {
        paths.iter().map(|p| (p.to_string(), Vec::new())).collect()
    }

    #[test]
    fn walk_rules_match_the_native_walker() {
        let files = scene(&[
            "main.composite",
            "assets/scene/main.composite",
            "assets/scene/.hidden.composite",
            "node_modules/pkg/x.composite",
            "bin/x.composite",
            "dist/x.composite",
            ".dcl-one/x.composite",
            "assets/other.json",
        ]);
        assert_eq!(
            find_composites(&files),
            vec!["assets/scene/main.composite", "main.composite"]
        );
    }

    #[test]
    fn scripts_keep_one_file_together_and_skip_a_bad_composite() {
        use serde_json::json;
        let none = scripts(&scene(&[]), "/scene");
        assert!(none.imports.is_empty() && none.argument.is_empty() && none.skipped.is_empty());

        let composite = |first: &str, data| {
            let script = json!({"name": "asset-packs::Script", "jsonSchema": {"type": "object"}, "data": data});
            json!({"version": 1, "components": [{"name": first, "data": {}}, script]}).to_string()
        };
        let a = json!({
            "513": {"json": {"value": [{"path": "s/B.ts", "priority": 1}]}},
            "512": {"json": {"value": [{"path": "s/A.tsx", "priority": 0}, {"path": "s/B.ts", "priority": 0}]}},
        });
        let b = json!({"514": {"json": {"value": [{"path": "s/C.ts", "priority": 0}]}}});
        let files: BTreeMap<String, Vec<u8>> = [
            (
                "a.composite".to_string(),
                composite("core::Transform", a).into(),
            ),
            (
                "b.composite".to_string(),
                composite("core::NotAThing", b).into(),
            ),
        ]
        .into();
        let found = scripts(&files, "/scene");
        assert_eq!(
            found.imports,
            "import * as script_s_A from \"/scene/s/A.tsx\"\nimport * as script_s_B from \"/scene/s/B.ts\"\n"
        );
        assert_eq!(
            found.argument,
            r#", [
  { ...{"path":"s/A.tsx","priority":0,"entity":512}, module: script_s_A },
  { ...{"path":"s/B.ts","priority":0,"entity":512}, module: script_s_B },
  { ...{"path":"s/B.ts","priority":1,"entity":513}, module: script_s_B }
]"#
        );
        assert_eq!(found.skipped.len(), 1);
        assert!(found.skipped[0].contains("b.composite can't be instanced"));
    }

    #[test]
    fn ordering_is_per_path_component() {
        let files = scene(&["a-b/x.composite", "a/x.composite"]);
        assert_eq!(
            find_composites(&files),
            vec!["a/x.composite", "a-b/x.composite"]
        );
    }
}

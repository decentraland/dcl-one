use crate::scene::Project;
use crate::ux::{TrySteps, UserError, write_error};
use anyhow::Result;
use dcl_scene_authoring::composite_norm::smart_item_name;
use std::path::{Path, PathBuf};

pub struct Generated {
    pub dir: PathBuf,
    pub entrypoint: PathBuf,
    pub max_composite_entity: u32,
}

fn write_in(dir: &Path, name: &str, content: impl AsRef<[u8]>) -> Result<PathBuf> {
    let path = dir.join(name);
    std::fs::write(&path, content).map_err(|e| write_error(&path, e))?;
    Ok(path)
}

pub fn generate(
    project: &Project,
    ignore_composite: bool,
    custom_entry: bool,
    split: bool,
) -> Result<Generated> {
    let dir = crate::scene::work_dir(&project.root)
        .map_err(|e| write_error(&project.root.join(".dcl-one"), e))?;

    let user_entry = project.root.join("src/index.ts");
    let safe_entry = serde_json::to_string(&user_entry.display().to_string().replace('\\', "/"))?;

    let content = if custom_entry {
        format!(";\"use strict\";export * from {safe_entry}")
    } else {
        write_all_composites(project, &dir, ignore_composite)?;
        write_in(
            &dir,
            "script-utils.js",
            script_utils_content(project, ignore_composite),
        )?;
        write_in(&dir, "sdk-boot.js", SDK_BOOT)?;
        entrypoint_code(&safe_entry, project.is_editor_scene(), split)
    };
    let entrypoint = write_in(&dir, "entrypoint.ts", content)?;

    let max_composite_entity = if ignore_composite {
        0
    } else {
        scan_max_composite_entity(&project.root)
    };
    Ok(Generated {
        dir,
        entrypoint,
        max_composite_entity,
    })
}

/// scene.json's documented activation flag for the authoritative-server
/// surface (docs/multiplayer-server-design.md): with it, the loader tells the
/// sdk chunk to trust only the authoritative server and re-labels the
/// preview host's frames as that server, and `start` attaches the host.
pub fn authoritative_multiplayer(project: &Project) -> bool {
    project
        .scene_json
        .get("authoritativeMultiplayer")
        .and_then(|v| v.as_bool())
        == Some(true)
}

const SDK_BOOT: &str = "import { engine, setCompositeProvider } from '@dcl/sdk/ecs'\n\
                        import { compositeProvider } from '@dcl/sdk/composite-provider'\n\
                        setCompositeProvider(engine, compositeProvider)\n";

fn entrypoint_code(safe_entry: &str, editor_scene: bool, split: bool) -> String {
    let composite_fill = if split {
        "import { compositeFromLoader as __sceneComposites } from './all-composites.js'\nObject.assign(compositeFromLoader, __sceneComposites)\n"
    } else {
        ""
    };
    let editor_block = if editor_scene {
        "\nimport { syncEntity } from '@dcl/sdk/network'\nimport players from '@dcl/sdk/players'\nimport { setCompositeProvider } from '@dcl/sdk/ecs'\nimport { initSmartItems } from '@dcl/smart-items'\nsetCompositeProvider(engine, compositeProvider)\ninitSmartItems(engine, { syncEntity }, players)\n"
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
// A scene that calls initSmartItems() at module scope -- as the Creator Hub
// templates do -- reads that provider while its own module body runs, and until
// this existed the only thing registering it was `@dcl/sdk`'s body, which the
// bundler emits at the END of the chunk. So the editor logged "No SDK composite
// provider registered; SPAWN_ENTITY cannot resolve composites" and placing a
// smart item resolved nothing. Upstream's bundler has the same ordering
// (logic/bundle.ts) and gets away with it only because upstream scenes never
// call initSmartItems themselves.
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
      _initializeScripts(engine)

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

/// Every composite under `root`, sorted; the walk itself is
/// [`crate::scene_files::SceneDir`]'s.
pub fn find_composites(root: &Path) -> Vec<PathBuf> {
    dcl_scene_authoring::find_composites(&crate::scene_files::SceneDir::new(root))
        .into_iter()
        .map(|rel| root.join(rel))
        .collect()
}

fn write_all_composites(project: &Project, dir: &Path, ignore: bool) -> Result<()> {
    let mut lines = Vec::new();
    if !ignore {
        let mut normalizer = dcl_scene_authoring::composite_norm::CompositeNormalizer::new();
        for path in find_composites(&project.root) {
            let rel = path
                .strip_prefix(&project.root)
                .unwrap_or(&path)
                .to_string_lossy()
                .replace('\\', "/");
            if rel != "main.composite" {
                continue;
            }
            let normalized = std::fs::read_to_string(&path)
                .map_err(anyhow::Error::from)
                .and_then(|raw| normalizer.normalize(&raw));
            match normalized {
                Ok(json) => lines.push(format!("'{rel}':{json}")),
                Err(err) => {
                    return Err(UserError::new(
                        format!("{rel} could not be loaded, so the scene would build with no content from the editor"),
                        TrySteps::one(format!("fix {rel} — the underlying error is below"))
                            .and("or build without it on purpose: dcl-scene-cli build --ignoreComposite"),
                    )
                    .why(format!("{err:#}"))
                    .into());
                }
            }
        }
    }
    let content = format!("export const compositeFromLoader = {{{}}}", lines.join(","));
    write_in(dir, "all-composites.js", content)?;
    Ok(())
}

pub fn scan_max_composite_entity(root: &Path) -> u32 {
    let mut max = 0u32;
    for path in find_composites(root) {
        let Ok(raw) = std::fs::read(&path) else {
            continue;
        };
        let Ok(json) = serde_json::from_slice::<serde_json::Value>(&raw) else {
            continue;
        };
        let comps = json
            .get("components")
            .and_then(|c| c.as_array())
            .map(Vec::as_slice)
            .unwrap_or(&[]);
        for comp in comps {
            let Some(data) = comp.get("data").and_then(|d| d.as_object()) else {
                continue;
            };
            for key in data.keys() {
                if let Ok(id) = key.parse::<u64>() {
                    max = max.max((id & 0xffff) as u32);
                }
            }
        }
    }
    max
}

/// The `~sdk/script-utils` no-op a scene without the smart-item runtime links
/// against; the generated entrypoint calls `_initializeScripts` unconditionally.
/// Same export surface as upstream's `generateScriptStubModuleContent`.
pub const SCRIPT_UTILS_STUB: &str = "export function _initializeScripts(_engine) {}\nexport function getScriptInstance(_entity, _scriptPath) { return null }\nexport function getScriptInstancesByPath(_scriptPath) { return [] }\nexport function getAllScriptInstances(_entity) { return [] }\nexport function callScriptMethod(entity, scriptPath, methodName, ..._args) {\n  console.error(`Method ${methodName} not found on script ${scriptPath} for entity ${entity}`)\n  return undefined\n}\n";

/// The real `~sdk/script-utils`: the script runtime of
/// `@dcl/smart-items/scripts` behind the stub's export surface, with the
/// scene's `_initializeScripts` in front of it. The prebuilt smart chunk
/// bundles this module (`prebuilt::build_chunks`), so a split build gets it
/// from that chunk's registry; the source-path SDK chunk bundles the generated
/// file.
pub const SCRIPT_UTILS_SOURCE: &str = "import { runScripts, getScriptInstance, getScriptInstancesByPath, getAllScriptInstances, callScriptMethod } from '@dcl/smart-items/scripts'\n\nexport function _initializeScripts(engine) {\n  const scriptsArray = []\n  return runScripts(engine, scriptsArray)\n}\n\nexport { getScriptInstance, getScriptInstancesByPath, getAllScriptInstances, callScriptMethod }\n";

/// Upstream skips the embedded script runtime for scenes that author no scripts
/// and are not editor scenes.
pub fn script_utils_content(project: &Project, ignore_composite: bool) -> String {
    let has_scripts = !ignore_composite && composites_have_scripts(&project.root);
    if !has_scripts && !project.is_editor_scene() {
        return SCRIPT_UTILS_STUB.to_string();
    }
    SCRIPT_UTILS_SOURCE.to_string()
}

/// The component a composite lists its smart-item scripts in.
const SCRIPT_COMPONENT: &str = "smart-items::Script";

/// Does any composite author a smart-item script? The Rust side of upstream's
/// `compositeData.scripts.size > 0`: any entry in a [`SCRIPT_COMPONENT`]'s
/// `value` array flips the gate, under either spelling of the prefix
/// (`composite_norm::smart_item_name`). Composites that would fail to instance
/// contribute nothing, matching upstream's per-composite try/catch.
pub fn composites_have_scripts(root: &Path) -> bool {
    let (has_scripts, skipped) = composites_script_scan(root);
    for message in skipped {
        crate::ux::note_stderr(message);
    }
    has_scripts
}

fn composites_script_scan(root: &Path) -> (bool, Vec<String>) {
    let mut normalizer = dcl_scene_authoring::composite_norm::CompositeNormalizer::new();
    let mut skipped = Vec::new();
    let has_scripts = find_composites(root).into_iter().any(|path| {
        let Ok(raw) = std::fs::read_to_string(&path) else {
            return false;
        };
        if let Err(err) = normalizer.normalize(&raw) {
            skipped.push(format!(
                "composite {} can't be instanced, so its components are ignored: {err:#}",
                crate::ux::rel_to(root, &path)
            ));
            return false;
        }
        let Ok(json) = serde_json::from_str::<serde_json::Value>(&raw) else {
            return false;
        };
        json.get("components")
            .and_then(|c| c.as_array())
            .is_some_and(|comps| {
                comps
                    .iter()
                    .filter(|c| {
                        c.get("name")
                            .and_then(|n| n.as_str())
                            .is_some_and(|n| smart_item_name(n) == SCRIPT_COMPONENT)
                    })
                    .any(script_component_has_instances)
            })
    });
    (has_scripts, skipped)
}

fn script_component_has_instances(comp: &serde_json::Value) -> bool {
    comp.get("data")
        .and_then(|d| d.as_object())
        .is_some_and(|data| {
            data.values().any(|entry| match entry.get("json") {
                Some(json) => json
                    .get("value")
                    .and_then(|v| v.as_array())
                    .is_some_and(|v| !v.is_empty()),
                None => entry.get("binary").is_some(),
            })
        })
}

#[cfg(test)]
mod tests {
    use super::{
        SCRIPT_UTILS_SOURCE, SCRIPT_UTILS_STUB, composites_have_scripts, composites_script_scan,
        scan_max_composite_entity, script_utils_content, write_all_composites,
    };
    use crate::scene::{Project, Tmp};

    const MAIN: &str = "assets/scene/main.composite";
    const UNINSTANCEABLE: &str = r#"{"version":1,"components":[{"name":"core::NotAThing","data":{}},{"name":"smart-items::Script","jsonSchema":{"type":"object"},"data":{"512":{"json":{"value":[{"path":"src/counter.ts"}]}}}}]}"#;

    fn project(t: &Tmp) -> Project {
        Project {
            root: t.0.clone(),
            scene_json: serde_json::json!({"main": "bin/index.js"}),
        }
    }

    #[test]
    fn composites_have_scripts_requires_script_instances() {
        let t = Tmp::new("hasscripts");
        std::fs::create_dir_all(t.0.join("assets/scene")).unwrap();
        assert!(!composites_have_scripts(&t.0));
        t.write(
            MAIN,
            r#"{"version":1,"components":[{"name":"smart-items::Script","jsonSchema":{"type":"object"},"data":{"512":{"json":{"value":[]}}}},{"name":"smart-items::Actions","jsonSchema":{"type":"object"},"data":{"512":{"json":{}}}}]}"#,
        );
        assert!(!composites_have_scripts(&t.0));
        t.write(
            MAIN,
            r#"{"version":1,"components":[{"name":"smart-items::Script","jsonSchema":{"type":"object"},"data":{"512":{"json":{"value":[{"path":"src/counter.ts","priority":0}]}}}}]}"#,
        );
        assert!(composites_have_scripts(&t.0));
    }

    #[test]
    fn a_legacy_script_component_flips_the_gate() {
        let t = Tmp::new("legacyscripts");
        t.write(
            MAIN,
            r#"{"version":1,"components":[{"name":"asset-packs::Script","jsonSchema":{"type":"object"},"data":{"512":{"json":{"value":[{"path":"src/counter.ts","priority":0}]}}}}]}"#,
        );
        assert!(composites_have_scripts(&t.0));
    }

    #[test]
    fn composites_have_scripts_counts_binary_entries_in_instanceable_composites() {
        let t = Tmp::new("binscripts");
        t.write(
            MAIN,
            r#"{"version":1,"components":[{"name":"smart-items::Script","jsonSchema":{"type":"object"},"data":{"512":{"binary":"AQID"}}}]}"#,
        );
        assert!(composites_have_scripts(&t.0));
    }

    #[test]
    fn non_instanceable_composites_do_not_flip_the_script_gate() {
        let t = Tmp::new("badscripts");
        t.write(MAIN, UNINSTANCEABLE);
        assert!(!composites_have_scripts(&t.0));
        t.write(
            "assets/scene/other.composite",
            r#"{"version":1,"components":[{"name":"smart-items::Script","data":{"512":{"json":{"value":[{"path":"src/counter.ts"}]}}}}]}"#,
        );
        assert!(!composites_have_scripts(&t.0));
    }

    #[test]
    fn non_instanceable_composite_skip_is_reported() {
        let t = Tmp::new("skipnote");
        t.write(MAIN, UNINSTANCEABLE);
        let (has_scripts, skipped) = composites_script_scan(&t.0);
        assert!(!has_scripts);
        assert_eq!(skipped.len(), 1);
        assert!(skipped[0].contains("can't be instanced"), "{}", skipped[0]);
        assert!(skipped[0].contains("main.composite"), "{}", skipped[0]);
    }

    #[test]
    fn script_utils_is_the_runtime_only_for_scenes_with_scripts() {
        let t = Tmp::new("scriptgate");
        let project = project(&t);
        assert_eq!(script_utils_content(&project, false), SCRIPT_UTILS_STUB);
        t.write(
            MAIN,
            r#"{"version":1,"components":[{"name":"smart-items::Script","jsonSchema":{"type":"object"},"data":{"0":{"json":{"value":[{"path":"src/counter.ts"}]}}}}]}"#,
        );
        assert_eq!(script_utils_content(&project, false), SCRIPT_UTILS_SOURCE);
        assert_eq!(script_utils_content(&project, true), SCRIPT_UTILS_STUB);
    }

    #[test]
    fn an_editor_scene_links_the_runtime_without_scripts() {
        let t = Tmp::new("editorutils");
        let project = project(&t);
        t.write(
            MAIN,
            r#"{"version":1,"components":[{"name":"asset-packs::Actions","jsonSchema":{"type":"object"},"data":{"512":{"json":{"value":[]}}}}]}"#,
        );
        assert_eq!(script_utils_content(&project, false), SCRIPT_UTILS_SOURCE);
    }

    #[test]
    fn stub_and_runtime_share_upstreams_export_surface() {
        for export in [
            "_initializeScripts",
            "getScriptInstance",
            "getScriptInstancesByPath",
            "getAllScriptInstances",
            "callScriptMethod",
        ] {
            assert!(SCRIPT_UTILS_STUB.contains(&format!("export function {export}")));
            assert!(SCRIPT_UTILS_SOURCE.contains(export), "{export}");
        }
        assert!(SCRIPT_UTILS_STUB.contains("not found on script"));
        assert!(SCRIPT_UTILS_SOURCE.contains("from '@dcl/smart-items/scripts'"));
        assert!(SCRIPT_UTILS_SOURCE.contains("export function _initializeScripts(engine)"));
    }

    #[test]
    fn inlined_composites_carry_the_runtime_namespace() {
        let t = Tmp::new("inlinelegacy");
        t.write(
            "main.composite",
            r#"{"version":1,"components":[{"name":"asset-packs::Actions","jsonSchema":{"type":"object"},"data":{"512":{"json":{"value":[]}}}}]}"#,
        );
        let out = t.0.join("generated");
        std::fs::create_dir_all(&out).unwrap();
        write_all_composites(&project(&t), &out, false).unwrap();
        let js = std::fs::read_to_string(out.join("all-composites.js")).unwrap();
        assert!(js.contains(r#""name":"smart-items::Actions""#), "{js}");
        assert!(!js.contains("asset-packs::"), "{js}");
    }

    #[test]
    fn max_composite_entity_scans_every_parseable_composite() {
        let t = Tmp::new("maxentity");
        assert_eq!(scan_max_composite_entity(&t.0), 0);
        t.write(
            "main.composite",
            r#"{"version":1,"components":[{"name":"core::Transform","data":{"512":{},"600":{}}}]}"#,
        );
        t.write(
            "sub/other.composite",
            r#"{"version":1,"components":[{"name":"my::Thing","data":{"5170":{}}}]}"#,
        );
        t.write("sub/broken.composite", "not json");
        assert_eq!(scan_max_composite_entity(&t.0), 5170);
    }
}

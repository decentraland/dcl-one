use crate::scene::Project;
use crate::ux::{TrySteps, UserError, write_error};
use anyhow::Result;
use serde_json::{Map, Value};
use std::path::{Path, PathBuf};

const RUNTIME_SCRIPT: &str = include_str!(concat!(
    env!("DCL_ONE_ROOT"),
    "/library/js-sdk-toolchain/packages/@dcl/sdk-commands/src/logic/runtime-script.ts"
));
const SCRIPT_MODULE_DTS: &str = include_str!(concat!(
    env!("DCL_ONE_ROOT"),
    "/library/js-sdk-toolchain/packages/@dcl/sdk-commands/src/logic/script-module.d.ts.template"
));
const SCRIPT_COMPONENT: &str = "asset-packs::Script";
const DTS_BLOCK_START: &str = "// @internal\ndeclare module '~sdk/script-utils' {";

/// The generated `~sdk/script-utils` the SDK chunk's registry serves.
pub const SCRIPT_UTILS_FILE: &str = "script-utils.ts";
/// The generated module that imports the scene's own scripts into its chunk.
pub const SCENE_SCRIPTS_FILE: &str = "scene-scripts.ts";

/// One script file the scene's composites attach, with every
/// `asset-packs::Script` entry that names it plus its `entity`.
#[derive(Debug, PartialEq)]
pub struct SceneScript {
    pub path: String,
    pub instances: Vec<Map<String, Value>>,
}

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
        let scripts = if ignore_composite {
            Vec::new()
        } else {
            collect_scripts(&project.root)?
        };
        write_in(
            &dir,
            SCRIPT_UTILS_FILE,
            script_utils_content(project, &scripts)?,
        )?;
        let scripts_from = if scripts.is_empty() {
            let _ = std::fs::remove_file(dir.join(SCENE_SCRIPTS_FILE));
            "~sdk/script-utils".to_string()
        } else {
            write_in(
                &dir,
                SCENE_SCRIPTS_FILE,
                scene_scripts_module(&project.root, &scripts)?,
            )?;
            format!("./{SCENE_SCRIPTS_FILE}")
        };
        write_script_declarations(project, &scripts)?;
        write_in(&dir, "sdk-boot.js", SDK_BOOT)?;
        entrypoint_code(&safe_entry, project.is_editor_scene(), split, &scripts_from)
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

/// Registers the composite provider; the entrypoint imports it before the
/// scene so a scene calling `initAssetPacks()` at module scope finds it.
const SDK_BOOT: &str = "import { engine, setCompositeProvider } from '@dcl/sdk/ecs'\n\
                        import { compositeProvider } from '@dcl/sdk/composite-provider'\n\
                        setCompositeProvider(engine, compositeProvider)\n";

fn entrypoint_code(
    safe_entry: &str,
    editor_scene: bool,
    split: bool,
    scripts_from: &str,
) -> String {
    let composite_fill = if split {
        "import { compositeFromLoader as __sceneComposites } from './all-composites.js'\nObject.assign(compositeFromLoader, __sceneComposites)\n"
    } else {
        ""
    };
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
import {{ _initializeScripts }} from '{scripts_from}'
import './sdk-boot.js'
import * as entrypoint from {safe_entry}
{composite_fill}
{editor_block}

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

/// The `~sdk/script-utils` a scene with no scripts and no smart items links
/// against; the generated entrypoint calls `_initializeScripts` unconditionally.
/// Same export surface as upstream's `generateScriptStubModuleContent`.
pub const SCRIPT_UTILS_STUB: &str = "export function _initializeAssetPacks() {}\nexport function _initializeScripts(_engine) {}\nexport function getScriptInstance(_entity, _scriptPath) { return null }\nexport function getScriptInstancesByPath(_scriptPath) { return [] }\nexport function getAllScriptInstances(_entity) { return [] }\nexport function callScriptMethod(entity, scriptPath, methodName, ..._args) {\n  console.error(`Method ${methodName} not found on script ${scriptPath} for entity ${entity}`)\n  return undefined\n}\n";

/// The scene-independent smart-item script runtime: upstream's
/// `runtime-script.ts`, which exports `runScripts` for the scene's own
/// [`SCENE_SCRIPTS_FILE`] and the instance lookups `@dcl/asset-packs` calls.
pub fn script_runtime() -> String {
    format!(
        "{}\nexport function _initializeAssetPacks() {{}}\n\nexport function _initializeScripts(engine: IEngine) {{\n  return runScripts(engine, [])\n}}\n",
        RUNTIME_SCRIPT.replace(
            "@dcl/inspector/node_modules/@dcl/asset-packs",
            "@dcl/asset-packs"
        )
    )
}

/// The registry's `~sdk/script-utils`: the stub unless the scene attaches
/// scripts or uses smart items, as upstream gates it.
pub fn script_utils_content(project: &Project, scripts: &[SceneScript]) -> Result<String> {
    if scripts.is_empty() && !project.is_editor_scene() {
        return Ok(SCRIPT_UTILS_STUB.to_string());
    }
    if !crate::split::has_asset_packs(project) && crate::prebuilt::locate(project).is_none() {
        return Err(UserError::new(
            "this scene uses smart items, but @dcl/asset-packs is not installed, so they cannot run",
            TrySteps::one("install it: npm install --save-dev @dcl/asset-packs")
                .and("or delete node_modules and build again: the vendored SDK is restored when the folder is absent"),
        )
        .into());
    }
    Ok(script_runtime())
}

/// Every script the scene's composites attach (upstream's `collectScriptData`):
/// grouped by path in first-seen order, entities ascending within a composite.
/// A composite that cannot be instanced is skipped with a note, as upstream does.
pub fn collect_scripts(root: &Path) -> Result<Vec<SceneScript>> {
    let (scripts, skipped) = scan_scripts(root)?;
    for message in skipped {
        crate::ux::note_stderr(message);
    }
    Ok(scripts)
}

fn scan_scripts(root: &Path) -> Result<(Vec<SceneScript>, Vec<String>)> {
    let mut normalizer = dcl_scene_authoring::composite_norm::CompositeNormalizer::new();
    let mut scripts: Vec<SceneScript> = Vec::new();
    let mut skipped = Vec::new();
    for path in find_composites(root) {
        let rel = crate::ux::rel_to(root, &path);
        let Ok(raw) = std::fs::read_to_string(&path) else {
            continue;
        };
        if let Err(err) = normalizer.normalize(&raw) {
            skipped.push(format!(
                "composite {rel} can't be instanced, so its components are ignored: {err:#}"
            ));
            continue;
        }
        let Ok(json) = serde_json::from_str::<Value>(&raw) else {
            continue;
        };
        let components = json["components"]
            .as_array()
            .map(Vec::as_slice)
            .unwrap_or(&[]);
        for component in components.iter().filter(|c| c["name"] == SCRIPT_COMPONENT) {
            let Some(data) = component["data"].as_object() else {
                continue;
            };
            let mut entities: Vec<(u64, &Value)> = data
                .iter()
                .filter_map(|(key, entry)| key.parse().ok().map(|entity| (entity, entry)))
                .collect();
            entities.sort_by_key(|(entity, _)| *entity);
            for (entity, entry) in entities {
                if entry.get("json").is_none() && entry.get("binary").is_some() {
                    return Err(UserError::new(
                        format!("{rel} stores {SCRIPT_COMPONENT} data in binary form, which the build cannot read, so its scripts would not run"),
                        TrySteps::one(format!("store the {SCRIPT_COMPONENT} data of entity {entity} as JSON: {{\"json\": {{\"value\": [...]}}}}"))
                            .and("or build without the composite on purpose: dcl-scene-cli build --ignoreComposite"),
                    )
                    .into());
                }
                for item in entry["json"]["value"].as_array().into_iter().flatten() {
                    let (Some(fields), Some(script_path)) =
                        (item.as_object(), item["path"].as_str())
                    else {
                        continue;
                    };
                    let mut instance = fields.clone();
                    instance.insert("entity".to_string(), entity.into());
                    match scripts.iter_mut().find(|s| s.path == script_path) {
                        Some(script) => script.instances.push(instance),
                        None => scripts.push(SceneScript {
                            path: script_path.to_string(),
                            instances: vec![instance],
                        }),
                    }
                }
            }
        }
    }
    Ok((scripts, skipped))
}

fn script_import_name(index: usize, script_path: &str) -> String {
    let stem = script_path
        .strip_suffix(".tsx")
        .or_else(|| script_path.strip_suffix(".ts"))
        .unwrap_or(script_path);
    let sanitized: String = stem
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect();
    format!("script_{index}_{sanitized}")
}

fn module_specifier(path: &Path) -> Result<String> {
    Ok(serde_json::to_string(
        &path.display().to_string().replace('\\', "/"),
    )?)
}

/// The scene chunk's script module: imports each script file and hands every
/// instance to the registry runtime's `runScripts`, as upstream's
/// `_initializeScripts` does.
pub fn scene_scripts_module(root: &Path, scripts: &[SceneScript]) -> Result<String> {
    let mut imports = String::new();
    let mut entries = Vec::new();
    for (index, script) in scripts.iter().enumerate() {
        let name = script_import_name(index, &script.path);
        let specifier = module_specifier(&root.join(&script.path))?;
        imports.push_str(&format!("import * as {name} from {specifier}\n"));
        for instance in &script.instances {
            entries.push(format!(
                "    {{ ...{}, module: {name} }}",
                serde_json::to_string(instance)?
            ));
        }
    }
    Ok(format!(
        "import {{ runScripts }} from '~sdk/script-utils'\n{imports}\nexport function _initializeScripts(engine: any) {{\n  return runScripts(engine, [\n{}\n  ] as any)\n}}\n",
        entries.join(",\n")
    ))
}

fn script_declarations(root: &Path, scripts: &[SceneScript]) -> Result<String> {
    let mut type_imports = Vec::new();
    let mut registry = Vec::new();
    for (index, script) in scripts.iter().enumerate() {
        let name = script_import_name(index, &script.path);
        type_imports.push(format!(
            "  type {name} = typeof import({})",
            module_specifier(&root.join(&script.path))?
        ));
        registry.push(format!(
            "  {}: ExtractScriptType<{name}>",
            serde_json::to_string(&script.path)?
        ));
    }
    Ok(SCRIPT_MODULE_DTS
        .replace("from '@dcl/ecs/dist-cjs'", "from '@dcl/ecs'")
        .replace("__TYPE_IMPORTS__", &type_imports.join("\n"))
        .replace(
            "__SCRIPT_TYPES__",
            &format!("interface ScriptRegistry {{\n{}\n}}", registry.join(",\n")),
        ))
}

fn block_end(text: &str, start: usize) -> usize {
    let mut depth = 0usize;
    for (offset, c) in text[start..].char_indices() {
        match c {
            '{' => depth += 1,
            '}' => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    return start + offset + 1;
                }
            }
            _ => {}
        }
    }
    text.len()
}

/// Types `getScriptInstance` and friends per script path, as upstream does, by
/// replacing the `~sdk/script-utils` block of `@dcl/js-runtime`'s declarations
/// (`sdk.d.ts`, or the vendored `index.d.ts` that inlines it) in place. Written
/// only when the block changes, and only for a scene with scripts or a block a
/// previous build filled, so a scene without scripts leaves the package alone.
pub fn write_script_declarations(project: &Project, scripts: &[SceneScript]) -> Result<()> {
    let Some(file) = project
        .node_module("@dcl/js-runtime/sdk.d.ts")
        .or_else(|| project.node_module("@dcl/js-runtime/index.d.ts"))
    else {
        return Ok(());
    };
    let Ok(text) = std::fs::read_to_string(&file) else {
        return Ok(());
    };
    let Some(start) = text.find(DTS_BLOCK_START) else {
        return Ok(());
    };
    let end = block_end(&text, start);
    let current = &text[start..end];
    if scripts.is_empty() && !current.contains("ExtractScriptType<script_") {
        return Ok(());
    }
    let block = script_declarations(&project.root, scripts)?;
    let block = block.trim_end();
    if current == block {
        return Ok(());
    }
    let updated = format!("{}{block}{}", &text[..start], &text[end..]);
    let staged = file.with_extension("ts.dcl-one-tmp");
    if let Err(e) = std::fs::write(&staged, updated).and_then(|()| std::fs::rename(&staged, &file))
    {
        let _ = std::fs::remove_file(&staged);
        crate::ux::note_stderr(format!(
            "could not add the scene's script types to {}: {e}",
            crate::ux::rel_to(&project.root, &file)
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        SCRIPT_UTILS_STUB, SceneScript, collect_scripts, scan_max_composite_entity, scan_scripts,
        scene_scripts_module, script_utils_content, write_script_declarations,
    };
    use crate::scene::{Project, Tmp};

    const MAIN: &str = "assets/scene/main.composite";
    const UNINSTANCEABLE: &str = r#"{"version":1,"components":[{"name":"core::NotAThing","data":{}},{"name":"asset-packs::Script","jsonSchema":{"type":"object"},"data":{"512":{"json":{"value":[{"path":"src/counter.ts"}]}}}}]}"#;
    const TWO_ENTITIES: &str = r#"{"version":1,"components":[{"name":"asset-packs::Script","jsonSchema":{"type":"object"},"data":{
        "600":{"json":{"value":[{"path":"assets/scene/Scripts/Counter.ts","priority":0}]}},
        "512":{"json":{"value":[
            {"path":"assets/scene/Scripts/Counter.ts","priority":0},
            {"path":"assets/scene/Scripts/Door.ts","priority":1,"layout":"{\"params\":{}}"}
        ]}}
    }}]}"#;

    fn project(t: &Tmp) -> Project {
        Project {
            root: t.0.clone(),
            scene_json: serde_json::json!({"main": "bin/index.js"}),
        }
    }

    fn instance(json: serde_json::Value) -> serde_json::Map<String, serde_json::Value> {
        json.as_object().unwrap().clone()
    }

    #[test]
    fn collects_each_script_with_its_entity_grouped_by_path_in_entity_order() {
        let t = Tmp::new("collect");
        t.write(MAIN, TWO_ENTITIES);
        assert_eq!(
            collect_scripts(&t.0).unwrap(),
            vec![
                SceneScript {
                    path: "assets/scene/Scripts/Counter.ts".into(),
                    instances: vec![
                        instance(
                            serde_json::json!({"path":"assets/scene/Scripts/Counter.ts","priority":0,"entity":512})
                        ),
                        instance(
                            serde_json::json!({"path":"assets/scene/Scripts/Counter.ts","priority":0,"entity":600})
                        ),
                    ],
                },
                SceneScript {
                    path: "assets/scene/Scripts/Door.ts".into(),
                    instances: vec![instance(serde_json::json!({
                        "path":"assets/scene/Scripts/Door.ts","priority":1,"layout":"{\"params\":{}}","entity":512
                    }))],
                },
            ]
        );
    }

    #[test]
    fn empty_script_lists_and_other_smart_item_components_collect_nothing() {
        let t = Tmp::new("noscripts");
        std::fs::create_dir_all(t.0.join("assets/scene")).unwrap();
        assert!(collect_scripts(&t.0).unwrap().is_empty());
        t.write(
            MAIN,
            r#"{"version":1,"components":[{"name":"asset-packs::Script","jsonSchema":{"type":"object"},"data":{"512":{"json":{"value":[]}}}},{"name":"asset-packs::Actions","jsonSchema":{"type":"object"},"data":{"512":{"json":{}}}}]}"#,
        );
        assert!(collect_scripts(&t.0).unwrap().is_empty());
    }

    #[test]
    fn binary_script_data_is_refused_instead_of_dropping_the_scripts() {
        let t = Tmp::new("binscripts");
        t.write(
            MAIN,
            r#"{"version":1,"components":[{"name":"asset-packs::Script","jsonSchema":{"type":"object"},"data":{"512":{"binary":"AQID"}}}]}"#,
        );
        let err = format!("{:#}", collect_scripts(&t.0).unwrap_err());
        assert!(err.contains("main.composite"), "{err}");
        assert!(err.contains("binary"), "{err}");
    }

    #[test]
    fn non_instanceable_composites_are_skipped_and_reported() {
        let t = Tmp::new("skipnote");
        t.write(MAIN, UNINSTANCEABLE);
        t.write(
            "assets/scene/other.composite",
            r#"{"version":1,"components":[{"name":"asset-packs::Script","data":{"512":{"json":{"value":[{"path":"src/counter.ts"}]}}}}]}"#,
        );
        let (scripts, skipped) = scan_scripts(&t.0).unwrap();
        assert!(scripts.is_empty());
        assert_eq!(skipped.len(), 2, "{skipped:?}");
        assert!(skipped[0].contains("can't be instanced"), "{}", skipped[0]);
        assert!(skipped[0].contains("main.composite"), "{}", skipped[0]);
    }

    #[test]
    fn script_utils_is_the_stub_without_scripts_and_the_runtime_with_them() {
        let t = Tmp::new("scriptgate");
        std::fs::create_dir_all(t.0.join("node_modules/@dcl/asset-packs")).unwrap();
        let project = project(&t);
        assert_eq!(
            script_utils_content(&project, &[]).unwrap(),
            SCRIPT_UTILS_STUB
        );
        t.write(MAIN, TWO_ENTITIES);
        let scripts = collect_scripts(&t.0).unwrap();
        let runtime = script_utils_content(&project, &scripts).unwrap();
        assert!(runtime.contains("export function runScripts"), "{runtime}");
        assert!(
            runtime.contains("export function _initializeScripts"),
            "{runtime}"
        );
        assert!(
            runtime.contains("from '@dcl/asset-packs/dist/events'"),
            "{runtime}"
        );
        assert!(
            !runtime.contains("@dcl/inspector/node_modules"),
            "{runtime}"
        );
    }

    #[test]
    fn scripts_without_asset_packs_are_refused() {
        let t = Tmp::new("noassetpacks");
        t.write(MAIN, TWO_ENTITIES);
        let scripts = collect_scripts(&t.0).unwrap();
        let err = format!(
            "{:#}",
            script_utils_content(&project(&t), &scripts).unwrap_err()
        );
        assert!(err.contains("@dcl/asset-packs"), "{err}");
    }

    #[test]
    fn scene_scripts_module_imports_every_script_and_passes_each_instance() {
        let t = Tmp::new("scenemodule");
        t.write(MAIN, TWO_ENTITIES);
        let module = scene_scripts_module(&t.0, &collect_scripts(&t.0).unwrap()).unwrap();
        let counter = t.0.join("assets/scene/Scripts/Counter.ts");
        assert!(
            module.starts_with("import { runScripts } from '~sdk/script-utils'\n"),
            "{module}"
        );
        assert!(
            module.contains(&format!(
                "import * as script_0_assets_scene_Scripts_Counter from \"{}\"",
                counter.display()
            )),
            "{module}"
        );
        assert!(
            module.contains("import * as script_1_assets_scene_Scripts_Door from"),
            "{module}"
        );
        assert_eq!(
            module
                .matches("module: script_0_assets_scene_Scripts_Counter")
                .count(),
            2,
            "{module}"
        );
        assert!(module.contains("\"entity\":600"), "{module}");
        assert!(module.contains("return runScripts(engine, ["), "{module}");
    }

    const DECLARATIONS: &str = "declare module '~sdk/all-composites' {\n  export const x: number\n}\n\n// @internal\ndeclare module '~sdk/script-utils' {\n  type Entity = number\n  export function getScriptInstance(entity: Entity, scriptPath: string): any\n}\n\ndeclare module \"@dcl/asset-packs/dist/action-types\" {\n  export const kept: true\n}\n";

    #[test]
    fn declarations_replace_only_the_script_utils_block_and_only_when_needed() {
        let t = Tmp::new("declarations");
        let dts = t.0.join("node_modules/@dcl/js-runtime/index.d.ts");
        t.write("node_modules/@dcl/js-runtime/index.d.ts", DECLARATIONS);
        let project = project(&t);

        write_script_declarations(&project, &[]).unwrap();
        assert_eq!(std::fs::read_to_string(&dts).unwrap(), DECLARATIONS);

        t.write(MAIN, TWO_ENTITIES);
        let scripts = collect_scripts(&t.0).unwrap();
        write_script_declarations(&project, &scripts).unwrap();
        let typed = std::fs::read_to_string(&dts).unwrap();
        assert!(typed.starts_with("declare module '~sdk/all-composites' {\n  export const x: number\n}\n\n// @internal\ndeclare module '~sdk/script-utils' {"), "{typed}");
        assert!(typed.contains("\"assets/scene/Scripts/Door.ts\": ExtractScriptType<script_1_assets_scene_Scripts_Door>"), "{typed}");
        assert!(
            typed.contains("import type { Entity } from '@dcl/ecs'\n"),
            "{typed}"
        );
        assert!(
            !t.0.join("node_modules/@dcl/js-runtime/index.d.ts.dcl-one-tmp")
                .exists()
        );
        assert!(typed.ends_with("}\n\ndeclare module \"@dcl/asset-packs/dist/action-types\" {\n  export const kept: true\n}\n"), "{typed}");
        assert_eq!(
            typed.matches("declare module '~sdk/script-utils'").count(),
            1
        );

        write_script_declarations(&project, &scripts).unwrap();
        assert_eq!(std::fs::read_to_string(&dts).unwrap(), typed);

        write_script_declarations(&project, &[]).unwrap();
        let emptied = std::fs::read_to_string(&dts).unwrap();
        assert!(!emptied.contains("ExtractScriptType<script_"), "{emptied}");
        assert!(emptied.contains("interface ScriptRegistry {"), "{emptied}");
        assert!(
            emptied.ends_with("export const kept: true\n}\n"),
            "{emptied}"
        );
    }

    #[test]
    fn stub_matches_upstreams_export_surface() {
        for export in [
            "_initializeAssetPacks",
            "_initializeScripts",
            "getScriptInstance",
            "getScriptInstancesByPath",
            "getAllScriptInstances",
            "callScriptMethod",
        ] {
            assert!(SCRIPT_UTILS_STUB.contains(&format!("export function {export}")));
        }
        assert!(SCRIPT_UTILS_STUB.contains("not found on script"));
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

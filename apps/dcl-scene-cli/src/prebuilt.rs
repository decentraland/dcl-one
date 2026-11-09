//! Prebuilt SDK runtime chunks: built once at blob-build time, not per scene.
//!
//! The SDK runtime chunk is scene-independent — keyed only on which `@dcl/*`
//! packages are installed, never on what the scene imports — so the 3.64 MB of
//! SDK source the blob used to ship existed only to let rolldown re-derive the
//! same bytes on every build.
//!
//! Two chunks, not one: **core** (always installed; the registry of
//! [`crate::split::core_registry_keys`]) and **smart** (installed only when the
//! scene uses smart items; `@dcl/smart-items`, its script runtime and the
//! real `~sdk/script-utils`, resolving everything else through core's
//! registry). The smart-item runtime is +30% on a bundle; the split makes only
//! smart-item scenes pay it.
//!
//! The chunks live inside the vendored `@dcl/sdk` (`node_modules/@dcl/sdk/
//! prebuilt/`) on purpose: they are valid only for the `@dcl/sdk` they were
//! built from, so an `npm install` that replaces the package removes them in
//! the same step and flips the build back to the source path atomically.

use crate::esbuild::EsbuildOptions;
use crate::scene::Project;
use crate::ux::{TrySteps, UserError};
use crate::{entrypoint, split};
use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

pub const DIR: &str = "@dcl/sdk/prebuilt";
pub const CORE_FILE: &str = "@dcl/sdk/prebuilt/core.js";
pub const SMART_FILE: &str = "@dcl/sdk/prebuilt/smart.js";
/// The keys each chunk publishes, written next to them so the blob's
/// unresolved-import scan can check the chunks against the registry that
/// actually resolves their imports instead of against `node_modules`.
pub const REGISTRY_FILE: &str = "@dcl/sdk/prebuilt/registry.json";

#[derive(Debug)]
pub struct Prebuilt {
    pub core: PathBuf,
    pub smart: Option<PathBuf>,
}

/// `Ok(None)` means a source `node_modules` (a package-manager install, or an
/// older blob without chunks): the SDK chunk is bundled from source.
///
/// The chunks are valid only for the `@dcl/sdk` this binary scaffolds
/// ([`crate::init::vendored_sdk_version`]): the entrypoint and the script
/// runtime it generates require keys only that `@dcl/sdk`'s smart chunk
/// publishes, so an installed `@dcl/sdk` of any other version is refused with
/// the reinstall step instead of a bundle that throws at scene start.
pub fn locate(project: &Project) -> Result<Option<Prebuilt>> {
    let Some(core) = project.node_module(CORE_FILE) else {
        return Ok(None);
    };
    let shipped = crate::init::vendored_sdk_version();
    let installed = crate::init::installed_sdk_version(&project.root);
    if installed.as_deref() != Some(shipped) {
        let installed = installed
            .map(|v| format!("@dcl/sdk {v}"))
            .unwrap_or_else(|| "an @dcl/sdk without a version".to_string());
        return Err(stale(format!(
            "node_modules holds {installed} with prebuilt SDK chunks, and this dcl-scene-cli ships @dcl/sdk {shipped}"
        )));
    }
    let smart = project.node_module(SMART_FILE);
    if smart.is_some() && !registry_lists(project, split::SMART_ITEMS) {
        return Err(stale(format!(
            "node_modules/{REGISTRY_FILE} does not list {} for the prebuilt smart-item chunk",
            split::SMART_ITEMS
        )));
    }
    Ok(Some(Prebuilt { core, smart }))
}

fn stale(what: String) -> anyhow::Error {
    UserError::new(
        what,
        TrySteps::one(
            "run dcl-scene-cli init --node-modules-only to reinstall the vendored node_modules",
        ),
    )
    .into()
}

/// Does the prebuilt registry manifest list `key` under the smart chunk?
fn registry_lists(project: &Project, key: &str) -> bool {
    let Some(path) = project.node_module(REGISTRY_FILE) else {
        return false;
    };
    let Ok(raw) = std::fs::read(path) else {
        return false;
    };
    let Ok(registry) = serde_json::from_slice::<serde_json::Value>(&raw) else {
        return false;
    };
    registry["smart"]
        .as_array()
        .is_some_and(|keys| keys.iter().any(|k| k.as_str() == Some(key)))
}

/// An editor scene's entrypoint calls `initSmartItems`; a scene can also import
/// `@dcl/smart-items` (or `@dcl/asset-packs`, which the loader maps onto it)
/// from its own source with no composite at all (`0,0-cube-spawner` in
/// sdk7-test-scenes), and since both are externals of the scene chunk the
/// specifier survives into the emitted bundle exactly when something reaches it.
pub fn scene_needs_smart_chunk(project: &Project, scene_chunk: &Path) -> bool {
    project.is_editor_scene()
        || chunk_requires(scene_chunk, split::SMART_ITEMS)
        || chunk_requires(scene_chunk, split::LEGACY_SMART_ITEMS)
}

fn chunk_requires(chunk: &Path, package: &str) -> bool {
    let Ok(code) = std::fs::read_to_string(chunk) else {
        return false;
    };
    code.contains(&format!("require(\"{package}")) || code.contains(&format!("require('{package}"))
}

pub fn install(src: &Path, dst: &Path) -> Result<()> {
    if let Some(dir) = dst.parent() {
        std::fs::create_dir_all(dir)
            .with_context(|| format!("creating the bundle directory {}", dir.display()))?;
    }
    std::fs::copy(src, dst).map_err(|e| {
        anyhow::Error::from(
            UserError::new(
                format!(
                    "cannot install the prebuilt SDK runtime chunk into {}",
                    dst.display()
                ),
                TrySteps::one("check write permission on the project directory").and(
                    "re-install the vendored toolchain with dcl-scene-cli init --node-modules-only",
                ),
            )
            .caused_by(e),
        )
    })?;
    Ok(())
}

/// Deleting the last smart item would otherwise leave a stale
/// `sdk-smart-items.js` the loader no longer names but `deploy` still uploads.
pub fn remove_stale_smart_chunk(root: &Path, smart_rel: &str) {
    let _ = std::fs::remove_file(root.join(smart_rel));
}

/// Build both chunks from a scene whose `node_modules` holds the full SDK
/// install tree (what `scripts/build-base-blob.py` calls through the hidden
/// `vendor-chunks` subcommand). The passes differ in entry and externals: core
/// aliases `~sdk/script-utils` to the no-op stub with nothing external; smart
/// makes everything core owns external ([`split::smart_externals`]), so it
/// bundles `@dcl/smart-items` and nothing else of the SDK, and aliases
/// `~sdk/script-utils` to the *real* runtime module, so exactly one copy is
/// bundled and the registry entry and the runtime's own import land on the same
/// module instance. Both are then checked for requires the loader could not
/// serve — `@dcl/sdk/platform` and `@dcl/sdk/text-codec` would otherwise throw
/// "not in the sdk runtime registry" at scene start.
pub async fn build_chunks(dir: &Path, out_core: &Path, out_smart: &Path) -> Result<()> {
    let project = Project::load(dir)?;
    let tsconfig = project.tsconfig()?;
    let work = crate::scene::work_dir(&project.root)
        .with_context(|| format!("creating {}", project.root.join(".dcl-one").display()))?;
    let write = |name: &str, content: &str| -> Result<PathBuf> {
        let path = work.join(name);
        std::fs::write(&path, content)?;
        Ok(path)
    };

    let core_keys = split::core_registry_keys(&project);
    let smart_keys = split::smart_registry_keys();

    let slot = write(
        "composite-slot.js",
        "export const compositeFromLoader = {}\n",
    )?;
    let stub = write("script-utils-stub.js", entrypoint::SCRIPT_UTILS_STUB)?;
    let core_entry = write("core-registry.js", &split::registry_module(&core_keys))?;
    let mut core_aliases = crate::esbuild::resolve_aliases(&project)?;
    core_aliases.push(("~sdk/all-composites".to_string(), slot));
    core_aliases.push(("~sdk/script-utils".to_string(), stub));
    bundle_chunk(
        &project,
        core_entry,
        out_core,
        &tsconfig,
        core_aliases,
        vec![],
    )
    .await?;

    let real_utils = write("script-utils.js", entrypoint::SCRIPT_UTILS_SOURCE)?;
    let smart_entry = write("smart-registry.js", &split::registry_module(smart_keys))?;
    bundle_chunk(
        &project,
        smart_entry,
        out_smart,
        &tsconfig,
        vec![("~sdk/script-utils".to_string(), real_utils)],
        split::smart_externals(),
    )
    .await?;

    verify_requires(out_core, &[])?;
    verify_requires(out_smart, &core_keys)?;

    let registry = serde_json::json!({
        "core": core_keys,
        "smart": smart_keys,
    });
    let manifest = out_core.with_file_name("registry.json");
    std::fs::write(&manifest, serde_json::to_vec_pretty(&registry)?)
        .with_context(|| format!("writing {}", manifest.display()))?;
    Ok(())
}

async fn bundle_chunk(
    project: &Project,
    entrypoint: PathBuf,
    outfile: &Path,
    tsconfig: &Path,
    aliases: Vec<(String, PathBuf)>,
    externals: Vec<String>,
) -> Result<()> {
    crate::esbuild::bundle(
        project,
        &EsbuildOptions {
            production: true,
            entrypoint,
            outfile: outfile.to_path_buf(),
            tsconfig: tsconfig.to_path_buf(),
            aliases,
            externals,
        },
    )
    .await
}

/// Every `require()` literal in a built chunk must be `~system/*` (passed to
/// the host) or a key the loader's registry already holds, a `@dcl/asset-packs`
/// specifier counting as the key the loader maps it onto
/// ([`split::smart_items_spec`]).
fn verify_requires(chunk: &Path, allowed: &[&str]) -> Result<()> {
    let code = std::fs::read_to_string(chunk)
        .with_context(|| format!("reading the built chunk {}", chunk.display()))?;
    let mut bad: Vec<String> = Vec::new();
    for spec in require_specifiers(&code) {
        if spec.starts_with("~system/") || allowed.contains(&&*split::smart_items_spec(&spec)) {
            continue;
        }
        if !bad.contains(&spec) {
            bad.push(spec);
        }
    }
    if bad.is_empty() {
        return Ok(());
    }
    bad.sort();
    Err(UserError::new(
        format!(
            "{} requires specifiers the sdk runtime registry does not publish",
            chunk.display()
        ),
        TrySteps::one("add each specifier below to REGISTRY_KEYS in src/split.rs")
            .and("or make it external of the chunk that should own it"),
    )
    .why(bad.join(", "))
    .into())
}

fn require_specifiers(code: &str) -> Vec<String> {
    let mut out = Vec::new();
    let bytes = code.as_bytes();
    let mut i = 0;
    while let Some(pos) = code[i..].find("require(") {
        let start = i + pos + "require(".len();
        i = start;
        let Some(&quote) = bytes.get(start) else {
            break;
        };
        if quote != b'"' && quote != b'\'' {
            continue;
        }
        let Some(end) = code[start + 1..].find(quote as char) else {
            break;
        };
        let spec = &code[start + 1..start + 1 + end];
        if code.as_bytes().get(start + 2 + end) == Some(&b')') {
            out.push(spec.to_string());
        }
        i = start + 1 + end;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scene::Tmp;

    #[test]
    fn require_specifiers_reads_both_quote_styles_and_ignores_calls() {
        let code = r#"var a=require("@dcl/sdk/ecs"),b=require('~system/Runtime');require(x);"#;
        assert_eq!(
            require_specifiers(code),
            vec!["@dcl/sdk/ecs".to_string(), "~system/Runtime".to_string()]
        );
    }

    #[test]
    fn verify_requires_accepts_system_and_registry_keys_only() {
        let t = Tmp::new("prebuilt");
        t.write(
            "smart.js",
            r#"require("~system/EngineApi");require("@dcl/sdk/ecs")"#,
        );
        let chunk = t.0.join("smart.js");
        assert!(verify_requires(&chunk, &["@dcl/sdk/ecs"]).is_ok());
        assert!(verify_requires(&chunk, &[]).is_err());
    }

    #[test]
    fn chunk_requires_matches_only_a_real_specifier() {
        let t = Tmp::new("prebuilt-cr");
        let chunk = t.0.join("scene.js");
        t.write(
            "scene.js",
            "var x = 1 // @dcl/smart-items is only a comment\n",
        );
        assert!(!chunk_requires(&chunk, split::SMART_ITEMS));
        t.write("scene.js", r#"var e=require("@dcl/smart-items/scripts");"#);
        assert!(chunk_requires(&chunk, split::SMART_ITEMS));
    }

    fn sdk_tree(t: &Tmp, version: &str) {
        t.write(
            "node_modules/@dcl/sdk/package.json",
            format!(r#"{{"name":"@dcl/sdk","version":"{version}"}}"#),
        );
        t.write("node_modules/@dcl/sdk/prebuilt/core.js", "");
    }

    #[test]
    fn locate_refuses_the_chunks_of_another_sdk() {
        let t = Tmp::new("prebuilt-locate");
        let project = Project {
            root: t.0.clone(),
            scene_json: serde_json::json!({}),
        };
        assert!(
            locate(&project).unwrap().is_none(),
            "no chunks: source build"
        );
        let shipped = crate::init::vendored_sdk_version();
        sdk_tree(&t, "7.0.0");
        let err = locate(&project).unwrap_err().to_string();
        assert!(
            err.contains("@dcl/sdk 7.0.0") && err.contains(shipped),
            "{err}"
        );
        sdk_tree(&t, shipped);
        assert!(locate(&project).unwrap().unwrap().smart.is_none());
    }

    #[test]
    fn locate_requires_the_smart_chunk_to_publish_its_key() {
        let t = Tmp::new("prebuilt-locate-smart");
        let project = Project {
            root: t.0.clone(),
            scene_json: serde_json::json!({}),
        };
        sdk_tree(&t, crate::init::vendored_sdk_version());
        t.write("node_modules/@dcl/sdk/prebuilt/smart.js", "");
        assert!(locate(&project).is_err(), "no registry manifest");
        t.write(
            "node_modules/@dcl/sdk/prebuilt/registry.json",
            r#"{"core":["@dcl/sdk"],"smart":["~sdk/script-utils"]}"#,
        );
        let err = locate(&project).unwrap_err().to_string();
        assert!(err.contains(split::SMART_ITEMS), "{err}");
        t.write(
            "node_modules/@dcl/sdk/prebuilt/registry.json",
            r#"{"core":["@dcl/sdk"],"smart":["@dcl/smart-items","~sdk/script-utils"]}"#,
        );
        assert!(locate(&project).unwrap().unwrap().smart.is_some());
    }

    #[test]
    fn a_legacy_smart_items_import_needs_the_smart_chunk() {
        let t = Tmp::new("prebuilt-legacy");
        let project = Project {
            root: t.0.clone(),
            scene_json: serde_json::json!({}),
        };
        let chunk = t.0.join("scene.js");
        t.write(
            "scene.js",
            r#"var h=require("@dcl/asset-packs/dist/helpers");"#,
        );
        assert!(scene_needs_smart_chunk(&project, &chunk));
    }

    #[test]
    fn verify_requires_maps_legacy_smart_items_specifiers() {
        let t = Tmp::new("prebuilt-verify-legacy");
        let chunk = t.0.join("c.js");
        t.write(
            "c.js",
            r#"var a=require("@dcl/asset-packs/dist/helpers.js");"#,
        );
        assert!(verify_requires(&chunk, &["@dcl/smart-items/helpers"]).is_ok());
        assert!(verify_requires(&chunk, &["@dcl/smart-items"]).is_err());
    }
}

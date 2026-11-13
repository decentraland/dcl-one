//! Skip tsc when nothing it would read has changed: node plus tsc spend
//! ~200 ms confirming an unchanged scene. The stamp records every file the
//! last passing program read (from tsc's own tsbuildinfo), the tsconfig
//! chain, the compiler, and the set of sources under the project; it is
//! written only by a pass and deleted by a failure.

use crate::scene::Project;
use crate::typescript::Compiler;
use dcl_scene_authoring::jsjson::JsValue;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

/// Under a dot-dir so the watcher skips it and tsc cannot feed back into
/// the rebuild that started it.
pub const TSBUILDINFO: &str = ".dcl-cache/tsbuildinfo";
const STAMP: &str = ".dcl-cache/tscheck.json";
const DEPRECATIONS: &str = ".dcl-cache/tsdeprecations.json";

#[derive(serde::Serialize, serde::Deserialize)]
struct Stamp {
    /// Size and mtime of tsc.js; hashing 9 MB every build is not worth it.
    tsc: String,
    files: Vec<Seen>,
    candidates: Vec<String>,
}

#[derive(serde::Serialize, serde::Deserialize, PartialEq)]
struct Seen {
    rel: String,
    size: u64,
    mtime_ns: u128,
    hash: String,
}

fn stamp_path(root: &Path) -> PathBuf {
    root.join(STAMP)
}

/// `path` relative to `root` with `/` separators; a path outside stays absolute.
fn rel_str(root: &Path, path: &Path) -> String {
    match path.strip_prefix(root) {
        Ok(rel) => rel.to_string_lossy().replace('\\', "/"),
        Err(_) => path.to_string_lossy().into_owned(),
    }
}

fn mtime_ns(meta: &std::fs::Metadata) -> Option<u128> {
    let since = meta
        .modified()
        .ok()?
        .duration_since(std::time::UNIX_EPOCH)
        .ok()?;
    Some(since.as_nanos())
}

fn fingerprint(path: &Path) -> Option<String> {
    let meta = std::fs::metadata(path).ok()?;
    Some(format!("{}:{}", meta.len(), mtime_ns(&meta)?))
}

fn hash_file(path: &Path) -> Option<String> {
    let digest = Sha256::digest(std::fs::read(path).ok()?);
    Some(digest.iter().map(|b| format!("{b:02x}")).collect())
}

fn seen(root: &Path, rel: &str) -> Option<Seen> {
    let path = root.join(rel);
    let meta = std::fs::metadata(&path).ok()?;
    Some(Seen {
        rel: rel.to_string(),
        size: meta.len(),
        mtime_ns: mtime_ns(&meta)?,
        hash: hash_file(&path)?,
    })
}

/// Same size and mtime, or same size and hash (a save without an edit).
fn still_same(root: &Path, was: &Seen) -> bool {
    let path = root.join(&was.rel);
    let Ok(meta) = std::fs::metadata(&path) else {
        return false;
    };
    meta.len() == was.size
        && (mtime_ns(&meta) == Some(was.mtime_ns)
            || hash_file(&path).as_deref() == Some(was.hash.as_str()))
}

/// Every TypeScript or JavaScript file under the project outside
/// dependencies, build output and dot-prefixed paths, sorted — a file added
/// since the pass is not in the stamp's file list, so the set is compared.
fn candidates(root: &Path) -> Vec<String> {
    fn walk(root: &Path, dir: &Path, out: &mut Vec<String>) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if name.starts_with('.') {
                continue;
            }
            if path.is_dir() {
                if !matches!(&*name, "node_modules" | "bin") {
                    walk(root, &path, out);
                }
            } else if matches!(
                path.extension().and_then(|e| e.to_str()),
                Some("ts" | "tsx" | "mts" | "cts" | "js" | "jsx" | "mjs" | "cjs")
            ) {
                out.push(rel_str(root, &path));
            }
        }
    }
    let mut out = Vec::new();
    walk(root, root, &mut out);
    out.sort();
    out
}

/// tsbuildinfo's `fileNames`, relative to its own directory, re-based onto
/// the root; a file outside the root stays absolute.
fn program_files(root: &Path) -> Option<Vec<String>> {
    let info_path = root.join(TSBUILDINFO);
    let info: serde_json::Value = serde_json::from_slice(&std::fs::read(&info_path).ok()?).ok()?;
    let base = info_path.parent()?;
    let root = root.canonicalize().unwrap_or_else(|_| root.to_path_buf());
    let names = info
        .get("fileNames")
        .or_else(|| info.get("program").and_then(|p| p.get("fileNames")))?
        .as_array()?;
    names
        .iter()
        .map(|name| {
            let abs = base.join(name.as_str()?);
            Some(rel_str(&root, &abs.canonicalize().unwrap_or(abs)))
        })
        .collect()
}

/// tsconfig.json and everything it extends, each with its path as the stamp
/// names it; None when a link does not resolve, and then tsc runs.
fn tsconfig_docs(project: &Project) -> Option<Vec<(String, JsValue)>> {
    let mut chain = Vec::new();
    let mut current = project.root.join("tsconfig.json");
    for _ in 0..8 {
        let doc =
            dcl_scene_authoring::jsjson::parse(&std::fs::read_to_string(&current).ok()?).ok()?;
        let next = doc
            .get("extends")
            .and_then(|e| e.as_str())
            .map(str::to_string);
        chain.push((rel_str(&project.root, &current), doc));
        let Some(next) = next else {
            return Some(chain);
        };
        current = match next.starts_with('.') || next.starts_with('/') {
            true => current.parent()?.join(next),
            false => project
                .node_module(&next)
                .or_else(|| project.node_module(&format!("{next}/tsconfig.json")))?,
        };
        if !current.is_file() {
            return None;
        }
    }
    None
}

fn tsconfig_chain(project: &Project) -> Option<Vec<String>> {
    Some(
        tsconfig_docs(project)?
            .into_iter()
            .map(|(rel, _)| rel)
            .collect(),
    )
}

/// What TypeScript 6.x deprecated in this tsconfig when it last said so:
/// kept so that later checks pass `--ignoreDeprecations` at once instead of
/// failing first. It stands while the compiler and the tsconfig chain do.
#[derive(serde::Serialize, serde::Deserialize)]
struct Deprecations {
    tsc: String,
    config: Vec<Seen>,
    options: Vec<String>,
}

/// The options a 6.x check of this scene is known to deprecate, without
/// running it: what tsc reported for this very tsconfig, else what the
/// tsconfig itself settles ([`crate::typescript::foreseen_deprecations`]).
/// None when unknown, and for every other TypeScript line.
pub fn known_deprecations(project: &Project, compiler: &Compiler) -> Option<Vec<String>> {
    if compiler.major() != 6 {
        return None;
    }
    let root = &project.root;
    let remembered = (|| {
        let memo: Deprecations =
            serde_json::from_slice(&std::fs::read(root.join(DEPRECATIONS)).ok()?).ok()?;
        let stands = fingerprint(&compiler.tsc).as_deref() == Some(memo.tsc.as_str())
            && !memo.config.is_empty()
            && !memo.options.is_empty()
            && memo.config.iter().all(|f| still_same(root, f));
        stands.then_some(memo.options)
    })();
    remembered.or_else(|| {
        let docs: Vec<JsValue> = tsconfig_docs(project)?
            .into_iter()
            .map(|(_, doc)| doc)
            .collect();
        crate::typescript::foreseen_deprecations(compiler, &docs)
    })
}

/// Best-effort, like the stamp: without it the next check fails first again.
pub fn remember_deprecations(project: &Project, compiler: &Compiler, options: &[String]) {
    let root = &project.root;
    let memo = (|| {
        Some(Deprecations {
            tsc: fingerprint(&compiler.tsc)?,
            config: tsconfig_chain(project)?
                .iter()
                .map(|rel| seen(root, rel))
                .collect::<Option<_>>()?,
            options: options.to_vec(),
        })
    })();
    let path = root.join(DEPRECATIONS);
    let Some(memo) = memo else {
        let _ = std::fs::remove_file(path);
        return;
    };
    if let Ok(json) = serde_json::to_vec(&memo) {
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let _ = std::fs::write(path, json);
    }
}

pub fn unchanged(project: &Project, tsc: &Path) -> bool {
    let root = &project.root;
    let Ok(text) = std::fs::read(stamp_path(root)) else {
        return false;
    };
    let Ok(stamp) = serde_json::from_slice::<Stamp>(&text) else {
        return false;
    };
    fingerprint(tsc).as_deref() == Some(stamp.tsc.as_str())
        && !stamp.files.is_empty()
        && stamp.files.iter().all(|f| still_same(root, f))
        && candidates(root) == stamp.candidates
}

/// Best-effort: a stamp that cannot be written only costs the next build
/// its shortcut.
pub fn record(project: &Project, tsc: &Path) {
    let root = &project.root;
    let stamp = (|| {
        let mut files = program_files(root)?;
        for link in tsconfig_chain(project)? {
            if !files.contains(&link) {
                files.push(link);
            }
        }
        Some(Stamp {
            tsc: fingerprint(tsc)?,
            files: files
                .iter()
                .map(|rel| seen(root, rel))
                .collect::<Option<_>>()?,
            candidates: candidates(root),
        })
    })();
    let Some(stamp) = stamp else {
        return forget(root);
    };
    if let Ok(json) = serde_json::to_vec(&stamp) {
        let path = stamp_path(root);
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let _ = std::fs::write(path, json);
    }
}

pub fn forget(root: &Path) {
    let _ = std::fs::remove_file(stamp_path(root));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scene(tag: &str) -> Project {
        let root =
            std::env::temp_dir().join(format!("dcl-scene-cli-stamp-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        for dir in [
            "src",
            "node_modules/typescript/lib",
            "node_modules/@dcl/sdk/types",
            ".dcl-cache",
        ] {
            std::fs::create_dir_all(root.join(dir)).unwrap();
        }
        for (rel, text) in [
            ("node_modules/typescript/lib/tsc.js", "// tsc"),
            (
                "node_modules/@dcl/sdk/types/tsconfig.ecs7.json",
                r#"{ "compilerOptions": { "strict": true } }"#,
            ),
            (
                "tsconfig.json",
                r#"{ "extends": "@dcl/sdk/types/tsconfig.ecs7.json", "include": ["src/**/*.ts"] }"#,
            ),
            ("src/index.ts", "export const a = 1\n"),
            (
                TSBUILDINFO,
                r#"{ "fileNames": ["../src/index.ts"], "version": "6.0.3" }"#,
            ),
            ("scene.json", "{}"),
        ] {
            std::fs::write(root.join(rel), text).unwrap();
        }
        Project {
            root,
            scene_json: serde_json::json!({}),
        }
    }

    fn tsc(p: &Project) -> PathBuf {
        p.root.join("node_modules/typescript/lib/tsc.js")
    }

    #[test]
    fn the_stamp_stands_until_something_tsc_would_read_moves() {
        let p = scene("stands");
        assert!(!unchanged(&p, &tsc(&p)), "no stamp, no shortcut");
        record(&p, &tsc(&p));
        assert!(unchanged(&p, &tsc(&p)));

        // Saves get an mtime of their own: two writes inside one tick of the
        // filesystem clock share one, and these saves keep the file's size.
        let src = p.root.join("src/index.ts");
        let was = std::fs::metadata(&src).unwrap().modified().unwrap();
        let save = |text: &str, after: u64| {
            std::fs::write(&src, text).unwrap();
            let at = was + std::time::Duration::from_secs(after);
            std::fs::File::options()
                .write(true)
                .open(&src)
                .unwrap()
                .set_modified(at)
                .unwrap();
            assert_eq!(std::fs::metadata(&src).unwrap().modified().unwrap(), at);
        };
        save(&std::fs::read_to_string(&src).unwrap(), 1);
        assert!(unchanged(&p, &tsc(&p)), "a touch is not a change");

        save("export const a = 2\n", 2);
        assert!(!unchanged(&p, &tsc(&p)), "an edit runs tsc");
        record(&p, &tsc(&p));
        assert!(unchanged(&p, &tsc(&p)));

        std::fs::write(p.root.join("src/new.ts"), "export {}\n").unwrap();
        assert!(!unchanged(&p, &tsc(&p)), "a new source runs tsc");
        std::fs::remove_file(p.root.join("src/new.ts")).unwrap();
        assert!(unchanged(&p, &tsc(&p)));

        std::fs::write(
            p.root
                .join("node_modules/@dcl/sdk/types/tsconfig.ecs7.json"),
            r#"{ "compilerOptions": { "strict": false } }"#,
        )
        .unwrap();
        assert!(!unchanged(&p, &tsc(&p)), "the extended tsconfig counts");
        record(&p, &tsc(&p));

        std::fs::write(tsc(&p), "// tsc 2").unwrap();
        assert!(
            !unchanged(&p, &tsc(&p)),
            "another compiler is another check"
        );
        record(&p, &tsc(&p));
        assert!(unchanged(&p, &tsc(&p)));

        forget(&p.root);
        assert!(!unchanged(&p, &tsc(&p)), "a failure leaves no stamp");
        let _ = std::fs::remove_dir_all(&p.root);
    }

    #[test]
    fn a_reported_deprecation_is_remembered_until_the_tsconfig_or_compiler_moves() {
        let p = scene("deprecations");
        let compiler = |version: &str| Compiler {
            tsc: tsc(&p),
            version: version.to_string(),
        };
        let six = compiler("6.0.3");
        assert_eq!(known_deprecations(&p, &six), None, "nothing known yet");

        let reported = vec!["moduleResolution=node10".to_string()];
        remember_deprecations(&p, &six, &reported);
        assert_eq!(known_deprecations(&p, &six), Some(reported.clone()));
        assert_eq!(known_deprecations(&p, &compiler("5.9.3")), None);
        assert_eq!(known_deprecations(&p, &compiler("7.0.2")), None);

        let tsconfig = p.root.join("tsconfig.json");
        let was = std::fs::read_to_string(&tsconfig).unwrap();
        std::fs::write(
            &tsconfig,
            was.replace("\"include\"", "\"files\": [], \"include\""),
        )
        .unwrap();
        assert_eq!(
            known_deprecations(&p, &six),
            None,
            "an edited tsconfig asks tsc again"
        );
        std::fs::write(&tsconfig, &was).unwrap();
        assert_eq!(known_deprecations(&p, &six), Some(reported.clone()));

        std::fs::write(
            p.root
                .join("node_modules/@dcl/sdk/types/tsconfig.ecs7.json"),
            r#"{ "compilerOptions": { "strict": false } }"#,
        )
        .unwrap();
        assert_eq!(
            known_deprecations(&p, &six),
            None,
            "the extended tsconfig counts"
        );
        remember_deprecations(&p, &six, &reported);

        std::fs::write(tsc(&p), "// tsc, another build").unwrap();
        assert_eq!(
            known_deprecations(&p, &six),
            None,
            "another compiler asks again"
        );
        let _ = std::fs::remove_dir_all(&p.root);
    }

    #[test]
    fn a_tsconfig_with_only_base_url_needs_no_failing_run_to_be_known() {
        let p = scene("foreseen");
        let six = Compiler {
            tsc: tsc(&p),
            version: "6.0.3".to_string(),
        };
        std::fs::write(
            p.root.join("tsconfig.json"),
            r#"{ "compilerOptions": { "baseUrl": "." }, "extends": "@dcl/sdk/types/tsconfig.ecs7.json" }"#,
        )
        .unwrap();
        assert_eq!(
            known_deprecations(&p, &six),
            Some(vec!["baseUrl".to_string()])
        );
        std::fs::write(
            p.root.join("tsconfig.json"),
            r#"{ "compilerOptions": { "paths": {} }, "extends": "@dcl/sdk/types/tsconfig.ecs7.json" }"#,
        )
        .unwrap();
        assert_eq!(known_deprecations(&p, &six), None, "baseUrl removed");
        let _ = std::fs::remove_dir_all(&p.root);
    }

    #[test]
    fn only_sources_count_as_candidates() {
        let p = scene("candidates");
        record(&p, &tsc(&p));
        for rel in [
            "bin/index.js",
            ".dcl-one-scratch.ts",
            ".dcl-one/entrypoint.ts",
            "node_modules/x.ts",
        ] {
            let path = p.root.join(rel);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, "").unwrap();
        }
        assert!(unchanged(&p, &tsc(&p)), "{:?}", candidates(&p.root));
        std::fs::create_dir_all(p.root.join("assets/scene")).unwrap();
        std::fs::write(p.root.join("assets/scene/entity-names.ts"), "").unwrap();
        assert!(!unchanged(&p, &tsc(&p)));
        let _ = std::fs::remove_dir_all(&p.root);
    }
}

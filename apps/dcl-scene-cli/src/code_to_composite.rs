//! `code-to-composite`: run the scene's code once under node against a mock
//! renderer, keep what the visual editor can edit and write it as
//! `assets/scene/main.composite` + `main.crdt`; the code under `src/` is then
//! commented out under upstream's marker and the scene rebuilt from its
//! composite, so `start` and `deploy` see the converted scene right away.

use crate::build::{self, BuildOptions};
use crate::scene::Project;
use crate::ux::{self, TrySteps, UserError};
use crate::workspace::Workspace;
use anyhow::{Context, Result};
use std::io::{IsTerminal, Write};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;

pub struct CodeToCompositeOptions {
    pub dir: PathBuf,
    /// Convert without the confirmation prompt.
    pub yes: bool,
}

/// Upstream's marker: a file carrying it is already converted and is left alone.
pub const COMMENT_MARKER: &str = "COMMENTED BY \"@dcl/sdk-commands code-to-composite\" COMMAND";

const DRIVER_TEMPLATE: &str = include_str!("templates/code-to-composite.mjs");
const RUN_TIMEOUT: Duration = Duration::from_secs(120);

#[derive(Debug, Default)]
pub struct Converted {
    pub entities: u64,
    pub components: u64,
    pub named: u64,
    pub ui_entities: u64,
    pub assets: Vec<(String, String)>,
    pub missing_assets: Vec<String>,
    /// `<model>: <uri>` for a glTF dependency that is not a relative path
    /// inside the model's folder (a URL, an absolute path, `..`): left as is.
    pub skipped_dependencies: Vec<String>,
    pub commented: usize,
}

pub async fn code_to_composite(opts: &CodeToCompositeOptions) -> Result<()> {
    let ws = Workspace::load(&opts.dir)?;
    if !confirm(opts.yes)? {
        ux::note("Operation cancelled by user.");
        return Ok(());
    }
    for (i, project) in ws.projects.iter().enumerate() {
        if let Some(header) = ws.member_header(i) {
            println!("{header}");
        }
        if project.root.join("wearable.json").is_file() {
            tracing::warn!(
                "{} is a smart wearable, not a scene — skipping",
                project.root.display()
            );
            continue;
        }
        convert(project).await?;
    }
    Ok(())
}

fn confirm(yes: bool) -> Result<bool> {
    if yes {
        return Ok(true);
    }
    tracing::warn!(
        "This command will modify your scene code by commenting it out and creating composite/CRDT files."
    );
    for line in [
        "  - every file under src/ is kept, commented out line by line",
        "  - referenced models, images, audio and video move under assets/scene/",
        "  - the scene is rebuilt from assets/scene/main.composite afterwards",
        "  - without version control, undoing this is cumbersome",
    ] {
        ux::note(line);
    }
    if !(std::io::stdin().is_terminal() && std::io::stdout().is_terminal() && !ux::ci()) {
        return Err(UserError::new(
            "code-to-composite needs a confirmation and there is no terminal to ask (or CI)",
            TrySteps::one("re-run with --yes (-y) to convert without the prompt"),
        )
        .into());
    }
    print!("Do you want to continue? [y/N] ");
    std::io::stdout().flush().ok();
    let mut line = String::new();
    std::io::stdin()
        .read_line(&mut line)
        .context("reading the confirmation")?;
    Ok(matches!(
        line.trim().to_ascii_lowercase().as_str(),
        "y" | "yes"
    ))
}

async fn convert(project: &Project) -> Result<Converted> {
    let root = &project.root;
    let mut steps = ux::Steps::new(4);

    build::build(&build_options(root, true)).await?;
    steps.done("Scene built without its composite");

    let mut converted = run_driver(root).await?;
    steps.done(format!(
        "Scene run once: {} entit{} in {} component type{} captured",
        converted.entities,
        if converted.entities == 1 { "y" } else { "ies" },
        converted.components,
        if converted.components == 1 { "" } else { "s" },
    ));
    for (from, to) in &converted.assets {
        ux::note(format!("  {from} -> {to}"));
    }
    for missing in &converted.missing_assets {
        tracing::warn!("asset not found, its path is left as is: {missing}");
    }
    for skipped in &converted.skipped_dependencies {
        tracing::warn!(
            "glTF dependency left as is, not a relative path inside the model's folder: {skipped}"
        );
    }
    if converted.ui_entities > 0 {
        ux::note(format!(
            "  {} UI entit{} left to code (the editor does not place react-ecs UI)",
            converted.ui_entities,
            if converted.ui_entities == 1 {
                "y"
            } else {
                "ies"
            }
        ));
    }

    converted.commented = comment_sources(root)?;
    steps.done(format!(
        "Commented out {} source file{} under src/",
        converted.commented,
        if converted.commented == 1 { "" } else { "s" }
    ));

    build::build(&build_options(root, false)).await?;
    steps.done("Scene rebuilt from assets/scene/main.composite");

    ux::note_good("Scene converted to the visual-editor format");
    ux::note("  open it with dcl-scene-cli start --data-layer, or in the Creator Hub");
    ux::note(
        "  uncomment parts of src/ to keep them dynamic; assets/scene/entity-names.ts names the entities",
    );
    Ok(converted)
}

fn build_options(root: &Path, ignore_composite: bool) -> BuildOptions {
    BuildOptions {
        built_in: true,
        dir: root.to_path_buf(),
        production: false,
        ignore_composite,
        custom_entry_point: false,
        skip_type_check: false,
        out_root: None,
        quiet: true,
        emit_declaration: false,
        preview: false,
    }
}

pub fn write_driver(root: &Path) -> Result<PathBuf> {
    let dir = crate::scene::work_dir(root)
        .with_context(|| format!("creating {}", root.join(".dcl-one").display()))?;
    let path = dir.join("code-to-composite.mjs");
    std::fs::write(&path, DRIVER_TEMPLATE)
        .with_context(|| format!("writing {}", path.display()))?;
    Ok(path)
}

async fn run_driver(root: &Path) -> Result<Converted> {
    let node = build::require_node(
        "code-to-composite",
        "the scene has to run once under node to capture what its code creates",
    )?;
    let driver = write_driver(root)?;
    let _progress = ux::Slow::start("running the scene");
    let out = tokio::time::timeout(
        RUN_TIMEOUT,
        tokio::process::Command::new(&node)
            .arg(&driver)
            .arg(root)
            .current_dir(root)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .output(),
    )
    .await
    .map_err(|_| {
        anyhow::Error::from(UserError::new(
            "the scene did not finish starting within two minutes",
            TrySteps::one("make main() return: code-to-composite captures the first frame only")
                .and("re-run with --verbose to see the scene's console output"),
        ))
    })?
    .map_err(|e| {
        anyhow::Error::from(
            UserError::new(
                "could not run the scene (node)",
                TrySteps::one("check node runs: node --version"),
            )
            .caused_by(e),
        )
    })?;
    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    if !out.status.success() {
        return Err(UserError::new(
            "the scene failed while running under node — nothing was written",
            TrySteps::one(
                "fix the error below, or guard code that needs a real explorer behind a system",
            )
            .and("run npm install if @dcl/inspector-service cannot be resolved"),
        )
        .why(
            format!("{}\n{}", stdout.trim(), stderr.trim())
                .trim()
                .to_string(),
        )
        .into());
    }
    if ux::verbose() && !stderr.trim().is_empty() {
        for line in stderr.lines() {
            ux::note(format!("  scene: {line}"));
        }
    }
    let summary = stdout
        .lines()
        .rev()
        .find_map(|l| serde_json::from_str::<serde_json::Value>(l.trim()).ok())
        .unwrap_or_default();
    if summary.get("ok").and_then(|v| v.as_bool()) != Some(true) {
        return Err(UserError::new(
            "the scene ran but reported no result",
            TrySteps::one("re-run with --verbose"),
        )
        .why(
            format!("{}\n{}", stdout.trim(), stderr.trim())
                .trim()
                .to_string(),
        )
        .into());
    }
    Ok(parse_summary(&summary))
}

fn parse_summary(summary: &serde_json::Value) -> Converted {
    let count = |key: &str| summary.get(key).and_then(|v| v.as_u64()).unwrap_or(0);
    let strings = |value: Option<&serde_json::Value>| -> Vec<String> {
        value
            .and_then(|v| v.as_array())
            .map(|a| {
                a.iter()
                    .filter_map(|s| s.as_str().map(str::to_string))
                    .collect()
            })
            .unwrap_or_default()
    };
    let assets = summary
        .get("assets")
        .and_then(|v| v.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|m| {
                    Some((
                        m.get("from")?.as_str()?.to_string(),
                        m.get("to")?.as_str()?.to_string(),
                    ))
                })
                .collect()
        })
        .unwrap_or_default();
    Converted {
        entities: count("entities"),
        components: count("components"),
        named: count("named"),
        ui_entities: count("uiEntities"),
        assets,
        missing_assets: strings(summary.get("missingAssets")),
        skipped_dependencies: strings(summary.get("skippedDependencies")),
        commented: 0,
    }
}

/// Every `.ts`, `.tsx`, `.js` and `.jsx` under `src/` except declarations
/// and anything under a `node_modules`, sorted. Upstream globs the same set.
pub fn source_files(src: &Path) -> Vec<PathBuf> {
    if !src.is_dir() {
        return Vec::new();
    }
    let mut files: Vec<PathBuf> = ignore::WalkBuilder::new(src)
        .standard_filters(false)
        .filter_entry(|e| e.file_name() != "node_modules")
        .build()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_some_and(|t| t.is_file()))
        .map(|e| e.into_path())
        .filter(|p| {
            let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("");
            let ext = p.extension().and_then(|e| e.to_str()).unwrap_or("");
            matches!(ext, "ts" | "tsx" | "js" | "jsx") && !name.ends_with(".d.ts")
        })
        .collect();
    files.sort();
    files
}

/// The file commented out line by line under upstream's header; None when it
/// already carries the marker. `export {}` keeps it a module, so the type
/// check still accepts it (and the entrypoint's `import *` of it).
pub fn comment_out(content: &str) -> Option<String> {
    if content.contains(COMMENT_MARKER) {
        return None;
    }
    let mut out = String::with_capacity(content.len() + 700);
    out.push_str(
        "// ============================================================================\n",
    );
    out.push_str(&format!("// {COMMENT_MARKER}\n"));
    out.push_str("//\n");
    out.push_str("// This code has been migrated to main.composite and main.crdt files.\n");
    out.push_str("// The scene can now be edited visually using the Creator Hub app.\n");
    out.push_str("//\n");
    out.push_str("// To restore code-based workflow: Uncomment the code below\n");
    out.push_str("// For hybrid workflow: Uncomment specific parts you want to keep dynamic\n");
    out.push_str("//\n");
    out.push_str("// Learn more: https://docs.decentraland.org/creator\n");
    out.push_str(
        "// ============================================================================\n",
    );
    out.push_str("//\n");
    for line in content.split('\n') {
        out.push_str("// ");
        out.push_str(line);
        out.push('\n');
    }
    out.push_str("export {}\n");
    Some(out)
}

fn comment_sources(root: &Path) -> Result<usize> {
    let mut commented = 0;
    for path in source_files(&root.join("src")) {
        let content = std::fs::read_to_string(&path)
            .with_context(|| format!("reading {}", path.display()))?;
        if let Some(out) = comment_out(&content) {
            std::fs::write(&path, out).map_err(|e| ux::write_error(&path, e))?;
            commented += 1;
        }
    }
    Ok(commented)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crdt_gen::TestDir;

    #[test]
    fn commenting_prefixes_every_line_once_and_keeps_a_module() {
        let src = "import { engine } from '@dcl/sdk/ecs'\n\nexport function main() {\n  engine.addEntity()\n}\n";
        let out = comment_out(src).unwrap();
        assert!(out.starts_with("// ====="), "{out}");
        assert!(out.contains(&format!("// {COMMENT_MARKER}\n")));
        for line in src.lines() {
            assert!(
                out.contains(&format!("// {line}\n")),
                "missing {line:?} in {out}"
            );
        }
        assert!(out.ends_with("// \nexport {}\n"), "{out}");
        assert!(
            !out.lines()
                .any(|l| !l.starts_with("//") && l != "export {}"),
            "{out}"
        );
        assert!(
            comment_out(&out).is_none(),
            "a converted file is left alone"
        );
    }

    #[test]
    fn source_files_take_ts_tsx_js_jsx_but_not_declarations_or_node_modules() {
        let t = TestDir::new("c2c-sources");
        for rel in [
            "src/index.ts",
            "src/ui/panel.tsx",
            "src/legacy/helper.js",
            "src/legacy/widget.jsx",
            "src/types/globals.d.ts",
            "src/node_modules/dep/index.ts",
            "src/data/table.json",
            "src/notes.md",
        ] {
            t.write(rel, "x");
        }
        let found: Vec<String> = source_files(&t.0.join("src"))
            .iter()
            .map(|p| {
                p.strip_prefix(&t.0)
                    .unwrap()
                    .to_string_lossy()
                    .replace('\\', "/")
            })
            .collect();
        assert_eq!(
            found,
            [
                "src/index.ts",
                "src/legacy/helper.js",
                "src/legacy/widget.jsx",
                "src/ui/panel.tsx",
            ]
        );
        assert!(source_files(&t.0.join("nope")).is_empty());
    }

    #[test]
    fn the_summary_line_is_read_whole() {
        let summary: serde_json::Value = serde_json::from_str(
            r#"{"ok":true,"entities":3,"components":4,"uiEntities":1,"named":2,
                "assets":[{"from":"models/a.glb","to":"assets/scene/Models/a/a.glb"}],
                "missingAssets":["sounds/x.mp3"],"skippedDependencies":["models/a.glb: ../x.bin"],
                "rewritten":1}"#,
        )
        .unwrap();
        let c = parse_summary(&summary);
        assert_eq!(
            (c.entities, c.components, c.named, c.ui_entities),
            (3, 4, 2, 1)
        );
        assert_eq!(
            c.assets,
            [(
                "models/a.glb".to_string(),
                "assets/scene/Models/a/a.glb".to_string()
            )]
        );
        assert_eq!(c.missing_assets, ["sounds/x.mp3"]);
        assert_eq!(c.skipped_dependencies, ["models/a.glb: ../x.bin"]);
        let empty = parse_summary(&serde_json::json!({"ok": true}));
        assert_eq!(empty.entities, 0);
        assert!(empty.assets.is_empty());
    }
}

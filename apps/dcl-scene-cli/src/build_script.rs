//! The package.json build contract, shared by build, preview/watch and deploy.
use crate::{
    build::{BuildOptions, Built},
    scene::Project,
    ux,
};
use anyhow::{Context, Result, bail};
use std::path::{Component, Path};
use std::process::Stdio;

const ACTIVE: &str = "DCL_SCENE_CLI_BUILD_SCRIPT";
/// Set to 1 when the caller narrates the build itself (the preview's step line and its
/// `\u{21bb} rebuilt` line), so a script can keep a passing build silent.
const QUIET: &str = "DCL_SCENE_CLI_QUIET";

/// `bin/index.js (256.5kb, 1.10 sec)`: what a caller that narrates the build says about it.
pub fn summary(built: &Built, elapsed: std::time::Duration, restore: &str) -> String {
    let size = std::fs::metadata(&built.outfile)
        .map(|m| m.len())
        .unwrap_or(0);
    format!(
        "{} ({}, {})",
        ux::rel_to(&built.project.root, &built.outfile),
        ux::fmt_bytes(size),
        ux::fmt_elapsed_tinted(elapsed, restore)
    )
}

pub fn command(root: &Path) -> Result<Option<String>> {
    let file = root.join("package.json");
    let bytes = match std::fs::read(&file) {
        Ok(bytes) => bytes,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e).with_context(|| format!("reading {}", file.display())),
    };
    let package: serde_json::Value =
        serde_json::from_slice(&bytes).with_context(|| format!("parsing {}", file.display()))?;
    let Some(value) = package.get("scripts").and_then(|s| s.get("build")) else {
        return Ok(None);
    };
    let script = value
        .as_str()
        .context("package.json scripts.build must be a nonempty string")?
        .trim();
    if script.is_empty() {
        bail!("package.json scripts.build must be a nonempty string");
    }
    // Existing SDK scaffolds delegated straight back to us. Preserve them as
    // a built-in alias; more complex delegations must use --built-in explicitly.
    if matches!(script, "dcl-scene-cli build" | "dcl-scene-cli build --built-in") {
        return Ok(None);
    }
    Ok(Some(script.to_owned()))
}

pub fn selected(project: &Project, opts: &BuildOptions) -> Result<Option<String>> {
    if opts.built_in {
        return Ok(None);
    }
    command(&project.root)
}

pub async fn run(project: Project, opts: &BuildOptions, script: &str) -> Result<Built> {
    if std::env::var_os(ACTIVE).is_some() {
        bail!(
            "recursive package.json scripts.build invocation; use dcl-scene-cli build --built-in inside the build script"
        );
    }
    let main = project.main_output()?;
    if Path::new(&main)
        .components()
        .any(|c| !matches!(c, Component::Normal(_) | Component::CurDir))
    {
        bail!("custom build scene.json main must be a relative path inside the project");
    }
    let outfile = project.root.join(&main);
    let mut paths = vec![project.root.join("node_modules/.bin")];
    paths.extend(std::env::split_paths(
        &std::env::var_os("PATH").unwrap_or_default(),
    ));
    #[cfg(windows)]
    let mut child = {
        let mut c = tokio::process::Command::new("cmd.exe");
        c.args(["/d", "/s", "/c", script]);
        c
    };
    #[cfg(not(windows))]
    let mut child = {
        let mut c = tokio::process::Command::new("sh");
        c.args(["-c", script]);
        c
    };
    if !opts.quiet {
        ux::note(format!("scripts.build: {script}"));
    }
    let started = std::time::Instant::now();
    let status = child
        .current_dir(&project.root)
        .env("PATH", std::env::join_paths(paths)?)
        .env(ACTIVE, &project.root)
        .env(
            "DCL_SCENE_CLI_PRODUCTION",
            if opts.production { "1" } else { "0" },
        )
        .env("DCL_SCENE_CLI_MAIN", &main)
        .env(QUIET, if opts.quiet { "1" } else { "0" })
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .kill_on_drop(true)
        .status()
        .await
        .context("starting package.json scripts.build")?;
    if !status.success() {
        return Err(ux::UserError::new(
            format!("package.json scripts.build failed ({status})"),
            ux::TrySteps::one(
                "fix what the build script reported above; its output is the whole story",
            )
            .and("run it on its own to iterate faster: npm run build"),
        )
        .why("the SDK ran no build of its own, and a preview keeps serving the last good bundle")
        .into());
    }
    let metadata = std::fs::metadata(&outfile)
        .with_context(|| format!("scripts.build succeeded but did not produce {main}"))?;
    if !metadata.is_file() || metadata.len() == 0 {
        bail!("scripts.build must produce a nonempty bundle at {main}");
    }
    if !dunce::canonicalize(&outfile)?.starts_with(&project.root) {
        bail!("scripts.build output {main} resolves outside the project");
    }
    let built = Built { project, outfile };
    if !opts.quiet {
        ux::note(format!(
            "built {}",
            summary(&built, started.elapsed(), ux::RESTORE_DIM)
        ));
    }
    Ok(built)
}

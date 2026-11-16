//! Optimized textures: the images GLB and glTF models sample become UASTC
//! KTX2 files (`KHR_texture_basisu`), staged under [`STAGE`] and laid over the
//! scene's file list. The creator's files are never written to: rewritten
//! models and new `.ktx2` files live in the staging directory, and an original
//! image only drops out of the published list ([`Overlay::removed`]).
//!
//! While the project's setting ([`settings`]) enables it, `start` imports in
//! the background ([`import`]), like a game engine's asset import: every run
//! fills the cache, and its finished overlay replaces the previous one in one
//! swap. `deploy`, `pack` and `export-static` finish whatever the cache lacks
//! in the foreground ([`stage_for_deploy`]) and read their payload through
//! their own run's overlay, never the preview's registry entry.
//!
//! The overlay is applied where the payload is decided — the payload's
//! `with_release_files`/`payload_path` ([`payload_publishable`],
//! [`payload_source`]) and the preview's scene entity and `/content/contents`
//! ([`publishable`], [`staged_source`]) — so the preview serves the bytes a
//! deploy uploads. With no enabled overlay registered every function here
//! hands its input back unchanged.
//!
//! A run keeps the stage's cache, KTX2 files and staged models consistent
//! with one lock across processes ([`lock_stage`]); a file's hash and a GLB's
//! plan are kept per size and time, so a save that changes no image or model
//! re-encodes, re-reads and re-parses nothing.

mod basisu;
mod encode;
mod gltf;
pub mod import;
pub mod settings;

pub use encode::Encoder;
pub use settings::{Overrides, ProjectSettings};

use crate::ux;
use anyhow::{Context, Result};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard, OnceLock, PoisonError};
use std::time::SystemTime;

/// The longer side a texture is encoded at, at most: what the explorer's own
/// texture path resizes to.
pub const MAX_TEXTURE_SIZE: u32 = 1024;
/// The staging directory, scene-relative: inside `.dcl-one`, which is never
/// published.
pub const STAGE: &str = ".dcl-one/optimized-textures";
/// The scene-relative path a model's KTX2 is published at: beside the model,
/// named by the sha256 of its bytes. The model names it with no `..`, which
/// every client resolves the same way; models in different folders sharing a
/// texture publish one file per folder, and content addressing uploads and
/// stores those bytes once.
pub fn published_ktx2(model_rel: &str, ktx2_sha: &str) -> String {
    match model_rel.rsplit_once('/') {
        Some((dir, _)) => format!("{dir}/{ktx2_sha}.ktx2"),
        None => format!("{ktx2_sha}.ktx2"),
    }
}
/// The background import's manifest, read back by `deploy` to compare which
/// originals each left out.
pub const PREVIEW_MANIFEST: &str = "manifest.json";
/// The last deploy's manifest.
pub const DEPLOY_MANIFEST: &str = "deploy-manifest.json";
/// Files searched for an original image's name before it is left out: the
/// bundle and its chunks, `main.crdt`, composites, `scene.json` and other
/// JSON the scene publishes.
const SCANNED_EXTENSIONS: [&str; 6] = ["js", "mjs", "cjs", "json", "crdt", "composite"];

pub const KTX2_WARNING: &str = "KTX2 textures: clients without KTX2 support (Unity's local \
    raw-glTF preview, older Bevy explorer builds) show these models untextured; Unity production \
    needs abgen's KTX2 support";

const IGNORED_REASON: &str = "replaced by KTX2 in every model using it; not named by scene code";

/// How a texture's texels are read, which picks the encoder's metrics and
/// the KTX2 transfer function.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ColorMode {
    Srgb,
    Linear,
    Normal,
}

impl ColorMode {
    pub fn as_str(self) -> &'static str {
        match self {
            ColorMode::Srgb => "srgb",
            ColorMode::Linear => "linear",
            ColorMode::Normal => "normal",
        }
    }
}

/// How one run encodes and what it leaves out.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Settings {
    /// `--keep-source-textures`: publish every original image, replaced or not.
    pub keep_source_textures: bool,
    pub max_size: u32,
}

impl Settings {
    pub fn for_project(project: ProjectSettings) -> Self {
        Settings {
            keep_source_textures: project.keep_source_textures,
            ..Default::default()
        }
    }
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            keep_source_textures: false,
            max_size: MAX_TEXTURE_SIZE,
        }
    }
}

/// What the published file list and bytes become.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct Overlay {
    /// Scene paths whose bytes come from a staged file: the rewritten models
    /// and the new KTX2 files.
    pub sources: BTreeMap<String, PathBuf>,
    /// The new KTX2 paths, appended to the list.
    pub added: BTreeSet<String>,
    /// Original images left out of the list.
    pub removed: BTreeSet<String>,
}

impl Overlay {
    pub fn apply(&self, rels: Vec<String>) -> Vec<String> {
        self.apply_with(rels, true)
    }

    /// [`Overlay::apply`], leaving the originals in when `remove` is false.
    fn apply_with(&self, rels: Vec<String>, remove: bool) -> Vec<String> {
        let mut out: Vec<String> = rels
            .into_iter()
            .filter(|r| !remove || !self.removed.contains(r))
            .collect();
        let present: std::collections::HashSet<String> = out.iter().cloned().collect();
        out.extend(self.added.iter().filter(|a| !present.contains(*a)).cloned());
        out
    }

    pub fn source(&self, rel: &str) -> Option<&Path> {
        self.sources.get(rel).map(PathBuf::as_path)
    }
}

/// A file as last seen: a moved stamp means the inputs changed.
type Stamp = (String, Option<FileKey>);

/// One project's last finished background run: what the preview serves.
#[derive(Clone, Debug, Default)]
struct Entry {
    /// The project setting the run read: whether the overlay is applied.
    enabled: bool,
    settings: Settings,
    overlay: Option<Arc<Overlay>>,
    /// The inputs the overlay was built from; `None` forces the next run.
    stamps: Option<Vec<Stamp>>,
    /// Each rewritten model's size and time when the run read it.
    model_inputs: Arc<HashMap<String, FileKey>>,
    /// The stage files the run read or publishes: what any sweep in this
    /// process keeps while the preview serves this run.
    keep: Arc<BTreeSet<PathBuf>>,
}

/// Projects this process has imported, by canonical root.
fn registry() -> MutexGuard<'static, HashMap<PathBuf, Entry>> {
    static REGISTRY: OnceLock<Mutex<HashMap<PathBuf, Entry>>> = OnceLock::new();
    REGISTRY
        .get_or_init(Default::default)
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
}

/// The overlay each deploy in progress reads its payload through, by root:
/// its own run's, or `None` with the setting off. Apart from [`registry`], so
/// a deploy never changes what the preview serves.
fn deploys() -> MutexGuard<'static, HashMap<PathBuf, Option<Arc<Overlay>>>> {
    static DEPLOYS: OnceLock<Mutex<HashMap<PathBuf, Option<Arc<Overlay>>>>> = OnceLock::new();
    DEPLOYS
        .get_or_init(Default::default)
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
}

/// One run at a time in this process, background or deploy: a deploy holds
/// it from its own run until its payload is read, so no background swap or
/// prune lands in between.
static RUN_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

/// The stage's lock file. Held exclusively by whatever writes, prunes or
/// sweeps the stage, or reads a payload from it, across processes: a `start`
/// and a `deploy` in another terminal share `.dcl-one/optimized-textures`.
fn stage_lock_file(root: &Path) -> std::io::Result<std::fs::File> {
    let stage = root.join(STAGE);
    std::fs::create_dir_all(&stage)?;
    std::fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(stage.join(".lock"))
}

/// Takes the stage's lock, waiting for another process that holds it.
fn lock_stage(root: &Path) -> std::io::Result<std::fs::File> {
    let file = stage_lock_file(root)?;
    file.lock()?;
    Ok(file)
}

/// Takes the stage's lock when it is free; `None` when another process holds it.
fn try_lock_stage(root: &Path) -> std::io::Result<Option<std::fs::File>> {
    let file = stage_lock_file(root)?;
    match file.try_lock() {
        Ok(()) => Ok(Some(file)),
        Err(std::fs::TryLockError::WouldBlock) => Ok(None),
        Err(std::fs::TryLockError::Error(e)) => Err(e),
    }
}

/// The overlay the preview applies, when the project enables it, with the
/// inputs of the models it rewrote.
fn overlay(root: &Path) -> Option<(Arc<Overlay>, Arc<HashMap<String, FileKey>>)> {
    let reg = registry();
    let entry = reg.get(root)?;
    let overlay = entry.overlay.clone().filter(|_| entry.enabled)?;
    Some((overlay, entry.model_inputs.clone()))
}

/// Whether model `rel` moved since the overlay's run read it: until the next
/// run swaps its new staged copy in, the preview serves the source.
fn is_stale(root: &Path, inputs: &HashMap<String, FileKey>, rel: &str) -> bool {
    inputs
        .get(rel)
        .is_some_and(|key| file_key(&root.join(rel)) != Some(*key))
}

/// Replaces `root`'s entry in one step and says whether what a client sees
/// changed: the overlay's files, or whether it is applied.
fn swap(root: &Path, next: Entry) -> bool {
    let mut reg = registry();
    let previous = reg.insert(root.to_path_buf(), next.clone());
    let (was_enabled, was) = match &previous {
        Some(e) => (e.enabled, e.overlay.as_deref().cloned().unwrap_or_default()),
        None => (false, Overlay::default()),
    };
    let now = next.overlay.as_deref().cloned().unwrap_or_default();
    was_enabled != next.enabled || was != now
}

/// The list the preview serves, with the overlay applied; `rels` itself when
/// `root` is not optimized. While a rewritten model has moved since the run
/// that staged it, its source is served (see [`staged_source`]) and no
/// original drops out, since the source names them.
pub fn publishable(root: &Path, rels: Vec<String>) -> Vec<String> {
    match overlay(root) {
        Some((o, inputs)) => {
            let any_stale = inputs.keys().any(|rel| is_stale(root, &inputs, rel));
            o.apply_with(rels, !any_stale)
        }
        None => rels,
    }
}

/// The staged file holding `rel`'s bytes in the preview, when the overlay has
/// one and, for a model, the model has not moved since the run read it.
pub fn staged_source(root: &Path, rel: &str) -> Option<PathBuf> {
    let (overlay, inputs) = overlay(root)?;
    if is_stale(root, &inputs, rel) {
        return None;
    }
    overlay.source(rel).map(Path::to_path_buf)
}

/// The list a payload carries: while a [`DeployStage`] for `root` is held,
/// that run's overlay (none with the setting off); otherwise the preview's.
pub fn payload_publishable(root: &Path, rels: Vec<String>) -> Vec<String> {
    match deploys().get(root).cloned() {
        Some(Some(o)) => o.apply(rels),
        Some(None) => rels,
        None => publishable(root, rels),
    }
}

/// The staged file holding `rel`'s payload bytes, read as
/// [`payload_publishable`] lists them.
pub fn payload_source(root: &Path, rel: &str) -> Option<PathBuf> {
    match deploys().get(root).cloned() {
        Some(o) => o?.source(rel).map(Path::to_path_buf),
        None => staged_source(root, rel),
    }
}

fn stamp(rel: &str, path: &Path) -> Stamp {
    (rel.to_string(), file_key(path))
}

/// The preview's inputs: the tree files it serves.
fn stamps_of(root: &Path, rels: &[String]) -> Vec<Stamp> {
    rels.iter().map(|rel| stamp(rel, &root.join(rel))).collect()
}

/// While held, the payload `deploy`, `pack` or `export-static` reads carries
/// its own run's overlay (or none, with the setting off), and no other run, in
/// this process or another, touches the stage. Dropping it asks the
/// background import, if any, to catch up with the setting it may have stored.
pub struct DeployStage {
    root: PathBuf,
    /// With the setting off, none: the stage is not read, so nothing is held.
    _turn: Option<tokio::sync::MutexGuard<'static, ()>>,
    lock: Option<std::fs::File>,
    /// The stage files this run read or publishes, kept by the sweep that
    /// runs when the stage is dropped, with the preview's.
    keep: Option<BTreeSet<PathBuf>>,
}

impl Drop for DeployStage {
    fn drop(&mut self) {
        // still under the stage lock: the fields drop after this body
        if self.lock.is_some()
            && let Some(keep) = self.keep.take()
        {
            let mut keep = keep;
            keep.extend(preview_keep(&self.root));
            sweep_stage(&self.root.join(STAGE), &keep);
        }
        deploys().remove(&self.root);
        import::request(&self.root);
    }
}

/// The stage files the preview in this process serves, which no sweep here
/// deletes.
fn preview_keep(root: &Path) -> BTreeSet<PathBuf> {
    registry()
        .get(root)
        .map(|entry| (*entry.keep).clone())
        .unwrap_or_default()
}

/// With the setting off, the stage's files serve nothing: deleted when the
/// stage exists and no other process holds it, but those the preview here
/// still serves. Nothing is created, and a lock that can't be taken skips it.
fn sweep_when_off(root: &Path) {
    let stage = root.join(STAGE);
    if !stage.is_dir() {
        return;
    }
    if let Ok(Some(_lock)) = try_lock_stage(root) {
        sweep_stage(&stage, &preview_keep(root));
    }
}

/// `deploy`, `pack` and `export-static`, after the build and before the
/// payload is read: applies the command line's `overrides` to the project
/// setting and, when it is on, runs over the release payload. Textures the
/// cache has (the preview's background import put them there) cost nothing;
/// the rest are encoded here with a progress line. A run that fails stops the
/// command rather than publishing a half-optimized scene; a texture that fails
/// alone keeps its original.
pub async fn stage_for_deploy(
    root: &Path,
    overrides: Overrides,
    quiet: bool,
) -> Result<DeployStage> {
    let project = settings::apply(root, overrides)?;
    if !project.optimize_textures {
        // the payload is the tree's: no lock, no stage, nothing to wait for
        sweep_when_off(root);
        deploys().insert(root.to_path_buf(), None);
        return Ok(DeployStage {
            root: root.to_path_buf(),
            _turn: None,
            lock: None,
            keep: None,
        });
    }
    let turn = match RUN_LOCK.try_lock() {
        Ok(turn) => turn,
        Err(_) => {
            if !quiet {
                ux::note("waiting for the texture import in progress");
            }
            RUN_LOCK.lock().await
        }
    };
    let lock = match try_lock_stage(root).context("locking the texture stage")? {
        Some(lock) => lock,
        None => {
            if !quiet {
                ux::note("waiting for another dcl-scene-cli working on this project's textures");
            }
            let owned = root.to_path_buf();
            tokio::task::spawn_blocking(move || lock_stage(&owned))
                .await
                .context("waiting for the texture stage")?
                .context("locking the texture stage")?
        }
    };
    let mut stage = DeployStage {
        root: root.to_path_buf(),
        _turn: Some(turn),
        lock: Some(lock),
        keep: None,
    };
    let settings = Settings::for_project(project);
    let owned = root.to_path_buf();
    let (overlay, keep) = tokio::task::spawn_blocking(move || deploy_run(&owned, settings, quiet))
        .await
        .context("the texture optimizer stopped")??;
    stage.keep = Some(keep);
    deploys().insert(root.to_path_buf(), Some(Arc::new(overlay)));
    Ok(stage)
}

/// One foreground run for `deploy`, on every core, over the files it uploads:
/// the tree's publishable files and the release build's own, each read where
/// the payload reads it.
fn deploy_run(
    root: &Path,
    settings: Settings,
    quiet: bool,
) -> Result<(Overlay, BTreeSet<PathBuf>)> {
    let rels =
        crate::deploy::tree_payload_files(root, crate::deploy::collect_publishable_files(root)?);
    let encoder = basisu::WasmBasisu::shared()?;
    let line = (!quiet).then(|| ux::Slow::start("optimizing textures"));
    let announced = std::sync::atomic::AtomicBool::new(false);
    let progress = |p: Progress| {
        let Some(line) = &line else { return };
        if !announced.swap(true, std::sync::atomic::Ordering::Relaxed) {
            ux::note(format!(
                "encoding {} of {} texture(s) the cache does not have yet",
                p.total - p.done,
                p.total
            ));
        }
        line.set_detail(format!("{}/{}", p.done, p.total));
    };
    let outcome = optimize(
        root,
        &rels,
        settings,
        encoder,
        &|rel| crate::deploy::tree_payload_path(root, rel),
        &Run {
            workers: encode::workers(),
            progress: &progress,
        },
    )
    .context("optimizing textures (pass --no-optimize-textures to publish the original files)")?;
    drop(line);
    write_manifest(root, DEPLOY_MANIFEST, &outcome.manifest)?;
    if !quiet {
        outcome.summary.report(false, DEPLOY_MANIFEST);
    }
    if !outcome.summary.ignored.is_empty() {
        ux::note_absent(ignored_warning(outcome.summary.ignored.len()));
    }
    if let Some(preview) = read_manifest(root, PREVIEW_MANIFEST)
        && let Some(warning) = ignore_difference_warning(&preview, &outcome.overlay.removed)
    {
        ux::note_absent(warning);
    }
    let keep = outcome.keep();
    Ok((outcome.overlay, keep))
}

/// The deploy's loud line about originals left out.
pub fn ignored_warning(count: usize) -> String {
    format!(
        "{count} original image(s) left out of the upload; scene code that builds texture paths at \
         runtime would 404 for them \u{2014} check the preview or use --keep-source-textures"
    )
}

/// Which originals the preview's last run left out and this deploy keeps, and
/// the reverse; `None` when they agree. The preview searches the dev bundle,
/// a deploy the release bundle, so a name only one of them carries moves an
/// image across.
pub fn ignore_difference_warning(
    preview_manifest: &Value,
    deployed: &BTreeSet<String>,
) -> Option<String> {
    let previewed: BTreeSet<String> = preview_manifest["ignoredOriginals"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|e| e["path"].as_str().map(str::to_string))
        .collect();
    let only_preview: Vec<&String> = previewed.difference(deployed).collect();
    let only_deploy: Vec<&String> = deployed.difference(&previewed).collect();
    if only_preview.is_empty() && only_deploy.is_empty() {
        return None;
    }
    let list = |v: &[&String]| match v.is_empty() {
        true => "none".to_string(),
        false => v.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(", "),
    };
    Some(format!(
        "the deploy leaves out different originals than the preview did (the release bundle is \
         searched, not the preview's): uploaded now but left out in the preview: {}; left out now \
         but served in the preview: {}",
        list(&only_preview),
        list(&only_deploy)
    ))
}

fn manifest_path(root: &Path, name: &str) -> PathBuf {
    root.join(STAGE).join(name)
}

fn write_manifest(root: &Path, name: &str, manifest: &Value) -> Result<()> {
    let mut text = serde_json::to_vec_pretty(manifest).expect("JSON values serialize");
    text.push(b'\n');
    write_if_changed(&manifest_path(root, name), &text).context("writing the texture manifest")
}

fn read_manifest(root: &Path, name: &str) -> Option<Value> {
    serde_json::from_slice(&std::fs::read(manifest_path(root, name)).ok()?).ok()
}

/// How far a run is: `done` of `total` distinct textures, cached ones
/// included. Reported only by runs that encode something.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Progress {
    pub done: usize,
    pub total: usize,
    pub failed: usize,
}

/// How [`optimize`] runs: encoder threads and where progress goes.
pub(crate) struct Run<'a> {
    pub workers: usize,
    pub progress: &'a (dyn Fn(Progress) + Sync),
}

/// What one run did, for the terminal.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct Summary {
    pub models: usize,
    /// KTX2 files the overlay publishes.
    pub textures: usize,
    /// Encoder runs this time, and encodes found in the cache.
    pub encoded: usize,
    pub cached: usize,
    /// `(model and image, why)` for each image left as it was.
    pub kept: Vec<(String, String)>,
    /// What the rewritten models and left-out originals weighed, and what
    /// replaces them.
    pub bytes_before: u64,
    pub bytes_after: u64,
    pub ignored: Vec<String>,
    pub kept_originals: Vec<(String, String)>,
    /// Distinct textures this run handled, and those that failed to encode.
    pub total: usize,
    pub failed: usize,
    /// Source bytes of the images replaced, and the KTX2 bytes replacing
    /// them, each distinct texture once.
    pub converted_bytes: u64,
    pub ktx2_bytes: u64,
}

impl Summary {
    /// The run's lines; `manifest` names the file with the details. The
    /// originals left out are said by the caller: softly by the preview,
    /// loudly by a deploy ([`ignored_warning`]).
    pub fn report(&self, brief: bool, manifest: &str) {
        use crate::deploy::human_size;
        if self.models == 0 {
            ux::note_arrow(format!(
                "Textures: no model texture to optimize ({} kept as they were)",
                self.kept.len()
            ));
            return;
        }
        ux::note_arrow(format!(
            "Textures: {} model(s) rewritten with {} KTX2 file(s) ({} encoded, {} cached), {} image(s) kept as they were; {} \u{2192} {}",
            self.models,
            self.textures,
            self.encoded,
            self.cached,
            self.kept.len(),
            human_size(self.bytes_before),
            human_size(self.bytes_after),
        ));
        if brief {
            return;
        }
        const SHOWN: usize = 5;
        for (path, why) in self.kept_originals.iter().take(SHOWN) {
            ux::note(format!("kept {path}: {why}"));
        }
        if self.kept_originals.len() > SHOWN {
            ux::note(format!(
                "\u{2026} and {} more kept originals",
                self.kept_originals.len() - SHOWN
            ));
        }
        ux::note_absent(KTX2_WARNING);
        ux::note(format!("details: {STAGE}/{manifest}"));
    }
}

/// What [`optimize`] produced.
pub struct Outcome {
    pub overlay: Overlay,
    pub summary: Summary,
    pub manifest: Value,
    /// The cache entries and KTX2 files this run read or publishes: what a
    /// sweep of the stage keeps.
    pub stage_files: BTreeSet<PathBuf>,
    /// Each rewritten model's size and time when the run read it: the preview
    /// serves the source of a model that moved since.
    pub model_inputs: HashMap<String, FileKey>,
}

impl Outcome {
    /// The stage files a sweep keeps for this run: those it read and the
    /// staged files its overlay publishes.
    fn keep(&self) -> BTreeSet<PathBuf> {
        let mut keep = self.stage_files.clone();
        keep.extend(self.overlay.sources.values().cloned());
        keep
    }
}

fn is_model(rel: &str) -> bool {
    let lower = rel.to_ascii_lowercase();
    lower.ends_with(".glb") || lower.ends_with(".gltf")
}

fn scanned(rel: &str) -> bool {
    let lower = rel.to_ascii_lowercase();
    lower
        .rsplit_once('.')
        .is_some_and(|(_, ext)| SCANNED_EXTENSIONS.contains(&ext))
}

/// Where a texture's source bytes are, read only when the texture must be
/// encoded: the cache answers for the rest from the hash alone.
enum JobSource {
    /// A published image file, at its payload path.
    File(PathBuf),
    /// Image `index` of the model at `path` (a buffer view or `data:` URI).
    Embedded {
        path: PathBuf,
        rel: String,
        index: usize,
    },
}

struct Job {
    sha: String,
    mode: ColorMode,
    source: JobSource,
}

#[derive(Debug, Clone, PartialEq)]
enum Encoded {
    Ok {
        ktx2_sha: String,
        len: u64,
        cached: bool,
    },
    /// Kept as its source, and why; `cached` when an earlier run decided it.
    Failed(String),
}

enum Planned {
    Unused,
    Keep(String),
    Encode {
        job: usize,
        source_len: u64,
        mode: ColorMode,
    },
}

struct ImageWork {
    index: usize,
    /// The published file the image names, when it names one.
    external: Option<String>,
    planned: Planned,
}

struct ModelWork {
    rel: String,
    /// Where its bytes are read (the payload path), and as what size and time.
    path: PathBuf,
    key: FileKey,
    original_len: u64,
    images: Vec<ImageWork>,
}

/// What tells a file's bytes moved without reading them: its size and
/// modification time and, on Unix, its inode and change time, which a copy
/// that keeps the modification time (`cp -p`, `rsync -t`, an unzip) still
/// moves. While all hold, its hash and plan are taken as unchanged.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct FileKey {
    pub len: u64,
    modified: Option<SystemTime>,
    /// Inode, then change time in seconds and nanoseconds; zero off Unix.
    changed: (u64, i64, i64),
}

fn file_key(path: &Path) -> Option<FileKey> {
    let meta = std::fs::metadata(path).ok()?;
    #[cfg(unix)]
    let changed = {
        use std::os::unix::fs::MetadataExt;
        (meta.ino(), meta.ctime(), meta.ctime_nsec())
    };
    #[cfg(not(unix))]
    let changed = (0, 0, 0);
    Some(FileKey {
        len: meta.len(),
        modified: meta.modified().ok(),
        changed,
    })
}

/// The sha256 of `path` and its size, read again only when its size or time
/// moved: a run whose images did not change reads none of them.
fn file_sha(path: &Path) -> Option<(String, u64)> {
    use sha2::Digest;
    use std::io::Read;
    static HASHES: OnceLock<Mutex<HashMap<PathBuf, (FileKey, String)>>> = OnceLock::new();
    let hashes = HASHES.get_or_init(Default::default);
    let key = file_key(path)?;
    if let Some((had, sha)) = hashes
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .get(path)
        && *had == key
    {
        return Some((sha.clone(), key.len));
    }
    let mut file = std::fs::File::open(path).ok()?;
    let mut hasher = sha2::Sha256::new();
    let mut buf = vec![0u8; 1 << 16];
    loop {
        let n = file.read(&mut buf).ok()?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    let sha = hex::encode(hasher.finalize().as_slice());
    hashes
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .insert(path.to_path_buf(), (key, sha.clone()));
    Some((sha, key.len))
}

/// What one image of a model is, as [`scan_model`] found it.
#[derive(Clone)]
enum ScanSource {
    /// A file of the scene, by the path the model gives.
    External(String),
    /// Inside the model, by the hash and size of its bytes.
    Embedded { sha: String, len: u64 },
}

#[derive(Clone)]
struct ImageScan {
    plan: gltf::ImagePlan,
    source: Result<ScanSource, String>,
}

/// A model as one run needs it, without its bytes: the plan of each image and
/// what to search for names. Kept per GLB until its size or time moves, so a
/// save that changes no model parses none.
enum ModelScan {
    Parsed {
        len: u64,
        unresolved: Vec<String>,
        images: Vec<ImageScan>,
    },
    /// Not readable as glTF: why, and its text, searched like scene code.
    Unparsed { why: String, text: String },
}

/// [`ModelScan`] of the model `rel` at `path`; `Err` when it can't be read.
/// Kept between runs only for a GLB whose images read nothing outside it: a
/// `.gltf`, or a GLB with an external buffer, reads images through other
/// files, whose changes its own key does not show.
fn scan_model(
    path: &Path,
    rel: &str,
    read: &dyn Fn(&str) -> Option<Vec<u8>>,
) -> std::result::Result<(Arc<ModelScan>, FileKey), String> {
    static SCANS: OnceLock<Mutex<HashMap<PathBuf, (FileKey, Arc<ModelScan>)>>> = OnceLock::new();
    let scans = SCANS.get_or_init(Default::default);
    let key = file_key(path).ok_or_else(|| "model not readable".to_string())?;
    if let Some((had, scan)) = scans
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .get(path)
        && *had == key
    {
        return Ok((scan.clone(), key));
    }
    let bytes = std::fs::read(path).map_err(|e| format!("model not readable: {e}"))?;
    let read_outside = std::cell::Cell::new(false);
    let scan = match gltf::Model::parse(&bytes) {
        Err(e) => ModelScan::Unparsed {
            why: format!("model not optimized: {e:#}"),
            text: String::from_utf8_lossy(&bytes).to_lowercase(),
        },
        Ok(model) => {
            let tracked = |path: &str| {
                read_outside.set(true);
                read(path)
            };
            let images = gltf::plan_images(&model)
                .into_iter()
                .enumerate()
                .map(|(index, plan)| {
                    let source =
                        gltf::image_source(&model, rel, index, &tracked).map(|s| match s {
                            gltf::ImageSource::External(path) => ScanSource::External(path),
                            gltf::ImageSource::Embedded(b) => ScanSource::Embedded {
                                sha: encode::sha256_hex(&b),
                                len: b.len() as u64,
                            },
                        });
                    ImageScan { plan, source }
                })
                .collect();
            ModelScan::Parsed {
                len: bytes.len() as u64,
                unresolved: gltf::unresolved_image_uris(&model, rel),
                images,
            }
        }
    };
    let scan = Arc::new(scan);
    if bytes.starts_with(b"glTF") && !read_outside.get() {
        scans
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .insert(path.to_path_buf(), (key, scan.clone()));
    }
    Ok((scan, key))
}

/// Checks `bytes` against the hash the run planned `job` with: a source
/// changed since then is kept as it is this run and encoded the next.
fn check_planned(job: &Job, bytes: &[u8]) -> std::result::Result<(), String> {
    if encode::sha256_hex(bytes) != job.sha {
        return Err("the source changed during the run".to_string());
    }
    Ok(())
}

/// The jobs of one source, taken together: a file's job alone, or every
/// missing embedded job of one model, so the model is read and parsed once.
fn work_units(jobs: &[Job], missing: &[usize]) -> Vec<Vec<usize>> {
    let mut units: Vec<Vec<usize>> = Vec::new();
    let mut by_model: HashMap<&Path, usize> = HashMap::new();
    for &i in missing {
        match &jobs[i].source {
            JobSource::File(_) => units.push(vec![i]),
            JobSource::Embedded { path, .. } => match by_model.get(path.as_path()) {
                Some(&unit) => units[unit].push(i),
                None => {
                    by_model.insert(path, units.len());
                    units.push(vec![i]);
                }
            },
        }
    }
    units
}

/// The bytes of a unit of embedded jobs: the model read and parsed once, its
/// missing images taken out, the model dropped before they are encoded.
fn embedded_sources(
    jobs: &[Job],
    unit: &[usize],
    read: &dyn Fn(&str) -> Option<Vec<u8>>,
) -> Vec<(usize, std::result::Result<Vec<u8>, String>)> {
    let Some(JobSource::Embedded { path, rel, .. }) = unit.first().map(|&i| &jobs[i].source) else {
        return Vec::new();
    };
    let model = std::fs::read(path)
        .map_err(|e| format!("reading {rel}: {e}"))
        .and_then(|raw| gltf::Model::parse(&raw).map_err(|e| format!("{e:#}")));
    unit.iter()
        .map(|&i| {
            let bytes = match (&model, &jobs[i].source) {
                (Err(why), _) => Err(why.clone()),
                (Ok(model), JobSource::Embedded { index, .. }) => {
                    match gltf::image_source(model, rel, *index, read) {
                        Ok(gltf::ImageSource::Embedded(b)) => Ok(b),
                        Ok(gltf::ImageSource::External(_)) => {
                            Err(format!("{rel} changed during the run"))
                        }
                        Err(why) => Err(why),
                    }
                }
                (Ok(_), JobSource::File(_)) => Err("not an embedded image".to_string()),
            };
            (i, bytes)
        })
        .collect()
}

fn ktx2_path(stage: &Path, sha: &str) -> PathBuf {
    stage.join("ktx2").join(format!("{sha}.ktx2"))
}

static WRITE_SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// Written beside its final name and renamed into place, so a reader never
/// sees half a file.
fn write_atomic(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let tmp = path.with_file_name(format!(
        ".{}.tmp-{}-{}",
        path.file_name().unwrap_or_default().to_string_lossy(),
        std::process::id(),
        WRITE_SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ));
    std::fs::write(&tmp, bytes)?;
    std::fs::rename(&tmp, path).inspect_err(|_| {
        let _ = std::fs::remove_file(&tmp);
    })
}

/// [`write_atomic`], skipped when the file already holds these bytes.
fn write_if_changed(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    if std::fs::read(path).is_ok_and(|have| have == bytes) {
        return Ok(());
    }
    write_atomic(path, bytes)
}

/// [`write_atomic`] for a file named by the hash of its bytes: one that
/// exists already holds them, and is not read back.
fn write_content_addressed(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    if path.is_file() {
        return Ok(());
    }
    write_atomic(path, bytes)
}

fn is_sha_hex(s: &str) -> bool {
    s.len() == 64 && s.bytes().all(|b| b.is_ascii_hexdigit())
}

/// What a cache entry holds for a texture the preparation refuses for good
/// (too small to gain, undecodable): not retried until the source, the
/// encoder or the settings change.
const KEPT_PREFIX: &str = "kept: ";

fn cache_entry(stage: &Path, job: &Job, encoder: &dyn Encoder, max_size: u32) -> PathBuf {
    let key = encode::cache_key(&job.sha, encoder.id(), job.mode, max_size);
    stage.join("cache").join(key)
}

/// One texture from the cache, when this encoder already encoded these source
/// bytes this way, or already found it can't gain from it.
fn cached(stage: &Path, job: &Job, encoder: &dyn Encoder, max_size: u32) -> Option<Encoded> {
    let text = std::fs::read_to_string(cache_entry(stage, job, encoder, max_size)).ok()?;
    if let Some(why) = text.strip_prefix(KEPT_PREFIX) {
        return Some(Encoded::Failed(why.trim().to_string()));
    }
    let sha = text.trim();
    if !is_sha_hex(sha) {
        return None;
    }
    let meta = std::fs::metadata(ktx2_path(stage, sha)).ok()?;
    (meta.len() > 0).then(|| Encoded::Ok {
        ktx2_sha: sha.to_string(),
        len: meta.len(),
        cached: true,
    })
}

/// One texture not in the cache: prepared and encoded, its KTX2 stored by
/// content hash and the cache pointed at it. A texture the preparation
/// refuses (too small to gain, undecodable) is recorded as kept, so later runs
/// don't retry it; an encoder or storage failure is retried by the next run.
fn encode_one(
    stage: &Path,
    job: &Job,
    bytes: &[u8],
    encoder: &dyn Encoder,
    max_size: u32,
) -> Encoded {
    let entry = cache_entry(stage, job, encoder, max_size);
    let kept = |why: String| {
        let _ = write_if_changed(&entry, format!("{KEPT_PREFIX}{why}").as_bytes());
        Encoded::Failed(why)
    };
    let png = match encode::prepare(bytes, max_size) {
        Ok(png) => png,
        Err(why) => return kept(why),
    };
    let ktx2 = match encoder.encode(&png, job.mode) {
        Ok(ktx2) => ktx2,
        // not recorded: a failure here can pass (a full disk, memory pressure)
        Err(e) => return Encoded::Failed(format!("encoder failed: {e:#}")),
    };
    let sha = encode::sha256_hex(&ktx2);
    let stored = write_content_addressed(&ktx2_path(stage, &sha), &ktx2)
        .and_then(|()| write_if_changed(&entry, sha.as_bytes()));
    match stored {
        Ok(()) => Encoded::Ok {
            ktx2_sha: sha,
            len: ktx2.len() as u64,
            cached: false,
        },
        Err(e) => Encoded::Failed(format!("could not stage the KTX2 file: {e}")),
    }
}

/// Where a rewritten model is staged: named by its bytes, so a run never
/// rewrites a file the overlay in use names, and the swap to the next
/// overlay is the only moment the preview's bytes change.
fn staged_model_path(stage: &Path, model_rel: &str, bytes: &[u8]) -> PathBuf {
    let ext = match model_rel.to_ascii_lowercase().ends_with(".gltf") {
        true => "gltf",
        false => "glb",
    };
    stage
        .join("models")
        .join(format!("{}.{ext}", encode::sha256_hex(bytes)))
}

/// Model `rel` (bytes at `path`, as `key` saw them) rewritten with
/// `replacements` and staged. Kept per model size, time and replacements: an
/// unchanged model is not read, parsed or serialized again while its staged
/// copy exists. Returns the staged path, its size and whether the binary chunk
/// was repacked.
fn stage_model(
    stage: &Path,
    rel: &str,
    path: &Path,
    key: FileKey,
    replacements: &BTreeMap<usize, String>,
) -> Result<(PathBuf, u64, bool)> {
    type RewriteKey = (PathBuf, FileKey, BTreeMap<usize, String>);
    static REWRITES: OnceLock<Mutex<HashMap<RewriteKey, (PathBuf, u64, bool)>>> = OnceLock::new();
    let rewrites = REWRITES.get_or_init(Default::default);
    let memo_key = (path.to_path_buf(), key, replacements.clone());
    if let Some(hit) = rewrites
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .get(&memo_key)
        .filter(|(staged, _, _)| staged.is_file())
        .cloned()
    {
        return Ok(hit);
    }
    if file_key(path) != Some(key) {
        anyhow::bail!("{rel} changed during the run; the next run stages it");
    }
    let raw = std::fs::read(path).with_context(|| format!("reading {rel}"))?;
    let mut model = gltf::Model::parse(&raw).with_context(|| format!("parsing {rel}"))?;
    let repacked = gltf::apply(&mut model, replacements);
    let bytes = model.to_bytes();
    let staged = staged_model_path(stage, rel, &bytes);
    write_content_addressed(&staged, &bytes)
        .with_context(|| format!("staging {}", staged.display()))?;
    let hit = (staged, bytes.len() as u64, repacked);
    rewrites
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .insert(memo_key, hit.clone());
    Ok(hit)
}

/// Deletes the KTX2 files, cache entries and staged models outside `keep`
/// (what the run in place read or publishes), so the stage doesn't grow
/// without bound; called after a background swap and when a deploy's stage is
/// dropped, under the stage lock, and with the setting off.
fn sweep_stage(stage: &Path, keep: &BTreeSet<PathBuf>) {
    for dir in ["ktx2", "cache", "models"] {
        let Ok(rd) = std::fs::read_dir(stage.join(dir)) else {
            continue;
        };
        for entry in rd.flatten() {
            let path = entry.path();
            if path.is_dir() {
                // the layout before models were named by content
                let _ = std::fs::remove_dir_all(&path);
            } else if !keep.contains(&path) {
                let _ = std::fs::remove_file(&path);
            }
        }
    }
}

/// Whether `text` names the image file `rel`: its file name anywhere, or its
/// bare stem closed by a quote and opened by one (`'wall'`, `"wall"`,
/// `` `wall` ``) or by a folder (`'images/wall'`), the shape of code that
/// appends the extension itself. `text` is lowercase.
fn names_image(text: &str, rel: &str) -> bool {
    let name = rel.rsplit('/').next().unwrap_or(rel).to_lowercase();
    if text.contains(&name) {
        return true;
    }
    let stem = name
        .rsplit_once('.')
        .map_or(name.as_str(), |(stem, _)| stem);
    !stem.is_empty()
        && ['\'', '"', '`'].iter().any(|q| {
            text.contains(&format!("{q}{stem}{q}")) || text.contains(&format!("/{stem}{q}"))
        })
}

/// One full run over the published files `rels` of `root`: plan every
/// model's images, encode what is not cached (`run.workers` encoders),
/// stage the rewritten models and decide which originals drop out. Nothing
/// is applied here: the caller swaps the overlay in and writes the
/// manifest. `payload` is where a published file's bytes are read from —
/// models, images and the files searched for names alike — so the run sees
/// what is served or uploaded: the tree for the preview, the release copy of
/// a bundle for a deploy that built one.
pub(crate) fn optimize(
    root: &Path,
    rels: &[String],
    settings: Settings,
    encoder: &dyn Encoder,
    payload: &(dyn Fn(&str) -> PathBuf + Sync),
    run: &Run<'_>,
) -> Result<Outcome> {
    crate::scene::work_dir(root).context("creating .dcl-one")?;
    let stage = root.join(STAGE);
    let by_lower: HashMap<String, &String> = rels.iter().map(|r| (r.to_lowercase(), r)).collect();
    let published = |path: &str| by_lower.get(&path.to_lowercase()).map(|r| r.to_string());
    let read = |path: &str| -> Option<Vec<u8>> { std::fs::read(payload(&published(path)?)).ok() };

    let mut jobs: Vec<Job> = Vec::new();
    let mut job_of: HashMap<(String, ColorMode), usize> = HashMap::new();
    let mut models: Vec<ModelWork> = Vec::new();
    let mut kept: Vec<(String, String)> = Vec::new();
    // searched like scene code: models this run cannot read, and image URIs
    // it cannot resolve to a scene file (a Windows path a client may still read)
    let mut unparsed: Vec<(String, String)> = Vec::new();
    for rel in rels.iter().filter(|r| is_model(r)) {
        let path = payload(rel);
        let (scan, key) = match scan_model(&path, rel, &read) {
            Ok(found) => found,
            Err(why) => {
                kept.push((rel.clone(), why));
                continue;
            }
        };
        let (len, unresolved, scans) = match &*scan {
            ModelScan::Unparsed { why, text } => {
                kept.push((rel.clone(), why.clone()));
                unparsed.push((rel.clone(), text.clone()));
                continue;
            }
            ModelScan::Parsed {
                len,
                unresolved,
                images,
            } => (*len, unresolved, images),
        };
        if !unresolved.is_empty() {
            unparsed.push((rel.clone(), unresolved.join("\n").to_lowercase()));
        }
        let mut images = Vec::with_capacity(scans.len());
        for (index, image) in scans.iter().enumerate() {
            let external = match &image.source {
                Ok(ScanSource::External(path)) => published(path),
                _ => None,
            };
            let planned = match &image.plan {
                gltf::ImagePlan::Unused => Planned::Unused,
                gltf::ImagePlan::Keep(why) => Planned::Keep(why.clone()),
                gltf::ImagePlan::Encode { mode } => {
                    // the hash only: bytes are read for the textures the cache lacks
                    let found = match &image.source {
                        Ok(ScanSource::Embedded { sha, len }) => Ok((
                            sha.clone(),
                            *len,
                            JobSource::Embedded {
                                path: path.clone(),
                                rel: rel.clone(),
                                index,
                            },
                        )),
                        Ok(ScanSource::External(given)) => external
                            .as_deref()
                            .map(payload)
                            .and_then(|file| {
                                file_sha(&file).map(|(sha, len)| (sha, len, JobSource::File(file)))
                            })
                            .ok_or_else(|| format!("{given} is not a published file of the scene")),
                        Err(why) => Err(why.clone()),
                    };
                    match found {
                        Err(why) => Planned::Keep(why),
                        Ok((sha, source_len, source)) => {
                            let key = (sha.clone(), *mode);
                            let job = match job_of.get(&key) {
                                Some(&job) => job,
                                None => {
                                    jobs.push(Job {
                                        sha,
                                        mode: *mode,
                                        source,
                                    });
                                    job_of.insert(key, jobs.len() - 1);
                                    jobs.len() - 1
                                }
                            };
                            Planned::Encode {
                                job,
                                source_len,
                                mode: *mode,
                            }
                        }
                    }
                }
            };
            images.push(ImageWork {
                index,
                external,
                planned,
            });
        }
        models.push(ModelWork {
            rel: rel.clone(),
            path,
            key,
            original_len: len,
            images,
        });
    }

    let mut results: Vec<Option<Encoded>> = jobs
        .iter()
        .map(|job| cached(&stage, job, encoder, settings.max_size))
        .collect();
    let missing: Vec<usize> = (0..jobs.len()).filter(|&i| results[i].is_none()).collect();
    if !missing.is_empty() {
        let done = std::sync::atomic::AtomicUsize::new(jobs.len() - missing.len());
        let failed = std::sync::atomic::AtomicUsize::new(0);
        let report = |done: usize| {
            (run.progress)(Progress {
                done,
                total: jobs.len(),
                failed: failed.load(std::sync::atomic::Ordering::Relaxed),
            })
        };
        report(done.load(std::sync::atomic::Ordering::Relaxed));
        // sources are read in the workers, only while encoded: a file's bytes,
        // or one model and one of its embedded images at a time
        let encode_job = |i: usize, bytes: std::result::Result<&[u8], String>| {
            let result = match bytes.and_then(|b| check_planned(&jobs[i], b).map(|()| b)) {
                Ok(bytes) => encode_one(&stage, &jobs[i], bytes, encoder, settings.max_size),
                Err(why) => Encoded::Failed(why),
            };
            if matches!(result, Encoded::Failed(_)) {
                failed.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            }
            report(done.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1);
            (i, result)
        };
        // sources are read only for the textures the cache lacks: each file in
        // its worker, and one model at a time for its embedded images, which
        // the workers then share
        let units = work_units(&jobs, &missing);
        let (files, models): (Vec<_>, Vec<_>) = units
            .into_iter()
            .partition(|unit| matches!(jobs[unit[0]].source, JobSource::File(_)));
        let files: Vec<usize> = files.into_iter().map(|unit| unit[0]).collect();
        let mut encoded = encode::run_pool(&files, run.workers, |&i| {
            let JobSource::File(path) = &jobs[i].source else {
                unreachable!("partitioned on the source")
            };
            match std::fs::read(path) {
                Ok(bytes) => encode_job(i, Ok(&bytes)),
                Err(e) => encode_job(i, Err(format!("reading {}: {e}", path.display()))),
            }
        });
        for unit in models {
            let sources = embedded_sources(&jobs, &unit, &read);
            encoded.extend(encode::run_pool(&sources, run.workers, |(i, bytes)| {
                encode_job(*i, bytes.as_deref().map_err(Clone::clone))
            }));
        }
        for (i, result) in encoded {
            results[i] = Some(result);
        }
    }
    let results: Vec<Encoded> = results
        .into_iter()
        .map(|r| r.expect("every job is cached or encoded"))
        .collect();
    let mut stage_files: BTreeSet<PathBuf> = jobs
        .iter()
        .map(|job| cache_entry(&stage, job, encoder, settings.max_size))
        .collect();
    for result in &results {
        if let Encoded::Ok { ktx2_sha, .. } = result {
            stage_files.insert(ktx2_path(&stage, ktx2_sha));
        }
    }
    let mut model_inputs: HashMap<String, FileKey> = HashMap::new();

    let mut overlay = Overlay::default();
    let mut summary = Summary {
        total: jobs.len(),
        ..Summary::default()
    };
    for result in &results {
        match result {
            Encoded::Ok { cached: true, .. } => summary.cached += 1,
            Encoded::Ok { cached: false, .. } => summary.encoded += 1,
            Encoded::Failed(_) => summary.failed += 1,
        }
    }
    let mut converted_jobs: BTreeSet<usize> = BTreeSet::new();
    // per original image file: replaced somewhere, and kept anywhere
    let mut replaced_files: BTreeSet<String> = BTreeSet::new();
    let mut kept_files: BTreeSet<String> = BTreeSet::new();
    let mut textures: BTreeMap<String, Value> = BTreeMap::new();
    let mut counted_ktx2: BTreeSet<String> = BTreeSet::new();
    let mut model_entries: Vec<Value> = Vec::new();
    for work in models {
        let mut replacements: BTreeMap<usize, String> = BTreeMap::new();
        for image in &work.images {
            let label = format!("{} image {}", work.rel, image.index);
            let keep_file = |kept_files: &mut BTreeSet<String>| {
                if let Some(path) = &image.external {
                    kept_files.insert(path.clone());
                }
            };
            match &image.planned {
                Planned::Unused => keep_file(&mut kept_files),
                Planned::Keep(why) => {
                    keep_file(&mut kept_files);
                    kept.push((label, why.clone()));
                }
                Planned::Encode {
                    job,
                    source_len,
                    mode,
                } => match &results[*job] {
                    Encoded::Failed(why) => {
                        keep_file(&mut kept_files);
                        kept.push((label, why.clone()));
                    }
                    Encoded::Ok { len, .. } if len >= source_len => {
                        keep_file(&mut kept_files);
                        kept.push((
                            label,
                            format!(
                                "the KTX2 file is not smaller ({len} bytes, the source {source_len})"
                            ),
                        ));
                    }
                    Encoded::Ok {
                        ktx2_sha,
                        len,
                        cached,
                    } => {
                        if converted_jobs.insert(*job) {
                            summary.converted_bytes += source_len;
                        }
                        let ktx2_rel = published_ktx2(&work.rel, ktx2_sha);
                        let uri = gltf::relative_uri(&work.rel, &ktx2_rel);
                        replacements.insert(image.index, uri);
                        if let Some(path) = &image.external {
                            replaced_files.insert(path.clone());
                        }
                        overlay
                            .sources
                            .insert(ktx2_rel.clone(), ktx2_path(&stage, ktx2_sha));
                        overlay.added.insert(ktx2_rel.clone());
                        // one upload per distinct KTX2, however many folders publish it
                        if counted_ktx2.insert(ktx2_sha.clone()) {
                            summary.bytes_after += len;
                            summary.ktx2_bytes += len;
                        }
                        let entry = textures.entry(ktx2_rel).or_insert_with(|| {
                            json!({
                                "bytes": len,
                                "sourceBytes": source_len,
                                "colorMode": mode.as_str(),
                                "cached": cached,
                                "usedBy": [],
                            })
                        });
                        if let Some(used) = entry["usedBy"].as_array_mut() {
                            used.push(json!(label));
                        }
                    }
                },
            }
        }
        if replacements.is_empty() {
            continue;
        }
        let (staged, staged_len, repacked) =
            stage_model(&stage, &work.rel, &work.path, work.key, &replacements)?;
        model_inputs.insert(work.rel.clone(), work.key);
        summary.models += 1;
        summary.bytes_before += work.original_len;
        summary.bytes_after += staged_len;
        model_entries.push(json!({
            "path": work.rel,
            "staged": staged
                .strip_prefix(root)
                .unwrap_or(&staged)
                .to_string_lossy()
                .replace('\\', "/"),
            "sourceBytes": work.original_len,
            "optimizedBytes": staged_len,
            "texturesReplaced": replacements.len(),
            "binaryChunkRepacked": repacked,
        }));
        overlay.sources.insert(work.rel, staged);
    }

    // an original image drops out only if every model naming it was rewritten
    // without it, and no scene code, scene data or composite names it
    let candidates: Vec<String> = replaced_files.difference(&kept_files).cloned().collect();
    if settings.keep_source_textures {
        summary.kept_originals = candidates
            .into_iter()
            .map(|c| (c, "--keep-source-textures".to_string()))
            .collect();
    } else if !candidates.is_empty() {
        let mut texts: Vec<(String, String)> = rels
            .iter()
            .filter(|r| scanned(r))
            .filter_map(|r| {
                let bytes = std::fs::read(payload(r)).ok()?;
                Some((r.clone(), String::from_utf8_lossy(&bytes).to_lowercase()))
            })
            .collect();
        texts.extend(unparsed);
        for candidate in candidates {
            match texts.iter().find(|(_, text)| names_image(text, &candidate)) {
                Some((file, _)) => summary
                    .kept_originals
                    .push((candidate, format!("referenced by scene code ({file})"))),
                None => {
                    summary.bytes_before += std::fs::metadata(payload(&candidate))
                        .map(|m| m.len())
                        .unwrap_or(0);
                    overlay.removed.insert(candidate);
                }
            }
        }
    }
    summary.ignored = overlay.removed.iter().cloned().collect();
    summary.textures = overlay.added.len();
    summary.kept = kept;

    let texture_entries: Vec<Value> = textures
        .into_iter()
        .map(|(path, mut entry)| {
            let name = path.rsplit('/').next().unwrap_or(&path);
            entry["staged"] = json!(format!("{STAGE}/ktx2/{name}"));
            entry["path"] = json!(path);
            entry
        })
        .collect();
    let reasons = |list: &[(String, String)], key: &str| -> Vec<Value> {
        list.iter()
            .map(|(what, why)| json!({ key: what, "reason": why }))
            .collect()
    };
    let ignored: Vec<Value> = summary
        .ignored
        .iter()
        .map(|path| json!({ "path": path, "reason": IGNORED_REASON }))
        .collect();
    let manifest = json!({
        "keepSourceTextures": settings.keep_source_textures,
        "encoder": {
            "id": encoder.id(),
            "maxTextureSize": settings.max_size,
        },
        "models": model_entries,
        "textures": texture_entries,
        "keptImages": reasons(&summary.kept, "image"),
        "ignoredOriginals": ignored,
        "keptOriginals": reasons(&summary.kept_originals, "path"),
        "bytes": {"before": summary.bytes_before, "after": summary.bytes_after},
    });
    Ok(Outcome {
        overlay,
        summary,
        manifest,
        stage_files,
        model_inputs,
    })
}

#[cfg(test)]
mod tests;

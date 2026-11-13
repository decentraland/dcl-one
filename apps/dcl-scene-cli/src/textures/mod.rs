//! Optimized textures: the images GLB and glTF models sample become UASTC
//! KTX2 files (`KHR_texture_basisu`), staged under [`STAGE`] and laid over the
//! scene's file list. The creator's files are never written to: rewritten
//! models and new `.ktx2` files live in the staging directory, and an original
//! image only drops out of the published list ([`Overlay::removed`]).
//!
//! While the project's setting ([`settings`]) enables it, `start` imports in
//! the background ([`import`]), like a game engine's asset import: every run
//! fills the cache, and its finished overlay replaces the previous one in one
//! swap. `deploy` finishes whatever the cache lacks in the foreground
//! ([`stage_for_deploy`]).
//!
//! The overlay is applied where the payload is decided — `deploy`'s
//! `with_release_files`/`payload_path` and the preview's scene entity and
//! `/content/contents` — so the preview serves exactly the bytes a deploy
//! uploads. With no enabled overlay registered every function here hands its
//! input back unchanged.

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
        let mut out: Vec<String> = rels
            .into_iter()
            .filter(|r| !self.removed.contains(r))
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
type Stamp = (String, u64, Option<SystemTime>);

/// One project's last finished run.
#[derive(Clone, Debug, Default)]
struct Entry {
    /// The project setting the run read: whether the overlay is applied.
    enabled: bool,
    settings: Settings,
    overlay: Option<Arc<Overlay>>,
    /// The inputs the overlay was built from; `None` forces the next run.
    stamps: Option<Vec<Stamp>>,
}

/// Projects this process has imported, by canonical root.
fn registry() -> MutexGuard<'static, HashMap<PathBuf, Entry>> {
    static REGISTRY: OnceLock<Mutex<HashMap<PathBuf, Entry>>> = OnceLock::new();
    REGISTRY
        .get_or_init(Default::default)
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
}

/// One run at a time in this process, background or deploy: a deploy holds
/// it from its own run until its payload is read, so no background swap or
/// prune lands in between.
static RUN_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

/// The overlay the preview and payload apply: the last finished run's, when
/// the project enables it.
fn overlay(root: &Path) -> Option<Arc<Overlay>> {
    let reg = registry();
    let entry = reg.get(root)?;
    entry.overlay.clone().filter(|_| entry.enabled)
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

/// The published list with the overlay applied; `rels` itself when `root`
/// is not optimized.
pub fn publishable(root: &Path, rels: Vec<String>) -> Vec<String> {
    match overlay(root) {
        Some(o) => o.apply(rels),
        None => rels,
    }
}

/// The staged file holding `rel`'s published bytes, when the overlay has one.
pub fn staged_source(root: &Path, rel: &str) -> Option<PathBuf> {
    overlay(root)?.source(rel).map(Path::to_path_buf)
}

fn stamp(rel: &str, path: &Path) -> Stamp {
    let meta = std::fs::metadata(path).ok();
    (
        rel.to_string(),
        meta.as_ref().map_or(0, |m| m.len()),
        meta.and_then(|m| m.modified().ok()),
    )
}

/// The preview's inputs: the tree files it serves.
fn stamps_of(root: &Path, rels: &[String]) -> Vec<Stamp> {
    rels.iter().map(|rel| stamp(rel, &root.join(rel))).collect()
}

/// While held, the payload `deploy` reads carries its own run's overlay (or
/// none, with the setting off); dropping it puts back what the preview had
/// and asks the background import, if any, to catch up.
pub struct DeployStage {
    root: PathBuf,
    previous: Option<Entry>,
    _turn: tokio::sync::MutexGuard<'static, ()>,
}

impl Drop for DeployStage {
    fn drop(&mut self) {
        {
            let mut reg = registry();
            match self.previous.take() {
                Some(entry) => reg.insert(self.root.clone(), entry),
                None => reg.remove(&self.root),
            };
        }
        import::request(&self.root);
    }
}

/// `deploy`, after the build and before the payload is read: applies the
/// command line's `overrides` to the project setting and, when it is on,
/// runs over the release payload. Textures the cache has (the preview's
/// background import put them there) cost nothing; the rest are encoded here
/// with a progress line. A run that fails stops the deploy rather than
/// publishing a half-optimized scene; a texture that fails alone keeps its
/// original.
pub async fn stage_for_deploy(
    root: &Path,
    overrides: Overrides,
    quiet: bool,
) -> Result<DeployStage> {
    let project = settings::apply(root, overrides)?;
    let turn = match RUN_LOCK.try_lock() {
        Ok(turn) => turn,
        Err(_) => {
            if !quiet {
                ux::note("waiting for the texture import in progress");
            }
            RUN_LOCK.lock().await
        }
    };
    let previous = registry().get(root).cloned();
    let stage = DeployStage {
        root: root.to_path_buf(),
        previous,
        _turn: turn,
    };
    let settings = Settings::for_project(project);
    if !project.optimize_textures {
        swap(
            root,
            Entry {
                enabled: false,
                settings,
                overlay: None,
                stamps: None,
            },
        );
        return Ok(stage);
    }
    let owned = root.to_path_buf();
    let overlay = tokio::task::spawn_blocking(move || deploy_run(&owned, settings, quiet))
        .await
        .context("the texture optimizer stopped")??;
    swap(
        root,
        Entry {
            enabled: true,
            settings,
            overlay: Some(Arc::new(overlay)),
            stamps: None,
        },
    );
    Ok(stage)
}

/// One foreground run for `deploy`, on every core, over the files it uploads:
/// the tree's publishable files and the release build's own, each read where
/// the payload reads it.
fn deploy_run(root: &Path, settings: Settings, quiet: bool) -> Result<Overlay> {
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
    Ok(outcome.overlay)
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

struct Job {
    sha: String,
    mode: ColorMode,
    bytes: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq)]
enum Encoded {
    Ok {
        ktx2_sha: String,
        len: u64,
        cached: bool,
    },
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
    model: gltf::Model,
    original_len: u64,
    images: Vec<ImageWork>,
}

fn ktx2_path(stage: &Path, sha: &str) -> PathBuf {
    stage.join("ktx2").join(format!("{sha}.ktx2"))
}

/// Written beside its final name and renamed into place, so a reader never
/// sees half a file; skipped when the file already holds these bytes.
fn write_if_changed(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    if std::fs::read(path).is_ok_and(|have| have == bytes) {
        return Ok(());
    }
    static SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let tmp = path.with_file_name(format!(
        ".{}.tmp-{}-{}",
        path.file_name().unwrap_or_default().to_string_lossy(),
        std::process::id(),
        SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ));
    std::fs::write(&tmp, bytes)?;
    std::fs::rename(&tmp, path).inspect_err(|_| {
        let _ = std::fs::remove_file(&tmp);
    })
}

fn is_sha_hex(s: &str) -> bool {
    s.len() == 64 && s.bytes().all(|b| b.is_ascii_hexdigit())
}

fn cache_entry(stage: &Path, job: &Job, encoder: &dyn Encoder, max_size: u32) -> PathBuf {
    let key = encode::cache_key(&job.sha, encoder.id(), job.mode, max_size);
    stage.join("cache").join(key)
}

/// One texture from the cache, when this encoder already encoded these
/// source bytes this way.
fn cached(stage: &Path, job: &Job, encoder: &dyn Encoder, max_size: u32) -> Option<Encoded> {
    let text = std::fs::read_to_string(cache_entry(stage, job, encoder, max_size)).ok()?;
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
/// content hash and the cache pointed at it.
fn encode_one(stage: &Path, job: &Job, encoder: &dyn Encoder, max_size: u32) -> Encoded {
    let entry = cache_entry(stage, job, encoder, max_size);
    let png = match encode::prepare(&job.bytes, max_size) {
        Ok(png) => png,
        Err(why) => return Encoded::Failed(why),
    };
    let ktx2 = match encoder.encode(&png, job.mode) {
        Ok(ktx2) => ktx2,
        Err(e) => return Encoded::Failed(format!("encoder failed: {e:#}")),
    };
    let sha = encode::sha256_hex(&ktx2);
    let stored = write_if_changed(&ktx2_path(stage, &sha), &ktx2)
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

/// Deletes staged models the overlay now in place does not name; called
/// after the swap.
fn prune_models(stage: &Path, keep: &Overlay) {
    let keep: BTreeSet<&Path> = keep.sources.values().map(PathBuf::as_path).collect();
    let Ok(rd) = std::fs::read_dir(stage.join("models")) else {
        return;
    };
    for entry in rd.flatten() {
        let path = entry.path();
        if path.is_dir() {
            // the layout before models were named by content
            let _ = std::fs::remove_dir_all(&path);
        } else if !keep.contains(path.as_path()) {
            let _ = std::fs::remove_file(&path);
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
    payload: &dyn Fn(&str) -> PathBuf,
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
        let bytes = match std::fs::read(payload(rel)) {
            Ok(b) => b,
            Err(e) => {
                kept.push((rel.clone(), format!("model not readable: {e}")));
                continue;
            }
        };
        let model = match gltf::Model::parse(&bytes) {
            Ok(m) => m,
            Err(e) => {
                kept.push((rel.clone(), format!("model not optimized: {e:#}")));
                unparsed.push((rel.clone(), String::from_utf8_lossy(&bytes).to_lowercase()));
                continue;
            }
        };
        let rejected = gltf::unresolved_image_uris(&model, rel);
        if !rejected.is_empty() {
            unparsed.push((rel.clone(), rejected.join("\n").to_lowercase()));
        }
        let plans = gltf::plan_images(&model);
        let mut images = Vec::with_capacity(plans.len());
        for (index, plan) in plans.into_iter().enumerate() {
            let source = gltf::image_source(&model, rel, index, &read);
            let external = match &source {
                Ok(gltf::ImageSource::External(path)) => published(path),
                _ => None,
            };
            let planned = match plan {
                gltf::ImagePlan::Unused => Planned::Unused,
                gltf::ImagePlan::Keep(why) => Planned::Keep(why),
                gltf::ImagePlan::Encode { mode } => {
                    let bytes = match source {
                        Ok(gltf::ImageSource::Embedded(b)) => Ok(b),
                        Ok(gltf::ImageSource::External(path)) => external
                            .as_deref()
                            .and_then(read)
                            .ok_or_else(|| format!("{path} is not a published file of the scene")),
                        Err(why) => Err(why),
                    };
                    match bytes {
                        Err(why) => Planned::Keep(why),
                        Ok(bytes) => {
                            let sha = encode::sha256_hex(&bytes);
                            let source_len = bytes.len() as u64;
                            let key = (sha.clone(), mode);
                            let job = match job_of.get(&key) {
                                Some(&job) => job,
                                None => {
                                    jobs.push(Job { sha, mode, bytes });
                                    job_of.insert(key, jobs.len() - 1);
                                    jobs.len() - 1
                                }
                            };
                            Planned::Encode {
                                job,
                                source_len,
                                mode,
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
            model,
            original_len: bytes.len() as u64,
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
        let encoded = encode::run_pool(&missing, run.workers, |&i| {
            let result = encode_one(&stage, &jobs[i], encoder, settings.max_size);
            if matches!(result, Encoded::Failed(_)) {
                failed.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            }
            report(done.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1);
            result
        });
        for (i, result) in missing.into_iter().zip(encoded) {
            results[i] = Some(result);
        }
    }
    let results: Vec<Encoded> = results
        .into_iter()
        .map(|r| r.expect("every job is cached or encoded"))
        .collect();

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
        let mut model = work.model;
        let repacked = gltf::apply(&mut model, &replacements);
        let bytes = model.to_bytes();
        let staged = staged_model_path(&stage, &work.rel, &bytes);
        write_if_changed(&staged, &bytes)
            .with_context(|| format!("staging {}", staged.display()))?;
        summary.models += 1;
        summary.bytes_before += work.original_len;
        summary.bytes_after += bytes.len() as u64;
        model_entries.push(json!({
            "path": work.rel,
            "staged": staged
                .strip_prefix(root)
                .unwrap_or(&staged)
                .to_string_lossy()
                .replace('\\', "/"),
            "sourceBytes": work.original_len,
            "optimizedBytes": bytes.len(),
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
    })
}

#[cfg(test)]
mod tests;

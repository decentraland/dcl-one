//! `--optimize-textures` on `start` and `deploy`: the images GLB and glTF
//! models sample become UASTC KTX2 files (`KHR_texture_basisu`), staged under
//! [`STAGE`] and laid over the scene's file list. The creator's files are
//! never written to: rewritten models and new `.ktx2` files live in the
//! staging directory, and an original image only drops out of the published
//! list ([`Overlay::removed`]).
//!
//! The overlay is applied where the payload is decided — `deploy`'s
//! `with_release_files`/`payload_path` and the preview's scene entity and
//! `/content/contents` — so the preview serves exactly the bytes a deploy
//! uploads. Nothing is registered unless the flag is given, and with nothing
//! registered every function here hands its input back unchanged.

mod basisu;
mod encode;
mod gltf;

pub use encode::Encoder;

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
/// Where the KTX2 files are published, scene-relative, each named by the
/// sha256 of its bytes so models sharing a texture share one file.
pub const PUBLISHED_DIR: &str = "optimized-textures";
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Settings {
    /// `--keep-source-textures`: publish every original image, replaced or not.
    pub keep_source_textures: bool,
    pub max_size: u32,
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

struct Entry {
    settings: Settings,
    overlay: Option<Arc<Overlay>>,
    stamps: Option<Vec<Stamp>>,
}

/// Projects this process optimizes, by canonical root.
fn registry() -> MutexGuard<'static, HashMap<PathBuf, Entry>> {
    static REGISTRY: OnceLock<Mutex<HashMap<PathBuf, Entry>>> = OnceLock::new();
    REGISTRY
        .get_or_init(Default::default)
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
}

/// Opts `root` in for this process; [`refresh`] then builds its overlay.
pub fn enable(root: &Path, settings: Settings) {
    let mut reg = registry();
    let entry = reg.entry(root.to_path_buf()).or_insert(Entry {
        settings,
        overlay: None,
        stamps: None,
    });
    if entry.settings != settings {
        entry.settings = settings;
        entry.stamps = None;
    }
}

pub fn settings(root: &Path) -> Option<Settings> {
    registry().get(root).map(|e| e.settings)
}

fn overlay(root: &Path) -> Option<Arc<Overlay>> {
    registry().get(root)?.overlay.clone()
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

/// Rebuilds `root`'s overlay when its inputs moved: `Some` with what changed,
/// `None` when the project is not optimized or nothing moved since the last
/// run. An error leaves the previous overlay in place.
pub fn refresh(root: &Path) -> Result<Option<Summary>> {
    static ONE_AT_A_TIME: Mutex<()> = Mutex::new(());
    let _turn = ONE_AT_A_TIME
        .lock()
        .unwrap_or_else(PoisonError::into_inner);
    let Some(settings) = settings(root) else {
        return Ok(None);
    };
    let rels = crate::deploy::collect_publishable_files(root)?;
    let stamps: Vec<Stamp> = rels
        .iter()
        .map(|rel| stamp(rel, &crate::deploy::tree_payload_path(root, rel)))
        .collect();
    let unchanged = registry().get(root).is_some_and(|e| {
        e.stamps.as_ref() == Some(&stamps)
            && e.overlay
                .as_ref()
                .is_some_and(|o| o.sources.values().all(|staged| staged.is_file()))
    });
    if unchanged {
        return Ok(None);
    }
    let encoder = basisu::WasmBasisu::shared()?;
    let outcome = optimize(root, &rels, settings, encoder, &|rel| {
        crate::deploy::tree_payload_path(root, rel)
    })?;
    if let Some(entry) = registry().get_mut(root) {
        entry.overlay = Some(Arc::new(outcome.overlay));
        entry.stamps = Some(stamps);
    }
    Ok(Some(outcome.summary))
}

/// [`refresh`] with a progress line and the summary printed (a one-line
/// summary when `brief`); `quiet` prints nothing.
pub fn refresh_and_report(root: &Path, quiet: bool, brief: bool) -> Result<()> {
    let progress = (!quiet).then(|| ux::Slow::start("optimizing textures"));
    let result = refresh(root);
    drop(progress);
    if let (Ok(Some(summary)), false) = (&result, quiet) {
        summary.report(brief);
    }
    result.map(|_| ())
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
    pub mixed: Vec<String>,
}

impl Summary {
    pub fn report(&self, brief: bool) {
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
        if !self.ignored.is_empty() {
            ux::note(format!(
                "{} original image(s) left out of the upload: only models used them (--keep-source-textures keeps them)",
                self.ignored.len()
            ));
        }
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
        if !self.mixed.is_empty() {
            ux::note(format!(
                "{} image(s) are sampled both as colour and as data and were encoded as sRGB: {}",
                self.mixed.len(),
                self.mixed.join(", ")
            ));
        }
        ux::note_absent(KTX2_WARNING);
        ux::note(format!("details: {STAGE}/manifest.json"));
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
        mixed: bool,
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

/// One texture: from the cache when this encoder already encoded these
/// source bytes this way, else prepared and encoded, its KTX2 stored by
/// content hash and the cache pointed at it.
fn encode_one(stage: &Path, job: &Job, encoder: &dyn Encoder, max_size: u32) -> Encoded {
    let key = encode::cache_key(&job.sha, encoder.id(), job.mode, max_size);
    let entry = stage.join("cache").join(&key);
    if let Ok(text) = std::fs::read_to_string(&entry) {
        let sha = text.trim();
        if is_sha_hex(sha)
            && let Ok(meta) = std::fs::metadata(ktx2_path(stage, sha))
            && meta.len() > 0
        {
            return Encoded::Ok {
                ktx2_sha: sha.to_string(),
                len: meta.len(),
                cached: true,
            };
        }
    }
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

/// Deletes staged models no longer in the overlay.
fn prune_models(dir: &Path, base: &Path, keep: &BTreeSet<String>) {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in rd.flatten() {
        let path = entry.path();
        if path.is_dir() {
            prune_models(&path, base, keep);
            let _ = std::fs::remove_dir(&path);
        } else if let Ok(rel) = path.strip_prefix(base) {
            let rel = rel.to_string_lossy().replace('\\', "/");
            if !keep.contains(&rel) {
                let _ = std::fs::remove_file(&path);
            }
        }
    }
}

/// One full run over the publishable files `rels` of `root`: plan every
/// model's images, encode what is not cached (one encoder per core), stage
/// the rewritten models, decide which originals drop out, and write the
/// manifest. `payload` is where a published file's bytes are read from
/// (the release copy of a bundle, when a deploy built one).
pub(crate) fn optimize(
    root: &Path,
    rels: &[String],
    settings: Settings,
    encoder: &dyn Encoder,
    payload: &dyn Fn(&str) -> PathBuf,
) -> Result<Outcome> {
    crate::scene::work_dir(root).context("creating .dcl-one")?;
    let stage = root.join(STAGE);
    let by_lower: HashMap<String, &String> = rels.iter().map(|r| (r.to_lowercase(), r)).collect();
    let published = |path: &str| by_lower.get(&path.to_lowercase()).map(|r| r.to_string());
    let read = |path: &str| -> Option<Vec<u8>> {
        std::fs::read(root.join(published(path)?)).ok()
    };

    let mut jobs: Vec<Job> = Vec::new();
    let mut job_of: HashMap<(String, ColorMode), usize> = HashMap::new();
    let mut models: Vec<ModelWork> = Vec::new();
    let mut kept: Vec<(String, String)> = Vec::new();
    // models this run cannot read are searched like scene code
    let mut unparsed: Vec<(String, String)> = Vec::new();
    for rel in rels.iter().filter(|r| is_model(r)) {
        let bytes = match std::fs::read(root.join(rel)) {
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
                gltf::ImagePlan::Encode { mode, mixed } => {
                    let bytes = match source {
                        Ok(gltf::ImageSource::Embedded(b)) => Ok(b),
                        Ok(gltf::ImageSource::External(path)) => external
                            .as_deref()
                            .and_then(&read)
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
                                mixed,
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

    let results = encode::run_pool(&jobs, encode::workers(), |job| {
        encode_one(&stage, job, encoder, settings.max_size)
    });

    let mut overlay = Overlay::default();
    let mut summary = Summary::default();
    for result in &results {
        match result {
            Encoded::Ok { cached: true, .. } => summary.cached += 1,
            Encoded::Ok { cached: false, .. } => summary.encoded += 1,
            Encoded::Failed(_) => {}
        }
    }
    // per original image file: replaced somewhere, and kept anywhere
    let mut replaced_files: BTreeSet<String> = BTreeSet::new();
    let mut kept_files: BTreeSet<String> = BTreeSet::new();
    let mut textures: BTreeMap<String, Value> = BTreeMap::new();
    let mut model_entries: Vec<Value> = Vec::new();
    let mut staged_models: BTreeSet<String> = BTreeSet::new();
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
                    mixed,
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
                        let ktx2_rel = format!("{PUBLISHED_DIR}/{ktx2_sha}.ktx2");
                        let uri = gltf::relative_uri(&work.rel, &ktx2_rel);
                        replacements.insert(image.index, uri);
                        if *mixed {
                            summary.mixed.push(label.clone());
                        }
                        if let Some(path) = &image.external {
                            replaced_files.insert(path.clone());
                        }
                        overlay
                            .sources
                            .insert(ktx2_rel.clone(), ktx2_path(&stage, ktx2_sha));
                        if overlay.added.insert(ktx2_rel.clone()) {
                            summary.bytes_after += len;
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
        let staged = stage.join("models").join(&work.rel);
        write_if_changed(&staged, &bytes)
            .with_context(|| format!("staging {}", staged.display()))?;
        summary.models += 1;
        summary.bytes_before += work.original_len;
        summary.bytes_after += bytes.len() as u64;
        model_entries.push(json!({
            "path": work.rel,
            "staged": format!("{STAGE}/models/{}", work.rel),
            "sourceBytes": work.original_len,
            "optimizedBytes": bytes.len(),
            "texturesReplaced": replacements.len(),
            "binaryChunkRepacked": repacked,
        }));
        staged_models.insert(work.rel.clone());
        overlay.sources.insert(work.rel, staged);
    }
    prune_models(&stage.join("models"), &stage.join("models"), &staged_models);

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
            let name = candidate
                .rsplit('/')
                .next()
                .unwrap_or(&candidate)
                .to_lowercase();
            match texts.iter().find(|(_, text)| text.contains(&name)) {
                Some((file, _)) => summary
                    .kept_originals
                    .push((candidate, format!("referenced by scene code ({file})"))),
                None => {
                    summary.bytes_before += std::fs::metadata(root.join(&candidate))
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
        "encoder": {
            "id": encoder.id(),
            "maxTextureSize": settings.max_size,
        },
        "models": model_entries,
        "textures": texture_entries,
        "keptImages": reasons(&summary.kept, "image"),
        "ignoredOriginals": ignored,
        "keptOriginals": reasons(&summary.kept_originals, "path"),
        "mixedColorUse": summary.mixed,
        "bytes": {"before": summary.bytes_before, "after": summary.bytes_after},
    });
    let mut text = serde_json::to_vec_pretty(&manifest).expect("JSON values serialize");
    text.push(b'\n');
    write_if_changed(&stage.join("manifest.json"), &text).context("writing the manifest")?;
    Ok(Outcome {
        overlay,
        summary,
        manifest,
    })
}

#[cfg(test)]
mod tests;

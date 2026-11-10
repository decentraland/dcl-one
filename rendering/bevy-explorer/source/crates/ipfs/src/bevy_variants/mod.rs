//! Prototype: GPU-ready "Bevy variants" of the textures and models of scenes, wearables and
//! emotes, published by abgen.
//!
//! Off unless a variants base url is configured (`--bevy-variants-url`, `?bevyVariants=`, or the
//! `bevy_variants_url` config field). For each scene, wearable and emote entity it loads, the
//! explorer fetches
//! `{base}/bevy/manifest/{entityId}.json` once; it maps original content CIDs to variants, each
//! with the path it is served at and the raw-leaf CID of its bytes. A content file read whose
//! original CID has a variant in a format this client takes is answered with
//! `{base}/{variantPath}` instead, checked against the variant CID and cached under that CID
//! only. A texture variant is a BC KTX2, handed
//! to the loaders as the DDS it unwraps to (`ktx2`); that DDS is cached too, under a name derived
//! from the variant CID. Anything else, at any step, reads the original. A variant that fails to
//! decode is marked bad for the session and its asset reloaded from the original.
//!
//! The formats taken are the [`SUPPORTED_FORMATS`] table: a manifest variant is loaded when its
//! `kind`/`format` pair has a row whose file types, GPU needs and decoder this client has. Of the
//! model variants, `glb-meshopt` (meshes under `EXT_meshopt_compression` and
//! `KHR_mesh_quantization`, textures untouched) is taken behind the `meshopt-variants` feature,
//! for the binary whose glTF loader decodes meshopt streams: its bytes are handed to that loader
//! as they are, at the original `.glb`/`.gltf` path (the asset server picks the loader by that
//! path's extension, not by the cache file's name). `glb` embeds its textures as KTX2 under
//! `KHR_texture_basisu`, which the glTF loader rejects when a file requires it, and is not taken.
//!
//! Preview mode, local scenes (`b64-` ids) and entities served from a loopback or file content
//! server never ask for a manifest; in preview mode that includes the wearables and emotes, so a
//! preview always shows the deployed files.

mod debug_panel;
pub mod ktx2;
#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests;

use std::{
    collections::{HashMap, HashSet, VecDeque},
    io::ErrorKind,
    path::{Path, PathBuf},
    sync::{atomic::Ordering, Arc, Mutex, OnceLock},
};

use bevy::{
    asset::{io::AssetReaderError, AssetLoadFailedEvent},
    image::CompressedImageFormats,
    prelude::*,
    render::renderer::RenderCapabilities,
    tasks::IoTaskPool,
};
use common::util::{TaskCompat, UrlLoopbackExt};
use serde::Deserialize;
use tokio::sync::watch;
use web_time::Duration;

use crate::{
    content_hash,
    ipfs_path::{CacheKey, IpfsPath},
    IpfsIo, IpfsResource, SharedBytes,
};

/// Image paths whose loader guesses the format from the bytes (or, for `.ktx2`, passes a dds
/// through), so a dds decodes at the original path. A path without an extension goes to the
/// last image loader, which guesses too.
const TEXTURE_EXTENSIONS: [&str; 5] = ["png", "jpg", "jpeg", "webp", "ktx2"];
/// Model paths the glTF loader reads (and the meshopt pre-pass loader stands in for).
#[cfg(feature = "meshopt-variants")]
const MODEL_EXTENSIONS: [&str; 2] = ["glb", "gltf"];
/// Every `format` the manifest schema (v1) may name, taken or not.
const SCHEMA_FORMATS: [&str; 5] = ["ktx2-bc7", "ktx2-bc1", "ktx2-bc5", "glb", "glb-meshopt"];
const RECENT_FALLBACKS: usize = 20;

/// What a supported variant's bytes are handed to the loaders as.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Decoded {
    /// A BC KTX2 texture, unwrapped to the DDS the image loaders read.
    BcTexture(ktx2::BcFormat),
    /// Handed to the loader of the original path as it is: a `glb-meshopt` model, whose meshopt
    /// streams the glTF loader's pre-pass decodes.
    #[cfg(feature = "meshopt-variants")]
    AsIs,
}

/// A manifest format this client takes: the schema's `kind`/`format` pair, the content files it
/// may replace, what the GPU must support and what the bytes decode as.
#[derive(Debug)]
pub struct SupportedFormat {
    pub kind: &'static str,
    pub format: &'static str,
    /// Lower-case extensions of the files the variant may stand in for.
    pub extensions: &'static [&'static str],
    /// Whether a file without an extension may be one of them (an image: its loader guesses the
    /// format from the bytes).
    pub without_extension: bool,
    /// The GPU must decode BC textures.
    pub needs_bc: bool,
    pub decoded: Decoded,
}

impl SupportedFormat {
    fn accepts_path(&self, file_path: &str) -> bool {
        match extension(file_path) {
            Some(ext) => self.extensions.contains(&ext.as_str()),
            None => self.without_extension,
        }
    }
}

const fn bc_texture(format: &'static str, bc: ktx2::BcFormat) -> SupportedFormat {
    SupportedFormat {
        kind: "texture",
        format,
        extensions: &TEXTURE_EXTENSIONS,
        without_extension: true,
        needs_bc: true,
        decoded: Decoded::BcTexture(bc),
    }
}

/// The manifest formats this client takes. A variant is loaded only through a row here; the
/// manifest's order of preference decides between rows.
pub const SUPPORTED_FORMATS: &[SupportedFormat] = &[
    bc_texture("ktx2-bc7", ktx2::BcFormat::Bc7),
    bc_texture("ktx2-bc1", ktx2::BcFormat::Bc1),
    bc_texture("ktx2-bc5", ktx2::BcFormat::Bc5),
    // the bytes pass through: the meshopt pre-pass glTF loader decodes the streams
    #[cfg(feature = "meshopt-variants")]
    SupportedFormat {
        kind: "model",
        format: "glb-meshopt",
        extensions: &MODEL_EXTENSIONS,
        without_extension: false,
        needs_bc: false,
        decoded: Decoded::AsIs,
    },
];

/// The row for a manifest variant's `kind`/`format`, if this client takes it.
pub fn supported_format(variant: &Variant) -> Option<&'static SupportedFormat> {
    SUPPORTED_FORMATS
        .iter()
        .find(|row| row.kind == variant.kind && row.format == variant.format)
}

/// The formats this client takes and the schema's it does not, in the debug panel's words.
pub fn taken_formats() -> String {
    let taken: Vec<_> = SUPPORTED_FORMATS.iter().map(|row| row.format).collect();
    let not_taken: Vec<_> = SCHEMA_FORMATS
        .iter()
        .copied()
        .filter(|format| !taken.contains(format))
        .collect();
    // the formats grouped by what they are handed to the loaders as, in table order
    let mut groups: Vec<(Vec<&str>, &str)> = Vec::new();
    for row in SUPPORTED_FORMATS {
        let how = match row.decoded {
            Decoded::BcTexture(_) => "loaded as DDS",
            #[cfg(feature = "meshopt-variants")]
            Decoded::AsIs => "meshopt streams decoded by the glTF loader",
        };
        match groups.iter_mut().find(|(_, group_how)| *group_how == how) {
            Some((formats, _)) => formats.push(row.format),
            None => groups.push((vec![row.format], how)),
        }
    }
    let mut words = groups
        .iter()
        .map(|(formats, how)| format!("{} ({how})", formats.join(", ")))
        .collect::<Vec<_>>()
        .join(", ");
    if !not_taken.is_empty() {
        words.push_str("; not ");
        words.push_str(&not_taken.join(", "));
    }
    words
}
/// Consecutive transport failures (manifest or variant) after which the service is treated as
/// down for the rest of the session, so a dead host costs a few requests, not one per file.
const BREAKER_FAILURES: u32 = 5;

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Variant {
    /// Where the variant is served, relative to the base: `bevy/{prefix}/{name}`. Missing
    /// reads as empty, which drops that variant, not the manifest.
    #[serde(default)]
    pub path: String,
    /// Raw-leaf CID of the variant's bytes: what they are checked against and cached under.
    pub cid: String,
    pub kind: String,
    pub format: String,
    #[serde(default)]
    pub width: Option<u32>,
    #[serde(default)]
    pub height: Option<u32>,
    #[serde(default)]
    pub mips: Option<u32>,
    #[serde(default)]
    pub size: Option<u64>,
}

#[derive(Debug)]
pub struct Manifest {
    pub generator: Option<String>,
    /// Variants by original content CID, in the manifest's order of preference.
    pub files: HashMap<String, Vec<Variant>>,
}

#[derive(Deserialize)]
struct ManifestVersion {
    v: u32,
}

#[derive(Deserialize)]
struct ManifestJson {
    entity: String,
    #[serde(default)]
    generator: Option<String>,
    #[serde(default)]
    files: HashMap<String, Vec<Variant>>,
}

/// The manifest schema version this client reads.
pub const MANIFEST_VERSION: u32 = 2;

/// A v2 manifest for `entity`. Variants whose cid is not a raw-leaf CIDv1 (`bafkrei…`) are
/// dropped (the cid becomes a cache file name, and only that scheme is checked against the
/// bytes), and so are variants whose path is not `bevy/{prefix}/{name}` with plain segments
/// (it becomes part of a url).
pub fn parse_manifest(bytes: &[u8], entity: &str) -> Result<Manifest, String> {
    let version: ManifestVersion = serde_json::from_slice(bytes).map_err(|e| e.to_string())?;
    if version.v != MANIFEST_VERSION {
        return Err(format!("unsupported manifest version {}", version.v));
    }
    let json: ManifestJson = serde_json::from_slice(bytes).map_err(|e| e.to_string())?;
    if json.entity != entity {
        return Err(format!("manifest names entity `{}`", json.entity));
    }
    let files = json
        .files
        .into_iter()
        .map(|(original, variants)| {
            let variants: Vec<_> = variants
                .into_iter()
                .filter(|variant| is_raw_cid(&variant.cid) && is_variant_path(&variant.path))
                .collect();
            (original, variants)
        })
        .filter(|(_, variants)| !variants.is_empty())
        .collect();
    Ok(Manifest {
        generator: json.generator,
        files,
    })
}

/// CIDv1, raw codec, sha2-256, base32: `b` + 58 base32 characters.
fn is_raw_cid(cid: &str) -> bool {
    cid.len() == 59
        && cid.starts_with("bafkrei")
        && cid
            .bytes()
            .all(|b| b.is_ascii_lowercase() || (b'2'..=b'7').contains(&b))
}

/// `bevy/{prefix}/{name}`: the prefix ASCII alphanumeric and not `manifest`, the name ASCII
/// alphanumeric or `_`. No dots, slashes or escapes can reach the url.
fn is_variant_path(path: &str) -> bool {
    let mut segments = path.split('/');
    let (Some("bevy"), Some(prefix), Some(name), None) = (
        segments.next(),
        segments.next(),
        segments.next(),
        segments.next(),
    ) else {
        return false;
    };
    path.len() <= 256
        && prefix != "manifest"
        && !prefix.is_empty()
        && prefix.bytes().all(|b| b.is_ascii_alphanumeric())
        && !name.is_empty()
        && name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
}

fn extension(file_path: &str) -> Option<String> {
    Path::new(file_path)
        .extension()
        .and_then(|ext| ext.to_str())
        .map(str::to_ascii_lowercase)
}

/// Whether a file of this type could be replaced by any supported format; others never wait on
/// a manifest.
pub fn could_have_variant(file_path: &str) -> bool {
    SUPPORTED_FORMATS
        .iter()
        .any(|row| row.accepts_path(file_path))
}

/// Whether this client can load `variant` at `file_path`: a supported format, for a file type
/// it may replace, on a GPU that has what it needs (`bc`: BC texture support).
pub fn supports(variant: &Variant, file_path: &str, bc: bool) -> bool {
    supported_format(variant)
        .is_some_and(|row| (bc || !row.needs_bc) && row.accepts_path(file_path))
}

/// The first variant this client supports that has not failed this session.
pub fn select_variant<'a>(
    variants: &'a [Variant],
    file_path: &str,
    bc: bool,
    is_bad: impl Fn(&str) -> bool,
) -> Option<&'a Variant> {
    variants
        .iter()
        .find(|variant| supports(variant, file_path, bc) && !is_bad(&variant.cid))
}

/// Why an entity (scene, wearable or emote) gets no manifest request, if it doesn't.
pub fn scene_eligibility(
    entity: &str,
    contents_base_url: Option<&str>,
    preview: bool,
) -> Result<(), &'static str> {
    if preview {
        return Err("preview mode");
    }
    if entity.starts_with("b64-") {
        return Err("local scene (b64- id)");
    }
    if entity.is_empty() || !entity.bytes().all(|b| b.is_ascii_alphanumeric()) {
        return Err("entity id is not a content id");
    }
    let Some(url) = contents_base_url else {
        return Err("no content server");
    };
    let Ok(url) = url::Url::parse(url) else {
        return Err("unparseable content server url");
    };
    if !matches!(url.scheme(), "http" | "https") {
        return Err("not an http content server");
    }
    if url.is_loopback() {
        return Err("loopback content server");
    }
    Ok(())
}

#[derive(Clone, Debug)]
pub enum ManifestState {
    Pending,
    Ineligible(&'static str),
    /// 404: abgen has no manifest for the entity (yet).
    None,
    Loaded(Arc<Manifest>),
    Failed(String),
}

#[derive(Clone, Debug)]
struct ServedVariant {
    scene: String,
    file: String,
    cid: String,
    format: String,
}

#[derive(Debug)]
struct Fallback {
    scene: String,
    file: String,
    reason: String,
}

#[derive(Default)]
struct Session {
    disabled: Option<String>,
    transport_failures: u32,
    bad: HashSet<String>,
    served: HashMap<PathBuf, ServedVariant>,
    downloaded: u64,
    cached: u64,
    fallbacks: u64,
    recent: VecDeque<Fallback>,
}

impl Session {
    fn fallback(&mut self, scene: &str, file: &str, reason: String) {
        debug!("bevy variant fallback for `{file}` in {scene}: {reason}");
        self.fallbacks += 1;
        if self.recent.len() == RECENT_FALLBACKS {
            self.recent.pop_front();
        }
        self.recent.push_back(Fallback {
            scene: scene.to_owned(),
            file: file.to_owned(),
            reason,
        });
    }
}

#[derive(Default)]
pub struct BevyVariants {
    base: OnceLock<String>,
    /// The GPU's BC support, once the renderer has reported it.
    bc: OnceLock<bool>,
    manifests: Mutex<HashMap<String, watch::Receiver<ManifestState>>>,
    session: Mutex<Session>,
}

impl BevyVariants {
    fn base(&self) -> Option<&str> {
        self.base.get().map(String::as_str)
    }

    fn bc(&self) -> bool {
        self.bc.get().copied().unwrap_or(false)
    }

    fn set_base(&self, url: &str) {
        let url = url.trim().trim_end_matches('/');
        match url::Url::parse(url) {
            Ok(parsed) if matches!(parsed.scheme(), "http" | "https") => {
                if self.base.set(url.to_owned()).is_ok() {
                    info!("bevy variants: on, from `{url}`");
                }
            }
            _ => warn!("bevy variants: ignoring `{url}`, not an http(s) url"),
        }
    }

    /// A new pending manifest for `entity`, or None when one was already requested.
    fn begin(&self, entity: &str) -> Option<watch::Sender<ManifestState>> {
        let mut manifests = self.manifests.lock().unwrap();
        if manifests.contains_key(entity) {
            return None;
        }
        let (sender, receiver) = watch::channel(ManifestState::Pending);
        manifests.insert(entity.to_owned(), receiver);
        Some(sender)
    }

    fn manifest(&self, entity: &str) -> Option<watch::Receiver<ManifestState>> {
        self.manifests.lock().unwrap().get(entity).cloned()
    }

    fn disabled(&self) -> Option<String> {
        self.session.lock().unwrap().disabled.clone()
    }

    fn transport_ok(&self) {
        self.session.lock().unwrap().transport_failures = 0;
    }

    fn transport_failed(&self, reason: &str) {
        let mut session = self.session.lock().unwrap();
        session.transport_failures += 1;
        if session.transport_failures >= BREAKER_FAILURES && session.disabled.is_none() {
            warn!("bevy variants: off for this session, {BREAKER_FAILURES} failed requests ({reason})");
            session.disabled = Some(format!(
                "{BREAKER_FAILURES} consecutive failed requests, last: {reason}"
            ));
        }
    }

    fn is_bad(&self, cid: &str) -> bool {
        self.session.lock().unwrap().bad.contains(cid)
    }

    fn fail(&self, scene: &str, file: &str, cid: Option<&str>, reason: String) {
        let mut session = self.session.lock().unwrap();
        if let Some(cid) = cid {
            session.bad.insert(cid.to_owned());
        }
        session.fallback(scene, file, reason);
    }

    fn served(&self, path: &Path, served: ServedVariant, cached: bool) {
        let mut session = self.session.lock().unwrap();
        if cached {
            session.cached += 1;
        } else {
            session.downloaded += 1;
        }
        session.served.insert(path.to_owned(), served);
    }

    /// Whether the asset at `path` was last read from a variant.
    pub fn is_served(&self, path: &Path) -> bool {
        self.session.lock().unwrap().served.contains_key(path)
    }

    /// Marks the variant served for `path` bad, if there was one; true when the asset should be
    /// reloaded (from its original).
    fn decode_failed(&self, path: &Path, error: &str) -> bool {
        let mut session = self.session.lock().unwrap();
        let Some(served) = session.served.remove(path) else {
            return false;
        };
        session.bad.insert(served.cid.clone());
        session.fallback(
            &served.scene,
            &served.file,
            format!("{} variant failed to load: {error}", served.format),
        );
        true
    }
}

/// What the loader reads for a variant (the DDS a BC texture unwraps to, a model's own bytes),
/// and whether it came from the cache.
pub(crate) struct VariantRead {
    pub(crate) bytes: Vec<u8>,
    pub(crate) cached: bool,
}

fn describe_fetch_error(error: &platform::FetchError<anyhow::Error>) -> String {
    match error {
        platform::FetchError::Headers => "timed out awaiting headers".to_owned(),
        platform::FetchError::Stalled => "stalled".to_owned(),
        platform::FetchError::Send(e) => format!("request failed: {e}"),
        platform::FetchError::Status(status, _) => format!("status {status}"),
        platform::FetchError::Body(e) => format!("body stream error: {e}"),
    }
}

impl IpfsIo {
    /// Turns the variants client on for the session. Startup only: a second call is ignored.
    pub fn set_bevy_variants_base(&self, url: &str) {
        self.bevy_variants.set_base(url);
    }

    /// Whether the asset at `path` was last read from a variant (and so must not be
    /// reprocessed as if it were the original).
    pub fn served_bevy_variant(&self, path: &Path) -> bool {
        self.bevy_variants.is_served(path)
    }

    /// Asks abgen for an entity's manifest (a scene, wearable or emote), once per session. A
    /// no-op when variants are off.
    pub fn request_bevy_variants(self: &Arc<Self>, entity: &str) {
        let Some(base) = self.bevy_variants.base().map(str::to_owned) else {
            return;
        };
        let Some(sender) = self.bevy_variants.begin(entity) else {
            return;
        };
        let io = self.clone();
        let entity = entity.to_owned();
        IoTaskPool::get()
            .spawn_compat(async move {
                let state = io.fetch_bevy_variants_manifest(&base, &entity).await;
                sender.send_replace(state);
            })
            .detach();
    }

    async fn fetch_bevy_variants_manifest(&self, base: &str, entity: &str) -> ManifestState {
        let contents = self.context.read().await.contents_base_url(entity);
        if let Err(reason) = scene_eligibility(entity, contents.as_deref(), self.is_preview) {
            return ManifestState::Ineligible(reason);
        }
        if let Some(reason) = self.bevy_variants.disabled() {
            return ManifestState::Failed(format!("variants off: {reason}"));
        }
        let url = format!("{base}/bevy/manifest/{entity}.json");
        let request = match self.client.get(&url).build() {
            Ok(request) => request,
            Err(e) => return ManifestState::Failed(e.to_string()),
        };
        match self
            .fetch_request(
                request,
                self.client.clone(),
                Duration::from_secs(15),
                Duration::from_secs(10),
                Some(entity.to_owned()),
            )
            .await
        {
            Ok(response) => {
                self.bevy_variants.transport_ok();
                match parse_manifest(&response.body, entity) {
                    Ok(manifest) => ManifestState::Loaded(Arc::new(manifest)),
                    Err(e) => ManifestState::Failed(format!("invalid manifest: {e}")),
                }
            }
            Err(platform::FetchError::Status(status, _))
                if status == reqwest::StatusCode::NOT_FOUND =>
            {
                self.bevy_variants.transport_ok();
                ManifestState::None
            }
            Err(e) => {
                let reason = describe_fetch_error(&e);
                self.bevy_variants.transport_failed(&reason);
                ManifestState::Failed(reason)
            }
        }
    }

    /// The variant's bytes for an entity's content file, or None to read the original.
    pub(crate) async fn read_bevy_variant(
        self: &Arc<Self>,
        ipfs_path: &IpfsPath,
        path: &Path,
    ) -> Option<VariantRead> {
        let variants = &self.bevy_variants;
        let base = variants.base()?;
        let (scene, file) = (ipfs_path.scene_hash()?, ipfs_path.content_path()?);
        if !could_have_variant(file) {
            return None;
        }
        let mut manifest = variants.manifest(scene)?;
        let state = manifest
            .wait_for(|state| !matches!(state, ManifestState::Pending))
            .await
            .ok()?
            .clone();
        let ManifestState::Loaded(manifest) = state else {
            return None;
        };
        let original = self
            .context
            .read()
            .await
            .entities
            .get(scene)?
            .collection
            .hash(file)?
            .into_owned();
        let options = manifest.files.get(&original)?;
        if variants.disabled().is_some() {
            return None;
        }
        let bc = variants.bc();
        if !options.iter().any(|variant| supports(variant, file, bc)) {
            let formats: Vec<_> = options.iter().map(|v| v.format.as_str()).collect();
            // a format this file type would take on a GPU with BC textures
            let wants_bc = !bc && options.iter().any(|variant| supports(variant, file, true));
            let reason = if wants_bc {
                "the GPU has no BC texture support (or has not reported it yet)".to_owned()
            } else {
                format!(
                    "no variant format this file type takes ({})",
                    formats.join(", ")
                )
            };
            variants.fail(scene, file, None, reason);
            return None;
        }
        let variant = select_variant(options, file, bc, |cid| variants.is_bad(cid))?.clone();

        let decoded = supported_format(&variant)?.decoded;

        let url = format!("{base}/{}", variant.path);
        let key = CacheKey::Content(variant.cid.clone());
        // a BC texture's unwrapped DDS, under a name no content hash can take
        #[cfg(not(target_arch = "wasm32"))]
        let dds_key = format!("{}.dds", variant.cid);
        // keyed by the variant CID, not its path: a cache entry must name the bytes it holds
        #[cfg(target_arch = "wasm32")]
        let dds_url = format!("{base}/bevy/dds/{}.dds", variant.cid);
        let served = ServedVariant {
            scene: scene.to_owned(),
            file: file.to_owned(),
            cid: variant.cid.clone(),
            format: variant.format.clone(),
        };

        // a warm visit of a texture skips the download and the zstd decode
        if matches!(decoded, Decoded::BcTexture(_)) {
            #[cfg(not(target_arch = "wasm32"))]
            let dds = self
                .read_disk_cache(&CacheKey::Url(dds_key.clone()))
                .await
                .ok()
                .flatten();
            #[cfg(target_arch = "wasm32")]
            let dds = crate::web_cache::read(&dds_url).await;
            if let Some(dds) = dds.filter(|dds| dds.starts_with(b"DDS ")) {
                variants.served(path, served, true);
                return Some(VariantRead {
                    bytes: dds,
                    cached: true,
                });
            }
        }

        // the cache holds bytes under a content key only once they hash to it
        #[cfg(not(target_arch = "wasm32"))]
        let cached = self.read_disk_cache(&key).await.ok().flatten();
        // the web cache is keyed by url, and a path names its bytes only by convention: check
        #[cfg(target_arch = "wasm32")]
        let cached = crate::web_cache::read(&url)
            .await
            .filter(|bytes| content_hash::matches_content_hash(&variant.cid, bytes));
        let (fetched, from_cache) = match cached {
            Some(bytes) => (SharedBytes(Arc::new(bytes)), true),
            None => {
                let token = self.reqno.fetch_add(1, Ordering::SeqCst);
                let (download, task) =
                    self.share_download(key, token, path.to_owned(), url, Some(scene.to_owned()));
                if let Some(task) = task {
                    IoTaskPool::get().spawn_compat(task).detach();
                }
                let mut result = download.result();
                let fetched = result
                    .wait_for(Option::is_some)
                    .await
                    .ok()
                    .and_then(|result| result.clone());
                match fetched {
                    Some(Ok(bytes)) if content_hash::matches_content_hash(&variant.cid, &bytes) => {
                        variants.transport_ok();
                        (bytes, false)
                    }
                    Some(Ok(bytes)) => {
                        let reason = format!(
                            "{} bytes from abgen do not hash to {}",
                            bytes.len(),
                            variant.cid
                        );
                        variants.fail(scene, file, Some(&variant.cid), reason);
                        return None;
                    }
                    Some(Err(AssetReaderError::Io(e))) if e.kind() == ErrorKind::Interrupted => {
                        return None
                    }
                    Some(Err(e)) => {
                        let reason = e.to_string();
                        variants.transport_failed(&reason);
                        variants.fail(scene, file, Some(&variant.cid), format!("fetch: {reason}"));
                        return None;
                    }
                    None => return None,
                }
            }
        };

        let bytes = match decoded {
            Decoded::BcTexture(format) => {
                let dds = match ktx2::bc_ktx2_to_dds(&fetched, format) {
                    Ok(dds) => dds,
                    Err(reason) => {
                        variants.fail(
                            scene,
                            file,
                            Some(&variant.cid),
                            format!("invalid KTX2: {reason}"),
                        );
                        return None;
                    }
                };
                #[cfg(not(target_arch = "wasm32"))]
                if let Err(error) = self.cache_bytes(&dds_key, &dds).await {
                    warn!("failed to cache the DDS of `{}`: {error}", variant.cid);
                }
                #[cfg(target_arch = "wasm32")]
                if let Err(error) = crate::web_cache::keep(&dds_url, &dds).await {
                    warn!("failed to cache the DDS of `{}`: {error:?}", variant.cid);
                }
                dds
            }
            // the loader of the original path (a `.glb`/`.gltf`: the meshopt pre-pass glTF
            // loader) reads the variant's bytes as they are; the cache keeps them under the
            // variant cid alone
            #[cfg(feature = "meshopt-variants")]
            Decoded::AsIs => {
                Arc::try_unwrap(fetched.0).unwrap_or_else(|shared| shared.as_ref().clone())
            }
        };
        variants.served(path, served, from_cache);
        Some(VariantRead {
            bytes,
            cached: from_cache,
        })
    }
}

pub(crate) fn build(app: &mut App) {
    app.add_systems(Update, detect_gpu_formats);
    debug_panel::register(app);
}

/// After the asset plugins exist: an app without images or models has nothing to fall back.
pub(crate) fn finish(app: &mut App) {
    if app
        .world()
        .contains_resource::<Events<AssetLoadFailedEvent<Image>>>()
        && app
            .world()
            .contains_resource::<Events<AssetLoadFailedEvent<bevy::gltf::Gltf>>>()
    {
        app.add_systems(Update, fall_back_from_failed_variants);
    }
}

fn detect_gpu_formats(
    capabilities: Option<Res<RenderCapabilities>>,
    ipfs: Res<IpfsResource>,
    mut done: Local<bool>,
) {
    if *done {
        return;
    }
    let Some(capabilities) = capabilities else {
        return;
    };
    *done = true;
    let bc = CompressedImageFormats::from_features(capabilities.features())
        .contains(CompressedImageFormats::BC);
    let _ = ipfs.bevy_variants.bc.set(bc);
}

fn fall_back_from_failed_variants(
    mut images: EventReader<AssetLoadFailedEvent<Image>>,
    mut models: EventReader<AssetLoadFailedEvent<bevy::gltf::Gltf>>,
    ipfs: Res<IpfsResource>,
    server: Res<AssetServer>,
) {
    let failed = images
        .read()
        .map(|event| (&event.path, event.error.to_string()))
        .chain(
            models
                .read()
                .map(|event| (&event.path, event.error.to_string())),
        );
    for (path, error) in failed {
        if ipfs.bevy_variants.decode_failed(path.path(), &error) {
            server.reload(path.without_label().into_owned());
        }
    }
}

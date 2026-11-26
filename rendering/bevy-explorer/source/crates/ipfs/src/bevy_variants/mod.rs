//! Prototype: GPU-ready "Bevy variants" of the textures and models of scenes, wearables and
//! emotes, published by abgen.
//!
//! The variants base is `--bevy-variants-url` / `?bevyVariants=`, else an `abgen_cdn` service
//! override; either settles it at launch. Otherwise the feature flag [`ENDPOINT_FLAG`] settles it
//! once the flags are read ([`flagged_base`]), falling back to the `bevy_variants_url` config
//! field, else to the asset-bundle generator's CDN for the base domain
//! (`https://abgen-cdn.<base>`). `off` turns the client off. The base is settled once per session
//! (`Base`); until then manifests are requested but wait for it, files wait for their manifest,
//! and far LODs are not known yet. For each scene, wearable and emote entity it loads, the
//! explorer fetches
//! `{base}/bevy/manifest/{entityId}.json` once; it maps original content CIDs to the
//! paths of their variants, `bevy/{prefix}/{name}`, whose prefix letter is the variant's class
//! (`t` a BC KTX2 texture, `m` a `glb-br` model, `g` a GLB). A content file read whose
//! original CID has a variant of a class this client takes is answered with
//! `{base}/{variantPath}` instead and cached under a digest of that url, never under the
//! original's hash. abgen keeps a path's bytes fixed for the life of its generation, and the
//! manifest names no hash for them, so a variant is trusted by its path and rejected only by
//! what it decodes to. A texture variant is a BC KTX2 whose header names its BC format, handed
//! to the image loaders as it is; they unwrap it to the DDS they decode (`ktx2`), inside the
//! asset load. A `g` model variant is a GLB whose embedded images are BC KTX2, handed to the
//! glTF loader as it is served: the loader decodes each embedded image through the same host
//! KTX2 transcoder, which unwraps BC to DDS, so its textures stay BC in memory. Anything else,
//! at any step, reads the original. A variant that fails to decode is marked bad for the
//! session, its cached copy dropped, and its asset (a texture or a GLB) reloaded from the
//! original. Manifests and variants are fetched in a single attempt with short timeouts: the
//! original is their retry. An entity's files wait for its manifest for at most
//! [`MANIFEST_DEADLINE`] from the request, or from the base settling if that came later, then
//! read their originals.
//!
//! The formats taken are the [`SUPPORTED_FORMATS`] table: a manifest variant is loaded when its
//! `kind`/`format` pair has a row whose file types, GPU needs and decoder this client has: BC
//! KTX2 textures and `glb` models with BC KTX2 images everywhere, and on web `glb-br` models,
//! the original `.glb` byte for byte. A `glb` variant is fetched at its own path everywhere:
//! its Brotli copy exists only when it is smaller, and the server sends it to a browser that
//! accepts `br`. On web a `glb-br` variant is fetched as its Brotli copy (`{path}.br`, stored
//! with `Content-Encoding: br`) which the browser decodes. Native leaves `glb-br` to a later
//! change (it has no Brotli decoder on its http client), loading the original model.
//!
//! Preview mode, local scenes (`b64-` ids) and entities served from a loopback or file content
//! server never ask for a manifest; in preview mode that includes the wearables and emotes, so a
//! preview always shows the deployed files.
//!
//! The same base serves a scene's near-LOD descriptor, the asset pipeline's initial scene state
//! (`{base}/LOD/lods-unity/manifests/{sceneId}_InitialSceneState.json`,
//! [`IpfsIo::bevy_lod_descriptor`]), and its far LOD, abgen's one simplified mesh for the scene
//! (`{base}/LOD/lods-bevy/lods/{sceneId}_1.glb`, [`IpfsIo::bevy_far_lod`]), the latter on a GPU
//! with BC textures only, and not turned off with the variants for the session. A far LOD the
//! server has none for is not asked for again in the session, and far LODs pause while their
//! requests fail, probing until one is answered (`far_lod`).

mod debug_panel;
mod far_lod;
pub mod ktx2;
#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests;

use common::sync::MutexExt;
use std::{
    collections::{BTreeMap, HashMap, HashSet, VecDeque},
    io::ErrorKind,
    path::{Path, PathBuf},
    sync::{atomic::Ordering, Arc, Mutex, MutexGuard, OnceLock},
};

use bevy::{
    asset::{io::AssetReaderError, AssetLoadFailedEvent},
    gltf::Gltf,
    image::CompressedImageFormats,
    prelude::*,
    render::renderer::RenderCapabilities,
    tasks::IoTaskPool,
};
use common::{
    base_domain,
    feature_flags::{FeatureFlags, FlagVariant, FlagsSource},
    util::{TaskCompat, UrlLoopbackExt},
};
use serde::Deserialize;
use tokio::sync::watch;
use web_time::Duration;

pub(crate) use far_lod::FarLodHealth;
pub use far_lod::FarLodStatus;
use far_lod::{BreakerChange, FAR_LOD_BREAKER_FAILURES, FAR_LOD_COOL_DOWN};

use crate::{
    content_hash::{self, ContentHashView},
    download::FetchPolicy,
    failed_remotes,
    ipfs_path::{is_http_url, CacheKey, IpfsPath},
    IpfsIo, IpfsResource, SharedBytes,
};

/// Image paths whose loader unwraps a BC KTX2 (`ktx2::bc_ktx2_dds`) before it decodes, so a
/// texture variant loads at the original path. A path without an extension goes to the last
/// image loader, which does too.
const TEXTURE_EXTENSIONS: [&str; 5] = ["png", "jpg", "jpeg", "webp", "ktx2"];
/// Every variant class the manifest names by a path's prefix letter, taken or not.
const SCHEMA_FORMATS: [&str; 3] = ["ktx2-bc", "glb-br", "glb"];
/// Model paths a model variant stands in for: only a binary glTF, which is what it holds.
const MODEL_EXTENSIONS: [&str; 1] = ["glb"];
const RECENT_FALLBACKS: usize = 20;

/// What a supported variant's bytes are handed to the loaders as.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ContentFilePayloadKind {
    /// A BC KTX2 texture (BC1, BC5 or BC7, as its header says), which the image loaders unwrap
    /// to the DDS they decode.
    BcTexture,
    /// A `glb-br` model: the original `.glb`, fetched as its Brotli copy that the browser
    /// decodes, handed to the glTF loader of the original path. Web only.
    #[cfg(target_arch = "wasm32")]
    BrotliGlb,
    /// A `glb` model: a GLB with its embedded images as BC KTX2, handed to the glTF loader of
    /// the original path as it is; the loader decodes each image through the host KTX2
    /// transcoder, which unwraps it to DDS (`ktx2::bc_ktx2_dds`).
    BcGlb,
}

/// A variant class this client takes: its `kind`/`format` (from the path's prefix letter), the
/// content files it may replace, what the GPU must support and what the bytes decode as.
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
    pub payload_kind: ContentFilePayloadKind,
}

impl SupportedFormat {
    fn accepts_path(&self, content_path: ContentFilePathView<'_>) -> bool {
        match extension(content_path.0) {
            Some(ext) => self.extensions.contains(&ext.as_str()),
            None => self.without_extension,
        }
    }
}

/// The variant classes this client takes. A variant is loaded only through a row here; the
/// manifest's order of preference decides between rows.
pub const SUPPORTED_FORMATS: &[SupportedFormat] = &[
    SupportedFormat {
        kind: "texture",
        format: "ktx2-bc",
        extensions: &TEXTURE_EXTENSIONS,
        without_extension: true,
        needs_bc: true,
        payload_kind: ContentFilePayloadKind::BcTexture,
    },
    SupportedFormat {
        kind: "model",
        format: "glb",
        extensions: &MODEL_EXTENSIONS,
        without_extension: false,
        needs_bc: true,
        payload_kind: ContentFilePayloadKind::BcGlb,
    },
    // web only: the browser decodes the Brotli copy; native has no decoder on its http client
    #[cfg(target_arch = "wasm32")]
    SupportedFormat {
        kind: "model",
        format: "glb-br",
        extensions: &MODEL_EXTENSIONS,
        without_extension: false,
        needs_bc: false,
        payload_kind: ContentFilePayloadKind::BrotliGlb,
    },
];

/// The row for a manifest variant's `kind`/`format`, if this client takes it.
pub fn supported_format(variant: &ContentFileVariant) -> Option<&'static SupportedFormat> {
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
        let how = match row.payload_kind {
            ContentFilePayloadKind::BcTexture => {
                "BC1/BC5/BC7, unwrapped to DDS by the image loaders"
            }
            #[cfg(target_arch = "wasm32")]
            ContentFilePayloadKind::BrotliGlb => "Brotli, decoded by the browser",
            ContentFilePayloadKind::BcGlb => {
                "embedded BC1/BC5/BC7, unwrapped to DDS by the glTF loader"
            }
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
/// How long an entity's files wait for its manifest, from the moment it was requested. A file
/// read after that goes to the original at once; a manifest that arrives later still serves
/// the files read after it. Long enough for a first connection to the service (DNS, TLS), and
/// short next to a cold scene load.
pub const MANIFEST_DEADLINE: Duration = Duration::from_secs(2);
/// How long a manifest request may wait for headers, or between body chunks, in its single
/// attempt. Its files stop waiting for it at [`MANIFEST_DEADLINE`] anyway; this bounds how long
/// a host that never answers holds a request slot.
const MANIFEST_TIMEOUT: Duration = Duration::from_secs(3);
/// A variant download: one attempt, with short timeouts, since the original is the retry.
pub(crate) const VARIANT_FETCH: FetchPolicy = FetchPolicy {
    headers_timeout: Duration::from_secs(5),
    idle_timeout: Duration::from_secs(5),
    max_attempts: 1,
};
/// Consecutive transport failures (manifest or variant) after which the service is treated as
/// down for the rest of the session, so a dead host costs a few requests, not one per file.
const BREAKER_FAILURES: u32 = 5;
/// How long an avatar waits for a model reloading from its original after its `glb` variant
/// failed, before it assembles without it. Long enough for the original's download on a slow
/// link; past it the avatar is built as it was before variants.
const MODEL_RELOAD_WAIT: Duration = Duration::from_secs(120);
/// Where a scene's far LOD is under the variants base:
/// `{base}{FAR_LOD_DIR}{sceneId}{FAR_LOD_SUFFIX}`.
const FAR_LOD_DIR: &str = "/LOD/lods-bevy/lods/";
const FAR_LOD_SUFFIX: &str = "_1.glb";

/// One variant of an original: where it is served, and the class its path's prefix letter
/// names. The path also identifies it (failed variants, caches): abgen keeps a path's bytes
/// fixed for the life of its generation.
#[derive(Debug, Clone, PartialEq)]
pub struct ContentFileVariant {
    /// Relative to the base: `bevy/{prefix}/{name}`.
    pub path: String,
    pub kind: String,
    pub format: String,
}

impl ContentFileVariant {
    /// The variant at `path`, when it is `bevy/{prefix}/{name}` with plain segments and its
    /// prefix letter is a class the schema names; anything else is dropped.
    pub fn from_path(path: &str) -> Option<ContentFileVariant> {
        if !is_variant_path(path) {
            return None;
        }
        let prefix = path.split('/').nth(1)?;
        let (kind, format) = match prefix.as_bytes().first()? {
            b't' => ("texture", "ktx2-bc"),
            b'm' => ("model", "glb-br"),
            b'g' => ("model", "glb"),
            _ => return None,
        };
        Some(ContentFileVariant {
            path: path.to_owned(),
            kind: kind.to_owned(),
            format: format.to_owned(),
        })
    }

    /// Whether this client can load the variant at `content_path`: a supported format, for a
    /// file type it may replace, on a GPU that has what it needs (`bc`: BC texture support).
    pub(crate) fn supports(&self, content_path: ContentFilePathView<'_>, bc: bool) -> bool {
        supported_format(self)
            .is_some_and(|row| (bc || !row.needs_bc) && row.accepts_path(content_path))
    }

    /// The first of `variants` this client supports at `content_path` and `is_bad` does not
    /// reject.
    pub(crate) fn preferred<'a>(
        variants: &'a [ContentFileVariant],
        content_path: ContentFilePathView<'_>,
        bc: bool,
        is_bad: impl Fn(&str) -> bool,
    ) -> Option<&'a ContentFileVariant> {
        variants
            .iter()
            .find(|variant| variant.supports(content_path, bc) && !is_bad(&variant.path))
    }
}

/// The feature flag that names the variants base (`explorer-bevy-variants-endpoint` in Unleash).
pub(crate) const ENDPOINT_FLAG: &str = "bevy-variants-endpoint";

/// Whether a flag may name `url` as the variants base: `https` on a Decentraland host (one of
/// [`base_domain::DECENTRALAND_DOMAINS`] or under it), on the default port, with no userinfo,
/// query or fragment.
fn is_decentraland_base(url: &str) -> bool {
    url::Url::parse(url).is_ok_and(|parsed| {
        parsed.scheme() == "https"
            && parsed.username().is_empty()
            && parsed.password().is_none()
            && parsed.port().is_none()
            && parsed.query().is_none()
            && parsed.fragment().is_none()
            && base_domain::decentraland_domain(url).is_some()
    })
}

/// The variants base [`ENDPOINT_FLAG`] chooses, None for off, and how it chose it. Its enabled
/// variant decides: `off` turns the variants off, `on` keeps `default`, and any other carries the
/// base as a `string` payload. Anything else keeps `default` (None: off): the flag disabled or
/// unread, no enabled variant, or a payload [`is_decentraland_base`] refuses, so a flag cannot
/// send the variants' requests elsewhere.
pub(crate) fn flagged_base(
    variant: Option<&FlagVariant>,
    default: Option<&str>,
) -> (Option<String>, String) {
    let default = || default.map(str::to_owned);
    let Some(variant) = variant.filter(|variant| variant.enabled) else {
        return (
            default(),
            "endpoint flag: no variant, the default".to_owned(),
        );
    };
    match variant.name.as_str() {
        "off" => return (None, "endpoint flag: `off`".to_owned()),
        "on" => return (default(), "endpoint flag: `on`, the default".to_owned()),
        _ => (),
    }
    let url = variant
        .payload
        .as_ref()
        .filter(|payload| payload.kind == "string")
        .map(|payload| payload.value.trim());
    match url {
        Some(url) if is_decentraland_base(url) => (
            Some(url.to_owned()),
            format!("endpoint flag: `{}`", variant.name),
        ),
        _ => {
            warn!(
                "bevy variants: the `{ENDPOINT_FLAG}` variant `{}` names no https Decentraland \
                 base url; keeping the default",
                variant.name
            );
            (
                default(),
                format!(
                    "endpoint flag: `{}` names no https Decentraland base url, the default",
                    variant.name
                ),
            )
        }
    }
}

#[derive(Debug)]
pub struct Manifest {
    /// The generation each variant class was built from (`texture` → `t2`, …), for the debug
    /// panel; the client does not act on it.
    pub lanes: BTreeMap<String, String>,
    /// Variants by original content CID, in the manifest's order of preference.
    pub files: HashMap<String, Vec<ContentFileVariant>>,
}

#[derive(Deserialize)]
struct ManifestJson {
    /// abgen's, and only shown: read as any JSON, so its shape never decides whether the
    /// manifest is accepted.
    #[serde(default)]
    lanes: serde_json::Value,
    files: HashMap<String, Vec<String>>,
}

/// A manifest: `lanes` and `files`, original content CID → variant paths. Its `v` and any other
/// field are abgen's own (they tell it when to rebuild) and are not read: whatever a manifest
/// lists is loaded, whichever generation built it. Of `lanes`, the entries that are strings are
/// kept for the debug panel. It is fetched at the entity's own url, so it names no entity. Paths
/// that are not `bevy/{prefix}/{name}` with plain segments (a path becomes part of a url) or
/// whose prefix letter names no class are dropped, and so is an original left with none.
pub fn parse_manifest(bytes: &[u8]) -> Result<Manifest, String> {
    let json: ManifestJson = serde_json::from_slice(bytes).map_err(|e| e.to_string())?;
    let files = json
        .files
        .into_iter()
        .map(|(original, paths)| {
            let variants: Vec<_> = paths
                .iter()
                .filter_map(|path| ContentFileVariant::from_path(path))
                .collect();
            (original, variants)
        })
        .filter(|(_, variants)| !variants.is_empty())
        .collect();
    let lanes = json
        .lanes
        .as_object()
        .into_iter()
        .flatten()
        .filter_map(|(class, prefix)| Some((class.clone(), prefix.as_str()?.to_owned())))
        .collect();
    Ok(Manifest { lanes, files })
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

/// The path of a content file as its scene names it: relative to the scene, or a url.
#[derive(Clone, Copy, Debug)]
pub(crate) struct ContentFilePathView<'a>(&'a str);

impl<'a> ContentFilePathView<'a> {
    /// The content file path of `ipfs_path`, if it names a content file.
    pub(crate) fn of(ipfs_path: &'a IpfsPath) -> Option<Self> {
        ipfs_path.content_path().map(Self)
    }

    pub(crate) fn as_str(self) -> &'a str {
        self.0
    }
}

/// The variants base url: http(s), in its serialised form, without a trailing `/`.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct VariantsBaseUrl(String);

impl VariantsBaseUrl {
    fn parse(url: &str) -> Option<Self> {
        match url::Url::parse(url.trim()) {
            Ok(parsed) if matches!(parsed.scheme(), "http" | "https") => {
                // `Url`'s serialised form, so it is a prefix of the urls `Url` serialises under it
                Some(Self(parsed.as_str().trim_end_matches('/').to_owned()))
            }
            _ => None,
        }
    }

    fn view(&self) -> VariantsBaseUrlView<'_> {
        VariantsBaseUrlView(&self.0)
    }
}

/// A borrowed [`VariantsBaseUrl`].
#[derive(Clone, Copy, Debug)]
struct VariantsBaseUrlView<'a>(&'a str);

impl<'a> VariantsBaseUrlView<'a> {
    fn as_str(self) -> &'a str {
        self.0
    }

    /// The url of `variant` under this base: its path, or for a `glb-br` variant the path of
    /// its Brotli copy (`{path}.br`).
    fn url_of(self, variant: &ContentFileVariant, payload_kind: ContentFilePayloadKind) -> String {
        match payload_kind {
            ContentFilePayloadKind::BcTexture | ContentFilePayloadKind::BcGlb => {
                format!("{self}/{}", variant.path)
            }
            #[cfg(target_arch = "wasm32")]
            ContentFilePayloadKind::BrotliGlb => format!("{self}/{}.br", variant.path),
        }
    }
}

impl std::fmt::Display for VariantsBaseUrlView<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.0)
    }
}

/// Whether a file of this type could be replaced by any supported format; others never wait on
/// a manifest. A url a scene names (a far LOD's among them) never could: a manifest maps the
/// scene's content files, by hash.
pub(crate) fn could_have_variant(content_path: ContentFilePathView<'_>) -> bool {
    !is_http_url(content_path.0)
        && SUPPORTED_FORMATS
            .iter()
            .any(|row| row.accepts_path(content_path))
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

/// Where a scene's far LOD comes from ([`IpfsIo::bevy_far_lod`]).
#[derive(Clone, Debug, PartialEq)]
pub enum FarLodSource {
    /// abgen's mesh for the scene.
    Url(String),
    /// Not known yet: the variants base has not settled, the GPU has not reported its BC
    /// support, or the content context is being written. Ask again later.
    NotYet,
    /// None for this scene in this session, and why.
    Unavailable(String),
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
    /// The variant's path, its identity.
    variant: String,
    format: String,
    /// Where its bytes came from, the key of their cached copy.
    url: String,
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
    /// [downloaded, from cache] by variant format.
    served_by_format: BTreeMap<String, [u64; 2]>,
    fallbacks: u64,
    /// Fallbacks from a variant that was chosen, by its format.
    fallbacks_by_format: BTreeMap<String, u64>,
    recent: VecDeque<Fallback>,
    /// Models reloading from their original after their `glb` variant failed, and since when.
    reloading: HashMap<PathBuf, web_time::Instant>,
}

impl Session {
    fn fallback(&mut self, scene: &str, file: &str, format: Option<&str>, reason: String) {
        debug!("bevy variant fallback for `{file}` in {scene}: {reason}");
        self.fallbacks += 1;
        if let Some(format) = format {
            *self
                .fallbacks_by_format
                .entry(format.to_owned())
                .or_default() += 1;
        }
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

/// The session's variants base. It leaves `Pending` once and changes no more.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Base {
    /// Waiting for [`ENDPOINT_FLAG`]; the base it falls back to, None for off.
    Pending(Option<VariantsBaseUrl>),
    Off,
    /// The base url, and when it settled.
    On(VariantsBaseUrl, web_time::Instant),
}

/// The variants base and what set it.
#[derive(Clone, Debug)]
pub(crate) struct Route {
    pub(crate) base: Base,
    pub(crate) source: String,
}

impl Default for Route {
    fn default() -> Self {
        Route {
            base: Base::Off,
            source: "not set".to_owned(),
        }
    }
}

/// The variants base and the GPU's BC support.
#[derive(Default)]
pub(crate) struct ContentFileVariantsConfig {
    route: watch::Sender<Route>,
    /// The GPU's BC support, once the renderer has reported it.
    bc: OnceLock<bool>,
}

impl ContentFileVariantsConfig {
    /// The settled base url; None while variants are off or the base is pending.
    fn base_url(&self) -> Option<VariantsBaseUrl> {
        match &self.route.borrow().base {
            Base::On(base_url, _) => Some(base_url.clone()),
            Base::Pending(_) | Base::Off => None,
        }
    }

    fn route(&self) -> Route {
        self.route.borrow().clone()
    }

    fn is_off(&self) -> bool {
        self.route.borrow().base == Base::Off
    }

    fn is_pending(&self) -> bool {
        matches!(self.route.borrow().base, Base::Pending(_))
    }

    /// The settled base url and when it settled, None for off; waits while the base is pending.
    async fn settled(&self) -> Option<(VariantsBaseUrl, web_time::Instant)> {
        let mut route = self.route.subscribe();
        let route = route
            .wait_for(|route| !matches!(route.base, Base::Pending(_)))
            .await
            .ok()?;
        match &route.base {
            Base::On(base_url, at) => Some((base_url.clone(), *at)),
            Base::Pending(_) | Base::Off => None,
        }
    }

    /// The fallback and its source while the base waits for [`ENDPOINT_FLAG`].
    fn pending_fallback(&self) -> Option<(Option<VariantsBaseUrl>, String)> {
        let route = self.route.borrow();
        match &route.base {
            Base::Pending(fallback) => Some((fallback.clone(), route.source.clone())),
            Base::On(..) | Base::Off => None,
        }
    }

    /// Settles the base: `url`, or off with None or a url that is not http(s).
    fn settle(&self, url: Option<&str>, source: &str) {
        let (base, source) = match url.map(|url| (url, VariantsBaseUrl::parse(url))) {
            Some((_, Some(base_url))) => {
                info!("bevy variants: on, from `{}` ({source})", base_url.view());
                (
                    Base::On(base_url, web_time::Instant::now()),
                    source.to_owned(),
                )
            }
            Some((url, None)) => {
                warn!("bevy variants: off, `{url}` is not an http(s) url ({source})");
                (Base::Off, format!("{source}: not an http(s) url"))
            }
            None => {
                info!("bevy variants: off ({source})");
                (Base::Off, source.to_owned())
            }
        };
        self.route.send_replace(Route { base, source });
    }

    /// Leaves the base to [`ENDPOINT_FLAG`], falling back to `fallback`: None, or a url that is
    /// not http(s), is off.
    fn wait_for_flag(&self, fallback: Option<&str>, source: &str) {
        let (fallback, source) = match fallback.map(|url| (url, VariantsBaseUrl::parse(url))) {
            Some((_, Some(base_url))) => (Some(base_url), source.to_owned()),
            Some((url, None)) => {
                warn!("bevy variants: the fallback `{url}` is not an http(s) url; off");
                (None, format!("{source}: not an http(s) url"))
            }
            None => (None, source.to_owned()),
        };
        self.route.send_replace(Route {
            base: Base::Pending(fallback),
            source,
        });
    }

    /// The GPU's BC support; `None` until it is reported.
    fn reported_bc(&self) -> Option<bool> {
        self.bc.get().copied()
    }

    /// The GPU's BC support; `false` until it is reported.
    fn bc(&self) -> bool {
        self.reported_bc().unwrap_or(false)
    }

    /// Sets the GPU's BC support. Only the first report is taken.
    fn init_bc(&self, bc: bool) {
        let _ = self.bc.set(bc);
    }

    /// The scene whose far LOD `remote` is ([`IpfsIo::bevy_far_lod`]'s url), if it is one.
    fn far_lod_scene<'a>(&self, remote: &'a str) -> Option<&'a str> {
        let base_url = self.base_url()?;
        remote
            .strip_prefix(base_url.view().as_str())?
            .strip_prefix(FAR_LOD_DIR)?
            .strip_suffix(FAR_LOD_SUFFIX)
    }
}

/// Each entity's variants manifest, and when it was requested.
#[derive(Default)]
pub(crate) struct ContentFileVariantManifests(Mutex<RequestedManifests>);

#[derive(Default)]
struct RequestedManifests {
    states: HashMap<String, watch::Receiver<ManifestState>>,
    /// For [`MANIFEST_DEADLINE`].
    requested_at: HashMap<String, web_time::Instant>,
}

impl ContentFileVariantManifests {
    /// A new pending manifest for `entity`, or None when one was already requested.
    fn begin(&self, entity: &str) -> Option<watch::Sender<ManifestState>> {
        let mut manifests = self.0.lock_ignore_poison();
        if manifests.states.contains_key(entity) {
            return None;
        }
        let (sender, receiver) = watch::channel(ManifestState::Pending);
        manifests.states.insert(entity.to_owned(), receiver);
        manifests
            .requested_at
            .insert(entity.to_owned(), web_time::Instant::now());
        Some(sender)
    }

    /// How much of [`MANIFEST_DEADLINE`] is left, counted from `entity`'s request, or from
    /// `settled_at` if that came later.
    fn wait_left(&self, entity: &str, settled_at: web_time::Instant) -> Duration {
        let from = self
            .0
            .lock_ignore_poison()
            .requested_at
            .get(entity)
            .map_or(settled_at, |at| (*at).max(settled_at));
        MANIFEST_DEADLINE.saturating_sub(from.elapsed())
    }

    fn get(&self, entity: &str) -> Option<watch::Receiver<ManifestState>> {
        self.0.lock_ignore_poison().states.get(entity).cloned()
    }
}

/// The variants served, failed and reloading this session, and whether variants are off.
#[derive(Default)]
pub(crate) struct ContentFileVariantSession(Mutex<Session>);

impl ContentFileVariantSession {
    fn lock(&self) -> MutexGuard<'_, Session> {
        self.0.lock_ignore_poison()
    }

    fn disabled(&self) -> Option<String> {
        self.lock().disabled.clone()
    }

    fn transport_ok(&self) {
        self.lock().transport_failures = 0;
    }

    fn transport_failed(&self, reason: &str) {
        let mut session = self.lock();
        session.transport_failures += 1;
        if session.transport_failures >= BREAKER_FAILURES && session.disabled.is_none() {
            warn!("bevy variants: off for this session, {BREAKER_FAILURES} failed requests ({reason})");
            session.disabled = Some(format!(
                "{BREAKER_FAILURES} consecutive failed requests, last: {reason}"
            ));
        }
    }

    fn is_bad(&self, variant: &str) -> bool {
        self.lock().bad.contains(variant)
    }

    fn fail(&self, scene: &str, file: &str, variant: Option<&ContentFileVariant>, reason: String) {
        let mut session = self.lock();
        if let Some(variant) = variant {
            session.bad.insert(variant.path.clone());
        }
        session.fallback(scene, file, variant.map(|v| v.format.as_str()), reason);
    }

    fn served(&self, path: &Path, served: ServedVariant, cached: bool) {
        let mut session = self.lock();
        if cached {
            session.cached += 1;
        } else {
            session.downloaded += 1;
        }
        session
            .served_by_format
            .entry(served.format.clone())
            .or_default()[usize::from(cached)] += 1;
        session.served.insert(path.to_owned(), served);
    }

    /// Whether the asset at `path` was last read from a variant.
    pub fn is_served(&self, path: &Path) -> bool {
        self.lock().served.contains_key(path)
    }

    /// The url of the variant the asset at `path` was last read from.
    fn served_url(&self, path: &Path) -> Option<String> {
        self.lock()
            .served
            .get(path)
            .map(|served| served.url.clone())
    }

    /// Whether the model at `path`, which failed to load, will still come back from its
    /// original: it was read from a `glb` variant, whose failure reloads it, or that reload is
    /// under way. A reload stops counting after [`MODEL_RELOAD_WAIT`].
    fn model_reload_pending(&self, path: &Path) -> bool {
        let mut session = self.lock();
        session
            .reloading
            .retain(|_, since| since.elapsed() < MODEL_RELOAD_WAIT);
        session.reloading.contains_key(path)
            || session.served.get(path).is_some_and(|served| {
                <Gltf as VariantAsset>::FORMATS.contains(&served.format.as_str())
            })
    }

    fn reload_started(&self, path: &Path) {
        self.lock()
            .reloading
            .insert(path.to_owned(), web_time::Instant::now());
    }

    fn reload_finished(&self, path: &Path) {
        self.lock().reloading.remove(path);
    }

    /// Marks the variant served for `path` bad, if there was one of `formats`, and returns the
    /// url of its cached copy: the asset should be reloaded (from its original), and the copy
    /// dropped.
    fn decode_failed(&self, path: &Path, error: &str, formats: &[&str]) -> Option<String> {
        let mut session = self.lock();
        if !session
            .served
            .get(path)
            .is_some_and(|served| formats.contains(&served.format.as_str()))
        {
            return None;
        }
        let served = session.served.remove(path)?;
        session.bad.insert(served.variant.clone());
        session.fallback(
            &served.scene,
            &served.file,
            Some(&served.format),
            format!("{} variant failed to load: {error}", served.format),
        );
        Some(served.url)
    }
}

impl IpfsIo {
    /// The variant `manifest` gives the content file at `content_path`, named by `content_hash`,
    /// with its payload kind and url; else `Err`, with the reason when one is worth reporting.
    fn fetchable_variant(
        &self,
        base_url: VariantsBaseUrlView<'_>,
        manifest: &Manifest,
        content_hash: ContentHashView<'_>,
        content_path: ContentFilePathView<'_>,
    ) -> Result<(ContentFileVariant, ContentFilePayloadKind, String), Option<String>> {
        let options = manifest.files.get(content_hash.as_str()).ok_or(None)?;
        if self.content_file_variant_session.disabled().is_some() {
            return Err(None);
        }
        let bc = self.content_file_variants_config.bc();
        if !options
            .iter()
            .any(|variant| variant.supports(content_path, bc))
        {
            let formats: Vec<_> = options.iter().map(|v| v.format.as_str()).collect();
            // a format this file type would take on a GPU with BC textures
            let wants_bc = !bc
                && options
                    .iter()
                    .any(|variant| variant.supports(content_path, true));
            return Err(Some(if wants_bc {
                "the GPU has no BC texture support (or has not reported it yet)".to_owned()
            } else {
                format!(
                    "no variant format this file type takes ({})",
                    formats.join(", ")
                )
            }));
        }
        let variant = ContentFileVariant::preferred(options, content_path, bc, |path| {
            self.content_file_variant_session.is_bad(path)
        })
        .ok_or(None)?
        .clone();
        let payload_kind = supported_format(&variant).ok_or(None)?.payload_kind;
        let url = base_url.url_of(&variant, payload_kind);
        Ok((variant, payload_kind, url))
    }

    /// The url of the variant the content file at `ipfs_path`, named by `content_hash`, maps to
    /// now: `Some(None)` when there is none, `None` while the base is pending, or its entity's
    /// manifest is and its wait has time left.
    fn planned_variant_url(
        &self,
        ipfs_path: &IpfsPath,
        content_hash: ContentHashView<'_>,
    ) -> Option<Option<String>> {
        let config = &self.content_file_variants_config;
        let (Some(scene), Some(content_path)) =
            (ipfs_path.scene_hash(), ContentFilePathView::of(ipfs_path))
        else {
            return Some(None);
        };
        if config.is_off() || !could_have_variant(content_path) {
            return Some(None);
        }
        let Some(manifest) = self.content_file_variant_manifests.get(scene) else {
            return Some(None);
        };
        let state = manifest.borrow().clone();
        match state {
            ManifestState::Pending => match config.route().base {
                Base::Pending(_) => None,
                Base::On(_, settled_at)
                    if !self
                        .content_file_variant_manifests
                        .wait_left(scene, settled_at)
                        .is_zero() =>
                {
                    None
                }
                Base::On(..) | Base::Off => Some(None),
            },
            ManifestState::Loaded(manifest) => {
                let base_url = config.base_url()?;
                Some(
                    self.fetchable_variant(base_url.view(), &manifest, content_hash, content_path)
                        .ok()
                        .map(|(_, _, url)| url),
                )
            }
            _ => Some(None),
        }
    }
}

/// What the loader reads for a variant (a BC texture's KTX2, a `glb` model or a `glb-br`
/// model as served), and whether it came from the cache.
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
    /// Settles the variants base for the session: `url`, or off with None or a url that is not
    /// http(s). `source` is what set it.
    pub fn init_bevy_variants_base(&self, url: Option<&str>, source: &str) {
        self.content_file_variants_config.settle(url, source);
    }

    /// Leaves the variants base to [`ENDPOINT_FLAG`] once the flags are read, falling back to
    /// `fallback` (None: off), which `source` set. Until it settles, manifests wait for it and
    /// far LODs are not known yet.
    pub fn route_bevy_variants_by_flag(&self, fallback: Option<&str>, source: &str) {
        self.content_file_variants_config
            .wait_for_flag(fallback, source);
    }

    /// Whether the variants base is waiting for [`ENDPOINT_FLAG`].
    pub fn bevy_variants_pending(&self) -> bool {
        self.content_file_variants_config.is_pending()
    }

    /// Whether the asset at `path` was last read from a variant (and so must not be
    /// reprocessed as if it were the original).
    pub fn served_bevy_variant(&self, path: &Path) -> bool {
        self.content_file_variant_session.is_served(path)
    }

    /// The url the bytes of the content file at asset `path` are read from, when every read of
    /// that url gives the same bytes: the variant that answered it, else the original's url when
    /// a content hash names the file and the server is not loopback (a preview server's files
    /// change). Two loads with the same url hold the same file.
    ///
    /// Blocking: reads the context.
    pub fn immutable_source_url(&self, path: &Path) -> Option<String> {
        if let Some(url) = self.content_file_variant_session.served_url(path) {
            return Some(url);
        }
        let ipfs_path = IpfsPath::new_from_path(path).ok()??;
        self.immutable_original_url(&ipfs_path).map(|(_, url)| url)
    }

    /// [`Self::immutable_source_url`] of a read of the content file at asset `path` that has not
    /// happened yet: the variant its entity's manifest picks now, else the original's url. None
    /// while that manifest is pending.
    ///
    /// Blocking: reads the context.
    pub(crate) fn planned_source_url(&self, path: &Path) -> Option<String> {
        if let Some(url) = self.content_file_variant_session.served_url(path) {
            return Some(url);
        }
        let ipfs_path = IpfsPath::new_from_path(path).ok()??;
        let (original, url) = self.immutable_original_url(&ipfs_path)?;
        Some(
            self.planned_variant_url(
                &ipfs_path,
                ContentHashView::try_from(original.as_str()).ok()?,
            )?
            .unwrap_or(url),
        )
    }

    /// The content hash of the file at `ipfs_path` and its original's url, when a content hash
    /// names it and the server is not loopback.
    fn immutable_original_url(&self, ipfs_path: &IpfsPath) -> Option<(String, String)> {
        let context = self.context.blocking_read();
        let hash = ipfs_path
            .hash(&context)
            .filter(|hash| ContentHashView::try_from(hash.as_str()).is_ok())?;
        let url = ipfs_path.to_url(&context).ok()?;
        (!common::util::is_loopback_url(&url)).then_some((hash, url))
    }

    /// Whether the model at `path`, which failed to load, is about to load again from its
    /// original because its `glb` variant failed. An avatar waits for it instead of assembling
    /// without that piece.
    pub fn bevy_variant_model_reload_pending(&self, path: &Path) -> bool {
        self.content_file_variant_session.model_reload_pending(path)
    }

    /// Drops the cached copy of the variant fetched from `url`: it was rejected, and must not be
    /// read again in a later session.
    async fn forget_variant(&self, url: &str) {
        #[cfg(not(target_arch = "wasm32"))]
        let forgotten = self
            .forget_disk_cache(&CacheKey::Url(content_hash::url_cache_name(url)))
            .await
            .map_err(|e| e.to_string());
        #[cfg(target_arch = "wasm32")]
        let forgotten = crate::web_cache::forget(url)
            .await
            .map_err(|e| format!("{e:?}"));
        if let Err(error) = forgotten {
            warn!("failed to drop the cached copy of `{url}`: {error}");
        }
    }

    /// The asset pipeline's initial-scene-state descriptor for a scene, which places its near
    /// LOD, under the variants base; `None` when variants are off or the base is pending
    /// ([`IpfsIo::bevy_variants_pending`]).
    pub fn bevy_lod_descriptor(&self, scene: &str) -> Option<String> {
        let base = self.content_file_variants_config.base_url()?;
        Some(format!(
            "{base}/LOD/lods-unity/manifests/{}_InitialSceneState.json",
            scene.to_lowercase()
        ))
    }

    /// abgen's one simplified mesh for a scene, its far LOD, under the variants base. Offered on
    /// the variants base (variants on, the scene eligible) and only on a GPU with BC textures:
    /// the mesh's atlases are BC KTX2 only. It does not depend on the variants client having
    /// turned itself off for the session: far LODs keep to their own breaker.
    /// Like a `glb` variant it is fetched at its own path: its Brotli copy exists only when it is
    /// smaller, and the server sends it to a browser that accepts `br`.
    ///
    /// Not offered again for a scene the server answered a definitive status for (404) this
    /// session. Naming a url does not spend the breaker's probe: whether its download may start
    /// is [`IpfsIo::far_lod_may_request`].
    pub fn bevy_far_lod(&self, scene: &str) -> FarLodSource {
        let base = match self.content_file_variants_config.route().base {
            Base::On(base_url, _) => base_url,
            Base::Pending(_) => return FarLodSource::NotYet,
            Base::Off => return FarLodSource::Unavailable("bevy variants off".to_owned()),
        };
        match self.content_file_variants_config.reported_bc() {
            None => return FarLodSource::NotYet,
            Some(false) => {
                return FarLodSource::Unavailable("the GPU has no BC texture support".to_owned())
            }
            Some(true) => (),
        }
        let contents = match self.context.try_read() {
            Ok(context) => context.contents_base_url(scene),
            Err(_) => return FarLodSource::NotYet,
        };
        if let Err(reason) = scene_eligibility(scene, contents.as_deref(), self.is_preview) {
            return FarLodSource::Unavailable(reason.to_owned());
        }
        let scene = scene.to_lowercase();
        {
            let mut health = self.far_lod_health.lock_ignore_poison();
            if health.is_missing(&scene) {
                return FarLodSource::Unavailable("the server has none for the scene".to_owned());
            }
            health.name(&scene);
        }
        FarLodSource::Url(format!(
            "{}{FAR_LOD_DIR}{scene}{FAR_LOD_SUFFIX}",
            base.view()
        ))
    }

    /// Records what a download of `remote` came to when it is a far LOD's: the mesh (`Ok`), a
    /// status (`Err(Some)`), or another failure: an error or a timeout (`Err(None)`). A
    /// definitive status (`failed_remotes::is_definitive`, such as 404) means the server has no
    /// mesh for the scene; any status other than that counts as a failure.
    pub(crate) fn far_lod_fetched(
        &self,
        remote: &str,
        result: Result<(), Option<reqwest::StatusCode>>,
    ) {
        let Some(scene) = self.content_file_variants_config.far_lod_scene(remote) else {
            return;
        };
        let change = {
            let mut health = self.far_lod_health.lock_ignore_poison();
            // only a url this client named is a far LOD: a scene can make up any other
            if !health.is_named(scene) {
                return;
            }
            match result {
                Ok(()) => health.answered(scene, false),
                Err(Some(status)) if failed_remotes::is_definitive(status) => {
                    health.answered(scene, true)
                }
                Err(_) => health.failed(scene, web_time::Instant::now()),
            }
        };
        match change {
            Some(BreakerChange::Tripped) => warn!(
                "far LODs: paused, {FAR_LOD_BREAKER_FAILURES} requests in a row failed (last \
                 `{remote}`); one is tried every {}s until it is answered",
                FAR_LOD_COOL_DOWN.as_secs()
            ),
            Some(BreakerChange::Recovered) => info!("far LODs: answered again, resumed"),
            None => (),
        }
    }

    /// Whether a far LOD's download may start now. While far LODs are paused it is false, except
    /// once per cool-down (the probe), which a true answer spends, so call it only right before
    /// starting a download. Independent of the variants session switch.
    pub fn far_lod_may_request(&self) -> bool {
        self.far_lod_health
            .lock_ignore_poison()
            .may_request(web_time::Instant::now())
    }

    /// Whether the server answered this session that it has no far LOD for `scene`.
    pub fn far_lod_missing(&self, scene: &str) -> bool {
        self.far_lod_health
            .lock_ignore_poison()
            .is_missing(&scene.to_lowercase())
    }

    /// Whether `remote` is a far LOD url this client named ([`IpfsIo::bevy_far_lod`]).
    pub(crate) fn is_named_far_lod(&self, remote: &str) -> bool {
        self.content_file_variants_config
            .far_lod_scene(remote)
            .is_some_and(|scene| self.far_lod_health.lock_ignore_poison().is_named(scene))
    }

    /// Whether no far LOD can be offered this session: variants off, a GPU known to have no BC
    /// textures, or preview. False while the base is pending. Far LODs are not turned off with
    /// the variants client for the session: an outage is the far LOD breaker's to handle, and it
    /// recovers.
    pub fn far_lods_off(&self) -> bool {
        self.is_preview
            || self.content_file_variants_config.is_off()
            || self.content_file_variants_config.reported_bc() == Some(false)
    }

    /// The far LODs' missing meshes and breaker, for `/scene_lod_state`.
    pub fn far_lod_status(&self) -> FarLodStatus {
        self.far_lod_health
            .lock_ignore_poison()
            .status(web_time::Instant::now())
    }

    /// Asks abgen for an entity's manifest (a scene, wearable or emote), once per session, from
    /// the base once it has settled. A no-op when variants are off.
    pub fn request_bevy_variants(self: &Arc<Self>, entity: &str) {
        if self.content_file_variants_config.is_off() {
            return;
        }
        let Some(sender) = self.content_file_variant_manifests.begin(entity) else {
            return;
        };
        let io = self.clone();
        let entity = entity.to_owned();
        IoTaskPool::get()
            .spawn_compat(async move {
                let state = match io.content_file_variants_config.settled().await {
                    Some((base_url, _)) => {
                        io.fetch_bevy_variants_manifest(base_url.view().as_str(), &entity)
                            .await
                    }
                    None => ManifestState::Failed("variants off".to_owned()),
                };
                sender.send_replace(state);
            })
            .detach();
    }

    async fn fetch_bevy_variants_manifest(&self, base: &str, entity: &str) -> ManifestState {
        let contents = self.context.read().await.contents_base_url(entity);
        if let Err(reason) = scene_eligibility(entity, contents.as_deref(), self.is_preview) {
            return ManifestState::Ineligible(reason);
        }
        if let Some(reason) = self.content_file_variant_session.disabled() {
            return ManifestState::Failed(format!("variants off: {reason}"));
        }
        let url = format!("{base}/bevy/manifest/{entity}.json");
        let request = match self.client.get(&url).build() {
            Ok(request) => request,
            Err(e) => return ManifestState::Failed(e.to_string()),
        };
        match self
            .fetch_request_once(
                request,
                self.client.clone(),
                MANIFEST_TIMEOUT,
                MANIFEST_TIMEOUT,
                Some(entity.to_owned()),
            )
            .await
        {
            Ok(response) => {
                self.content_file_variant_session.transport_ok();
                match parse_manifest(&response.body) {
                    Ok(manifest) => ManifestState::Loaded(Arc::new(manifest)),
                    Err(e) => ManifestState::Failed(format!("invalid manifest: {e}")),
                }
            }
            Err(platform::FetchError::Status(status, _))
                if status == reqwest::StatusCode::NOT_FOUND =>
            {
                self.content_file_variant_session.transport_ok();
                ManifestState::None
            }
            Err(e) => {
                let reason = describe_fetch_error(&e);
                self.content_file_variant_session.transport_failed(&reason);
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
        let session = &self.content_file_variant_session;
        let config = &self.content_file_variants_config;
        if config.is_off() {
            return None;
        }
        let (scene, content_path) = (ipfs_path.scene_hash()?, ContentFilePathView::of(ipfs_path)?);
        let file = content_path.as_str();
        if !could_have_variant(content_path) {
            return None;
        }
        let mut manifest = self.content_file_variant_manifests.get(scene)?;
        let state = manifest.borrow().clone();
        let state = if matches!(state, ManifestState::Pending) {
            // the base first, which settles once the flags are read, then the manifest, but
            // only until its deadline: a slow service must not hold the entity's files back
            let (_, settled_at) = config.settled().await?;
            let left = self
                .content_file_variant_manifests
                .wait_left(scene, settled_at);
            let arrived = if left.is_zero() {
                None
            } else {
                platform::within(left, async {
                    manifest
                        .wait_for(|state| !matches!(state, ManifestState::Pending))
                        .await
                        .ok()
                        .map(|state| state.clone())
                })
                .await
            };
            match arrived {
                Some(state) => state?,
                None => {
                    session.fail(
                        scene,
                        file,
                        None,
                        format!(
                            "the manifest was not there within {}s",
                            MANIFEST_DEADLINE.as_secs()
                        ),
                    );
                    return None;
                }
            }
        } else {
            state
        };
        let ManifestState::Loaded(manifest) = state else {
            return None;
        };
        let base_url = config.base_url()?;
        let original = self
            .context
            .read()
            .await
            .entities
            .get(scene)?
            .collection
            .hash(file)?
            .into_owned();
        let content_hash = ContentHashView::try_from(original.as_str()).ok()?;
        let (variant, payload_kind, url) =
            match self.fetchable_variant(base_url.view(), &manifest, content_hash, content_path) {
                Ok(chosen) => chosen,
                Err(reason) => {
                    if let Some(reason) = reason {
                        session.fail(scene, file, None, reason);
                    }
                    return None;
                }
            };
        // the manifest names no hash for a variant, so its bytes are cached under a digest of
        // the url they came from, never under a content hash
        let key = CacheKey::Url(content_hash::url_cache_name(&url));
        let served = ServedVariant {
            scene: scene.to_owned(),
            file: file.to_owned(),
            variant: variant.path.clone(),
            format: variant.format.clone(),
            url: url.clone(),
        };

        #[cfg(not(target_arch = "wasm32"))]
        let cached = self.read_disk_cache(&key).await.ok().flatten();
        #[cfg(target_arch = "wasm32")]
        let cached = crate::web_cache::read(&url).await;
        let (fetched, from_cache) = match cached {
            Some(bytes) => (SharedBytes(Arc::new(bytes)), true),
            None => {
                let token = self.reqno.fetch_add(1, Ordering::SeqCst);
                let (download, task) = self.share_download_with(
                    key,
                    token,
                    path.to_owned(),
                    url.clone(),
                    Some(scene.to_owned()),
                    VARIANT_FETCH,
                );
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
                    Some(Ok(bytes)) => {
                        session.transport_ok();
                        (bytes, false)
                    }
                    Some(Err(AssetReaderError::Io(e))) if e.kind() == ErrorKind::Interrupted => {
                        return None
                    }
                    Some(Err(e)) => {
                        let reason = e.to_string();
                        session.transport_failed(&reason);
                        session.fail(scene, file, Some(&variant), format!("fetch: {reason}"));
                        return None;
                    }
                    None => return None,
                }
            }
        };

        // checks that need no decoding; a copy that fails them is dropped from the cache too,
        // so a later session does not read it again
        let rejected = match payload_kind {
            // the loaders unwrap the KTX2 (`ktx2::bc_ktx2_dds`), inside the asset load; here
            // only its header is read, so a file that is not a BC KTX2 at all falls back
            // before it reaches them
            ContentFilePayloadKind::BcTexture => ktx2::bc_format(&fetched)
                .err()
                .map(|reason| format!("invalid KTX2: {reason}")),
            // the browser hands over the decoded bytes, which are the original `.glb` byte for
            // byte: checked against the original's own content hash, a truncated or corrupt
            // copy falls back here, and one that passes is the original, so a glTF loader
            // failure on it is the original's own and needs no reload
            #[cfg(target_arch = "wasm32")]
            ContentFilePayloadKind::BrotliGlb => {
                (!content_hash::matches_content_hash(&original, &fetched)).then(|| {
                    format!(
                        "{} bytes from the Brotli copy are not the original `{original}`",
                        fetched.len()
                    )
                })
            }
            // the glTF loader decodes each embedded KTX2 through the host transcoder, which
            // unwraps BC to DDS inside the asset load; here only the GLB magic is read. An image
            // that does not unwrap fails the `Gltf` load, which reloads the original
            // (`fall_back_from_failed_variants::<Gltf>`)
            ContentFilePayloadKind::BcGlb => (!fetched.starts_with(b"glTF"))
                .then(|| "invalid glb variant: not a binary glTF".to_owned()),
        };
        if let Some(reason) = rejected {
            self.forget_variant(&served.url).await;
            session.fail(scene, file, Some(&variant), reason);
            return None;
        }
        let bytes = Arc::try_unwrap(fetched.0).unwrap_or_else(|shared| shared.as_ref().clone());
        session.served(path, served, from_cache);
        Some(VariantRead {
            bytes,
            cached: from_cache,
        })
    }
}

/// Once the flags are read, settles a base pending on [`ENDPOINT_FLAG`] where the flag says.
fn route_by_endpoint_flag(flags: Res<FeatureFlags>, ipfs: Res<IpfsResource>) {
    if flags.source() == FlagsSource::Pending {
        return;
    }
    let config = &ipfs.content_file_variants_config;
    let Some((fallback, fallback_source)) = config.pending_fallback() else {
        return;
    };
    let fallback = fallback.as_ref().map(|base_url| base_url.view().as_str());
    let (base, source) = flagged_base(flags.variant(ENDPOINT_FLAG).as_ref(), fallback);
    config.settle(
        base.as_deref(),
        &format!("{source}; default: {fallback_source}"),
    );
}

/// Settles a variants base pending on [`ENDPOINT_FLAG`] once the flags are read.
pub(crate) struct EndpointFlagRoutingPlugin;

impl Plugin for EndpointFlagRoutingPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            route_by_endpoint_flag
                .run_if(resource_exists::<IpfsResource>.and(resource_exists::<FeatureFlags>)),
        );
    }
}

pub(crate) struct BevyVariantsPlugin;

impl Plugin for BevyVariantsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, detect_gpu_formats);
        app.add_plugins(EndpointFlagRoutingPlugin);
        app.add_plugins(debug_panel::BevyVariantsDebugPanelPlugin);
    }

    /// After the asset plugins exist: an app without images or glTFs has nothing to fall back.
    fn finish(&self, app: &mut App) {
        if app
            .world()
            .contains_resource::<Events<AssetLoadFailedEvent<Image>>>()
        {
            // after the cache check, which skips a variant's failure only while it is marked served
            app.add_systems(
                Update,
                fall_back_from_failed_variants::<Image>
                    .after(crate::cache_heal::drop_failed_cached_entries),
            );
        }
        if app
            .world()
            .contains_resource::<Events<AssetLoadFailedEvent<Gltf>>>()
        {
            app.add_systems(
                Update,
                (
                    fall_back_from_failed_variants::<Gltf>
                        .after(crate::cache_heal::drop_failed_cached_entries),
                    finish_model_reloads.after(fall_back_from_failed_variants::<Gltf>),
                ),
            );
        }
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
    ipfs.content_file_variants_config.init_bc(bc);
}

/// An asset type a variant can fail to load as, and the variant formats whose failure reloads
/// it from the original.
trait VariantAsset: Asset {
    const FORMATS: &'static [&'static str];
}

impl VariantAsset for Image {
    const FORMATS: &'static [&'static str] = &["ktx2-bc"];
}

/// Only `glb`: a `glb-br` variant is the original byte for byte, so a glTF loader failure on
/// it is the original's own and a reload would fail the same way. A Draco original fails on the
/// web through its `glb-br` copy too, and isn't repaired: image processing skips paths read
/// from a variant, and the copy stays served. abgen decodes Draco before it builds a `glb`
/// variant, so this only happens where the `glb` variant isn't taken (no BC on the GPU) or has
/// failed.
impl VariantAsset for Gltf {
    const FORMATS: &'static [&'static str] = &["glb"];
}

/// Reloads from its original each texture or `glb` model whose variant failed to load. A GLB
/// that never loaded reloads whole; a scene's container retries it (`retry_repaired_gltfs`),
/// and an avatar waits for it (`bevy_variant_model_reload_pending`).
fn fall_back_from_failed_variants<A: VariantAsset>(
    mut failed: EventReader<AssetLoadFailedEvent<A>>,
    ipfs: Res<IpfsResource>,
    server: Res<AssetServer>,
) {
    for event in failed.read() {
        match ipfs.content_file_variant_session.decode_failed(
            event.path.path(),
            &event.error.to_string(),
            A::FORMATS,
        ) {
            Some(url) => {
                ipfs.content_file_variant_session
                    .reload_started(event.path.path());
                server.reload(event.path.without_label().into_owned());
                let io = ipfs.clone();
                IoTaskPool::get()
                    .spawn_compat(async move { io.forget_variant(&url).await })
                    .detach();
            }
            // the original's own failure: a reload from it is over
            None => ipfs
                .content_file_variant_session
                .reload_finished(event.path.path()),
        }
    }
}

/// Ends the wait on a model reloading from its original once it has loaded.
fn finish_model_reloads(
    mut events: EventReader<AssetEvent<Gltf>>,
    ipfs: Res<IpfsResource>,
    server: Res<AssetServer>,
) {
    for event in events.read() {
        if let AssetEvent::LoadedWithDependencies { id } = event {
            if let Some(path) = server.get_path(*id) {
                ipfs.content_file_variant_session
                    .reload_finished(path.path());
            }
        }
    }
}

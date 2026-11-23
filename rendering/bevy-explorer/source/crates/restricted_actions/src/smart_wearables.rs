//! Smart wearables: equipped (or previewed, or console-run) wearables whose content carries an
//! SDK7 scene, run as portable experiences.
//!
//! The plugin follows the primary user's profile and the realm's `/preview-wearables` (restarting a
//! preview wearable on the new files when the preview server reports its project saved), decides
//! from the wearable's data whether it has a scene for the player's body shape, and keeps the
//! smart-wearable entries of [`PortableScenes`] equal to the wearables that should run: smart and
//! not stopped. The scene itself loads like any other portable, from the wearable's
//! `{shape}.smart_wearable` content file ([`collectibles::smart_wearables`]); its permission
//! requests, failures and the parcel scene's `portableExperiences` toggle are handled as for any
//! portable.
//!
//! The decisions live in [`Model`], plain data with no Bevy world, so they are unit tested
//! directly; the systems feed it observations and apply its output.

mod diagnostics;

use std::{
    collections::{BTreeMap, BTreeSet},
    hash::{DefaultHasher, Hash, Hasher},
};

use avatar::AvatarShape;
use bevy::{
    platform::collections::{HashMap, HashSet},
    prelude::*,
    tasks::{IoTaskPool, Task},
};
use bevy_console::ConsoleCommand;
use collectibles::{
    request_collectibles,
    smart_wearables::{scene_file, scene_id, scene_json_key, with_scene_aliases},
    urn::PREVIEW_COLLECTION,
    wearables::{
        inline_wearable, Wearable, WearableCategory, WearableData, WearableMeta,
        WearableRepresentation, WearableUrn,
    },
    CollectibleError, CollectibleManager,
};
use common::{
    sets::SceneSets,
    structs::{server_mode, CurrentRealm, NoRenderApp, PreviewCommand, PrimaryUser},
    util::{TaskCompat, TaskExt},
};
use comms::profile::UserProfile;
use console::DoAddConsoleCommand;
use ipfs::{ContentMap, IpfsAssetServer, IpfsModifier, SceneIpfsLocation};
use reqwest::StatusCode;
use scene_runner::initialize_scene::{PortableKind, PortableScenes, PortableSource};
use serde::Deserialize;
use system_bridge::{PortableState, SmartWearableSource};

pub struct SmartWearablesPlugin;

impl Plugin for SmartWearablesPlugin {
    fn build(&self, app: &mut App) {
        // the portables listing reads this everywhere, so it exists even where it stays empty
        app.init_resource::<SmartWearables>();
        // a headless app loads no wearables (no wearable collectibles, no smart-wearable scene
        // loader), so it has none to run
        if app.world().contains_resource::<NoRenderApp>() {
            return;
        }
        // `reconcile` writes `PortableScenes`, which the scene lifecycle reads in `Init`
        app.add_systems(
            Update,
            (
                observe_equipped,
                preview_wearables,
                resolve_entries,
                reconcile,
                wear_previews,
            )
                .chain()
                .in_set(SceneSets::RestrictedActions)
                .after(request_collectibles::<Wearable>)
                .run_if(|| !server_mode()),
        );
        app.add_console_command::<SmartWearableCommand, _>(smart_wearable_command);
        app.add_plugins(diagnostics::SmartWearablesDiagnosticsPlugin);
    }
}

const MALE: &str = "male";
const FEMALE: &str = "female";

/// The key every smart wearable state is held under: a parsed wearable urn (lowercase, without a
/// token id), else the lowercased string (a preview wearable's id need not be a catalyst urn: a
/// creator preview lists a `urn:` uuid, which must not become a bare-name urn). Parsed quietly:
/// this runs on every profile change and command, and an off-chain preview id is not a failure
/// worth a log line.
pub fn normalize_urn(urn: &str) -> String {
    let urn = urn.trim();
    if !urn.contains(':') {
        return urn.to_lowercase();
    }
    WearableUrn::new_quiet(urn)
        .map(String::from)
        .unwrap_or_else(|_| urn.to_lowercase())
}

/// A wearable listed by the realm's `/preview-wearables`.
#[derive(Clone, Debug)]
pub struct PreviewWearable {
    pub urn: String,
    pub name: String,
    pub content: ContentMap,
    pub base_url: String,
    /// the wearable it defines, for the local avatar to wear
    pub data: Option<Box<WearableData>>,
}

/// Why the engine runs a wearable it may not have equipped.
#[derive(Clone, Debug)]
pub enum ForcedSource {
    Console,
    Preview(PreviewWearable),
}

impl ForcedSource {
    fn source(&self) -> SmartWearableSource {
        match self {
            ForcedSource::Console => SmartWearableSource::Console,
            ForcedSource::Preview(_) => SmartWearableSource::Preview,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Phase {
    /// waiting for the wearable's data
    Loading,
    /// no scene for the body shape
    NotSmart,
    /// the wearable could not be resolved
    Failed(String),
    /// the scene loads from `{shape}.smart_wearable` of this collection
    Ready { collection: String },
}

#[derive(Clone, Debug)]
pub struct ModelEntry {
    pub source: SmartWearableSource,
    pub shape: &'static str,
    pub name: String,
    pub phase: Phase,
}

impl ModelEntry {
    pub fn scene_id(&self) -> Option<String> {
        match &self.phase {
            Phase::Ready { collection } => Some(scene_id(collection, self.shape)),
            _ => None,
        }
    }
}

/// What the profile says, normalized: the equipped wearables and the body shape.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EquippedSet {
    pub urns: BTreeSet<String>,
    pub shape: &'static str,
}

/// Every smart wearable decision, over plain data.
#[derive(Debug, Default)]
pub struct Model {
    pub entries: BTreeMap<String, ModelEntry>,
    /// stopped while still held (equipped, previewed or run): not started again until restarted
    pub stopped: HashSet<String>,
    pub forced: HashMap<String, ForcedSource>,
    /// the profile last seen, `None` while logged out
    pub equipped: Option<EquippedSet>,
}

impl Model {
    pub fn shape(&self) -> &'static str {
        self.equipped.as_ref().map(|e| e.shape).unwrap_or(MALE)
    }

    /// Feed the primary user's profile (`None`: logged out). Returns whether anything changed.
    pub fn observe_profile(&mut self, equipped: Option<EquippedSet>) -> bool {
        if self.equipped == equipped {
            return false;
        }
        if equipped.is_none() {
            // logout: forget the session's stops; console and preview runs stay
            self.stopped.clear();
        }
        self.equipped = equipped;
        self.sync_entries();
        true
    }

    /// The wearables that should have an entry, with the source each is held for.
    fn desired(&self) -> BTreeMap<String, SmartWearableSource> {
        let mut desired = BTreeMap::new();
        for (urn, forced) in self.forced.iter() {
            desired.insert(urn.clone(), forced.source());
        }
        if let Some(equipped) = &self.equipped {
            for urn in equipped.urns.iter() {
                desired.insert(urn.clone(), SmartWearableSource::Equipped);
            }
        }
        desired
    }

    /// Bring the entries in line with the equipped and forced sets: a wearable that left both is
    /// forgotten with its stop (re-equipping starts over), a body-shape change re-resolves, a new
    /// one starts loading.
    pub fn sync_entries(&mut self) {
        let desired = self.desired();
        let shape = self.shape();

        let gone: Vec<String> = self
            .entries
            .keys()
            .filter(|urn| !desired.contains_key(*urn))
            .cloned()
            .collect();
        for urn in gone {
            self.entries.remove(&urn);
            self.stopped.remove(&urn);
        }

        for (urn, source) in desired {
            let entry = self
                .entries
                .entry(urn.clone())
                .or_insert_with(|| ModelEntry {
                    source,
                    shape,
                    name: urn.clone(),
                    phase: Phase::Loading,
                });
            entry.source = source;
            if entry.shape != shape {
                entry.shape = shape;
                entry.phase = Phase::Loading;
            }
        }
    }

    pub fn set_phase(&mut self, urn: &str, phase: Phase) {
        if let Some(entry) = self.entries.get_mut(urn) {
            entry.phase = phase;
        }
    }

    pub fn set_name(&mut self, urn: &str, name: &str) {
        if let Some(entry) = self.entries.get_mut(urn) {
            entry.name = name.to_owned();
        }
    }

    /// An entry's state, `None` for one with no scene. `scene` gives the state of a scene that
    /// should be running, by its id.
    pub fn state_of(
        &self,
        urn: &str,
        scene: impl Fn(&str) -> (PortableState, Option<String>),
    ) -> Option<(PortableState, Option<String>)> {
        let entry = self.entries.get(urn)?;
        match &entry.phase {
            Phase::Loading => Some((PortableState::Loading, None)),
            Phase::NotSmart => None,
            Phase::Failed(reason) => Some((PortableState::Failed, Some(reason.clone()))),
            Phase::Ready { .. } if self.stopped.contains(urn) => {
                Some((PortableState::Stopped, None))
            }
            Phase::Ready { .. } => entry.scene_id().map(|id| scene(&id)),
        }
    }

    /// The scenes that should run: scene id -> urn.
    pub fn effective(&self) -> BTreeMap<String, String> {
        self.entries
            .iter()
            .filter(|(urn, _)| !self.stopped.contains(*urn))
            .filter_map(|(urn, entry)| Some((entry.scene_id()?, urn.clone())))
            .collect()
    }

    /// The portable to run for `urn`, if it should run.
    pub fn portable(&self, urn: &str) -> Option<(String, PortableSource)> {
        let entry = self.entries.get(urn)?;
        let Phase::Ready { collection } = &entry.phase else {
            return None;
        };
        Some((
            scene_id(collection, entry.shape),
            PortableSource {
                pid: urn.to_owned(),
                parent_scene: None,
                ens: None,
                super_user: false,
                kind: PortableKind::SmartWearable {
                    name: entry.name.clone(),
                    location: SceneIpfsLocation::ContentFile {
                        hash: collection.clone(),
                        file: scene_file(entry.shape),
                    },
                },
            },
        ))
    }

    /// `/smart_wearable run`: run without equipping.
    pub fn run(&mut self, urn: String) {
        self.stopped.remove(&urn);
        self.forced.entry(urn).or_insert(ForcedSource::Console);
        self.sync_entries();
    }

    /// Stop a smart wearable that is loading or has a scene: false when there is nothing to stop.
    pub fn stop(&mut self, urn: &str) -> bool {
        let stoppable = matches!(
            self.entries.get(urn).map(|e| &e.phase),
            Some(Phase::Loading | Phase::Ready { .. })
        );
        stoppable && self.stopped.insert(urn.to_owned())
    }

    /// Clear a stop, or retry a failure.
    pub fn restart(&mut self, urn: &str) -> bool {
        let Some(entry) = self.entries.get_mut(urn) else {
            return false;
        };
        if matches!(entry.phase, Phase::Failed(_)) {
            entry.phase = Phase::Loading;
        }
        self.stopped.remove(urn);
        true
    }

    /// Replace the realm's preview wearables.
    pub fn set_previews(&mut self, previews: Vec<PreviewWearable>) {
        self.forced
            .retain(|_, forced| !matches!(forced, ForcedSource::Preview(_)));
        for preview in previews {
            self.insert_preview(preview);
        }
        self.sync_entries();
    }

    /// A save of a previewed wearable's project: the new files replace the preview's and it runs
    /// again, stopped or not.
    pub fn reload_preview(&mut self, preview: PreviewWearable) {
        self.stopped.remove(&preview.urn);
        self.insert_preview(preview);
        self.sync_entries();
    }

    /// Hold `preview` for its urn, resolved afresh; a console run of the same urn becomes the
    /// preview.
    fn insert_preview(&mut self, preview: PreviewWearable) {
        self.entries.remove(&preview.urn);
        self.forced
            .insert(preview.urn.clone(), ForcedSource::Preview(preview));
    }

    pub fn preview_count(&self) -> usize {
        self.forced
            .values()
            .filter(|f| matches!(f, ForcedSource::Preview(_)))
            .count()
    }
}

#[derive(Deserialize)]
struct PreviewDocument {
    #[serde(default)]
    data: Vec<PreviewDocumentWearable>,
}

/// A catalyst, e2e realm or creator preview (`dcl-scene-cli start`, `sdk-commands start`) entry
/// nests the content under `data.representations`, with keys under the body-shape folders; an
/// entry may instead list unprefixed `contents` at the top level with a `baseUrl`, and then its
/// `data` is not read for content.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PreviewDocumentWearable {
    id: String,
    name: Option<String>,
    base_url: Option<String>,
    contents: Option<Vec<PreviewDocumentContent>>,
    #[serde(default)]
    data: serde_json::Value,
}

/// The `{ key, url, hash }` entries of `data.representations[*].contents`, each under the folder
/// of every body shape its representation's `bodyShapes` names, in place of the folder its key
/// has: a creator preview lists every file under `male/` in each representation, whichever bodies
/// it is for. Entries of a representation naming no body shape keep their keys; anything else
/// there is ignored.
fn representation_contents(data: &serde_json::Value) -> Vec<PreviewDocumentContent> {
    let mut contents = Vec::new();
    let representations = data
        .get("representations")
        .and_then(serde_json::Value::as_array);
    for representation in representations.into_iter().flatten() {
        let shapes: Vec<&str> = representation
            .get("bodyShapes")
            .and_then(serde_json::Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|urn| body_shape_folder(urn.as_str()?))
            .collect();
        let entries = representation
            .get("contents")
            .and_then(serde_json::Value::as_array);
        for entry in entries.into_iter().flatten() {
            let Ok(entry) = serde_json::from_value::<PreviewDocumentContent>(entry.clone()) else {
                continue;
            };
            if shapes.is_empty() {
                contents.push(entry);
                continue;
            }
            let file = unshaped(&entry.key);
            for shape in shapes.iter() {
                contents.push(PreviewDocumentContent {
                    key: format!("{shape}/{file}"),
                    url: entry.url.clone(),
                    hash: entry.hash.clone(),
                });
            }
        }
    }
    contents
}

/// `key` without its body-shape folder.
fn unshaped(key: &str) -> &str {
    [MALE, FEMALE]
        .iter()
        .find_map(|shape| key.strip_prefix(&format!("{shape}/")))
        .unwrap_or(key)
}

/// The wearable a preview entry defines: its `data` with one representation for each body shape a
/// representation names, its files under that body's folder as [`representation_contents`] keys
/// them. `None` without a category or a representation for a body shape.
fn preview_definition(data: &serde_json::Value) -> Option<WearableData> {
    let category: WearableCategory = serde_json::from_value(data.get("category")?.clone()).ok()?;
    let categories = |value: &serde_json::Value, field: &str| -> Vec<WearableCategory> {
        value
            .get(field)
            .and_then(serde_json::Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|category| serde_json::from_value(category.clone()).ok())
            .collect()
    };
    let mut representations = Vec::new();
    let listed = data
        .get("representations")
        .and_then(serde_json::Value::as_array);
    for representation in listed.into_iter().flatten() {
        let Some(main_file) = representation
            .get("mainFile")
            .and_then(serde_json::Value::as_str)
        else {
            continue;
        };
        let files: Vec<&str> = representation
            .get("contents")
            .and_then(serde_json::Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|entry| entry.get("key")?.as_str())
            .map(unshaped)
            .collect();
        let body_shapes = representation
            .get("bodyShapes")
            .and_then(serde_json::Value::as_array);
        for urn in body_shapes
            .into_iter()
            .flatten()
            .filter_map(serde_json::Value::as_str)
        {
            let Some(shape) = body_shape_folder(urn) else {
                continue;
            };
            representations.push(WearableRepresentation {
                body_shapes: vec![urn.to_owned()],
                main_file: format!("{shape}/{}", unshaped(main_file)),
                override_replaces: categories(representation, "overrideReplaces"),
                override_hides: categories(representation, "overrideHides"),
                contents: files.iter().map(|file| format!("{shape}/{file}")).collect(),
            });
        }
    }
    (!representations.is_empty()).then(|| WearableData {
        tags: Vec::new(),
        category,
        representations,
        hides: categories(data, "hides"),
        replaces: categories(data, "replaces"),
        removes_default_hiding: None,
        outline_compatible: None,
        spring_bones: None,
    })
}

/// The content folder of a body shape urn (`...:BaseMale`, `...:BaseFemale`).
fn body_shape_folder(urn: &str) -> Option<&'static str> {
    match urn.rsplit(':').next()?.to_ascii_lowercase().as_str() {
        "basemale" => Some(MALE),
        "basefemale" => Some(FEMALE),
        _ => None,
    }
}

#[derive(Deserialize)]
struct PreviewDocumentContent {
    key: String,
    url: String,
    hash: String,
}

/// Parse a realm's `/preview-wearables` reply, in either shape ([`PreviewDocumentWearable`]). A
/// wearable with no `baseUrl` whose content urls do not end in their hash is skipped (its base
/// url is unknown).
pub fn parse_preview_wearables(body: &[u8]) -> Result<Vec<PreviewWearable>, String> {
    let document: PreviewDocument =
        serde_json::from_slice(body).map_err(|e| format!("invalid preview-wearables: {e}"))?;
    Ok(document
        .data
        .into_iter()
        .filter_map(|wearable| {
            let contents = match wearable.contents {
                Some(contents) => contents,
                None => representation_contents(&wearable.data),
            };
            // the ipfs modifier is the prefix a hash is appended to
            let base_url = wearable
                .base_url
                .map(|base| {
                    if base.ends_with('/') {
                        base
                    } else {
                        format!("{base}/")
                    }
                })
                .or_else(|| {
                    contents
                        .iter()
                        .find_map(|c| c.url.strip_suffix(&c.hash))
                        .map(ToOwned::to_owned)
                });
            let Some(base_url) = base_url else {
                debug!("preview wearable {} has no content base url", wearable.id);
                return None;
            };
            let urn = normalize_urn(&wearable.id);
            Some(PreviewWearable {
                name: wearable.name.unwrap_or_else(|| wearable.id.clone()),
                urn,
                content: ContentMap::from_content(contents.into_iter().map(|c| (c.key, c.hash))),
                base_url,
                data: preview_definition(&wearable.data).map(Box::new),
            })
        })
        .collect())
}

/// A digest of a preview's whole content: a preview whose files changed gets a new one.
fn content_digest(content: &ContentMap) -> String {
    let mut files: Vec<(&String, &String)> = content.values().collect();
    files.sort();
    let mut hasher = DefaultHasher::new();
    files.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

/// The collection a preview's content is registered under.
fn content_collection(content: &ContentMap) -> String {
    format!("{}-preview", content_digest(content))
}

/// The collection a preview's scene for `shape` loads from, `None` when it has no scene for that
/// body. Named after the whole content, so a preview whose files changed is a new collection and
/// its scene a new scene.
pub fn preview_collection(content: &ContentMap, shape: &str) -> Option<String> {
    scene_json_key(content.files(), shape)?;
    Some(content_collection(content))
}

/// The collectible urn a preview's wearable is registered under, in [`PREVIEW_COLLECTION`] and
/// named after the whole content, so a save that changes its files makes a new collectible.
pub fn preview_wearable_urn(content: &ContentMap) -> String {
    format!("{PREVIEW_COLLECTION}:{}", content_digest(content))
}

fn is_preview_wearable(urn: &str) -> bool {
    urn.strip_prefix(PREVIEW_COLLECTION)
        .is_some_and(|rest| rest.starts_with(':'))
}

/// The wearables the local avatar shows: `previews` first, so each takes its category's slot from
/// whatever `worn` has there, then `worn` without any earlier preview wearable.
pub fn with_previews(worn: &[String], previews: &[String]) -> Vec<String> {
    previews
        .iter()
        .chain(worn.iter().filter(|urn| !is_preview_wearable(urn)))
        .cloned()
        .collect()
}

/// The realm's `/preview-wearables/{scene id}` for a preview reload: the preview server keys the
/// route on the id it sends when a project is saved.
pub fn preview_reload_url(about_url: &str, command: &PreviewCommand) -> Option<String> {
    let scene = match command {
        PreviewCommand::ReloadScene { hash } => hash,
        PreviewCommand::ReloadModel { scene_id, .. }
        | PreviewCommand::RemoveModel { scene_id, .. } => scene_id,
    };
    Some(format!(
        "{}/preview-wearables/{scene}",
        realm_base(about_url)?
    ))
}

fn realm_base(about_url: &str) -> Option<&str> {
    let base = about_url
        .strip_suffix("/about")
        .unwrap_or(about_url)
        .trim_end_matches('/');
    base.starts_with("http").then_some(base)
}

/// The plugin's state: the [`Model`] plus what cannot be plain data.
#[derive(Resource, Default)]
pub struct SmartWearables {
    pub model: Model,
    preview_task: Option<Task<Result<Vec<PreviewWearable>, String>>>,
    reload_tasks: Vec<Task<Result<Vec<PreviewWearable>, String>>>,
    /// preview collectible urns registered with the wearables manager, and whether that worked
    dressed: HashMap<String, bool>,
}

fn equipped_set(profile: &UserProfile) -> EquippedSet {
    EquippedSet {
        urns: profile
            .content
            .avatar
            .wearables
            .iter()
            // base wearables never carry a scene
            .filter(|urn| {
                WearableUrn::new_quiet(urn)
                    .ok()
                    .is_none_or(|urn| urn.collection_type() != Some("base-avatars"))
            })
            .map(|urn| normalize_urn(urn))
            .collect(),
        shape: if profile.is_female() { FEMALE } else { MALE },
    }
}

fn observe_equipped(
    mut sw: ResMut<SmartWearables>,
    player: Query<Option<Ref<UserProfile>>, With<PrimaryUser>>,
) {
    let profile = player.single().ok().flatten();
    let present = profile.is_some();
    let changed = profile.as_ref().is_some_and(|p| p.is_changed());
    if present == sw.model.equipped.is_some() && !changed {
        return;
    }
    let equipped = profile.map(|p| equipped_set(&p));
    sw.model.observe_profile(equipped);
}

fn fetch_previews(
    client: reqwest::Client,
    url: String,
) -> Task<Result<Vec<PreviewWearable>, String>> {
    IoTaskPool::get().spawn_compat(async move {
        let response = client.get(&url).send().await.map_err(|e| e.to_string())?;
        if response.status() == StatusCode::NOT_FOUND {
            return Ok(Vec::new());
        }
        if !response.status().is_success() {
            return Err(format!("status {}", response.status()));
        }
        let body = response.bytes().await.map_err(|e| e.to_string())?;
        parse_preview_wearables(&body)
    })
}

/// Follow the realm's preview wearables, and restart the one whose project a preview reload names.
fn preview_wearables(
    mut sw: ResMut<SmartWearables>,
    realm: Res<CurrentRealm>,
    ipfas: IpfsAssetServer,
    mut reloads: EventReader<PreviewCommand>,
) {
    if realm.is_changed() {
        sw.model.set_previews(Vec::new());
        sw.preview_task = None;
        sw.reload_tasks.clear();
        if let Some(base) = realm_base(&realm.about_url) {
            let url = format!("{base}/preview-wearables");
            sw.preview_task = Some(fetch_previews(ipfas.ipfs().client(), url));
        }
    }

    for command in reloads.read() {
        if sw.model.preview_count() == 0 {
            continue;
        }
        if let Some(url) = preview_reload_url(&realm.about_url, command) {
            let task = fetch_previews(ipfas.ipfs().client(), url);
            sw.reload_tasks.push(task);
        }
    }
    let mut reloaded = Vec::new();
    sw.reload_tasks.retain_mut(|task| match task.complete() {
        Some(result) => {
            reloaded.push(result);
            false
        }
        None => true,
    });
    for result in reloaded {
        match result {
            Ok(previews) => {
                for preview in previews {
                    info!("reloading preview smart wearable {}", preview.urn);
                    sw.model.reload_preview(preview);
                }
            }
            Err(e) => warn!("preview smart wearable reload failed: {e}"),
        }
    }

    if sw.preview_task.is_none() {
        return;
    }
    let Some(result) = sw.preview_task.as_mut().and_then(|task| task.complete()) else {
        return;
    };
    sw.preview_task = None;
    match result {
        Ok(previews) => {
            if !previews.is_empty() {
                info!("realm previews {} smart wearable(s)", previews.len());
            }
            sw.model.set_previews(previews);
        }
        Err(e) => debug!("no preview wearables: {e}"),
    }
}

/// Decide each loading entry from the wearable's data: whether it has a scene for the body
/// shape, and which collection the scene loads from. Reads nothing itself: the scene.json is read
/// when the scene loads.
fn resolve_entries(
    mut sw: ResMut<SmartWearables>,
    mut wearables: CollectibleManager<Wearable>,
    ipfas: IpfsAssetServer,
) {
    let loading: Vec<(String, &'static str)> = sw
        .model
        .entries
        .iter()
        .filter(|(_, e)| e.phase == Phase::Loading)
        .map(|(urn, e)| (urn.clone(), e.shape))
        .collect();
    for (urn, shape) in loading {
        let preview = match sw.model.forced.get(&urn) {
            Some(ForcedSource::Preview(preview)) => Some(preview.clone()),
            _ => None,
        };
        let phase = match preview {
            Some(preview) => {
                sw.model.set_name(&urn, &preview.name);
                match preview_collection(&preview.content, shape) {
                    None => Phase::NotSmart,
                    Some(collection) => {
                        ipfas.ipfs().add_collection(
                            collection.clone(),
                            with_scene_aliases(preview.content),
                            Some(IpfsModifier {
                                base_url: Some(preview.base_url),
                            }),
                            None,
                        );
                        Phase::Ready { collection }
                    }
                }
            }
            // equipped and console runs resolve through the catalyst
            None => {
                let Ok(wearable_urn) = WearableUrn::new(&urn) else {
                    sw.model
                        .set_phase(&urn, Phase::Failed("invalid wearable urn".to_owned()));
                    continue;
                };
                let data = match wearables.get_data(&wearable_urn) {
                    Err(CollectibleError::Loading) => continue,
                    Err(e) => {
                        sw.model
                            .set_phase(&urn, Phase::Failed(format!("wearable {e:?}")));
                        continue;
                    }
                    Ok(data) => data,
                };
                let name = data.name.clone();
                let smart = scene_json_key(&data.extra_data.scene_json_files, shape).is_some();
                sw.model.set_name(&urn, &name);
                if !smart {
                    Phase::NotSmart
                } else {
                    // the pointer's hash, not `CollectibleData::hash`: the data loader reads the
                    // entity through a content-file path and leaves that field empty
                    match wearables.get_hash(&wearable_urn) {
                        Ok(collection) => Phase::Ready { collection },
                        Err(_) => Phase::Failed("wearable hash unavailable".to_owned()),
                    }
                }
            }
        };
        if let Phase::Ready { collection } = &phase {
            info!(
                "smart wearable {urn} ready as {}",
                scene_id(collection, shape)
            );
        }
        sw.model.set_phase(&urn, phase);
    }
}

/// Dress the local avatar in the realm's preview wearables, for this client's rendering alone: each
/// is registered as a collectible from its listing, never looked up on a catalyst, and worn ahead
/// of the profile's wearables. The profile, what other players see and what is deployed keep the
/// wearables the player has.
fn wear_previews(
    mut sw: ResMut<SmartWearables>,
    mut wearables: CollectibleManager<Wearable>,
    mut avatars: Query<&mut AvatarShape, With<PrimaryUser>>,
    ipfas: IpfsAssetServer,
) {
    let sw = &mut *sw;
    let mut worn = Vec::new();
    for forced in sw.model.forced.values() {
        let ForcedSource::Preview(preview) = forced else {
            continue;
        };
        let Some(data) = &preview.data else {
            continue;
        };
        let urn = preview_wearable_urn(&preview.content);
        let registered = *sw.dressed.entry(urn.clone()).or_insert_with(|| {
            let Ok(collectible_urn) = WearableUrn::new(&urn) else {
                return false;
            };
            let collection = content_collection(&preview.content);
            ipfas.ipfs().add_collection(
                collection.clone(),
                with_scene_aliases(preview.content.clone()),
                Some(IpfsModifier {
                    base_url: Some(preview.base_url.clone()),
                }),
                None,
            );
            let meta = WearableMeta {
                id: preview.urn.clone(),
                name: Some(preview.name.clone()),
                description: None,
                thumbnail: String::new(),
                rarity: None,
                data: WearableData::clone(data),
            };
            match inline_wearable(
                meta,
                &preview.content,
                &collection,
                urn.clone(),
                ipfas.asset_server(),
            ) {
                Ok(collectible) => {
                    info!(
                        "the local avatar wears preview wearable {} as {urn}",
                        preview.urn
                    );
                    wearables.add_builtin(collectible_urn, collectible);
                    true
                }
                Err(e) => {
                    warn!("preview wearable {} cannot be worn: {e}", preview.urn);
                    false
                }
            }
        });
        if registered {
            worn.push(urn);
        }
    }
    worn.sort();
    for mut avatar in avatars.iter_mut() {
        let next = with_previews(&avatar.0.wearables, &worn);
        if next != avatar.0.wearables {
            avatar.0.wearables = next;
        }
    }
}

/// Keep the smart-wearable entries of `PortableScenes` equal to the scenes that should run. A
/// stop from a scene (`kill`, `exit`) or the console is a [`crate::portables::StopPortable`]
/// event, whose handler removes the scene and records the wearable as stopped.
fn reconcile(mut commands: Commands, sw: Res<SmartWearables>, portables: Res<PortableScenes>) {
    let effective = sw.model.effective();
    let running: HashSet<&String> = portables
        .iter()
        .filter(|(_, source)| source.is_smart_wearable())
        .map(|(hash, _)| hash)
        .collect();

    let remove: Vec<String> = running
        .iter()
        .filter(|hash| !effective.contains_key(**hash))
        .map(|hash| (*hash).clone())
        .collect();
    let insert: Vec<(String, PortableSource)> = effective
        .iter()
        .filter(|(scene_id, _)| !running.contains(scene_id))
        .filter_map(|(_, urn)| sw.model.portable(urn))
        .collect();

    if remove.is_empty() && insert.is_empty() {
        return;
    }
    for hash in remove.iter() {
        info!("stopping smart wearable scene {hash}");
    }
    for (hash, source) in insert.iter() {
        info!("starting smart wearable {} as {hash}", source.pid);
    }
    // deferred: ContainingScene reads PortableScenes
    commands.queue(move |world: &mut World| {
        let mut portables = world.resource_mut::<PortableScenes>();
        for hash in remove {
            portables.remove(&hash);
        }
        for (hash, source) in insert {
            portables.insert(hash, source);
        }
    });
}

#[derive(clap::ValueEnum, Clone, Copy, Debug)]
enum SmartWearableAction {
    Run,
    Kill,
    Restart,
}

/// Run a smart wearable without equipping it, or stop or restart one.
#[derive(clap::Parser, ConsoleCommand)]
#[command(name = "/smart_wearable")]
struct SmartWearableCommand {
    action: SmartWearableAction,
    urn: String,
}

fn smart_wearable_command(
    mut input: ConsoleCommand<SmartWearableCommand>,
    mut sw: ResMut<SmartWearables>,
) {
    let Some(Ok(command)) = input.take() else {
        return;
    };
    if server_mode() {
        input.reply_failed("smart wearables do not run on the authoritative server");
        return;
    }
    let given = command.urn.trim().to_owned();
    let urn = normalize_urn(&given);
    match command.action {
        SmartWearableAction::Run => {
            if !sw.model.forced.contains_key(&urn) && WearableUrn::new_quiet(&urn).is_err() {
                input.reply_failed(format!("invalid wearable urn: {given}"));
                return;
            }
            sw.model.run(urn);
            input.reply_ok(format!("running {given}"));
        }
        SmartWearableAction::Kill => {
            if sw.model.stop(&urn) {
                input.reply_ok(format!("killed {given}"));
            } else {
                input.reply_failed(format!("not running: {given}"));
            }
        }
        SmartWearableAction::Restart => {
            if sw.model.restart(&urn) {
                input.reply_ok(format!("restarted {given}"));
            } else {
                input.reply_failed(format!("unknown smart wearable: {given}"));
            }
        }
    }
}

#[cfg(test)]
mod tests;

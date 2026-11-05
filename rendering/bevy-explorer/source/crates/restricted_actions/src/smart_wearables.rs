//! Smart wearables: equipped (or previewed, or console-run) wearables whose content carries an
//! SDK7 scene, run as portable experiences.
//!
//! The plugin follows the primary user's profile and the realm's `/preview-wearables`, resolves
//! each wearable's `scene.json` for the player's body shape, asks the player to allow the gated
//! permissions a wearable declares, and keeps the smart-wearable entries of [`PortableScenes`]
//! equal to the set that may run: resolved, not killed or denied, authorized when it has to be,
//! and allowed by the parcel scene the player stands in.
//!
//! The decisions live in [`Model`], plain data with no Bevy world, so they are unit tested
//! directly; the systems feed it observations and apply its output.

mod diagnostics;

use std::{
    collections::{BTreeMap, BTreeSet},
    path::PathBuf,
    sync::Arc,
};

use bevy::{
    asset::AsyncReadExt,
    platform::collections::{HashMap, HashSet},
    prelude::*,
    tasks::{IoTaskPool, Task},
};
use bevy_console::ConsoleCommand;
use collectibles::{
    request_collectibles,
    wearables::{Wearable, WearableUrn},
    CollectibleError, CollectibleManager,
};
use common::{
    rpc::RpcStreamSender,
    structs::{
        server_mode, smart_wearable_permission, CurrentRealm, PermissionType,
        PortableExperiencesToggle, PrimaryUser, SceneMeta,
    },
    util::{TaskCompat, TaskExt},
};
use comms::profile::UserProfile;
use console::DoAddConsoleCommand;
use ipfs::{
    ipfs_path::{IpfsPath, IpfsType},
    ContentMap, EntityDefinition, IpfsAssetServer, IpfsIo, IpfsModifier,
};
use reqwest::StatusCode;
use scene_runner::{
    initialize_scene::{KilledSmartWearables, PortableKind, PortableScenes, PortableSource},
    renderer_context::RendererSceneContext,
    ContainingScene,
};
use serde::Deserialize;
use system_bridge::{
    SmartWearableAuthReason, SmartWearableAuthRequest, SmartWearableEntryInfo, SmartWearableSource,
    SmartWearableState, SmartWearableStateInfo, SystemApi,
};

pub struct SmartWearablesPlugin;

impl Plugin for SmartWearablesPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<SmartWearables>();
        // a scene kill/exit records the urn here; scene_runner normally owns it
        app.init_resource::<KilledSmartWearables>();
        app.add_systems(
            Update,
            (
                observe_equipped,
                preview_wearables,
                resolve_entries,
                authorize,
                reconcile,
                answer_state,
            )
                .chain()
                .after(request_collectibles::<Wearable>)
                .run_if(|| !server_mode()),
        );
        app.add_console_command::<SmartWearableCommand, _>(smart_wearable_command);
        diagnostics::register(app);
    }
}

/// Body-shape folders of a smart wearable's content.
const MALE_PREFIX: &str = "male/";
const FEMALE_PREFIX: &str = "female/";

/// The key every smart wearable state is held under: a parsed wearable urn (lowercase, without a
/// token id), else the lowercased string (a preview wearable's id need not be a catalyst urn: a
/// `dcl-scene-cli start` preview lists its `wearable.json` uuid, which must not become a
/// bare-name urn).
pub fn normalize_urn(urn: &str) -> String {
    let urn = urn.trim();
    if !urn.contains(':') {
        return urn.to_lowercase();
    }
    WearableUrn::new(urn)
        .map(String::from)
        .unwrap_or_else(|_| urn.to_lowercase())
}

fn other_prefix(prefix: &str) -> &'static str {
    if prefix == FEMALE_PREFIX {
        MALE_PREFIX
    } else {
        FEMALE_PREFIX
    }
}

fn shape_name(prefix: &str) -> &'static str {
    if prefix == FEMALE_PREFIX {
        "female"
    } else {
        "male"
    }
}

/// A wearable listed by the realm's `/preview-wearables`.
#[derive(Clone, Debug)]
pub struct PreviewWearable {
    pub urn: String,
    pub name: String,
    pub content: ContentMap,
    pub base_url: String,
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

/// A smart wearable whose scene is assembled and may run.
#[derive(Clone, Debug, PartialEq)]
pub struct ReadyScene {
    pub scene_id: String,
    pub name: String,
    /// `requiredPermissions` as declared
    pub declared: Vec<String>,
    /// the declared permissions the player must allow
    pub gated: HashSet<PermissionType>,
}

impl ReadyScene {
    pub fn requires_auth(&self) -> bool {
        !self.gated.is_empty()
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Phase {
    /// waiting for the wearable's data or its scene.json
    Loading,
    NotSmart,
    Failed(String),
    Ready(ReadyScene),
}

#[derive(Clone, Debug)]
pub struct ModelEntry {
    pub source: SmartWearableSource,
    pub body_shape_prefix: String,
    pub name: String,
    pub phase: Phase,
}

#[derive(Clone, Debug)]
struct PendingAuth {
    urn: String,
    request: SmartWearableAuthRequest,
}

/// What the profile says, normalized: the equipped wearables and the body-shape folder.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EquippedSet {
    pub urns: BTreeSet<String>,
    pub body_shape_prefix: String,
}

/// Every smart wearable decision, over plain data.
#[derive(Debug)]
pub struct Model {
    pub entries: BTreeMap<String, ModelEntry>,
    pub killed: HashSet<String>,
    pub authorized: HashSet<String>,
    pub denied: HashSet<String>,
    pub forced: HashMap<String, ForcedSource>,
    /// the profile last seen, `None` while logged out
    pub equipped: Option<EquippedSet>,
    /// the wearables equipped when the current login started: they are asked about with reason
    /// `login`, and only for web3
    pub login_set: HashSet<String>,
    pending_auth: Option<PendingAuth>,
    next_auth_id: u32,
}

impl Default for Model {
    fn default() -> Self {
        Self {
            entries: Default::default(),
            killed: Default::default(),
            authorized: Default::default(),
            denied: Default::default(),
            forced: Default::default(),
            equipped: None,
            login_set: Default::default(),
            pending_auth: None,
            next_auth_id: 1,
        }
    }
}

impl Model {
    pub fn body_shape_prefix(&self) -> &str {
        self.equipped
            .as_ref()
            .map(|e| e.body_shape_prefix.as_str())
            .unwrap_or(MALE_PREFIX)
    }

    /// Feed the primary user's profile (`None`: logged out). Returns whether anything changed.
    pub fn observe_profile(&mut self, equipped: Option<EquippedSet>) -> bool {
        if self.equipped == equipped {
            return false;
        }
        match equipped {
            None => {
                // logout: forget the session; console and preview runs stay
                self.equipped = None;
                self.killed.clear();
                self.authorized.clear();
                self.denied.clear();
                self.login_set.clear();
                self.pending_auth = None;
            }
            Some(equipped) => {
                if self.equipped.is_none() {
                    // the first profile of a login
                    self.login_set = equipped.urns.iter().cloned().collect();
                }
                self.equipped = Some(equipped);
            }
        }
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
    /// forgotten with its kill and authorization (re-equipping starts over), a body-shape change
    /// re-resolves, a new one starts loading.
    pub fn sync_entries(&mut self) {
        let desired = self.desired();
        let prefix = self.body_shape_prefix().to_owned();

        let gone: Vec<String> = self
            .entries
            .keys()
            .filter(|urn| !desired.contains_key(*urn))
            .cloned()
            .collect();
        for urn in gone {
            self.forget(&urn);
        }

        for (urn, source) in desired {
            let entry = self
                .entries
                .entry(urn.clone())
                .or_insert_with(|| ModelEntry {
                    source,
                    body_shape_prefix: prefix.clone(),
                    name: urn.clone(),
                    phase: Phase::Loading,
                });
            entry.source = source;
            if entry.body_shape_prefix != prefix {
                entry.body_shape_prefix = prefix.clone();
                entry.phase = Phase::Loading;
            }
        }
    }

    fn forget(&mut self, urn: &str) {
        self.entries.remove(urn);
        self.killed.remove(urn);
        self.authorized.remove(urn);
        self.denied.remove(urn);
        self.login_set.remove(urn);
        if self.pending_auth.as_ref().is_some_and(|p| p.urn == urn) {
            self.pending_auth = None;
        }
    }

    /// Whether the entry for `urn` still needs the resolution started for `prefix`.
    pub fn is_loading(&self, urn: &str, prefix: &str) -> bool {
        self.entries
            .get(urn)
            .is_some_and(|e| e.phase == Phase::Loading && e.body_shape_prefix == prefix)
    }

    pub fn set_phase(&mut self, urn: &str, phase: Phase) {
        if let Some(entry) = self.entries.get_mut(urn) {
            if let Phase::Ready(ready) = &phase {
                entry.name = ready.name.clone();
            }
            entry.phase = phase;
        }
    }

    pub fn set_name(&mut self, urn: &str, name: &str) {
        if let Some(entry) = self.entries.get_mut(urn) {
            entry.name = name.to_owned();
        }
    }

    /// A login entry still loading: until none is, a newly ready one is asked about as `login`.
    pub fn login_pending(&self) -> bool {
        self.login_set
            .iter()
            .any(|urn| matches!(self.entries.get(urn), Some(e) if e.phase == Phase::Loading))
    }

    fn undecided(&self, urn: &str, ready: &ReadyScene) -> bool {
        ready.requires_auth() && !self.authorized.contains(urn) && !self.denied.contains(urn)
    }

    /// Authorize what needs no question, and return the next question to put to the player if
    /// one can be asked (`can_ask`: a HUD listens) and none is open.
    pub fn next_auth_request(&mut self, can_ask: bool) -> Option<SmartWearableAuthRequest> {
        // auto-allow login entries that do not ask for web3 (the stock explorer asks only for
        // web3 at login)
        let auto: Vec<String> = self
            .entries
            .iter()
            .filter_map(|(urn, entry)| match &entry.phase {
                Phase::Ready(ready)
                    if self.login_set.contains(urn)
                        && self.undecided(urn, ready)
                        && !ready.gated.contains(&PermissionType::Web3) =>
                {
                    Some(urn.clone())
                }
                _ => None,
            })
            .collect();
        self.authorized.extend(auto);

        if let Some(pending) = &self.pending_auth {
            let still_open = match self.entries.get(&pending.urn).map(|e| &e.phase) {
                Some(Phase::Ready(ready)) => self.undecided(&pending.urn, ready),
                _ => false,
            };
            if still_open {
                return None;
            }
            self.pending_auth = None;
        }

        if !can_ask {
            return None;
        }

        let request = self
            .entries
            .iter()
            .find_map(|(urn, entry)| match &entry.phase {
                Phase::Ready(ready) if self.undecided(urn, ready) && !self.killed.contains(urn) => {
                    Some(SmartWearableAuthRequest {
                        id: self.next_auth_id,
                        urn: urn.clone(),
                        name: entry.name.clone(),
                        permissions: ready.declared.clone(),
                        reason: if self.login_set.contains(urn) {
                            SmartWearableAuthReason::Login
                        } else {
                            SmartWearableAuthReason::Equip
                        },
                    })
                }
                _ => None,
            })?;
        self.next_auth_id = self.next_auth_id.wrapping_add(1).max(1);
        self.pending_auth = Some(PendingAuth {
            urn: request.urn.clone(),
            request: request.clone(),
        });
        Some(request)
    }

    /// The open question, to repeat to a HUD that subscribes after it was asked.
    pub fn pending_request(&self) -> Option<&SmartWearableAuthRequest> {
        self.pending_auth.as_ref().map(|p| &p.request)
    }

    /// The player's answer. Returns false for an id that is not the open question.
    pub fn answer(&mut self, id: u32, allow: bool) -> bool {
        let Some(pending) = self.pending_auth.take_if(|p| p.request.id == id) else {
            return false;
        };
        if allow {
            self.authorized.insert(pending.urn);
        } else {
            self.denied.insert(pending.urn);
        }
        true
    }

    /// The state the player sees for an entry while the current parcel `allows` portables.
    pub fn state_of(&self, urn: &str, allows: bool) -> (SmartWearableState, Option<String>) {
        let Some(entry) = self.entries.get(urn) else {
            return (SmartWearableState::Failed, Some("unknown".to_owned()));
        };
        match &entry.phase {
            Phase::Loading => (SmartWearableState::Loading, None),
            Phase::NotSmart => (SmartWearableState::NotSmart, None),
            Phase::Failed(reason) => (SmartWearableState::Failed, Some(reason.clone())),
            Phase::Ready(ready) => {
                if self.killed.contains(urn) {
                    (SmartWearableState::Killed, None)
                } else if self.denied.contains(urn) {
                    (SmartWearableState::Denied, None)
                } else if ready.requires_auth() && !self.authorized.contains(urn) {
                    (SmartWearableState::AwaitingAuthorization, None)
                } else if !allows {
                    (
                        SmartWearableState::Blocked,
                        Some("portable experiences are disabled in this scene".to_owned()),
                    )
                } else {
                    (SmartWearableState::Running, None)
                }
            }
        }
    }

    /// The scenes that should run: scene id -> urn.
    pub fn effective(&self, allows: bool) -> BTreeMap<String, String> {
        self.entries
            .iter()
            .filter_map(|(urn, entry)| match &entry.phase {
                Phase::Ready(ready)
                    if self.state_of(urn, allows).0 == SmartWearableState::Running =>
                {
                    Some((ready.scene_id.clone(), urn.clone()))
                }
                _ => None,
            })
            .collect()
    }

    pub fn info(&self, toggle: PortableExperiencesToggle) -> SmartWearableStateInfo {
        let allows = toggle != PortableExperiencesToggle::Disabled;
        SmartWearableStateInfo {
            entries: self
                .entries
                .iter()
                .map(|(urn, entry)| {
                    let (state, reason) = self.state_of(urn, allows);
                    SmartWearableEntryInfo {
                        urn: urn.clone(),
                        name: entry.name.clone(),
                        state,
                        reason,
                        scene_hash: match &entry.phase {
                            Phase::Ready(ready) => Some(ready.scene_id.clone()),
                            _ => None,
                        },
                        source: entry.source,
                    }
                })
                .collect(),
            current_scene_allows: allows,
            current_scene_hides_ui: toggle == PortableExperiencesToggle::HideUi,
        }
    }

    /// `/smart_wearable run`: run without equipping.
    pub fn run(&mut self, urn: String) {
        self.killed.remove(&urn);
        self.forced.entry(urn).or_insert(ForcedSource::Console);
        self.sync_entries();
    }

    /// `/smart_wearable kill`: false when there is nothing to stop.
    pub fn kill(&mut self, urn: &str) -> bool {
        let stoppable = matches!(
            self.entries.get(urn).map(|e| &e.phase),
            Some(Phase::Loading | Phase::Ready(_))
        );
        stoppable && self.killed.insert(urn.to_owned())
    }

    /// `/smart_wearable restart`: clear a kill or a denial (asking again), retry a failure.
    pub fn restart(&mut self, urn: &str) -> bool {
        let Some(entry) = self.entries.get_mut(urn) else {
            return false;
        };
        if matches!(entry.phase, Phase::Failed(_)) {
            entry.phase = Phase::Loading;
        }
        self.killed.remove(urn);
        self.denied.remove(urn);
        true
    }

    /// Replace the realm's preview wearables.
    pub fn set_previews(&mut self, previews: Vec<PreviewWearable>) {
        self.forced
            .retain(|_, forced| !matches!(forced, ForcedSource::Preview(_)));
        for preview in previews {
            // a console run of the same urn becomes the preview
            self.entries.remove(&preview.urn);
            self.forced
                .insert(preview.urn.clone(), ForcedSource::Preview(preview));
        }
        self.sync_entries();
    }

    pub fn preview_count(&self) -> usize {
        self.forced
            .values()
            .filter(|f| matches!(f, ForcedSource::Preview(_)))
            .count()
    }
}

/// The outcome of reading a smart wearable's scene.json.
#[derive(Debug)]
pub struct Resolved {
    pub scene_id: String,
    pub stripped: ContentMap,
    pub meta_value: serde_json::Value,
    pub scene_json: String,
    pub base_url: String,
    pub ready: ReadyScene,
}

/// The content key of the scene.json for `prefix`: the body shape's own, else a bare one.
pub fn scene_json_key(files: &[String], prefix: &str) -> Option<String> {
    let own = format!("{prefix}scene.json");
    files
        .iter()
        .find(|f| f.eq_ignore_ascii_case(&own))
        .or_else(|| files.iter().find(|f| f.eq_ignore_ascii_case("scene.json")))
        .cloned()
}

/// Build a smart wearable's scene from its scene.json and the wearable's content: an SDK7 scene
/// whose `main` exists once the body shape's folder is the root.
pub fn build_resolved(
    hash: &str,
    prefix: &str,
    name: String,
    scene_json: &[u8],
    content: &ContentMap,
    base_url: String,
) -> Result<Resolved, String> {
    let scene_json =
        String::from_utf8(scene_json.to_vec()).map_err(|_| "scene.json is not utf-8".to_owned())?;
    let meta_value: serde_json::Value =
        serde_json::from_str(&scene_json).map_err(|e| format!("invalid scene.json: {e}"))?;
    let meta: SceneMeta = serde_json::from_value(meta_value.clone())
        .map_err(|e| format!("invalid scene.json: {e}"))?;
    if meta.runtime_version.as_deref() != Some("7") {
        return Err(format!(
            "runtimeVersion {} is not 7",
            meta.runtime_version.as_deref().unwrap_or("(none)")
        ));
    }
    let stripped = content.strip_prefix(prefix, &[other_prefix(prefix)]);
    if stripped.hash(&meta.main).is_none() {
        return Err(format!("main file {} is missing", meta.main));
    }
    let declared = meta.required_permissions.clone().unwrap_or_default();
    let gated = declared
        .iter()
        .filter_map(|name| smart_wearable_permission(name))
        .collect();
    let scene_id = format!("{hash}-sw-{}", shape_name(prefix));
    Ok(Resolved {
        scene_id: scene_id.clone(),
        stripped,
        meta_value,
        scene_json,
        base_url,
        ready: ReadyScene {
            scene_id,
            name,
            declared,
            gated,
        },
    })
}

#[derive(Deserialize)]
struct PreviewDocument {
    #[serde(default)]
    data: Vec<PreviewDocumentWearable>,
}

/// A catalyst (and e2e realm) entry nests the content under `data.representations`, with keys
/// under the body-shape folders; a `dcl-scene-cli start` entry lists unprefixed `contents` at the
/// top level with a `baseUrl`, and keeps the `wearable.json` `data` (whose representation
/// `contents` are bare file names), so `data` is read only in the first shape.
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

/// The `{ key, url, hash }` entries of `data.representations[*].contents`; anything else there is
/// ignored.
fn representation_contents(data: &serde_json::Value) -> Vec<PreviewDocumentContent> {
    data.get("representations")
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|r| r.get("contents")?.as_array())
        .flatten()
        .filter_map(|c| serde_json::from_value(c.clone()).ok())
        .collect()
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
            })
        })
        .collect())
}

/// The plugin's state: the [`Model`] plus what cannot be plain data.
#[derive(Resource, Default)]
pub struct SmartWearables {
    pub model: Model,
    /// scene.json reads in flight, by urn, with the body-shape folder they resolve
    tasks: HashMap<String, (String, Task<Result<Resolved, String>>)>,
    /// the assembled entity of each ready wearable, by urn
    definitions: HashMap<String, Handle<EntityDefinition>>,
    auth_streams: Vec<RpcStreamSender<SmartWearableAuthRequest>>,
    preview_task: Option<Task<Result<Vec<PreviewWearable>, String>>>,
}

impl SmartWearables {
    pub fn has_auth_listener(&self) -> bool {
        self.auth_streams.iter().any(|s| !s.is_closed())
    }
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
                WearableUrn::new(urn)
                    .ok()
                    .is_none_or(|urn| urn.collection_type() != Some("base-avatars"))
            })
            .map(|urn| normalize_urn(urn))
            .collect(),
        body_shape_prefix: if profile.is_female() {
            FEMALE_PREFIX
        } else {
            MALE_PREFIX
        }
        .to_owned(),
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

fn preview_wearables(
    mut sw: ResMut<SmartWearables>,
    realm: Res<CurrentRealm>,
    ipfas: IpfsAssetServer,
) {
    if realm.is_changed() {
        sw.model.set_previews(Vec::new());
        sw.preview_task = None;
        let base = realm
            .about_url
            .strip_suffix("/about")
            .unwrap_or(&realm.about_url)
            .trim_end_matches('/');
        if base.starts_with("http") {
            let url = format!("{base}/preview-wearables");
            let client = ipfas.ipfs().client();
            sw.preview_task = Some(IoTaskPool::get().spawn_compat(async move {
                let response = client.get(&url).send().await.map_err(|e| e.to_string())?;
                if response.status() == StatusCode::NOT_FOUND {
                    return Ok(Vec::new());
                }
                if !response.status().is_success() {
                    return Err(format!("status {}", response.status()));
                }
                let body = response.bytes().await.map_err(|e| e.to_string())?;
                parse_preview_wearables(&body)
            }));
        }
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

async fn read_scene_json(ipfs: &Arc<IpfsIo>, hash: &str, key: &str) -> Result<Vec<u8>, String> {
    let path = PathBuf::from(&IpfsPath::new(IpfsType::new_content_file(
        hash.to_owned(),
        key.to_owned(),
    )));
    let mut reader = ipfs
        .read(&path)
        .await
        .map_err(|e| format!("reading {key}: {e}"))?;
    let mut bytes = Vec::new();
    reader
        .read_to_end(&mut bytes)
        .await
        .map_err(|e| format!("reading {key}: {e}"))?;
    Ok(bytes)
}

fn resolve_entries(
    mut sw: ResMut<SmartWearables>,
    mut wearables: CollectibleManager<Wearable>,
    ipfas: IpfsAssetServer,
    mut definitions: ResMut<Assets<EntityDefinition>>,
) {
    let sw = &mut *sw;

    // drop work for entries that left or changed shape, and definitions of entries no longer ready
    sw.tasks
        .retain(|urn, (prefix, _)| sw.model.is_loading(urn, prefix));
    sw.definitions.retain(|urn, _| {
        matches!(
            sw.model.entries.get(urn).map(|e| &e.phase),
            Some(Phase::Ready(_))
        )
    });

    // finished reads
    let finished: Vec<(String, Result<Resolved, String>)> = sw
        .tasks
        .iter_mut()
        .filter_map(|(urn, (_, task))| task.complete().map(|result| (urn.clone(), result)))
        .collect();
    for (urn, result) in finished {
        sw.tasks.remove(&urn);
        match result {
            Ok(resolved) => {
                ipfas.ipfs().add_collection(
                    resolved.scene_id.clone(),
                    resolved.stripped.clone(),
                    Some(IpfsModifier {
                        base_url: Some(resolved.base_url.clone()),
                    }),
                    Some(resolved.scene_json.clone()),
                );
                let handle = definitions.add(EntityDefinition {
                    id: resolved.scene_id.clone(),
                    pointers: Vec::new(),
                    content: resolved.stripped,
                    metadata: Some(resolved.meta_value),
                });
                sw.definitions.insert(urn.clone(), handle);
                info!("smart wearable {urn} ready as {}", resolved.scene_id);
                sw.model.set_phase(&urn, Phase::Ready(resolved.ready));
            }
            Err(e) => {
                warn!("smart wearable {urn} failed: {e}");
                sw.model.set_phase(&urn, Phase::Failed(e));
            }
        }
    }

    // start reads
    let starting: Vec<(String, String)> = sw
        .model
        .entries
        .iter()
        .filter(|(urn, e)| e.phase == Phase::Loading && !sw.tasks.contains_key(*urn))
        .map(|(urn, e)| (urn.clone(), e.body_shape_prefix.clone()))
        .collect();
    for (urn, prefix) in starting {
        let ipfs = ipfas.ipfs().clone();
        let preview = match sw.model.forced.get(&urn) {
            Some(ForcedSource::Preview(preview)) => Some(preview.clone()),
            _ => None,
        };
        let task = match preview {
            Some(preview) => {
                let files: Vec<String> = preview.content.files().cloned().collect();
                let Some(key) = scene_json_key(&files, &prefix) else {
                    sw.model.set_phase(&urn, Phase::NotSmart);
                    continue;
                };
                // the preview's content is registered under a synthetic collection so the
                // scene.json can be read through the cache like any content file
                let hash = format!("{}-preview", preview.content.hash(&key).unwrap_or_default());
                ipfas.ipfs().add_collection(
                    hash.clone(),
                    preview.content.clone(),
                    Some(IpfsModifier {
                        base_url: Some(preview.base_url.clone()),
                    }),
                    None,
                );
                let PreviewWearable {
                    name,
                    content,
                    base_url,
                    ..
                } = preview;
                sw.model.set_name(&urn, &name);
                let prefix = prefix.clone();
                IoTaskPool::get().spawn_compat(async move {
                    let bytes = read_scene_json(&ipfs, &hash, &key).await?;
                    build_resolved(&hash, &prefix, name, &bytes, &content, base_url)
                })
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
                sw.model.set_name(&urn, &name);
                let Some(key) = scene_json_key(&data.extra_data.scene_json_files, &prefix) else {
                    sw.model.set_phase(&urn, Phase::NotSmart);
                    continue;
                };
                // the pointer's hash, not `CollectibleData::hash`: the data loader reads the
                // entity through a content-file path and leaves that field empty
                let Ok(hash) = wearables.get_hash(&wearable_urn) else {
                    sw.model
                        .set_phase(&urn, Phase::Failed("wearable hash unavailable".to_owned()));
                    continue;
                };
                let prefix = prefix.clone();
                IoTaskPool::get().spawn_compat(async move {
                    let (entity, base_url) = ipfs
                        .entity_definition(&hash)
                        .await
                        .ok_or_else(|| "wearable entity is not registered".to_owned())?;
                    let bytes = read_scene_json(&ipfs, &hash, &key).await?;
                    build_resolved(&hash, &prefix, name, &bytes, &entity.collection, base_url)
                })
            }
        };
        sw.tasks.insert(urn, (prefix, task));
    }
}

fn authorize(mut sw: ResMut<SmartWearables>, mut events: EventReader<SystemApi>) {
    for ev in events.read() {
        match ev {
            SystemApi::GetSmartWearableAuthorizationStream(stream) => {
                if let Some(request) = sw.model.pending_request() {
                    let _ = stream.send(request.clone());
                }
                sw.auth_streams.push(stream.clone());
            }
            SystemApi::AnswerSmartWearableAuthorization { id, allow }
                if !sw.model.answer(*id, *allow) =>
            {
                warn!("smart wearable authorization {id} is not pending");
            }
            _ => (),
        }
    }
    sw.auth_streams.retain(|s| !s.is_closed());

    let can_ask = !sw.auth_streams.is_empty();
    if let Some(request) = sw.model.next_auth_request(can_ask) {
        info!(
            "asking to authorize smart wearable {} ({:?})",
            request.urn, request.permissions
        );
        for stream in sw.auth_streams.iter() {
            let _ = stream.send(request.clone());
        }
    }
}

/// The `featureToggles.portableExperiences` of the parcel scene the player stands in.
pub(crate) fn current_toggle(
    player: &Query<Entity, With<PrimaryUser>>,
    containing_scene: &ContainingScene,
    contexts: &Query<&RendererSceneContext>,
) -> PortableExperiencesToggle {
    player
        .single()
        .ok()
        .and_then(|player| containing_scene.get_parcel(player))
        .and_then(|scene| contexts.get(scene).ok())
        .map(|ctx| ctx.portable_experiences)
        .unwrap_or_default()
}

fn reconcile(
    mut commands: Commands,
    mut sw: ResMut<SmartWearables>,
    killed_by_scenes: Option<ResMut<KilledSmartWearables>>,
    portables: Res<PortableScenes>,
    player: Query<Entity, With<PrimaryUser>>,
    containing_scene: ContainingScene,
    contexts: Query<&RendererSceneContext>,
) {
    if let Some(mut killed) = killed_by_scenes {
        if !killed.0.is_empty() {
            let drained: Vec<String> = killed.0.drain().map(|urn| normalize_urn(&urn)).collect();
            sw.model.killed.extend(drained);
        }
    }

    let toggle = current_toggle(&player, &containing_scene, &contexts);
    let effective = sw
        .model
        .effective(toggle != PortableExperiencesToggle::Disabled);

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
        .filter_map(|(scene_id, urn)| {
            let definition = sw.definitions.get(urn)?.clone();
            let name = sw.model.entries.get(urn)?.name.clone();
            Some((
                scene_id.clone(),
                PortableSource {
                    pid: urn.clone(),
                    parent_scene: None,
                    ens: None,
                    super_user: false,
                    kind: PortableKind::SmartWearable {
                        urn: urn.clone(),
                        name,
                        definition,
                    },
                },
            ))
        })
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

fn answer_state(
    sw: Res<SmartWearables>,
    mut events: EventReader<SystemApi>,
    player: Query<Entity, With<PrimaryUser>>,
    containing_scene: ContainingScene,
    contexts: Query<&RendererSceneContext>,
) {
    let mut senders = events
        .read()
        .filter_map(|ev| match ev {
            SystemApi::SmartWearableState(sender) => Some(sender.clone()),
            _ => None,
        })
        .peekable();
    if senders.peek().is_none() {
        return;
    }
    let info = sw
        .model
        .info(current_toggle(&player, &containing_scene, &contexts));
    for sender in senders {
        sender.send(info.clone());
    }
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
            if WearableUrn::new(&urn).is_err() && !sw.model.forced.contains_key(&urn) {
                input.reply_failed(format!("invalid wearable urn: {given}"));
                return;
            }
            sw.model.run(urn);
            input.reply_ok(format!("running {given}"));
        }
        SmartWearableAction::Kill => {
            if sw.model.kill(&urn) {
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

pub mod name_color;
pub mod outfits;

use std::{io::Read, path::PathBuf, sync::Arc};

use alloy_core::primitives::Address;
use anyhow::anyhow;
use bevy::{
    ecs::system::SystemParam,
    platform::collections::HashMap,
    prelude::*,
    tasks::{IoTaskPool, Task},
};
use dcl_types::interface::CrdtType;
use ipfs::{ipfs_path::IpfsPath, IpfsAssetServer, IpfsIo};
use multihash_codetable::MultihashDigest;
use reqwest::StatusCode;
use serde::{Deserialize, Serialize};

use crate::{global_crdt::GlobalCrdtState, profile::name_color::name_color};

use super::{
    broadcast,
    global_crdt::{process_transport_updates, ForeignPlayer, ProfileEvent, ProfileEventType},
    BroadcastTarget, NetworkMessage, NetworkMessageRecipient, ProfileUpdate, Transport,
    TransportType,
};
use common::{
    profile::{
        AvatarColor, AvatarEmote, AvatarSnapshots, CompactProfile, LambdaProfiles,
        SerializedProfile,
    },
    rpc::{RpcCallEvent, RpcEventSender, RpcResultSender, RpcStreamSender},
    sets::SceneSets,
    structs::{AppConfig, PrimaryUser},
    util::{TaskCompat, TaskExt},
};
use common::{rpc::RpcCall, util::AsH160};
use dcl_component::{
    proto_components::{
        kernel::comms::rfc4, sdk::components::PbPlayerIdentityData, Color3DclToBevy,
    },
    SceneComponentId, SceneEntityId,
};
use system_bridge::{
    ProfileChangedEvent, SetAvatarData, SetAvatarError, SetAvatarResult, SystemApi,
};
use wallet::Wallet;

use bevy_console::ConsoleCommand;
use console::{AddDebugSection, DebugSection, DoAddConsoleCommand};

pub struct UserProfilePlugin;

impl Plugin for UserProfilePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (
                (
                    drive_profile_fetches,
                    request_missing_profiles,
                    serve_compact_profiles,
                )
                    .chain(),
                process_profile_events,
            )
                .before(process_transport_updates), // .in_set(TODO)
        );
        app.add_systems(Update, outfits::process_outfits)
            .init_resource::<outfits::OutfitsState>();
        if common::structs::server_mode() {
            // A server never publishes or deploys a local player.
            app.add_systems(
                Update,
                (process_profile, pipe_profile_changes_to_scene)
                    .chain()
                    .in_set(SceneSets::RestrictedActions),
            );
        } else {
            // Queue edits, settle the upload and refresh PLAYER before notifying scene readers.
            app.add_systems(
                Update,
                (
                    process_profile,
                    drive_profile_deploys,
                    setup_primary_profile,
                    pipe_profile_changes_to_scene,
                )
                    .chain()
                    .in_set(SceneSets::RestrictedActions),
            );
        }

        app.add_console_command::<ProfileRequestsCommand, _>(profile_requests)
            .add_debug_section(DebugSection {
                title: "Entity requests / profiles",
                query: "/profile_requests",
                fields: &[
                    ("Catalyst POSTs", "posts"),
                    ("Single-profile POSTs", "single"),
                    ("Aggregated POSTs", "aggregated"),
                    ("Last batch size", "lastBatch"),
                    ("Mean batch size", "meanBatch"),
                    ("Batches in flight", "inFlight"),
                    ("Compact POSTs", "compact"),
                    ("Compact source", "compactSource"),
                    ("Compact route missing", "compactUnsupported"),
                    ("Unresolved cached profiles", "unresolved"),
                    ("Own profile version", "ownVersion"),
                    ("Version other players see", "peerVersion"),
                    ("Own profile deploy pending", "deployPending"),
                    ("Outfits deploys in flight", "outfitsInFlight"),
                    ("Last outfits deploy", "lastOutfitsDeploy"),
                ],
                actions: &[],
                inputs: &[],
            });
        app.insert_resource(CurrentUserProfile::default());
        app.init_resource::<ProfileCache>();
    }
}

/// Pacing for the fetch cascade: how long an unsatisfied entry waits after its last
/// cascade concluded before running another. Covers both failure retry (without it one
/// transient error would leave an address unresolvable for the engine's whole uptime)
/// and the re-probe of data still behind the announced version.
const PROFILE_FETCH_RETRY: std::time::Duration = std::time::Duration::from_secs(30);
/// The catalyst serves many ids per request; keep a batch well under any server-side cap
/// and split anything longer across requests.
const PROFILE_REQUEST_BATCH: usize = 100;
/// An entry nobody has asked about for this long is dropped (and its fetch retries with
/// it). Live foreign players touch their entry every frame, so this only reaps departed
/// peers and addresses scenes/ui asked about once.
const PROFILE_EVICT_AFTER: std::time::Duration = std::time::Duration::from_secs(300);

/// Where held profile data came from. Declared in ascending order of authority so the
/// derived `Ord` ranks them: an equal-version answer only displaces held data when it
/// comes from a more authoritative source.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
enum ProfileSource {
    Peer,
    Catalyst,
}

/// How much of a profile a reader needs. Declared smallest first so the derived `Ord`
/// keeps the larger of two asks.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug, Default)]
enum ProfileTier {
    /// Name, claimed-name flag, name colour and face: a list row
    #[default]
    Compact,
    /// The whole profile: an avatar to render, a passport
    Full,
}

fn profile_is_guest(profile: &UserProfile) -> bool {
    !profile.content.has_connected_web3.unwrap_or(false)
}

#[derive(Default)]
struct ProfileEntry {
    /// highest version any announcement or response has claimed for this address
    announced: u32,
    /// best data we hold and where it came from
    data: Option<(Box<UserProfile>, ProfileSource)>,
    /// the most any reader has asked of this address
    wanted: ProfileTier,
    /// the compact tier's answer, held only while no full data is
    compact: Option<CompactProfile>,
    /// the tier of the catalyst POST holding this address, while one is in flight; p2p is
    /// fire-and-forget, tracked by `last_p2p`
    fetching: Option<ProfileTier>,
    /// when the next cascade may start; None = due now. Irrelevant once satisfied.
    next_fetch: Option<web_time::Instant>,
    /// `time.elapsed_secs_f64()` of the last `ProfileRequest` sent to the peer
    last_p2p: Option<f64>,
    /// last time anyone read, announced or wrote this entry; drives eviction
    last_access: Option<web_time::Instant>,
}

impl ProfileEntry {
    fn profile(&self) -> Option<&UserProfile> {
        self.data.as_ref().map(|(profile, _)| profile.as_ref())
    }

    /// nothing better to look for. A compact reader takes any data; a full reader takes
    /// the catalyst's answer, or a throwaway guest's own answer, at the announced version: a
    /// throwaway guest's profile is never on the catalyst, and says nothing about web3. A guest
    /// account's is on the catalyst, so its own answer is only a stand-in, as any account's.
    fn satisfied(&self) -> bool {
        if self.wanted == ProfileTier::Compact {
            return self.data.is_some() || self.compact.is_some();
        }
        self.data.as_ref().is_some_and(|(profile, source)| {
            profile.version >= self.announced
                && (*source == ProfileSource::Catalyst
                    || (*source == ProfileSource::Peer
                        && profile.content.has_connected_web3.is_none()))
        })
    }

    fn due(&self, now: web_time::Instant) -> bool {
        self.next_fetch.is_none_or(|at| at <= now)
    }

    fn wants_fetch(&self, now: web_time::Instant) -> bool {
        !self.satisfied() && self.fetching.is_none() && self.due(now)
    }

    /// a cascade has concluded with nothing a reader of `tier` can use, and its retry isn't due
    /// yet
    fn missing(&self, tier: ProfileTier, now: web_time::Instant) -> bool {
        let held = match tier {
            ProfileTier::Compact => self.data.is_some() || self.compact.is_some(),
            ProfileTier::Full => self.data.is_some(),
        };
        !held && self.fetching.is_none() && !self.due(now)
    }

    /// the POST this entry's next fetch goes in: full for everyone once the catalyst has
    /// shown it serves no compact profiles
    fn fetch_tier(&self, compact_unsupported: bool) -> ProfileTier {
        if compact_unsupported {
            ProfileTier::Full
        } else {
            self.wanted
        }
    }

    /// raise what readers need; asking for more than was fetched makes the next fetch due
    fn want(&mut self, tier: ProfileTier) {
        if tier > self.wanted {
            self.wanted = tier;
            self.next_fetch = None;
        }
    }

    /// nobody has accessed it within `PROFILE_EVICT_AFTER`, and no fetch is in flight
    fn evictable(&self, now: web_time::Instant) -> bool {
        self.fetching.is_none()
            && self
                .last_access
                .is_none_or(|at| now.saturating_duration_since(at) >= PROFILE_EVICT_AFTER)
    }

    /// accept a newer version from anywhere, and an equal version only from a more
    /// authoritative source; also lifts `announced` so a response can't be outranked by
    /// an announcement we haven't seen
    fn apply(&mut self, profile: UserProfile, source: ProfileSource) -> bool {
        self.announced = self.announced.max(profile.version);
        let improvement = match &self.data {
            None => true,
            Some((held, held_source)) => {
                profile.version > held.version
                    || (profile.version == held.version && source > *held_source)
            }
        };
        if improvement {
            self.data = Some((Box::new(profile), source));
            self.compact = None;
        }
        improvement
    }

    /// a compact POST holding this address has landed; full data, once held, is never
    /// replaced by it. A reader that asked for the full profile meanwhile gets that fetch
    /// at once, anything else unsatisfied waits out the retry.
    fn apply_compact(&mut self, compact: Option<CompactProfile>, now: web_time::Instant) {
        self.fetching = None;
        if self.data.is_none() {
            if let Some(compact) = compact {
                self.compact = Some(compact);
            }
        }
        if !self.satisfied() {
            self.next_fetch =
                (self.wanted == ProfileTier::Compact).then(|| now + PROFILE_FETCH_RETRY);
        }
    }

    /// hold the data one version behind the announcement so the fetch cascade comes due:
    /// an answer at `version` displaces it, a lagging one at `version - 1` ties and loses.
    /// Only the cache's ranking sees the lowered version; the content keeps the real one.
    /// Newer data (a local edit made since) is left alone.
    fn refetch_deployed(&mut self, version: u32) {
        let Some((profile, _)) = self.data.as_mut() else {
            return;
        };
        if profile.version != version {
            return;
        }
        let Some(lowered) = version.checked_sub(1) else {
            return;
        };
        profile.version = lowered;
        self.next_fetch = None;
    }

    /// ask the peer directly when we're missing or behind the announcement: for guests
    /// this is the only source there is, for everyone else the peer is the authority on
    /// its own latest profile. Held back until the first cascade concludes so the
    /// authoritative sources get first go, except when we already hold (stale) data —
    /// then the request rides alongside the re-fetch.
    fn wants_p2p(&self, now: f64) -> bool {
        let behind = self
            .data
            .as_ref()
            .is_none_or(|(profile, _)| profile.version < self.announced);
        if !behind {
            return false;
        }
        let concluded = self.fetching.is_none() && self.next_fetch.is_some();
        if self.data.is_none() && !concluded {
            return false;
        }
        self.last_p2p.is_none_or(|at| now - at >= 10.0)
    }
}

type ProfileBatch = Task<Vec<(Address, Result<Option<UserProfile>, anyhow::Error>)>>;
type CompactAnswer = Result<Vec<(Address, Option<CompactProfile>)>, CompactFetchError>;

struct CompactBatch {
    addresses: Vec<Address>,
    task: Task<CompactAnswer>,
}

enum CompactFetchError {
    /// The catalyst has no compact route (Decentraland's catalysts)
    Unsupported,
    Failed(anyhow::Error),
}

#[derive(Resource, Default)]
pub struct ProfileCache {
    entries: HashMap<Address, ProfileEntry>,
    requests: ProfileRequestCounts,
    batches: Vec<ProfileBatch>,
    compact_batches: Vec<CompactBatch>,
    /// the compact source answered a compact POST as an unknown route: compact readers are
    /// served from full profiles for the rest of the session
    compact_unsupported: bool,
}

#[derive(Default)]
struct ProfileRequestCounts {
    single: u64,
    aggregated: u64,
    addresses: u64,
    last_batch: usize,
    compact: u64,
}

/// Read profile batching, outstanding fetches and our own profile's deploy state.
#[derive(clap::Parser, ConsoleCommand)]
#[command(name = "/profile_requests")]
struct ProfileRequestsCommand;

fn profile_requests(
    mut input: ConsoleCommand<ProfileRequestsCommand>,
    cache: Res<ProfileCache>,
    current: Res<CurrentUserProfile>,
    outfits: Res<outfits::OutfitsState>,
) {
    if let Some(Ok(_)) = input.take() {
        let counts = &cache.requests;
        let last_outfits_deploy = outfits.last_deploy.as_ref().map(|result| match result {
            Ok(()) => "ok".to_owned(),
            Err(e) => format!("failed: {e}"),
        });
        let posts = counts.single + counts.aggregated;
        input.reply_ok(serde_json::json!({"posts": posts, "single": counts.single, "aggregated": counts.aggregated,
            "lastBatch": counts.last_batch, "meanBatch": (posts > 0).then(|| counts.addresses as f64 / posts as f64),
            "unavailable": {"meanBatch": "No profile requests yet", "ownVersion": "Not signed in", "peerVersion": "Not signed in",
                "lastOutfitsDeploy": "No outfits deployed yet"},
            "outfitsInFlight": outfits.deploys_in_flight(), "lastOutfitsDeploy": last_outfits_deploy,
            "compact": counts.compact, "compactSource": compact_source(&lambdas_url()),
            "compactUnsupported": cache.compact_unsupported,
            "inFlight": cache.batches.len() + cache.compact_batches.len(),
            "unresolved": cache.entries.values().filter(|entry| entry.data.is_none() && entry.compact.is_none()).count(),
            "ownVersion": current.profile.as_ref().map(|profile| profile.version),
            "peerVersion": current.peer_profile().map(|profile| profile.version),
            "deployPending": current.saves.active.is_some() || !current.saves.queued.is_empty()}).to_string());
    }
}

impl ProfileCache {
    /// the entry for `address`, created if missing, marked as accessed now by a reader that
    /// needs `tier` of it
    fn touch(&mut self, address: Address, tier: ProfileTier) -> &mut ProfileEntry {
        let entry = self.entries.entry(address).or_default();
        entry.last_access = Some(web_time::Instant::now());
        entry.want(tier);
        entry
    }

    fn get_compact(
        &mut self,
        address: Address,
    ) -> Result<Option<CompactProfile>, ProfileMissingError> {
        let entry = self.touch(address, ProfileTier::Compact);
        if entry.missing(ProfileTier::Compact, web_time::Instant::now()) {
            return Err(ProfileMissingError);
        }
        Ok(entry
            .profile()
            .map(|profile| CompactProfile::from(&profile.content))
            .or_else(|| entry.compact.clone()))
    }

    /// One `getCompactProfiles` request: None while any of its addresses is still being
    /// fetched (all of them are asked each time, so they batch together), then the compact
    /// profiles of those that resolved, in request order.
    fn compact_answer(&mut self, addresses: &[Address]) -> Option<Vec<CompactProfile>> {
        let mut settled = true;
        let mut found = Vec::with_capacity(addresses.len());
        for address in addresses {
            match self.get_compact(*address) {
                Ok(Some(profile)) => found.push(profile),
                Ok(None) => settled = false,
                Err(ProfileMissingError) => (),
            }
        }
        settled.then_some(found)
    }

    fn update(&mut self, profile: UserProfile) {
        if let Some(address) = profile.content.eth_address.as_h160() {
            let entry = self.touch(address, ProfileTier::Full);
            // Preserve a pending server snapshot read-back (its ranking version is lowered).
            if entry.announced == profile.version
                && entry.profile().is_some_and(|held| {
                    held.version < profile.version
                        && held.content == profile.content
                        && held.base_url == profile.base_url
                })
            {
                return;
            }
            entry.announced = profile.version;
            entry.data = Some((Box::new(profile), ProfileSource::Catalyst));
            entry.compact = None;
        }
    }

    /// Record a profile `sender` sent of itself, under the sender's address; false, with
    /// nothing recorded, for one naming another address. `apply`'s authority ranking keeps
    /// a room-broadcast response from displacing an equal-version catalyst answer; a
    /// guest's own answer counts as final via `satisfied`.
    fn update_from_peer(&mut self, sender: Address, profile: UserProfile) -> bool {
        if !is_senders_own(sender, &profile.content) {
            return false;
        }
        self.touch(sender, ProfileTier::Full)
            .apply(profile, ProfileSource::Peer);
        true
    }

    /// the catalyst has no compact route: from now on every fetch is full, and the
    /// addresses of the refused POST are due again at once
    fn compact_endpoint_missing(&mut self, addresses: &[Address]) {
        if !self.compact_unsupported {
            warn!("the catalyst serves no compact profiles; reading full profiles instead");
            self.compact_unsupported = true;
        }
        for address in addresses {
            if let Some(entry) = self.entries.get_mut(address) {
                entry.fetching = None;
                entry.next_fetch = None;
            }
        }
    }

    /// a compact POST failed in some other way (a server error, a body that isn't the list, a
    /// fetch the browser refused): the full route may still answer, so these addresses are read
    /// whole from now on
    fn compact_fetch_failed(&mut self, addresses: &[Address]) {
        for address in addresses {
            if let Some(entry) = self.entries.get_mut(address) {
                entry.fetching = None;
                entry.want(ProfileTier::Full);
            }
        }
    }

    fn server_snapshots(&self, profile: &UserProfile) -> Option<AvatarSnapshots> {
        let address = profile.content.eth_address.as_h160()?;
        let held = self.entries.get(&address)?.profile()?;
        let snapshots = held.content.avatar.snapshots.as_ref()?;
        (held.version == profile.version
            && profile.content.avatar.snapshots.as_ref() != Some(snapshots))
        .then(|| snapshots.clone())
    }
}

/// Whose download a profile picture is, which decides whether it waits while engine memory is
/// full.
#[derive(Clone, Copy, Debug)]
pub enum ProfileImageOwner<'a> {
    /// UI the player opens: never waits.
    Player,
    /// A scene's texture: ranks with that scene's hash.
    Scene(&'a str),
    /// Anything else: ranks with the requests no scene or avatar owns.
    Unowned,
}

#[derive(SystemParam)]
pub struct ProfileManager<'w, 's> {
    cache: ResMut<'w, ProfileCache>,
    ipfs: IpfsAssetServer<'w, 's>,
}

pub struct ProfileMissingError;

impl ProfileManager<'_, '_> {
    /// Serve the best data we hold (however stale); creating the entry queues the fetch
    /// cascade, which `drive_profile_fetches` runs. Ok(None) while a fetch is pending;
    /// Err once a cascade has concluded with nothing (until the retry comes due).
    pub fn get_data(
        &mut self,
        address: Address,
    ) -> Result<Option<&UserProfile>, ProfileMissingError> {
        let entry = self.cache.touch(address, ProfileTier::Full);
        if entry.missing(ProfileTier::Full, web_time::Instant::now()) {
            return Err(ProfileMissingError);
        }
        Ok(entry.profile())
    }

    /// The compact tier: name, claimed-name flag, name colour and face, for a list row.
    /// Served from the full profile when one is held, else fetched compact from the
    /// catalyst. Same Ok(None) / Err semantics as `get_data`.
    pub fn get_compact(
        &mut self,
        address: Address,
    ) -> Result<Option<CompactProfile>, ProfileMissingError> {
        self.cache.get_compact(address)
    }

    /// Record that this address' profile is claimed to exist at `version`; a version
    /// ahead of what we hold makes the fetch cascade due immediately.
    pub fn announce(&mut self, address: Address, version: u32) {
        let entry = self.cache.touch(address, ProfileTier::Full);
        if version > entry.announced {
            entry.announced = version;
            entry.next_fetch = None;
        }
    }

    pub fn get_image(
        &mut self,
        address: Address,
        owner: ProfileImageOwner,
    ) -> Result<Option<Handle<Image>>, ProfileMissingError> {
        let Some(profile) = self.get_compact(address)? else {
            return Ok(None);
        };
        if profile.thumbnail_url.is_empty() {
            return Err(ProfileMissingError);
        }
        // through the deployment's gateway when one serves profile images
        let url = common::base_domain::profile_image(&profile.thumbnail_url);
        let ipfs_path = IpfsPath::new_from_url(&url, "png");
        let ipfs_path = match owner {
            ProfileImageOwner::Player => ipfs_path.with_player_slot_owner(),
            ProfileImageOwner::Scene(hash) => ipfs_path.with_slot_owner(hash),
            ProfileImageOwner::Unowned => ipfs_path,
        };
        Ok(Some(
            self.ipfs.asset_server().load(PathBuf::from(&ipfs_path)),
        ))
    }

    pub fn get_name(&mut self, address: Address) -> Result<Option<&String>, ProfileMissingError> {
        let entry = self.cache.touch(address, ProfileTier::Compact);
        if entry.missing(ProfileTier::Compact, web_time::Instant::now()) {
            return Err(ProfileMissingError);
        }
        Ok(entry
            .data
            .as_ref()
            .map(|(profile, _)| &profile.content.name)
            .or(entry.compact.as_ref().map(|compact| &compact.name)))
    }

    /// Publish the local account's confirmed profile to cache readers. A pending read-back
    /// for server-generated thumbnails stays due when the same content is published again.
    pub fn update(&mut self, profile: UserProfile) {
        self.cache.update(profile);
    }

    /// Our own profile is deployed at `version`: read it back, since server-generated
    /// fields (the snapshots) only arrive that way.
    pub fn refetch_deployed(&mut self, address: Address, version: u32) {
        if let Some(entry) = self.cache.entries.get_mut(&address) {
            entry.refetch_deployed(version);
        }
    }

    /// The server-generated snapshots for our own `profile`, once the read-back of its
    /// version has landed with ones that differ from what `profile` holds.
    pub fn server_snapshots(&self, profile: &UserProfile) -> Option<AvatarSnapshots> {
        self.cache.server_snapshots(profile)
    }

    /// Record a profile `sender` sent of itself; see `ProfileCache::update_from_peer`.
    pub fn update_from_peer(&mut self, sender: Address, profile: UserProfile) -> bool {
        self.cache.update_from_peer(sender, profile)
    }
}

/// Whether a profile received from a peer is that peer's own: every address it carries is
/// the sender's. A peer speaks only for itself; a profile naming anyone else would be
/// shown as theirs. An address counts in any letter case, `0X` included, and an empty
/// user id as none, so a sender's spelling alone never drops its profile.
fn is_senders_own(sender: Address, content: &SerializedProfile) -> bool {
    let is_sender = |address: &str| address.to_ascii_lowercase().as_h160() == Some(sender);
    is_sender(&content.eth_address)
        && content
            .user_id
            .as_deref()
            .is_none_or(|id| id.is_empty() || is_sender(id))
}

fn setup_primary_profile(
    mut commands: Commands,
    player: Query<(Entity, Option<&UserProfile>), With<PrimaryUser>>,
    mut current_profile: ResMut<CurrentUserProfile>,
    transports: Query<&Transport>,
    mut senders: Local<Vec<RpcEventSender>>,
    mut subscribe_events: EventReader<RpcCallEvent>,
    mut contexts: Query<&mut GlobalCrdtState>,
    mut cache: ProfileManager,
    mut last_announce: Local<f64>,
    mut announced: Local<Option<PublishedProfile>>,
    time: Res<Time>,
) {
    // gather any event receivers
    for sender in subscribe_events.read().filter_map(|ev| match &ev.call {
        RpcCall::SubscribeProfileChanged { sender } => Some(sender),
        _ => None,
    }) {
        senders.push(sender.clone());
    }

    // adopt the snapshots the server generated for our deployed profile: a change like any
    // other, so the component, scenes and the profile-changed stream all pick them up
    if let Some(snapshots) = current_profile
        .peer_profile()
        .and_then(|profile| cache.server_snapshots(profile))
    {
        current_profile.set_server_snapshots(snapshots);
    }

    if let Ok((player, maybe_profile)) = player.single() {
        if maybe_profile.is_none() || current_profile.is_changed() {
            let Some(profile) = current_profile.profile.as_ref() else {
                *announced = None;
                if maybe_profile.is_some() {
                    commands.entity(player).remove::<UserProfile>();
                    // logged out: scenes must stop seeing the previous account as `PLAYER`
                    for mut global_crdt in contexts.iter_mut() {
                        for component_id in [
                            SceneComponentId::PLAYER_IDENTITY_DATA,
                            SceneComponentId::AVATAR_BASE,
                            SceneComponentId::AVATAR_EQUIPPED_DATA,
                        ] {
                            global_crdt.delete_crdt(component_id, SceneEntityId::PLAYER);
                        }
                    }
                }
                return;
            };

            // update component
            commands.entity(player).try_insert(profile.clone());

            // send to scenes (every context: the local player is `PLAYER` in all of them)
            for mut global_crdt in contexts.iter_mut() {
                global_crdt.update_crdt(
                    SceneComponentId::PLAYER_IDENTITY_DATA,
                    CrdtType::LWW_ANY,
                    SceneEntityId::PLAYER,
                    &PbPlayerIdentityData {
                        address: profile.content.eth_address.clone(),
                        is_guest: !(profile.content.has_connected_web3.unwrap_or(false)),
                    },
                );
            }
        }

        let Some(peer) = current_profile.peer_profile() else {
            *announced = None;
            return;
        };
        let now = time.elapsed_secs_f64();
        if current_profile.is_changed() || announced.is_none() {
            let next = PublishedProfile::of(peer);
            if *announced != next {
                cache.update(peer.clone());
                announce_version(&transports, peer);
                *announced = next;
                *last_announce = now;
                senders.retain(|sender| {
                    let _ = sender.send(format!(
                        "{{\"ethAddress\":\"{}\",\"version\":\"{}\"}}",
                        peer.content.eth_address, peer.version
                    ));
                    !sender.is_closed()
                });
            }
        }
        if now > *last_announce + 5.0 {
            broadcast(
                transports.iter(),
                BroadcastTarget::PULSE,
                false,
                rfc4::AnnounceProfileVersion {
                    profile_version: peer.version,
                },
            );
            *last_announce = now;
        }
    }
}

fn drive_profile_deploys(
    mut current: ResMut<CurrentUserProfile>,
    mut deploys: Local<Vec<PendingDeploy>>,
    wallet: Res<Wallet>,
    ipfas: IpfsAssetServer,
    mut cache: ProfileManager,
) {
    let mut index = 0;
    while index < deploys.len() {
        let Some(result) = deploys[index].task.complete() else {
            index += 1;
            continue;
        };
        let PendingDeploy {
            address, submitted, ..
        } = deploys.swap_remove(index);
        let version = submitted.profile.version;
        let outcome = result.map_err(|error| {
            error!("failed to deploy profile v{version}: {error:#}");
            profile_deploy_message(&error)
        });
        let landed = outcome.is_ok();
        current.finish_deploy(submitted, outcome);
        if landed {
            info!("deployed profile v{version}");
            if let Some(known) = current.peer_profile().filter(|profile| {
                profile.version == version && profile.content.eth_address.as_h160() == Some(address)
            }) {
                cache.update(known.clone());
                cache.refetch_deployed(address, version);
            }
        }
    }

    let Some(address) = wallet.address() else {
        return;
    };
    if wallet.is_guest()
        || current
            .profile
            .as_ref()
            .and_then(|profile| profile.content.eth_address.as_h160())
            != Some(address)
        || deploys.iter().any(|deploy| deploy.address == address)
    {
        return;
    }
    if let Some(submitted) = current.bypass_change_detection().begin_deploy() {
        let task = IoTaskPool::get().spawn_compat(deploy_profile(
            ipfas.ipfs().clone(),
            wallet.snapshot(),
            submitted.profile.clone(),
        ));
        deploys.push(PendingDeploy {
            address,
            task,
            submitted,
        });
    }
}

struct PendingDeploy {
    address: Address,
    submitted: SubmittedProfile,
    task: Task<Result<(), anyhow::Error>>,
}

/// Announce `profile`'s version over Pulse, converted to a `ProfileVersionAnnouncement`.
fn announce_version(transports: &Query<&Transport>, profile: &UserProfile) {
    debug!("announcing profile new version {:?}", profile.version);
    let serialized_profile = match serde_json::to_string(&profile.content) {
        Ok(serialized_profile) => serialized_profile,
        Err(e) => {
            error_once!("not announcing profile, serialization failed: {e}");
            return;
        }
    };
    broadcast(
        transports.iter(),
        BroadcastTarget::PULSE,
        false,
        ProfileUpdate {
            serialized_profile,
            base_url: profile.base_url.clone(),
            version: profile.version,
        },
    );
}

#[derive(Resource, Default)]
pub struct CurrentUserProfile {
    pub profile: Option<UserProfile>,
    /// False while a profile needs submission or an existing submission is in flight.
    pub is_deployed: bool,
    /// A new profile made while the world is held: the host's welcome page deploys it, with its
    /// save after the terms are accepted.
    pub deploy_held: bool,
    pub(crate) saves: ProfileSaves,
    /// The highest version this session gave an edit of an account's profile. A deploy that
    /// failed here may still have reached the server (an error after the catalyst took it),
    /// and a peer that read it skips any other profile announced under the same number, so the
    /// next edit never reuses it.
    pub(crate) last_versions: HashMap<Address, u32>,
}

/// The base, active submission and edits waiting for the next payload, in arrival order.
#[derive(Default)]
pub(crate) struct ProfileSaves {
    session: u64,
    base: Option<UserProfile>,
    base_published: bool,
    active: Option<u32>,
    queued: Vec<ProfileEdit>,
}

type AvatarReply = RpcResultSender<SetAvatarResult>;

struct ProfileEdit {
    version: u32,
    patch: ProfilePatch,
}

enum ProfilePatch {
    Avatar(Box<SetAvatarData>, AvatarReply),
    MarkGuestAccount,
}

impl ProfilePatch {
    fn into_sender(self) -> Option<AvatarReply> {
        match self {
            Self::Avatar(_, sender) => Some(sender),
            Self::MarkGuestAccount => None,
        }
    }
}

struct SubmittedProfile {
    session: u64,
    profile: UserProfile,
    senders: Vec<AvatarReply>,
}

impl CurrentUserProfile {
    /// Give the edited profile a fresh version even when an earlier write failed.
    fn bump_version(&mut self) {
        let Some(profile) = self.profile.as_mut() else {
            return;
        };
        let address = profile.content.eth_address.as_h160();
        let Some(address) = address else { return };
        let used = self.last_versions.entry(address).or_default();
        profile.version = profile.version.max(*used) + 1;
        profile.content.version = i64::from(profile.version);
        *used = profile.version;
    }

    /// Keep the published profile, or the initial draft for an account creating its first profile.
    fn begin_edits(&mut self) {
        if self.saves.base.is_none() {
            self.saves.base = self.profile.clone();
            self.saves.base_published = self.is_deployed;
        }
    }

    /// Login marks authenticated guest profiles locally before persisting that classification.
    pub fn queue_guest_resave(&mut self) {
        self.begin_edits();
        self.bump_version();
        let Some(profile) = self.profile.as_ref() else {
            return;
        };
        self.saves.queued.push(ProfileEdit {
            version: profile.version,
            patch: ProfilePatch::MarkGuestAccount,
        });
        self.is_deployed = false;
    }

    /// Account transitions discard unsent patches. Submitted writes keep their original signer
    /// and receive the result of the request that owns them.
    pub fn reset_saves(&mut self) {
        let previous = std::mem::take(&mut self.saves);
        self.saves.session = previous.session.wrapping_add(1);
        for sender in previous
            .queued
            .into_iter()
            .filter_map(|edit| edit.patch.into_sender())
        {
            sender.send(Err(SetAvatarError {
                message: "Account changed before the profile was submitted.".to_owned(),
                account_changed: true,
                profile: None,
            }));
        }
    }

    fn begin_deploy(&mut self) -> Option<SubmittedProfile> {
        if self.is_deployed || self.deploy_held || self.saves.active.is_some() {
            return None;
        }
        self.begin_edits();
        let profile = self.profile.clone()?;
        self.saves.active = Some(profile.version);
        Some(SubmittedProfile {
            session: self.saves.session,
            profile,
            senders: self
                .saves
                .queued
                .drain(..)
                .filter_map(|edit| edit.patch.into_sender())
                .collect(),
        })
    }

    fn set_server_snapshots(&mut self, snapshots: AvatarSnapshots) {
        if let Some(base) = self.saves.base.as_mut() {
            base.content.avatar.snapshots = Some(snapshots.clone());
        }
        if let Some(profile) = self.profile.as_mut() {
            profile.content.avatar.snapshots = Some(snapshots);
        }
    }

    fn rebase_queued(&mut self) {
        let mut shown = self.saves.base.as_ref().unwrap().clone();
        for edit in &mut self.saves.queued {
            match &edit.patch {
                ProfilePatch::Avatar(patch, _) => apply_avatar_patch(&mut shown, patch),
                ProfilePatch::MarkGuestAccount => shown.content.has_connected_web3 = Some(false),
            }
            edit.version = edit.version.max(shown.version + 1);
            shown.version = edit.version;
            shown.content.version = i64::from(edit.version);
        }
        if let Some(address) = shown.content.eth_address.as_h160() {
            let used = self.last_versions.entry(address).or_default();
            *used = (*used).max(shown.version);
        }
        self.profile = Some(shown);
    }

    /// Only unsent patches need replaying. The active payload is immutable and no later
    /// payload for this account is signed until the active request finishes.
    fn finish_deploy(&mut self, submitted: SubmittedProfile, result: Result<(), String>) {
        let SubmittedProfile {
            session,
            profile,
            senders,
        } = submitted;
        let version = profile.version;
        let same_account = self.profile.as_ref().is_some_and(|current| {
            current
                .content
                .eth_address
                .eq_ignore_ascii_case(&profile.content.eth_address)
        });
        let active =
            same_account && self.saves.session == session && self.saves.active == Some(version);
        if active {
            self.saves.active = None;
            if result.is_ok()
                && self
                    .peer_profile()
                    .is_none_or(|known| known.version <= version)
            {
                self.saves.base = Some(profile);
                self.saves.base_published = true;
            }
            self.rebase_queued();
            self.is_deployed = self.saves.queued.is_empty();
            if self.is_deployed && self.saves.base_published {
                self.saves.base = None;
            }
        } else if result.is_ok() && same_account {
            // The prior login supplied the payload. The current login supplies its account kind.
            let mut profile = profile;
            profile.content.has_connected_web3 =
                self.profile.as_ref().unwrap().content.has_connected_web3;
            self.adopt_server_profile(profile);
        }
        for sender in senders {
            sender.send(
                result
                    .as_ref()
                    .map(|()| version)
                    .map_err(|message| SetAvatarError {
                        message: message.clone(),
                        account_changed: !same_account || self.saves.session != session,
                        profile: if same_account && self.saves.session == session {
                            self.peer_profile()
                                .map(|profile| Box::new(profile.content.clone()))
                        } else {
                            None
                        },
                    }),
            );
        }
    }

    /// A realm refresh or a completed write from a previous login can advance this account's
    /// confirmed base. Older replies cannot replace a newer known profile.
    pub fn adopt_server_profile(&mut self, profile: UserProfile) {
        if self.peer_profile().is_some_and(|known| {
            known
                .content
                .eth_address
                .eq_ignore_ascii_case(&profile.content.eth_address)
                && known.version > profile.version
        }) {
            return;
        }
        if self.saves.active.is_some() || !self.saves.queued.is_empty() {
            self.saves.base = Some(profile);
            self.saves.base_published = true;
            if self.saves.active.is_none() {
                self.rebase_queued();
            }
            return;
        }
        self.reset_saves();
        self.profile = Some(profile);
        self.is_deployed = true;
        self.deploy_held = false;
    }

    /// Peers and profile readers see the latest published version. A throwaway guest's
    /// local version is authoritative because that guest has no server profile.
    pub fn peer_profile(&self) -> Option<&UserProfile> {
        match &self.saves.base {
            Some(base) => self.saves.base_published.then_some(base),
            None if self.is_deployed => self.profile.as_ref(),
            None => None,
        }
    }
}

/// Run the fetch cascade for every entry that wants one: catalyst batches of the tier its
/// readers need, pacing unsatisfied entries with `PROFILE_FETCH_RETRY`. All fetch policy
/// lives here — the fetchers themselves are dumb HTTP. Entries nobody has asked about
/// recently are dropped here too, so departed peers and one-off lookups (unknown addresses
/// especially, which never satisfy) stop being re-fetched for the rest of the session.
fn drive_profile_fetches(mut manager: ProfileManager, wallet: Res<Wallet>) {
    let ProfileManager { cache, ipfs } = &mut manager;

    // apply landed compact batches; a catalyst without the route turns every fetch full
    let mut landed = Vec::new();
    cache
        .compact_batches
        .retain_mut(|batch| match batch.task.complete() {
            Some(result) => {
                landed.push((std::mem::take(&mut batch.addresses), result));
                false
            }
            None => true,
        });
    let now = web_time::Instant::now();
    for (addresses, result) in landed {
        match result {
            Ok(found) => {
                for (address, compact) in found {
                    if let Some(entry) = cache.entries.get_mut(&address) {
                        entry.apply_compact(compact, now);
                    }
                }
            }
            Err(CompactFetchError::Unsupported) => cache.compact_endpoint_missing(&addresses),
            Err(CompactFetchError::Failed(e)) => {
                debug!("compact profile fetch failed, reading those profiles whole: {e}");
                cache.compact_fetch_failed(&addresses);
            }
        }
    }

    let ProfileCache {
        entries, batches, ..
    } = &mut **cache;

    // apply landed batches; the cascade concludes here either way
    batches.retain_mut(|batch| {
        let Some(results) = batch.complete() else {
            return true;
        };
        for (address, result) in results {
            let Some(entry) = entries.get_mut(&address) else {
                continue;
            };
            entry.fetching = None;
            match result {
                Ok(Some(profile)) => {
                    entry.apply(profile, ProfileSource::Catalyst);
                }
                Ok(None) => debug!("catalyst has no profile for {address:#x}"),
                Err(e) => {
                    if entry.data.is_none() {
                        warn!("profile fetch failed for {address:#x}: {e}");
                    } else {
                        debug!("profile re-fetch failed for {address:#x}: {e}");
                    }
                }
            }
            if !entry.satisfied() {
                entry.next_fetch = Some(web_time::Instant::now() + PROFILE_FETCH_RETRY);
            }
        }
        false
    });

    // evict stale entries, except the local player's (so a same-version local edit isn't
    // replaced by a lagging catalyst answer), and collect due entries into batches
    let local = wallet.address();
    let compact_unsupported = cache.compact_unsupported;
    let mut full = Vec::new();
    let mut compact = Vec::new();
    cache.entries.retain(|address, entry| {
        if entry.evictable(now) && Some(*address) != local {
            debug!("evicting stale profile entry for {address:#x}");
            return false;
        }
        if entry.wants_fetch(now) {
            let tier = entry.fetch_tier(compact_unsupported);
            entry.fetching = Some(tier);
            match tier {
                ProfileTier::Full => full.push(*address),
                ProfileTier::Compact => compact.push(*address),
            }
        }
        true
    });
    if full.is_empty() && compact.is_empty() {
        return;
    }
    let lambdas = lambdas_url();
    let compact_base = compact_source(&lambdas);
    let ProfileCache {
        batches,
        compact_batches,
        requests,
        ..
    } = &mut **cache;
    for chunk in full.chunks(PROFILE_REQUEST_BATCH) {
        requests.last_batch = chunk.len();
        requests.addresses += chunk.len() as u64;
        if chunk.len() == 1 {
            requests.single += 1;
        } else {
            requests.aggregated += 1;
        }
        batches.push(IoTaskPool::get().spawn_compat(fetch_profiles(
            lambdas.clone(),
            chunk.to_vec(),
            ipfs.ipfs().clone(),
        )));
    }
    for chunk in compact.chunks(PROFILE_REQUEST_BATCH) {
        requests.compact += 1;
        compact_batches.push(CompactBatch {
            addresses: chunk.to_vec(),
            task: IoTaskPool::get().spawn_compat(fetch_compact_profiles(
                compact_base.clone(),
                chunk.to_vec(),
                ipfs.ipfs().clone(),
            )),
        });
    }
}

fn request_missing_profiles(
    mut commands: Commands,
    missing: Query<(Entity, &ForeignPlayer), Without<UserProfile>>,
    versioned: Query<(Entity, &ForeignPlayer, &UserProfile)>,
    mut manager: ProfileManager,
    mut contexts: Query<&mut GlobalCrdtState>,
    transports: Query<&Transport>,
    time: Res<Time>,
) {
    let now = time.elapsed_secs_f64();

    // resolved players: push cache movement, but diff before any push — only real
    // changes reach the entity/scenes
    for (ent, player, current) in versioned.iter() {
        manager.announce(player.address, player.profile_version);
        let Some(entry) = manager.cache.entries.get_mut(&player.address) else {
            continue;
        };

        if let Some(data) = entry.profile() {
            if data.version >= current.version && data != current {
                let was_guest = profile_is_guest(current);
                let is_guest = profile_is_guest(data);
                if was_guest != is_guest {
                    if let Ok(mut global_crdt) = contexts.get_mut(player.context) {
                        global_crdt.update_crdt(
                            SceneComponentId::PLAYER_IDENTITY_DATA,
                            CrdtType::LWW_ANY,
                            player.scene_id,
                            &PbPlayerIdentityData {
                                address: format!("{:#x}", player.address),
                                is_guest,
                            },
                        );
                    }
                }
                commands.entity(ent).try_insert(data.clone());
            }
        }

        if entry.wants_p2p(now) && send_profile_request(player, &transports) {
            entry.last_p2p = Some(now);
        }
    }

    // unresolved players: push the first data that lands, even when it's older than the
    // announced version — a name beats a truncated address, and the fetch cascade keeps
    // working toward the announced version
    for (ent, player) in missing.iter() {
        manager.announce(player.address, player.profile_version);
        let Some(entry) = manager.cache.entries.get_mut(&player.address) else {
            continue;
        };

        if let Some(data) = entry.profile() {
            if let Ok(mut global_crdt) = contexts.get_mut(player.context) {
                global_crdt.update_crdt(
                    SceneComponentId::PLAYER_IDENTITY_DATA,
                    CrdtType::LWW_ANY,
                    player.scene_id,
                    &PbPlayerIdentityData {
                        address: format!("{:#x}", player.address),
                        is_guest: profile_is_guest(data),
                    },
                );
            }
            commands.entity(ent).try_insert(data.clone());
        }

        if entry.wants_p2p(now) && send_profile_request(player, &transports) {
            entry.last_p2p = Some(now);
        }
    }
}

/// Answers `getCompactProfiles` (the HUD's list rows) once every address of a request has
/// settled. Runs after `drive_profile_fetches`, so a batch that lands answers in that frame.
fn serve_compact_profiles(
    mut requests: EventReader<RpcCallEvent>,
    mut pending: Local<Vec<(Vec<Address>, RpcResultSender<Vec<CompactProfile>>)>>,
    mut cache: ResMut<ProfileCache>,
) {
    pending.extend(requests.read().filter_map(|ev| {
        match &ev.call {
            RpcCall::GetCompactProfiles {
                addresses,
                response,
            } => Some((
                addresses
                    .iter()
                    .filter_map(|address| address.as_h160())
                    .collect(),
                response.clone(),
            )),
            _ => None,
        }
    }));
    pending.retain(
        |(addresses, response)| match cache.compact_answer(addresses) {
            Some(found) => {
                response.send(found);
                false
            }
            None => true,
        },
    );
}

/// Pick a transport at request time rather than remembering one. `player.transports` is the
/// set this peer is actually on — joined rooms are added, departures remove them — so any
/// non-Pulse member is a channel we provably share; Pulse itself is excluded because it
/// relays version announcements only (its bridge can't encode a `ProfileRequest`). Nothing
/// is cached, so a room joined later is picked up on the next retry.
fn send_profile_request(player: &ForeignPlayer, transports: &Query<&Transport>) -> bool {
    let Some(transport) = player
        .transports
        .iter()
        .filter_map(|transport| transports.get(*transport).ok())
        .find(|t| t.transport_type != TransportType::Pulse)
    else {
        return false;
    };

    let request = rfc4::Packet {
        message: Some(rfc4::packet::Message::ProfileRequest(
            rfc4::ProfileRequest {
                address: format!("{:#x}", player.address),
                profile_version: player.profile_version,
            },
        )),
        protocol_version: 100,
    };
    match transport.sender.try_send(NetworkMessage {
        recipient: NetworkMessageRecipient::Peer(player.address),
        ..NetworkMessage::unreliable(&request)
    }) {
        Err(e) => {
            warn!("failed to send request: {e}");
        }
        Ok(_) => {
            debug!("sent profile request for player {player:?}");
        }
    };
    true
}

/// The HUD also refetches when the server supplies thumbnails for a previously announced version.
#[derive(PartialEq)]
struct PublishedProfile {
    address: Address,
    version: u32,
    snapshots: Option<AvatarSnapshots>,
}

impl PublishedProfile {
    fn of(profile: &UserProfile) -> Option<Self> {
        Some(Self {
            address: profile.content.eth_address.as_h160()?,
            version: profile.version,
            snapshots: profile.content.avatar.snapshots.clone(),
        })
    }
}

/// Foreign profile components are authoritative. The local component includes optimistic edits,
/// so the local stream follows the account's published profile instead.
fn pipe_profile_changes_to_scene(
    mut requests: EventReader<SystemApi>,
    mut senders: Local<Vec<RpcStreamSender<ProfileChangedEvent>>>,
    changed: Query<(&ForeignPlayer, &UserProfile), Changed<UserProfile>>,
    current: Res<CurrentUserProfile>,
    mut published: Local<Option<PublishedProfile>>,
) {
    senders.extend(requests.read().filter_map(|ev| {
        if let SystemApi::GetProfileChangedStream(sender) = ev {
            Some(sender.clone())
        } else {
            None
        }
    }));
    senders.retain(|s| !s.is_closed());
    let notify = |address: Address, version| {
        let event = ProfileChangedEvent {
            address: format!("{address:#x}"),
            version,
        };
        for sender in senders.iter() {
            let _ = sender.send(event.clone());
        }
    };
    if current.is_changed() {
        let next = current.peer_profile().and_then(PublishedProfile::of);
        if *published != next {
            if let Some(next) = &next {
                notify(next.address, next.version);
            }
            *published = next;
        }
    }
    for (player, profile) in &changed {
        notify(player.address, profile.version);
    }
}

pub fn process_profile_events(
    mut commands: Commands,
    mut players: Query<(&mut ForeignPlayer, Option<&mut UserProfile>)>,
    mut events: EventReader<ProfileEvent>,
    mut last_sent_request: Local<HashMap<Entity, f64>>,
    time: Res<Time>,
    wallet: Res<Wallet>,
    transports: Query<&Transport>,
    current_profile: Res<CurrentUserProfile>,
    mut contexts: Query<&mut GlobalCrdtState>,
    mut cache: ProfileManager,
) {
    for ev in events.read() {
        match &ev.event {
            ProfileEventType::Request {
                request: r,
                transport: request_transport,
            } => {
                if let Some(req_address) = r.address.as_h160() {
                    if Some(req_address) == wallet.address() {
                        // Answer on the transport the request arrived on — the one channel we know
                        // we share with the asker. Still `recipient: All`: anyone else in that room
                        // needing our profile gets it for free, which is what the per-transport
                        // debounce below is rate-limiting.
                        let Ok(transport) = transports.get(*request_transport) else {
                            debug!("not sending profile, no transport");
                            continue;
                        };

                        if last_sent_request.get(request_transport).is_some() {
                            debug!("ignoring request for my profile (sent recently)");
                            continue;
                        }

                        // what peers may know about: not an edit still deploying
                        let Some(current_profile) = current_profile.peer_profile() else {
                            continue;
                        };

                        let serialized_profile =
                            match serde_json::to_string(&current_profile.content) {
                                Ok(serialized_profile) => serialized_profile,
                                Err(e) => {
                                    error_once!("not sending profile, serialization failed: {e}");
                                    continue;
                                }
                            };

                        debug!("sending my profile");
                        let response = rfc4::Packet {
                            message: Some(rfc4::packet::Message::ProfileResponse(
                                rfc4::ProfileResponse {
                                    serialized_profile,
                                    base_url: current_profile.base_url.clone(),
                                },
                            )),
                            protocol_version: 100,
                        };
                        let _ = transport
                            .sender
                            .try_send(NetworkMessage::reliable(&response));
                        last_sent_request.insert(*request_transport, time.elapsed_secs_f64());
                    }
                }
            }
            ProfileEventType::Version(v) => {
                if let Ok((mut player, _)) = players.get_mut(ev.sender) {
                    if player.profile_version != v.profile_version {
                        player.profile_version = v.profile_version;
                    }
                    // a version ahead of what we hold makes the fetch cascade due
                    cache.announce(player.address, v.profile_version);
                } else {
                    warn!("profile version for unknown player {:?}", ev.sender);
                }
            }
            ProfileEventType::Response(r) => {
                if let Ok((mut player, maybe_profile)) = players.get_mut(ev.sender) {
                    let serialized_profile: SerializedProfile =
                        match serde_json::from_str(&r.serialized_profile) {
                            Ok(p) => p,
                            Err(e) => {
                                warn!("failed to parse profile: {e}");
                                continue;
                            }
                        };
                    let version = serialized_profile.version as u32;

                    if version < player.profile_version {
                        continue;
                    }

                    let profile = UserProfile {
                        version,
                        content: serialized_profile,
                        base_url: r.base_url.clone(),
                    };

                    // dropped whole when it names another address: nothing of it reaches
                    // the sender's entity, version or identity either
                    if !cache.update_from_peer(player.address, profile.clone()) {
                        warn!(
                            "dropping a profile from {:#x} that names another address",
                            player.address
                        );
                        continue;
                    }

                    if version > player.profile_version {
                        player.profile_version = version;
                    }

                    // diff before any push: only write the identity crdt when the guest
                    // flag actually changes (or on first sight of this player's profile)
                    let is_guest = !(profile.content.has_connected_web3.unwrap_or(false));
                    let guest_changed = maybe_profile.as_ref().is_none_or(|existing| {
                        let was_guest = !existing.content.has_connected_web3.unwrap_or(false);
                        was_guest != is_guest
                    });
                    if guest_changed {
                        if let Ok(mut global_crdt) = contexts.get_mut(player.context) {
                            global_crdt.update_crdt(
                                SceneComponentId::PLAYER_IDENTITY_DATA,
                                CrdtType::LWW_ANY,
                                player.scene_id,
                                &PbPlayerIdentityData {
                                    address: format!("{:#x}", player.address),
                                    is_guest,
                                },
                            );
                        }
                    }

                    if let Some(mut existing_profile) = maybe_profile {
                        if existing_profile.as_ref() != &profile {
                            *existing_profile = profile;
                        }
                    } else {
                        commands.entity(ev.sender).try_insert(profile);
                    }
                } else {
                    warn!("profile update for unknown player {:?}", ev.sender);
                }
            }
        }
    }

    last_sent_request.retain(|_, req_time| *req_time > time.elapsed_secs_f64() - 10.0);
}

#[derive(Component, Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
pub struct UserProfile {
    pub version: u32,
    pub content: SerializedProfile,
    pub base_url: String,
}

impl UserProfile {
    pub fn is_female(&self) -> bool {
        self.content
            .avatar
            .body_shape
            .as_ref()
            .and_then(|s| s.rsplit(':').next())
            .is_none_or(|shape| shape.to_lowercase() == "basefemale")
    }

    /// A claimed name's chosen colour, else the colour hashed from the name.
    pub fn name_color(&self) -> Color {
        match self.content.name_color {
            Some(custom) if self.content.has_claimed_name => custom.convert_srgb(),
            // lowercased as the HUD has it: the tag's letters are part of what is hashed
            _ => name_color(
                &self.content.name,
                &self.content.eth_address.to_lowercase(),
                self.content.has_claimed_name,
            ),
        }
    }
}

#[derive(Serialize)]
pub struct Deployment<'a> {
    version: &'a str,
    #[serde(rename = "type")]
    ty: &'a str,
    pointers: Vec<String>,
    timestamp: u128,
    metadata: serde_json::Value,
}

/// Server bodies and failing request URLs may contain account data. Expose only the status or
/// the kind of failure in the save reply.
fn profile_deploy_message(error: &anyhow::Error) -> String {
    if let Some(status) = error.downcast_ref::<StatusCode>() {
        format!("The server rejected the profile deployment ({status}).")
    } else if let Some(http) = error.downcast_ref::<reqwest::Error>() {
        if http.is_timeout() {
            "The profile deployment timed out.".to_owned()
        } else {
            "Could not reach the profile deployment server.".to_owned()
        }
    } else {
        "Could not prepare or sign the profile deployment.".to_owned()
    }
}

async fn deploy_profile(
    ipfs: Arc<IpfsIo>,
    wallet: Wallet,
    mut profile: UserProfile,
) -> Result<(), anyhow::Error> {
    profile.content.avatar.snapshots = None;
    let pointer = profile.content.eth_address.clone();
    deploy_entity(
        ipfs,
        wallet,
        "profile",
        pointer,
        serde_json::json!({
            "avatars": [
                profile.content
            ]
        }),
    )
    .await
}

/// Signs and deploys a file-less entity of type `ty` on `pointer` to the realm's catalyst.
pub(crate) async fn deploy_entity(
    ipfs: Arc<IpfsIo>,
    wallet: Wallet,
    ty: &'static str,
    pointer: String,
    metadata: serde_json::Value,
) -> Result<(), anyhow::Error> {
    let unix_time = web_time::SystemTime::now()
        .duration_since(web_time::UNIX_EPOCH)
        .unwrap()
        .as_millis();

    let deployment = serde_json::to_string(&Deployment {
        version: "v3",
        ty,
        pointers: vec![pointer],
        timestamp: unix_time,
        metadata,
    })?;

    let post = {
        let hash = multihash_codetable::Code::Sha2_256.digest(deployment.as_bytes());
        let cid = cid::Cid::new_v1(0x55, hash).to_string();
        let chain = wallet.sign_message(cid.clone()).await?;

        let mut form_data = multipart::client::lazy::Multipart::new();
        form_data.add_text("entityId", cid.clone());
        for (key, data) in chain.formdata() {
            form_data.add_text(key, data);
        }
        form_data.add_stream(
            cid,
            std::io::Cursor::new(deployment.into_bytes()),
            Option::<&str>::None,
            None,
        );

        let mut prepared = form_data.prepare()?;
        let mut prepared_data = Vec::default();
        prepared.read_to_end(&mut prepared_data)?;

        let url = ipfs
            .entities_endpoint()
            .ok_or_else(|| anyhow!("no entities endpoint"))?;
        debug!("deploying {ty} to {url}");

        ipfs.client()
            .post(url)
            .timeout(std::time::Duration::from_secs(60))
            .header(
                "Content-Type",
                format!("multipart/form-data; boundary={}", prepared.boundary()),
            )
            .body(prepared_data)
    };

    let response = send_deployment(post).await?;

    let status = response.status();
    if status == StatusCode::OK {
        return Ok(());
    }
    let body = response.bytes().await?;
    Err(anyhow::Error::msg(status).context(format!(
        "bad response: {status}: {}",
        String::from_utf8_lossy(&body)
    )))
}

/// Send a deployment, and send it once more when the catalyst refuses it for now with its word on
/// when to come back (`Retry-After` on a 429 or a 503, up to `platform::http::MAX_RETRY_AFTER`):
/// the server's own instruction, where reporting the save as failed would leave the user a change
/// a moment's wait makes (a client's burst of deployments past the catalyst's limit).
async fn send_deployment(
    post: reqwest::RequestBuilder,
) -> Result<reqwest::Response, anyhow::Error> {
    let again = post.try_clone();
    let response = post.send().await?;
    let delay = matches!(
        response.status(),
        StatusCode::TOO_MANY_REQUESTS | StatusCode::SERVICE_UNAVAILABLE
    )
    .then(|| response.headers().get(reqwest::header::RETRY_AFTER))
    .flatten()
    .and_then(|value| value.to_str().ok())
    .and_then(|value| {
        let now = web_time::SystemTime::now()
            .duration_since(web_time::UNIX_EPOCH)
            .ok()?;
        platform::http::parse_retry_after(value, std::time::UNIX_EPOCH + now)
    })
    .filter(|delay| *delay <= platform::http::MAX_RETRY_AFTER);
    match (delay, again) {
        (Some(delay), Some(again)) => {
            warn!(
                "the catalyst refused the deployment for now ({}): sending it again in {delay:?}",
                response.status()
            );
            async_std::task::sleep(delay).await;
            Ok(again.send().await?)
        }
        _ => Ok(response),
    }
}

/// The lambdas of the base domain's catalyst: where full profiles are read, and compact ones
/// off Decentraland's own deployments ([`compact_source`]).
fn lambdas_url() -> String {
    common::base_domain::url(common::base_domain::Service::Catalyst, "/lambdas")
}

/// Where compact profiles are read (`POST {source}/profiles/metadata`): the catalyst's lambdas,
/// except when that catalyst is one of Decentraland's own deployments
/// ([`common::base_domain::decentraland_domain`]), whose catalysts have no compact route. There
/// the same deployment's asset-bundle registry serves the tier. A dcl-one page or a `--gateway`
/// launch keeps the default base domain with its own catalyst, so the resolved catalyst selects
/// the source.
///
/// Temporary: the registry branch is to be removed once Decentraland's catalysts answer
/// `POST /lambdas/profiles/metadata` (docs/adr/2026-11-04-explorer-profiles-from-the-catalyst.md,
/// "Follow-ups"); compact reads then go to the catalyst everywhere.
fn compact_source(lambdas: &str) -> String {
    match common::base_domain::decentraland_domain(lambdas) {
        Some(domain) => format!("https://asset-bundle-registry.{domain}"),
        None => lambdas.trim_end_matches('/').to_owned(),
    }
}

/// The content endpoint of the catalyst serving `lambdas`, which carries the profile's files.
fn content_base(lambdas: &str) -> Option<String> {
    lambdas
        .trim_end_matches('/')
        .strip_suffix("/lambdas")
        .map(|catalyst| format!("{catalyst}/content/contents/"))
}

/// One catalyst POST for a chunk of ids, reported per item: Ok(Some) found, Ok(None)
/// authoritatively absent, Err a chunk-wide transport/parse failure.
async fn fetch_profiles(
    lambdas: String,
    addresses: Vec<Address>,
    ipfs: Arc<IpfsIo>,
) -> Vec<(Address, Result<Option<UserProfile>, anyhow::Error>)> {
    let url = format!("{}/profiles", lambdas.trim_end_matches('/'));
    let ids = addresses
        .iter()
        .map(|address| format!("{address:#x}"))
        .collect::<Vec<_>>();
    debug!("requesting {} profiles from {url}", ids.len());

    let outcome: Result<Vec<LambdaProfiles>, anyhow::Error> = async {
        let response = platform::TracedJsonResponse::send(
            ipfs.client()
                .post(&url)
                .timeout(std::time::Duration::from_secs(10))
                .body(serde_json::json!({ "ids": ids }).to_string())
                .header("content-type", "application/json"),
        )
        .await?;
        if response.status() != StatusCode::OK {
            anyhow::bail!("bad response: {}", response.discard());
        }
        response.json::<Vec<LambdaProfiles>>().await
    }
    .await;

    match outcome {
        Err(e) => {
            let message = format!("profile fetch from {url}: {e}");
            addresses
                .into_iter()
                .map(|address| (address, Err(anyhow!(message.clone()))))
                .collect()
        }
        Ok(batches) => {
            let base_url = content_base(&lambdas)
                .unwrap_or_else(|| ipfs.contents_endpoint().unwrap_or_default());
            profiles_from_lambdas(batches, addresses, &base_url)
                .into_iter()
                .map(|(address, profile)| (address, Ok(profile)))
                .collect()
        }
    }
}

/// The asked addresses, in order, each with the profile the response holds for it. A
/// profile is matched by its deployed metadata, which is written by whoever deployed it,
/// so only asked-for addresses are kept and a later entry never displaces an earlier one.
fn profiles_from_lambdas(
    batches: Vec<LambdaProfiles>,
    addresses: Vec<Address>,
    base_url: &str,
) -> Vec<(Address, Option<UserProfile>)> {
    let mut found = HashMap::<Address, SerializedProfile>::default();
    for batch in batches {
        let Some(content) = batch.avatars.into_iter().next() else {
            continue;
        };
        let Some(address) = content.eth_address.as_h160() else {
            continue;
        };
        found.entry(address).or_insert(content);
    }
    addresses
        .into_iter()
        .map(|address| {
            let profile = found.remove(&address).map(|content| UserProfile {
                version: content.version as u32,
                content,
                base_url: base_url.to_owned(),
            });
            (address, profile)
        })
        .collect()
}

/// One POST of the compact tier to the [`compact_source`] for a chunk of ids: the asked
/// addresses, in order, each with what the source holds for it. A source without the route
/// (a catalyst answers 404, or 405 when its `/profiles/{id}` takes the path) is `Unsupported`.
async fn fetch_compact_profiles(
    source: String,
    addresses: Vec<Address>,
    ipfs: Arc<IpfsIo>,
) -> CompactAnswer {
    let url = format!("{}/profiles/metadata", source.trim_end_matches('/'));
    let ids = addresses
        .iter()
        .map(|address| format!("{address:#x}"))
        .collect::<Vec<_>>();
    debug!("requesting {} compact profiles from {url}", ids.len());

    let response = platform::TracedJsonResponse::send(
        ipfs.client()
            .post(&url)
            .timeout(std::time::Duration::from_secs(10))
            .body(serde_json::json!({ "ids": ids }).to_string())
            .header("content-type", "application/json"),
    )
    .await
    .map_err(|e| CompactFetchError::Failed(anyhow!("compact profile fetch from {url}: {e}")))?;
    if let Err(e) = compact_status(&url, response.status()) {
        response.discard();
        return Err(e);
    }
    let found = response
        .json::<Vec<CompactProfile>>()
        .await
        .map_err(|e| CompactFetchError::Failed(anyhow!("compact profile fetch from {url}: {e}")))?;
    Ok(compact_profiles_for(found, addresses))
}

/// What a compact POST's status says: 404 or 405 is a catalyst without the route (405 where its
/// `/lambdas/profiles/{id}` takes the path), any other status but 200 a failure of this request
/// alone.
fn compact_status(url: &str, status: StatusCode) -> Result<(), CompactFetchError> {
    match status {
        StatusCode::OK => Ok(()),
        StatusCode::NOT_FOUND | StatusCode::METHOD_NOT_ALLOWED => {
            Err(CompactFetchError::Unsupported)
        }
        status => Err(CompactFetchError::Failed(anyhow!(
            "compact profile fetch from {url}: bad response: {status}"
        ))),
    }
}

/// The asked addresses, in order, each with the compact profile whose `pointer` is that
/// address (written as the lowercase address, however the server cased it, as
/// `CompactProfile::from` does); a later entry never displaces an earlier one.
fn compact_profiles_for(
    found: Vec<CompactProfile>,
    addresses: Vec<Address>,
) -> Vec<(Address, Option<CompactProfile>)> {
    let mut by_address = HashMap::<Address, CompactProfile>::default();
    for mut profile in found {
        if let Some(address) = profile.pointer.as_h160() {
            profile.pointer = format!("{address:#x}");
            by_address.entry(address).or_insert(profile);
        }
    }
    addresses
        .into_iter()
        .map(|address| (address, by_address.remove(&address)))
        .collect()
}

async fn fetch_catalyst_profile_by_id(
    endpoint: String,
    id: String,
    ipfs: std::sync::Arc<IpfsIo>,
) -> Result<Option<UserProfile>, anyhow::Error> {
    let url = format!("{}/profiles/{id}", endpoint.trim_end_matches('/'));
    debug!("requesting profile from {url}");

    let response = ipfs
        .client()
        .get(&url)
        .timeout(std::time::Duration::from_secs(10))
        .send()
        .await
        .map_err(|e| anyhow!("catalyst fetch from {url}: {e}"))?;
    if response.status() == StatusCode::NOT_FOUND {
        return Ok(None);
    }
    if response.status() != StatusCode::OK {
        anyhow::bail!(
            "catalyst fetch from {url}: bad response: {}",
            response.status()
        );
    }

    let body = response.text().await?;
    let content = match serde_json::from_str::<LambdaProfiles>(&body) {
        Ok(content) => content,
        Err(e) => {
            // the sdk preview server reports a missing profile as a 200 with an
            // `{"error": ...}` body rather than a 404; treat it as authoritatively absent
            if serde_json::from_str::<serde_json::Value>(&body)
                .is_ok_and(|value| value.get("error").is_some())
            {
                return Ok(None);
            }
            return Err(anyhow!("catalyst fetch from {url}: {e}"));
        }
    };
    // the profile's content is on the catalyst it came from, which need not be the realm's
    let base_url =
        content_base(&endpoint).unwrap_or_else(|| ipfs.contents_endpoint().unwrap_or_default());
    Ok(content
        .avatars
        .into_iter()
        .next()
        .map(|content| UserProfile {
            version: content.version as u32,
            content,
            base_url,
        }))
}

/// The curated default looks (avatar-assets `default-profiles`), deployed as profiles under
/// the pointers `default1` to `default{DEFAULT_LOOKS}`. Fetched from the base-domain catalyst
/// like the base wearables: not every realm's catalyst carries them.
pub const DEFAULT_LOOKS: u32 = 160;

pub async fn get_default_look(
    ipfs: std::sync::Arc<IpfsIo>,
    index: u32,
) -> Result<Option<UserProfile>, anyhow::Error> {
    fetch_catalyst_profile_by_id(lambdas_url(), format!("default{index}"), ipfs).await
}

/// Resolve one profile from the centralized catalyst without the engine cache (used at login,
/// before any player entities exist). Ok(None) means the catalyst authoritatively has no
/// profile, so callers can tell "no profile" from "couldn't fetch".
pub async fn get_remote_profile(
    address: Address,
    ipfs: Arc<IpfsIo>,
) -> Result<Option<UserProfile>, anyhow::Error> {
    fetch_profiles(lambdas_url(), vec![address], ipfs)
        .await
        .pop()
        .map_or(Ok(None), |(_, result)| result)
}

fn apply_avatar_patch(profile: &mut UserProfile, set_avatar: &SetAvatarData) {
    if let Some(base) = &set_avatar.base {
        // As with the colors below, a base that doesn't carry a body shape must not strip the
        // profile's — a caller editing only the name has no reason to restate it.
        if !base.body_shape_urn.is_empty() {
            profile.content.avatar.body_shape = Some(base.body_shape_urn.clone());
        }

        // a base without colors must not strip them from the profile
        if let Some(hair) = base.hair_color {
            profile.content.avatar.hair = Some(AvatarColor::new(hair));
        }
        if let Some(eyes) = base.eyes_color {
            profile.content.avatar.eyes = Some(AvatarColor::new(eyes));
        }
        if let Some(skin) = base.skin_color {
            profile.content.avatar.skin = Some(AvatarColor::new(skin));
        }

        if !base.name.is_empty() {
            profile.content.name = base.name.clone();
            profile.content.avatar.name = Some(base.name.clone());
            if let Some(claimed) = set_avatar.has_claimed_name {
                profile.content.has_claimed_name = claimed;
            }
        }
    }

    if let Some(equip) = &set_avatar.equip {
        profile.content.avatar.wearables = equip.wearable_urns.to_vec();
        profile.content.avatar.emotes = Some(
            equip
                .emote_urns
                .iter()
                .enumerate()
                .flat_map(|(ix, e)| {
                    (!e.is_empty()).then_some(AvatarEmote {
                        slot: ix as u32,
                        urn: if e.starts_with("urn:decentraland:off-chain:base-emotes") {
                            e.rsplit_once(':').unwrap().1.to_string()
                        } else {
                            e.clone()
                        },
                    })
                })
                .collect(),
        );
        profile.content.avatar.force_render = Some(equip.force_render.to_vec());
    }

    if let Some(extras) = &set_avatar.profile_extras {
        // Merged per key, not assigned: `extra_fields` is a catch-all for every profile key we
        // don't model (and for keys written by other explorers), so a caller that edits one
        // field must not have to restate the rest to avoid deleting them. An explicit `null`
        // removes a key — the only way back out, now that omission means "leave alone".
        for (key, value) in extras {
            if value.is_null() {
                profile.content.extra_fields.remove(key);
            } else {
                profile
                    .content
                    .extra_fields
                    .insert(key.clone(), value.clone());
            }
        }
    }

    if let Some(name_color) = set_avatar.name_color {
        profile.content.name_color = name_color.to_color3();
    }
}

fn process_profile(
    mut requests: EventReader<SystemApi>,
    mut current: ResMut<CurrentUserProfile>,
    config: Res<AppConfig>,
    wallet: Res<Wallet>,
) {
    for request in requests.read() {
        let SystemApi::SetAvatar(expected, patch, sender) = request else {
            continue;
        };
        let address = current
            .profile
            .as_ref()
            .and_then(|profile| profile.content.eth_address.as_h160());
        if address.is_none() || address != wallet.address() || address != expected.as_h160() {
            sender.send(Err(SetAvatarError {
                message: "No profile for the active account.".to_owned(),
                account_changed: true,
                profile: None,
            }));
            continue;
        }
        if current.deploy_held && config.terms_accepted() {
            current.deploy_held = false;
        }
        let deploys = !wallet.is_guest();
        if deploys {
            current.begin_edits();
        }
        apply_avatar_patch(current.profile.as_mut().unwrap(), patch);
        current.bump_version();
        if deploys {
            let version = current.profile.as_ref().unwrap().version;
            current.saves.queued.push(ProfileEdit {
                version,
                patch: ProfilePatch::Avatar(Box::new(patch.clone()), sender.clone()),
            });
            current.is_deployed = false;
        } else {
            sender.send(Ok(u32::MAX));
        }
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod deploy_tests {
    use std::io::{Read, Write};

    use super::*;

    const OK: &[u8] = b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\nConnection: close\r\n\r\n";

    fn refused(retry_after: Option<&str>) -> Vec<u8> {
        let header = retry_after
            .map(|s| format!("Retry-After: {s}\r\n"))
            .unwrap_or_default();
        format!("HTTP/1.1 429 Too Many Requests\r\n{header}Content-Length: 0\r\nConnection: close\r\n\r\n").into_bytes()
    }

    /// A server answering `answers` in turn; it returns the bodies it was sent.
    fn serve(answers: Vec<Vec<u8>>) -> (String, std::thread::JoinHandle<Vec<Vec<u8>>>) {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/content/entities", listener.local_addr().unwrap());
        let server = std::thread::spawn(move || {
            let mut bodies = Vec::new();
            for answer in answers {
                let (mut socket, _) = listener.accept().unwrap();
                let mut request = Vec::new();
                let mut chunk = [0u8; 1024];
                let body = loop {
                    let n = socket.read(&mut chunk).unwrap();
                    request.extend_from_slice(&chunk[..n]);
                    let Some(end) = request.windows(4).position(|w| w == b"\r\n\r\n") else {
                        continue;
                    };
                    let head = String::from_utf8_lossy(&request[..end]).to_ascii_lowercase();
                    let length: usize = head
                        .lines()
                        .find_map(|line| line.strip_prefix("content-length:"))
                        .map(|value| value.trim().parse().unwrap())
                        .unwrap_or(0);
                    while request.len() < end + 4 + length {
                        let n = socket.read(&mut chunk).unwrap();
                        request.extend_from_slice(&chunk[..n]);
                    }
                    break request[end + 4..end + 4 + length].to_vec();
                };
                bodies.push(body);
                socket.write_all(&answer).unwrap();
            }
            bodies
        });
        (url, server)
    }

    fn deployment(url: &str) -> reqwest::RequestBuilder {
        reqwest::Client::new()
            .post(url)
            .body(b"the deployment".to_vec())
    }

    #[tokio::test]
    async fn a_deployment_refused_for_now_is_sent_again_once_the_server_says() {
        let (url, server) = serve(vec![refused(Some("0")), OK.to_vec()]);
        let response = send_deployment(deployment(&url)).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(server.join().unwrap(), vec![b"the deployment".to_vec(); 2]);
    }

    #[tokio::test]
    async fn a_refusal_without_a_wait_or_past_the_bound_is_the_answer() {
        for retry_after in [None, Some("3600")] {
            let (url, server) = serve(vec![refused(retry_after)]);
            let response = send_deployment(deployment(&url)).await.unwrap();
            assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
            assert_eq!(server.join().unwrap().len(), 1);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn evicts_only_stale_idle_entries() {
        let now = web_time::Instant::now();
        let mut entry = ProfileEntry {
            last_access: Some(now),
            ..Default::default()
        };

        assert!(!entry.evictable(now + PROFILE_EVICT_AFTER / 2));
        assert!(entry.evictable(now + PROFILE_EVICT_AFTER));

        entry.fetching = Some(ProfileTier::Full);
        assert!(!entry.evictable(now + PROFILE_EVICT_AFTER));
    }

    #[test]
    fn touch_refreshes_access() {
        let address = Address::ZERO;
        let mut cache = ProfileCache::default();
        cache.touch(address, ProfileTier::Full).last_access = None;
        let now = web_time::Instant::now();
        assert!(cache.entries[&address].evictable(now));

        cache.touch(address, ProfileTier::Full);
        assert!(!cache.entries[&address].evictable(now));
    }

    fn profile(version: u32, name: &str) -> UserProfile {
        UserProfile {
            version,
            content: SerializedProfile {
                name: name.to_owned(),
                version: version as i64,
                ..Default::default()
            },
            ..Default::default()
        }
    }

    fn held_name(entry: &ProfileEntry) -> &str {
        &entry.profile().unwrap().content.name
    }

    #[test]
    fn refetch_deployed_reads_back_own_profile() {
        let now = web_time::Instant::now();
        // as `ProfileManager::update` records a local edit
        let mut entry = ProfileEntry {
            announced: 5,
            data: Some((Box::new(profile(5, "local")), ProfileSource::Catalyst)),
            wanted: ProfileTier::Full,
            ..Default::default()
        };
        assert!(!entry.wants_fetch(now));

        entry.refetch_deployed(5);
        assert!(entry.wants_fetch(now));
        assert_eq!(entry.profile().unwrap().content.version, 5);

        // a lagging answer ties and loses, and the read-back stays due
        assert!(!entry.apply(profile(4, "stale"), ProfileSource::Catalyst));
        assert_eq!(held_name(&entry), "local");
        assert!(!entry.satisfied());

        // the deployed version's answer displaces the local copy and settles it
        assert!(entry.apply(profile(5, "catalyst"), ProfileSource::Catalyst));
        assert!(entry.satisfied());
        assert_eq!(held_name(&entry), "catalyst");
    }

    #[test]
    fn own_profile_is_read_back_after_its_deploy() {
        let address = Address::repeat_byte(7);
        let mut own = profile(5, "local");
        own.content.eth_address = format!("{address:#x}");
        let mut cache = ProfileCache::default();
        cache.update(own.clone());
        cache.entries.get_mut(&address).unwrap().refetch_deployed(5);
        // Publishing the same local profile to scene readers must leave the fetch due.
        cache.update(own);
        let entry = &cache.entries[&address];
        assert!(entry.wants_fetch(web_time::Instant::now()));
        assert_eq!(entry.fetch_tier(false), ProfileTier::Full);
    }

    #[test]
    fn catalyst_answer_satisfies_the_announced_version() {
        let mut entry = ProfileEntry {
            announced: 5,
            wanted: ProfileTier::Full,
            ..Default::default()
        };
        entry.apply(profile(4, "behind"), ProfileSource::Catalyst);
        assert!(!entry.satisfied());
        entry.apply(profile(5, "current"), ProfileSource::Catalyst);
        assert!(entry.satisfied());

        // a non-guest peer's own answer is only a stand-in until the catalyst confirms it
        let mut peer = ProfileEntry {
            announced: 5,
            wanted: ProfileTier::Full,
            ..Default::default()
        };
        let mut connected = profile(5, "peer");
        connected.content.has_connected_web3 = Some(true);
        peer.apply(connected, ProfileSource::Peer);
        assert!(!peer.satisfied());

        // so is a guest account's: its profile is on the catalyst too
        let mut account = ProfileEntry {
            announced: 5,
            wanted: ProfileTier::Full,
            ..Default::default()
        };
        let mut guest_account = profile(5, "guest account");
        guest_account.content.has_connected_web3 = Some(false);
        account.apply(guest_account, ProfileSource::Peer);
        assert!(!account.satisfied());

        // a throwaway guest's own answer is final: the catalyst never holds it
        let mut throwaway = ProfileEntry {
            announced: 5,
            wanted: ProfileTier::Full,
            ..Default::default()
        };
        throwaway.apply(profile(5, "throwaway"), ProfileSource::Peer);
        assert!(throwaway.satisfied());
    }

    fn profile_of(address: Address, version: u32, name: &str) -> UserProfile {
        let mut profile = profile(version, name);
        profile.content.eth_address = format!("{address:#x}");
        profile
    }

    #[test]
    fn a_peer_profile_naming_another_address_is_not_recorded() {
        let (sender, other) = (Address::repeat_byte(0xa1), Address::repeat_byte(0xb2));
        let mut cache = ProfileCache::default();

        // at a version no catalyst answer would displace
        assert!(!cache.update_from_peer(sender, profile_of(other, u32::MAX, "forged")));

        let mut other_user_id = profile_of(sender, 1, "forged");
        other_user_id.content.user_id = Some(format!("{other:#x}"));
        assert!(!cache.update_from_peer(sender, other_user_id));

        let mut no_address = profile(1, "forged");
        no_address.content.eth_address = String::new();
        assert!(!cache.update_from_peer(sender, no_address));

        assert!(cache.entries.is_empty());
    }

    #[test]
    fn a_peer_profile_is_recorded_under_its_sender() {
        let sender = Address::repeat_byte(0xa1);
        let mut cache = ProfileCache::default();

        // however the sender spells its address, and with or without a user id
        let upper = format!("{sender:#x}").to_uppercase();
        assert!(upper.starts_with("0XA1"));
        let spellings = [
            (format!("{sender:#x}"), None),
            (upper.clone(), Some(upper)),
            (format!("{sender:x}"), Some(String::new())),
        ];
        for (version, (eth_address, user_id)) in (1..).zip(spellings) {
            let mut own = profile(version, "peer");
            own.content.eth_address = eth_address;
            own.content.user_id = user_id;
            assert!(cache.update_from_peer(sender, own));
            assert_eq!(cache.entries[&sender].profile().unwrap().version, version);
        }
        assert_eq!(held_name(&cache.entries[&sender]), "peer");
        assert_eq!(cache.entries.len(), 1);

        // the catalyst's answer at that version replaces it, and is not replaced back
        let entry = cache.entries.get_mut(&sender).unwrap();
        assert!(entry.apply(profile_of(sender, 3, "catalyst"), ProfileSource::Catalyst));
        assert!(cache.update_from_peer(sender, profile_of(sender, 3, "peer")));
        assert_eq!(held_name(&cache.entries[&sender]), "catalyst");
    }

    #[test]
    fn profiles_from_lambdas_matches_by_address() {
        let address = |n: u8| Address::repeat_byte(n);
        let lambdas = |n: u8, version: i64| LambdaProfiles {
            avatars: vec![SerializedProfile {
                eth_address: format!("{:#x}", address(n)),
                version,
                ..Default::default()
            }],
        };
        let batches = vec![lambdas(2, 7), lambdas(1, 3), lambdas(9, 1), lambdas(1, 4)];

        let found = profiles_from_lambdas(
            batches,
            vec![address(1), address(2), address(3)],
            "https://peer.example/content/contents/",
        );
        let versions: Vec<_> = found
            .iter()
            .map(|(address, profile)| (*address, profile.as_ref().map(|p| p.version)))
            .collect();
        assert_eq!(
            versions,
            vec![
                (address(1), Some(3)),
                (address(2), Some(7)),
                (address(3), None)
            ]
        );
        assert_eq!(
            found[0].1.as_ref().unwrap().base_url,
            "https://peer.example/content/contents/"
        );
    }

    #[test]
    fn content_base_is_the_catalyst_of_the_lambdas() {
        assert_eq!(
            content_base("https://peer.example/lambdas/").as_deref(),
            Some("https://peer.example/content/contents/")
        );
        assert_eq!(content_base("https://peer.example/other"), None);
    }

    /// Full profiles come from the catalyst everywhere; compact ones from the same catalyst
    /// except on Decentraland's own deployments, whose catalysts lack the route: there from the
    /// same deployment's asset-bundle registry (temporary, see `compact_source`).
    #[test]
    fn compact_reads_use_the_registry_only_for_decentraland_catalysts() {
        // the engine's own resolution: nothing overridden, so Decentraland's catalyst
        assert_eq!(lambdas_url(), "https://peer.decentraland.org/lambdas");
        assert_eq!(
            compact_source(&lambdas_url()),
            "https://asset-bundle-registry.decentraland.org"
        );
        for (lambdas, compact) in [
            (
                "https://peer.decentraland.zone/lambdas",
                "https://asset-bundle-registry.decentraland.zone",
            ),
            (
                "https://peer-ec1.decentraland.org/lambdas",
                "https://asset-bundle-registry.decentraland.org",
            ),
            // a dcl-one node, a gateway or a local catalyst serves its own compact tier
            (
                "https://play.example.test/lambdas",
                "https://play.example.test/lambdas",
            ),
            (
                "https://gateway.example.test/mount/peer/lambdas/",
                "https://gateway.example.test/mount/peer/lambdas",
            ),
            (
                "http://127.0.0.1:5222/lambdas",
                "http://127.0.0.1:5222/lambdas",
            ),
            (
                "https://peer.decentraland.org.example.test/lambdas",
                "https://peer.decentraland.org.example.test/lambdas",
            ),
        ] {
            assert_eq!(compact_source(lambdas), compact, "{lambdas}");
        }
    }

    fn compact(name: &str) -> CompactProfile {
        CompactProfile {
            pointer: format!("{:#x}", Address::ZERO),
            name: name.to_owned(),
            has_claimed_name: false,
            name_color: None,
            thumbnail_url: String::new(),
        }
    }

    #[test]
    fn compact_read_is_satisfied_by_either_tier() {
        let now = web_time::Instant::now();
        let mut compact_only = ProfileEntry::default();
        assert!(compact_only.wants_fetch(now));
        compact_only.apply_compact(Some(compact("row")), now);
        assert!(compact_only.satisfied());

        let mut full_only = ProfileEntry::default();
        full_only.apply(profile(1, "full"), ProfileSource::Catalyst);
        assert!(full_only.satisfied());

        // a full reader is not served by the compact answer
        compact_only.want(ProfileTier::Full);
        assert!(!compact_only.satisfied());
        assert!(compact_only.wants_fetch(now));
    }

    #[test]
    fn compact_answer_never_replaces_full_data() {
        let now = web_time::Instant::now();
        let mut entry = ProfileEntry::default();
        entry.apply(profile(3, "full"), ProfileSource::Catalyst);
        entry.apply_compact(Some(compact("row")), now);
        assert_eq!(held_name(&entry), "full");
        assert!(entry.compact.is_none());

        // full data landing drops a compact answer held before it
        let mut entry = ProfileEntry::default();
        entry.apply_compact(Some(compact("row")), now);
        assert!(entry.compact.is_some());
        entry.apply(profile(3, "full"), ProfileSource::Catalyst);
        assert!(entry.compact.is_none());
    }

    #[test]
    fn a_full_read_promotes_a_compact_entry() {
        let now = web_time::Instant::now();
        let mut entry = ProfileEntry {
            fetching: Some(ProfileTier::Compact),
            ..Default::default()
        };
        entry.want(ProfileTier::Full);
        entry.apply_compact(Some(compact("row")), now);
        assert_eq!(entry.next_fetch, None);
        assert!(entry.wants_fetch(now));
        assert_eq!(entry.fetch_tier(false), ProfileTier::Full);

        // a compact read the catalyst can't answer waits out the retry instead
        let mut absent = ProfileEntry {
            fetching: Some(ProfileTier::Compact),
            ..Default::default()
        };
        absent.apply_compact(None, now);
        assert!(!absent.wants_fetch(now));
        assert!(absent.missing(ProfileTier::Compact, now));
    }

    #[test]
    fn compact_requests_fall_back_to_full_after_a_missing_endpoint() {
        let now = web_time::Instant::now();
        let mut cache = ProfileCache::default();
        let entry = cache.touch(Address::ZERO, ProfileTier::Compact);
        entry.fetching = Some(ProfileTier::Compact);
        entry.next_fetch = Some(now + PROFILE_FETCH_RETRY);

        cache.compact_endpoint_missing(&[Address::ZERO]);
        assert!(cache.compact_unsupported);
        let entry = &cache.entries[&Address::ZERO];
        assert!(entry.wants_fetch(now));
        assert_eq!(
            entry.fetch_tier(cache.compact_unsupported),
            ProfileTier::Full
        );
    }

    fn own(version: u32, name: &str) -> UserProfile {
        let mut profile = profile(version, name);
        profile.content.eth_address = format!("{:#x}", Address::repeat_byte(7));
        profile
    }

    fn deployed(profile: UserProfile) -> CurrentUserProfile {
        CurrentUserProfile {
            profile: Some(profile),
            is_deployed: true,
            ..Default::default()
        }
    }

    fn signed_in(guest_account: bool) -> Wallet {
        let mut wallet = Wallet::default();
        let address = Address::repeat_byte(7);
        wallet.finalize(
            address,
            alloy_signer_local::PrivateKeySigner::random(),
            vec![common::structs::ChainLink {
                ty: "SIGNER".to_owned(),
                payload: format!("{address:#x}"),
                signature: String::new(),
            }],
            guest_account,
        );
        wallet
    }

    fn patch(name: &str, shape: &str) -> SetAvatarData {
        SetAvatarData {
            base: Some(
                dcl_component::proto_components::sdk::components::PbAvatarBase {
                    name: name.to_owned(),
                    body_shape_urn: shape.to_owned(),
                    ..default()
                },
            ),
            equip: None,
            has_claimed_name: Some(true),
            profile_extras: None,
            name_color: None,
        }
    }

    /// Runs the engine's edit system and controls when its submitted profile finishes.
    struct Saves(App);

    impl Saves {
        fn new() -> Self {
            Self::with_wallet(signed_in(false))
        }

        fn with_wallet(wallet: Wallet) -> Self {
            let mut profile = own(5, "before");
            profile.content.eth_address = format!("{:#x}", wallet.address().unwrap());
            let mut app = App::new();
            app.add_event::<SystemApi>()
                .insert_resource(AppConfig::default())
                .insert_resource(deployed(profile))
                .insert_resource(wallet)
                .add_systems(Update, process_profile);
            Self(app)
        }

        fn edit(&mut self, data: SetAvatarData) -> common::rpc::RpcResultReceiver<SetAvatarResult> {
            let (sender, receiver) = RpcResultSender::channel();
            let address = self.0.world().resource::<Wallet>().address().unwrap();
            self.0.world_mut().send_event(SystemApi::SetAvatar(
                format!("{address:#x}"),
                data,
                sender,
            ));
            receiver
        }

        fn current(&mut self) -> Mut<'_, CurrentUserProfile> {
            self.0.world_mut().resource_mut::<CurrentUserProfile>()
        }

        fn submit(&mut self) -> SubmittedProfile {
            self.0.update();
            self.current()
                .begin_deploy()
                .expect("a profile ready to submit")
        }

        fn finish(&mut self, batch: SubmittedProfile, success: bool) {
            self.current().finish_deploy(
                batch,
                if success {
                    Ok(())
                } else {
                    Err("Server rejected the deployment.".into())
                },
            );
        }
    }

    #[test]
    fn same_frame_edits_share_the_payload_and_both_get_its_result() {
        for success in [true, false] {
            let mut saves = Saves::new();
            let mut rename = saves.edit(patch("Alice", ""));
            let mut look = saves.edit(patch("", "body:b"));
            let batch = saves.submit();
            assert_eq!(batch.profile.version, 7);
            assert_eq!(batch.profile.content.name, "Alice");
            assert_eq!(
                batch.profile.content.avatar.body_shape.as_deref(),
                Some("body:b")
            );
            assert!(batch.profile.content.has_claimed_name);
            assert_eq!(saves.current().peer_profile().unwrap().version, 5);
            saves.finish(batch, success);
            for receiver in [&mut rename, &mut look] {
                let answer = receiver.poll_once().unwrap().unwrap();
                if success {
                    assert_eq!(answer.unwrap(), 7);
                } else {
                    let profile = answer.unwrap_err().profile.unwrap();
                    assert_eq!((profile.version, profile.name.as_str()), (5, "before"));
                }
            }
        }
    }

    #[test]
    fn failure_reports_the_status_and_confirmed_profile_without_echoing_the_response() {
        let mut saves = Saves::new();
        let mut edit = saves.edit(patch("Alice", ""));
        let batch = saves.submit();
        let rejection = anyhow::Error::msg(StatusCode::CONFLICT)
            .context("response containing private account data");
        saves
            .current()
            .finish_deploy(batch, Err(profile_deploy_message(&rejection)));
        let reason = edit.poll_once().unwrap().unwrap().unwrap_err();
        assert_eq!(
            reason.message,
            "The server rejected the profile deployment (409 Conflict)."
        );
        assert_eq!(reason.profile.unwrap().name, "before");
    }

    #[test]
    fn edits_to_the_same_field_acknowledge_the_final_ordered_value() {
        let mut saves = Saves::new();
        let mut first = saves.edit(patch("Alice", ""));
        let mut last = saves.edit(patch("Bob", ""));
        let batch = saves.submit();
        assert_eq!(batch.profile.content.name, "Bob");
        saves.finish(batch, true);
        assert_eq!(first.poll_once().unwrap().unwrap().unwrap(), 7);
        assert_eq!(last.poll_once().unwrap().unwrap().unwrap(), 7);
    }

    #[test]
    fn queued_edits_rebase_after_each_deploy_and_answer_in_order() {
        for first_lands in [false, true] {
            for next_lands in [false, true] {
                let mut saves = Saves::new();
                let mut rename = saves.edit(patch("Alice", ""));
                let first = saves.submit();
                let mut look = saves.edit(patch("", "body:b"));
                saves.0.update();
                assert!(saves.current().begin_deploy().is_none());
                assert!(rename.poll_once().unwrap().is_none());
                assert!(look.poll_once().unwrap().is_none());
                assert_eq!(
                    saves.current().profile.as_ref().unwrap().content.name,
                    "Alice"
                );
                saves.finish(first, first_lands);
                let reply = rename.poll_once().unwrap().unwrap();
                let (name, known) = if first_lands {
                    assert_eq!(reply.unwrap(), 6);
                    ("Alice", 6)
                } else {
                    assert_eq!(reply.unwrap_err().profile.unwrap().version, 5);
                    ("before", 5)
                };
                assert!(look.poll_once().unwrap().is_none());
                assert_eq!(saves.current().peer_profile().unwrap().version, known);
                let mut extras = patch("", "");
                extras.profile_extras = Some([("description".into(), "bio".into())].into());
                let mut bio = saves.edit(extras);
                let next = saves.submit();
                assert_eq!(next.profile.version, 8);
                assert_eq!(next.profile.content.name, name);
                assert_eq!(next.profile.content.has_claimed_name, first_lands);
                assert_eq!(
                    next.profile.content.avatar.body_shape.as_deref(),
                    Some("body:b")
                );
                assert_eq!(next.profile.content.extra_fields["description"], "bio");
                saves.finish(next, next_lands);
                for receiver in [&mut look, &mut bio] {
                    let reply = receiver.poll_once().unwrap().unwrap();
                    if next_lands {
                        assert_eq!(reply.unwrap(), 8);
                    } else {
                        let profile = reply.unwrap_err().profile.unwrap();
                        assert_eq!(
                            (profile.version, profile.name.as_str()),
                            (known as i64, name)
                        );
                    }
                }
                let current = saves.current();
                let final_profile = current.peer_profile().unwrap();
                assert_eq!(current.profile.as_ref(), Some(final_profile));
                assert_eq!(final_profile.version, if next_lands { 8 } else { known });
            }
        }
    }

    #[test]
    fn signed_guest_deploys_and_throwaway_guest_commits_locally() {
        let mut guest = Saves::with_wallet(signed_in(true));
        let mut edit = guest.edit(patch("signed", ""));
        let batch = guest.submit();
        assert!(edit.poll_once().unwrap().is_none());
        guest.finish(batch, true);
        assert_eq!(edit.poll_once().unwrap().unwrap().unwrap(), 6);

        let mut wallet = Wallet::default();
        wallet.finalize_as_guest();
        let mut guest = Saves::with_wallet(wallet);
        let mut edit = guest.edit(patch("local", ""));
        guest.0.update();
        assert_eq!(edit.poll_once().unwrap().unwrap().unwrap(), u32::MAX);
        assert!(guest.current().begin_deploy().is_none());
        assert_eq!(
            guest.current().peer_profile().unwrap().content.name,
            "local"
        );
    }

    #[test]
    fn initial_held_edits_wait_for_terms_and_failure_has_no_published_profile() {
        let mut saves = Saves::new();
        saves.current().is_deployed = false;
        saves.current().deploy_held = true;
        let mut first = saves.edit(patch("Alice", ""));
        saves.0.update();
        assert!(saves.current().begin_deploy().is_none());
        assert!(saves.current().peer_profile().is_none());
        saves
            .0
            .world_mut()
            .resource_mut::<AppConfig>()
            .accepted_terms = Some(common::structs::TERMS_VERSION);
        let mut second = saves.edit(patch("", "body:b"));
        let batch = saves.submit();
        assert_eq!(batch.profile.version, 7);
        saves.finish(batch, false);
        for receiver in [&mut first, &mut second] {
            let rejected = receiver.poll_once().unwrap().unwrap().unwrap_err();
            assert!(!rejected.account_changed);
            assert!(rejected.profile.is_none());
        }
        assert!(saves.current().peer_profile().is_none());
        assert!(saves.current().begin_deploy().is_none());
        let mut retry = saves.edit(patch("new", ""));
        let batch = saves.submit();
        assert_eq!(batch.profile.version, 8);
        saves.finish(batch, true);
        assert_eq!(retry.poll_once().unwrap().unwrap().unwrap(), 8);
        assert_eq!(saves.current().peer_profile().unwrap().content.name, "new");
    }

    #[test]
    fn account_switch_settles_unsubmitted_edits_and_isolates_in_flight_results() {
        let mut saves = Saves::new();
        let mut submitted = saves.edit(patch("Alice", ""));
        let batch = saves.submit();
        let mut queued = saves.edit(patch("", "body:b"));
        saves.0.update();
        saves.current().reset_saves();
        let mut other = own(2, "other");
        other.content.eth_address = format!("{:#x}", Address::repeat_byte(9));
        *saves.current() = deployed(other.clone());
        let rejected = queued.poll_once().unwrap().unwrap().unwrap_err();
        assert!(rejected.account_changed);
        assert!(rejected.profile.is_none());
        saves.finish(batch, false);
        let rejected = submitted.poll_once().unwrap().unwrap().unwrap_err();
        assert!(rejected.account_changed);
        assert!(rejected.profile.is_none());
        assert_eq!(saves.current().peer_profile(), Some(&other));
    }

    #[test]
    fn late_same_wallet_success_keeps_a_newer_login_and_its_queued_edits() {
        for login_version in [5, 10] {
            let mut saves = Saves::new();
            let mut old = saves.edit(patch("Alice", ""));
            let previous = saves.submit();
            saves.current().reset_saves();
            saves.current().profile = Some(own(login_version, "server"));
            saves.current().is_deployed = true;

            let mut next = saves.edit(patch("", "body:b"));
            saves.0.update();
            saves.finish(previous, true);
            assert_eq!(old.poll_once().unwrap().unwrap().unwrap(), 6);
            assert!(next.poll_once().unwrap().is_none());
            let batch = saves.submit();
            let expected_name = if login_version == 5 {
                "Alice"
            } else {
                "server"
            };
            assert_eq!(batch.profile.content.name, expected_name);
            assert_eq!(
                batch.profile.content.avatar.body_shape.as_deref(),
                Some("body:b")
            );
            let expected_version = login_version.max(6) + 1;
            assert_eq!(batch.profile.version, expected_version);
            saves.finish(batch, true);
            assert_eq!(
                next.poll_once().unwrap().unwrap().unwrap(),
                expected_version
            );
        }
    }

    #[test]
    fn guest_login_resave_survives_an_old_upload_or_a_realm_read() {
        for old_upload in [true, false] {
            for edit_look in [false, true] {
                let mut saves = Saves::new();
                saves
                    .current()
                    .profile
                    .as_mut()
                    .unwrap()
                    .content
                    .has_connected_web3 = Some(true);
                let old = old_upload.then(|| {
                    let reply = saves.edit(patch("earlier", ""));
                    (saves.submit(), reply)
                });

                saves.current().reset_saves();
                let mut login = own(5, "login");
                login.content.has_connected_web3 = Some(false);
                saves.current().profile = Some(login);
                saves.current().is_deployed = true;
                saves.current().queue_guest_resave();
                let mut look = edit_look.then(|| saves.edit(patch("", "body:b")));
                saves.0.update();

                let (base_version, name) = if let Some((batch, mut reply)) = old {
                    saves.finish(batch, true);
                    assert_eq!(reply.poll_once().unwrap().unwrap().unwrap(), 6);
                    (6, "earlier")
                } else {
                    let mut read = own(10, "realm");
                    read.content.has_connected_web3 = Some(false);
                    saves.current().adopt_server_profile(read);
                    (10, "realm")
                };
                let batch = saves.submit();
                assert_eq!(batch.profile.content.name, name);
                assert_eq!(batch.profile.content.has_connected_web3, Some(false));
                assert_eq!(
                    saves
                        .current()
                        .peer_profile()
                        .unwrap()
                        .content
                        .has_connected_web3,
                    Some(false)
                );
                let version = base_version + 1 + u32::from(edit_look);
                assert_eq!(batch.profile.version, version);
                assert_eq!(batch.profile.content.version, i64::from(version));
                if edit_look {
                    assert_eq!(
                        batch.profile.content.avatar.body_shape.as_deref(),
                        Some("body:b")
                    );
                }
                saves.finish(batch, true);
                if let Some(reply) = look.as_mut() {
                    assert_eq!(reply.poll_once().unwrap().unwrap().unwrap(), version);
                }
                assert_eq!(saves.current().peer_profile().unwrap().version, version);
            }
        }
    }

    #[test]
    fn newer_realm_read_is_preserved_when_an_older_submission_settles() {
        for lands in [false, true] {
            let mut saves = Saves::new();
            let mut rename = saves.edit(patch("Alice", ""));
            let first = saves.submit();
            let mut look = saves.edit(patch("", "body:b"));
            saves.0.update();
            saves.current().adopt_server_profile(own(12, "remote"));
            saves.finish(first, lands);
            let result = rename.poll_once().unwrap().unwrap();
            if lands {
                assert_eq!(result.unwrap(), 6);
            } else {
                assert_eq!(result.unwrap_err().profile.unwrap().version, 12);
            }
            let batch = saves.submit();
            assert_eq!(batch.profile.content.name, "remote");
            assert_eq!(batch.profile.version, 13);
            saves.finish(batch, true);
            assert_eq!(look.poll_once().unwrap().unwrap().unwrap(), 13);
        }
    }

    #[test]
    fn a_patch_from_a_previous_wallet_is_answered_without_applying_it() {
        let mut saves = Saves::new();
        let (sender, mut receiver) = RpcResultSender::channel();
        saves.0.world_mut().send_event(SystemApi::SetAvatar(
            format!("{:#x}", Address::repeat_byte(9)),
            patch("other", ""),
            sender,
        ));
        saves.0.update();
        let rejected = receiver.poll_once().unwrap().unwrap().unwrap_err();
        assert!(rejected.account_changed);
        assert!(rejected.profile.is_none());
        assert_eq!(
            saves.current().profile.as_ref().unwrap().content.name,
            "before"
        );
        assert!(saves.current().begin_deploy().is_none());
    }

    #[test]
    fn local_stream_reports_publication_and_same_version_server_thumbnails() {
        let mut saves = Saves::new();
        saves
            .0
            .add_systems(Update, pipe_profile_changes_to_scene.after(process_profile));
        let (sender, mut published) = RpcStreamSender::channel();
        saves
            .0
            .world_mut()
            .send_event(SystemApi::GetProfileChangedStream(sender));
        saves.0.update();
        assert_eq!(published.try_recv().unwrap().version, 5);

        let _rename = saves.edit(patch("Alice", ""));
        let pending = saves.submit();
        assert!(published.try_recv().is_err());
        saves.current().set_server_snapshots(AvatarSnapshots {
            face256: "face".into(),
            body: "body".into(),
        });
        saves.0.update();
        assert_eq!(published.try_recv().unwrap().version, 5);
        assert_eq!(
            saves
                .current()
                .peer_profile()
                .unwrap()
                .content
                .avatar
                .snapshots
                .as_ref()
                .unwrap()
                .face256,
            "face"
        );
        saves.finish(pending, false);
        saves.0.update();
        assert!(published.try_recv().is_err());

        let _next = saves.edit(patch("", "body:b"));
        let pending = saves.submit();
        assert!(published.try_recv().is_err());
        saves.finish(pending, true);
        saves.0.update();
        assert_eq!(published.try_recv().unwrap().version, 7);
    }

    #[test]
    fn a_missing_compact_route_is_told_from_a_failed_request() {
        let status = |code: u16| compact_status("url", StatusCode::from_u16(code).unwrap());
        assert!(status(200).is_ok());
        assert!(matches!(status(404), Err(CompactFetchError::Unsupported)));
        assert!(matches!(status(405), Err(CompactFetchError::Unsupported)));
        assert!(matches!(status(403), Err(CompactFetchError::Failed(_))));
        assert!(matches!(status(500), Err(CompactFetchError::Failed(_))));
    }

    #[test]
    fn a_full_reader_is_not_served_by_compact_data() {
        let now = web_time::Instant::now();
        let mut entry = ProfileEntry::default();
        entry.apply_compact(Some(compact("row")), now);
        entry.want(ProfileTier::Full);
        entry.next_fetch = Some(now + PROFILE_FETCH_RETRY);
        assert!(entry.missing(ProfileTier::Full, now));
        assert!(!entry.missing(ProfileTier::Compact, now));
    }

    #[test]
    fn a_failed_compact_fetch_reads_the_address_whole() {
        let now = web_time::Instant::now();
        let mut cache = ProfileCache::default();
        cache.touch(Address::ZERO, ProfileTier::Compact).fetching = Some(ProfileTier::Compact);

        cache.compact_fetch_failed(&[Address::ZERO]);
        assert!(!cache.compact_unsupported);
        let entry = &cache.entries[&Address::ZERO];
        assert!(entry.wants_fetch(now));
        assert_eq!(
            entry.fetch_tier(cache.compact_unsupported),
            ProfileTier::Full
        );
    }

    #[test]
    fn restored_own_profile_is_not_left_waiting() {
        let mut cache = ProfileCache::default();
        cache.update(own(6, "rejected"));
        cache.update(own(5, "before"));
        let entry = &cache.entries[&Address::repeat_byte(7)];
        assert_eq!(entry.announced, 5);
        assert!(entry.satisfied());
        assert_eq!(held_name(entry), "before");
    }

    #[test]
    fn compact_requests_answer_once_every_address_settles() {
        let now = web_time::Instant::now();
        let (a, b) = (Address::repeat_byte(1), Address::repeat_byte(2));
        let row = |address: Address, name: &str| CompactProfile {
            pointer: format!("{address:#x}"),
            ..compact(name)
        };
        let names = |found: Vec<CompactProfile>| -> Vec<String> {
            found.into_iter().map(|profile| profile.name).collect()
        };
        let mut cache = ProfileCache::default();

        // first sight: both addresses become compact entries due for a fetch
        assert_eq!(cache.compact_answer(&[a, b]), None);
        assert_eq!(cache.entries[&a].fetch_tier(false), ProfileTier::Compact);

        // `a` resolves while `b` is still in flight: the answer waits for `b`
        cache
            .entries
            .get_mut(&a)
            .unwrap()
            .apply_compact(Some(row(a, "A")), now);
        cache.entries.get_mut(&b).unwrap().fetching = Some(ProfileTier::Compact);
        assert_eq!(cache.compact_answer(&[a, b]), None);

        // `b` concludes with nothing: answered without it
        cache.entries.get_mut(&b).unwrap().apply_compact(None, now);
        assert_eq!(names(cache.compact_answer(&[a, b]).unwrap()), ["A"]);

        // a held full profile answers a compact request too, in request order
        let mut full = profile(4, "B");
        full.content.eth_address = format!("{b:#x}");
        cache.update(full);
        assert_eq!(names(cache.compact_answer(&[b, a]).unwrap()), ["B", "A"]);
    }

    #[test]
    fn compact_profiles_parse_upstream_shape() {
        let address = Address::repeat_byte(0xab);
        let body = r#"[{"pointer":"0xABABABABABABABABABABABABABABABABABABABAB","hasClaimedName":true,
            "name":"Ryumi","thumbnailUrl":"https://cdn.example/entities/bafy/face.png"}]"#;
        let found: Vec<CompactProfile> = serde_json::from_str(body).unwrap();

        let matched = compact_profiles_for(found, vec![address, Address::ZERO]);
        assert_eq!(matched[0].0, address);
        let row = matched[0].1.as_ref().unwrap();
        // the server's checksummed pointer comes back as the lowercase address
        assert_eq!(row.pointer, format!("{address:#x}"));
        assert_eq!(
            (row.name.as_str(), row.has_claimed_name, row.name_color),
            ("Ryumi", true, None)
        );
        assert_eq!(
            row.thumbnail_url,
            "https://cdn.example/entities/bafy/face.png"
        );
        assert_eq!(matched[1], (Address::ZERO, None));
    }

    #[test]
    fn server_snapshots_only_after_read_back() {
        let snapshots = |face: &str| AvatarSnapshots {
            face256: face.to_owned(),
            body: String::new(),
        };
        let with_face = |version, face: &str| {
            let mut profile = profile(version, "me");
            profile.content.eth_address = format!("{:#x}", Address::ZERO);
            profile.content.avatar.snapshots = Some(snapshots(face));
            profile
        };

        // our edit at v5 still carries the snapshots of the version before it
        let mut current = with_face(5, "old");
        let mut cache = ProfileCache::default();
        let entry = cache.touch(Address::ZERO, ProfileTier::Full);
        entry.announced = 5;
        entry.data = Some((Box::new(current.clone()), ProfileSource::Catalyst));
        assert_eq!(cache.server_snapshots(&current), None);

        let entry = cache.entries.get_mut(&Address::ZERO).unwrap();
        entry.refetch_deployed(5);
        assert_eq!(cache.server_snapshots(&current), None);

        let entry = cache.entries.get_mut(&Address::ZERO).unwrap();
        entry.apply(with_face(5, "new"), ProfileSource::Catalyst);
        assert_eq!(cache.server_snapshots(&current), Some(snapshots("new")));

        current.content.avatar.snapshots = Some(snapshots("new"));
        assert_eq!(cache.server_snapshots(&current), None);

        // a newer local edit isn't handed the deployed version's snapshots
        assert_eq!(cache.server_snapshots(&with_face(6, "old")), None);
    }

    #[test]
    fn refetch_deployed_leaves_newer_edit() {
        let mut entry = ProfileEntry {
            announced: 6,
            data: Some((Box::new(profile(6, "newer")), ProfileSource::Catalyst)),
            wanted: ProfileTier::Full,
            ..Default::default()
        };
        entry.refetch_deployed(5);
        assert!(entry.satisfied());
        assert_eq!(entry.profile().unwrap().version, 6);
    }
}

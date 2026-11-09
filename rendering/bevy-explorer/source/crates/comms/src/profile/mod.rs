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

use crate::{
    global_crdt::GlobalCrdtState,
    profile::name_color::{name_color_from_address, UNCLAIMED_NAME_COLOR},
};

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
use system_bridge::{ProfileChangedEvent, SystemApi};
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
        app.add_systems(
            Update,
            (
                pipe_profile_changes_to_scene,
                process_profile,
                outfits::process_outfits,
            ),
        );
        app.init_resource::<outfits::OutfitsState>();

        // a server has no real local player: never insert/announce/deploy the fake
        // player's profile or write PLAYER identity into scene crdts
        if !common::structs::server_mode() {
            app.add_systems(
                Update,
                setup_primary_profile.in_set(SceneSets::RestrictedActions), // in restricted actions so we get profile updates from login before a scene tick runs
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
        app.init_resource::<ProfileCache>()
            .add_event::<ProfileDeployedEvent>();
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
    /// the catalyst's answer, or a guest peer's own answer (which the catalyst never
    /// holds), at the announced version
    fn satisfied(&self) -> bool {
        if self.wanted == ProfileTier::Compact {
            return self.data.is_some() || self.compact.is_some();
        }
        self.data.as_ref().is_some_and(|(profile, source)| {
            profile.version >= self.announced
                && (*source == ProfileSource::Catalyst
                    || (*source == ProfileSource::Peer && profile_is_guest(profile)))
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
    /// the catalyst answered a compact POST as an unknown route: compact readers are served
    /// from full profiles for the rest of the session
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
            "compact": counts.compact, "compactUnsupported": cache.compact_unsupported,
            "inFlight": cache.batches.len() + cache.compact_batches.len(),
            "unresolved": cache.entries.values().filter(|entry| entry.data.is_none() && entry.compact.is_none()).count(),
            "ownVersion": current.profile.as_ref().map(|profile| profile.version),
            "peerVersion": current.peer_profile().map(|profile| profile.version),
            "deployPending": current.revert_to.is_some()}).to_string());
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
            entry.announced = profile.version;
            entry.data = Some((Box::new(profile), ProfileSource::Catalyst));
            entry.compact = None;
        }
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

    /// Record an authoritative profile (our own, which we deploy ourselves).
    /// Unconditional: even a same-version edit replaces what the cache holds, and its version
    /// is the one awaited, so a profile put back after a failed deploy doesn't wait for the
    /// version that failed.
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

    /// Record a profile received directly from the owning peer. `apply`'s authority
    /// ranking keeps a room-broadcast response from displacing an equal-version
    /// catalyst answer; a guest's own answer counts as final via `satisfied`.
    pub fn update_from_peer(&mut self, profile: UserProfile) {
        if let Some(address) = profile.content.eth_address.as_h160() {
            self.cache
                .touch(address, ProfileTier::Full)
                .apply(profile, ProfileSource::Peer);
        }
    }
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn setup_primary_profile(
    mut commands: Commands,
    player: Query<(Entity, Option<&UserProfile>), With<PrimaryUser>>,
    mut current_profile: ResMut<CurrentUserProfile>,
    transports: Query<&Transport>,
    mut senders: Local<Vec<RpcEventSender>>,
    mut subscribe_events: EventReader<RpcCallEvent>,
    mut deploys: Local<Vec<PendingDeploy>>,
    wallet: Res<Wallet>,
    ipfas: IpfsAssetServer,
    mut contexts: Query<&mut GlobalCrdtState>,
    mut cache: ProfileManager,
    mut last_announce: Local<f64>,
    mut announced: Local<Option<u32>>,
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
        .profile
        .as_ref()
        .and_then(|profile| cache.server_snapshots(profile))
    {
        if let Some(profile) = current_profile.profile.as_mut() {
            profile.content.avatar.snapshots = Some(snapshots);
        }
    }

    if let Ok((player, maybe_profile)) = player.single() {
        if maybe_profile.is_none() || current_profile.is_changed() {
            let Some(profile) = current_profile.profile.as_ref() else {
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

            // update cache
            cache.update(profile.clone());

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

            // Announce the new version over Pulse alone, as a `ProfileVersionAnnouncement` — peers
            // refetch from catalyst, or ask us directly with an rfc4 `ProfileRequest`, which is still
            // answered on the byte transports (that request/response pair is what resolves a guest,
            // who has no catalyst presence). No byte transport carries the unsolicited announcement:
            // a peer that sees one over LiveKit answers it with LiveKit movement, which is exactly
            // the traffic Pulse replaces. While an edit deploys, peers hear the version from before
            // it (`peer_profile`); the new one goes out below once the deploy lands.
            // Reset the keepalive timer so the periodic re-announce below doesn't immediately fire
            // again.
            let peer = current_profile.peer_profile().unwrap_or(profile);
            announce_version(&transports, peer);
            *announced = Some(peer.version);
            *last_announce = time.elapsed_secs_f64();

            // send to event receivers
            senders.retain(|sender| {
                let _ = sender.send(format!(
                    "{{ \"ethAddress\": \"{}\", \"version\": \"{}\" }}",
                    profile.content.user_id.as_ref().unwrap(),
                    profile.version
                ));
                !sender.is_closed()
            });

            // deploy to server
            if !current_profile.is_deployed && !current_profile.deploy_held {
                debug!("deploying {:#?}", profile);
                let ipfs = ipfas.ipfs().clone();
                let profile = profile.clone();
                let wallet = wallet.clone();
                // a deploy still running is not dropped: it may land, and its result decides
                // what a failure of this one puts back
                deploys.push(PendingDeploy {
                    task: IoTaskPool::get().spawn_compat(deploy_profile(
                        ipfs,
                        wallet,
                        profile.clone(),
                    )),
                    profile,
                });
                current_profile.is_deployed = true;
            }
        } else if let Some(peer) = current_profile.peer_profile() {
            let now = time.elapsed_secs_f64();
            match due_announcement(*announced, peer.version, *last_announce, now) {
                Some(Announcement::Version) => {
                    // the deploy that held this version back has landed
                    announce_version(&transports, peer);
                    *announced = Some(peer.version);
                    *last_announce = now;
                }
                Some(Announcement::Keepalive) => {
                    debug!("announcing profile v {}", peer.version);
                    // The keepalive re-announce goes the same way as the version bump above:
                    // Pulse only, converted to a `ProfileVersionAnnouncement`.
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
                None => (),
            }
        }
    }

    // settle every deploy that finished, oldest first
    let mut index = 0;
    while index < deploys.len() {
        let Some(result) = deploys[index].task.complete() else {
            index += 1;
            continue;
        };
        let deployed = deploys.remove(index).profile;
        let address = deployed.content.eth_address.as_h160();
        let version = deployed.version;
        let success = match result {
            Ok(()) => {
                info!("deployed profile v{version}");
                if let Some(address) = address {
                    cache.refetch_deployed(address, version);
                }
                true
            }
            Err(e) => {
                error!("failed to deploy profile v{version}: {e}");
                false
            }
        };
        // only a profile that changed is a change the avatar and scenes must see; otherwise
        // just what peers may be told moves on
        if current_profile
            .bypass_change_detection()
            .deploy_finished(&deployed, success)
        {
            current_profile.set_changed();
        }
        commands.send_event(ProfileDeployedEvent {
            address,
            version,
            success,
        });
    }
}

/// A deploy of the local player's profile still running, and the profile it carries.
pub struct PendingDeploy {
    profile: UserProfile,
    task: Task<Result<(), anyhow::Error>>,
}

/// What peers are owed about the profile they may know (`peer_version`, see `peer_profile`).
#[derive(Debug, PartialEq)]
enum Announcement {
    /// Its version, which differs from the last one announced: a deploy that held it back landed.
    Version,
    /// The five-second keepalive of the version already announced.
    Keepalive,
}

fn due_announcement(
    announced: Option<u32>,
    peer_version: u32,
    last_announce: f64,
    now: f64,
) -> Option<Announcement> {
    if announced != Some(peer_version) {
        Some(Announcement::Version)
    } else if now > last_announce + 5.0 {
        Some(Announcement::Keepalive)
    } else {
        None
    }
}

/// Announce `profile`'s version over Pulse, converted to a `ProfileVersionAnnouncement`.
fn announce_version(transports: &Query<&Transport>, profile: &UserProfile) {
    debug!("announcing profile new version {:?}", profile.version);
    broadcast(
        transports.iter(),
        BroadcastTarget::PULSE,
        false,
        ProfileUpdate {
            serialized_profile: serde_json::to_string(&profile.content).unwrap(),
            base_url: profile.base_url.clone(),
            version: profile.version,
        },
    );
}

#[derive(Resource, Default)]
pub struct CurrentUserProfile {
    pub profile: Option<UserProfile>,
    pub is_deployed: bool,
    /// A new profile made while the world is held: the host's welcome page deploys it, with its
    /// save after the terms are accepted.
    pub deploy_held: bool,
    /// While an edit deploys, the newest profile the server is known to hold: the one from
    /// before the first edit still deploying, or a later edit whose deploy landed since. Other
    /// players are told about it meanwhile, and a failed deploy puts it back.
    pub revert_to: Option<UserProfile>,
    /// The highest version this session gave an edit of an account's profile. A deploy that
    /// failed here may still have reached the server (an error after the catalyst took it),
    /// and a peer that read it skips any other profile announced under the same number, so the
    /// next edit never reuses it.
    pub last_version: Option<(Address, u32)>,
}

impl CurrentUserProfile {
    /// Give the edited profile its version: one above its own and above every version this
    /// session already gave an edit of the same account.
    pub fn bump_version(&mut self) {
        let Some(profile) = self.profile.as_mut() else {
            return;
        };
        let address = profile.content.eth_address.as_h160();
        let used = match self.last_version {
            Some((last, version)) if Some(last) == address => version,
            _ => 0,
        };
        profile.version = profile.version.max(used) + 1;
        profile.content.version = profile.version as i64;
        self.last_version = address.map(|address| (address, profile.version));
    }

    /// Keep the profile as it is before an edit that will deploy; an older edit still deploying
    /// keeps the snapshot it took.
    pub fn snapshot_for_revert(&mut self) {
        if self.revert_to.is_none() {
            self.revert_to = self.profile.clone();
        }
    }

    /// The deploy of `deployed` has finished; a result for another account changes nothing.
    /// Every edit deploys, and a newer one does not cancel an older one still running.
    /// - The newest edit landing keeps it; failing puts `revert_to` back without deploying it
    ///   again.
    /// - An older edit landing while a newer one still deploys becomes `revert_to`: the server
    ///   holds it, so that is what peers hear and what a failure of the newer one puts back.
    /// - An older edit landing after a newer one failed and put an older profile back is taken:
    ///   the server holds it.
    /// - Anything else (an older edit failing, or landing after a newer one landed) changes
    ///   nothing.
    ///
    /// Returns whether the profile changed.
    pub fn deploy_finished(&mut self, deployed: &UserProfile, success: bool) -> bool {
        let Some(current) = self.profile.as_ref() else {
            return false;
        };
        let address = deployed.content.eth_address.as_h160();
        if address.is_none() || address != current.content.eth_address.as_h160() {
            return false;
        }
        let (shown, landed) = (current.version, deployed.version);
        if !success {
            if landed != shown {
                return false;
            }
            return match self.revert_to.take() {
                Some(snapshot) => {
                    self.profile = Some(snapshot);
                    true
                }
                None => false,
            };
        }
        match landed.cmp(&shown) {
            std::cmp::Ordering::Equal => {
                self.revert_to = None;
                false
            }
            std::cmp::Ordering::Less => {
                if let Some(snapshot) = self.revert_to.as_mut() {
                    if snapshot.version < landed {
                        *snapshot = deployed.clone();
                    }
                }
                false
            }
            std::cmp::Ordering::Greater => {
                self.profile = Some(deployed.clone());
                self.revert_to = None;
                true
            }
        }
    }

    /// Take the profile the server holds (a realm change reads it again), unless an edit still
    /// deploying is newer: that edit stays, with its snapshot, until its deploy settles.
    pub fn adopt_server_profile(&mut self, profile: UserProfile) {
        let newer_edit_deploying = self.revert_to.is_some()
            && self
                .profile
                .as_ref()
                .is_some_and(|current| current.version > profile.version);
        if newer_edit_deploying {
            return;
        }
        self.profile = Some(profile);
        self.is_deployed = true;
        self.deploy_held = false;
        self.revert_to = None;
    }

    /// The profile other players may know about: the one from before an edit still deploying,
    /// as they only hear versions the server holds.
    pub fn peer_profile(&self) -> Option<&UserProfile> {
        self.revert_to.as_ref().or(self.profile.as_ref())
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
                lambdas.clone(),
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
#[allow(clippy::type_complexity)]
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

/// Pipes every profile insert/replace the engine sees — a foreign player's, or the local
/// player's own — to system-scene stream subscribers as (address, version).
fn pipe_profile_changes_to_scene(
    mut requests: EventReader<SystemApi>,
    mut senders: Local<Vec<RpcStreamSender<ProfileChangedEvent>>>,
    changed: Query<(Option<&ForeignPlayer>, &UserProfile), Changed<UserProfile>>,
    wallet: Res<Wallet>,
) {
    senders.extend(requests.read().filter_map(|ev| {
        if let SystemApi::GetProfileChangedStream(sender) = ev {
            Some(sender.clone())
        } else {
            None
        }
    }));
    senders.retain(|s| !s.is_closed());

    if senders.is_empty() {
        return;
    }

    for (player, profile) in &changed {
        let address = match player {
            Some(player) => player.address,
            None => match wallet.address() {
                Some(address) => address,
                None => continue,
            },
        };
        let event = ProfileChangedEvent {
            address: format!("{:#x}", address),
            version: profile.version,
        };
        for sender in senders.iter() {
            let _ = sender.send(event.clone());
        }
    }
}

#[allow(clippy::too_many_arguments)]
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
                            return;
                        };

                        debug!("sending my profile");
                        let response = rfc4::Packet {
                            message: Some(rfc4::packet::Message::ProfileResponse(
                                rfc4::ProfileResponse {
                                    serialized_profile: serde_json::to_string(
                                        &current_profile.content,
                                    )
                                    .unwrap(),
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

                    // check/update profile version
                    if version < player.profile_version {
                        continue;
                    }
                    if version > player.profile_version {
                        player.profile_version = version;
                    }

                    let profile = UserProfile {
                        version,
                        content: serialized_profile,
                        base_url: r.base_url.clone(),
                    };

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

                    cache.update_from_peer(profile.clone());

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

    pub fn name_color(&self) -> Color {
        if !self.content.has_claimed_name {
            UNCLAIMED_NAME_COLOR
        } else if let Some(custom_name_color) = self.content.name_color {
            custom_name_color.convert_srgb()
        } else {
            name_color_from_address(self.content.eth_address.as_h160().unwrap_or_default())
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

/// A deploy of the local player's profile settled: which account and version it carried.
#[derive(Event)]
pub struct ProfileDeployedEvent {
    pub address: Option<Address>,
    pub version: u32,
    pub success: bool,
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

    match response.status() {
        StatusCode::OK => Ok(()),
        _ => Err(anyhow!(
            "bad response: {}: {}",
            response.status(),
            String::from_utf8_lossy(&response.bytes().await?)
        )),
    }
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

/// The lambdas of the base domain's catalyst, where every profile is read.
fn lambdas_url() -> String {
    common::base_domain::url(common::base_domain::Service::Catalyst, "/lambdas")
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

/// One catalyst POST of the compact tier for a chunk of ids: the asked addresses, in
/// order, each with what the catalyst holds for it. A catalyst without the route
/// (Decentraland's answer 404; one whose `/profiles/{id}` takes the path, 405) is
/// `Unsupported`.
async fn fetch_compact_profiles(
    lambdas: String,
    addresses: Vec<Address>,
    ipfs: Arc<IpfsIo>,
) -> CompactAnswer {
    let url = format!("{}/profiles/metadata", lambdas.trim_end_matches('/'));
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

/// Resolve one profile from the catalyst without the engine cache (used at login, before
/// any player entities exist): `lambdas` is the sign-in's catalyst, else the centralized
/// one. Ok(None) means the catalyst authoritatively has no profile, so callers can tell
/// "no profile" from "couldn't fetch".
pub async fn get_remote_profile(
    address: Address,
    ipfs: Arc<IpfsIo>,
    lambdas: Option<String>,
) -> Result<Option<UserProfile>, anyhow::Error> {
    let lambdas = lambdas.unwrap_or_else(lambdas_url);
    fetch_profiles(lambdas, vec![address], ipfs)
        .await
        .pop()
        .map_or(Ok(None), |(_, result)| result)
}

enum ProcessProfileState {
    /// An edit's deploy: the account and version it carries, and who waits for its result.
    Deploying {
        address: Option<Address>,
        version: u32,
        sender: RpcResultSender<Result<u32, String>>,
    },
}

/// What the edit waiting on the deploy of `version` of `address` hears from a deploy that
/// settled: that deploy's result, "cancelled" when another deploy replaced it, and nothing from an
/// older deploy of the same account, which settled before the edit was made (versions only climb:
/// `CurrentUserProfile::bump_version`).
fn deploy_answer(
    address: Option<Address>,
    version: u32,
    settled: &ProfileDeployedEvent,
) -> Option<Result<u32, String>> {
    if settled.address != address || settled.version > version {
        Some(Err("cancelled".to_owned()))
    } else if settled.version < version {
        None
    } else if settled.success {
        Some(Ok(version))
    } else {
        Some(Err("failed to deploy to server.".to_owned()))
    }
}

fn process_profile(
    mut e: EventReader<SystemApi>,
    mut current_profile: ResMut<CurrentUserProfile>,
    mut processing: Local<Option<ProcessProfileState>>,
    mut deployed: EventReader<ProfileDeployedEvent>,
    config: Res<AppConfig>,
) {
    if let Some(SystemApi::SetAvatar(set_avatar, sender)) = e
        .read()
        .filter(|ev| matches!(ev, SystemApi::SetAvatar(..)))
        .last()
    {
        // the welcome page's save, after its accept: a held new profile deploys with it
        if current_profile.deploy_held && config.terms_accepted() {
            current_profile.deploy_held = false;
        }

        // an edit that will deploy keeps the profile from before it until the deploy settles
        if current_profile
            .profile
            .as_ref()
            .is_some_and(|profile| profile.content.has_connected_web3.unwrap_or_default())
        {
            current_profile.snapshot_for_revert();
        }

        let Some(profile) = current_profile.profile.as_mut() else {
            error!("can't amend missing profile");
            return;
        };

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

            profile.content.name = base.name.clone();
            profile.content.avatar.name = Some(base.name.clone());
        }

        if let Some(has_claimed_name) = set_avatar.has_claimed_name {
            profile.content.has_claimed_name = has_claimed_name;
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

        current_profile.bump_version();
        let Some(profile) = current_profile.profile.as_ref() else {
            return;
        };
        debug!("{:#?}", profile.content);
        let address = profile.content.eth_address.as_h160();
        let version = profile.version;
        let deploys = profile.content.has_connected_web3.unwrap_or_default();

        // The deploy this edit replaces hears its own result if that has already come in, else
        // "cancelled": the new deploy carries its change too. Every result queued now settled
        // before this edit, so none of them is the new deploy's.
        match processing.take() {
            Some(ProcessProfileState::Deploying {
                address: replaced,
                version: replaced_version,
                sender: waiting,
            }) => {
                let own = deployed
                    .read()
                    .filter(|settled| {
                        settled.address == replaced && settled.version == replaced_version
                    })
                    .last()
                    .map(|settled| settled.success);
                waiting.send(match own {
                    Some(true) => Ok(replaced_version),
                    Some(false) => Err("failed to deploy to server.".to_owned()),
                    None => Err("cancelled".to_owned()),
                });
            }
            None => deployed.clear(),
        }

        if deploys {
            *processing = Some(ProcessProfileState::Deploying {
                address,
                version,
                sender: sender.clone(),
            });
            current_profile.is_deployed = false;
        } else {
            sender.send(Ok(u32::MAX))
        }

        return;
    }

    if let Some(ProcessProfileState::Deploying {
        address,
        version,
        sender,
    }) = processing.take()
    {
        debug!("checking for response: {}", version);
        for settled in deployed.read() {
            debug!("got response: {} vs {}", version, settled.version);
            if let Some(answer) = deploy_answer(address, version, settled) {
                sender.send(answer);
                return;
            }
        }
        *processing = Some(ProcessProfileState::Deploying {
            address,
            version,
            sender,
        });
    } else {
        deployed.clear();
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
        cache.update(own);

        let entry = cache.entries.get_mut(&address).unwrap();
        entry.refetch_deployed(5);
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

    fn own_address() -> Option<Address> {
        Some(Address::repeat_byte(7))
    }

    /// as `process_profile` amends a profile that will deploy; returns what its deploy carries
    fn edit(current: &mut CurrentUserProfile, name: &str) -> UserProfile {
        current.snapshot_for_revert();
        current.bump_version();
        current.profile.as_mut().unwrap().content.name = name.to_owned();
        current.profile.clone().unwrap()
    }

    fn deployed(profile: UserProfile) -> CurrentUserProfile {
        CurrentUserProfile {
            profile: Some(profile),
            is_deployed: true,
            ..Default::default()
        }
    }

    fn version_and_name(profile: Option<&UserProfile>) -> (u32, &str) {
        let profile = profile.unwrap();
        (profile.version, profile.content.name.as_str())
    }

    #[test]
    fn failed_deploy_restores_the_pre_edit_profile() {
        let mut current = deployed(own(5, "before"));
        let rejected = edit(&mut current, "rejected");
        assert!(current.deploy_finished(&rejected, false));
        assert_eq!(version_and_name(current.profile.as_ref()), (5, "before"));
        assert!(current.revert_to.is_none());
        assert!(
            current.is_deployed,
            "the restored profile is not deployed again"
        );
    }

    #[test]
    fn overlapping_edits_revert_to_the_oldest_snapshot() {
        let mut current = deployed(own(5, "before"));
        edit(&mut current, "first");
        let second = edit(&mut current, "second");
        assert_eq!(version_and_name(current.profile.as_ref()), (7, "second"));
        current.deploy_finished(&second, false);
        assert_eq!(version_and_name(current.profile.as_ref()), (5, "before"));
    }

    #[test]
    fn a_replaced_deploy_that_lands_is_what_a_failure_puts_back() {
        let mut current = deployed(own(5, "before"));
        let first = edit(&mut current, "first");
        let second = edit(&mut current, "second");

        // the first edit lands while the second still deploys: peers may hear it now
        assert!(!current.deploy_finished(&first, true));
        assert_eq!(version_and_name(current.peer_profile()), (6, "first"));

        // the second fails: back to the first, which the server holds, not to before both
        assert!(current.deploy_finished(&second, false));
        assert_eq!(version_and_name(current.profile.as_ref()), (6, "first"));
        assert!(current.revert_to.is_none());
    }

    #[test]
    fn a_replaced_deploy_landing_after_the_newer_one_failed_is_taken() {
        let mut current = deployed(own(5, "before"));
        let first = edit(&mut current, "first");
        let second = edit(&mut current, "second");

        assert!(current.deploy_finished(&second, false));
        assert_eq!(version_and_name(current.profile.as_ref()), (5, "before"));
        assert!(current.deploy_finished(&first, true));
        assert_eq!(version_and_name(current.profile.as_ref()), (6, "first"));
    }

    #[test]
    fn a_replaced_deploy_settling_after_the_newer_one_landed_changes_nothing() {
        let mut current = deployed(own(5, "before"));
        let first = edit(&mut current, "first");
        let second = edit(&mut current, "second");

        assert!(!current.deploy_finished(&second, true));
        assert!(!current.deploy_finished(&first, true));
        assert!(!current.deploy_finished(&first, false));
        assert_eq!(version_and_name(current.profile.as_ref()), (7, "second"));
        assert!(current.revert_to.is_none());
    }

    #[test]
    fn successful_deploy_clears_the_snapshot() {
        let mut current = deployed(own(5, "before"));
        let after = edit(&mut current, "after");
        assert!(!current.deploy_finished(&after, true));
        assert_eq!(version_and_name(current.profile.as_ref()), (6, "after"));
        assert!(current.revert_to.is_none());
        assert_eq!(version_and_name(current.peer_profile()), (6, "after"));
    }

    #[test]
    fn peers_see_the_snapshot_while_a_deploy_is_pending() {
        let mut current = deployed(own(5, "before"));
        assert_eq!(version_and_name(current.peer_profile()), (5, "before"));
        edit(&mut current, "pending");
        assert_eq!(version_and_name(current.profile.as_ref()), (6, "pending"));
        assert_eq!(version_and_name(current.peer_profile()), (5, "before"));
    }

    #[test]
    fn a_result_for_another_account_changes_nothing() {
        let mut current = deployed(own(5, "before"));
        let rejected = edit(&mut current, "rejected");
        let mut other = profile(2, "other");
        other.content.eth_address = format!("{:#x}", Address::repeat_byte(9));
        current.profile = Some(other.clone());

        assert!(!current.deploy_finished(&rejected, false));
        assert_eq!(current.profile, Some(other));
    }

    #[test]
    fn a_realm_refresh_keeps_an_edit_still_deploying() {
        let mut current = deployed(own(5, "before"));
        let pending = edit(&mut current, "pending");
        current.adopt_server_profile(own(5, "server"));
        assert_eq!(version_and_name(current.profile.as_ref()), (6, "pending"));
        assert_eq!(version_and_name(current.peer_profile()), (5, "before"));

        // once nothing is deploying, the server's profile is taken
        current.deploy_finished(&pending, true);
        current.adopt_server_profile(own(6, "server"));
        assert_eq!(version_and_name(current.profile.as_ref()), (6, "server"));
    }

    #[test]
    fn a_failed_version_is_not_reused() {
        let mut current = deployed(own(5, "before"));
        let rejected = edit(&mut current, "rejected");
        current.deploy_finished(&rejected, false);
        assert_eq!(version_and_name(current.profile.as_ref()), (5, "before"));

        // the rejected v6 may still have reached the server: the next edit goes above it
        edit(&mut current, "next");
        assert_eq!(version_and_name(current.profile.as_ref()), (7, "next"));
    }

    #[test]
    fn another_account_counts_from_its_own_version() {
        let mut current = deployed(own(5, "before"));
        edit(&mut current, "edited");
        let mut other = profile(2, "other");
        other.content.eth_address = format!("{:#x}", Address::repeat_byte(9));
        current.profile = Some(other);
        current.revert_to = None;

        edit(&mut current, "other edited");
        assert_eq!(
            version_and_name(current.profile.as_ref()),
            (3, "other edited")
        );
    }

    #[test]
    fn peers_hear_the_new_version_once_its_deploy_lands() {
        let mut current = deployed(own(5, "before"));
        let announced = Some(5);
        let pending = edit(&mut current, "pending");

        // pending: peers keep the version from before the edit, repeated by the keepalive
        let peer = current.peer_profile().unwrap().version;
        assert_eq!(peer, 5);
        assert_eq!(due_announcement(announced, peer, 0.0, 1.0), None);
        assert_eq!(
            due_announcement(announced, peer, 0.0, 6.0),
            Some(Announcement::Keepalive)
        );

        // landed: the edit's version goes out once
        current.deploy_finished(&pending, true);
        let peer = current.peer_profile().unwrap().version;
        assert_eq!(peer, 6);
        assert_eq!(
            due_announcement(announced, peer, 0.0, 1.0),
            Some(Announcement::Version)
        );
        assert_eq!(due_announcement(Some(6), peer, 1.0, 2.0), None);
    }

    fn web3(version: u32, name: &str) -> UserProfile {
        let mut profile = own(version, name);
        profile.content.has_connected_web3 = Some(true);
        profile
    }

    /// `process_profile` alone: edits go in as `SetAvatar`s, deploys settle as events.
    struct Saves(App);

    impl Saves {
        fn new(profile: UserProfile) -> Self {
            let mut app = App::new();
            app.add_event::<SystemApi>()
                .add_event::<ProfileDeployedEvent>()
                .insert_resource(AppConfig::default())
                .insert_resource(deployed(profile))
                .add_systems(Update, process_profile);
            Self(app)
        }

        fn edit(&mut self) -> common::rpc::RpcResultReceiver<Result<u32, String>> {
            let (sender, receiver) = RpcResultSender::channel();
            let data = system_bridge::SetAvatarData {
                base: None,
                equip: None,
                has_claimed_name: None,
                profile_extras: None,
                name_color: None,
            };
            self.0
                .world_mut()
                .send_event(SystemApi::SetAvatar(data, sender));
            receiver
        }

        fn settle(&mut self, version: u32, success: bool) {
            self.0.world_mut().send_event(ProfileDeployedEvent {
                address: own_address(),
                version,
                success,
            });
        }

        fn current(&mut self) -> Mut<'_, CurrentUserProfile> {
            self.0.world_mut().resource_mut::<CurrentUserProfile>()
        }
    }

    #[test]
    fn an_edit_replacing_a_pending_deploy_cancels_it() {
        let mut saves = Saves::new(web3(5, "before"));
        let mut first = saves.edit();
        saves.0.update();
        assert_eq!(first.poll_once(), Ok(None));

        let mut second = saves.edit();
        saves.0.update();
        assert_eq!(first.poll_once(), Ok(Some(Err("cancelled".to_owned()))));
        assert_eq!(second.poll_once(), Ok(None));

        saves.settle(7, true);
        saves.0.update();
        assert_eq!(second.poll_once(), Ok(Some(Ok(7))));
    }

    #[test]
    fn a_stale_result_does_not_answer_the_next_edit() {
        let mut saves = Saves::new(web3(5, "before"));
        let mut first = saves.edit();
        saves.0.update();

        // the first deploy fails (the engine puts v5 back) in the frame a second edit arrives:
        // the first edit hears its own result, and the second edit becomes v7, not v6 again
        let first_edit = saves.current().profile.clone().unwrap();
        saves.current().deploy_finished(&first_edit, false);
        saves.settle(6, false);
        let mut second = saves.edit();
        saves.0.update();
        assert_eq!(
            first.poll_once(),
            Ok(Some(Err("failed to deploy to server.".to_owned())))
        );
        assert_eq!(saves.current().profile.as_ref().unwrap().version, 7);

        // a result of the first deploy that comes in late leaves the second edit waiting
        saves.settle(6, false);
        saves.0.update();
        assert_eq!(second.poll_once(), Ok(None));

        saves.settle(7, true);
        saves.0.update();
        assert_eq!(second.poll_once(), Ok(Some(Ok(7))));
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

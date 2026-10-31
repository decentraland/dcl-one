//! The permission core: one place that answers "may WHO do WHAT in WHICH
//! world or scene" from the shared SQL (`worlds.*`, `social.scene_admins`,
//! `social.scene_bans`, `social.user_bans`, `social.community_*`) and the
//! name/LAND authority (the marketplace subgraph readers, the old squid
//! `ens`/`nft` lookups).
//! This is the local call the comms gatekeeper, the deploy/permission/settings
//! routes and the storage routes make where the old services made HTTP round
//! trips: comms → worlds `GET /world/{name}/permissions` (+ `/permissions/
//! {perm}/parcels`), worlds → comms `/worlds/{w}/parcels/{p}/users/{a}/ban-
//! status` and `/users/{a}/bans`, world-storage → worlds `/permissions` and
//! → lambdas `/users/{a}/parcels/{x}/{y}/permissions`.
//!
//! Every answer is a [`PermissionDecision`]-shaped value: a yes with its
//! reason, a no, or "could not tell" (a database or authority fault), never
//! a bare `bool` that a fault could flatten into a permit or a denial. The
//! current time is always the caller's parameter.
//!
//! # Decision table
//!
//! Scopes. `owner`: the NAME owner of a world. `deployer`/`streamer`: an
//! address holding a `deployment`/`streaming` grant, world-wide or scoped to
//! parcels covering the footprint in question. `visitor`: an address the
//! world's access setting admits. `operator`: a LAND owner/operator on a
//! Genesis parcel (the LAND authority used by comms administration and storage).
//!
//! | question | order of evidence | answer |
//! |---|---|---|
//! | [`Policy::is_owner`] (world, actor): the owner question of every mutation (publish, permission and settings edits, unpublish, storage clears) | 1. personal world (`<addr>.dcl.eth`, only when the personal-worlds policy is enabled): the label is the owner. 2. the name authority: `owned_names(actor)` contains the label right now (the old deploy's `resolve_name_owner_id` through squid). 3. only without a configured names reader: `worlds.owner` (the old permission routes' degrade). | 1: compare; 2: `Allowed` / refused, a reader fault is undetermined (503 on the routes, 403 fail-closed on a publish); 3: compare, refused when unset |
//! | [`Policy::can_deploy`] (world, actor, footprint) | 1. personal world: the actor is its owner and the realm cap admits it (a world already published here is always admitted; otherwise the count of personal worlds published here, mirrored copies excluded, must be below the cap). 2. else the name authority names the actor as the label's holder; **no names reader denies outright** (the old squid-less node denied every non-personal publish). 3. else a `deployment` grant covering the footprint **and** every parcel of every already-deployed scene it overlaps. | 1/2: `Deploy::Owner` (replaces every overlapping scene, and the publish rewrites `worlds.owner` from this answer; the `worlds-owner-reconcile` job also rewrites it after a NAME transfer); 3: `Deploy::Scoped(overlapping ids)` (may replace exactly those); a lookup fault is undetermined (the old route rendered it 403 "could not verify", fail-closed) |
//! | [`Policy::can_stream`] (world, actor, parcels) | the owner, else a `streaming` grant covering the parcels (world-wide when none are given) | `Allowed` / refused |
//! | [`Policy::can_access`] (world, actor, secret, scene) | 1. `worlds.blocked_wallets` or an in-force `social.user_bans` row (address or recorded device) → banned from the platform. 2. a `social.scene_bans` row for the resolved scene → banned from the world. 3. the owner ([`Policy::current_owner_or_stored`]: the personal rule, else the name authority's holder of a DCL NAME (memoized 30 s per name), else `worlds.owner`, which an unresolved name, a missing reader or an authority fault falls back to; never a 503) → allowed. 4. the access setting: `unrestricted` → allowed; `allow-list` → the actor holds an `access` grant, else is a member of a listed active community (`social.community_members`, not banned); `shared-secret` → bcrypt-verify the presented secret; `nft-ownership` → refused (the old code never checked ownership). An unknown world is unrestricted with no owner (the old lookup's default). | [`Access`] |
//! | [`Policy::is_admin`] (scene, actor) | 1. an active `social.scene_admins` row keyed by the scene id (comms' write). 2. world scene: the world's owner. Genesis scene: LAND ownership at any of its parcels (directly or through the estate) through the marketplace reader. | `Allowed` / refused; a reader fault is undetermined; no reader configured refuses non-admins (the old degrade) |
//! | [`Policy::may_manage_land_storage`] (parcel, actor): the storage routes' Genesis cell | never a scene admin: the LAND manager document of `parcel` ([`LandOperatorsReader`], the lambdas `/parcels/{x}/{y}/operators` comms reads for its protected set): owner (the estate's owner when the parcel sits in one), operator, updateOperator, an updateManager or an approvedForAll operator (the old world-storage `has_any()` matrix); with no document source, LAND ownership at `parcel` through the marketplace reader. | `Allowed` / refused; a reader fault is undetermined; no source configured refuses |
//! | [`Policy::is_banned`] (actor, scene, device, now) | worlds `blocked_wallets`, then the comms-owned `BanStatus` answer for platform/device and scene bans | the first [`Ban`] in that order, or none |
//! | [`Policy::protected_addresses`] (world, parcels) | [`Policy::current_owner_or_stored`] plus every `deployment`/`streaming` grantee whose grant is world-wide or touches one of the parcels | the set (the old extra addresses) |
//!
//! Left to the routes: rate limiting of shared-secret attempts, the world
//! capacity check (presence), the external wallet denylist, the EIP-712
//! signed-fetch verification (`src/auth`, `shared::signed_fetch`) that
//! produces the actor, and every HTTP status/message.

use std::collections::BTreeSet;
use std::sync::Arc;
use std::time::{Duration, Instant};

use chrono::NaiveDateTime;
use sqlx::PgPool;
use sqlx::Row;

use crate::chain_index::ChainIndex;
use crate::comms::gatekeeper::moderation::{BanStatus, Moderation};
use crate::comms::world_identity::{Parcel, ParcelSet, SceneId, WorldName};
use crate::http::Outbound;
use crate::land_authz::{self, LandAuthzState};
use crate::shared::cache::TtlMap;
use crate::shared::identity::{
    Address, AuthorityNotEstablished, COMMUNITY_MEMBERSHIP_SQL, PermissionDecision,
};

// The access vocabulary `Policy` returns and reads lives in `comms::access`.
pub use crate::comms::access::{Access, AccessGrant, AccessType, Ban, LandOperators, SceneRef};

/// The permission-relevant columns of one `worlds.worlds` row.
#[derive(Debug, Clone)]
pub struct WorldRow {
    /// The stored spelling.
    pub name: String,
    pub owner: Option<Address>,
    pub access_type: AccessType,
    pub access_secret_hash: Option<String>,
    pub access_nft: Option<String>,
    pub blocked_since: Option<NaiveDateTime>,
    pub mirror_source: Option<String>,
}

pub const MAX_WORLD_SIZE_BYTES: i64 = 300 * 1024 * 1024;
pub const DEFAULT_PERSONAL_WORLD_MAX_SIZE_BYTES: i64 = 50 * 1024 * 1024;

pub const NOT_OWNER_MSG: &str = "signer does not own this world";
pub const NOT_OWNER_OR_ADMIN_MSG: &str = "signer is not an owner or admin of this scene";
pub const NO_NAME_AUTHORITY_MSG: &str = "NAME-ownership verification is unavailable";

/// `worlds.world_permissions.permission`, the old wire spellings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Permission {
    Deployment,
    Streaming,
    Access,
}

impl Permission {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Deployment => "deployment",
            Self::Streaming => "streaming",
            Self::Access => "access",
        }
    }

    pub fn parse(raw: &str) -> Option<Self> {
        match raw {
            "deployment" => Some(Self::Deployment),
            "streaming" => Some(Self::Streaming),
            "access" => Some(Self::Access),
            _ => None,
        }
    }

    /// The two grants the parcel-scoped routes accept (`access` is world-wide only).
    pub fn supports_parcels(self) -> bool {
        matches!(self, Self::Deployment | Self::Streaming)
    }
}

/// The personal-worlds policy (`WORLDS_PERSONAL_WORLDS_MAX`,
/// `WORLDS_PERSONAL_WORLD_MAX_SIZE_BYTES`): `<address>.dcl.eth` is owned by
/// its address while the realm hosts fewer than `max_worlds` of them. Off
/// (`max_worlds == 0`), a personal name is an ordinary NAME nobody owns.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PersonalWorldsPolicy {
    pub max_worlds: u64,
    pub max_size_bytes: i64,
}

#[derive(Debug, PartialEq, Eq)]
pub enum PersonalWorldDeny {
    Disabled,
    RealmFull { max_worlds: u64 },
}

impl PersonalWorldsPolicy {
    pub fn new(max_worlds: u64, max_size_bytes: u64, world_size_cap: i64) -> anyhow::Result<Self> {
        if max_size_bytes == 0 || max_size_bytes > world_size_cap as u64 {
            anyhow::bail!(
                "WORLDS_PERSONAL_WORLD_MAX_SIZE_BYTES must be between 1 and {world_size_cap}, got {max_size_bytes}"
            );
        }
        Ok(Self {
            max_worlds,
            max_size_bytes: max_size_bytes as i64,
        })
    }

    pub fn disabled() -> Self {
        Self {
            max_worlds: 0,
            max_size_bytes: DEFAULT_PERSONAL_WORLD_MAX_SIZE_BYTES,
        }
    }

    pub fn enabled(&self) -> bool {
        self.max_worlds > 0
    }

    pub fn owner_of(&self, world: &WorldName) -> Option<Address> {
        if self.enabled() {
            world.personal_owner()
        } else {
            None
        }
    }

    pub fn size_cap_for(&self, world: &WorldName, world_size_cap: i64) -> i64 {
        if self.owner_of(world).is_some() {
            self.max_size_bytes
        } else {
            world_size_cap
        }
    }

    /// `already_deployed`: the world already holds a scene here (always
    /// admitted); `hosted`: how many personal worlds this realm hosts.
    pub fn admit(&self, already_deployed: bool, hosted: u64) -> Result<(), PersonalWorldDeny> {
        if !self.enabled() {
            return Err(PersonalWorldDeny::Disabled);
        }
        if already_deployed {
            return Ok(());
        }
        if hosted >= self.max_worlds {
            return Err(PersonalWorldDeny::RealmFull {
                max_worlds: self.max_worlds,
            });
        }
        Ok(())
    }
}

/// What a publish may replace once authorized (the old `SceneReplacement`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Deploy {
    /// The NAME owner (or the personal world's owner): every overlapping
    /// scene is replaced.
    Owner,
    /// A grantee whose deployment grant covers the footprint and the full
    /// footprint of these overlapping scenes: exactly they may be replaced.
    Scoped(Vec<SceneId>),
}

/// `COMMS_LAND_OPERATORS_URL` behind the guarded client, memoized per
/// parcel for [`LAND_OPERATORS_TTL`] (the old external.rs cached the
/// decision, the old lambdas the document, both for a minute).
pub struct LandOperatorsReader {
    http: Outbound,
    base: String,
    cache: TtlMap<String, Arc<Option<LandOperators>>>,
}

pub const LAND_OPERATORS_TTL: std::time::Duration = std::time::Duration::from_secs(60);

impl LandOperatorsReader {
    pub fn new(http: Outbound, base_url: &str) -> Self {
        Self {
            http,
            base: base_url.trim_end_matches('/').to_string(),
            cache: TtlMap::bounded(LAND_OPERATORS_TTL, 50_000),
        }
    }

    /// The document, `None` for a parcel the lambdas do not know (404).
    /// Upstream's `getLandOperators` re-throws on a transport error and on
    /// any other non-2xx, so a fault is an error, never "nobody manages it".
    pub async fn fetch(&self, parcel: Parcel) -> Result<Arc<Option<LandOperators>>, String> {
        let url = format!("{}/parcels/{}/{}/operators", self.base, parcel.x, parcel.y);
        self.cache
            .get_or_fetch(parcel.canonical(), || async move {
                let resp = self
                    .http
                    .get(&url)
                    .timeout(std::time::Duration::from_secs(10))
                    .send()
                    .await
                    .map_err(|e| format!("LAND operators request failed: {e}"))?;
                if resp.status().as_u16() == 404 {
                    return Ok(Arc::new(None));
                }
                if !resp.status().is_success() {
                    return Err(format!(
                        "LAND operators answered HTTP {}",
                        resp.status().as_u16()
                    ));
                }
                resp.json::<LandOperators>()
                    .await
                    .map(|ops| Arc::new(Some(ops)))
                    .map_err(|e| format!("LAND operators bad body: {e}"))
            })
            .await
    }
}

fn store_unavailable(store: &'static str, e: &sqlx::Error) -> AuthorityNotEstablished {
    AuthorityNotEstablished::store_unavailable(store, e)
}

fn reader_unavailable(store: &'static str, reason: &str) -> AuthorityNotEstablished {
    AuthorityNotEstablished::UndeterminedStoreUnavailable {
        store,
        reason_for_operators: reason.to_string(),
    }
}

/// Window of the owner-fallback warning (the name_owner memo's TTL).
const OWNER_FALLBACK_WARN_WINDOW: Duration = Duration::from_secs(30);

/// Admits one warning per window, counting the ones it suppresses.
struct WarnGate {
    window: Duration,
    state: parking_lot::Mutex<(Option<Instant>, u64)>,
}

impl WarnGate {
    fn new(window: Duration) -> Self {
        Self {
            window,
            state: parking_lot::Mutex::new((None, 0)),
        }
    }

    /// `Some(suppressed since the last emission)` when a line should be logged now.
    fn admit(&self, now: Instant) -> Option<u64> {
        let mut state = self.state.lock();
        let (last, suppressed) = &mut *state;
        match *last {
            Some(at) if now.saturating_duration_since(at) < self.window => {
                *suppressed += 1;
                None
            }
            _ => {
                *last = Some(now);
                Some(std::mem::take(suppressed))
            }
        }
    }
}

pub struct Policy {
    pool: PgPool,
    personal_worlds: PersonalWorldsPolicy,
    bans: Arc<dyn BanStatus>,
    /// The name/LAND authority; `None` refuses every ownership question the
    /// row itself cannot answer (the old node without a squid pool).
    marketplace: Option<Arc<ChainIndex>>,
    /// The LAND manager document reader; `None` admits no LAND operator.
    land_operators: Option<Arc<LandOperatorsReader>>,
    /// The LAND authorization index (`land_authz`): the owner and operator
    /// legs once its fold is published, the marketplace reader's owner leg
    /// before that; `None` keeps the two readers above as the only sources.
    land_authz: Option<Arc<LandAuthzState>>,
    owner_fallback_warn: WarnGate,
}

impl Policy {
    /// A `social.scene_bans` row bans `address` from the scene `place_id`.
    pub async fn is_scene_banned(
        &self,
        place_id: &str,
        address: &Address,
    ) -> Result<bool, sqlx::Error> {
        sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM social.scene_bans WHERE place_id = $1 AND banned_address = $2)")
            .bind(place_id)
            .bind(address)
            .fetch_one(&self.pool)
            .await
    }

    pub fn new(
        pool: PgPool,
        personal_worlds: PersonalWorldsPolicy,
        marketplace: Option<Arc<ChainIndex>>,
    ) -> Self {
        Self {
            bans: Arc::new(Moderation::new(pool.clone())),
            pool,
            personal_worlds,
            marketplace,
            land_operators: None,
            land_authz: None,
            owner_fallback_warn: WarnGate::new(OWNER_FALLBACK_WARN_WINDOW),
        }
    }

    pub fn with_ban_status(mut self, bans: Arc<dyn BanStatus>) -> Self {
        self.bans = bans;
        self
    }

    pub fn with_land_operators(mut self, reader: Option<LandOperatorsReader>) -> Self {
        self.land_operators = reader.map(Arc::new);
        self
    }

    pub fn with_land_authz(mut self, land: Option<Arc<LandAuthzState>>) -> Self {
        self.land_authz = land;
        self
    }

    /// Configured chain identity for storage's ownership-history checks.
    pub fn land_authz_settings(&self) -> Option<&crate::land_authz::Settings> {
        self.land_authz.as_ref().map(|land| &land.settings)
    }

    /// The LAND manager document reader, when `COMMS_LAND_OPERATORS_URL` is set.
    pub fn land_operators_reader(&self) -> Option<&Arc<LandOperatorsReader>> {
        self.land_operators.as_ref()
    }

    /// The LAND manager document of a Genesis parcel (the shape of
    /// `/lambdas/parcels/{x}/{y}/operators`): from the `land_authz` index
    /// when its fold is published, else through `COMMS_LAND_OPERATORS_URL`.
    /// `Ok(None)` means no source can answer (neither is configured or
    /// ready), which every caller treats as "no LAND evidence", never as an
    /// empty document; `Ok(Some(None))` is a parcel the source does not know.
    pub async fn land_document(
        &self,
        parcel: Parcel,
    ) -> Result<Option<Arc<Option<LandOperators>>>, AuthorityNotEstablished> {
        if let Some(land) = self.land_authz.as_ref()
            && land.ready().await.is_some()
        {
            let rights = land
                .store()
                .rights(parcel.x, parcel.y)
                .await
                .map_err(|e| store_unavailable("land-authz index", &e))?;
            return Ok(Some(Arc::new(rights.map(|r| LandOperators {
                owner: r.subject.owner,
                operator: r.subject.operator,
                update_operator: r.subject.update_operator,
                update_managers: r.operators.update_managers,
                approved_for_all: r.operators.approved_for_all,
            }))));
        }
        match self.land_operators.as_ref() {
            Some(reader) => reader
                .fetch(parcel)
                .await
                .map(Some)
                .map_err(|e| reader_unavailable("LAND operators reader", &e)),
            None => Ok(None),
        }
    }

    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    pub fn personal_worlds(&self) -> &PersonalWorldsPolicy {
        &self.personal_worlds
    }

    /// The size cap a publish to `world` must stay under.
    pub fn world_size_cap(&self, world: &WorldName) -> i64 {
        self.personal_worlds
            .size_cap_for(world, MAX_WORLD_SIZE_BYTES)
    }

    pub async fn world(&self, world: &WorldName) -> Result<Option<WorldRow>, sqlx::Error> {
        let row = sqlx::query(
            "SELECT name, owner, access_type, access_secret_hash, access_nft, blocked_since, mirror_source \
             FROM worlds.worlds WHERE name = $1",
        )
        .bind(world.key())
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.map(|r| WorldRow {
            name: r.get("name"),
            owner: r
                .get::<Option<String>, _>("owner")
                .and_then(|o| Address::parse(&o).ok()),
            access_type: AccessType::parse(&r.get::<String, _>("access_type"))
                .unwrap_or(AccessType::Unrestricted),
            access_secret_hash: r.get("access_secret_hash"),
            access_nft: r.get("access_nft"),
            blocked_since: r.get("blocked_since"),
            mirror_source: r.get("mirror_source"),
        }))
    }

    /// The names reader, when one is configured (the old node's squid pool).
    pub(crate) fn names_reader(&self) -> Option<&Arc<ChainIndex>> {
        self.marketplace
            .as_ref()
            .filter(|m| m.settings.marketplace_subgraph_url.is_some())
    }

    /// The name authority's answer: does `actor` hold `world`'s label now?
    async fn authority_owns(
        &self,
        market: &ChainIndex,
        world: &WorldName,
        actor: &Address,
    ) -> PermissionDecision {
        match crate::chain_index::graph::owned_names(market, actor.as_str()).await {
            Ok(names)
                if names
                    .iter()
                    .any(|n| n.name.eq_ignore_ascii_case(world.label())) =>
            {
                PermissionDecision::Allowed
            }
            Ok(_) => PermissionDecision::refused(NOT_OWNER_MSG),
            Err(e) => {
                PermissionDecision::Refused(reader_unavailable("marketplace names reader", &e))
            }
        }
    }

    /// Whether `actor` owns `world` (see the decision table): the answer
    /// every mutation takes. `worlds.owner` is the last owner publish's copy
    /// of the authority's answer, consulted only when no reader is
    /// configured; with one, a transferred NAME refuses its previous holder
    /// at once (the reader memoizes for `marketplace::graph::OWNED_TTL`).
    pub async fn is_owner(&self, world: &WorldName, actor: &Address) -> PermissionDecision {
        if let Some(owner) = self.personal_worlds.owner_of(world) {
            return if owner == *actor {
                PermissionDecision::Allowed
            } else {
                PermissionDecision::refused(NOT_OWNER_MSG)
            };
        }
        if let Some(market) = self.names_reader() {
            return self.authority_owns(market, world, actor).await;
        }
        let row = match self.world(world).await {
            Ok(row) => row,
            Err(e) => return PermissionDecision::Refused(store_unavailable("worlds.worlds", &e)),
        };
        match row.and_then(|r| r.owner) {
            Some(owner) if owner == *actor => PermissionDecision::Allowed,
            Some(_) => PermissionDecision::refused(NOT_OWNER_MSG),
            None => {
                tracing::warn!(world = %world, "no marketplace names reader; NAME ownership denied (fail-closed)");
                PermissionDecision::refused(NO_NAME_AUTHORITY_MSG)
            }
        }
    }

    /// Whether a scene `signer` signed could have been published to `world`
    /// here: the authority half of [`Self::can_deploy`] a read-through copy
    /// is held to. The personal world's own address (whatever this realm's
    /// personal-worlds cap), else the NAME holder the names reader names; no
    /// reader refuses (fail-closed) and a reader fault is undetermined. So a
    /// read-through fills only DCL-NAME and personal worlds whose scenes the
    /// NAME holder (the address) signed, and an ENS world is never asked for
    /// (`WorldName::is_dcl_world_name`). A grantee's scene is refused here (the
    /// grants of a world this node does not hold live upstream); the mirror's
    /// `Signatures::upstream_grant` may still take it on the upstream's word
    /// under `WORLDS_READ_THROUGH_TRUST_UPSTREAM_GRANTS`.
    pub async fn may_have_published(
        &self,
        world: &WorldName,
        signer: &Address,
    ) -> PermissionDecision {
        if let Some(owner) = world.personal_owner() {
            return if owner == *signer {
                PermissionDecision::Allowed
            } else {
                PermissionDecision::refused(NOT_OWNER_MSG)
            };
        }
        let Some(market) = self.names_reader() else {
            return PermissionDecision::refused(NO_NAME_AUTHORITY_MSG);
        };
        self.authority_owns(market, world, signer).await
    }

    /// The owner of `world` now: the personal rule, else the name authority's
    /// holder of a DCL NAME (memoized per name for `OWNED_TTL`), else the
    /// stored owner (no reader, an ENS world, or a name the subgraph does not
    /// list). A reader fault is undetermined.
    pub async fn current_owner(
        &self,
        world: &WorldName,
        stored: Option<Address>,
    ) -> Result<Option<Address>, AuthorityNotEstablished> {
        let stored = self.personal_worlds.owner_of(world).or(stored);
        let Some(market) = self
            .names_reader()
            .filter(|_| world.is_dcl_world_name() && world.personal_owner().is_none())
        else {
            return Ok(stored);
        };
        match crate::chain_index::graph::name_owner(market, world.label()).await {
            Ok(holder) => Ok(holder.and_then(|h| Address::parse(&h).ok()).or(stored)),
            Err(e) => Err(reader_unavailable("marketplace names reader", &e)),
        }
    }

    /// [`Self::current_owner`] for the read-side consumers (admission, the
    /// permissions document, protected addresses, notifications): a reader
    /// fault answers the stored owner, so a previous holder may keep the
    /// owner's view for an outage but the holder is never locked out.
    /// Personal worlds never reach the reader, so the personal rule holds.
    pub async fn current_owner_or_stored(
        &self,
        world: &WorldName,
        stored: Option<Address>,
    ) -> Option<Address> {
        match self.current_owner(world, stored.clone()).await {
            Ok(owner) => owner,
            Err(e) => {
                if let Some(suppressed) = self.owner_fallback_warn.admit(Instant::now()) {
                    tracing::warn!(world = %world, error = %e, suppressed, "world owner not resolved; using the stored one");
                }
                stored
            }
        }
    }

    /// Whether `actor` holds `permission` world-wide (no parcel scoping).
    pub async fn has_world_wide_permission(
        &self,
        world: &WorldName,
        permission: Permission,
        actor: &Address,
    ) -> Result<bool, sqlx::Error> {
        sqlx::query_scalar(
            "SELECT EXISTS (SELECT 1 FROM worlds.world_permissions wp \
               WHERE wp.world_name = $1 AND wp.permission = $2 AND wp.address = $3 \
                 AND NOT EXISTS (SELECT 1 FROM worlds.world_permission_parcels wpp \
                                  WHERE (wpp.world_name, wpp.permission, wpp.address) = (wp.world_name, wp.permission, wp.address)))",
        )
        .bind(world.key())
        .bind(permission.as_str())
        .bind(actor)
        .fetch_one(&self.pool)
        .await
    }

    /// Whether `actor` belongs to a community on `world`'s allow-list
    /// ([`COMMUNITY_MEMBERSHIP_SQL`]).
    pub async fn is_listed_community_member(
        &self,
        world: &WorldName,
        actor: &Address,
    ) -> Result<bool, sqlx::Error> {
        sqlx::query_scalar(sqlx::AssertSqlSafe(format!(
            "SELECT EXISTS (SELECT 1 FROM {COMMUNITY_MEMBERSHIP_SQL} AND c.id IN \
               (SELECT community_id FROM worlds.world_access_communities WHERE world_name = $1))"
        )))
        .bind(world.key())
        .bind(actor)
        .fetch_one(&self.pool)
        .await
    }

    /// The ids among `communities` `actor` is a member of, by the same rule.
    pub async fn member_communities(
        &self,
        actor: &Address,
        communities: &[uuid::Uuid],
    ) -> Result<BTreeSet<uuid::Uuid>, sqlx::Error> {
        let ids: Vec<uuid::Uuid> = sqlx::query_scalar(sqlx::AssertSqlSafe(format!(
            "SELECT c.id FROM {COMMUNITY_MEMBERSHIP_SQL} AND c.id = ANY($1)"
        )))
        .bind(communities)
        .bind(actor)
        .fetch_all(&self.pool)
        .await?;
        Ok(ids.into_iter().collect())
    }

    /// Whether `actor` holds `permission` covering every parcel of `required`:
    /// world-wide, or parcel-scoped over the full set (an empty set is
    /// covered by any grant). Both sides canonical `x,y`.
    pub async fn has_permission_covering(
        &self,
        world: &WorldName,
        permission: Permission,
        actor: &Address,
        required: &ParcelSet,
    ) -> Result<bool, sqlx::Error> {
        sqlx::query_scalar(
            "SELECT EXISTS ( \
               SELECT 1 FROM worlds.world_permissions wp \
               LEFT JOIN worlds.world_permission_parcels wpp \
                 ON (wpp.world_name, wpp.permission, wpp.address) = (wp.world_name, wp.permission, wp.address) \
               WHERE wp.world_name = $1 AND wp.permission = $2 AND wp.address = $3 \
               GROUP BY wp.world_name, wp.permission, wp.address \
               HAVING count(wpp.parcel) = 0 \
                   OR count(DISTINCT wpp.parcel) FILTER (WHERE wpp.parcel = ANY($4::text[])) = cardinality($4::text[]))",
        )
        .bind(world.key())
        .bind(permission.as_str())
        .bind(actor)
        .bind(required.canonical())
        .fetch_one(&self.pool)
        .await
    }

    /// The deployed scenes of `world` whose footprint overlaps `parcels`.
    pub async fn overlapping_scenes(
        &self,
        world: &WorldName,
        parcels: &ParcelSet,
    ) -> Result<Vec<(SceneId, ParcelSet)>, sqlx::Error> {
        if parcels.is_empty() {
            return Ok(Vec::new());
        }
        let rows = sqlx::query(
            "SELECT entity_id, parcels FROM worlds.world_scenes \
             WHERE world_name = $1 AND parcels && $2::text[] ORDER BY entity_id",
        )
        .bind(world.key())
        .bind(parcels.canonical())
        .fetch_all(&self.pool)
        .await?;
        Ok(rows
            .into_iter()
            .map(|r| {
                let stored: Vec<String> = r.get("parcels");
                let footprint = stored
                    .iter()
                    .filter_map(|p| Parcel::parse(p).ok())
                    .collect();
                (
                    SceneId::from_stored(r.get::<String, _>("entity_id")),
                    footprint,
                )
            })
            .collect())
    }

    /// Whether `actor` may publish `parcels` to `world`, and what the publish
    /// may replace.
    pub async fn can_deploy(
        &self,
        world: &WorldName,
        actor: &Address,
        parcels: &ParcelSet,
    ) -> Result<Deploy, AuthorityNotEstablished> {
        if let Some(owner) = world.personal_owner() {
            let (already, hosted): (bool, i64) = sqlx::query_as(
                "SELECT EXISTS (SELECT 1 FROM worlds.world_scenes WHERE world_name = $1 AND mirror_source IS NULL), \
                        (SELECT count(DISTINCT world_name) FROM worlds.world_scenes \
                          WHERE world_name ~ '^0x[0-9a-f]{40}\\.dcl\\.eth$' AND mirror_source IS NULL)",
            )
            .bind(world.key())
            .fetch_one(&self.pool)
            .await
            .map_err(|e| store_unavailable("worlds.world_scenes", &e))?;
            return match self.personal_worlds.admit(already, hosted.max(0) as u64) {
                Ok(()) if owner == *actor => Ok(Deploy::Owner),
                Ok(()) => Err(AuthorityNotEstablished::RefusedLacksAuthority {
                    detail: format!("The signer {actor} is not authorized to deploy to the world {world}"),
                }),
                Err(PersonalWorldDeny::Disabled) => Err(AuthorityNotEstablished::RefusedLacksAuthority {
                    detail: "Not authorized: personal test worlds are not enabled on this realm (deploy denied)".to_string(),
                }),
                Err(PersonalWorldDeny::RealmFull { max_worlds }) => Err(AuthorityNotEstablished::RefusedLacksAuthority {
                    detail: format!(
                        "Not authorized: this realm already hosts its maximum of {max_worlds} personal test worlds (deploy denied)"
                    ),
                }),
            };
        }
        let Some(market) = self.names_reader() else {
            tracing::warn!(world = %world, signer = %actor, "deploy denied: no marketplace names reader, cannot resolve NAME ownership (fail-closed)");
            return Err(AuthorityNotEstablished::RefusedLacksAuthority {
                detail: format!("Not authorized: {NO_NAME_AUTHORITY_MSG} (deploy denied)"),
            });
        };
        match self.authority_owns(market, world, actor).await {
            PermissionDecision::Allowed => return Ok(Deploy::Owner),
            PermissionDecision::Refused(AuthorityNotEstablished::RefusedLacksAuthority {
                ..
            }) => {}
            PermissionDecision::Refused(undetermined) => return Err(undetermined),
        }
        let overlapping = self
            .overlapping_scenes(world, parcels)
            .await
            .map_err(|e| store_unavailable("worlds.world_scenes", &e))?;
        let mut required = parcels.clone();
        for (_, footprint) in &overlapping {
            required.extend(footprint.iter().copied());
        }
        let covered = self
            .has_permission_covering(world, Permission::Deployment, actor, &required)
            .await
            .map_err(|e| store_unavailable("worlds.world_permissions", &e))?;
        if !covered {
            return Err(AuthorityNotEstablished::RefusedLacksAuthority {
                detail: format!(
                    "The signer {actor} is not authorized to deploy to the world {world}"
                ),
            });
        }
        Ok(Deploy::Scoped(
            overlapping.into_iter().map(|(id, _)| id).collect(),
        ))
    }

    /// The owner, or a `streaming` grant covering `parcels` (world-wide when
    /// `None`).
    pub async fn can_stream(
        &self,
        world: &WorldName,
        actor: &Address,
        parcels: Option<&ParcelSet>,
    ) -> PermissionDecision {
        match self.is_owner(world, actor).await {
            PermissionDecision::Allowed => return PermissionDecision::Allowed,
            PermissionDecision::Refused(AuthorityNotEstablished::RefusedLacksAuthority {
                ..
            }) => {}
            undetermined => return undetermined,
        }
        let granted = match parcels {
            Some(p) if !p.is_empty() => {
                self.has_permission_covering(world, Permission::Streaming, actor, p)
                    .await
            }
            _ => {
                self.has_world_wide_permission(world, Permission::Streaming, actor)
                    .await
            }
        };
        match granted {
            Ok(true) => PermissionDecision::Allowed,
            Ok(false) => PermissionDecision::refused(
                "signer holds no streaming permission covering these parcels",
            ),
            Err(e) => PermissionDecision::undetermined("worlds.world_permissions", &e),
        }
    }

    /// Whether `actor` may enter `world` (a scene room when `scene` is
    /// given; the world lobby otherwise). `device_id` widens the platform
    /// ban to the caller's device, as the comms gate does.
    pub async fn can_access(
        &self,
        world: &WorldName,
        actor: &Address,
        secret: Option<&str>,
        scene: Option<&SceneId>,
        device_id: Option<&str>,
        now: NaiveDateTime,
    ) -> Result<Access, AuthorityNotEstablished> {
        if let Some(ban) = self.is_banned(actor, scene, device_id, now).await? {
            return Ok(Access::Banned(ban));
        }
        let row = self
            .world(world)
            .await
            .map_err(|e| store_unavailable("worlds.worlds", &e))?;
        // An owner-lookup outage falls back to the stored owner, never a 503.
        let owner = self
            .current_owner_or_stored(world, row.as_ref().and_then(|r| r.owner.clone()))
            .await;
        if owner.as_ref() == Some(actor) {
            return Ok(Access::Allowed(AccessGrant::Owner));
        }
        let access_type = row
            .as_ref()
            .map(|r| r.access_type)
            .unwrap_or(AccessType::Unrestricted);
        Ok(match access_type {
            AccessType::Unrestricted => Access::Allowed(AccessGrant::Unrestricted),
            AccessType::NftOwnership => Access::NotAllowed,
            AccessType::AllowList => {
                let listed = self
                    .has_world_wide_permission(world, Permission::Access, actor)
                    .await
                    .map_err(|e| store_unavailable("worlds.world_permissions", &e))?;
                if listed {
                    Access::Allowed(AccessGrant::AllowListed)
                } else if self
                    .is_listed_community_member(world, actor)
                    .await
                    .map_err(|e| store_unavailable("social.community_members", &e))?
                {
                    Access::Allowed(AccessGrant::Community)
                } else {
                    Access::NotAllowed
                }
            }
            AccessType::SharedSecret => {
                let hash = row.as_ref().and_then(|r| r.access_secret_hash.as_deref());
                match (hash, secret) {
                    (Some(hash), Some(secret)) if bcrypt::verify(secret, hash).unwrap_or(false) => {
                        Access::Allowed(AccessGrant::SharedSecret)
                    }
                    _ => Access::InvalidSecret,
                }
            }
        })
    }

    /// Worlds owns the operator block list; comms owns platform/device/scene bans.
    pub async fn is_blocked(&self, actor: &Address) -> Result<bool, AuthorityNotEstablished> {
        sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM worlds.blocked_wallets WHERE wallet = $1)")
            .bind(actor)
            .fetch_one(&self.pool)
            .await
            .map_err(|e| store_unavailable("worlds.blocked_wallets", &e))
    }

    /// Full permission answer: operator block first, then comms-owned platform/scene bans.
    pub async fn is_banned(
        &self,
        actor: &Address,
        scene: Option<&SceneId>,
        device_id: Option<&str>,
        now: NaiveDateTime,
    ) -> Result<Option<Ban>, AuthorityNotEstablished> {
        if self.is_blocked(actor).await? {
            return Ok(Some(Ban::Blocked));
        }
        self.bans.ban(actor, scene, device_id, now).await
    }

    /// An explicit scene admin, else the owner of the world (world scene) or
    /// of the LAND (Genesis scene).
    pub async fn is_admin(&self, scene: &SceneRef<'_>, actor: &Address) -> PermissionDecision {
        let explicit: Result<Option<bool>, sqlx::Error> = sqlx::query_scalar(
            "SELECT active FROM social.scene_admins WHERE place_id = $1 AND admin_address = $2",
        )
        .bind(scene.scene_id())
        .bind(actor)
        .fetch_optional(&self.pool)
        .await;
        match explicit {
            Ok(Some(true)) => return PermissionDecision::Allowed,
            Ok(_) => {}
            Err(e) => return PermissionDecision::undetermined("social.scene_admins", &e),
        }
        match scene {
            SceneRef::World { world, .. } => match self.is_owner(world, actor).await {
                PermissionDecision::Allowed => PermissionDecision::Allowed,
                PermissionDecision::Refused(AuthorityNotEstablished::RefusedLacksAuthority {
                    ..
                }) => PermissionDecision::refused(NOT_OWNER_OR_ADMIN_MSG),
                undetermined => undetermined,
            },
            SceneRef::Genesis { parcels, .. } => {
                let coords: Vec<(i32, i32)> = parcels.iter().map(|p| (p.x, p.y)).collect();
                self.owns_land_at(&coords, actor).await
            }
        }
    }

    /// LAND ownership at any of `coords`, directly or through the estate.
    async fn owns_land_at(&self, coords: &[(i32, i32)], actor: &Address) -> PermissionDecision {
        if coords.is_empty() {
            return PermissionDecision::refused(NOT_OWNER_OR_ADMIN_MSG);
        }
        if let Some(land) = self.land_authz.as_ref() {
            return match land_authz::owns_land_at(land, actor.as_str(), coords).await {
                Ok(true) => PermissionDecision::Allowed,
                Ok(false) => PermissionDecision::refused(NOT_OWNER_OR_ADMIN_MSG),
                Err(land_authz::Unavailable::NoSource) => {
                    tracing::warn!(
                        "scene authz: no LAND rights source (index not ready, no marketplace subgraph); refusing without LAND ownership evidence"
                    );
                    PermissionDecision::refused(NOT_OWNER_OR_ADMIN_MSG)
                }
                Err(e) => PermissionDecision::Refused(reader_unavailable(
                    "LAND rights source",
                    &e.to_string(),
                )),
            };
        }
        let Some(market) = self.marketplace.as_ref() else {
            tracing::warn!(
                "scene authz: no marketplace LAND reader; refusing without LAND ownership evidence"
            );
            return PermissionDecision::refused(NOT_OWNER_OR_ADMIN_MSG);
        };
        match crate::chain_index::graph::owns_land_at(market, actor.as_str(), coords).await {
            Ok(true) => PermissionDecision::Allowed,
            Ok(false) => PermissionDecision::refused(NOT_OWNER_OR_ADMIN_MSG),
            Err(e) => {
                PermissionDecision::Refused(reader_unavailable("marketplace LAND reader", &e))
            }
        }
    }

    /// Whether `actor` may manage the storage of the Genesis scene standing
    /// on `parcel`: LAND authority at that parcel only (the old world-storage
    /// lambdas matrix): owner, operator, updateOperator, updateManager or
    /// approvedForAll in its manager document; with no document source, LAND
    /// ownership there. Scene admins get nothing here.
    pub async fn may_manage_land_storage(
        &self,
        parcel: Parcel,
        actor: &Address,
    ) -> PermissionDecision {
        match self.land_document(parcel).await {
            Ok(None) => self.owns_land_at(&[(parcel.x, parcel.y)], actor).await,
            Ok(Some(ops)) if ops.as_ref().as_ref().is_some_and(|o| o.manages(actor)) => {
                PermissionDecision::Allowed
            }
            Ok(Some(_)) => PermissionDecision::refused(NOT_OWNER_OR_ADMIN_MSG),
            Err(e) => PermissionDecision::Refused(e),
        }
    }

    /// The addresses a world's other permission surfaces protect from a scene
    /// ban: the current owner (stored on a reader fault) and every
    /// deployment/streaming grantee whose grant is world-wide or touches one
    /// of `parcels` (the old `/permissions/{perm}/parcels` listing plus the
    /// document's owner).
    pub async fn protected_addresses(
        &self,
        world: &WorldName,
        parcels: &ParcelSet,
    ) -> Result<BTreeSet<Address>, AuthorityNotEstablished> {
        let row = self
            .world(world)
            .await
            .map_err(|e| store_unavailable("worlds.worlds", &e))?;
        let mut out = BTreeSet::new();
        if let Some(owner) = self
            .current_owner_or_stored(world, row.and_then(|r| r.owner))
            .await
        {
            out.insert(owner);
        }
        let grantees: Vec<String> = sqlx::query_scalar(
            "SELECT DISTINCT wp.address FROM worlds.world_permissions wp \
             WHERE wp.world_name = $1 AND wp.permission IN ('deployment', 'streaming') \
               AND (NOT EXISTS (SELECT 1 FROM worlds.world_permission_parcels wpp \
                                 WHERE (wpp.world_name, wpp.permission, wpp.address) = (wp.world_name, wp.permission, wp.address)) \
                    OR EXISTS (SELECT 1 FROM worlds.world_permission_parcels wpp \
                                WHERE (wpp.world_name, wpp.permission, wpp.address) = (wp.world_name, wp.permission, wp.address) \
                                  AND wpp.parcel = ANY($2::text[])))",
        )
        .bind(world.key())
        .bind(parcels.canonical())
        .fetch_all(&self.pool)
        .await
        .map_err(|e| store_unavailable("worlds.world_permissions", &e))?;
        out.extend(grantees.iter().filter_map(|a| Address::parse(a).ok()));
        Ok(out)
    }
}

/// The questions comms asks the permission core (the archipelago ban check),
/// each answered by the inherent method of the same name.
#[async_trait::async_trait]
impl crate::comms::interfaces::AccessPolicy for Policy {
    async fn access_type(
        &self,
        world: &WorldName,
    ) -> Result<Option<AccessType>, AuthorityNotEstablished> {
        Policy::world(self, world)
            .await
            .map(|row| row.map(|row| row.access_type))
            .map_err(|e| store_unavailable("worlds.worlds", &e))
    }

    async fn world_owner(
        &self,
        world: &WorldName,
    ) -> Result<Option<Address>, AuthorityNotEstablished> {
        let stored = Policy::world(self, world)
            .await
            .map_err(|e| store_unavailable("worlds.worlds", &e))?
            .and_then(|row| row.owner);
        Ok(self.current_owner_or_stored(world, stored).await)
    }

    async fn land_document(
        &self,
        parcel: Parcel,
    ) -> Result<Option<Arc<Option<LandOperators>>>, AuthorityNotEstablished> {
        Policy::land_document(self, parcel).await
    }

    async fn can_access(
        &self,
        world: &WorldName,
        actor: &Address,
        secret: Option<&str>,
        scene: Option<&SceneId>,
        device_id: Option<&str>,
        now: NaiveDateTime,
    ) -> Result<Access, AuthorityNotEstablished> {
        Policy::can_access(self, world, actor, secret, scene, device_id, now).await
    }

    async fn is_blocked(&self, actor: &Address) -> Result<bool, AuthorityNotEstablished> {
        Policy::is_blocked(self, actor).await
    }

    async fn is_admin(&self, scene: &SceneRef<'_>, actor: &Address) -> PermissionDecision {
        Policy::is_admin(self, scene, actor).await
    }

    async fn protected_addresses(
        &self,
        world: &WorldName,
        parcels: &ParcelSet,
    ) -> Result<BTreeSet<Address>, AuthorityNotEstablished> {
        Policy::protected_addresses(self, world, parcels).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chain_index::graph::test_support::{Names, index_with_names};
    use uuid::Uuid;

    const ADDR: &str = "0x4d02aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa7bf2";
    const CAP: i64 = 300 * 1024 * 1024;

    fn policy(max_worlds: u64) -> PersonalWorldsPolicy {
        PersonalWorldsPolicy::new(max_worlds, 1024, CAP).unwrap()
    }

    #[test]
    fn the_owner_fallback_warning_is_emitted_once_per_window_with_the_suppressed_count() {
        let gate = WarnGate::new(Duration::from_secs(30));
        let t0 = Instant::now();
        assert_eq!(gate.admit(t0), Some(0));
        assert_eq!(gate.admit(t0 + Duration::from_secs(1)), None);
        assert_eq!(gate.admit(t0 + Duration::from_secs(10)), None);
        assert_eq!(gate.admit(t0 + Duration::from_secs(29)), None);
        assert_eq!(gate.admit(t0 + Duration::from_secs(30)), Some(3));
        assert_eq!(gate.admit(t0 + Duration::from_secs(31)), None);
        assert_eq!(gate.admit(t0 + Duration::from_secs(60)), Some(1));
    }

    #[test]
    fn the_policy_is_off_unless_the_operator_sets_a_cap() {
        let mine = WorldName::parse(&format!("{ADDR}.dcl.eth")).unwrap();
        let off = policy(0);
        assert!(!off.enabled());
        assert_eq!(off.owner_of(&mine), None);
        assert_eq!(off.size_cap_for(&mine, CAP), CAP);
        assert_eq!(off.admit(false, 0), Err(PersonalWorldDeny::Disabled));
        let on = policy(1);
        assert_eq!(on.owner_of(&mine), Some(Address::parse(ADDR).unwrap()));
        assert_eq!(
            on.owner_of(&WorldName::parse("boedo.dcl.eth").unwrap()),
            None
        );
        assert_eq!(on.size_cap_for(&mine, CAP), 1024);
        assert!(PersonalWorldsPolicy::new(1, 0, CAP).is_err());
        assert!(PersonalWorldsPolicy::new(1, CAP as u64 + 1, CAP).is_err());
        assert!(PersonalWorldsPolicy::new(1, CAP as u64, CAP).is_ok());
    }

    #[test]
    fn the_realm_cap_counts_only_new_personal_worlds() {
        let on = policy(1);
        assert_eq!(on.admit(false, 0), Ok(()));
        assert_eq!(
            on.admit(false, 1),
            Err(PersonalWorldDeny::RealmFull { max_worlds: 1 })
        );
        assert_eq!(on.admit(true, 1), Ok(()));
        assert_eq!(policy(2).admit(false, 1), Ok(()));
    }

    #[test]
    fn grant_and_access_spellings_are_the_old_wire_values() {
        for p in [
            Permission::Deployment,
            Permission::Streaming,
            Permission::Access,
        ] {
            assert_eq!(Permission::parse(p.as_str()), Some(p));
        }
        assert!(
            Permission::Deployment.supports_parcels() && !Permission::Access.supports_parcels()
        );
        assert_eq!(Permission::parse("owner"), None);
        for a in [
            AccessType::Unrestricted,
            AccessType::SharedSecret,
            AccessType::NftOwnership,
            AccessType::AllowList,
        ] {
            assert_eq!(AccessType::parse(a.as_str()), Some(a));
        }
    }

    fn wallet() -> Address {
        Address::parse(&format!("0x00000000{}", Uuid::new_v4().simple())).unwrap()
    }

    fn world_name() -> WorldName {
        WorldName::parse(&format!(
            "W{}.dcl.eth",
            &Uuid::new_v4().simple().to_string()[..10]
        ))
        .unwrap()
    }

    fn now() -> NaiveDateTime {
        chrono::Utc::now().naive_utc()
    }

    async fn scratch() -> Option<Policy> {
        let pool = crate::db::scratch::pool(4).await?;
        Some(Policy::new(pool, PersonalWorldsPolicy::disabled(), None))
    }

    #[tokio::test]
    async fn world_owner_seam_reports_store_fault_instead_of_unknown_owner() {
        use crate::comms::interfaces::AccessPolicy;
        let pool = PgPool::connect_lazy("postgres://postgres@127.0.0.1:1/postgres").unwrap();
        pool.close().await;
        let p = Policy::new(pool, policy(1), None);
        let world = WorldName::parse(&format!("{ADDR}.dcl.eth")).unwrap();
        assert!(matches!(
            p.world_owner(&world).await,
            Err(AuthorityNotEstablished::UndeterminedStoreUnavailable {
                store: "worlds.worlds",
                ..
            })
        ));
    }

    #[tokio::test]
    async fn world_owner_seam_answers_the_current_owner() {
        use crate::comms::interfaces::AccessPolicy;
        let Some(pool) = crate::db::scratch::pool(4).await else {
            return;
        };
        let p = Policy::new(pool, policy(1), None);
        let personal = wallet();
        let world = WorldName::parse(&format!("{}.dcl.eth", personal.as_str())).unwrap();
        assert_eq!(
            p.world_owner(&world).await.unwrap(),
            Some(personal.clone()),
            "the personal rule needs no row"
        );
        seed_world(&p, &world, Some(&wallet()), AccessType::Unrestricted, None).await;
        assert_eq!(p.world_owner(&world).await.unwrap(), Some(personal));
        let (named, stored) = (world_name(), wallet());
        assert_eq!(p.world_owner(&named).await.unwrap(), None);
        seed_world(&p, &named, Some(&stored), AccessType::Unrestricted, None).await;
        assert_eq!(p.world_owner(&named).await.unwrap(), Some(stored));
    }

    #[tokio::test]
    async fn ban_boundary_uses_blocklist_first_then_the_injected_comms_answer() {
        use crate::comms::gatekeeper::moderation::test_support::StubBanStatus;
        let Some(pool) = crate::db::scratch::pool(4).await else {
            return;
        };
        let actor = wallet();
        let scene = SceneId::parse("test-scene").unwrap();
        let p = Policy::new(pool.clone(), PersonalWorldsPolicy::disabled(), None)
            .with_ban_status(Arc::new(StubBanStatus(Ok(Some(Ban::Platform)))));
        assert_eq!(
            p.is_banned(&actor, Some(&scene), Some("device"), now())
                .await
                .unwrap(),
            Some(Ban::Platform)
        );
        // Raw provider failure cannot override the definite blocked-wallet result.
        let p = p.with_ban_status(Arc::new(StubBanStatus(Err(
            AuthorityNotEstablished::store_unavailable(
                "social.user_bans",
                &sqlx::Error::PoolClosed,
            ),
        ))));
        assert!(
            p.is_banned(&actor, Some(&scene), None, now())
                .await
                .is_err()
        );
        sqlx::query("INSERT INTO worlds.blocked_wallets (wallet) VALUES ($1)")
            .bind(&actor)
            .execute(&pool)
            .await
            .unwrap();
        assert_eq!(
            p.is_banned(&actor, Some(&scene), None, now())
                .await
                .unwrap(),
            Some(Ban::Blocked)
        );
    }

    /// A policy with the loopback name authority and its registry.
    async fn scratch_with_names() -> Option<(Policy, Names, Arc<ChainIndex>)> {
        let pool = crate::db::scratch::pool(4).await?;
        let (market, names) = index_with_names().await;
        Some((
            Policy::new(pool, PersonalWorldsPolicy::disabled(), Some(market.clone())),
            names,
            market,
        ))
    }

    async fn seed_world(
        p: &Policy,
        world: &WorldName,
        owner: Option<&Address>,
        access: AccessType,
        secret_hash: Option<&str>,
    ) {
        sqlx::query(
            "INSERT INTO worlds.worlds (name, owner, access_type, access_secret_hash, access_nft) \
             VALUES ($1, $2, $3, $4, CASE WHEN $3 = 'nft-ownership' THEN 'urn:x' END)",
        )
        .bind(world.key())
        .bind(owner)
        .bind(access.as_str())
        .bind(secret_hash)
        .execute(p.pool())
        .await
        .unwrap();
    }

    async fn seed_scene(p: &Policy, world: &WorldName, id: &str, parcels: &[&str]) {
        sqlx::query(
            "INSERT INTO worlds.world_scenes (world_name, entity_id, deployment_auth_chain, entity, deployer, parcels, size) \
             VALUES ($1, $2, '[]'::json, '{}'::jsonb, $3, $4, 1)",
        )
        .bind(world.key())
        .bind(id)
        .bind(crate::ZERO_ADDRESS)
        .bind(parcels.iter().map(|s| s.to_string()).collect::<Vec<_>>())
        .execute(p.pool())
        .await
        .unwrap();
    }

    async fn grant(
        p: &Policy,
        world: &WorldName,
        permission: Permission,
        who: &Address,
        parcels: &[&str],
    ) {
        sqlx::query("INSERT INTO worlds.world_permissions (world_name, permission, address) VALUES ($1, $2, $3) ON CONFLICT DO NOTHING")
            .bind(world.key())
            .bind(permission.as_str())
            .bind(who)
            .execute(p.pool())
            .await
            .unwrap();
        for parcel in parcels {
            sqlx::query("INSERT INTO worlds.world_permission_parcels (world_name, permission, address, parcel) VALUES ($1, $2, $3, $4)")
                .bind(world.key())
                .bind(permission.as_str())
                .bind(who)
                .bind(parcel)
                .execute(p.pool())
                .await
                .unwrap();
        }
    }

    fn parcels(raw: &[&str]) -> ParcelSet {
        ParcelSet::parse(raw).unwrap()
    }

    #[tokio::test]
    async fn the_name_holder_deploys_unrestricted_and_a_stranger_is_refused() {
        let Some((p, names, _)) = scratch_with_names().await else {
            return;
        };
        let (world, owner, stranger) = (world_name(), wallet(), wallet());
        names.set(&owner, world.label());
        seed_world(&p, &world, Some(&owner), AccessType::Unrestricted, None).await;
        seed_scene(&p, &world, "bafyA", &["0,0", "0,1"]).await;
        assert_eq!(
            p.can_deploy(&world, &owner, &parcels(&["0,1"]))
                .await
                .unwrap(),
            Deploy::Owner
        );
        assert!(p.is_owner(&world, &owner).await.is_allowed());
        let err = p
            .can_deploy(&world, &stranger, &parcels(&["0,1"]))
            .await
            .unwrap_err();
        assert_eq!(err.http_status(), 403);
        assert_eq!(
            p.is_owner(&world, &stranger)
                .await
                .into_result()
                .unwrap_err()
                .http_status(),
            403
        );
        // A NAME nobody holds: refused, not undetermined.
        let unknown = world_name();
        assert_eq!(
            p.is_owner(&unknown, &stranger)
                .await
                .into_result()
                .unwrap_err()
                .http_status(),
            403
        );
    }

    /// `worlds.owner` is a cache of the authority's answer, not the authority:
    /// once the NAME moves, the stored owner loses every mutation and, after
    /// the name memo expires, admission and every owner read too.
    #[tokio::test]
    async fn a_transferred_name_refuses_its_previous_holder() {
        let Some((p, names, market)) = scratch_with_names().await else {
            return;
        };
        let (world, old, new) = (world_name(), wallet(), wallet());
        names.set(&old, world.label());
        seed_world(&p, &world, Some(&old), AccessType::AllowList, None).await;
        seed_scene(&p, &world, "bafyT", &["0,0"]).await;
        assert!(p.is_owner(&world, &old).await.is_allowed());
        names.forget(&old);
        names.set(&new, world.label());
        market.graph.names.clear();
        assert_eq!(
            p.can_deploy(&world, &old, &parcels(&["0,0"]))
                .await
                .unwrap_err()
                .http_status(),
            403
        );
        assert_eq!(
            p.is_owner(&world, &old)
                .await
                .into_result()
                .unwrap_err()
                .http_status(),
            403
        );
        assert_eq!(
            p.can_stream(&world, &old, None)
                .await
                .into_result()
                .unwrap_err()
                .http_status(),
            403
        );
        assert_eq!(
            p.can_deploy(&world, &new, &parcels(&["0,0"]))
                .await
                .unwrap(),
            Deploy::Owner
        );
        assert!(p.is_owner(&world, &new).await.is_allowed());
        market.graph.name_owner.clear();
        assert_eq!(
            p.can_access(&world, &old, None, None, None, now())
                .await
                .unwrap(),
            Access::NotAllowed,
            "the stored owner no longer bypasses the allow-list"
        );
        assert_eq!(
            p.can_access(&world, &new, None, None, None, now())
                .await
                .unwrap(),
            Access::Allowed(AccessGrant::Owner),
            "no republish needed"
        );
        let protected = p
            .protected_addresses(&world, &parcels(&["0,0"]))
            .await
            .unwrap();
        assert!(protected.contains(&new) && !protected.contains(&old));
        assert_eq!(
            crate::comms::interfaces::AccessPolicy::world_owner(&p, &world)
                .await
                .unwrap(),
            Some(new)
        );
    }

    #[tokio::test]
    async fn a_name_the_subgraph_does_not_list_keeps_its_stored_owner_at_admission() {
        let Some((p, _names, _market)) = scratch_with_names().await else {
            return;
        };
        let (world, stored, stranger) = (world_name(), wallet(), wallet());
        seed_world(&p, &world, Some(&stored), AccessType::AllowList, None).await;
        assert_eq!(
            p.current_owner(&world, Some(stored.clone())).await.unwrap(),
            Some(stored.clone())
        );
        assert_eq!(
            p.can_access(&world, &stored, None, None, None, now())
                .await
                .unwrap(),
            Access::Allowed(AccessGrant::Owner)
        );
        assert_eq!(
            p.can_access(&world, &stranger, None, None, None, now())
                .await
                .unwrap(),
            Access::NotAllowed
        );
    }

    /// The name authority down: the stored owner keeps the owner bypass and
    /// every other visitor goes through the access setting; nothing 503s.
    #[tokio::test]
    async fn an_authority_fault_falls_back_to_the_stored_owner() {
        use crate::comms::interfaces::AccessPolicy;
        let Some(pool) = crate::db::scratch::pool(4).await else {
            return;
        };
        let settings = crate::chain_index::IndexSettings {
            marketplace_subgraph_url: Some("http://127.0.0.1:1/subgraph".to_string()),
            ..crate::chain_index::IndexSettings::for_tests()
        };
        let market = ChainIndex::new(Outbound::for_tests(), settings);
        market.allow_loopback_for_tests();
        let p = Policy::new(
            pool,
            PersonalWorldsPolicy::disabled(),
            Some(Arc::new(market)),
        );
        let (open, closed) = (world_name(), world_name());
        let (owner, listed, visitor) = (wallet(), wallet(), wallet());
        seed_world(&p, &open, Some(&owner), AccessType::Unrestricted, None).await;
        seed_world(&p, &closed, Some(&owner), AccessType::AllowList, None).await;
        grant(&p, &closed, Permission::Access, &listed, &[]).await;
        assert!(p.current_owner(&closed, Some(owner.clone())).await.is_err());
        assert_eq!(
            p.can_access(&closed, &owner, None, None, None, now())
                .await
                .unwrap(),
            Access::Allowed(AccessGrant::Owner)
        );
        assert_eq!(
            p.can_access(&closed, &listed, None, None, None, now())
                .await
                .unwrap(),
            Access::Allowed(AccessGrant::AllowListed)
        );
        assert_eq!(
            p.can_access(&closed, &visitor, None, None, None, now())
                .await
                .unwrap(),
            Access::NotAllowed
        );
        assert_eq!(
            p.can_access(&open, &visitor, None, None, None, now())
                .await
                .unwrap(),
            Access::Allowed(AccessGrant::Unrestricted)
        );
        assert_eq!(p.world_owner(&closed).await.unwrap(), Some(owner.clone()));
        let protected = p
            .protected_addresses(&closed, &parcels(&["0,0"]))
            .await
            .unwrap();
        assert!(protected.contains(&owner));
    }

    /// Without a names reader every non-personal publish is refused (the old
    /// squid-less node's rule) while the routes that only need the owner
    /// fall back to the stored column, as the old permission routes did.
    #[tokio::test]
    async fn without_a_names_reader_publishes_fail_closed_and_the_stored_owner_still_answers() {
        let Some(p) = scratch().await else { return };
        let (world, owner, stranger) = (world_name(), wallet(), wallet());
        seed_world(&p, &world, Some(&owner), AccessType::Unrestricted, None).await;
        let err = p
            .can_deploy(&world, &owner, &parcels(&["0,0"]))
            .await
            .unwrap_err();
        assert_eq!(err.http_status(), 403);
        assert!(err.to_string().contains(NO_NAME_AUTHORITY_MSG), "{err}");
        assert!(p.is_owner(&world, &owner).await.is_allowed());
        assert_eq!(
            p.is_owner(&world, &stranger)
                .await
                .into_result()
                .unwrap_err()
                .http_status(),
            403
        );
        assert_eq!(
            p.is_owner(&world_name(), &stranger)
                .await
                .into_result()
                .unwrap_err()
                .http_status(),
            403
        );
    }

    #[tokio::test]
    async fn a_scoped_deployer_must_cover_the_footprint_of_every_overlapping_scene() {
        let Some((p, names, _)) = scratch_with_names().await else {
            return;
        };
        let (world, owner, deployer) = (world_name(), wallet(), wallet());
        names.set(&owner, world.label());
        seed_world(&p, &world, Some(&owner), AccessType::Unrestricted, None).await;
        seed_scene(&p, &world, "bafyBig", &["0,0", "0,1", "1,1"]).await;
        seed_scene(&p, &world, "bafyFar", &["5,5"]).await;
        grant(
            &p,
            &world,
            Permission::Deployment,
            &deployer,
            &["0,0", "0,1"],
        )
        .await;
        // Deploying onto 0,0 overlaps bafyBig, whose footprint reaches 1,1:
        // the grant does not cover 1,1.
        assert_eq!(
            p.can_deploy(&world, &deployer, &parcels(&["0,0"]))
                .await
                .unwrap_err()
                .http_status(),
            403
        );
        assert!(
            !p.has_permission_covering(
                &world,
                Permission::Deployment,
                &deployer,
                &parcels(&["0,0", "1,1"])
            )
            .await
            .unwrap()
        );
        assert!(
            p.has_permission_covering(
                &world,
                Permission::Deployment,
                &deployer,
                &parcels(&["0,1"])
            )
            .await
            .unwrap()
        );
        assert!(
            !p.has_world_wide_permission(&world, Permission::Deployment, &deployer)
                .await
                .unwrap()
        );
        // Widen the grant: now the overlapping scene is exactly what may be replaced.
        grant(&p, &world, Permission::Deployment, &deployer, &["1,1"]).await;
        assert_eq!(
            p.can_deploy(&world, &deployer, &parcels(&["0,0"]))
                .await
                .unwrap(),
            Deploy::Scoped(vec![SceneId::parse("bafyBig").unwrap()])
        );
        // A free parcel needs only itself.
        assert_eq!(
            p.can_deploy(&world, &deployer, &parcels(&["0,1"]))
                .await
                .unwrap(),
            Deploy::Scoped(vec![SceneId::parse("bafyBig").unwrap()])
        );
        assert_eq!(
            p.can_deploy(&world, &deployer, &parcels(&["9,9"]))
                .await
                .unwrap_err()
                .http_status(),
            403
        );
        // A world-wide grant covers anything.
        let wide = wallet();
        grant(&p, &world, Permission::Deployment, &wide, &[]).await;
        assert_eq!(
            p.can_deploy(&world, &wide, &parcels(&["5,5", "9,9"]))
                .await
                .unwrap(),
            Deploy::Scoped(vec![SceneId::parse("bafyFar").unwrap()])
        );
        assert!(
            p.has_world_wide_permission(&world, Permission::Deployment, &wide)
                .await
                .unwrap()
        );
        // Streaming is its own grant.
        assert_eq!(
            p.can_stream(&world, &wide, None)
                .await
                .into_result()
                .unwrap_err()
                .http_status(),
            403
        );
        grant(&p, &world, Permission::Streaming, &wide, &[]).await;
        assert!(
            p.can_stream(&world, &wide, Some(&parcels(&["5,5"])))
                .await
                .is_allowed()
        );
        assert!(p.can_stream(&world, &owner, None).await.is_allowed());
    }

    #[tokio::test]
    async fn access_follows_the_old_mint_order() {
        let Some(p) = scratch().await else { return };
        let (world, owner, visitor) = (world_name(), wallet(), wallet());
        let hash = bcrypt::hash("open sesame", 4).unwrap();
        seed_world(
            &p,
            &world,
            Some(&owner),
            AccessType::SharedSecret,
            Some(&hash),
        )
        .await;
        seed_scene(&p, &world, "bafyS", &["0,0"]).await;
        let scene = SceneId::parse("bafyS").unwrap();
        assert_eq!(
            p.can_access(&world, &owner, None, Some(&scene), None, now())
                .await
                .unwrap(),
            Access::Allowed(AccessGrant::Owner)
        );
        assert_eq!(
            p.can_access(&world, &visitor, None, Some(&scene), None, now())
                .await
                .unwrap(),
            Access::InvalidSecret
        );
        assert_eq!(
            p.can_access(&world, &visitor, Some("wrong"), None, None, now())
                .await
                .unwrap(),
            Access::InvalidSecret
        );
        assert_eq!(
            p.can_access(&world, &visitor, Some("open sesame"), None, None, now())
                .await
                .unwrap(),
            Access::Allowed(AccessGrant::SharedSecret)
        );
        // A scene ban outranks the secret; a platform ban outranks the scene ban.
        sqlx::query("INSERT INTO social.scene_bans (place_id, banned_address, banned_by) VALUES ($1, $2, $3)")
            .bind(scene.as_str())
            .bind(&visitor)
            .bind(&owner)
            .execute(p.pool())
            .await
            .unwrap();
        assert_eq!(
            p.can_access(
                &world,
                &visitor,
                Some("open sesame"),
                Some(&scene),
                None,
                now()
            )
            .await
            .unwrap(),
            Access::Banned(Ban::Scene)
        );
        assert_eq!(
            p.can_access(&world, &visitor, Some("open sesame"), None, None, now())
                .await
                .unwrap(),
            Access::Allowed(AccessGrant::SharedSecret),
            "the lobby room carries no scene ban"
        );
        sqlx::query("INSERT INTO social.user_bans (banned_address, banned_by, reason, expires_at) VALUES ($1, 'mod', 'r', $2)")
            .bind(&visitor)
            .bind(now() + chrono::Duration::hours(1))
            .execute(p.pool())
            .await
            .unwrap();
        assert_eq!(
            p.is_banned(&visitor, Some(&scene), None, now())
                .await
                .unwrap(),
            Some(Ban::Platform)
        );
        assert_eq!(
            p.is_banned(
                &visitor,
                Some(&scene),
                None,
                now() + chrono::Duration::hours(2)
            )
            .await
            .unwrap(),
            Some(Ban::Scene),
            "expired at the caller's now"
        );
        sqlx::query("INSERT INTO worlds.blocked_wallets (wallet) VALUES ($1)")
            .bind(&visitor)
            .execute(p.pool())
            .await
            .unwrap();
        assert_eq!(
            p.can_access(&world, &visitor, Some("open sesame"), None, None, now())
                .await
                .unwrap(),
            Access::Banned(Ban::Blocked)
        );
        // The owner is banned too when blocked: bans come first.
        sqlx::query("INSERT INTO worlds.blocked_wallets (wallet) VALUES ($1)")
            .bind(&owner)
            .execute(p.pool())
            .await
            .unwrap();
        assert_eq!(
            p.can_access(&world, &owner, None, None, None, now())
                .await
                .unwrap(),
            Access::Banned(Ban::Blocked)
        );
    }

    #[tokio::test]
    async fn allow_list_nft_and_unknown_worlds() {
        let Some(p) = scratch().await else { return };
        let (world, listed, other) = (world_name(), wallet(), wallet());
        seed_world(&p, &world, None, AccessType::AllowList, None).await;
        grant(&p, &world, Permission::Access, &listed, &[]).await;
        assert_eq!(
            p.can_access(&world, &listed, None, None, None, now())
                .await
                .unwrap(),
            Access::Allowed(AccessGrant::AllowListed)
        );
        assert_eq!(
            p.can_access(&world, &other, None, None, None, now())
                .await
                .unwrap(),
            Access::NotAllowed
        );
        let nft = world_name();
        seed_world(&p, &nft, None, AccessType::NftOwnership, None).await;
        assert_eq!(
            p.can_access(&nft, &listed, None, None, None, now())
                .await
                .unwrap(),
            Access::NotAllowed
        );
        let unknown = world_name();
        assert_eq!(
            p.can_access(&unknown, &other, None, None, None, now())
                .await
                .unwrap(),
            Access::Allowed(AccessGrant::Unrestricted)
        );
        assert_eq!(p.current_owner(&unknown, None).await.unwrap(), None);
    }

    async fn seed_community(p: &Policy, world: &WorldName, members: &[&Address]) -> Uuid {
        let id = Uuid::new_v4();
        sqlx::query(
            "INSERT INTO social.communities (id, name, owner_address) VALUES ($1, 'c', $2)",
        )
        .bind(id)
        .bind(wallet())
        .execute(p.pool())
        .await
        .unwrap();
        for m in members {
            sqlx::query("INSERT INTO social.community_members (community_id, member_address, role) VALUES ($1, $2, 'member')")
                .bind(id)
                .bind(m)
                .execute(p.pool())
                .await
                .unwrap();
        }
        sqlx::query("INSERT INTO worlds.world_access_communities (world_name, community_id) VALUES ($1, $2)")
            .bind(world.key())
            .bind(id)
            .execute(p.pool())
            .await
            .unwrap();
        id
    }

    #[tokio::test]
    async fn allow_list_admits_members_of_listed_communities() {
        let Some(p) = scratch().await else { return };
        let (world, member, both, stranger) = (world_name(), wallet(), wallet(), wallet());
        seed_world(&p, &world, None, AccessType::AllowList, None).await;
        grant(&p, &world, Permission::Access, &both, &[]).await;
        let community = seed_community(&p, &world, &[&member, &both]).await;
        let access = async |who: &Address| {
            p.can_access(&world, who, None, None, None, now())
                .await
                .unwrap()
        };
        assert_eq!(
            access(&member).await,
            Access::Allowed(AccessGrant::Community)
        );
        assert_eq!(
            access(&both).await,
            Access::Allowed(AccessGrant::AllowListed),
            "one grant: the wallet's"
        );
        assert_eq!(access(&stranger).await, Access::NotAllowed);
        assert_eq!(
            p.member_communities(&member, &[community, Uuid::new_v4()])
                .await
                .unwrap(),
            BTreeSet::from([community])
        );
        // A banned, suspended or deleted community admits nobody.
        sqlx::query("INSERT INTO social.community_bans (community_id, banned_address, banned_by) VALUES ($1, $2, 'mod')")
            .bind(community)
            .bind(&member)
            .execute(p.pool())
            .await
            .unwrap();
        assert_eq!(access(&member).await, Access::NotAllowed, "banned");
        let other = wallet();
        let suspended = seed_community(&p, &world, &[&other]).await;
        assert_eq!(
            access(&other).await,
            Access::Allowed(AccessGrant::Community)
        );
        sqlx::query("UPDATE social.communities SET suspended = TRUE, suspended_at = now(), suspended_by = 'mod' WHERE id = $1")
            .bind(suspended)
            .execute(p.pool())
            .await
            .unwrap();
        assert_eq!(access(&other).await, Access::NotAllowed, "suspended");
        let deleted = seed_community(&p, &world, &[&stranger]).await;
        sqlx::query("UPDATE social.communities SET active = FALSE WHERE id = $1")
            .bind(deleted)
            .execute(p.pool())
            .await
            .unwrap();
        assert_eq!(access(&stranger).await, Access::NotAllowed, "deleted");
        assert!(
            p.member_communities(&stranger, &[deleted])
                .await
                .unwrap()
                .is_empty()
        );
    }

    #[tokio::test]
    async fn personal_worlds_are_owned_by_their_address_under_the_cap() {
        let Some(pool) = crate::db::scratch::pool(4).await else {
            return;
        };
        let p = Policy::new(pool, policy(1), None);
        let (me, other) = (wallet(), wallet());
        let mine = WorldName::personal(&me);
        assert_eq!(
            p.can_deploy(&mine, &me, &parcels(&["0,0"])).await.unwrap(),
            Deploy::Owner
        );
        assert_eq!(
            p.can_deploy(&mine, &other, &parcels(&["0,0"]))
                .await
                .unwrap_err()
                .http_status(),
            403
        );
        assert!(p.is_owner(&mine, &me).await.is_allowed());
        assert_eq!(p.world_size_cap(&mine), 1024);
        // A personal world this realm only mirrors does not count against its cap.
        let mirrored = WorldName::personal(&wallet());
        seed_world(&p, &mirrored, None, AccessType::Unrestricted, None).await;
        sqlx::query(
            "INSERT INTO worlds.world_scenes (world_name, entity_id, deployment_auth_chain, entity, deployer, parcels, size, mirror_source) \
             VALUES ($1, 'bafyMirroredPersonal', '[]'::json, '{}'::jsonb, $2, ARRAY['0,0'], 1, 'https://upstream.example')",
        )
        .bind(mirrored.key())
        .bind(crate::ZERO_ADDRESS)
        .execute(p.pool())
        .await
        .unwrap();
        assert_eq!(
            p.can_deploy(&mine, &me, &parcels(&["0,0"])).await.unwrap(),
            Deploy::Owner,
            "a mirrored copy is not hosted here"
        );
        // Someone else's personal world fills the realm; mine, once deployed, stays admitted.
        seed_world(
            &p,
            &WorldName::personal(&other),
            Some(&other),
            AccessType::Unrestricted,
            None,
        )
        .await;
        seed_scene(&p, &WorldName::personal(&other), "bafyO", &["0,0"]).await;
        let err = p
            .can_deploy(&mine, &me, &parcels(&["0,0"]))
            .await
            .unwrap_err();
        assert!(
            err.to_string()
                .contains("maximum of 1 personal test worlds"),
            "{err}"
        );
        seed_world(&p, &mine, Some(&me), AccessType::Unrestricted, None).await;
        seed_scene(&p, &mine, "bafyM", &["0,0"]).await;
        assert_eq!(
            p.can_deploy(&mine, &me, &parcels(&["0,0"])).await.unwrap(),
            Deploy::Owner
        );
        // Disabled: a personal name is nobody's.
        let off = Policy::new(p.pool().clone(), PersonalWorldsPolicy::disabled(), None);
        let err = off
            .can_deploy(&WorldName::personal(&wallet()), &me, &parcels(&["0,0"]))
            .await
            .unwrap_err();
        assert!(err.to_string().contains("not enabled"), "{err}");
    }

    #[tokio::test]
    async fn scene_admins_and_protected_addresses() {
        let Some(p) = scratch().await else { return };
        let (world, owner, admin, deployer, streamer, nobody) = (
            world_name(),
            wallet(),
            wallet(),
            wallet(),
            wallet(),
            wallet(),
        );
        seed_world(&p, &world, Some(&owner), AccessType::Unrestricted, None).await;
        seed_scene(&p, &world, "bafyAdm", &["0,0"]).await;
        let scene = SceneId::parse("bafyAdm").unwrap();
        sqlx::query("INSERT INTO social.scene_admins (place_id, admin_address, added_by) VALUES ($1, $2, $3)")
            .bind(scene.as_str())
            .bind(&admin)
            .bind(&owner)
            .execute(p.pool())
            .await
            .unwrap();
        let sref = SceneRef::World {
            world: &world,
            scene: &scene,
        };
        assert!(p.is_admin(&sref, &admin).await.is_allowed());
        assert!(p.is_admin(&sref, &owner).await.is_allowed());
        assert_eq!(
            p.is_admin(&sref, &nobody)
                .await
                .into_result()
                .unwrap_err()
                .http_status(),
            403
        );
        let genesis = SceneId::parse("bafyGen").unwrap();
        let footprint = parcels(&["10,20"]);
        let gref = SceneRef::Genesis {
            scene: &genesis,
            parcels: &footprint,
        };
        assert_eq!(
            p.is_admin(&gref, &nobody)
                .await
                .into_result()
                .unwrap_err()
                .http_status(),
            403,
            "no LAND reader: non-admins refused"
        );
        grant(&p, &world, Permission::Deployment, &deployer, &["3,3"]).await;
        grant(&p, &world, Permission::Streaming, &streamer, &[]).await;
        let protected = p
            .protected_addresses(&world, &parcels(&["0,0"]))
            .await
            .unwrap();
        assert!(
            protected.contains(&owner)
                && protected.contains(&streamer)
                && !protected.contains(&deployer)
        );
        let protected = p
            .protected_addresses(&world, &parcels(&["3,3"]))
            .await
            .unwrap();
        assert!(protected.contains(&deployer));
    }
}

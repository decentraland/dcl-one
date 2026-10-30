//! World and player storage (the old `catalyrst-world-storage` service, the
//! upstream worlds-content-server `/values`, `/players`, `/env` and `/usage`
//! routes): three key/value namespaces a running scene reads and writes
//! through a signed fetch, scoped by the stable [`SceneNamespace`] the ONE
//! room resolver mints from the active deployment.
//!
//! Where the old service made two HTTP hops per request (the Places API for
//! a place id, the worlds `/permissions` document or lambdas' LAND
//! permissions for the signer), this module resolves the scene through
//! [`rooms::resolve`] over the shared `worlds.world_scenes` /
//! `content.active_pointers` rows and asks [`Policy`] locally:
//!
//! | who | how the signer is admitted | may |
//! |---|---|---|
//! | the authoritative server (`AUTHORITATIVE_SERVER_ADDRESS`) or an `AUTHORIZED_ADDRESSES` wallet | the signed fetch alone | every route |
//! | a guest worker holding a scoped delegation (`x-authoritative-scope`, signed by the authoritative server for exactly this world, scene id and parcel) | [`delegation::verify_storage_delegation`] | read one env value (`GET /env/{key}`) and the world/player values routes; never list, write or clear env |
//! | the world's owner, or a `deployment` grantee whose grant covers the active deployment's footprint | [`Policy::is_owner`], [`Policy::has_permission_covering`] | every route, including the clears |
//! | the LAND owner or a LAND manager (operator, updateOperator, updateManager, approvedForAll: the old lambdas matrix) of the parcel a Genesis request names; never a scene admin | [`Policy::may_manage_land_storage`] (the request's parcel) | the same |
//! | a wallet in the active deployment's `metadata.logsPermissions` (local scenes only) | [`has_logs_access`] (`worlds.logs_permissions`) | the per-key and list `/values*` and `/players*` routes; never the clears, `/env*` or `/usage*` |
//!
//! Every other signer is refused 401 after the scene resolved (a missing
//! scene is a 400 first, as the old Places miss was). The namespace is the
//! world scene's identity (`explore.world_scene_place`, the old Places id)
//! and `(main, effective base parcel)` for a Genesis scene
//! (`world_identity::SceneNamespace`): a world redeploy keeps every row when
//! its footprint overlaps exactly one previous scene (a moved base too), a
//! Genesis one while its base stays; an undeploy touches none (only the
//! explicit clear routes delete). The scenes of a world never share a
//! namespace, so a guest reaches storage only through this surface with the
//! capability the delegation names, never SQL and never another scene's
//! values or env values (the namespace is in every statement). Quotas still
//! span the whole world (`store::size_info_sql`).
//! `GET /collaborator` ([`collaborator`]) lists the scenes a signer reaches
//! through `logsPermissions`; it needs no scene context. `POST /exports`
//! ([`export`]) admits owners, deployers and collaborators only, and refuses
//! the authoritative server even when it holds a grant.

pub mod auth;
pub mod collaborator;
pub mod delegation;
pub mod encryption;
pub mod errors;
pub mod export;
pub mod handlers;
pub mod store;

#[cfg(test)]
mod tests;

use std::sync::Arc;
use std::time::Duration;

use axum::Router;
use axum::http::{HeaderMap, Uri};
use axum::routing::{get, post};
use sqlx::PgPool;

use crate::auth::eip1654::Eip1654Validator;
use crate::comms::world_identity::{GENESIS_CITY_REALM, Parcel, RealmId, SceneNamespace};
use crate::policy::{Permission, Policy};
use crate::rooms::{self, ActiveDeployment, ResolveError, RoomRequest};
use crate::shared::cors;
use crate::shared::identity::{Address, AuthorityNotEstablished, PermissionDecision};
use crate::shared::nul_guard;
use crate::shared::signed_fetch::{ORIGINAL_PATH_HEADER, header_str};

use auth::{AuthChainError, SceneAuthMetadata, verify_request};
use delegation::{AUTHORITATIVE_SCOPE_HEADER, StorageDelegationTarget, verify_storage_delegation};
use encryption::{Encryptor, parse_encryption_key};
use export::{ExportLimits, ExportSettings};

pub use errors::ApiError;
pub use store::{NamespaceLimits, StorageCacheConfig, Store};

pub const NOT_AUTHORIZED_MSG: &str =
    "Unauthorized: Signer is not authorized to perform operations on this world";
pub const AUTHORITATIVE_EXPORT_MSG: &str =
    "Unauthorized: The authoritative server cannot export storage";

/// The raw settings as the process environment (or `Config`) carries them
/// under the old names; [`Settings::resolve`] checks them.
#[derive(Debug, Clone)]
pub struct RawSettings {
    /// `ENCRYPTION_KEY`: 64 hex characters; unset leaves the env routes
    /// answering 503 and every other route serving.
    pub encryption_key: Option<String>,
    /// `AUTHORITATIVE_SERVER_ADDRESS`: the signer always authorized, and the
    /// only wallet whose storage delegations are trusted (shared with comms).
    pub authoritative_server_address: Option<String>,
    /// `AUTHORIZED_ADDRESSES`: comma-separated wallets always authorized.
    pub authorized_addresses: String,
    /// `STORAGE_CACHE_ENABLED` (unset disables the process-local cache),
    /// `STORAGE_CACHE_TTL_SECONDS` (60), `STORAGE_CACHE_MAX` (8000),
    /// `STORAGE_CACHE_MAX_VALUE_BYTES` (32768).
    pub storage_cache_enabled: Option<bool>,
    pub storage_cache_ttl_seconds: u64,
    pub storage_cache_max: u64,
    pub storage_cache_max_value_bytes: u64,
    /// `ENV_STORAGE_MAX_VALUE_SIZE_BYTES` (10240) / `ENV_STORAGE_MAX_TOTAL_SIZE_BYTES` (262144).
    pub env_max_value_size_bytes: i64,
    pub env_max_total_size_bytes: i64,
    /// `WORLD_STORAGE_MAX_VALUE_SIZE_BYTES` (524288) / `WORLD_STORAGE_MAX_TOTAL_SIZE_BYTES` (10485760).
    pub world_max_value_size_bytes: i64,
    pub world_max_total_size_bytes: i64,
    /// `PLAYER_STORAGE_MAX_VALUE_SIZE_BYTES` (102400) / `PLAYER_STORAGE_MAX_TOTAL_SIZE_BYTES` (1048576).
    pub player_max_value_size_bytes: i64,
    pub player_max_total_size_bytes: i64,
    /// `EXPORT_MAX_CONCURRENT_PER_SCENE` (1), `EXPORT_MAX_CONCURRENT_PER_POD`
    /// (5, per process), `EXPORT_SCENE_COOLDOWN_SECONDS` (10),
    /// `EXPORT_TIMEOUT_SECONDS` (120), `EXPORT_MAX_SIZE_BYTES` (524288000 raw).
    pub export_max_concurrent_per_scene: u64,
    pub export_max_concurrent_per_pod: u64,
    pub export_scene_cooldown_seconds: u64,
    pub export_timeout_seconds: u64,
    pub export_max_size_bytes: u64,
}

impl Default for RawSettings {
    fn default() -> Self {
        Self::defaults()
    }
}

impl RawSettings {
    /// The old binary's defaults for everything but the secrets.
    pub fn defaults() -> Self {
        Self {
            encryption_key: None,
            authoritative_server_address: None,
            authorized_addresses: String::new(),
            storage_cache_enabled: None,
            storage_cache_ttl_seconds: 60,
            storage_cache_max: 8_000,
            storage_cache_max_value_bytes: 32_768,
            env_max_value_size_bytes: 10_240,
            env_max_total_size_bytes: 262_144,
            world_max_value_size_bytes: 524_288,
            world_max_total_size_bytes: 10_485_760,
            player_max_value_size_bytes: 102_400,
            player_max_total_size_bytes: 1_048_576,
            export_max_concurrent_per_scene: 1,
            export_max_concurrent_per_pod: 5,
            export_scene_cooldown_seconds: 10,
            export_timeout_seconds: 120,
            export_max_size_bytes: 524_288_000,
        }
    }
}

#[derive(Clone)]
pub struct Settings {
    pub encryption_key: Option<[u8; 32]>,
    pub authoritative_server_address: Option<Address>,
    pub authorized_addresses: Vec<Address>,
    pub storage_cache: StorageCacheConfig,
    pub env_limits: NamespaceLimits,
    pub world_limits: NamespaceLimits,
    pub player_limits: NamespaceLimits,
    pub export: ExportSettings,
}

impl std::fmt::Debug for Settings {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Settings")
            .field("encryption_key", &self.encryption_key.map(|_| "<set>"))
            .field(
                "authoritative_server_address",
                &self.authoritative_server_address,
            )
            .field("authorized_addresses", &self.authorized_addresses)
            .field("storage_cache", &self.storage_cache)
            .field("env_limits", &self.env_limits)
            .field("world_limits", &self.world_limits)
            .field("player_limits", &self.player_limits)
            .field("export", &self.export)
            .finish()
    }
}

fn at_least_one(name: &str, value: u64) -> anyhow::Result<u64> {
    if value == 0 {
        anyhow::bail!("{name} must be at least 1");
    }
    Ok(value)
}

fn positive(name: &str, value: i64) -> anyhow::Result<i64> {
    if value <= 0 {
        anyhow::bail!("{name} must be positive, got {value}");
    }
    Ok(value)
}

impl Settings {
    /// Checks the settings the way the old binary did at startup: a malformed
    /// key, a non-wallet address or a non-positive limit refuses to start; an
    /// unset key is a warning (the env routes answer 503).
    pub fn resolve(raw: RawSettings) -> anyhow::Result<Self> {
        anyhow::ensure!(
            (1..=export::MAX_TIMEOUT_SECONDS).contains(&raw.export_timeout_seconds),
            "EXPORT_TIMEOUT_SECONDS must be between 1 and {}",
            export::MAX_TIMEOUT_SECONDS
        );
        let encryption_key = raw
            .encryption_key
            .as_deref()
            .map(parse_encryption_key)
            .transpose()?;
        if encryption_key.is_none() {
            tracing::warn!(
                "ENCRYPTION_KEY unset; the world-storage /env routes answer 503, /values and /players serve"
            );
        }
        let authoritative_server_address = match raw
            .authoritative_server_address
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
        {
            Some(s) => Some(
                Address::parse(s)
                    .map_err(|e| anyhow::anyhow!("AUTHORITATIVE_SERVER_ADDRESS: {e}"))?,
            ),
            None => None,
        };
        let mut authorized_addresses = Vec::new();
        for raw in raw
            .authorized_addresses
            .split(',')
            .map(str::trim)
            .filter(|s| !s.is_empty())
        {
            authorized_addresses.push(
                Address::parse(raw).map_err(|e| anyhow::anyhow!("AUTHORIZED_ADDRESSES: {e}"))?,
            );
        }
        if authoritative_server_address.is_none() {
            tracing::warn!(
                "AUTHORITATIVE_SERVER_ADDRESS unset; world-storage scoped delegations are refused (owners and deployers still write)"
            );
        }
        let cache_enabled = raw.storage_cache_enabled.unwrap_or(false);
        if cache_enabled {
            tracing::warn!(
                "STORAGE_CACHE_ENABLED=true requires a single storage-serving process; reads may be stale up to STORAGE_CACHE_TTL_SECONDS after another process's write"
            );
        }
        Ok(Self {
            encryption_key,
            authoritative_server_address,
            authorized_addresses,
            storage_cache: StorageCacheConfig {
                enabled: cache_enabled,
                ttl_seconds: raw.storage_cache_ttl_seconds,
                max_entries: raw.storage_cache_max,
                max_value_bytes: raw.storage_cache_max_value_bytes as usize,
            },
            env_limits: NamespaceLimits {
                max_value_size_bytes: positive(
                    "ENV_STORAGE_MAX_VALUE_SIZE_BYTES",
                    raw.env_max_value_size_bytes,
                )?,
                max_total_size_bytes: positive(
                    "ENV_STORAGE_MAX_TOTAL_SIZE_BYTES",
                    raw.env_max_total_size_bytes,
                )?,
            },
            world_limits: NamespaceLimits {
                max_value_size_bytes: positive(
                    "WORLD_STORAGE_MAX_VALUE_SIZE_BYTES",
                    raw.world_max_value_size_bytes,
                )?,
                max_total_size_bytes: positive(
                    "WORLD_STORAGE_MAX_TOTAL_SIZE_BYTES",
                    raw.world_max_total_size_bytes,
                )?,
            },
            player_limits: NamespaceLimits {
                max_value_size_bytes: positive(
                    "PLAYER_STORAGE_MAX_VALUE_SIZE_BYTES",
                    raw.player_max_value_size_bytes,
                )?,
                max_total_size_bytes: positive(
                    "PLAYER_STORAGE_MAX_TOTAL_SIZE_BYTES",
                    raw.player_max_total_size_bytes,
                )?,
            },
            export: ExportSettings {
                max_concurrent_per_scene: at_least_one(
                    "EXPORT_MAX_CONCURRENT_PER_SCENE",
                    raw.export_max_concurrent_per_scene,
                )? as usize,
                max_concurrent_per_pod: at_least_one(
                    "EXPORT_MAX_CONCURRENT_PER_POD",
                    raw.export_max_concurrent_per_pod,
                )? as usize,
                scene_cooldown: Duration::from_secs(raw.export_scene_cooldown_seconds),
                timeout: Duration::from_secs(at_least_one(
                    "EXPORT_TIMEOUT_SECONDS",
                    raw.export_timeout_seconds,
                )?),
                max_size_bytes: at_least_one("EXPORT_MAX_SIZE_BYTES", raw.export_max_size_bytes)?,
            },
        })
    }

    /// The defaults with a fixed key and the given authoritative signer.
    pub fn for_tests(authoritative_server_address: Option<Address>) -> Self {
        let mut s = Self::resolve(RawSettings::defaults()).expect("the defaults resolve");
        s.encryption_key = Some([7u8; 32]);
        s.authoritative_server_address = authoritative_server_address;
        s
    }
}

pub struct StorageState {
    pub pool: PgPool,
    /// The permission core (shared with the deploy, permission and comms routes).
    pub policy: Arc<Policy>,
    pub store: Store,
    pub settings: Settings,
    encryptor: Option<Encryptor>,
    /// Contract-wallet (EIP-1654) signature check for signed-fetch chains;
    /// `None` refuses such chains.
    pub eip1654: Option<Arc<dyn Eip1654Validator>>,
    /// The per-process export caps (`POST /exports`).
    pub exports: Arc<ExportLimits>,
}

pub type SharedState = Arc<StorageState>;

impl StorageState {
    pub fn new(
        pool: PgPool,
        policy: Arc<Policy>,
        settings: Settings,
        eip1654: Option<Arc<dyn Eip1654Validator>>,
    ) -> Self {
        Self {
            store: Store::new(pool.clone(), settings.storage_cache),
            encryptor: settings.encryption_key.as_ref().map(Encryptor::new),
            exports: ExportLimits::new(settings.export),
            pool,
            policy,
            settings,
            eip1654,
        }
    }

    /// The env cipher, or the 503 the env routes answer without a key.
    pub fn encryptor(&self) -> Result<&Encryptor, ApiError> {
        self.encryptor
            .as_ref()
            .ok_or(AuthorityNotEstablished::CredentialNotConfigured {
                environment_variable: "ENCRYPTION_KEY",
            })
            .map_err(ApiError::from)
    }
}

/// What a route acts on once the signed fetch verified and the scene
/// resolved: the signer, the realm and parcel the metadata named, the active
/// deployment standing there and its storage namespace.
pub struct SceneContext {
    pub signer: Address,
    pub realm: RealmId,
    pub parcel: Parcel,
    /// `sceneId` as the metadata carried it (the delegation binds to it).
    pub scene_id: Option<String>,
    pub deployment: ActiveDeployment,
    pub namespace: SceneNamespace,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AuthPolicy {
    pub allow_authorized_addresses: bool,
    pub allow_owners_and_deployers: bool,
    pub allow_scoped_delegation: bool,
    /// A wallet in the active deployment's `metadata.logsPermissions`.
    pub allow_logs_access: bool,
    /// `AUTHORITATIVE_SERVER_ADDRESS` is refused even when it holds a grant.
    pub refuse_authoritative_server: bool,
}

impl AuthPolicy {
    pub const DEFAULT: AuthPolicy = AuthPolicy {
        allow_authorized_addresses: true,
        allow_owners_and_deployers: true,
        allow_scoped_delegation: true,
        allow_logs_access: false,
        refuse_authoritative_server: false,
    };

    /// The per-key and list `/values*` and `/players*` routes (not the clears).
    pub const LOGS_ACCESS: AuthPolicy = AuthPolicy {
        allow_logs_access: true,
        ..AuthPolicy::DEFAULT
    };

    /// `POST /exports`: owners, deployers and collaborators, never the
    /// authoritative server, an authorized address or a delegation.
    pub const EXPORT: AuthPolicy = AuthPolicy {
        allow_authorized_addresses: false,
        allow_owners_and_deployers: true,
        allow_scoped_delegation: false,
        allow_logs_access: true,
        refuse_authoritative_server: true,
    };

    pub const OWNERS_DEPLOYERS_ONLY: AuthPolicy = AuthPolicy {
        allow_authorized_addresses: false,
        allow_owners_and_deployers: true,
        allow_scoped_delegation: false,
        allow_logs_access: false,
        refuse_authoritative_server: false,
    };

    pub const AUTHORIZED_ADDRESSES_OR_SCOPED_DELEGATION: AuthPolicy = AuthPolicy {
        allow_authorized_addresses: true,
        allow_owners_and_deployers: false,
        allow_scoped_delegation: true,
        allow_logs_access: false,
        refuse_authoritative_server: false,
    };
}

fn path_and_query(uri: &Uri) -> String {
    match uri.query() {
        Some(q) if !q.is_empty() => format!("{}?{}", uri.path(), q),
        _ => uri.path().to_string(),
    }
}

/// Signature paths are limited to the route itself and its public storage
/// mounts. Queries are optional, matching upstream pathname signatures.
/// Never accept arbitrary suffixes: a player route also ends in `/values`.
pub fn signed_paths(headers: &HeaderMap, uri: &Uri, original: Option<&Uri>) -> Vec<String> {
    let route_pq = path_and_query(uri);
    let route_path = uri.path();
    let mut out = Vec::with_capacity(6);
    let mut push = |candidate: &str| {
        let path = candidate.split('?').next().unwrap_or(candidate);
        let known_mount = path == route_path
            || ["/world-storage", "/storage"]
                .iter()
                .any(|mount| path.strip_prefix(mount) == Some(route_path));
        if known_mount && !out.iter().any(|c| c == candidate) {
            out.push(candidate.to_string());
        }
    };
    if let Some(raw) = header_str(headers, ORIGINAL_PATH_HEADER) {
        push(raw);
        push(raw.split('?').next().unwrap_or(raw));
    }
    if let Some(original) = original {
        push(&path_and_query(original));
        push(original.path());
    }
    push(&route_pq);
    push(route_path);
    out
}

/// Whether `signer` is the authoritative server address or a member of
/// `authorized_addresses`.
fn signer_directly_authorized(
    authoritative_server_address: Option<&Address>,
    authorized_addresses: &[Address],
    signer: &Address,
) -> bool {
    authoritative_server_address == Some(signer) || authorized_addresses.contains(signer)
}

enum LocalAuth {
    Granted,
    NeedsPermission,
    Denied,
}

/// The policy checks that need no lookup: the direct addresses and the
/// scoped delegation header. `NeedsPermission` defers to the permission core.
fn authorize_locally(
    state: &StorageState,
    signer: &Address,
    world: &str,
    scene_id: &str,
    parcel: &str,
    scope_header: Option<&str>,
    policy: AuthPolicy,
) -> LocalAuth {
    if policy.allow_authorized_addresses
        && signer_directly_authorized(
            state.settings.authoritative_server_address.as_ref(),
            &state.settings.authorized_addresses,
            signer,
        )
    {
        return LocalAuth::Granted;
    }

    if policy.allow_scoped_delegation
        && let Some(scope_header) = scope_header
    {
        let trusted_signers: Vec<String> = state
            .settings
            .authoritative_server_address
            .iter()
            .map(|a| a.as_str().to_string())
            .collect();
        match verify_storage_delegation(
            scope_header,
            &StorageDelegationTarget {
                signer: signer.as_str(),
                world,
                scene_id,
                parcel,
                trusted_signers: &trusted_signers,
            },
        ) {
            Ok(()) => return LocalAuth::Granted,
            Err(reason) => {
                tracing::warn!(world, reason, "Rejected world-scoped storage delegation")
            }
        }
    }

    if policy.allow_owners_and_deployers || policy.allow_logs_access {
        LocalAuth::NeedsPermission
    } else {
        LocalAuth::Denied
    }
}

/// The realm and parcel the metadata names: `realm.serverName`, else
/// `realmName`, lowercased; a `*.eth` realm is a world, anything else a
/// shared realm; no realm is Genesis City (`main`). The parcel defaults to
/// `0,0`; the raw spelling is kept for the delegation claim, which the host
/// minted from the same metadata.
fn derive_realm_and_parcel(
    meta: &SceneAuthMetadata,
) -> Result<(RealmId, Parcel, String), ApiError> {
    let realm = meta
        .realm
        .as_ref()
        .and_then(|r| r.server_name.clone())
        .or_else(|| meta.realm_name.clone())
        .map(|s| s.trim().to_lowercase())
        .filter(|s| !s.is_empty());
    let parcel_raw = meta.parcel.clone().filter(|s| !s.is_empty());

    let parcel = match &parcel_raw {
        Some(p) => Parcel::parse(p).map_err(|_| {
            ApiError::bad_request(
                "Parcel must be two comma-separated integer coordinates, e.g. \"10,-25\"",
            )
        })?,
        None => Parcel { x: 0, y: 0 },
    };
    if realm.is_none() && parcel_raw.is_none() {
        return Err(ApiError::bad_request(
            "Request must include a realm name or a parcel",
        ));
    }
    let realm = match realm {
        Some(r) => RealmId::parse(&r)
            .map_err(|e| ApiError::bad_request(format!("Invalid realm name: {e}")))?,
        None => RealmId::Shared(GENESIS_CITY_REALM.to_string()),
    };
    Ok((
        realm,
        parcel,
        parcel_raw.unwrap_or_else(|| "0,0".to_string()),
    ))
}

fn auth_chain_to_api(err: AuthChainError) -> ApiError {
    ApiError::SignedFetch {
        status: err.status_code(),
        retryable: matches!(err, AuthChainError::CatalystUnavailable(_)),
        error: err.raw_message(),
    }
}

fn resolve_to_api(err: ResolveError, realm: &RealmId, parcel: Parcel) -> ApiError {
    match err {
        ResolveError::NoSceneAtParcel { .. } | ResolveError::NoParcels { .. } => {
            ApiError::bad_request(format!(
                "Scene not found for world \"{}\" at parcel \"{parcel}\"",
                realm.storage_realm()
            ))
        }
        ResolveError::Db(e) => ApiError::Database(e),
        other => ApiError::internal(other.to_string()),
    }
}

/// Owner or deployer of a world scene (a `deployment` grant covering the
/// active deployment's footprint: the grant that could have published it);
/// LAND owner or LAND manager of `parcel`, the parcel a Genesis request
/// named (as the old world-storage asked the lambdas).
async fn has_scene_permission(
    state: &StorageState,
    realm: &RealmId,
    dep: &ActiveDeployment,
    parcel: Parcel,
    signer: &Address,
) -> Result<bool, ApiError> {
    match realm {
        RealmId::World(world) => match state.policy.is_owner(world, signer).await {
            PermissionDecision::Allowed => Ok(true),
            PermissionDecision::Refused(AuthorityNotEstablished::RefusedLacksAuthority {
                ..
            }) => state
                .policy
                .has_permission_covering(world, Permission::Deployment, signer, &dep.parcels)
                .await
                .map_err(|e| {
                    AuthorityNotEstablished::store_unavailable("worlds.world_permissions", &e)
                        .into()
                }),
            PermissionDecision::Refused(undetermined) => Err(undetermined.into()),
        },
        RealmId::Shared(_) => match state.policy.may_manage_land_storage(parcel, signer).await {
            PermissionDecision::Allowed => Ok(true),
            PermissionDecision::Refused(AuthorityNotEstablished::RefusedLacksAuthority {
                ..
            }) => Ok(false),
            PermissionDecision::Refused(undetermined) => Err(undetermined.into()),
        },
    }
}

/// Whether `signer` is in the `logsPermissions` of the resolved deployment
/// (`worlds.logs_permissions`, lowercased string entries). Read by scene id,
/// so a redeploy that drops the wallet revokes it at once; a mirrored world
/// scene grants nothing. Genesis grants also require complete, fresh ownership history
/// across the whole footprint. The shared SQL predicate subtracts the accepted
/// 15-minute future skew from the immutable entity timestamp: receipt time
/// cannot renew a synced old entity. A valid redeployment within 15 minutes of
/// transfer may be denied permanently; waiting does not change its timestamp,
/// so a later NEW deployment is required. This conservative trade-off avoids
/// treating an old future-dated entity as a post-transfer grant.
async fn has_logs_access(
    state: &StorageState,
    realm: &RealmId,
    dep: &ActiveDeployment,
    signer: &Address,
) -> Result<bool, ApiError> {
    if !dep.local {
        return Ok(false);
    }
    let pool = &state.pool;
    let land = state.policy.land_authz_settings();
    let scene = dep.scene_id.as_str();
    let granted = match realm {
        RealmId::World(world) => sqlx::query_scalar(
            "SELECT EXISTS (SELECT 1 FROM worlds.world_scenes WHERE world_name = $1 AND entity_id = $2 \
             AND $3 = ANY(worlds.logs_permissions((entity #> '{metadata,logsPermissions}')::text)))",
        )
        .bind(world.key())
        .bind(scene)
        .bind(signer)
        .fetch_one(pool)
        .await?,
        RealmId::Shared(_) => sqlx::query_scalar(
            "SELECT EXISTS (SELECT 1 FROM content.deployments WHERE entity_id = $1 AND deleter_deployment IS NULL \
             AND CASE WHEN $2 = ANY(worlds.logs_permissions((entity_metadata -> 'logsPermissions')::text)) \
                 THEN land_authz.scene_collaborators_current($3, $4, $5, entity_pointers, entity_timestamp) \
                 ELSE false END)",
        )
        .bind(scene)
        .bind(signer)
        .bind(land.map(|s| s.network.as_str()))
        .bind(land.map(|s| s.land_registry.as_str()))
        .bind(land.map(|s| s.estate_registry.as_str()))
        .fetch_one(pool)
        .await?,
    };
    Ok(granted)
}

/// Verifies the signed fetch, derives the realm and parcel, resolves the
/// active deployment standing there (a miss is the 400 the old Places miss
/// was, ahead of any authorization answer) and admits the signer under
/// `policy`.
pub async fn resolve_authorized(
    state: &StorageState,
    headers: &HeaderMap,
    method: &str,
    uri: &Uri,
    original: Option<&Uri>,
    policy: AuthPolicy,
) -> Result<SceneContext, ApiError> {
    resolve_and_authorize(state, headers, method, uri, original, policy, None).await
}

/// [`resolve_authorized`] for an upsert: the 411/413 body check runs between
/// the scene resolution and the authorization, where upstream's body-size
/// middleware sits.
pub async fn resolve_authorized_upsert(
    state: &StorageState,
    headers: &HeaderMap,
    uri: &Uri,
    original: Option<&Uri>,
    policy: AuthPolicy,
    max_value_size: i64,
) -> Result<SceneContext, ApiError> {
    resolve_and_authorize(
        state,
        headers,
        "put",
        uri,
        original,
        policy,
        Some(max_value_size),
    )
    .await
}

async fn resolve_and_authorize(
    state: &StorageState,
    headers: &HeaderMap,
    method: &str,
    uri: &Uri,
    original: Option<&Uri>,
    policy: AuthPolicy,
    max_value_size: Option<i64>,
) -> Result<SceneContext, ApiError> {
    let paths = signed_paths(headers, uri, original);
    let verified = verify_request(headers, method, &paths, state.eip1654.as_deref())
        .await
        .map_err(auth_chain_to_api)?;
    let (realm, parcel, parcel_raw) = derive_realm_and_parcel(&verified.metadata)?;
    let scope_header = headers
        .get(AUTHORITATIVE_SCOPE_HEADER)
        .and_then(|v| v.to_str().ok());
    let scene_id = verified.metadata.scene_id.clone();
    let signer = verified.signer;

    let resolved = rooms::resolve(
        &state.pool,
        RoomRequest::Parcel {
            realm: &realm,
            parcel,
        },
    )
    .await
    .map_err(|e| resolve_to_api(e, &realm, parcel))?;
    let deployment = resolved
        .deployment
        .ok_or_else(|| ApiError::internal("a parcel request resolved without a deployment"))?;
    if let Some(max) = max_value_size {
        handlers::check_content_length(headers, max)?;
    }

    if policy.refuse_authoritative_server
        && state.settings.authoritative_server_address.as_ref() == Some(&signer)
    {
        return Err(ApiError::not_authorized(AUTHORITATIVE_EXPORT_MSG));
    }

    // The delegation's `SceneId:` line is checked against the scene the
    // request resolved to, not the client's own `x-identity-metadata.sceneId`
    // (which the ephemeral signer writes itself): the host minted the claim
    // for the active deployment it loaded.
    let local = authorize_locally(
        state,
        &signer,
        realm.storage_realm(),
        deployment.scene_id.as_str(),
        &parcel_raw,
        scope_header,
        policy,
    );

    let granted = match local {
        LocalAuth::Granted => true,
        LocalAuth::Denied => false,
        LocalAuth::NeedsPermission => {
            // Each grant is sufficient on its own. An unavailable authority must
            // not suppress a valid alternative grant (in either direction).
            let logs = if policy.allow_logs_access {
                has_logs_access(state, &realm, &deployment, &signer).await
            } else {
                Ok(false)
            };
            if matches!(logs, Ok(true)) {
                true
            } else if policy.allow_owners_and_deployers
                && has_scene_permission(state, &realm, &deployment, parcel, &signer).await?
            {
                true
            } else {
                logs?
            }
        }
    };
    if !granted {
        return Err(ApiError::not_authorized(NOT_AUTHORIZED_MSG));
    }

    // Every world scene write records one (`scene_identity`); refuse rather than guess.
    let namespace = deployment.namespace.clone().ok_or_else(|| {
        ApiError::internal(format!(
            "scene {} of \"{}\" has no storage identity",
            deployment.scene_id,
            realm.storage_realm()
        ))
    })?;
    Ok(SceneContext {
        signer,
        realm,
        parcel,
        scene_id,
        deployment,
        namespace,
    })
}

/// The old route table (`api_router` plus `/ping`), to nest under
/// `/world-storage` (the old gateway location, which stripped the prefix and
/// set `x-original-path`) with the content routes' CORS and NUL guards.
pub fn router(state: SharedState) -> Router {
    let largest_value = [
        state.settings.world_limits,
        state.settings.player_limits,
        state.settings.env_limits,
    ]
    .iter()
    .map(|l| l.max_value_size_bytes)
    .max()
    .unwrap_or(0);
    let body_limit =
        usize::try_from(largest_value + handlers::BODY_ENVELOPE_SLACK_BYTES).unwrap_or(usize::MAX);
    Router::new()
        .route("/ping", get(handlers::ping))
        .route("/health", get(handlers::health))
        .route("/collaborator", get(collaborator::get_collaborator))
        .route("/exports", post(export::create_export))
        .route("/usage/world", get(handlers::usage_world))
        .route(
            "/usage/players/{player_address}",
            get(handlers::usage_player),
        )
        .route("/usage/env", get(handlers::usage_env))
        .route(
            "/values",
            get(handlers::world_list).delete(handlers::world_clear),
        )
        .route(
            "/values/{key}",
            get(handlers::world_get)
                .put(handlers::world_upsert)
                .delete(handlers::world_delete),
        )
        .route(
            "/players",
            get(handlers::players_list).delete(handlers::players_clear_all),
        )
        .route(
            "/players/{player_address}/values",
            get(handlers::player_list).delete(handlers::player_clear),
        )
        .route(
            "/players/{player_address}/values/{key}",
            get(handlers::player_get)
                .put(handlers::player_upsert)
                .delete(handlers::player_delete),
        )
        .route(
            "/env",
            get(handlers::env_list_keys).delete(handlers::env_clear),
        )
        .route(
            "/env/{key}",
            get(handlers::env_get)
                .put(handlers::env_upsert)
                .delete(handlers::env_delete),
        )
        .layer(axum::extract::DefaultBodyLimit::max(body_limit))
        .layer(axum::middleware::from_fn(nul_guard::nul_guard_middleware))
        .layer(axum::middleware::from_fn(cors::cors_middleware))
        .with_state(state)
}

#[cfg(test)]
mod scene_context_tests {
    use super::*;
    use axum::http::HeaderValue;

    fn meta(realm_name: Option<&str>, parcel: Option<&str>) -> SceneAuthMetadata {
        SceneAuthMetadata {
            realm_name: realm_name.map(str::to_string),
            parcel: parcel.map(str::to_string),
            ..Default::default()
        }
    }

    #[test]
    fn realm_is_lowercased_so_casing_cannot_split_storage() {
        let (realm, parcel, raw) =
            derive_realm_and_parcel(&meta(Some("MyWorld.DCL.eth"), None)).unwrap();
        assert_eq!(realm.storage_realm(), "myworld.dcl.eth");
        assert!(realm.is_world());
        assert_eq!((parcel.canonical().as_str(), raw.as_str()), ("0,0", "0,0"));
        let m = SceneAuthMetadata {
            realm: Some(auth::RealmField {
                server_name: Some("Other.dcl.eth".into()),
            }),
            realm_name: Some("ignored.dcl.eth".into()),
            ..Default::default()
        };
        assert_eq!(
            derive_realm_and_parcel(&m).unwrap().0.storage_realm(),
            "other.dcl.eth",
            "realm.serverName wins"
        );
    }

    #[test]
    fn shared_realms_fold_into_genesis_and_keep_the_raw_parcel() {
        let (realm, parcel, raw) =
            derive_realm_and_parcel(&meta(Some("artemis"), Some(" 010,-25 "))).unwrap();
        assert!(!realm.is_world());
        assert_eq!(realm.storage_realm(), GENESIS_CITY_REALM);
        assert_eq!(parcel.canonical(), "10,-25");
        assert_eq!(raw, " 010,-25 ");
        let (realm, ..) = derive_realm_and_parcel(&meta(None, Some("1,1"))).unwrap();
        assert_eq!(realm.storage_realm(), GENESIS_CITY_REALM);
    }

    #[test]
    fn parcel_must_be_two_integers() {
        for good in ["0,0", "10,-25", "-150,150", "1234567,-1234567"] {
            assert!(
                derive_realm_and_parcel(&meta(None, Some(good))).is_ok(),
                "{good}"
            );
        }
        for bad in [
            "10",
            "10,",
            ",25",
            "10,25,3",
            "a,b",
            "10.5,2",
            "--1,2",
            "-,2",
            "12345678901,2",
            "1,2/../x",
        ] {
            assert!(
                matches!(
                    derive_realm_and_parcel(&meta(None, Some(bad))),
                    Err(ApiError::BadRequest(_))
                ),
                "{bad}"
            );
        }
    }

    #[test]
    fn missing_realm_and_parcel_is_rejected() {
        assert!(matches!(
            derive_realm_and_parcel(&meta(None, None)),
            Err(ApiError::BadRequest(_))
        ));
        assert!(matches!(
            derive_realm_and_parcel(&meta(Some("  "), Some(""))),
            Err(ApiError::BadRequest(_))
        ));
    }

    #[test]
    fn scoped_delegation_policy_matrix() {
        let cases = [
            (AuthPolicy::DEFAULT, true, true, false),
            (AuthPolicy::LOGS_ACCESS, true, true, true),
            (AuthPolicy::OWNERS_DEPLOYERS_ONLY, false, true, false),
            (AuthPolicy::EXPORT, false, true, true),
            (
                AuthPolicy::AUTHORIZED_ADDRESSES_OR_SCOPED_DELEGATION,
                true,
                false,
                false,
            ),
        ];
        for (policy, delegation, owners, logs) in cases {
            assert_eq!(policy.allow_scoped_delegation, delegation);
            assert_eq!(policy.allow_owners_and_deployers, owners);
            assert_eq!(policy.allow_logs_access, logs);
        }
    }

    #[test]
    fn signer_directly_authorized_matches_the_old_list_scan() {
        let a = |s: &str| Address::parse(&format!("0x{}", s.repeat(40))).unwrap();
        let (auth, first, last, nope) = (a("a"), a("1"), a("2"), a("f"));
        let list = vec![first.clone(), last.clone()];
        assert!(signer_directly_authorized(Some(&auth), &list, &auth));
        assert!(signer_directly_authorized(Some(&auth), &list, &first));
        assert!(signer_directly_authorized(Some(&auth), &list, &last));
        assert!(!signer_directly_authorized(Some(&auth), &list, &nope));
        assert!(!signer_directly_authorized(None, &[], &auth));
        assert!(signer_directly_authorized(None, &list, &first));
    }

    #[test]
    fn maps_each_signed_fetch_failure_to_its_real_status() {
        let status_of = |err: AuthChainError| auth_chain_to_api(err).status();
        assert_eq!(
            status_of(AuthChainError::MalformedChain {
                detail: "boom".into()
            }),
            400
        );
        assert_eq!(status_of(AuthChainError::InsufficientLinks), 400);
        assert_eq!(status_of(AuthChainError::InvalidTimestamp("x".into())), 400);
        assert_eq!(status_of(AuthChainError::SceneSignerRejected), 400);
        assert_eq!(
            status_of(AuthChainError::Expired {
                signed_at: 0,
                now: 1,
                window_secs: 60
            }),
            401
        );
        assert_eq!(
            status_of(AuthChainError::InvalidSignature("no".into())),
            401
        );
        assert_eq!(status_of(AuthChainError::EipNotImplemented), 503);
        assert_eq!(
            status_of(AuthChainError::CatalystUnavailable("rpc".into())),
            503
        );
        match auth_chain_to_api(AuthChainError::SceneSignerRejected) {
            ApiError::SignedFetch { status, error, .. } => {
                assert_eq!((status, error.as_str()), (400, "Invalid metadata"))
            }
            other => panic!("expected SignedFetch, got {other:?}"),
        }
    }

    #[test]
    fn signed_paths_try_the_proxy_path_the_nested_original_and_the_route() {
        let route: Uri = "/values/k?x=1".parse().unwrap();
        let original: Uri = "/world-storage/values/k?x=1".parse().unwrap();
        let mut headers = HeaderMap::new();
        assert_eq!(
            signed_paths(&headers, &route, None),
            vec!["/values/k?x=1", "/values/k"]
        );
        assert_eq!(
            signed_paths(&headers, &route, Some(&original)),
            vec![
                "/world-storage/values/k?x=1",
                "/world-storage/values/k",
                "/values/k?x=1",
                "/values/k"
            ]
        );
        headers.insert(
            ORIGINAL_PATH_HEADER,
            HeaderValue::from_static("/storage/values/k?x=1"),
        );
        assert_eq!(
            signed_paths(&headers, &route, Some(&original)),
            vec![
                "/storage/values/k?x=1",
                "/storage/values/k",
                "/world-storage/values/k?x=1",
                "/world-storage/values/k",
                "/values/k?x=1",
                "/values/k"
            ]
        );
        // A forwarded path that does not end with the route is never tried.
        headers.insert(ORIGINAL_PATH_HEADER, HeaderValue::from_static("/elsewhere"));
        assert_eq!(
            signed_paths(&headers, &route, None),
            vec!["/values/k?x=1", "/values/k"]
        );
        // Nor is another route's path once its query is stripped.
        let env: Uri = "/env".parse().unwrap();
        headers.insert(
            ORIGINAL_PATH_HEADER,
            HeaderValue::from_static("/values?limit=1"),
        );
        assert_eq!(signed_paths(&headers, &env, None), vec!["/env"]);
        let values: Uri = "/values".parse().unwrap();
        headers.insert(
            ORIGINAL_PATH_HEADER,
            HeaderValue::from_static("/storage/values?limit=1"),
        );
        assert_eq!(
            signed_paths(&headers, &values, None),
            vec!["/storage/values?limit=1", "/storage/values", "/values"]
        );
        for forged in [
            "/players/0xabc/values",
            "/unknown/values",
            "/world-storagex/values",
        ] {
            headers.insert(ORIGINAL_PATH_HEADER, HeaderValue::from_static(forged));
            assert_eq!(signed_paths(&headers, &values, None), vec!["/values"]);
        }
        // Without a query the proxy header's path form is the same string, deduplicated.
        headers.insert(
            ORIGINAL_PATH_HEADER,
            HeaderValue::from_static("/world-storage/env"),
        );
        assert_eq!(
            signed_paths(&headers, &env, None),
            vec!["/world-storage/env", "/env"]
        );
    }

    #[test]
    fn export_timeout_is_bounded() {
        for seconds in [0, export::MAX_TIMEOUT_SECONDS + 1, u64::MAX] {
            let mut raw = RawSettings::defaults();
            raw.export_timeout_seconds = seconds;
            assert!(
                Settings::resolve(raw)
                    .unwrap_err()
                    .to_string()
                    .contains("EXPORT_TIMEOUT_SECONDS")
            );
        }
        for seconds in [1, export::MAX_TIMEOUT_SECONDS] {
            let mut raw = RawSettings::defaults();
            raw.export_timeout_seconds = seconds;
            assert_eq!(
                Settings::resolve(raw).unwrap().export.timeout,
                Duration::from_secs(seconds)
            );
        }
    }

    #[test]
    fn signed_fetch_retries_only_transient_rpc_failures() {
        use axum::response::IntoResponse;
        for (error, retryable) in [
            (AuthChainError::CatalystUnavailable("rpc".into()), true),
            (AuthChainError::EipNotImplemented, false),
        ] {
            let response = auth_chain_to_api(error).into_response();
            assert_eq!(
                response.status(),
                axum::http::StatusCode::SERVICE_UNAVAILABLE
            );
            assert_eq!(response.headers().contains_key("retry-after"), retryable);
        }
    }

    #[test]
    fn settings_resolve_the_old_defaults_and_refuse_garbage() {
        let s = Settings::resolve(RawSettings::defaults()).unwrap();
        assert!(
            s.encryption_key.is_none()
                && s.authoritative_server_address.is_none()
                && s.authorized_addresses.is_empty()
        );
        assert_eq!(
            s.storage_cache,
            StorageCacheConfig {
                enabled: false,
                ttl_seconds: 60,
                max_entries: 8_000,
                max_value_bytes: 32_768
            }
        );
        assert_eq!(
            s.env_limits,
            NamespaceLimits {
                max_value_size_bytes: 10_240,
                max_total_size_bytes: 262_144
            }
        );
        assert_eq!(
            s.world_limits,
            NamespaceLimits {
                max_value_size_bytes: 524_288,
                max_total_size_bytes: 10_485_760
            }
        );
        assert_eq!(
            s.player_limits,
            NamespaceLimits {
                max_value_size_bytes: 102_400,
                max_total_size_bytes: 1_048_576
            }
        );

        let mut raw = RawSettings::defaults();
        raw.encryption_key = Some("ab".repeat(32));
        raw.authoritative_server_address =
            Some("0xAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA".into());
        raw.authorized_addresses = " 0xBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB ,,0xcccccccccccccccccccccccccccccccccccccccc".into();
        let s = Settings::resolve(raw.clone()).unwrap();
        assert_eq!(s.encryption_key, Some([0xabu8; 32]));
        assert_eq!(
            s.authoritative_server_address.as_ref().unwrap().as_str(),
            "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
        );
        assert_eq!(s.authorized_addresses.len(), 2);
        assert!(!format!("{s:?}").contains("abab"), "the key never prints");

        raw.encryption_key = Some("short".into());
        assert!(Settings::resolve(raw.clone()).is_err());
        raw.encryption_key = None;
        raw.authorized_addresses = "nope".into();
        assert!(Settings::resolve(raw.clone()).is_err());
        raw.authorized_addresses.clear();
        raw.world_max_total_size_bytes = 0;
        assert!(Settings::resolve(raw).is_err());
        assert_eq!(
            Settings::resolve(RawSettings::defaults()).unwrap().export,
            ExportSettings::default()
        );
        let mut raw = RawSettings::defaults();
        raw.export_max_concurrent_per_pod = 0;
        assert!(Settings::resolve(raw).is_err());
    }

    #[test]
    fn the_storage_cache_defaults_off_on_every_node_unless_set() {
        for (explicit, expected) in [(None, false), (Some(true), true), (Some(false), false)] {
            let raw = RawSettings {
                storage_cache_enabled: explicit,
                ..RawSettings::defaults()
            };
            let s = Settings::resolve(raw).unwrap();
            assert_eq!(s.storage_cache.enabled, expected, "{explicit:?}");
        }
    }
}

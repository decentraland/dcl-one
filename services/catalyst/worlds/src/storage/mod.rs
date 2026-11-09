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
//! | the authoritative server (`AUTHORITATIVE_SERVER_ADDRESS`) or an `AUTHORIZED_ADDRESSES` wallet | the signed fetch alone | the per-key and list `/values*` and `/players*` routes, `GET /env/{key}`, `GET /usage/world` and `GET /usage/players/{address}`; never the clears, `GET /env`, `PUT`/`DELETE /env/{key}`, `GET /usage/env` or `POST /exports` |
//! | a guest worker holding a scoped delegation (`x-authoritative-scope`, signed by the authoritative server for exactly this world, the metadata's `sceneId` and parcel) | [`delegation::verify_storage_delegation`] | the same routes |
//! | the world's owner, or a `deployment` grantee whose grant covers the active deployment's footprint | [`Policy::is_owner`], [`Policy::has_permission_covering`] | every route but `GET /env/{key}`, the clears and `POST /exports` included |
//! | the LAND owner or a LAND manager (operator, updateOperator, updateManager, approvedForAll: the old lambdas matrix) of the parcel a Genesis request names; never a scene admin | [`Policy::may_manage_land_storage`] (the request's parcel) | the same |
//! | a wallet in the active deployment's `metadata.logsPermissions` (local scenes only) | [`has_logs_access`] (`worlds.logs_permissions_decoded`) | the per-key and list `/values*` and `/players*` routes and `POST /exports`; never the clears, `/env*` or `/usage*` |
//!
//! The checks run in upstream's order: the direct addresses and the
//! delegation, then the owner and deployer lookup (one that cannot answer is
//! 401 `Failed to verify world permissions`, before `logsPermissions` is
//! read), then `logsPermissions` (a lookup that cannot answer denies).
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
//! values or env values (the namespace is in every statement). A world's
//! quota, and the `/usage` numbers, span its active scenes and legacy scopes
//! but exclude scenes retired by an undeploy or by a redeploy into a new
//! place (inactive `explore.world_scene_place` identities); a shared realm's
//! is per scene (`store::size_info_sql`). Retired rows are kept as a safety
//! copy: no undeploy, redeploy, world deletion, sweep or mirror deletes them,
//! and the clear routes reach only the resolved active scene.
//! `GET /collaborator` ([`collaborator`]) lists the scenes a signer reaches
//! through `logsPermissions`; it needs no scene context and no deployment
//! consumer feeds it: it is read from the active deployments'
//! `metadata.logsPermissions` (`worlds.logs_permissions_decoded`) on every request,
//! where upstream keeps a table fed by an SQS deployment queue. `POST /exports`
//! ([`export`]) admits owners, deployers and collaborators only, and refuses
//! the authoritative server even when it holds a grant.

pub mod auth;
pub mod collaborator;
pub mod delegation;
pub mod encryption;
pub mod errors;
pub mod export;
pub mod handlers;
pub mod js;
pub mod js_json;
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
use crate::comms::world_identity::{
    GENESIS_CITY_REALM, Parcel, RealmId, SceneNamespace, WorldName,
};
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
use js::JsValue;

pub use errors::ApiError;
pub use store::{NamespaceLimits, StorageCacheConfig, Store};

pub const NOT_AUTHORIZED_MSG: &str =
    "Unauthorized: Signer is not authorized to perform operations on this world";
pub const FAILED_TO_VERIFY_MSG: &str = "Unauthorized: Failed to verify world permissions";
pub const AUTHORITATIVE_EXPORT_MSG: &str =
    "Unauthorized: The authoritative server cannot export storage";

/// The raw settings as the process environment (or `Config`) carries them
/// under the old names; [`Settings::resolve`] checks them.
#[derive(Debug, Clone)]
pub struct RawSettings {
    /// `ENCRYPTION_KEY`: exactly 64 hex characters, required (unset refuses
    /// to start, as upstream's `requireString`).
    pub encryption_key: Option<String>,
    /// `AUTHORITATIVE_SERVER_ADDRESS`: the signer always authorized, and the
    /// only wallet whose storage delegations are trusted (shared with comms).
    pub authoritative_server_address: Option<String>,
    /// `AUTHORIZED_ADDRESSES`: comma-separated wallets always authorized.
    pub authorized_addresses: String,
    /// `STORAGE_CACHE_ENABLED` (anything but exactly `false`, unset included,
    /// enables the process-local cache, as upstream's),
    /// `STORAGE_CACHE_TTL_SECONDS` (60), `STORAGE_CACHE_MAX` (8000, LRU),
    /// `STORAGE_CACHE_MAX_VALUE_BYTES` (32768).
    pub storage_cache_enabled: Option<String>,
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
    /// `EXPORT_TIMEOUT_SECONDS` (120, from admission), `EXPORT_MAX_SIZE_BYTES`
    /// (524288000 raw), `PG_COMPONENT_STREAM_QUERY_TIMEOUT` (120000 ms, each
    /// export statement). Any value is accepted, as upstream's.
    pub export_max_concurrent_per_scene: u64,
    pub export_max_concurrent_per_pod: u64,
    pub export_scene_cooldown_seconds: u64,
    pub export_timeout_seconds: u64,
    pub export_max_size_bytes: u64,
    pub export_stream_query_timeout_ms: u64,
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
            export_stream_query_timeout_ms: 120_000,
        }
    }
}

#[derive(Clone)]
pub struct Settings {
    pub encryption_key: [u8; 32],
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
            .field("encryption_key", &"<set>")
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

fn positive(name: &str, value: i64) -> anyhow::Result<i64> {
    if value <= 0 {
        anyhow::bail!("{name} must be positive, got {value}");
    }
    Ok(value)
}

impl Settings {
    /// Checks the settings the way the old binary did at startup: a missing
    /// or malformed key, a non-wallet address or a non-positive limit refuses
    /// to start. The export settings take any value, as upstream's.
    pub fn resolve(raw: RawSettings) -> anyhow::Result<Self> {
        let encryption_key =
            parse_encryption_key(raw.encryption_key.as_deref().ok_or_else(|| {
                anyhow::anyhow!("ENCRYPTION_KEY (or ENCRYPTION_KEY_FILE) is required")
            })?)?;
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
        Ok(Self {
            encryption_key,
            authoritative_server_address,
            authorized_addresses,
            storage_cache: StorageCacheConfig {
                enabled: raw.storage_cache_enabled.as_deref() != Some("false"),
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
                max_concurrent_per_scene: usize::try_from(raw.export_max_concurrent_per_scene)
                    .unwrap_or(usize::MAX),
                max_concurrent_per_pod: usize::try_from(raw.export_max_concurrent_per_pod)
                    .unwrap_or(usize::MAX),
                scene_cooldown: Duration::from_secs(raw.export_scene_cooldown_seconds),
                timeout: Duration::from_secs(raw.export_timeout_seconds),
                max_size_bytes: raw.export_max_size_bytes,
                stream_query_timeout: Duration::from_millis(raw.export_stream_query_timeout_ms),
            },
        })
    }

    /// The defaults with a fixed key and the given authoritative signer.
    pub fn for_tests(authoritative_server_address: Option<Address>) -> Self {
        let mut s = Self::resolve(RawSettings {
            encryption_key: Some("07".repeat(32)),
            ..RawSettings::defaults()
        })
        .expect("the defaults resolve");
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
    encryptor: Encryptor,
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
            encryptor: Encryptor::new(&settings.encryption_key),
            exports: ExportLimits::new(settings.export),
            pool,
            policy,
            settings,
            eip1654,
        }
    }

    /// The env cipher (`ENCRYPTION_KEY` is required at startup).
    pub fn encryptor(&self) -> Result<&Encryptor, ApiError> {
        Ok(&self.encryptor)
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

/// Signature paths are the route's pathname and its public storage mounts
/// (`/world-storage`, `/storage`, which the gateway strips and forwards in
/// `x-original-path`): upstream binds the pathname only, so a signature over
/// the path with its query never verifies. Never accept arbitrary suffixes:
/// a player route also ends in `/values`.
pub fn signed_paths(headers: &HeaderMap, uri: &Uri, original: Option<&Uri>) -> Vec<String> {
    let route_path = uri.path();
    let mut out = Vec::with_capacity(3);
    let mut push = |candidate: &str| {
        let path = candidate.split('?').next().unwrap_or(candidate);
        let known_mount = path == route_path
            || ["/world-storage", "/storage"]
                .iter()
                .any(|mount| path.strip_prefix(mount) == Some(route_path));
        if known_mount && !out.iter().any(|c| c == path) {
            out.push(path.to_string());
        }
    };
    if let Some(raw) = header_str(headers, ORIGINAL_PATH_HEADER) {
        push(raw);
    }
    if let Some(original) = original {
        push(original.path());
    }
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

/// Upstream's step 1, which needs no lookup: the direct addresses, then the
/// scoped delegation header, both only where authorized addresses are
/// allowed. The claim's `SceneId:` is compared with the signed metadata's
/// `sceneId` (missing or not a string reads as `""`, which no claim names)
/// and its `Parcel:` with the metadata's parcel string.
fn authorized_locally(
    state: &StorageState,
    signer: &Address,
    target: &SceneTarget,
    metadata_scene_id: &str,
    scope_header: Option<&str>,
    policy: AuthPolicy,
) -> bool {
    if !policy.allow_authorized_addresses {
        return false;
    }
    if signer_directly_authorized(
        state.settings.authoritative_server_address.as_ref(),
        &state.settings.authorized_addresses,
        signer,
    ) {
        return true;
    }
    let Some(scope_header) =
        scope_header.filter(|h| policy.allow_scoped_delegation && !h.is_empty())
    else {
        return false;
    };
    let world = target.realm.storage_realm();
    if !target.parcel_is_string {
        // `claim.parcel !== parcel` for a parcel that is not a string.
        tracing::warn!(
            world,
            reason = "claim parcel does not match target parcel",
            "Rejected world-scoped storage delegation"
        );
        return false;
    }
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
            scene_id: metadata_scene_id,
            parcel: &target.parcel_raw,
            trusted_signers: &trusted_signers,
        },
    ) {
        Ok(()) => true,
        Err(reason) => {
            tracing::warn!(world, reason, "Rejected world-scoped storage delegation");
            false
        }
    }
}

/// The scene a request names, read as upstream's scene-context middleware
/// reads the signed metadata.
struct SceneTarget {
    realm: RealmId,
    parcel: Parcel,
    /// `String(parcel)`, or `0,0` when the metadata names none: the scene-miss
    /// text and the delegation's `Parcel:` compare against it.
    parcel_raw: String,
    /// Upstream compares and splits the parcel as a string; an array that
    /// stringifies to a valid parcel resolves the scene but matches no claim,
    /// no LAND lookup and no collaborator scene.
    parcel_is_string: bool,
}

/// Upstream's parcel pattern, `^-?\d{1,10},-?\d{1,10}$`.
fn is_parcel_pattern(raw: &str) -> bool {
    let coordinate = |c: &str| {
        let digits = c.strip_prefix('-').unwrap_or(c);
        (1..=10).contains(&digits.len()) && digits.bytes().all(|b| b.is_ascii_digit())
    };
    raw.split_once(',')
        .is_some_and(|(x, y)| coordinate(x) && coordinate(y))
}

/// The 400 upstream's Places adapter answers when no scene stands there.
fn scene_miss(world: &str, parcel: &str) -> ApiError {
    ApiError::bad_request(format!(
        "Scene not found in Places API for world \"{world}\" at parcel \"{parcel}\""
    ))
}

/// `(realm.serverName || realmName)?.toLowerCase()`: a `*.eth` realm is a
/// world, anything else (padded, `artemis`, absent) Genesis City (`main`);
/// a non-string realm is upstream's `toLowerCase` TypeError, a 500. The
/// parcel must match upstream's pattern exactly (no whitespace) and defaults
/// to `0,0`. A world name or parcel that names no scene here (a malformed
/// world name, a coordinate past 32 bits) is the scene miss Places answered.
fn derive_scene_target(meta: &SceneAuthMetadata) -> Result<SceneTarget, ApiError> {
    let realm = match meta.realm() {
        None => None,
        Some(JsValue::String(s)) => Some(s.to_lowercase()),
        Some(_) => {
            return Err(ApiError::internal(
                "scene context: the realm name is not a string",
            ));
        }
    };
    let parcel_value = meta.parcel();
    let parcel_raw = parcel_value.map(JsValue::to_js_string);
    if parcel_raw.as_deref().is_some_and(|p| !is_parcel_pattern(p)) {
        return Err(ApiError::bad_request(
            "Parcel must be two comma-separated integer coordinates, e.g. \"10,-25\"",
        ));
    }
    if realm.is_none() && parcel_raw.is_none() {
        return Err(ApiError::bad_request(
            "Request must include a realm name or a parcel",
        ));
    }
    let parcel_is_string = parcel_value.is_none_or(|v| v.as_str().is_some());
    let parcel_raw = parcel_raw.unwrap_or_else(|| "0,0".to_string());
    let realm = match realm {
        Some(name) if name.ends_with(".eth") => match WorldName::parse(&name) {
            Ok(world) if world.key() == name => RealmId::World(world),
            _ => return Err(scene_miss(&name, &parcel_raw)),
        },
        _ => RealmId::Shared(GENESIS_CITY_REALM.to_string()),
    };
    let parcel =
        Parcel::parse(&parcel_raw).map_err(|_| scene_miss(realm.storage_realm(), &parcel_raw))?;
    Ok(SceneTarget {
        realm,
        parcel,
        parcel_raw,
        parcel_is_string,
    })
}

/// crypto-middleware's `onError` as upstream wires it: a 5xx refusal's
/// `error` is replaced with `Internal error` (it carries catalyst hosts and
/// upstream detail); the detail goes to the log.
fn auth_chain_to_api(err: AuthChainError) -> ApiError {
    let status = err.status_code();
    let error = if status >= 500 {
        tracing::warn!(status, error = %err.raw_message(), "world storage: signed fetch could not be verified");
        "Internal error".to_string()
    } else {
        err.raw_message()
    };
    ApiError::SignedFetch {
        status,
        retryable: matches!(err, AuthChainError::CatalystUnavailable(_)),
        error,
    }
}

fn resolve_to_api(err: ResolveError, target: &SceneTarget) -> ApiError {
    match err {
        ResolveError::NoSceneAtParcel { .. } | ResolveError::NoParcels { .. } => {
            scene_miss(target.realm.storage_realm(), &target.parcel_raw)
        }
        ResolveError::Db(e) => ApiError::Database(e),
        other => ApiError::internal(other.to_string()),
    }
}

/// Owner or deployer of a world scene (a `deployment` grant covering the
/// active deployment's footprint: the grant that could have published it);
/// LAND owner or LAND manager of the parcel a Genesis request named (as the
/// old world-storage asked the lambdas). A lookup that cannot answer is an
/// error, which the caller answers 401 as upstream does.
async fn has_scene_permission(
    state: &StorageState,
    target: &SceneTarget,
    dep: &ActiveDeployment,
    signer: &Address,
) -> Result<bool, ApiError> {
    match &target.realm {
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
        // Upstream splits the parcel string for the lambdas; a non-string throws.
        RealmId::Shared(_) if !target.parcel_is_string => {
            Err(ApiError::not_authorized(FAILED_TO_VERIFY_MSG))
        }
        RealmId::Shared(_) => match state
            .policy
            .may_manage_land_storage(target.parcel, signer)
            .await
        {
            PermissionDecision::Allowed => Ok(true),
            PermissionDecision::Refused(AuthorityNotEstablished::RefusedLacksAuthority {
                ..
            }) => Ok(false),
            PermissionDecision::Refused(undetermined) => Err(undetermined.into()),
        },
    }
}

/// Whether `signer` is in the `logsPermissions` of the resolved deployment
/// (`worlds.logs_permissions_decoded`: the lowercased string entries, JSON
/// escapes decoded). Read by scene id, so a redeploy that drops the wallet
/// revokes it at once; a mirrored world scene grants nothing. Genesis grants
/// also require complete, fresh ownership history across the whole
/// footprint. The shared SQL predicate subtracts the accepted 15-minute
/// future skew from the immutable entity timestamp: receipt time cannot
/// renew a synced old entity. A valid redeployment within 15 minutes of
/// transfer may be denied permanently; waiting does not change its
/// timestamp, so a later NEW deployment is required. This conservative
/// trade-off avoids treating an old future-dated entity as a post-transfer
/// grant.
async fn has_logs_access(
    state: &StorageState,
    target: &SceneTarget,
    dep: &ActiveDeployment,
    signer: &Address,
) -> Result<bool, ApiError> {
    if !dep.local || !target.parcel_is_string {
        return Ok(false);
    }
    let pool = &state.pool;
    let land = state.policy.land_authz_settings();
    let scene = dep.scene_id.as_str();
    let granted = match &target.realm {
        RealmId::World(world) => sqlx::query_scalar(
            "SELECT EXISTS (SELECT 1 FROM worlds.world_scenes WHERE world_name = $1 AND entity_id = $2 \
             AND $3 = ANY(worlds.logs_permissions_decoded((entity #> '{metadata,logsPermissions}')::text)))",
        )
        .bind(world.key())
        .bind(scene)
        .bind(signer)
        .fetch_one(pool)
        .await?,
        RealmId::Shared(_) => sqlx::query_scalar(
            "SELECT EXISTS (SELECT 1 FROM content.deployments WHERE entity_id = $1 AND deleter_deployment IS NULL \
             AND CASE WHEN $2 = ANY(worlds.logs_permissions_decoded((entity_metadata -> 'logsPermissions')::text)) \
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
        Some(&move |h: &HeaderMap| handlers::check_content_length(h, max_value_size)),
    )
    .await
}

/// [`resolve_authorized`] with a route's own body check in the same place.
pub async fn resolve_authorized_checking_body(
    state: &StorageState,
    headers: &HeaderMap,
    method: &str,
    uri: &Uri,
    original: Option<&Uri>,
    policy: AuthPolicy,
    body_check: &BodyCheck,
) -> Result<SceneContext, ApiError> {
    resolve_and_authorize(
        state,
        headers,
        method,
        uri,
        original,
        policy,
        Some(body_check),
    )
    .await
}

/// A declared-length check run after the scene resolved, before authorization.
pub type BodyCheck = dyn Fn(&HeaderMap) -> Result<(), ApiError> + Sync;

/// Upstream's authorization middleware, in its order: authorized addresses
/// and the scoped delegation; then owners and deployers, where a lookup
/// that cannot answer is 401 `Failed to verify world permissions` before
/// anything else is consulted; then `logsPermissions`, where a lookup that
/// cannot answer denies.
async fn authorize(
    state: &StorageState,
    signer: &Address,
    target: &SceneTarget,
    deployment: &ActiveDeployment,
    metadata_scene_id: &str,
    scope_header: Option<&str>,
    policy: AuthPolicy,
) -> Result<(), ApiError> {
    if authorized_locally(
        state,
        signer,
        target,
        metadata_scene_id,
        scope_header,
        policy,
    ) {
        return Ok(());
    }
    if policy.allow_owners_and_deployers {
        match has_scene_permission(state, target, deployment, signer).await {
            Ok(true) => return Ok(()),
            Ok(false) => {}
            Err(e) => {
                tracing::warn!(world = target.realm.storage_realm(), error = %e, "world storage: unable to verify world permissions");
                return Err(ApiError::not_authorized(FAILED_TO_VERIFY_MSG));
            }
        }
    }
    if policy.allow_logs_access {
        match has_logs_access(state, target, deployment, signer).await {
            Ok(true) => return Ok(()),
            Ok(false) => {}
            Err(e) => {
                tracing::warn!(world = target.realm.storage_realm(), error = %e, "world storage: logs-access permission check failed; denying");
            }
        }
    }
    Err(ApiError::not_authorized(NOT_AUTHORIZED_MSG))
}

async fn resolve_and_authorize(
    state: &StorageState,
    headers: &HeaderMap,
    method: &str,
    uri: &Uri,
    original: Option<&Uri>,
    policy: AuthPolicy,
    body_check: Option<&BodyCheck>,
) -> Result<SceneContext, ApiError> {
    let paths = signed_paths(headers, uri, original);
    let verified = verify_request(headers, method, &paths, state.eip1654.as_deref())
        .await
        .map_err(auth_chain_to_api)?;
    let target = derive_scene_target(&verified.metadata)?;
    let scope_header = headers
        .get(AUTHORITATIVE_SCOPE_HEADER)
        .and_then(|v| v.to_str().ok());
    let scene_id = verified.metadata.scene_id().map(str::to_string);
    let signer = verified.signer;

    let resolved = rooms::resolve(
        &state.pool,
        RoomRequest::Parcel {
            realm: &target.realm,
            parcel: target.parcel,
        },
    )
    .await
    .map_err(|e| resolve_to_api(e, &target))?;
    let deployment = resolved
        .deployment
        .ok_or_else(|| ApiError::internal("a parcel request resolved without a deployment"))?;
    if let Some(check) = body_check {
        check(headers)?;
    }

    if policy.refuse_authoritative_server
        && state.settings.authoritative_server_address.as_ref() == Some(&signer)
    {
        return Err(ApiError::not_authorized(AUTHORITATIVE_EXPORT_MSG));
    }

    authorize(
        state,
        &signer,
        &target,
        &deployment,
        scene_id.as_deref().unwrap_or(""),
        scope_header,
        policy,
    )
    .await?;

    // Every world scene write records one (`scene_identity`); refuse rather than guess.
    let namespace = deployment.namespace.clone().ok_or_else(|| {
        ApiError::internal(format!(
            "scene {} of \"{}\" has no storage identity",
            deployment.scene_id,
            target.realm.storage_realm()
        ))
    })?;
    Ok(SceneContext {
        signer,
        realm: target.realm,
        parcel: target.parcel,
        scene_id,
        deployment,
        namespace,
    })
}

/// The old route table (`api_router` plus `/ping`), to nest under
/// `/world-storage` (the old gateway location, which stripped the prefix and
/// set `x-original-path`) with the content routes' CORS guard and the query
/// half of their NUL guard.
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
        // A NUL in a key or player address is the handler's 400, after the
        // credential, as upstream answers it; `validate_key` and
        // `parse_player` keep it from every statement.
        .layer(axum::middleware::from_fn(
            nul_guard::nul_query_guard_middleware,
        ))
        .layer(axum::middleware::from_fn(cors::cors_middleware))
        .with_state(state)
}

#[cfg(test)]
mod scene_context_tests {
    use super::*;
    use axum::http::HeaderValue;

    fn meta(raw: &str) -> SceneAuthMetadata {
        SceneAuthMetadata::parse(raw).unwrap()
    }

    fn target(raw: &str) -> Result<SceneTarget, ApiError> {
        derive_scene_target(&meta(raw))
    }

    fn message(err: ApiError) -> (u16, String) {
        (err.status(), err.to_string())
    }

    #[test]
    fn realm_is_lowercased_so_casing_cannot_split_storage() {
        let t = target(r#"{"realmName":"MyWorld.DCL.eth"}"#).unwrap();
        assert_eq!(t.realm.storage_realm(), "myworld.dcl.eth");
        assert!(t.realm.is_world());
        assert_eq!(
            (t.parcel.canonical().as_str(), t.parcel_raw.as_str()),
            ("0,0", "0,0")
        );
        let t = target(r#"{"realm":{"serverName":"Other.dcl.eth"},"realmName":"ignored.dcl.eth"}"#)
            .unwrap();
        assert_eq!(
            t.realm.storage_realm(),
            "other.dcl.eth",
            "realm.serverName wins"
        );
    }

    /// `serverName || realmName`: an empty, zero, false or null serverName
    /// falls back to realmName.
    #[test]
    fn a_falsy_server_name_falls_back_to_the_realm_name() {
        for server_name in [r#""""#, "0", "false", "null"] {
            let t = target(&format!(
                r#"{{"realm":{{"serverName":{server_name}}},"realmName":"x.dcl.eth"}}"#
            ))
            .unwrap();
            assert_eq!(t.realm.storage_realm(), "x.dcl.eth", "{server_name}");
        }
        // A realm that is not an object has no serverName.
        let t = target(r#"{"realm":"y.dcl.eth","realmName":"x.dcl.eth"}"#).unwrap();
        assert_eq!(t.realm.storage_realm(), "x.dcl.eth");
    }

    /// Upstream does not trim the realm: a padded world name is not `*.eth`
    /// and reads as Genesis City; a blank one still names a realm.
    #[test]
    fn the_realm_is_not_trimmed() {
        let t = target(r#"{"realmName":"x.dcl.eth ","parcel":"1,2"}"#).unwrap();
        assert!(!t.realm.is_world());
        assert_eq!(t.realm.storage_realm(), GENESIS_CITY_REALM);
        let t = target(r#"{"realmName":"  "}"#).unwrap();
        assert_eq!(
            (t.realm.storage_realm(), t.parcel_raw.as_str()),
            (GENESIS_CITY_REALM, "0,0")
        );
    }

    /// A `*.eth` realm no world could be named is the scene miss upstream's
    /// Places lookup answered, not a realm-name error.
    #[test]
    fn a_malformed_world_name_is_the_scene_miss() {
        for (raw, world) in [
            (r#"{"realmName":" x.dcl.eth"}"#, " x.dcl.eth"),
            (
                r#"{"realmName":"a/b.dcl.eth","parcel":"3,4"}"#,
                "a/b.dcl.eth",
            ),
            (r#"{"realmName":"a b.dcl.eth"}"#, "a b.dcl.eth"),
        ] {
            let parcel = if raw.contains("3,4") { "3,4" } else { "0,0" };
            assert_eq!(
                message(target(raw).err().unwrap()),
                (
                    400,
                    format!(
                        "Scene not found in Places API for world \"{world}\" at parcel \"{parcel}\""
                    )
                ),
                "{raw}"
            );
        }
    }

    #[test]
    fn a_non_string_realm_is_upstreams_type_error() {
        for realm in ["5", "true", "{}", "[]"] {
            let err = target(&format!(r#"{{"realmName":{realm},"parcel":"1,1"}}"#))
                .err()
                .unwrap();
            assert_eq!(err.status(), 500, "{realm}");
        }
    }

    #[test]
    fn shared_realms_fold_into_genesis_and_keep_the_raw_parcel() {
        let t = target(r#"{"realmName":"artemis","parcel":"010,-25"}"#).unwrap();
        assert!(!t.realm.is_world());
        assert_eq!(t.realm.storage_realm(), GENESIS_CITY_REALM);
        assert_eq!(t.parcel.canonical(), "10,-25");
        assert_eq!(t.parcel_raw, "010,-25");
        let t = target(r#"{"parcel":"1,1"}"#).unwrap();
        assert_eq!(t.realm.storage_realm(), GENESIS_CITY_REALM);
    }

    #[test]
    fn parcel_must_match_the_upstream_pattern() {
        for good in ["0,0", "10,-25", "-150,150", "1234567,-1234567", "-0,00"] {
            assert!(
                target(&format!(r#"{{"parcel":"{good}"}}"#)).is_ok(),
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
            " 1,2",
            "1, 2",
            "1,2 ",
            "+1,2",
        ] {
            assert_eq!(
                message(target(&format!(r#"{{"parcel":"{bad}"}}"#)).err().unwrap()),
                (
                    400,
                    "Parcel must be two comma-separated integer coordinates, e.g. \"10,-25\""
                        .to_string()
                ),
                "{bad}"
            );
        }
        // Ten digits pass the pattern but name no scene here.
        assert_eq!(
            message(target(r#"{"parcel":"9999999999,1"}"#).err().unwrap()),
            (
                400,
                "Scene not found in Places API for world \"main\" at parcel \"9999999999,1\""
                    .to_string()
            )
        );
    }

    /// The parcel is tested as `String(parcel)`: a number or object fails
    /// the pattern, an array may pass it but is never a string afterwards.
    #[test]
    fn a_non_string_parcel_is_read_as_javascript_does() {
        for parcel in ["12", "true", "{}"] {
            assert_eq!(
                target(&format!(r#"{{"parcel":{parcel}}}"#))
                    .err()
                    .unwrap()
                    .status(),
                400,
                "{parcel}"
            );
        }
        let t = target(r#"{"parcel":["1","2"]}"#).unwrap();
        assert_eq!((t.parcel_raw.as_str(), t.parcel_is_string), ("1,2", false));
        // A falsy parcel is absent.
        let t = target(r#"{"realmName":"x.dcl.eth","parcel":0}"#).unwrap();
        assert_eq!((t.parcel_raw.as_str(), t.parcel_is_string), ("0,0", true));
    }

    /// One field of the wrong type does not blank the others.
    #[test]
    fn a_mistyped_field_leaves_the_others() {
        let t =
            target(r#"{"realmName":"x.dcl.eth","parcel":"1,1","sceneId":7,"signer":3}"#).unwrap();
        assert_eq!(
            (t.realm.storage_realm(), t.parcel_raw.as_str()),
            ("x.dcl.eth", "1,1")
        );
        let t =
            target(r#"{"realmName":"x.dcl.eth","realmName":"y.dcl.eth","parcel":"2,2"}"#).unwrap();
        assert_eq!(
            t.realm.storage_realm(),
            "y.dcl.eth",
            "the last duplicate wins"
        );
    }

    #[test]
    fn missing_realm_and_parcel_is_rejected() {
        for raw in [
            "{}",
            r#"{"realmName":"","parcel":""}"#,
            r#"{"realmName":null}"#,
        ] {
            assert_eq!(
                message(target(raw).err().unwrap()),
                (
                    400,
                    "Request must include a realm name or a parcel".to_string()
                ),
                "{raw}"
            );
        }
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
        assert_eq!(status_of(AuthChainError::InvalidMetadata("x".into())), 400);
        assert_eq!(
            status_of(AuthChainError::SceneSignerRejected("x".into())),
            400
        );
        assert_eq!(status_of(AuthChainError::Expired("x".into())), 401);
        assert_eq!(
            status_of(AuthChainError::InvalidSignature("no".into())),
            401
        );
        assert_eq!(status_of(AuthChainError::EipNotImplemented), 503);
        assert_eq!(
            status_of(AuthChainError::CatalystUnavailable("rpc".into())),
            503
        );
        match auth_chain_to_api(AuthChainError::SceneSignerRejected(
            "Invalid metadata content: {}".into(),
        )) {
            ApiError::SignedFetch { status, error, .. } => {
                assert_eq!(
                    (status, error.as_str()),
                    (400, "Invalid metadata content: {}")
                )
            }
            other => panic!("expected SignedFetch, got {other:?}"),
        }
    }

    /// Upstream's `onError` replaces a 5xx refusal's `error` with
    /// `Internal error`: the catalyst host and RPC detail never reach the client.
    #[test]
    fn a_5xx_signed_fetch_refusal_carries_no_detail() {
        for err in [
            AuthChainError::CatalystUnavailable("https://rpc.example/secret timed out".into()),
            AuthChainError::EipNotImplemented,
        ] {
            match auth_chain_to_api(err) {
                ApiError::SignedFetch { status, error, .. } => {
                    assert_eq!((status, error.as_str()), (503, "Internal error"))
                }
                other => panic!("expected SignedFetch, got {other:?}"),
            }
        }
    }

    #[test]
    fn signed_paths_are_pathnames_on_the_route_or_its_mounts() {
        let route: Uri = "/values/k?x=1".parse().unwrap();
        let original: Uri = "/world-storage/values/k?x=1".parse().unwrap();
        let mut headers = HeaderMap::new();
        assert_eq!(signed_paths(&headers, &route, None), vec!["/values/k"]);
        assert_eq!(
            signed_paths(&headers, &route, Some(&original)),
            vec!["/world-storage/values/k", "/values/k"]
        );
        headers.insert(
            ORIGINAL_PATH_HEADER,
            HeaderValue::from_static("/storage/values/k?x=1"),
        );
        assert_eq!(
            signed_paths(&headers, &route, Some(&original)),
            vec!["/storage/values/k", "/world-storage/values/k", "/values/k"]
        );
        // A forwarded path that does not end with the route is never tried.
        headers.insert(ORIGINAL_PATH_HEADER, HeaderValue::from_static("/elsewhere"));
        assert_eq!(signed_paths(&headers, &route, None), vec!["/values/k"]);
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
            vec!["/storage/values", "/values"]
        );
        for forged in [
            "/players/0xabc/values",
            "/unknown/values",
            "/world-storagex/values",
        ] {
            headers.insert(ORIGINAL_PATH_HEADER, HeaderValue::from_static(forged));
            assert_eq!(signed_paths(&headers, &values, None), vec!["/values"]);
        }
        headers.insert(
            ORIGINAL_PATH_HEADER,
            HeaderValue::from_static("/world-storage/env"),
        );
        assert_eq!(
            signed_paths(&headers, &env, None),
            vec!["/world-storage/env", "/env"]
        );
    }

    fn keyed() -> RawSettings {
        RawSettings {
            encryption_key: Some("ab".repeat(32)),
            ..RawSettings::defaults()
        }
    }

    #[test]
    fn export_settings_take_any_value_as_upstream() {
        for value in [0, 1, 3601, u64::MAX] {
            let s = Settings::resolve(RawSettings {
                export_max_concurrent_per_scene: value,
                export_max_concurrent_per_pod: value,
                export_scene_cooldown_seconds: value,
                export_timeout_seconds: value,
                export_max_size_bytes: value,
                export_stream_query_timeout_ms: value,
                ..keyed()
            })
            .unwrap();
            assert_eq!(s.export.max_concurrent_per_pod as u64, value);
            assert_eq!(s.export.timeout, Duration::from_secs(value));
            assert_eq!(s.export.max_size_bytes, value);
            assert_eq!(s.export.stream_query_timeout, Duration::from_millis(value));
        }
    }

    #[test]
    fn encryption_key_is_required() {
        for missing in [None, Some(String::new())] {
            let raw = RawSettings {
                encryption_key: missing.clone(),
                ..RawSettings::defaults()
            };
            assert!(Settings::resolve(raw).is_err(), "{missing:?}");
        }
        let err = Settings::resolve(RawSettings::defaults())
            .unwrap_err()
            .to_string();
        assert!(err.contains("ENCRYPTION_KEY"), "{err}");
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
        let s = Settings::resolve(keyed()).unwrap();
        assert!(s.authoritative_server_address.is_none() && s.authorized_addresses.is_empty());
        assert_eq!(
            s.storage_cache,
            StorageCacheConfig {
                enabled: true,
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
        assert_eq!(s.export, ExportSettings::default());

        let mut raw = keyed();
        raw.authoritative_server_address =
            Some("0xAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA".into());
        raw.authorized_addresses = " 0xBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB ,,0xcccccccccccccccccccccccccccccccccccccccc".into();
        let s = Settings::resolve(raw.clone()).unwrap();
        assert_eq!(s.encryption_key, [0xabu8; 32]);
        assert_eq!(
            s.authoritative_server_address.as_ref().unwrap().as_str(),
            "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
        );
        assert_eq!(s.authorized_addresses.len(), 2);
        assert!(!format!("{s:?}").contains("abab"), "the key never prints");

        raw.encryption_key = Some("short".into());
        assert!(Settings::resolve(raw.clone()).is_err());
        raw.encryption_key = Some(format!(" {}", "ab".repeat(32)));
        assert!(Settings::resolve(raw.clone()).is_err(), "never trimmed");
        raw.encryption_key = Some("ab".repeat(32));
        raw.authorized_addresses = "nope".into();
        assert!(Settings::resolve(raw.clone()).is_err());
        raw.authorized_addresses.clear();
        raw.world_max_total_size_bytes = 0;
        assert!(Settings::resolve(raw).is_err());
    }

    #[test]
    fn the_storage_cache_is_on_unless_exactly_false() {
        for (explicit, expected) in [
            (None, true),
            (Some("true"), true),
            (Some(""), true),
            (Some("0"), true),
            (Some("FALSE"), true),
            (Some("false"), false),
        ] {
            let raw = RawSettings {
                storage_cache_enabled: explicit.map(str::to_string),
                ..keyed()
            };
            let s = Settings::resolve(raw).unwrap();
            assert_eq!(s.storage_cache.enabled, expected, "{explicit:?}");
        }
    }
}

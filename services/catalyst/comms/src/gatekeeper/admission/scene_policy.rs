//! Scene policy responsibilities.
use super::*;

/// The world of a realm name as the permission core keys it.
fn world_of(realm_or_name: &str) -> Option<WorldName> {
    WorldName::parse(realm_or_name).ok()
}

fn parcel_set(positions: &[String]) -> ParcelSet {
    positions
        .iter()
        .filter_map(|p| Parcel::parse(p).ok())
        .collect()
}

/// Whether `identity` may enter `world_name` (the scene `scene_id` of it,
/// when known): `Policy::can_access`, the local call that replaced the
/// worlds service's `/permissions` document. The owner, an unrestricted
/// world, an allow-listed wallet or a matching shared secret (the
/// `secret` field of the signed metadata, the scene host's contract) are
/// admitted; a ban, an allow-list miss, an nft-ownership world and a wrong
/// or missing secret are not. A store that cannot answer is a 503, never a
/// denial.
pub async fn has_world_access_permission(
    state: &CommsState,
    identity: &Address,
    world_name: &str,
    scene_id: Option<&str>,
    secret: Option<&str>,
    device_id: Option<&str>,
    caller: &str,
) -> Result<bool, ApiError> {
    world_access(
        state, identity, world_name, scene_id, secret, device_id, caller,
    )
    .await
    .map(|access| access == WorldAccess::Allowed)
}

/// How a world answered a join: admitted, refused by a ban of the scene
/// (its own key or a successor's, which upstream answers 403 apart from the
/// world's 401), or refused otherwise.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorldAccess {
    Allowed,
    SceneBanned,
    Refused,
}

pub async fn world_access(
    state: &CommsState,
    identity: &Address,
    world_name: &str,
    scene_id: Option<&str>,
    secret: Option<&str>,
    device_id: Option<&str>,
    caller: &str,
) -> Result<WorldAccess, ApiError> {
    let Some(world) = world_of(world_name) else {
        return Ok(WorldAccess::Refused);
    };
    let scene = scene_id.and_then(|id| SceneId::parse(id).ok());
    let now = chrono::Utc::now().naive_utc();
    let admission = state
        .policy
        .world_admission(&world, now)
        .await
        .map_err(|_| service_unavailable(WORLD_ACCESS_UNAVAILABLE_MSG))?;
    if !matches!(admission, WorldAdmission::Open(_)) {
        return Ok(WorldAccess::Refused);
    }
    let throttle = super::super::world_mint::SecretThrottle {
        pool: &state.pool,
        world: world.key(),
        subject: caller,
        max_attempts: state.world_mint.max_secret_attempts,
    };
    // A no-secret decision also catches an access-type change since world_admission.
    let mut access = state
        .policy
        .can_access(&world, identity, None, scene.as_ref(), device_id, now)
        .await;
    if matches!(access, Ok(Access::InvalidSecret)) || (access.is_err() && secret.is_some()) {
        let reservation = throttle
            .reserve(now)
            .await
            .map_err(|_| service_unavailable(WORLD_ACCESS_UNAVAILABLE_MSG))?
            .ok_or_else(|| ApiError::http(429, "Too many shared-secret attempts"))?;
        access = state
            .policy
            .can_access(&world, identity, secret, scene.as_ref(), device_id, now)
            .await;
        throttle
            .settle(matches!(access, Ok(Access::InvalidSecret)), reservation)
            .await;
    }
    match access {
        Ok(Access::Allowed(_)) => Ok(WorldAccess::Allowed),
        Ok(Access::Banned(crate::access::Ban::Scene)) => Ok(WorldAccess::SceneBanned),
        Ok(Access::Banned(_) | Access::NotAllowed | Access::InvalidSecret) => {
            Ok(WorldAccess::Refused)
        }
        Err(AuthorityNotEstablished::RefusedLacksAuthority { .. }) => Ok(WorldAccess::Refused),
        Err(why) => {
            tracing::error!(error = %why, world = %world, "world access could not be decided");
            Err(service_unavailable(WORLD_ACCESS_UNAVAILABLE_MSG))
        }
    }
}

#[derive(Debug, Deserialize)]
struct LeaseAuthorization {
    #[serde(default)]
    addresses: Vec<String>,
    #[serde(default)]
    plots: Vec<String>,
    #[serde(default, rename = "startDate")]
    start_date: Option<String>,
    #[serde(default, rename = "endDate")]
    end_date: Option<String>,
    #[serde(default, rename = "onlyDev")]
    only_dev: Option<bool>,
}

impl LeaseAuthorization {
    /// Whether the entry grants at `now`, as linker-server reads it: not before `startDate`, not
    /// after `endDate`, and `onlyDev` entries only on a development node. A date that does not
    /// parse grants nothing.
    fn in_force(&self, now: chrono::DateTime<chrono::Utc>, dev_entries: bool) -> bool {
        if self.only_dev == Some(true) && !dev_entries {
            return false;
        }
        let bound = |raw: &Option<String>| match raw.as_deref().map(str::trim) {
            None | Some("") => Ok(None),
            Some(raw) => lease_date(raw).map(Some).ok_or(()),
        };
        match (bound(&self.start_date), bound(&self.end_date)) {
            (Ok(start), Ok(end)) => {
                start.is_none_or(|start| start <= now) && end.is_none_or(|end| end >= now)
            }
            _ => false,
        }
    }
}

/// The document's entries that read as lease entries; a malformed one is skipped (it grants
/// nothing) instead of failing the whole document.
fn lease_entries(entries: Vec<serde_json::Value>) -> Vec<LeaseAuthorization> {
    let total = entries.len();
    let parsed: Vec<LeaseAuthorization> = entries
        .into_iter()
        .filter_map(|entry| serde_json::from_value(entry).ok())
        .collect();
    if parsed.len() < total {
        tracing::warn!(
            skipped = total - parsed.len(),
            "lease authorizations entries skipped as malformed"
        );
    }
    parsed
}

/// A lease date as JavaScript's `Date` reads the published ones: `YYYY-MM-DD` is UTC midnight,
/// otherwise an RFC 3339 timestamp.
fn lease_date(raw: &str) -> Option<chrono::DateTime<chrono::Utc>> {
    if let Ok(day) = chrono::NaiveDate::parse_from_str(raw, "%Y-%m-%d") {
        return Some(day.and_hms_opt(0, 0, 0)?.and_utc());
    }
    chrono::DateTime::parse_from_rfc3339(raw)
        .ok()
        .map(|at| at.with_timezone(&chrono::Utc))
}

/// The upstream caches: the lease authorizations and the ENS names.
pub struct SceneCaches {
    lease_holders: TtlMap<String, Arc<Vec<LeaseAuthorization>>>,
    names: TtlMap<String, String>,
}

impl Default for SceneCaches {
    fn default() -> Self {
        Self {
            lease_holders: TtlMap::new(LEASE_AUTHORIZATIONS_TTL),
            names: TtlMap::bounded(NAMES_TTL, 50_000),
        }
    }
}

pub(super) fn parse_xy(s: &str) -> Option<(i32, i32)> {
    let mut it = s.splitn(2, ',');
    Some((
        it.next()?.trim().parse().ok()?,
        it.next()?.trim().parse().ok()?,
    ))
}

/// The LAND manager document of a Genesis parcel through the permission
/// core (`Policy::land_document`: the attached `land_authz` index alone,
/// unavailable until its fold is published; `COMMS_LAND_OPERATORS_URL` only
/// without an index). Upstream's `getLandOperators`
/// re-throws on a transport error and on any non-2xx, so a source fault
/// reaches the caller as a rejection and never as "this parcel has no
/// operators". Without any source v2 has no LAND evidence at all: that is
/// the old "places pool unset" degraded mode (no extra address is protected
/// on Genesis places), logged, not a 503.
pub(super) async fn try_fetch_land_operators(
    state: &CommsState,
    parcel: &str,
) -> Result<Option<Arc<Option<LandOperators>>>, ApiError> {
    let Some((x, y)) = parse_xy(parcel) else {
        return Ok(None);
    };
    match state.policy.land_document(Parcel { x, y }).await {
        Ok(Some(doc)) => Ok(Some(doc)),
        Ok(None) => {
            tracing::debug!(
                parcel,
                "no LAND operator source (no land-authz index attached, COMMS_LAND_OPERATORS_URL unset); no LAND operator is protected"
            );
            Ok(None)
        }
        // The index's own name, on a ban and on an admin listing alike.
        Err(e) if e.is_land_index_not_ready() => {
            tracing::warn!(error = %e, parcel, "LAND operators undetermined: land-authz index not ready");
            Err(service_unavailable(LAND_INDEX_UNREADY_MSG))
        }
        Err(e) => {
            tracing::warn!(error = %e, parcel, "LAND operators could not be read");
            Err(service_unavailable(LAND_OPERATORS_UNAVAILABLE_MSG))
        }
    }
}

/// Whether `error` is the attached land-authz index not being published
/// ([`try_fetch_land_operators`]).
fn is_land_index_unready(error: &ApiError) -> bool {
    error.code == 503 && error.message == LAND_INDEX_UNREADY_MSG
}

/// The addresses a place's other permission surfaces grant: the world's
/// owner and deployment/streaming wallets, or the parcel's LAND owner and
/// operators. A surface that cannot be read is an error, never an empty set
/// (an empty set here reads as "nobody is protected").
pub async fn try_get_extra_addresses(
    state: &CommsState,
    place: &PlaceInfo,
) -> Result<BTreeSet<String>, ApiError> {
    let mut extra: BTreeSet<String> = BTreeSet::new();
    if place.world {
        let Some(world) = place.world_name.as_deref().and_then(world_of) else {
            return Ok(extra);
        };
        // The known owner plus every deployment/streaming grantee whose grant
        // is world-wide or touches the place's parcels (the old
        // `/permissions/{perm}/parcels` listings plus the document's owner).
        let protected = state
            .policy
            .protected_addresses(&world, &parcel_set(&place.positions))
            .await
            .map_err(|why| {
                tracing::error!(error = %why, world = %world, "protected addresses could not be read");
                service_unavailable(WORLD_PERMISSIONS_UNAVAILABLE_MSG)
            })?;
        extra.extend(protected.into_iter().map(|a| a.as_str().to_string()));
    } else {
        extra = try_get_land_operator_addresses(state, place).await?;
    }
    Ok(extra)
}

/// The LAND owner and operators of a Genesis place's base parcel (else its
/// first position); an unreadable source is an error.
async fn try_get_land_operator_addresses(
    state: &CommsState,
    place: &PlaceInfo,
) -> Result<BTreeSet<String>, ApiError> {
    let parcel = place
        .base_position
        .clone()
        .or_else(|| place.positions.first().cloned());
    if let Some(parcel) = parcel
        && let Some(doc) = try_fetch_land_operators(state, &parcel).await?
        && let Some(ops) = doc.as_ref()
    {
        return Ok(ops.addresses().collect());
    }
    Ok(BTreeSet::new())
}

/// The listing's best-effort variant: an outage reads as no extra address,
/// except an attached land-authz index that is not published, which is an
/// error: an empty set would mark every Genesis admin removable.
pub async fn get_extra_addresses(
    state: &CommsState,
    place: &PlaceInfo,
) -> Result<BTreeSet<String>, ApiError> {
    match try_get_extra_addresses(state, place).await {
        Ok(extra) => Ok(extra),
        Err(e) if is_land_index_unready(&e) => Err(e),
        Err(_) => Ok(BTreeSet::new()),
    }
}

/// Holders of a linker-server lease over any of `parcels`, from the document
/// at `LEASE_AUTHORIZATIONS_URL` (the mirror posture's: Decentraland's
/// linker-server-authorizations list), counting only entries in force
/// ([`LeaseAuthorization::in_force`]); no URL or an unreadable document reads
/// as no holder, as upstream.
pub async fn get_lease_holders_for_parcels(
    state: &CommsState,
    parcels: &[String],
) -> BTreeSet<String> {
    match try_get_lease_holders_for_parcels(state, parcels).await {
        Ok(holders) => holders,
        Err(error) => {
            tracing::debug!(error = %error.message, "lease authorizations unavailable; no lease holder is listed");
            BTreeSet::new()
        }
    }
}

/// Metadata refresh must preserve its previous roles when a configured reader
/// fails. Other listings retain the best-effort wrapper above.
pub(crate) async fn try_get_lease_holders_for_parcels(
    state: &CommsState,
    parcels: &[String],
) -> Result<BTreeSet<String>, ApiError> {
    let mut holders: BTreeSet<String> = BTreeSet::new();
    let Some(url) = state.settings.lease_authorizations_url.as_deref() else {
        return Ok(holders);
    };
    if parcels.is_empty() {
        return Ok(holders);
    }
    let fetched = state
        .caches
        .lease_holders
        .get_or_fetch("leases".to_string(), || async {
            let resp = state
                .http
                .get(url)
                .timeout(UPSTREAM_TIMEOUT)
                .send()
                .await
                .map_err(|e| e.to_string())?;
            if !resp.status().is_success() {
                return Err(format!("lease authorizations returned {}", resp.status()));
            }
            resp.json::<Vec<serde_json::Value>>()
                .await
                .map(|entries| Arc::new(lease_entries(entries)))
                .map_err(|e| e.to_string())
        })
        .await;
    let auths = match fetched {
        Ok(auths) => auths,
        Err(e) => {
            tracing::debug!(error = %e, "lease authorizations unavailable");
            return Err(service_unavailable(
                "Lease authorizations are temporarily unavailable",
            ));
        }
    };
    let parcel_set: BTreeSet<&str> = parcels.iter().map(String::as_str).collect();
    let now = chrono::Utc::now();
    for auth in auths.iter() {
        if auth.in_force(now, state.settings.lease_dev_entries)
            && auth
                .plots
                .iter()
                .any(|plot| parcel_set.contains(plot.as_str()))
        {
            holders.extend(auth.addresses.iter().map(|a| a.to_lowercase()));
        }
    }
    Ok(holders)
}

/// DCL NAME display names for a bounded set of wallets through the node's
/// NAME source (the old crate read the squid `ens` table). No source or a
/// failing read answers `""`, as before; a failure is never memoized. The
/// nft index is read each time, the whole set in one statement: the memo
/// keeps no publication, so it could serve a name the index has since moved.
pub async fn names_for(
    state: &CommsState,
    addresses: &[String],
) -> std::collections::HashMap<String, String> {
    use crate::chain_index::names::Purpose;
    let mut out = std::collections::HashMap::new();
    // No wallet, no read (a scene without bans, a page past the end).
    if addresses.is_empty() {
        return out;
    }
    let Some(names) = state.name_authority.as_ref() else {
        for addr in addresses {
            out.insert(addr.to_lowercase(), String::new());
        }
        return out;
    };
    // Best-effort display enrichment: a source that cannot answer shows no
    // name (never an authorization).
    if names.observes() {
        let wanted: Vec<String> = addresses.iter().map(|a| a.to_lowercase()).collect();
        let read = names
            .owned_names_of(&wanted, Purpose::Display)
            .await
            .inspect_err(|e| {
                tracing::warn!(error = %e, "names resolve failed; falling back to empty names");
            })
            .ok();
        for addr in wanted {
            let name = read
                .as_ref()
                .and_then(|answer| answer.value.get(&addr))
                .and_then(|held| held.first())
                .map(|n| n.name.clone())
                .unwrap_or_default();
            out.insert(addr, name);
        }
        return out;
    }
    let read = |addr: String| async move {
        match names.owned_names(&addr, Purpose::Display).await {
            Ok(answer) => Ok(answer
                .value
                .first()
                .map(|n| n.name.clone())
                .unwrap_or_default()),
            Err(e) => {
                tracing::warn!(error = %e, "names resolve failed; falling back to an empty name");
                Err(())
            }
        }
    };
    for addr in addresses {
        let addr = addr.to_lowercase();
        let name = state
            .caches
            .names
            .get_or_fetch(addr.clone(), || read(addr.clone()))
            .await;
        out.insert(addr, name.unwrap_or_default());
    }
    out
}

/// An explicit active `scene_admins` row, else ownership of the place: the
/// world's NAME (personal rule, then the name authority; never the stored
/// owner) for a world, the LAND (a parcel directly or through its estate) for a
/// Genesis place, all through `Policy::is_admin`. Without a LAND index a
/// non-admin caller is refused (the old crate's "only explicit scene admins
/// can moderate" degrade); an index that cannot be read or is not published
/// is 503, never "not an owner".
pub async fn scene_owner_or_admin_decision(
    state: &CommsState,
    place: &PlaceLookup<'_>,
    signer: &Address,
) -> PermissionDecision {
    let place_id = place.place_id();
    let place_row = match place.get().await {
        Ok(row) => row,
        Err(error) => {
            tracing::error!(
                error,
                place_id,
                "place lookup failed during the scene authz check"
            );
            return PermissionDecision::Refused(
                AuthorityNotEstablished::UndeterminedStoreUnavailable {
                    store: "explore.place",
                    reason_for_operators: error.to_string(),
                },
            );
        }
    };
    let empty = ParcelSet::new();
    let (world, parcels) = match place_row {
        Some(row) if row.world => (row.world_name.as_deref().and_then(world_of), empty),
        Some(row) => (None, parcel_set(&row.positions)),
        None => (None, empty),
    };
    let scene_ref = match world.as_ref() {
        Some(world) => SceneRef::World {
            world,
            place: place.admin_key(),
        },
        None => SceneRef::Genesis {
            place: place.admin_key(),
            parcels: &parcels,
        },
    };
    match state.policy.is_admin(&scene_ref, signer).await {
        PermissionDecision::Allowed => PermissionDecision::Allowed,
        PermissionDecision::Refused(AuthorityNotEstablished::RefusedLacksAuthority { .. }) => {
            PermissionDecision::refused(NOT_OWNER_OR_ADMIN_MSG)
        }
        undetermined => undetermined,
    }
}

/// Upstream's `isSceneOwnerOrAdmin`, the gate of every scene-signed write
/// (admins, bans, the stream key, Cast): the owner-or-admin decision, or,
/// failing that, an extended holder of the place (a world deployment or
/// streaming grantee, a LAND operator or lease holder), the addresses
/// `/scene-admin` lists beside the explicit admins. A source that cannot be
/// read stays undetermined (503), never a refusal.
pub async fn scene_manager_decision(
    state: &CommsState,
    place: &PlaceLookup<'_>,
    signer: &Address,
) -> PermissionDecision {
    match scene_owner_or_admin_decision(state, place, signer).await {
        PermissionDecision::Refused(AuthorityNotEstablished::RefusedLacksAuthority { .. }) => {}
        decided => return decided,
    }
    let undetermined = |store: &'static str, reason: String| {
        PermissionDecision::Refused(AuthorityNotEstablished::UndeterminedStoreUnavailable {
            store,
            reason_for_operators: reason,
        })
    };
    match place.get().await {
        Ok(Some(row)) => match holds_extended_permission(state, row, signer).await {
            Ok(true) => PermissionDecision::Allowed,
            Ok(false) => PermissionDecision::refused(NOT_OWNER_OR_ADMIN_MSG),
            Err(error) => undetermined("scene permission grants", error.to_string()),
        },
        Ok(None) => PermissionDecision::refused(NOT_OWNER_OR_ADMIN_MSG),
        Err(error) => undetermined("explore.place", error.to_string()),
    }
}

/// Owners, admins, extra addresses and lease holders cannot be banned. A
/// protection surface that cannot be read is answered apart from a genuine
/// "not protected": 503, never "nobody is protected".
pub async fn ensure_target_not_protected(
    state: &CommsState,
    place: &PlaceLookup<'_>,
    target: &Address,
) -> Result<(), ApiError> {
    match scene_owner_or_admin_decision(state, place, target).await {
        PermissionDecision::Allowed => {
            return Err(ApiError::bad_request("Cannot ban this address"));
        }
        PermissionDecision::Refused(AuthorityNotEstablished::RefusedLacksAuthority { .. }) => {}
        PermissionDecision::Refused(why) => return Err(why.into()),
    }
    let place_id = place.place_id().to_string();
    let row = place.get().await.map_err(|error| {
        tracing::error!(error, %place_id, "places lookup failed while checking a ban target for protection");
        service_unavailable(PLACE_LOOKUP_UNAVAILABLE_MSG)
    })?;
    if let Some(row) = row {
        let (extra, leases) = tokio::join!(try_get_extra_addresses(state, row), async {
            if row.world {
                BTreeSet::new()
            } else {
                get_lease_holders_for_parcels(state, &row.positions).await
            }
        });
        let mut protected = extra?;
        protected.extend(leases);
        if protected.contains(target.as_str()) {
            return Err(ApiError::bad_request("Cannot ban this address"));
        }
    }
    Ok(())
}

/// Whether `signer` holds a place through its extended permission surfaces
/// (upstream's `hasExtendedPermissions` and `hasLandLease`): a world's
/// deployment or streaming grantee, or a Genesis place's LAND operator or
/// lease holder. Ownership is not asked here (the owner-or-admin decision
/// already answered it), so unlike the ban-target protection set no owner
/// fallback widens this on a fault. An unreadable grant or operator source
/// is 503; an unreadable lease document reads as no holder, as in the
/// `/scene-admin` listing.
pub async fn holds_extended_permission(
    state: &CommsState,
    place: &PlaceInfo,
    signer: &Address,
) -> Result<bool, ApiError> {
    if place.world {
        let Some(world) = place.world_name.as_deref().and_then(world_of) else {
            return Ok(false);
        };
        let grantees = state
            .policy
            .world_grantees(&world, &parcel_set(&place.positions))
            .await
            .map_err(|why| {
                tracing::error!(error = %why, world = %world, "world grantees could not be read");
                service_unavailable(WORLD_PERMISSIONS_UNAVAILABLE_MSG)
            })?;
        return Ok(grantees.contains(signer));
    }
    let (operators, leases) = tokio::join!(
        try_get_land_operator_addresses(state, place),
        get_lease_holders_for_parcels(state, &place.positions)
    );
    Ok(operators?.contains(signer.as_str()) || leases.contains(signer.as_str()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(doc: serde_json::Value) -> LeaseAuthorization {
        serde_json::from_value(doc).unwrap()
    }

    fn at(raw: &str) -> chrono::DateTime<chrono::Utc> {
        lease_date(raw).unwrap()
    }

    #[test]
    fn a_lease_entry_grants_only_within_its_dates() {
        let dated = entry(serde_json::json!({
            "addresses": ["0xa"], "plots": ["1,1"],
            "startDate": "2024-03-01", "endDate": "2024-03-30"
        }));
        assert!(!dated.in_force(at("2024-02-29T23:59:59Z"), false));
        assert!(dated.in_force(at("2024-03-01"), false));
        assert!(dated.in_force(at("2024-03-30"), false));
        assert!(!dated.in_force(at("2024-03-30T00:00:01Z"), false));
        let open = entry(serde_json::json!({
            "addresses": ["0xa"], "plots": ["1,1"],
            "startDate": null, "endDate": "", "onlyDev": null
        }));
        assert!(open.in_force(at("2030-01-01"), false));
        let precise = entry(serde_json::json!({
            "addresses": ["0xa"], "plots": ["1,1"], "endDate": "2024-03-30T12:00:00+02:00"
        }));
        assert!(precise.in_force(at("2024-03-30T10:00:00Z"), false));
        assert!(!precise.in_force(at("2024-03-30T10:00:01Z"), false));
    }

    #[test]
    fn a_lease_entry_with_an_unreadable_date_grants_nothing() {
        let unreadable = entry(serde_json::json!({
            "addresses": ["0xa"], "plots": ["1,1"], "endDate": "next spring"
        }));
        assert!(!unreadable.in_force(at("2024-03-01"), true));
    }

    #[test]
    fn a_dev_only_lease_entry_grants_only_on_a_development_node() {
        let dev = entry(serde_json::json!({
            "addresses": ["0xa"], "plots": ["1,1"], "onlyDev": true
        }));
        assert!(!dev.in_force(at("2024-03-01"), false));
        assert!(dev.in_force(at("2024-03-01"), true));
        let explicit = entry(serde_json::json!({
            "addresses": ["0xa"], "plots": ["1,1"], "onlyDev": false
        }));
        assert!(explicit.in_force(at("2024-03-01"), false));
    }

    /// A NAME source that counts its reads: the one-owner read and the
    /// batch apart, or every read failing.
    struct Counting {
        observes: bool,
        fails: bool,
        single: std::sync::atomic::AtomicUsize,
        batched: std::sync::atomic::AtomicUsize,
    }

    impl Counting {
        fn new(observes: bool, fails: bool) -> Arc<Self> {
            Arc::new(Self {
                observes,
                fails,
                single: Default::default(),
                batched: Default::default(),
            })
        }

        fn held(owner: &str) -> Vec<crate::chain_index::names::OwnedName> {
            // The wallets ending in an even digit hold one NAME, spelled
            // after the wallet; the others hold none.
            let even = owner
                .chars()
                .last()
                .and_then(|c| c.to_digit(16))
                .is_some_and(|d| d % 2 == 0);
            even.then(|| crate::chain_index::names::OwnedName {
                name: format!("n{}", &owner[owner.len() - 4..]),
                contract_address: "0x0000000000000000000000000000000000000001".into(),
                token_id: "1".into(),
                price: None,
            })
            .into_iter()
            .collect()
        }

        fn refused(&self) -> Result<(), crate::chain_index::names::NamesError> {
            if self.fails {
                Err(crate::chain_index::names::NamesError::NotEstablished(
                    "the test source is not published".into(),
                ))
            } else {
                Ok(())
            }
        }
    }

    #[async_trait::async_trait]
    impl crate::chain_index::names::NameAuthority for Counting {
        async fn owned_names(
            &self,
            owner: &str,
            _: crate::chain_index::names::Purpose,
        ) -> Result<
            crate::chain_index::names::Answer<Arc<Vec<crate::chain_index::names::OwnedName>>>,
            crate::chain_index::names::NamesError,
        > {
            self.single
                .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            self.refused()?;
            Ok(crate::chain_index::names::Answer {
                value: Arc::new(Self::held(owner)),
                observation: None,
            })
        }

        async fn owned_names_of(
            &self,
            owners: &[String],
            _: crate::chain_index::names::Purpose,
        ) -> Result<
            crate::chain_index::names::Answer<
                std::collections::HashMap<String, Arc<Vec<crate::chain_index::names::OwnedName>>>,
            >,
            crate::chain_index::names::NamesError,
        > {
            self.batched
                .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            self.refused()?;
            Ok(crate::chain_index::names::Answer {
                value: owners
                    .iter()
                    .map(|o| (o.clone(), Arc::new(Self::held(o))))
                    .collect(),
                observation: None,
            })
        }

        async fn name_owner(
            &self,
            _: &str,
            _: crate::chain_index::names::Purpose,
        ) -> Result<
            crate::chain_index::names::Answer<Option<String>>,
            crate::chain_index::names::NamesError,
        > {
            unreachable!("display names read owners")
        }

        async fn name_owners(
            &self,
            _: &[String],
            _: crate::chain_index::names::Purpose,
        ) -> Result<
            crate::chain_index::names::Answer<std::collections::HashMap<String, Option<String>>>,
            crate::chain_index::names::NamesError,
        > {
            unreachable!("display names read owners")
        }

        async fn still_current(
            &self,
            _: &crate::chain_index::names::Observation,
            _: crate::chain_index::names::Purpose,
        ) -> Result<bool, crate::chain_index::names::NamesError> {
            Ok(true)
        }

        fn observes(&self) -> bool {
            self.observes
        }
    }

    fn state_with(source: Arc<Counting>) -> CommsState {
        use crate::interfaces::test_support::{StubDirectory, StubPolicy, StubScenes};
        CommsState::new(
            sqlx::PgPool::connect_lazy("postgres://postgres@127.0.0.1:1/postgres").unwrap(),
            crate::http::Outbound::for_tests(),
            crate::gatekeeper::Settings::for_tests(),
            None,
            None,
            Arc::new(StubPolicy::default()),
            Arc::new(StubScenes::default()),
            Arc::new(crate::presence::Presence::new()),
            Arc::new(StubDirectory::default()),
        )
        .with_name_authority(Some(
            source as Arc<dyn crate::chain_index::names::NameAuthority>,
        ))
    }

    fn wallets(n: usize) -> Vec<String> {
        (0..n).map(|i| format!("0xAB{i:038x}")).collect()
    }

    /// The nft index answers a scene's whole set of display names in one
    /// read, keyed lowercased, a wallet holding none answering `""`.
    #[tokio::test]
    async fn the_index_reads_every_display_name_at_once() {
        let source = Counting::new(true, false);
        let state = state_with(source.clone());
        let asked = wallets(50);
        let names = names_for(&state, &asked).await;
        assert_eq!(source.batched.load(std::sync::atomic::Ordering::SeqCst), 1);
        assert_eq!(source.single.load(std::sync::atomic::Ordering::SeqCst), 0);
        assert_eq!(names.len(), 50);
        for wallet in &asked {
            let wallet = wallet.to_lowercase();
            let expected = Counting::held(&wallet)
                .first()
                .map(|n| n.name.clone())
                .unwrap_or_default();
            assert_eq!(names[&wallet], expected, "{wallet}");
        }
        assert!(names.values().any(String::is_empty));
        assert!(names.values().any(|n| !n.is_empty()));
    }

    /// No wallet asks nothing of the source.
    #[tokio::test]
    async fn no_wallets_read_nothing() {
        let source = Counting::new(true, true);
        let state = state_with(source.clone());
        assert!(names_for(&state, &[]).await.is_empty());
        assert_eq!(source.batched.load(std::sync::atomic::Ordering::SeqCst), 0);
        assert_eq!(source.single.load(std::sync::atomic::Ordering::SeqCst), 0);
    }

    /// An index that cannot answer shows no names, nothing kept: the next
    /// call reads again.
    #[tokio::test]
    async fn an_unanswered_index_shows_empty_names_and_reads_again() {
        let source = Counting::new(true, true);
        let state = state_with(source.clone());
        let asked = wallets(3);
        for round in 1..=2 {
            let names = names_for(&state, &asked).await;
            assert_eq!(names.len(), 3);
            assert!(names.values().all(String::is_empty));
            assert_eq!(
                source.batched.load(std::sync::atomic::Ordering::SeqCst),
                round
            );
        }
    }

    /// A source without publications (the subgraph) keeps its per-wallet
    /// memo: a second call reads nothing.
    #[tokio::test]
    async fn the_subgraph_keeps_its_per_wallet_memo() {
        let source = Counting::new(false, false);
        let state = state_with(source.clone());
        let asked = wallets(4);
        let first = names_for(&state, &asked).await;
        let second = names_for(&state, &asked).await;
        assert_eq!(first, second);
        assert_eq!(source.single.load(std::sync::atomic::Ordering::SeqCst), 4);
        assert_eq!(source.batched.load(std::sync::atomic::Ordering::SeqCst), 0);
    }
}

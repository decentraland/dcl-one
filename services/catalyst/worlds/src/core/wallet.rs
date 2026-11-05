//! The wallet routes of the worlds content server: `GET /wallet/contribute`
//! (signed: the worlds the signer holds a deployment or streaming grant in)
//! and `GET /wallet/{wallet}/stats` (the wallet's worlds, the space they use
//! and the space it may use).

use std::collections::BTreeMap;

use axum::Json;
use axum::extract::{OriginalUri, Path, State};
use axum::http::{HeaderMap, Uri};
use chrono::NaiveDateTime;
use serde::Serialize;
use sqlx::Row;

use crate::comms::world_identity::WorldName;
use crate::policy::Policy;
use crate::shared::identity::Address;
use crate::shared::signed_fetch::SignedPath;

use super::SharedState;
use super::auth::{map_auth_error_api, require_verified};
use super::http::ApiError;

#[derive(Debug, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, export_to = "worlds/"))]
pub struct ContributorDomain {
    pub name: String,
    pub user_permissions: Vec<String>,
    pub size: String,
    pub owner: Option<String>,
    #[serde(rename = "parcelCount")]
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub parcel_count: i64,
}

#[derive(Debug, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, export_to = "worlds/"))]
pub struct ContributableDomains {
    pub domains: Vec<ContributorDomain>,
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub count: usize,
}

/// GET /wallet/contribute
pub async fn get_contributable_domains(
    State(state): State<SharedState>,
    OriginalUri(original): OriginalUri,
    uri: Uri,
    headers: HeaderMap,
) -> Result<Json<ContributableDomains>, ApiError> {
    let auth = require_verified(
        &headers,
        "get",
        &SignedPath::from_uris(&original, &uri),
        &[],
        state.eip1654.as_deref(),
    )
    .await
    .map_err(map_auth_error_api)?;
    // parcelCount: 0 when any grant is world-wide, else the smallest scope.
    let rows = sqlx::query(
        "SELECT w.name, w.owner, \
                array_agg(DISTINCT wp.permission ORDER BY wp.permission) AS user_permissions, \
                (SELECT COALESCE(sum(s.size), 0) FROM worlds.world_scenes s \
                  WHERE s.world_name = w.name)::text AS size, \
                CASE WHEN bool_or(pc.n = 0) THEN 0 ELSE min(pc.n) END AS parcel_count \
           FROM worlds.world_permissions wp \
           JOIN worlds.worlds w ON w.name = wp.world_name \
           CROSS JOIN LATERAL ( \
             SELECT count(*) AS n FROM worlds.world_permission_parcels p \
              WHERE p.world_name = wp.world_name AND p.permission = wp.permission \
                AND p.address = wp.address) pc \
          WHERE wp.address = $1 AND wp.permission IN ('deployment', 'streaming') \
          GROUP BY w.name, w.owner \
          ORDER BY w.name",
    )
    .bind(auth.signer.as_str())
    .fetch_all(&state.pool)
    .await?;
    let domains: Vec<ContributorDomain> = rows
        .iter()
        .map(|r| ContributorDomain {
            name: r.get("name"),
            user_permissions: r.get("user_permissions"),
            size: r.get("size"),
            owner: r.get("owner"),
            parcel_count: r.get("parcel_count"),
        })
        .collect();
    Ok(Json(ContributableDomains {
        count: domains.len(),
        domains,
    }))
}

#[derive(Debug, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, export_to = "worlds/"))]
pub struct WorldSize {
    pub name: String,
    pub size: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, export_to = "worlds/"))]
pub struct WalletStats {
    pub wallet: String,
    pub dcl_names: Vec<WorldSize>,
    pub ens_names: Vec<WorldSize>,
    pub used_space: String,
    pub max_allowed_space: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub blocked_since: Option<String>,
}

/// The storage the wallet may use across its DCL worlds: one world-size cap
/// (the publish limit) per DCL world it owns.
pub fn max_allowed_space<'a>(
    policy: &Policy,
    owned_dcl_worlds: impl IntoIterator<Item = &'a WorldName>,
) -> i64 {
    owned_dcl_worlds
        .into_iter()
        .map(|w| policy.world_size_cap(w))
        .sum()
}

fn is_eth_address(raw: &str) -> bool {
    raw.len() == 42 && raw.starts_with("0x") && raw[2..].bytes().all(|b| b.is_ascii_hexdigit())
}

/// GET /wallet/{wallet}/stats
pub async fn get_wallet_stats(
    State(state): State<SharedState>,
    Path(wallet): Path<String>,
) -> Result<Json<WalletStats>, ApiError> {
    let address = Some(wallet.as_str())
        .filter(|w| is_eth_address(w))
        .and_then(|w| Address::parse(w).ok())
        .ok_or_else(|| {
            ApiError::bad_request(
                "Invalid request. Missing or invalid wallet in request url param.",
            )
        })?;
    let policy = &state.policy;
    // The DCL worlds the wallet owns now: its held NAMEs and its personal world.
    let held = policy.held_name_labels(&address).await?;
    let mut owned_dcl: BTreeMap<String, WorldName> = held
        .iter()
        .flatten()
        .filter_map(|label| WorldName::parse(&format!("{label}.dcl.eth")).ok())
        .map(|w| (w.key().to_string(), w))
        .collect();
    let personal = WorldName::parse(&format!("{}.dcl.eth", address.as_str()))
        .ok()
        .filter(|w| policy.personal_worlds().owner_of(w).as_ref() == Some(&address));
    owned_dcl.extend(personal.map(|w| (w.key().to_string(), w)));
    let candidates: Vec<String> = owned_dcl.keys().cloned().collect();

    let rows = sqlx::query(
        "SELECT w.name, w.owner, COALESCE(sum(s.size), 0)::bigint AS size \
           FROM worlds.worlds w \
           LEFT JOIN worlds.world_scenes s ON s.world_name = w.name \
          WHERE w.owner = $1 OR w.name = ANY($2) \
          GROUP BY w.name, w.owner \
          ORDER BY w.name",
    )
    .bind(address.as_str())
    .bind(&candidates)
    .fetch_all(&state.pool)
    .await?;

    let (mut dcl_names, mut ens_names, mut used_space) = (Vec::new(), Vec::new(), 0i64);
    for r in &rows {
        let name: String = r.get("name");
        let row_owner: Option<String> = r.get("owner");
        let size: i64 = r.get("size");
        let Ok(world) = WorldName::parse(&name) else {
            continue;
        };
        let owns_row = row_owner.as_deref() == Some(address.as_str());
        if world.is_dcl_eth() {
            // With a names reader the NAME decides; without one the row's owner does.
            let owned = owned_dcl.contains_key(world.key()) || (held.is_none() && owns_row);
            if !owned {
                continue;
            }
            owned_dcl.insert(world.key().to_string(), world);
            used_space += size;
            dcl_names.push(WorldSize {
                name,
                size: size.to_string(),
            });
        } else if owns_row {
            ens_names.push(WorldSize {
                name,
                size: size.to_string(),
            });
        }
    }

    let blocked_since: Option<NaiveDateTime> =
        sqlx::query_scalar("SELECT created_at FROM worlds.blocked_wallets WHERE wallet = $1")
            .bind(address.as_str())
            .fetch_optional(&state.pool)
            .await?;

    Ok(Json(WalletStats {
        wallet,
        dcl_names,
        ens_names,
        used_space: used_space.to_string(),
        max_allowed_space: max_allowed_space(policy, owned_dcl.values()).to_string(),
        blocked_since: blocked_since.map(|t| {
            t.and_utc()
                .to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
        }),
    }))
}

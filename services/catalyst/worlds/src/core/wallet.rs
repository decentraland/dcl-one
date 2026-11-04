//! The wallet routes of the worlds content server: `GET /wallet/contribute`
//! (signed: the worlds the signer holds a deployment or streaming grant in)
//! and `GET /wallet/{wallet}/stats` (the worlds stored as the wallet's, the
//! space its DCL NAME worlds use, its storage allowance and its quota block).

use axum::Json;
use axum::extract::{OriginalUri, Path, State};
use axum::http::{HeaderMap, Uri};
use chrono::NaiveDateTime;
use serde::Serialize;
use sqlx::Row;

use crate::shared::identity::Address;
use crate::shared::signed_fetch::SignedPath;

use super::SharedState;
use super::auth::{map_auth_error_api, require_verified};
use super::http::ApiError;
use super::quota;

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
    let allowance = state
        .allowance
        .allowance_bytes(&address)
        .await
        .map_err(|e| {
            tracing::warn!(wallet = %address.as_str(), error = %e, "wallet stats: allowance unavailable");
            ApiError::service_unavailable("The wallet's storage allowance could not be read right now")
        })?;
    let usage = quota::wallet_usage(&state.pool, &address).await?;
    let sizes = |worlds: Vec<quota::WorldUsage>| -> Vec<WorldSize> {
        worlds
            .into_iter()
            .map(|w| WorldSize {
                name: w.name,
                size: w.size.to_string(),
            })
            .collect()
    };
    let blocked_since: Option<NaiveDateTime> =
        sqlx::query_scalar("SELECT created_at FROM worlds.quota_blocks WHERE wallet = $1")
            .bind(address.as_str())
            .fetch_optional(&state.pool)
            .await?;

    Ok(Json(WalletStats {
        wallet,
        dcl_names: sizes(usage.dcl_names),
        ens_names: sizes(usage.ens_names),
        used_space: usage.used_space.to_string(),
        max_allowed_space: allowance.to_string(),
        blocked_since: blocked_since.map(|t| {
            t.and_utc()
                .to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
        }),
    }))
}

//! The lambdas land routes over the index: the operators of a parcel (what
//! comms' `COMMS_LAND_OPERATORS_URL` fetches), the reverse lookup of an
//! update operator, the per-address permission flags (single and batch) and
//! the index status. Each names its source per the module doc: the index
//! once ready; before that the land-manager subgraph (`LAND_SUBGRAPH_URL`,
//! `chain_index::land_graph`, lamb2's only source) for the operators, the
//! single permissions route and the reverse lookup, and the marketplace
//! reader's owner leg for the batch; the unavailable envelope otherwise.

use std::collections::HashSet;

use axum::Json;
use axum::Router;
use axum::extract::{DefaultBodyLimit, Path, Request, State};
use axum::http::{HeaderName, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use serde_json::{Value, json};

use super::resolve::operator_flags;
use super::{
    DecisionSource, LandOperators, ParcelPermissionFlags, SharedState, Unavailable, Verdict,
    can_deploy_batch, indexer, is_address,
};
use crate::chain_index::source::{Source, unavailable};
use crate::chain_index::{ChainIndex, land_graph};
use crate::shared::http_util::{bad_request, fetcher_error, internal_server_error, lambdas_error};
use crate::shared::parcel::is_inside_world_limits;
use crate::shared::query_params::{JsPagination, js_parse_int, parse_query_string};
use crate::shared::{cors, nul_guard};

const COORDS_ERR: &str = "Coordinates X and Y must be valid numbers in a valid range";
/// One footprint's worth of `"x,y"` parcels; a request above this answers 400,
/// never a truncated batch.
pub const MAX_BATCH_PARCELS: usize = 100;
pub const SOURCE_HEADER: &str = "x-land-authz-source";
const NOT_READY_MSG: &str = "LAND rights are unavailable on this node: the land-authz index has no published fold yet (it is replaying the registries' logs, or the indexer is off) and no LAND subgraph is configured";
const NO_SOURCE_MSG: &str = "LAND rights are unavailable on this node: the land-authz index is not ready and no marketplace subgraph is configured";

pub fn router(state: SharedState) -> Router {
    Router::new()
        .route("/lambdas/parcels/{x}/{y}/operators", get(parcel_operators))
        .route(
            "/lambdas/users/{address}/lands-permissions",
            get(user_lands_permissions),
        )
        .route(
            "/lambdas/users/{address}/parcels/{x}/{y}/permissions",
            get(parcel_permissions),
        )
        .route(
            "/lambdas/users/{address}/parcels/permissions",
            post(parcels_permissions_batch).layer(DefaultBodyLimit::max(64 * 1024)),
        )
        .route("/lambdas/land-authz/status", get(status))
        .layer(axum::middleware::from_fn(nul_guard::nul_guard_middleware))
        .layer(axum::middleware::from_fn(cors::cors_middleware))
        .with_state(state)
}

fn parcel_or_estate_not_found(x: i64, y: i64) -> Response {
    lambdas_error(
        StatusCode::NOT_FOUND,
        "Bad Request",
        &format!("Parcel or estate rights not found for x: {x}, y: {y}"),
    )
}

fn not_ready() -> Response {
    unavailable(Source::LandAuthzIndex, NOT_READY_MSG)
}

/// The land-manager subgraph reader, when the chain index carries its URL.
fn land_graph_reader(state: &SharedState) -> Option<&ChainIndex> {
    state
        .market
        .as_deref()
        .filter(|m| land_graph::configured(m))
}

/// lamb2's answer to any error it does not map (`errorHandler.ts`): 500 and
/// no `message`.
fn upstream_internal_error() -> Response {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(json!({ "error": "Internal Server Error" })),
    )
        .into_response()
}

fn unavailable_response(why: &Unavailable) -> Response {
    match why {
        Unavailable::NoSource => unavailable(Source::LandAuthzIndex, NO_SOURCE_MSG),
        Unavailable::Index(e) => {
            tracing::warn!(error = %e, "land-authz: index read failed");
            internal_server_error()
        }
        Unavailable::Reader(e) => {
            tracing::warn!(error = %e, "land-authz: marketplace LAND reader failed");
            unavailable(
                Source::MarketplaceSubgraph,
                "the marketplace LAND reader did not answer",
            )
        }
    }
}

/// An integer pair inside Genesis City, the parcels `Parcel.validate`
/// accepts upstream ([`is_inside_world_limits`]).
pub(crate) fn validate_coords(x: &str, y: &str) -> Result<(i64, i64), Box<Response>> {
    let xi: i64 = x.parse().map_err(|_| bad_request(COORDS_ERR))?;
    let yi: i64 = y.parse().map_err(|_| bad_request(COORDS_ERR))?;
    if !is_inside_world_limits(xi, yi) {
        return Err(Box::new(bad_request(COORDS_ERR)));
    }
    Ok((xi, yi))
}

/// The single-parcel routes' coordinates as lamb2 reads them: `parseInt(x)`
/// (white space, a sign, a `0x` prefix; anything after the digits ignored),
/// then the same [`is_inside_world_limits`]. The batch keeps
/// [`validate_coords`].
fn validate_path_coords(x: &str, y: &str) -> Result<(i64, i64), Box<Response>> {
    match (js_parse_int(x, true), js_parse_int(y, true)) {
        (Some(xi), Some(yi)) if is_inside_world_limits(xi, yi) => Ok((xi, yi)),
        _ => Err(Box::new(bad_request(COORDS_ERR))),
    }
}

/// `GET /lambdas/parcels/{x}/{y}/operators`: the subject and the four legs
/// from the index; 404 for an unindexed parcel; before the index is ready
/// the land-manager subgraph, as the old route (owner-only from the
/// marketplace reader would misreport the operators); the envelope with
/// neither. A failed subgraph or index read answers lamb2's bare 500.
async fn parcel_operators(
    State(state): State<SharedState>,
    Path((x, y)): Path<(String, String)>,
) -> Response {
    let (xi, yi) = match validate_path_coords(&x, &y) {
        Ok(c) => c,
        Err(resp) => return *resp,
    };
    if state.ready().await.is_none() {
        let Some(market) = land_graph_reader(&state) else {
            return not_ready();
        };
        return match land_graph::parcel_operators(market, xi, yi).await {
            Ok(Some(ops)) => Json(json!({
                "owner": ops.owner,
                "operator": ops.operator,
                "updateOperator": ops.update_operator,
                "updateManagers": ops.update_managers,
                "approvedForAll": ops.approved_for_all,
            }))
            .into_response(),
            Ok(None) => parcel_or_estate_not_found(xi, yi),
            Err(e) => {
                tracing::warn!(error = %e, "land-authz: LAND subgraph reader failed on parcel operators");
                upstream_internal_error()
            }
        };
    }
    match state.store().rights(xi as i32, yi as i32).await {
        Ok(Some(rights)) => Json(json!({
            "owner": rights.subject.owner,
            "operator": rights.subject.operator,
            "updateOperator": rights.subject.update_operator,
            "updateManagers": rights.operators.update_managers,
            "approvedForAll": rights.operators.approved_for_all,
        }))
        .into_response(),
        Ok(None) => parcel_or_estate_not_found(xi, yi),
        Err(e) => {
            tracing::warn!(x = xi, y = yi, error = %e, "land-authz: local parcel operators lookup failed");
            upstream_internal_error()
        }
    }
}

/// `GET /lambdas/users/{address}/lands-permissions`: the parcels whose own
/// update operator is `address`, paged as lamb2's `fetchAndPaginate`
/// ([`JsPagination`], no `pageSize` maximum: `Number.MAX_VALUE`),
/// `totalAmount` from the same query. A failed LAND subgraph answers
/// lamb2's 502 `FetcherError`.
async fn user_lands_permissions(
    State(state): State<SharedState>,
    Path(addr): Path<String>,
    req: Request,
) -> Response {
    let update_operator = addr.to_lowercase();
    let params = parse_query_string(req.uri().query().unwrap_or(""));
    let p = match JsPagination::parse(&params, i64::MAX) {
        Ok(p) => p,
        Err(msg) => return bad_request(&msg),
    };
    if state.ready().await.is_none() {
        let Some(market) = land_graph_reader(&state) else {
            return not_ready();
        };
        return match land_graph::parcels_by_update_operator(market, &update_operator).await {
            Ok(elements) => {
                let total = elements.len() as i64;
                let page: Vec<Value> = elements[p.range(elements.len())].to_vec();
                Json(json!({ "elements": page, "totalAmount": total, "pageNum": p.page_num, "pageSize": p.page_size })).into_response()
            }
            Err(e) => {
                tracing::warn!(error = %e, "land-authz: LAND subgraph reader failed on lands-permissions");
                fetcher_error(&addr)
            }
        };
    }
    let store = state.store();
    // A negative bound counts from the end of the list: its length first.
    let window = match p.sql_window() {
        Some(window) => Ok(window),
        None => store
            .parcels_with_update_operator_page(&update_operator, 0, 0)
            .await
            .map(|(_, total)| {
                let r = p.range(total.max(0) as usize);
                (r.start as i64, r.len() as i64)
            }),
    };
    let page = match window {
        Ok((offset, limit)) => {
            store
                .parcels_with_update_operator_page(&update_operator, limit, offset)
                .await
        }
        Err(e) => Err(e),
    };
    match page {
        Ok((parcels, total)) => {
            let elements: Vec<Value> = parcels
                .into_iter()
                .map(|parcel| {
                    json!({
                        "id": parcel.id,
                        "x": parcel.x.to_string(),
                        "y": parcel.y.to_string(),
                        "owner": parcel.owner,
                        "updateOperator": update_operator,
                    })
                })
                .collect();
            Json(json!({ "elements": elements, "totalAmount": total, "pageNum": p.page_num, "pageSize": p.page_size }))
                .into_response()
        }
        Err(e) => {
            tracing::error!(update_operator = %update_operator, error = %e, "land-authz: lands-permissions lookup failed");
            upstream_internal_error()
        }
    }
}

/// The flags on the wire. From the index every leg is decided; from the
/// marketplace subgraph only `owner` is (its Parcel and Estate carry no
/// operator, updateOperator, updateManager or approvedForAll: probed
/// 2026-09-25), so those four legs are `null`, an undecided leg, never a
/// `false` that reads as a denial. The write decision behind them stays
/// fail-closed (`can_deploy` denies what it cannot see).
fn flags_value(flags: ParcelPermissionFlags, source: &DecisionSource) -> Value {
    let mut v = serde_json::to_value(flags).expect("flags serialize");
    if matches!(source, DecisionSource::SubgraphOwnerOnly) {
        for leg in [
            "operator",
            "updateOperator",
            "updateManager",
            "approvedForAll",
        ] {
            v[leg] = Value::Null;
        }
    }
    v
}

fn with_source(mut resp: Response, source: &'static str) -> Response {
    resp.headers_mut().insert(
        HeaderName::from_static(SOURCE_HEADER),
        HeaderValue::from_static(source),
    );
    resp
}

/// `GET /lambdas/users/{address}/parcels/{x}/{y}/permissions`: five
/// booleans, never `null`. From the index once ready (the flags
/// [`can_deploy`](super::can_deploy) decides from); before that from the
/// land-manager subgraph's operators, lamb2's `getParcelPermissions`; with
/// neither, lamb2's 500.
async fn parcel_permissions(
    State(state): State<SharedState>,
    Path((address, x, y)): Path<(String, String, String)>,
) -> Response {
    let (xi, yi) = match validate_path_coords(&x, &y) {
        Ok(c) => c,
        Err(resp) => return *resp,
    };
    // `EthAddress.validate`: `^0x[a-fA-F0-9]{40}$`, so `0X…` is refused.
    if !(address.starts_with("0x") && is_address(&address)) {
        return bad_request("Address must be a valid Ethereum address");
    }
    let addr = address.to_lowercase();
    if state.ready().await.is_none() {
        let Some(market) = land_graph_reader(&state) else {
            tracing::warn!(
                "land-authz: parcel permissions have no source (index not ready, no LAND_SUBGRAPH_URL)"
            );
            return upstream_internal_error();
        };
        let source = Source::LandSubgraph.name();
        return match land_graph::parcel_operators(market, xi, yi).await {
            Ok(Some(ops)) => {
                let mut flags = operator_flags(
                    &addr,
                    &LandOperators {
                        operator: ops.operator,
                        update_operator: ops.update_operator,
                        update_managers: ops.update_managers,
                        approved_for_all: ops.approved_for_all,
                    },
                );
                flags.owner = ops.owner.eq_ignore_ascii_case(&addr);
                with_source(Json(flags).into_response(), source)
            }
            Ok(None) => with_source(parcel_or_estate_not_found(xi, yi), source),
            Err(e) => {
                tracing::warn!(error = %e, "land-authz: LAND subgraph reader failed on parcel permissions");
                upstream_internal_error()
            }
        };
    }
    match state.store().rights(xi as i32, yi as i32).await {
        Ok(Some(rights)) => with_source(
            Json(rights.flags_for(&addr)).into_response(),
            "land-authz-index",
        ),
        Ok(None) => with_source(parcel_or_estate_not_found(xi, yi), "land-authz-index"),
        Err(e) => {
            tracing::warn!(x = xi, y = yi, error = %e, "land-authz: parcel permissions lookup failed");
            upstream_internal_error()
        }
    }
}

#[derive(Debug, serde::Deserialize)]
pub struct BatchParcelPermissionsRequest {
    pub parcels: Vec<String>,
}

/// Per-entry rule is exactly `validate_coords` (integer parse plus the
/// Genesis City bounds); any invalid entry rejects the whole request, naming the
/// entry. Duplicates collapse silently to their first occurrence, so the
/// response stays one element per distinct parcel in request order.
pub(crate) fn parse_batch_parcels(entries: &[String]) -> Result<Vec<(i64, i64)>, String> {
    if entries.len() > MAX_BATCH_PARCELS {
        return Err(format!(
            "Too many parcels: {} requested, the limit is {MAX_BATCH_PARCELS}",
            entries.len()
        ));
    }
    let mut seen = HashSet::new();
    let mut parcels = Vec::with_capacity(entries.len());
    for entry in entries {
        let coords = entry
            .split_once(',')
            .ok_or(())
            .and_then(|(x, y)| validate_coords(x, y).map_err(|_| ()));
        let Ok((xi, yi)) = coords else {
            return Err(format!("Invalid parcel \"{entry}\": {COORDS_ERR}"));
        };
        if seen.insert((xi, yi)) {
            parcels.push((xi, yi));
        }
    }
    Ok(parcels)
}

/// `POST /lambdas/users/{address}/parcels/permissions`: the batch analogue,
/// one index round trip for the set; a parcel with no row answers
/// `"permissions": null`, the batch reading of the single route's 404.
async fn parcels_permissions_batch(
    State(state): State<SharedState>,
    Path(address): Path<String>,
    Json(body): Json<BatchParcelPermissionsRequest>,
) -> Response {
    let addr = address.to_lowercase();
    if !is_address(&addr) {
        return bad_request("Address must be a valid Ethereum address");
    }
    let parcels = match parse_batch_parcels(&body.parcels) {
        Ok(p) => p,
        Err(msg) => return bad_request(&msg),
    };
    let coords: Vec<(i32, i32)> = parcels.iter().map(|&(x, y)| (x as i32, y as i32)).collect();
    match can_deploy_batch(&state, &coords, &addr).await {
        Ok(decisions) => {
            let source = decisions
                .first()
                .map(|d| d.source.clone())
                .unwrap_or(DecisionSource::SubgraphOwnerOnly);
            let elements: Vec<Value> = coords
                .iter()
                .zip(decisions)
                .map(|(&(x, y), d)| {
                    let permissions = match d.verdict {
                        Verdict::Unknown => Value::Null,
                        _ => flags_value(d.flags, &d.source),
                    };
                    json!({ "x": x, "y": y, "permissions": permissions })
                })
                .collect();
            let resp = Json(json!({ "elements": elements })).into_response();
            if coords.is_empty() {
                resp
            } else {
                with_source(resp, source.header_value())
            }
        }
        Err(why) => unavailable_response(&why),
    }
}

/// `GET /lambdas/land-authz/status`: the cursor row and the readiness rule
/// applied to it now, for operators and the tests. Always 200. The cursor's
/// own registries are shown: a cursor indexed from other registries than
/// this node's is never ready for it.
async fn status(State(state): State<SharedState>) -> Response {
    let settings = &state.settings;
    let observation = match indexer::cursor_observed(&state.pool, &settings.network).await {
        Ok(row) => row,
        Err(e) => {
            tracing::warn!(error = %e, "land-authz: cannot read index_cursor");
            return internal_server_error();
        }
    };
    let ready = observation.as_ref().is_some_and(|o| {
        indexer::observation_ready(o, &settings.land_registry, &settings.estate_registry)
    });
    let fold_age_secs = observation.as_ref().and_then(|o| {
        o.cursor
            .folded_at
            .map(|at| o.observed_at.signed_duration_since(at).num_seconds())
    });
    let row = observation.map(|o| o.cursor);
    Json(json!({
        "network": settings.network,
        "landRegistry": settings.land_registry,
        "estateRegistry": settings.estate_registry,
        "indexerEnabled": settings.is_configured(),
        "ready": ready,
        "readyWindowSecs": indexer::READY_WINDOW.num_seconds(),
        "fallback": state.market.as_ref().is_some_and(|m| m.settings.marketplace_subgraph_url.is_some()).then_some("marketplace-subgraph:owner-only"),
        "cursor": row.map(|r| json!({
            "landRegistry": r.land_registry,
            "estateRegistry": r.estate_registry,
            "lastBlock": r.last_block,
            "foldedBlock": r.folded_block,
            "foldedAt": r.folded_at.map(|t| t.and_utc().to_rfc3339()),
            "foldAgeSecs": fold_age_secs,
            "fullReplayStarted": r.full_replay_started,
            "updatedAt": r.updated_at.and_utc().to_rfc3339(),
        })),
    }))
    .into_response()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn batch_parcels_dedupe_silently_in_request_order() {
        let entries = ["1,2", "3,4", "1,2", "5,6"].map(String::from);
        assert_eq!(
            parse_batch_parcels(&entries).unwrap(),
            vec![(1, 2), (3, 4), (5, 6)]
        );
    }

    #[test]
    fn batch_parcels_reject_any_invalid_entry_naming_it() {
        let entries = ["1,2", "x,4"].map(String::from);
        let err = parse_batch_parcels(&entries).unwrap_err();
        assert!(err.starts_with("Invalid parcel \"x,4\""), "{err}");
        let exception_block = ["62,151", "162,158"].map(String::from);
        assert_eq!(
            parse_batch_parcels(&exception_block).unwrap(),
            vec![(62, 151), (162, 158)]
        );
        let out_of_range = ["1,2", "151,0"].map(String::from);
        assert!(
            parse_batch_parcels(&out_of_range)
                .unwrap_err()
                .contains("151,0")
        );
        let no_comma = ["12".to_string()];
        assert!(parse_batch_parcels(&no_comma).is_err());
    }

    #[test]
    fn batch_parcels_capped_at_100() {
        let ok: Vec<String> = (0..100)
            .map(|i| format!("{},{}", i % 150, i / 150))
            .collect();
        assert_eq!(parse_batch_parcels(&ok).unwrap().len(), 100);
        let too_many: Vec<String> = (0..101).map(|i| format!("{i},0")).collect();
        assert!(
            parse_batch_parcels(&too_many)
                .unwrap_err()
                .starts_with("Too many parcels: 101")
        );
    }

    #[test]
    fn coordinates_must_be_integers_inside_genesis_city() {
        assert_eq!(validate_coords("-150", "150").unwrap(), (-150, 150));
        assert_eq!(validate_coords("160", "155").unwrap(), (160, 155));
        assert_eq!(validate_coords("163", "59").unwrap(), (163, 59));
        assert!(validate_coords("151", "0").is_err());
        assert!(validate_coords("164", "100").is_err());
        assert!(validate_coords("1.5", "0").is_err());
        assert!(validate_coords("", "0").is_err());
    }
}

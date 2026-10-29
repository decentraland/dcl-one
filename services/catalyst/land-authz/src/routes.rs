//! The lambdas land routes over the index: the operators of a parcel (what
//! comms' `COMMS_LAND_OPERATORS_URL` fetches), the reverse lookup of an
//! update operator, the per-address permission flags (single and batch) and
//! the index status. Each names its source per the module doc: the index
//! once ready, the marketplace reader's owner leg for the flags routes and
//! the land-manager subgraph (`LAND_SUBGRAPH_URL`, `marketplace::land_graph`,
//! the old handlers' second source) for the operators and the reverse lookup
//! before that, the unavailable envelope otherwise.

use std::collections::HashSet;

use axum::Json;
use axum::Router;
use axum::extract::{DefaultBodyLimit, Path, Request, State};
use axum::http::{HeaderName, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use serde_json::{Value, json};

use super::{
    Decision, DecisionSource, ParcelPermissionFlags, SharedState, Unavailable, Verdict,
    can_deploy_batch, indexer, is_address,
};
use crate::chain_index::source::{Source, unavailable};
use crate::chain_index::{ChainIndex, land_graph};
use crate::shared::http_util::{bad_request, internal_server_error, lambdas_error};
use crate::shared::query_params::{
    MAX_PAGE_SIZE, NonPositivePolicy, OversizePolicy, Pagination, parse_pagination_with,
    parse_query_string,
};
use crate::shared::{cors, nul_guard};

const PARCEL_MIN: i64 = -150;
const PARCEL_MAX: i64 = 150;
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

fn land_graph_failed(route: &'static str, error: &str) -> Response {
    tracing::warn!(route, error, "land-authz: LAND subgraph reader failed");
    unavailable(
        Source::LandSubgraph,
        "the LAND subgraph reader did not answer",
    )
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

pub(crate) fn validate_coords(x: &str, y: &str) -> Result<(i64, i64), Box<Response>> {
    let xi: i64 = x.parse().map_err(|_| bad_request(COORDS_ERR))?;
    let yi: i64 = y.parse().map_err(|_| bad_request(COORDS_ERR))?;
    if !(PARCEL_MIN..=PARCEL_MAX).contains(&xi) || !(PARCEL_MIN..=PARCEL_MAX).contains(&yi) {
        return Err(Box::new(bad_request(COORDS_ERR)));
    }
    Ok((xi, yi))
}

/// `GET /lambdas/parcels/{x}/{y}/operators`: the subject and the four legs
/// from the index; 404 for an unindexed parcel; before the index is ready
/// the land-manager subgraph, as the old route (owner-only from the
/// marketplace reader would misreport the operators); the envelope with
/// neither.
async fn parcel_operators(
    State(state): State<SharedState>,
    Path((x, y)): Path<(String, String)>,
) -> Response {
    let (xi, yi) = match validate_coords(&x, &y) {
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
            Err(e) => land_graph_failed("parcel-operators", &e),
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
            internal_server_error()
        }
    }
}

/// `GET /lambdas/users/{address}/lands-permissions`: the parcels whose own
/// update operator is `address`, paged (`pageSize` clamps to 1000, a
/// non-positive page or size to 1), `totalAmount` from the same query.
async fn user_lands_permissions(
    State(state): State<SharedState>,
    Path(addr): Path<String>,
    req: Request,
) -> Response {
    let update_operator = addr.to_lowercase();
    let params = parse_query_string(req.uri().query().unwrap_or(""));
    let p = parse_pagination_with(
        &params,
        MAX_PAGE_SIZE as i64,
        OversizePolicy::Clamp,
        NonPositivePolicy::ClampToOne,
    )
    .unwrap_or(Pagination {
        page_size: 1,
        page_num: 1,
        offset: 0,
        limit: 1,
    });
    let offset = p.page_num.saturating_sub(1).saturating_mul(p.page_size);
    if state.ready().await.is_none() {
        let Some(market) = land_graph_reader(&state) else {
            return not_ready();
        };
        return match land_graph::parcels_by_update_operator(market, &update_operator).await {
            Ok(elements) => {
                let total = elements.len() as i64;
                let page: Vec<Value> = elements
                    .iter()
                    .skip(offset.max(0) as usize)
                    .take(p.page_size.max(0) as usize)
                    .cloned()
                    .collect();
                Json(json!({ "elements": page, "totalAmount": total, "pageNum": p.page_num, "pageSize": p.page_size })).into_response()
            }
            Err(e) => land_graph_failed("lands-permissions", &e),
        };
    }
    match state
        .store()
        .parcels_with_update_operator_page(&update_operator, p.page_size, offset)
        .await
    {
        Ok((parcels, total)) => {
            let elements: Vec<Value> = parcels
                .into_iter()
                .map(|parcel| {
                    json!({
                        "id": parcel.token_id,
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
            internal_server_error()
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

fn with_source(mut resp: Response, source: &DecisionSource) -> Response {
    resp.headers_mut().insert(
        HeaderName::from_static(SOURCE_HEADER),
        HeaderValue::from_static(source.header_value()),
    );
    resp
}

/// `GET /lambdas/users/{address}/parcels/{x}/{y}/permissions`: the flags
/// [`can_deploy`](super::can_deploy) decides from, so this route cannot
/// report a right the write path would then refuse to honour; the legs the
/// fallback source cannot decide are `null` ([`flags_value`]).
async fn parcel_permissions(
    State(state): State<SharedState>,
    Path((address, x, y)): Path<(String, String, String)>,
) -> Response {
    let (xi, yi) = match validate_coords(&x, &y) {
        Ok(c) => c,
        Err(resp) => return *resp,
    };
    let addr = address.to_lowercase();
    if !is_address(&addr) {
        return bad_request("Address must be a valid Ethereum address");
    }
    match can_deploy_batch(&state, &[(xi as i32, yi as i32)], &addr).await {
        Ok(mut decisions) => {
            let Decision {
                verdict,
                source,
                flags,
            } = decisions.pop().expect("one parcel in, one decision out");
            match verdict {
                Verdict::Unknown => with_source(parcel_or_estate_not_found(xi, yi), &source),
                _ => with_source(Json(flags_value(flags, &source)).into_response(), &source),
            }
        }
        Err(why) => unavailable_response(&why),
    }
}

#[derive(Debug, serde::Deserialize)]
pub struct BatchParcelPermissionsRequest {
    pub parcels: Vec<String>,
}

/// Per-entry rule is exactly `validate_coords` (integer parse plus the
/// +/-150 range); any invalid entry rejects the whole request, naming the
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
                with_source(resp, &source)
            }
        }
        Err(why) => unavailable_response(&why),
    }
}

/// `GET /lambdas/land-authz/status`: the cursor row and the readiness rule
/// applied to it now, for operators and the tests. Always 200.
async fn status(State(state): State<SharedState>) -> Response {
    let settings = &state.settings;
    let observation = match indexer::cursor_observed(&state.pool, &settings.network).await {
        Ok(row) => row,
        Err(e) => {
            tracing::warn!(error = %e, "land-authz: cannot read index_cursor");
            return internal_server_error();
        }
    };
    let ready = observation
        .as_ref()
        .is_some_and(|o| indexer::index_ready(&o.cursor, o.observed_at.and_utc()));
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
    fn coordinates_must_be_integers_in_the_genesis_range() {
        assert_eq!(validate_coords("-150", "150").unwrap(), (-150, 150));
        assert!(validate_coords("151", "0").is_err());
        assert!(validate_coords("1.5", "0").is_err());
        assert!(validate_coords("", "0").is_err());
    }
}

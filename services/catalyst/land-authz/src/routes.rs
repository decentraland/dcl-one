//! The lambdas land routes over the index: the operators of a parcel (what
//! comms' `COMMS_LAND_OPERATORS_URL` fetches), the reverse lookup of an
//! update operator, the per-address permission flags (single and batch), the
//! Creator Hub's LAND list ([`super::lands_access`]) and the index status. Each reads the index once it is ready and answers the
//! unavailable envelope (`Source::LandAuthzIndex`) before that: no subgraph
//! stands in for it.

use std::collections::HashSet;

use axum::Json;
use axum::Router;
use axum::extract::{DefaultBodyLimit, Path, Request, State};
use axum::http::{HeaderName, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use serde_json::{Value, json};

use super::{
    LAMBDAS_FOLD_WINDOW, NOT_READY_MSG, SharedState, Unavailable, Verdict, can_deploy_batch,
    indexer, is_address,
};
use crate::chain_index::source::{Source, unavailable};
use crate::shared::casts::{index_usize, len_i64};
use crate::shared::http_util::{bad_request, internal_server_error, lambdas_error};
use crate::shared::parcel::is_inside_world_limits;
use crate::shared::query_params::{JsPagination, js_parse_int, parse_query_string};
use crate::shared::{cors, nul_guard};

const COORDS_ERR: &str = "Coordinates X and Y must be valid numbers in a valid range";
/// One footprint's worth of `"x,y"` parcels; a request above this answers 400,
/// never a truncated batch.
pub const MAX_BATCH_PARCELS: usize = 100;
pub const SOURCE_HEADER: &str = "x-land-authz-source";

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
        .route(
            "/lambdas/users/{address}/lands-access",
            post(user_lands_access).layer(DefaultBodyLimit::max(256 * 1024)),
        )
        .route("/lambdas/land-authz/status", get(status))
        .layer(axum::middleware::from_fn(nul_guard::nul_guard_middleware))
        .layer(axum::middleware::from_fn(cors::lambdas_cors))
        .with_state(state)
}

fn parcel_or_estate_not_found(x: i32, y: i32) -> Response {
    lambdas_error(
        StatusCode::NOT_FOUND,
        "Bad Request",
        &format!("Parcel or estate rights not found for x: {x}, y: {y}"),
    )
}

fn not_ready() -> Response {
    unavailable(Source::LandAuthzIndex, NOT_READY_MSG)
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
        Unavailable::NotReady => not_ready(),
        Unavailable::Index(e) => {
            tracing::warn!(error = %e, "land-authz: index read failed");
            internal_server_error()
        }
    }
}

/// A pair inside Genesis City ([`is_inside_world_limits`]) as the index's
/// `integer` coordinates.
fn world_coords(x: i64, y: i64) -> Option<(i32, i32)> {
    if !is_inside_world_limits(x, y) {
        return None;
    }
    Some((i32::try_from(x).ok()?, i32::try_from(y).ok()?))
}

/// An integer pair inside Genesis City, the parcels `Parcel.validate`
/// accepts upstream ([`is_inside_world_limits`]).
pub(crate) fn validate_coords(x: &str, y: &str) -> Result<(i32, i32), Box<Response>> {
    let xi: i64 = x.parse().map_err(|_| bad_request(COORDS_ERR))?;
    let yi: i64 = y.parse().map_err(|_| bad_request(COORDS_ERR))?;
    world_coords(xi, yi).ok_or_else(|| Box::new(bad_request(COORDS_ERR)))
}

/// The single-parcel routes' coordinates as lamb2 reads them: `parseInt(x)`
/// (white space, a sign, a `0x` prefix; anything after the digits ignored),
/// then the same [`is_inside_world_limits`]. The batch keeps
/// [`validate_coords`].
fn validate_path_coords(x: &str, y: &str) -> Result<(i32, i32), Box<Response>> {
    match (js_parse_int(x, true), js_parse_int(y, true)) {
        (Some(xi), Some(yi)) => world_coords(xi, yi),
        _ => None,
    }
    .ok_or_else(|| Box::new(bad_request(COORDS_ERR)))
}

/// `GET /lambdas/parcels/{x}/{y}/operators`: the subject and the four legs
/// from the index; 404 for an unindexed parcel; the envelope before the
/// index is ready. A failed index read answers lamb2's bare 500.
async fn parcel_operators(
    State(state): State<SharedState>,
    Path((x, y)): Path<(String, String)>,
) -> Response {
    let (xi, yi) = match validate_path_coords(&x, &y) {
        Ok(c) => c,
        Err(resp) => return *resp,
    };
    let Some(mut snapshot) = state.ready_snapshot_within(LAMBDAS_FOLD_WINDOW).await else {
        return not_ready();
    };
    match state.store().rights_on(&mut snapshot.tx, xi, yi).await {
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
/// `totalAmount` from the same query; the envelope before the index is
/// ready.
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
    let Some(mut snapshot) = state.ready_snapshot_within(LAMBDAS_FOLD_WINDOW).await else {
        return not_ready();
    };
    let store = state.store();
    // A negative bound counts from the end of the list: its length first.
    let window = match p.sql_window() {
        Some(window) => Ok(window),
        None => store
            .parcels_with_update_operator_page_on(&mut snapshot.tx, &update_operator, 0, 0)
            .await
            .map(|(_, total)| {
                let r = p.range(index_usize(total));
                (len_i64(r.start), len_i64(r.len()))
            }),
    };
    let page = match window {
        Ok((offset, limit)) => {
            store
                .parcels_with_update_operator_page_on(
                    &mut snapshot.tx,
                    &update_operator,
                    limit,
                    offset,
                )
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

fn with_source(mut resp: Response, source: &'static str) -> Response {
    resp.headers_mut().insert(
        HeaderName::from_static(SOURCE_HEADER),
        HeaderValue::from_static(source),
    );
    resp
}

/// `GET /lambdas/users/{address}/parcels/{x}/{y}/permissions`: five
/// booleans, never `null`, from the index (the flags
/// [`can_deploy`](super::can_deploy) decides from); the envelope before it
/// is ready.
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
    let Some(mut snapshot) = state.ready_snapshot_within(LAMBDAS_FOLD_WINDOW).await else {
        return not_ready();
    };
    match state.store().rights_on(&mut snapshot.tx, xi, yi).await {
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

/// The token ids of the wallet's rentals, which the Creator Hub reads from
/// the rentals source; the body and both lists are optional.
#[derive(serde::Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct LandsAccessRequest {
    pub tenant_token_ids: Vec<String>,
    pub lessor_token_ids: Vec<String>,
}

/// A token id as the registries mint it: a decimal of at most 78 digits.
fn is_token_id(raw: &str) -> bool {
    !raw.is_empty() && raw.len() <= 78 && raw.bytes().all(|b| b.is_ascii_digit())
}

/// `POST /lambdas/users/{address}/lands-access`: the Creator Hub's LAND list
/// ([`super::lands_access`]), every list read in one snapshot of the index;
/// the envelope before it is ready.
async fn user_lands_access(
    State(state): State<SharedState>,
    Path(address): Path<String>,
    body: axum::body::Bytes,
) -> Response {
    let address = address.to_lowercase();
    if !is_address(&address) {
        return bad_request("Address must be a valid Ethereum address");
    }
    // No body is no rentals; a body is JSON whatever its content type says.
    let body: LandsAccessRequest = if body.iter().all(u8::is_ascii_whitespace) {
        LandsAccessRequest::default()
    } else {
        match serde_json::from_slice(&body) {
            Ok(body) => body,
            Err(e) => return bad_request(&format!("Invalid body: {e}")),
        }
    };
    for (field, ids) in [
        ("tenantTokenIds", &body.tenant_token_ids),
        ("lessorTokenIds", &body.lessor_token_ids),
    ] {
        if ids.len() > super::lands_access::MAX_RENTAL_TOKEN_IDS {
            return bad_request(&format!(
                "{field} holds more than {} token ids",
                super::lands_access::MAX_RENTAL_TOKEN_IDS
            ));
        }
        if let Some(bad) = ids.iter().find(|id| !is_token_id(id)) {
            return bad_request(&format!("{field}: \"{bad}\" is not a token id"));
        }
    }
    let Some(mut snapshot) = state.ready_snapshot_within(LAMBDAS_FOLD_WINDOW).await else {
        return not_ready();
    };
    match super::lands_access::lands_access_on(
        &mut snapshot.tx,
        &state.settings.land_registry,
        &state.settings.estate_registry,
        &address,
        &body.tenant_token_ids,
        &body.lessor_token_ids,
    )
    .await
    {
        Ok(access) => with_source(Json(access).into_response(), "land-authz-index"),
        Err(e) => {
            tracing::error!(error = %e, "land-authz: lands-access read failed");
            upstream_internal_error()
        }
    }
}

/// Per-entry rule is exactly `validate_coords` (integer parse plus the
/// Genesis City bounds); any invalid entry rejects the whole request, naming the
/// entry. Duplicates collapse silently to their first occurrence, so the
/// response stays one element per distinct parcel in request order.
pub(crate) fn parse_batch_parcels(entries: &[String]) -> Result<Vec<(i32, i32)>, String> {
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
    match can_deploy_batch(&state, &parcels, &addr).await {
        Ok(decisions) => {
            let source = decisions
                .first()
                .map(|d| d.source.header_value())
                .unwrap_or(super::INDEX_SOURCE);
            let elements: Vec<Value> = parcels
                .iter()
                .zip(decisions)
                .map(|(&(x, y), d)| {
                    let permissions = match d.verdict {
                        Verdict::Unknown => Value::Null,
                        _ => serde_json::to_value(d.flags).expect("flags serialize"),
                    };
                    json!({ "x": x, "y": y, "permissions": permissions })
                })
                .collect();
            let resp = Json(json!({ "elements": elements })).into_response();
            if parcels.is_empty() {
                resp
            } else {
                with_source(resp, source)
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
        indexer::observation_ready_within(
            o,
            &settings.land_registry,
            &settings.estate_registry,
            LAMBDAS_FOLD_WINDOW,
        )
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
        "readyWindowSecs": LAMBDAS_FOLD_WINDOW.num_seconds(),
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

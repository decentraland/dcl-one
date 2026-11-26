// from: credits-server@d9c6a261fb7acf3beb926e10f03d1aed33e9771b:src/controllers/handlers/admin-studios.ts
//! The studio routes: create, list, read, rename, pause or resume a studio,
//! choose the wallets that act for it, and top up or adjust its budget. The
//! reads answer [`require_read_admin`], the changes [`require_write_admin`]:
//! the operator bearer, or the ops dashboard's wallet-signed requests, as
//! credits-server's read and read-write admins. Every other refusal answers
//! `{ error, code }` with credits-server's codes ([`StudioError`]); every
//! change is audited into `ops.admin_audit` with the gate's actor (the
//! bearer's name, or the signing wallet).

use axum::Json;
use axum::Router;
use axum::body::Bytes;
use axum::extract::{OriginalUri, Path, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use serde::Deserialize;
use serde_json::{Map, Value as JsonValue, json};

use crate::credits::SharedState;
use crate::credits::admin::{normalize_address, require_read_admin, require_write_admin};
use crate::credits::studios::{
    BudgetChangeRequest, DEFAULT_PAGE_SIZE, MAX_PAGE_SIZE, StudioDetail, StudioError,
    USD_CENTS_PER_CREDIT, parse_credits_change, parse_idempotency_key, parse_name, parse_note,
    parse_studio_id,
};
use crate::shared::routing::{delete, erase, get, post};

/// The same paths in the credits router and the canonical `/shop-api` one.
pub fn router() -> Router<SharedState> {
    Router::new()
        .route(
            "/admin/studios",
            get(list_studios).post(erase(create_studio)),
        )
        .route(
            "/admin/studios/{studio_id}",
            get(get_studio).patch(erase(update_studio)),
        )
        .route("/admin/studios/{studio_id}/operators", post(add_operator))
        .route(
            "/admin/studios/{studio_id}/operators/{address}",
            delete(remove_operator),
        )
        .route("/admin/studios/{studio_id}/budget", post(record_budget))
}

/// The read gate over `route`, the path the handler serves.
async fn read_gate(
    state: &SharedState,
    headers: &HeaderMap,
    route: &str,
    original: &OriginalUri,
) -> Result<(), StudioError> {
    require_read_admin(state, headers, route, original)
        .await
        .map_err(StudioError::Refused)
}

/// The write gate over `method`, `route` and the request's `body`; answers
/// the audit actor.
async fn write_gate(
    state: &SharedState,
    headers: &HeaderMap,
    method: &str,
    route: &str,
    original: &OriginalUri,
    body: &[u8],
) -> Result<String, StudioError> {
    require_write_admin(state, headers, method, route, original, body)
        .await
        .map_err(StudioError::Refused)
}

/// A JSON object body, or credits-server's `Invalid request body`.
fn json_object(body: &Bytes) -> Result<Map<String, JsonValue>, StudioError> {
    match serde_json::from_slice(body) {
        Ok(JsonValue::Object(map)) => Ok(map),
        _ => Err(StudioError::Invalid("Invalid request body".into())),
    }
}

/// `GET /admin/studios`: every studio with its budget figures.
async fn list_studios(
    State(state): State<SharedState>,
    original: OriginalUri,
    headers: HeaderMap,
) -> Result<Json<JsonValue>, StudioError> {
    read_gate(&state, &headers, "/admin/studios", &original).await?;
    Ok(Json(json!({ "studios": state.credits.studios().await? })))
}

/// `POST /admin/studios`: creates an active studio with an empty budget.
/// Body: `{ name }`.
async fn create_studio(
    State(state): State<SharedState>,
    original: OriginalUri,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Response, StudioError> {
    let actor = write_gate(&state, &headers, "post", "/admin/studios", &original, &body).await?;
    let body = json_object(&body)?;
    let name = parse_name(body.get("name"))?;
    let studio = state.credits.create_studio(&name, &actor).await?;
    tracing::info!(studio_id = %studio.id, name = %studio.name, actor = %actor, "studio created");
    Ok((StatusCode::CREATED, Json(json!({ "studio": studio }))).into_response())
}

#[derive(Debug, Default, Deserialize)]
struct PageQuery {
    limit: Option<String>,
    offset: Option<String>,
}

impl PageQuery {
    /// credits-server's reading: a positive limit up to [`MAX_PAGE_SIZE`] and
    /// a non-negative offset; anything else is the default.
    fn page(&self) -> (i64, i64) {
        let number = |v: &Option<String>| v.as_deref().and_then(|v| v.trim().parse::<i64>().ok());
        let limit = number(&self.limit)
            .filter(|l| *l > 0)
            .map_or(DEFAULT_PAGE_SIZE, |l| l.min(MAX_PAGE_SIZE));
        let offset = number(&self.offset).filter(|o| *o >= 0).unwrap_or(0);
        (limit, offset)
    }
}

/// `GET /admin/studios/{studioId}`: the studio, its operators and a page of
/// its budget history, newest first. Query: `limit` (default 50, at most
/// 200) and `offset`.
async fn get_studio(
    State(state): State<SharedState>,
    Path(studio_id): Path<String>,
    Query(page): Query<PageQuery>,
    original: OriginalUri,
    headers: HeaderMap,
) -> Result<Json<JsonValue>, StudioError> {
    let route = format!("/admin/studios/{studio_id}");
    read_gate(&state, &headers, &route, &original).await?;
    let studio_id = parse_studio_id(&studio_id)?;
    let (limit, offset) = page.page();
    let StudioDetail {
        studio,
        operators,
        entries,
        total,
    } = state
        .credits
        .studio_detail(&studio_id, limit, offset)
        .await?
        .ok_or(StudioError::NotFound)?;
    Ok(Json(json!({
        "studio": studio,
        "operators": operators,
        "entries": entries,
        "page": { "limit": limit, "offset": offset, "total": total },
        // The largest grant a client may send; null while studios cannot grant.
        "grantLimits": { "maxGrantCents": null },
    })))
}

/// `PATCH /admin/studios/{studioId}`: renames, pauses or resumes a studio.
/// Body: any of `{ name, status: "active" | "paused" }`.
async fn update_studio(
    State(state): State<SharedState>,
    Path(studio_id): Path<String>,
    original: OriginalUri,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Json<JsonValue>, StudioError> {
    let actor = write_gate(
        &state,
        &headers,
        "patch",
        &format!("/admin/studios/{studio_id}"),
        &original,
        &body,
    )
    .await?;
    let studio_id = parse_studio_id(&studio_id)?;
    let body = json_object(&body)?;
    let name = match body.get("name") {
        None => None,
        name => Some(parse_name(name)?),
    };
    let status = match body.get("status") {
        None => None,
        Some(JsonValue::String(s)) if s == "active" || s == "paused" => Some(s.as_str()),
        Some(_) => {
            return Err(StudioError::Invalid(
                "\"status\" must be \"active\" or \"paused\"".into(),
            ));
        }
    };
    if name.is_none() && status.is_none() {
        return Err(StudioError::Invalid("Nothing to update".into()));
    }
    let studio = state
        .credits
        .update_studio(&studio_id, name.as_deref(), status, &actor)
        .await?;
    tracing::info!(studio_id = %studio_id, actor = %actor, renamed = name.is_some(), status = ?status, "studio updated");
    Ok(Json(json!({ "studio": studio })))
}

/// `POST /admin/studios/{studioId}/operators`: lets a wallet act for the
/// studio. Body: `{ address }`. 201 when added, 200 when it already did.
async fn add_operator(
    State(state): State<SharedState>,
    Path(studio_id): Path<String>,
    original: OriginalUri,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Response, StudioError> {
    let actor = write_gate(
        &state,
        &headers,
        "post",
        &format!("/admin/studios/{studio_id}/operators"),
        &original,
        &body,
    )
    .await?;
    let studio_id = parse_studio_id(&studio_id)?;
    let body = json_object(&body)?;
    let address = body
        .get("address")
        .and_then(JsonValue::as_str)
        .and_then(|a| normalize_address(a).ok())
        .ok_or_else(|| StudioError::Invalid("\"address\" must be a wallet address".into()))?;
    let added = state
        .credits
        .add_studio_operator(&studio_id, &address, &actor)
        .await?;
    tracing::info!(studio_id = %studio_id, address = %address, actor = %actor, already_present = !added, "studio operator added");
    let status = if added {
        StatusCode::CREATED
    } else {
        StatusCode::OK
    };
    Ok((
        status,
        Json(json!({ "studioId": studio_id, "address": address, "added": added })),
    )
        .into_response())
}

/// `DELETE /admin/studios/{studioId}/operators/{address}`: stops a wallet
/// acting for the studio. An unknown studio is `STUDIO_NOT_FOUND`; a wallet
/// that did not act for it, `OPERATOR_NOT_FOUND`.
async fn remove_operator(
    State(state): State<SharedState>,
    Path((studio_id, address)): Path<(String, String)>,
    original: OriginalUri,
    headers: HeaderMap,
) -> Result<Json<JsonValue>, StudioError> {
    let actor = write_gate(
        &state,
        &headers,
        "delete",
        &format!("/admin/studios/{studio_id}/operators/{address}"),
        &original,
        &[],
    )
    .await?;
    let studio_id = parse_studio_id(&studio_id)?;
    let address = normalize_address(&address)
        .map_err(|_| StudioError::Invalid("Invalid wallet address".into()))?;
    if !state
        .credits
        .remove_studio_operator(&studio_id, &address, &actor)
        .await?
    {
        return Err(StudioError::OperatorNotFound);
    }
    tracing::info!(studio_id = %studio_id, address = %address, actor = %actor, "studio operator removed");
    Ok(Json(
        json!({ "studioId": studio_id, "address": address, "removed": true }),
    ))
}

/// `POST /admin/studios/{studioId}/budget`: tops up or adjusts a studio's
/// budget. Body: `{ kind: "deposit" | "adjustment", credits, note?,
/// idempotencyKey }`. A deposit is positive; an adjustment goes either way,
/// never below a zero balance, and says why. New budget counts against the
/// day's ceiling (`CREDITS_DAILY_STUDIO_DEPOSIT_LIMIT_CENTS`).
///
/// 201 with the new entry, or 200 with `replayed: true` and the entry the
/// same key already wrote.
async fn record_budget(
    State(state): State<SharedState>,
    Path(studio_id): Path<String>,
    original: OriginalUri,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Response, StudioError> {
    let actor = write_gate(
        &state,
        &headers,
        "post",
        &format!("/admin/studios/{studio_id}/budget"),
        &original,
        &body,
    )
    .await?;
    let studio_id = parse_studio_id(&studio_id)?;
    let body = json_object(&body)?;
    let kind = match body.get("kind").and_then(JsonValue::as_str) {
        Some(kind @ ("deposit" | "adjustment")) => kind,
        _ => {
            return Err(StudioError::Invalid(
                "\"kind\" must be \"deposit\" or \"adjustment\"".into(),
            ));
        }
    };
    let credits = parse_credits_change(body.get("credits"))?;
    if kind == "deposit" && credits < 0 {
        return Err(StudioError::Invalid(
            "A deposit must be positive; use an adjustment to take budget away".into(),
        ));
    }
    let note = parse_note(body.get("note"))?;
    if kind == "adjustment" && note.is_none() {
        // An adjustment rewrites what the studio was given, so it says why.
        return Err(StudioError::Invalid(
            "An adjustment needs a \"note\" saying why".into(),
        ));
    }
    let idempotency_key = parse_idempotency_key(body.get("idempotencyKey"))?;
    let usd_cents = credits * USD_CENTS_PER_CREDIT;
    let change = state
        .credits
        .record_studio_budget_change(BudgetChangeRequest {
            studio_id: &studio_id,
            kind,
            usd_cents,
            note: note.as_deref(),
            idempotency_key: &idempotency_key,
            daily_deposit_limit_cents: state.settings.daily_studio_deposit_limit_cents,
            actor: &actor,
        })
        .await?;
    let out = json!({
        "entryId": change.entry_id,
        "studioId": studio_id,
        "kind": kind,
        "usdCents": usd_cents,
        "balanceCents": change.balance_cents,
        "replayed": change.replayed,
    });
    if change.replayed {
        tracing::info!(studio_id = %studio_id, entry_id = %change.entry_id, actor = %actor, "studio budget change replayed");
        return Ok((StatusCode::OK, Json(out)).into_response());
    }
    tracing::info!(
        studio_id = %studio_id,
        studio_name = %change.studio_name,
        kind,
        usd_cents,
        balance_cents = change.balance_cents,
        note = ?note,
        actor = %actor,
        entry_id = %change.entry_id,
        "studio budget changed"
    );
    Ok((StatusCode::CREATED, Json(out)).into_response())
}

#[cfg(test)]
mod tests {
    use super::PageQuery;

    #[test]
    fn a_history_page_reads_its_query_as_credits_server_does() {
        let page = |limit: Option<&str>, offset: Option<&str>| {
            PageQuery {
                limit: limit.map(str::to_string),
                offset: offset.map(str::to_string),
            }
            .page()
        };
        assert_eq!(page(None, None), (50, 0));
        assert_eq!(page(Some("20"), Some("40")), (20, 40));
        assert_eq!(page(Some("500"), None), (200, 0));
        assert_eq!(page(Some("0"), Some("-1")), (50, 0));
        assert_eq!(page(Some("abc"), Some("x")), (50, 0));
    }
}

//! `/v1/thirdParties`: the providers a signer manages, one provider, its
//! free item slots, and the removal and update of a virtual third party,
//! with builder-server's bodies and refusals. Each reads the registry with
//! no connection held; a write locks the provider and decides again inside
//! its transaction.

use crate::kept::Since;
use axum::Json;
use axum::extract::{Path, State};
use serde_json::{Map, Value, json};

use crate::SharedState;
use crate::access;
use crate::auth::Signed;
use crate::http::{ApiData, ApiError};
use crate::third_parties::{
    has_manager, lock_provider, managed_by, provider, provider_view, virtual_by_id, virtual_view,
};

pub(crate) const ID_MAX: usize = 512;

fn missing(id: &str) -> ApiError {
    ApiError::not_found("The Third Party doesn't exists.").with_data(json!({ "id": id }))
}

fn not_its_manager(id: &str) -> ApiError {
    ApiError::unauthorized("You are not the manager of this Third Party.")
        .with_data(json!({ "id": id }))
}

/// `GET /v1/thirdParties` (`getThirdParties`): the providers the signer
/// manages, registry records first; a `manager` query parameter is ignored.
pub async fn list(
    State(state): State<SharedState>,
    Signed(signer): Signed,
) -> Result<Json<ApiData<Vec<Value>>>, ApiError> {
    let registry = state.third_parties.registry(&state, Since::read()).await?;
    let mut conn = state.items.pool().acquire().await?;
    let listed = managed_by(&mut conn, &registry, signer.as_str()).await?;
    Ok(Json(ApiData::ok(
        listed.into_iter().map(|l| l.view).collect(),
    )))
}

/// `GET /v1/thirdParties/:id` (`getThirdParty`): the registry's record to
/// any signer; a virtual third party only to its managers, 404 to anyone
/// else, as for an unknown provider.
pub async fn get(
    State(state): State<SharedState>,
    Signed(signer): Signed,
    Path(id): Path<String>,
) -> Result<Json<ApiData<Value>>, ApiError> {
    if id.len() > ID_MAX {
        return Err(missing(&id));
    }
    let registry = state.third_parties.registry(&state, Since::read()).await?;
    if let Some(record) = provider(&registry, &id) {
        return Ok(Json(ApiData::ok(provider_view(record))));
    }
    let mut conn = state.items.pool().acquire().await?;
    match virtual_by_id(&mut conn, &id).await? {
        Some(v) if has_manager(&v.managers, signer.as_str()) => {
            Ok(Json(ApiData::ok(virtual_view(&v))))
        }
        _ => Err(missing(&id)),
    }
}

/// `GET /v1/thirdParties/:id/slots` (`getThirdPartyAvailableSlots`): the
/// provider's `maxItems` (the registry's, or `0` for a virtual third party)
/// less its items submitted for curation; 0 when that can't be read.
pub async fn slots(
    State(state): State<SharedState>,
    Signed(signer): Signed,
    Path(id): Path<String>,
) -> Result<Json<ApiData<Value>>, ApiError> {
    if id.len() > ID_MAX {
        return Err(ApiError::unauthorized(
            "Unauthorized access. Account is not manager of the collection",
        )
        .with_data(json!({ "eth_address": signer.as_str() })));
    }
    let grant = access::manage_provider(&state, &signer, &id).await?;
    let max_items = provider(grant.registry(), grant.id())
        .map_or("0", |p| p.max_items.as_str())
        .parse::<i128>()
        .ok();
    let submitted: Option<i64> = sqlx::query_scalar(sqlx::AssertSqlSafe(format!(
        "SELECT count(DISTINCT i.id) FROM builder.collections c JOIN builder.items i ON i.collection_id = c.id \
         WHERE c.third_party_id = $1 AND {}",
        crate::curation::ITEM_CURATED
    )))
    .bind(grant.id())
    .fetch_one(state.items.pool())
    .await
    .ok();
    let free = match (max_items, submitted) {
        (Some(max), Some(submitted)) => max - i128::from(submitted),
        _ => 0,
    };
    Ok(Json(ApiData::ok(match i64::try_from(free) {
        Ok(free) => json!(free),
        Err(_) => json!(free as f64),
    })))
}

/// `DELETE /v1/thirdParties/:id` (`removeVirtualThirdParty`): a manager of
/// the virtual third party that also manages the provider
/// ([`access::changes_virtual_third_party`]) removes it once the registry
/// has the provider;
/// a missing one is 404, another signer 401, a provider the registry
/// doesn't have yet 400.
pub async fn delete(
    State(state): State<SharedState>,
    Signed(signer): Signed,
    Path(id): Path<String>,
) -> Result<Json<Value>, ApiError> {
    if id.len() > ID_MAX {
        return Err(missing(&id));
    }
    let since = Since::write();
    {
        let mut conn = state.items.pool().acquire().await?;
        match virtual_by_id(&mut conn, &id).await? {
            None => return Err(missing(&id)),
            Some(v) if !has_manager(&v.managers, signer.as_str()) => {
                return Err(not_its_manager(&id));
            }
            Some(_) => {}
        }
    }
    let registry = state.third_parties.registry(&state, since).await?;
    let decide = |found: Option<crate::third_parties::Virtual>| match found {
        None => Err(missing(&id)),
        Some(v)
            if !access::changes_virtual_third_party(
                &registry,
                &id,
                &v.managers,
                signer.as_str(),
            ) =>
        {
            Err(not_its_manager(&id))
        }
        Some(_) => Ok(()),
    };
    let mut tx = state.items.pool().begin().await?;
    lock_provider(&mut tx, &id).await?;
    decide(virtual_by_id(&mut tx, &id).await?)?;
    if provider(&registry, &id).is_none() {
        return Err(ApiError::bad_request(
            "The Third Party can only be deleted if it's already on the graph.",
        )
        .with_data(json!({ "id": id })));
    }
    sqlx::query("DELETE FROM builder.virtual_third_parties WHERE id = $1")
        .bind(&id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(Json(json!({ "ok": true })))
}

/// `PATCH /v1/thirdParties/:id` (`updateThirdParty`): the body holds
/// `isProgrammatic`, a boolean (400 otherwise), and anything else is ignored; a manager of the virtual
/// third party that also manages the provider
/// ([`access::changes_virtual_third_party`]) sets it; a missing one is 404,
/// another signer 401, 503 while the registry can't be read.
pub async fn update(
    State(state): State<SharedState>,
    Signed(signer): Signed,
    Path(id): Path<String>,
    body: Option<Json<Value>>,
) -> Result<Json<Value>, ApiError> {
    let programmatic = body
        .as_ref()
        .and_then(|Json(body)| body.as_object())
        .and_then(|fields: &Map<String, Value>| fields.get("isProgrammatic"))
        .and_then(Value::as_bool)
        .ok_or_else(|| ApiError::bad_request("Invalid request body"))?;
    if id.len() > ID_MAX {
        return Err(missing(&id));
    }
    let registry = state.third_parties.registry(&state, Since::write()).await?;
    let mut tx = state.items.pool().begin().await?;
    lock_provider(&mut tx, &id).await?;
    match virtual_by_id(&mut tx, &id).await? {
        None => return Err(missing(&id)),
        Some(v)
            if !access::changes_virtual_third_party(
                &registry,
                &id,
                &v.managers,
                signer.as_str(),
            ) =>
        {
            return Err(not_its_manager(&id));
        }
        Some(_) => {}
    }
    sqlx::query(
        "UPDATE builder.virtual_third_parties SET is_programmatic = $2, updated_at = timezone('UTC', now()) \
         WHERE id = $1 AND is_programmatic IS DISTINCT FROM $2",
    )
    .bind(&id)
    .bind(programmatic)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(Json(json!({ "ok": true })))
}

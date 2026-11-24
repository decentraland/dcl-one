//! `POST /v1/collections/:id/publish` of a third-party collection, as
//! builder-server publishes one: a manager of the provider submits the items
//! the body names for curation with a slot cheque it signed. One transaction
//! locks the collection, then the provider (the order every write of a
//! collection and its provider takes), decides again, counts the provider's
//! free slots, checks the items and the cheque in builder-server's order, and
//! writes the cheque, a pending curation for each item and a new pending
//! curation of the collection; a refusal writes nothing. As the standard
//! publication, it refuses an item naming a file whose name's writer never
//! uploaded it. The cheque's signer
//! is recovered before the transaction. After the commit the curators are
//! told on Slack and on the forum.

use serde::Serialize;
use serde_json::{Value, json};
use uuid::Uuid;

use crate::State;
use crate::access::{self, ThirdPartyPublish};
use crate::chain_state;
use crate::cheques::Offer;
use crate::curation::rows::ItemCurationOut;
use crate::http::ApiError;
use crate::items::body::FullItemOut;
use crate::third_parties;

/// The most items one publish names: the largest third-party collection
/// builder-server holds is under it.
pub const MAX_ITEMS: usize = 20_000;

/// What the publish answers.
#[derive(Debug, Serialize)]
pub struct ThirdPartyPublished {
    pub collection: Value,
    pub items: Vec<FullItemOut>,
    #[serde(rename = "itemCurations")]
    pub item_curations: Vec<ItemCurationOut>,
}

/// A publish's body: the items it names and its cheque.
#[derive(Debug)]
pub struct Request {
    pub items: Vec<Uuid>,
    pub cheque: Offer,
}

fn truthy(value: Option<&Value>) -> Option<&Value> {
    value.filter(|v| match v {
        Value::Null => false,
        Value::Bool(b) => *b,
        Value::Number(n) => n.as_f64() != Some(0.0),
        Value::String(s) => !s.is_empty(),
        _ => true,
    })
}

/// Reads a publish's body as builder-server takes it: `itemIds`, then
/// `cheque`, each refused with HTTP 200 when missing; `itemIds` must be a
/// list of at most [`MAX_ITEMS`] item ids.
pub fn parse(body: &[u8]) -> Result<Request, ApiError> {
    let body: Value = if body.iter().all(u8::is_ascii_whitespace) {
        json!({})
    } else {
        serde_json::from_slice(body).map_err(|_| ApiError::bad_request("Invalid request body"))?
    };
    let items = truthy(body.get("itemIds"))
        .ok_or_else(|| ApiError::http(200, "Could not get itemIds from request"))?;
    let cheque = truthy(body.get("cheque"))
        .ok_or_else(|| ApiError::http(200, "Could not get cheque from request"))?;
    let items = items
        .as_array()
        .filter(|ids| ids.len() <= MAX_ITEMS)
        .and_then(|ids| {
            ids.iter()
                .map(|id| id.as_str().and_then(|id| Uuid::parse_str(id).ok()))
                .collect::<Option<Vec<_>>>()
        })
        .ok_or_else(|| ApiError::bad_request("Invalid request body"))?;
    Ok(Request {
        items,
        cheque: Offer::parse(cheque),
    })
}

fn refused(status: u16, message: &str, id: Uuid) -> ApiError {
    ApiError::http(status, message).with_data(json!({ "id": id }))
}

/// The provider's free slots: its registry `maxItems` (none for a provider
/// the registry doesn't list) less its items ever submitted for curation.
async fn free_slots(
    conn: &mut sqlx::PgConnection,
    grant: &ThirdPartyPublish,
) -> Result<i128, ApiError> {
    let max_items = third_parties::provider(grant.registry(), grant.provider())
        .and_then(|p| p.max_items.parse::<i128>().ok())
        .unwrap_or(0);
    let submitted: i64 = sqlx::query_scalar(sqlx::AssertSqlSafe(format!(
        "SELECT count(DISTINCT i.id) FROM builder.collections c JOIN builder.items i ON i.collection_id = c.id \
         WHERE c.third_party_id = $1 AND {}",
        crate::curation::ITEM_CURATED
    )))
    .bind(grant.provider())
    .fetch_one(conn)
    .await?;
    Ok(max_items - i128::from(submitted))
}

#[derive(sqlx::FromRow)]
struct Named {
    id: Uuid,
    name: String,
    collection_id: Option<Uuid>,
    missing_content: bool,
    local_content_hash: Option<String>,
    mapped: bool,
}

/// Publishes `request`'s items under `grant`.
pub async fn publish(
    state: &State,
    grant: &ThirdPartyPublish,
    request: &Request,
) -> Result<ThirdPartyPublished, ApiError> {
    let id = grant.id();
    let signer = request
        .cheque
        .signer(&state.settings.network, grant.provider());
    let stored = request.cheque.stored();
    let mut tx = state.items.pool().begin().await?;
    sqlx::query("SELECT id FROM builder.collections WHERE id = $1 FOR UPDATE")
        .bind(id)
        .fetch_optional(&mut *tx)
        .await?;
    third_parties::lock_provider(&mut tx, grant.provider()).await?;
    let view = chain_state::for_decision(&mut tx).await?;
    access::confirm_third_party_publish(&mut tx, &view, grant).await?;
    if request.items.len() as i128 > free_slots(&mut tx, grant).await? {
        return Err(refused(
            409,
            "The amount of items to publish exceeds the available slots.",
            id,
        ));
    }
    let named: Vec<Named> = sqlx::query_as(
        "SELECT i.id, i.name, i.collection_id, i.local_content_hash, i.mappings IS NOT NULL AS mapped, \
           EXISTS (SELECT 1 FROM builder.item_contents ic WHERE ic.item_id = i.id AND NOT EXISTS ( \
             SELECT 1 FROM builder.uploaded_contents u WHERE u.hash = ic.hash AND u.uploaded_by = ic.named_by \
             AND (u.video OR ic.file <> 'video.mp4'))) \
           AS missing_content \
         FROM builder.items i WHERE i.id = ANY($1) ORDER BY i.created_at, i.id FOR UPDATE OF i",
    )
    .bind(&request.items)
    .fetch_all(&mut *tx)
    .await?;
    if named.is_empty() {
        return Err(refused(400, "Tried to publish no TP items", id));
    }
    if request.cheque.qty.and_then(|q| usize::try_from(q).ok()) != Some(named.len()) {
        return Err(refused(
            400,
            "The check quantity is different from the amount of published items",
            id,
        ));
    }
    let invalid = |why: &str| {
        refused(
            400,
            &format!(
                "Tried to publish TP items with an invalid signed message or signature. Error: {why}"
            ),
            id,
        )
    };
    match &signer {
        Err(why) => return Err(invalid(why)),
        Ok(recovered) if recovered != grant.signer() => return Err(invalid("Address missmatch")),
        Ok(_) => {}
    }
    if named.iter().any(|item| item.collection_id != Some(id)) {
        return Err(refused(
            400,
            "Cannot publish items that belong to different collections",
            id,
        ));
    }
    let pending: bool = sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM builder.item_curations k JOIN builder.items i ON i.id = k.item_id \
         WHERE i.collection_id = $1 AND k.status = 'pending')",
    )
    .bind(id)
    .fetch_one(&mut *tx)
    .await?;
    if pending {
        return Err(refused(
            409,
            "The third party collection already has published items. It can't be updated.",
            id,
        ));
    }
    let named_hashes: Vec<(Uuid, String)> =
        sqlx::query_as("SELECT item_id, hash FROM builder.item_contents WHERE item_id = ANY($1)")
            .bind(&request.items)
            .fetch_all(&mut *tx)
            .await?;
    let mut held = std::collections::HashMap::new();
    let mut lost = std::collections::HashSet::new();
    for (item, hash) in named_hashes {
        let stored = match held.get(&hash) {
            Some(stored) => *stored,
            None => {
                let stored = state
                    .items
                    .blobs
                    .exist(&hash)
                    .await
                    .map_err(|e| ApiError::internal(e.to_string()))?;
                held.insert(hash, stored);
                stored
            }
        };
        if !stored {
            lost.insert(item);
        }
    }
    let mut submitted = Vec::with_capacity(named.len());
    for item in &named {
        match item
            .local_content_hash
            .as_deref()
            .filter(|h| crate::curation::content_hash(h))
        {
            Some(hash) => submitted.push((item.id, hash.to_string(), item.mapped)),
            None => {
                return Err(
                    ApiError::conflict("The item has no content hash to publish")
                        .with_data(json!({ "id": item.id })),
                );
            }
        }
        if item.missing_content || lost.contains(&item.id) {
            return Err(ApiError::bad_request(format!(
                "{}: upload missing content before publishing",
                item.name
            )));
        }
    }
    let cheque = stored.ok_or_else(|| invalid("invalid signature"))?;
    let kept: Option<Uuid> = sqlx::query_scalar(
        "INSERT INTO builder.slot_usage_cheques (id, collection_id, third_party_id, signature, salt, qty) \
         VALUES ($1, $2, $3, $4, $5, $6) ON CONFLICT (third_party_id, salt) DO NOTHING RETURNING id",
    )
    .bind(Uuid::new_v4())
    .bind(id)
    .bind(grant.provider())
    .bind(&cheque.signature)
    .bind(&cheque.salt)
    .bind(cheque.qty)
    .fetch_optional(&mut *tx)
    .await?;
    if kept.is_none() {
        return Err(refused(
            400,
            "An error occurred trying to publish: the cheque was already used",
            id,
        ));
    }
    let item_curations =
        crate::curation::record_item_curations(&mut tx, grant, &submitted, "pending").await?;
    crate::curation::record_collection_curation(&mut tx, grant, "pending", None).await?;
    crate::review::on_third_party_publish(&mut tx, state, grant.id()).await?;
    tx.commit().await?;
    let ids: Vec<Uuid> = named.iter().map(|item| item.id).collect();
    let mut collection = state.items.collection(&grant.full_read()).await?;
    if let Some(fields) = collection.as_object_mut() {
        fields.remove("is_programmatic");
    }
    Ok(ThirdPartyPublished {
        collection,
        items: crate::items::listing::bodies(
            &state.items,
            &ids,
            crate::items::body::Reached::InList,
            true,
            None,
        )
        .await?,
        item_curations,
    })
}

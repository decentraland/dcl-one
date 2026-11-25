//! A collection's lifecycle as builder-server answers it: the owner's lock,
//! the delete, the standard publish, and the check an address-changing save
//! passes. Each write is one transaction that takes the collection row
//! `FOR UPDATE`, then its items, and decides again inside it; a read of the
//! Polygon RPC ([`super::deployment`]) runs before the transaction, never
//! while it holds a connection.

use std::collections::BTreeSet;

use chrono::{DateTime, Utc};
use serde::Serialize;
use serde_json::{Value, json};
use uuid::Uuid;

use crate::State;
use crate::access::{self, CollectionEdit, CollectionLock, CollectionManage, CollectionSync};
use crate::chain_state;
use crate::collections::deployment::Onchain;
use crate::collections::listing::ListQuery;
use crate::curation::slack::Notice;
use crate::http::{ApiError, Subject};
use crate::items::listing::ItemsOut;
use crate::items::save::forget_unnamed;
use crate::limits::Listing;
use crate::validator;

const CHAIN_UNKNOWN: &str = "The collection's chain state is not available. Try again later.";

/// `POST /v1/collections/:id/lock`: stores the lock at the decision's
/// instant, to the millisecond, and answers it.
pub async fn lock(state: &State, lock: &CollectionLock) -> Result<DateTime<Utc>, ApiError> {
    let at = DateTime::from_timestamp_millis(lock.at().timestamp_millis()).unwrap_or(lock.at());
    let mut tx = state.items.pool().begin().await?;
    let view = chain_state::for_decision(&mut tx).await?;
    sqlx::query("SELECT id FROM builder.collections WHERE id = $1 FOR UPDATE")
        .bind(lock.id())
        .fetch_optional(&mut *tx)
        .await?;
    access::confirm_collection_lock(&mut tx, &view, lock).await?;
    sqlx::query(
        "UPDATE builder.collections SET lock = $2, updated_at = timezone('UTC', now()) WHERE id = $1",
    )
    .bind(lock.id())
    .bind(at.naive_utc())
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(at)
}

/// Whether the RPC shows code at `contract`; an RPC that can't tell, or
/// none configured, is 503.
pub(crate) async fn deployed(state: &State, contract: &str) -> Result<bool, ApiError> {
    match state.deployments.of(state, contract).await {
        Onchain::Deployed(_) => Ok(true),
        Onchain::Absent => Ok(false),
        Onchain::Unknown | Onchain::Unconfigured => {
            Err(ApiError::service_unavailable(CHAIN_UNKNOWN))
        }
    }
}

/// A standard save that gives the stored collection another address is
/// refused when the address it holds is deployed: the row would no longer
/// name its contract.
pub async fn guard_address_change(
    state: &State,
    edit: &CollectionEdit,
    next: &str,
) -> Result<(), ApiError> {
    let stored: Option<Option<String>> = sqlx::query_scalar(
        "SELECT contract_address FROM builder.collections WHERE id = $1 AND third_party_id IS NULL",
    )
    .bind(edit.id())
    .fetch_optional(state.items.pool())
    .await?;
    let Some(Some(stored)) = stored else {
        return Ok(());
    };
    if stored != next && deployed(state, &stored).await? {
        return Err(ApiError::published(
            Subject::Collection,
            &edit.id().to_string(),
            "updated",
        ));
    }
    Ok(())
}

/// `DELETE /v1/collections/:id`: a third-party collection stays once
/// submitted and a standard one once published or deployed (409); either
/// waits while its lock holds or the node publishes it (423). The items go
/// with it, their contents and the upload records no other item names, and
/// the provider's virtual third party once no collection of the provider is
/// left (builder-server's `delete_virtual_third_party_trigger`).
pub async fn delete(state: &State, manage: &CollectionManage) -> Result<(), ApiError> {
    let id = manage.id();
    let stored: Option<(bool, Option<String>)> = sqlx::query_as(
        "SELECT third_party_id IS NOT NULL, contract_address FROM builder.collections WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(state.items.pool())
    .await?;
    let checked = match stored {
        Some((false, Some(contract))) if deployed(state, &contract).await? => Some(contract),
        _ => None,
    };
    let mut tx = state.items.pool().begin().await?;
    let view = chain_state::for_decision(&mut tx).await?;
    sqlx::query("SELECT id FROM builder.collections WHERE id = $1 FOR UPDATE")
        .bind(id)
        .fetch_optional(&mut *tx)
        .await?;
    sqlx::query("SELECT id FROM builder.items WHERE collection_id = $1 ORDER BY id FOR UPDATE")
        .bind(id)
        .fetch_all(&mut *tx)
        .await?;
    let c = access::confirm_collection_manage(&mut tx, &view, manage).await?;
    if c.third_party && c.published {
        return Err(ApiError::conflict(
            "The third party collection already has published items. It can't be deleted.",
        )
        .with_data(json!({ "id": id })));
    }
    if !c.third_party && (c.published || (checked.is_some() && checked == c.contract)) {
        return Err(ApiError::published(
            Subject::Collection,
            &id.to_string(),
            "deleted",
        ));
    }
    if c.pending || c.locked {
        return Err(ApiError::locked(
            Subject::Collection,
            &id.to_string(),
            "deleted",
        ));
    }
    let mut hashes: Vec<String> = sqlx::query_scalar(
        "DELETE FROM builder.item_contents WHERE item_id IN \
         (SELECT id FROM builder.items WHERE collection_id = $1) RETURNING hash",
    )
    .bind(id)
    .fetch_all(&mut *tx)
    .await?;
    let videos: Vec<Option<String>> =
        sqlx::query_scalar("DELETE FROM builder.items WHERE collection_id = $1 RETURNING video")
            .bind(id)
            .fetch_all(&mut *tx)
            .await?;
    hashes.extend(videos.into_iter().flatten());
    let provider: Option<String> = sqlx::query_scalar(
        "DELETE FROM builder.collections WHERE id = $1 RETURNING third_party_id",
    )
    .bind(id)
    .fetch_one(&mut *tx)
    .await?;
    if let Some(provider) = provider {
        crate::third_parties::lock_provider(&mut tx, &provider).await?;
        sqlx::query(
            "DELETE FROM builder.virtual_third_parties WHERE id = $1 \
             AND NOT EXISTS (SELECT 1 FROM builder.collections WHERE third_party_id = $1)",
        )
        .bind(&provider)
        .execute(&mut *tx)
        .await?;
    }
    forget_unnamed(&mut tx, &hashes).await?;
    tx.commit().await?;
    Ok(())
}

/// What a standard publish answers: the collection's body and its items.
#[derive(Debug, Serialize)]
pub struct Published {
    pub collection: Value,
    pub items: ItemsOut,
}

fn not_published(id: Uuid) -> ApiError {
    ApiError::unauthorized("The collection is not published.").with_data(json!({ "id": id }))
}

/// The collection's contract, whether the node publishes it itself, its
/// items in creation order with their on-chain ids, and whether the node
/// already holds it as published.
struct Snapshot {
    contract: Option<String>,
    pending: bool,
    items: Vec<(Uuid, Option<String>)>,
    published: bool,
    copy: Option<BTreeSet<u64>>,
    approved: bool,
}

async fn snapshot(conn: &mut sqlx::PgConnection, id: Uuid) -> Result<Snapshot, ApiError> {
    let view = chain_state::for_decision(conn).await?;
    let (contract, pending): (Option<String>, bool) = sqlx::query_as(
        "SELECT contract_address, publication_pending FROM builder.collections WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(&mut *conn)
    .await?
    .ok_or_else(|| not_published(id))?;
    let items: Vec<(Uuid, Option<String>)> = sqlx::query_as(
        "SELECT id, blockchain_item_id FROM builder.items WHERE collection_id = $1 ORDER BY created_at, id",
    )
    .bind(id)
    .fetch_all(&mut *conn)
    .await?;
    let published = chain_state::published(conn, &view, id).await?;
    let facts = match &contract {
        Some(contract) => chain_state::collection(conn, &view, contract).await?,
        None => None,
    };
    let copy = match &contract {
        Some(contract) if facts.is_some() => {
            let ids: Vec<String> = sqlx::query_scalar(
                "SELECT item_id::text FROM shop.items WHERE network = 'matic' AND contract_address = lower($1)",
            )
            .bind(contract)
            .fetch_all(&mut *conn)
            .await?;
            Some(ids.iter().filter_map(|id| id.parse().ok()).collect())
        }
        _ => None,
    };
    Ok(Snapshot {
        contract,
        pending,
        items,
        published,
        copy,
        approved: facts.is_some_and(|facts| facts.is_approved),
    })
}

/// `POST /v1/collections/:id/publish` for a standard collection: once the
/// chain holds the collection every item is given the on-chain id of its
/// place in creation order, which the client deployed it at. The copy
/// tells when it holds the collection and an item at every place; else the
/// Polygon RPC's item count at the stored address does. 401 while neither
/// can tell (the client retries), 409 for a place at or past the chain's
/// count. A collection whose items all hold their ids answers as it is.
/// Its items are answered as their listing answers `listing`'s reader, after
/// the ids are kept and the notices sent.
pub async fn publish(
    state: &State,
    sync: &CollectionSync,
    listing: &Listing<'_>,
) -> Result<Published, ApiError> {
    let id = sync.id();
    let before = {
        let mut conn = state.items.pool().acquire().await?;
        snapshot(&mut conn, id).await?
    };
    if before.pending {
        return Err(not_published(id));
    }
    let contract = before.contract.clone().ok_or_else(|| not_published(id))?;
    let missing = before.items.iter().any(|(_, chain_id)| chain_id.is_none());
    let in_copy = |place: u64| before.copy.as_ref().is_some_and(|ids| ids.contains(&place));
    let copy_tells = (0..before.items.len() as u64).all(in_copy);
    let chain_count = if (missing && !copy_tells) || (before.copy.is_none() && !before.published) {
        match state.deployments.of(state, &contract).await {
            Onchain::Deployed(items) => Some(items),
            _ => return Err(not_published(id)),
        }
    } else {
        None
    };
    let mut first_publication = false;
    let mut validation = None;
    if missing {
        for (place, (item, _)) in before.items.iter().enumerate() {
            let place = place as u64;
            if !in_copy(place) && chain_count.is_none_or(|items| place >= items) {
                return Err(ApiError::conflict("The item is not published.")
                    .with_data(json!({ "id": item })));
            }
        }
        let mut tx = state.items.pool().begin().await?;
        sqlx::query("SELECT id FROM builder.collections WHERE id = $1 FOR UPDATE")
            .bind(id)
            .fetch_optional(&mut *tx)
            .await?;
        sqlx::query("SELECT id FROM builder.items WHERE collection_id = $1 ORDER BY id FOR UPDATE")
            .bind(id)
            .fetch_all(&mut *tx)
            .await?;
        let view = chain_state::for_decision(&mut tx).await?;
        access::confirm_collection_sync(&mut tx, &view, state, sync).await?;
        let now = snapshot(&mut tx, id).await?;
        let same_items = now
            .items
            .iter()
            .map(|(item, _)| item)
            .eq(before.items.iter().map(|(item, _)| item));
        if now.pending || now.contract.as_deref() != Some(contract.as_str()) || !same_items {
            return Err(not_published(id));
        }
        let first = now.items.iter().any(|(_, chain_id)| chain_id.is_none())
            && sqlx::query_scalar(
                "SELECT NOT EXISTS (SELECT 1 FROM builder.collection_curations WHERE collection_id = $1)",
            )
            .bind(id)
            .fetch_one(&mut *tx)
            .await?;
        let items: Vec<Uuid> = now.items.iter().map(|(item, _)| *item).collect();
        sqlx::query(
            "UPDATE builder.items i SET blockchain_item_id = (p.place - 1)::text, is_published = true, \
             urn_suffix = $2 || (p.place - 1), updated_at = timezone('UTC', now()) \
             FROM unnest($1::uuid[]) WITH ORDINALITY AS p(id, place) WHERE i.id = p.id",
        )
        .bind(&items)
        .bind(state.settings.network.item_urn(&contract, ""))
        .execute(&mut *tx)
        .await?;
        if first && !now.approved {
            validation = validator::on_publish(&mut tx, state, id, &items, sync.signer()).await?;
        }
        tx.commit().await?;
        first_publication = first;
    }
    let read = sync.full_read();
    let collection = state.items.collection(&read).await?;
    let name = collection["name"].as_str().unwrap_or_default();
    if first_publication && collection.get("is_approved") != Some(&Value::Bool(true)) {
        state.notices.send(
            &state.guarded,
            id,
            name,
            sync.signer(),
            Notice::Published {
                items: before.items.len(),
            },
        );
    }
    if let Some(notice) = validation {
        state
            .notices
            .send(&state.guarded, id, name, sync.signer(), notice);
    }
    let items = state
        .items
        .collection_items(&read, &ListQuery::parse(None), Some(listing))
        .await?;
    Ok(Published { collection, items })
}

//! `PUT /v1/items/:idOrURN` as builder-server's `upsertItem` answers it: the
//! path names the item by its id or, for a third-party item, by its URN;
//! [`prepare`] checks the body against the path and resolves the item, and
//! [`ItemsComponent::save_item`] writes it in one transaction that locks the
//! item's collections and the item, decides again
//! ([`access::confirm_item_save`]) and writes the row, its contents in the
//! order saved and the content hash a published item carries.
//!
//! A key the body leaves out keeps the stored value of an existing item
//! (`thumbnail`, `video`, `price`, `beneficiary`, `rarity`, `utility`,
//! `mappings`); `collection_id` left out keeps the item where it is. An item
//! may name any content hash; naming one grants nothing over its bytes
//! ([`super::upload`] records who uploaded what). The `video` is a name too:
//! `video_named_by` keeps the wallet whose save wrote it (the writer kept
//! while the value is, else the writer of the item's own entry for that hash,
//! else the saving signer). Content an item stops naming, in its contents or
//! as its video, by a save or a delete, loses its upload records once no item
//! names it, so the blob collector can take the bytes.

use sqlx::{Postgres, Transaction};
use uuid::Uuid;

use crate::SharedState;
use crate::access::{self, ItemChange, ItemSave, SaveIntent, SavePlan, UrnField};
use crate::chain_state;
use crate::content::JsJson;
use crate::http::{ApiError, Subject};

use super::ItemsComponent;
use super::body::{FullItemOut, Reached};
use super::hash::{self, Hashed};
use super::listing;
use super::schema::{ItemInput, ItemType};
use super::urn::ThirdPartyItemUrn;

pub(crate) const MAX_TAGS: usize = 20;
pub(crate) const MAX_OUTCOMES: usize = 10;

/// The item a save writes: its id and what the decision reads of the body.
#[derive(Debug)]
pub struct Target {
    pub id: Uuid,
    pub intent: SaveIntent,
}

fn array_len(value: Option<&JsJson>) -> Option<usize> {
    value.and_then(JsJson::as_array).map(<[JsJson]>::len)
}

fn representations_named(input: &ItemInput) -> bool {
    let files: Vec<&str> = input.contents.iter().map(|(f, _)| f.as_str()).collect();
    input
        .data
        .get("representations")
        .and_then(JsJson::as_array)
        .unwrap_or_default()
        .iter()
        .all(|representation| {
            let names: Vec<&str> = representation
                .get("contents")
                .and_then(JsJson::as_array)
                .unwrap_or_default()
                .iter()
                .filter_map(JsJson::as_str)
                .collect();
            representation
                .get("mainFile")
                .and_then(JsJson::as_str)
                .is_some_and(|main| names.contains(&main))
                && names.iter().all(|name| files.contains(name))
        })
}

fn not_matching(path: &str, body: Option<Option<&str>>) -> ApiError {
    let mut data = serde_json::json!({ "urlId": path });
    if let Some(body) = body {
        data["bodyId"] = body.into();
    }
    ApiError::bad_request("The body and URL item id or urn do not match").with_data(data)
}

/// The path against the body, builder-server's checks before its service,
/// and the item they name: the id the body names, or the stored item of the
/// URN the path names. Refusals in upstream's order: the path and the body
/// disagree (400); a representation names a file the contents lack (400);
/// more than 20 tags on a new item, or more on an item already over, and
/// more than 10 emote outcomes (200 `ok:false`); a URN no item of the
/// provider's collections holds (404).
pub async fn prepare(
    state: &SharedState,
    path: &str,
    input: &ItemInput,
) -> Result<Target, ApiError> {
    let path_urn = ThirdPartyItemUrn::parse(path);
    let body_urn = input.urn.clone().flatten();
    match &path_urn {
        Some(_) if body_urn.as_deref() != Some(path) => {
            return Err(not_matching(path, input.urn.as_ref().map(Option::as_deref)));
        }
        None if input.id.as_deref() != Some(path) => {
            return Err(not_matching(path, input.id.as_deref().map(Some)));
        }
        _ => {}
    }
    if !representations_named(input) {
        return Err(ApiError::bad_request(
            "Representation files must be part of the item's content",
        )
        .with_data(serde_json::json!({ "id": path })));
    }
    let path_names_id = path_urn.is_none();
    let by_urn = input.id.is_none().then_some(path_urn).flatten();
    let pool = state.items.pool();
    let stored_id: Option<Uuid> = match (&by_urn, &input.id) {
        (Some(urn), _) => sqlx::query_scalar(
            "SELECT i.id FROM builder.items i JOIN builder.collections c ON c.id = i.collection_id \
             WHERE c.third_party_id = $1 AND i.urn_suffix = c.urn_suffix || ':' || $2 \
             ORDER BY i.created_at, i.id LIMIT 1",
        )
        .bind(&urn.third_party_id)
        .bind(&urn.item)
        .fetch_optional(pool)
        .await?,
        (None, Some(id)) => {
            let id =
                Uuid::parse_str(id).map_err(|_| ApiError::bad_request("Invalid request body"))?;
            sqlx::query_scalar("SELECT id FROM builder.items WHERE id = $1")
                .bind(id)
                .fetch_optional(pool)
                .await?
        }
        (None, None) => None,
    };
    let stored: Option<(Option<Uuid>, Option<i32>)> = match stored_id {
        Some(id) => {
            sqlx::query_as(
                "SELECT collection_id, CASE WHEN json_typeof(data->'tags') = 'array' \
             THEN json_array_length(data->'tags') END FROM builder.items WHERE id = $1",
            )
            .bind(id)
            .fetch_optional(pool)
            .await?
        }
        None => None,
    };
    let tags = array_len(input.data.get("tags")).unwrap_or(0);
    if tags > MAX_TAGS {
        let stored_tags = stored
            .as_ref()
            .and_then(|(_, t)| *t)
            .and_then(|t| usize::try_from(t).ok())
            .unwrap_or(0);
        if stored.is_none() || (stored_tags > MAX_TAGS && tags > stored_tags) {
            return Err(ApiError::thrown_without_status(format!(
                "You hace exceeded the maximun amount of tags allowed ({MAX_TAGS})."
            )));
        }
    }
    let social = input.data.get("startAnimation").is_some() && input.data.get("outcomes").is_some();
    if input.item_type == ItemType::Emote
        && social
        && array_len(input.data.get("outcomes")).unwrap_or(0) > MAX_OUTCOMES
    {
        return Err(ApiError::thrown_without_status(format!(
            "You have exceeded the maximum amount of outcomes allowed ({MAX_OUTCOMES})."
        )));
    }
    let id = match (by_urn, stored_id, &input.id) {
        (Some(_), None, _) => {
            return Err(
                ApiError::not_found("The third party item can not be created by URN.")
                    .with_data(serde_json::json!({ "urn": path })),
            );
        }
        (_, Some(id), _) => id,
        (None, None, Some(id)) => {
            Uuid::parse_str(id).map_err(|_| ApiError::bad_request("Invalid request body"))?
        }
        (None, None, None) => return Err(ApiError::bad_request("Invalid request body")),
    };
    let collection = match &input.collection_id {
        Some(Some(c)) => {
            Some(Uuid::parse_str(c).map_err(|_| ApiError::bad_request("Invalid request body"))?)
        }
        Some(None) => None,
        None => stored.and_then(|(c, _)| c),
    };
    let urn = match &input.urn {
        None => UrnField::Absent,
        Some(None) => UrnField::Null,
        Some(Some(urn)) => {
            ThirdPartyItemUrn::parse(urn).map_or(UrnField::Other, UrnField::ThirdParty)
        }
    };
    Ok(Target {
        id,
        intent: SaveIntent {
            collection,
            rarity: input.rarity.clone().flatten(),
            urn,
            id_given: input.id.is_some(),
            path_id: path_names_id.then(|| path.to_string()),
        },
    })
}

/// The item's id (an advisory lock, so two requests that create one new
/// item queue and the second decides on the first's row), its collections
/// (the one it is in and the one it moves to), then the item, `FOR UPDATE`:
/// publication's parent-then-item order. 409 when another request moved the
/// item in between.
pub(crate) async fn lock_item(
    tx: &mut Transaction<'_, Postgres>,
    id: Uuid,
    next_collection: Option<Uuid>,
) -> Result<(), ApiError> {
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended('builder item ' || $1::text, 0))")
        .bind(id)
        .execute(&mut **tx)
        .await?;
    let previous: Option<Option<Uuid>> =
        sqlx::query_scalar("SELECT collection_id FROM builder.items WHERE id=$1")
            .bind(id)
            .fetch_optional(&mut **tx)
            .await?;
    let previous_collection = previous.flatten();
    let mut parents: Vec<_> = previous_collection
        .into_iter()
        .chain(next_collection)
        .collect();
    parents.sort();
    parents.dedup();
    for parent in parents {
        sqlx::query("SELECT id FROM builder.collections WHERE id=$1 FOR UPDATE")
            .bind(parent)
            .fetch_optional(&mut **tx)
            .await?;
    }
    let current: Option<Option<Uuid>> =
        sqlx::query_scalar("SELECT collection_id FROM builder.items WHERE id=$1 FOR UPDATE")
            .bind(id)
            .fetch_optional(&mut **tx)
            .await?;
    if current.is_some_and(|parent| parent != previous_collection) {
        return Err(ApiError::conflict(
            "Item moved to another collection. Reload it before saving.",
        ));
    }
    Ok(())
}

/// A save's contents, each name attributed to the wallet that wrote it: the
/// writer of the same file and hash before the save, else of the same hash
/// under another file (one holding an upload record for it first), else the
/// saving signer `$7`.
const NAMED_CONTENTS: &str = "INSERT INTO builder.item_contents (item_id, file, hash, position, named_by) \
     SELECT $1, t.f, t.h, (t.n - 1)::int, COALESCE( \
       (SELECT p.w FROM unnest($4::text[], $5::text[], $6::text[]) AS p(f, h, w) WHERE p.f = t.f AND p.h = t.h), \
       (SELECT p.w FROM unnest($4::text[], $5::text[], $6::text[]) AS p(f, h, w) WHERE p.h = t.h \
          ORDER BY EXISTS (SELECT 1 FROM builder.uploaded_contents u WHERE u.hash = p.h AND u.uploaded_by = p.w) DESC, p.w \
          LIMIT 1), \
       $7) \
     FROM unnest($2::text[], $3::text[]) WITH ORDINALITY AS t(f, h, n)";

/// Drops the upload records of `hashes` that no item names any more, in its
/// contents or as its video.
pub(crate) async fn forget_unnamed(
    conn: &mut sqlx::PgConnection,
    hashes: &[String],
) -> Result<(), ApiError> {
    if hashes.is_empty() {
        return Ok(());
    }
    sqlx::query(
        "DELETE FROM builder.uploaded_contents u WHERE u.hash = ANY($1) \
         AND NOT EXISTS (SELECT 1 FROM builder.item_contents f WHERE f.hash = u.hash) \
         AND NOT EXISTS (SELECT 1 FROM builder.items v WHERE v.video = u.hash)",
    )
    .bind(hashes)
    .execute(conn)
    .await?;
    Ok(())
}

fn refused_input() -> ApiError {
    ApiError::bad_request("Invalid request body")
}

/// A field the body named, as the column takes it, and whether it was named.
fn named<T: Clone>(field: &Option<Option<T>>) -> (bool, Option<T>) {
    (field.is_some(), field.clone().flatten())
}

impl ItemsComponent {
    /// Saves the item `save` was decided for, as `input` and `intent` ask, and
    /// answers its body. A third-party item's pending curation takes the
    /// saved content hash, so a review always sees what it would approve.
    pub async fn save_item(
        &self,
        save: &ItemSave,
        mut input: ItemInput,
        intent: &SaveIntent,
    ) -> Result<FullItemOut, ApiError> {
        let name = input.name.trim().to_string();
        if name.is_empty()
            || [&input.price, &input.beneficiary]
                .iter()
                .any(|f| matches!(f, Some(Some(v)) if v.is_empty()))
        {
            return Err(refused_input());
        }
        input.sanitize();
        let id = save.id();
        let mut tx = self.pool.begin().await?;
        let view = chain_state::for_decision(&mut tx, &self.clock).await?;
        lock_item(&mut tx, id, intent.collection).await?;
        if let UrnField::ThirdParty(urn) = &intent.urn {
            sqlx::query(
                "SELECT pg_advisory_xact_lock(hashtextextended('builder item urn ' || lower($1), 0))",
            )
            .bind(&urn.item)
            .execute(&mut *tx)
            .await?;
        }
        let plan = access::confirm_item_save(&mut tx, &view, save, intent).await?;
        let (owner, urn, keep_video, local_hash) = {
            let hashed = Hashed {
                name: &name,
                description: input.description.as_deref(),
                rarity: input.rarity.as_ref().map(|r| r.as_deref()),
                item_type: input.item_type,
                data: &input.data,
                metrics: &input.metrics,
                contents: &input.contents,
                mappings: input.mappings.as_ref().and_then(Option::as_ref),
            };
            match &plan {
                SavePlan::Standard {
                    owner,
                    blockchain_item_id,
                    hash_contract,
                    keep_video,
                } => (
                    owner.clone(),
                    None,
                    *keep_video,
                    blockchain_item_id
                        .as_deref()
                        .zip(hash_contract.as_deref())
                        .map(|(item, contract)| {
                            hash::standard(&self.network, &hashed, contract, item)
                        }),
                ),
                SavePlan::ThirdParty { urn } => (
                    save.signer().to_string(),
                    Some(urn.clone()),
                    false,
                    Some(hash::third_party(&hashed, urn)),
                ),
            }
        };
        let (thumbnail_named, thumbnail) = (input.thumbnail.is_some(), input.thumbnail.clone());
        let (video_named, video) = named(&input.video);
        let (price_named, price) = named(&input.price);
        let (beneficiary_named, beneficiary) = named(&input.beneficiary);
        let (rarity_named, rarity) = named(&input.rarity);
        let (utility_named, utility) = named(&input.utility);
        let mappings_named = input.mappings.is_some();
        let mappings = input
            .mappings
            .as_ref()
            .and_then(Option::as_ref)
            .map(crate::content::js_stringify);
        let data = crate::content::js_stringify(&input.data);
        let metrics = crate::content::js_stringify(&input.metrics);
        let incoming = data.len() + metrics.len() + mappings.as_ref().map_or(0, String::len);
        let incoming = i64::try_from(incoming)
            .map_err(|_| ApiError::internal("an item's documents past i64 bytes"))?;
        self.within_documents(&mut tx, &owner, id, incoming).await?;
        let (video_before, video_after): (Option<String>, Option<String>) = sqlx::query_as(
            "WITH before AS (SELECT video FROM builder.items WHERE id = $1) \
             INSERT INTO builder.items AS i (id, collection_id, eth_address, name, description, thumbnail, video, type, \
               rarity, price, beneficiary, data, metrics, utility, mappings, urn_suffix, local_content_hash, video_named_by) \
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12::text::json, $13::text::json, $14, \
               $15::text::jsonb, $16, $17, CASE WHEN $7::text IS NULL THEN NULL ELSE $26 END) \
             ON CONFLICT (id) DO UPDATE SET collection_id = EXCLUDED.collection_id, eth_address = EXCLUDED.eth_address, \
               name = EXCLUDED.name, description = EXCLUDED.description, \
               thumbnail = CASE WHEN $18 THEN EXCLUDED.thumbnail ELSE i.thumbnail END, \
               video = CASE WHEN $19 AND NOT $25 THEN EXCLUDED.video ELSE i.video END, \
               video_named_by = CASE WHEN NOT $19 OR $25 OR EXCLUDED.video IS NOT DISTINCT FROM i.video \
                 THEN i.video_named_by \
                 WHEN EXCLUDED.video IS NULL THEN NULL \
                 ELSE COALESCE((SELECT p.named_by FROM builder.item_contents p \
                   WHERE p.item_id = i.id AND p.hash = EXCLUDED.video \
                   ORDER BY EXISTS (SELECT 1 FROM builder.uploaded_contents u \
                     WHERE u.hash = p.hash AND u.uploaded_by = p.named_by) DESC, p.named_by \
                   LIMIT 1), $26) END, \
               type = EXCLUDED.type, \
               rarity = CASE WHEN $20 THEN EXCLUDED.rarity ELSE i.rarity END, \
               price = CASE WHEN $21 THEN EXCLUDED.price ELSE i.price END, \
               beneficiary = CASE WHEN $22 THEN EXCLUDED.beneficiary ELSE i.beneficiary END, \
               data = EXCLUDED.data, metrics = EXCLUDED.metrics, \
               utility = CASE WHEN $23 THEN EXCLUDED.utility ELSE i.utility END, \
               mappings = CASE WHEN $24 THEN EXCLUDED.mappings ELSE i.mappings END, \
               urn_suffix = COALESCE(EXCLUDED.urn_suffix, i.urn_suffix), \
               local_content_hash = EXCLUDED.local_content_hash, \
               updated_at = timezone('UTC', now()) \
             RETURNING (SELECT video FROM before), i.video",
        )
        .bind(id)
        .bind(intent.collection)
        .bind(&owner)
        .bind(&name)
        .bind(&input.description)
        .bind(&thumbnail)
        .bind(&video)
        .bind(input.item_type.as_str())
        .bind(&rarity)
        .bind(&price)
        .bind(&beneficiary)
        .bind(&data)
        .bind(&metrics)
        .bind(&utility)
        .bind(&mappings)
        .bind(&urn)
        .bind(&local_hash)
        .bind(thumbnail_named)
        .bind(video_named)
        .bind(rarity_named)
        .bind(price_named)
        .bind(beneficiary_named)
        .bind(utility_named)
        .bind(mappings_named)
        .bind(keep_video)
        .bind(save.signer())
        .fetch_one(&mut *tx)
        .await?;
        if let (SavePlan::ThirdParty { .. }, Some(hash)) = (&plan, &local_hash) {
            sqlx::query(sqlx::AssertSqlSafe(format!(
                "UPDATE builder.item_curations SET content_hash = $2 WHERE id = {} AND status = 'pending'",
                crate::curation::newest_item_curation("$1", "k.id")
            )))
            .bind(id)
            .bind(hash)
            .execute(&mut *tx)
            .await?;
        }
        let previous: Vec<(String, String, String)> = sqlx::query_as(
            "DELETE FROM builder.item_contents WHERE item_id = $1 RETURNING file, hash, named_by",
        )
        .bind(id)
        .fetch_all(&mut *tx)
        .await?;
        let files: Vec<&str> = input.contents.iter().map(|(f, _)| f.as_str()).collect();
        let hashes: Vec<&str> = input.contents.iter().map(|(_, h)| h.as_str()).collect();
        let (before_files, before_hashes, before_writers): (Vec<&str>, Vec<&str>, Vec<&str>) =
            previous
                .iter()
                .map(|(f, h, w)| (f.as_str(), h.as_str(), w.as_str()))
                .collect();
        sqlx::query(NAMED_CONTENTS)
            .bind(id)
            .bind(&files)
            .bind(&hashes)
            .bind(&before_files)
            .bind(&before_hashes)
            .bind(&before_writers)
            .bind(save.signer())
            .execute(&mut *tx)
            .await?;
        let dropped: Vec<String> = previous
            .into_iter()
            .map(|(_, h, _)| h)
            .chain(video_before.filter(|v| video_after.as_ref() != Some(v)))
            .filter(|h| !hashes.contains(&h.as_str()))
            .collect();
        forget_unnamed(&mut tx, &dropped).await?;
        tx.commit().await?;
        listing::item(self, id, Reached::ById, true).await
    }
}

/// The stored bytes of the documents of the items `i`.
const STORED_BYTES: &str = "COALESCE(sum(pg_column_size(i.data) + pg_column_size(i.metrics) + COALESCE(pg_column_size(i.mappings), 0)), 0)::bigint";

/// A refusal of a save that would pass a wallet's bound of drafts.
pub(crate) fn drafts_full(over: serde_json::Value) -> ApiError {
    ApiError::too_large("The wallet's unpublished drafts reached their limit").with_data(over)
}

/// Queues the saves of `owner`'s drafts, so its bounds hold however many
/// it sends at once.
pub(crate) async fn lock_owner(
    tx: &mut Transaction<'_, Postgres>,
    owner: &str,
) -> Result<(), ApiError> {
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended('builder drafts of ' || $1, 0))")
        .bind(owner)
        .execute(&mut **tx)
        .await?;
    Ok(())
}

impl ItemsComponent {
    /// Refuses (413, [`drafts_full`]) saving the item `id` for `owner` with
    /// `incoming` bytes of documents when the owner's unpublished items would
    /// pass `documents.items` (a new item) or their stored documents
    /// `documents.bytes`. Counts every item the owner holds first, and the
    /// unpublished ones only past a bound.
    async fn within_documents(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        owner: &str,
        id: Uuid,
        incoming: i64,
    ) -> Result<(), ApiError> {
        let bound = self.documents;
        let (items, bytes) = (
            i64::try_from(bound.items).unwrap_or(i64::MAX),
            i64::try_from(bound.bytes).unwrap_or(i64::MAX),
        );
        lock_owner(tx, owner).await?;
        let (new, held): (bool, i64) = sqlx::query_as(
            "SELECT NOT EXISTS (SELECT 1 FROM builder.items WHERE id = $2), \
               (SELECT count(*) FROM builder.items WHERE eth_address = $1 AND id <> $2)",
        )
        .bind(owner)
        .bind(id)
        .fetch_one(&mut **tx)
        .await?;
        let may_pass_bytes = (held + 1).saturating_mul(crate::ITEM_BODY_LIMIT as i64) > bytes;
        let stored: i64 = if may_pass_bytes {
            sqlx::query_scalar(sqlx::AssertSqlSafe(format!(
                "SELECT {STORED_BYTES} FROM builder.items i WHERE i.eth_address = $1 AND i.id <> $2"
            )))
            .bind(owner)
            .bind(id)
            .fetch_one(&mut **tx)
            .await?
        } else {
            0
        };
        if !(new && held >= items) && stored.saturating_add(incoming) <= bytes {
            return Ok(());
        }
        let (chain_join, published) = chain_state::published_by_any_pass();
        let (held, stored): (i64, i64) = sqlx::query_as(sqlx::AssertSqlSafe(format!(
            "SELECT count(*), {STORED_BYTES} FROM builder.items i \
             LEFT JOIN builder.collections c ON c.id = i.collection_id {chain_join} \
             WHERE i.eth_address = $1 AND i.id <> $2 AND NOT COALESCE(CASE WHEN c.third_party_id IS NOT NULL \
               THEN {curated} ELSE {published} END, false)",
            curated = crate::curation::ITEM_CURATED,
        )))
        .bind(owner)
        .bind(id)
        .fetch_one(&mut **tx)
        .await?;
        if new && held >= items {
            return Err(drafts_full(serde_json::json!({ "maxItems": bound.items })));
        }
        if stored.saturating_add(incoming) > bytes {
            return Err(drafts_full(serde_json::json!({ "maxBytes": bound.bytes })));
        }
        Ok(())
    }
}

impl ItemsComponent {
    /// `DELETE /v1/items/:id`: builder-server's `deleteItem` rules after the
    /// decision, in one transaction with the item locked: a third-party item
    /// waits while the node publishes its collection (423) and stays once
    /// submitted (409); a standard item of a collection stays once the
    /// collection is published (409) and waits while the node publishes it
    /// or its owner's lock holds (423). The item's contents go with it, and the upload records of the
    /// files no other item names.
    pub async fn delete_item(&self, change: &ItemChange) -> Result<(), ApiError> {
        let id = change.id();
        let mut tx = self.pool.begin().await?;
        let view = chain_state::for_decision(&mut tx, &self.clock).await?;
        lock_item(&mut tx, id, None).await?;
        let state = access::confirm_item_change(&mut tx, &view, change).await?;
        let third_party = state.third_party && state.urn.is_some();
        if state.pending {
            return Err(ApiError::locked(Subject::Item, &id.to_string(), "deleted"));
        }
        if third_party && state.submitted {
            return Err(ApiError::conflict(
                "The third party item is already published. It can't be deleted.",
            )
            .with_data(serde_json::json!({ "id": id, "urn": state.urn })));
        }
        if !third_party && state.published {
            return Err(
                ApiError::published(Subject::Item, &id.to_string(), "deleted")
                    .with_data(serde_json::json!({ "id": id, "contract_address": state.contract })),
            );
        }
        if state.locked {
            return Err(ApiError::locked(Subject::Item, &id.to_string(), "deleted"));
        }
        let mut hashes: Vec<String> = sqlx::query_scalar(
            "DELETE FROM builder.item_contents WHERE item_id = $1 RETURNING hash",
        )
        .bind(id)
        .fetch_all(&mut *tx)
        .await?;
        let video: Option<String> =
            sqlx::query_scalar("DELETE FROM builder.items WHERE id = $1 RETURNING video")
                .bind(id)
                .fetch_one(&mut *tx)
                .await?;
        hashes.extend(video);
        forget_unnamed(&mut tx, &hashes).await?;
        tx.commit().await?;
        Ok(())
    }
}

//! `GET /v1/collections/:id/approvalData`, as builder-server answers it:
//! what the committee's `reviewThirdPartyWithRoot` transaction needs to
//! approve a third-party collection's submitted items. The newest cheque,
//! each item's newest curation's content hash (the client builds the
//! merkle tree over them), the provider's registry root and whether the
//! registry already consumed the cheque. A save through the node moves the
//! pending curation's hash to the saved content, so the hash answered is the
//! item's current one; a pending curation whose hash the item no longer
//! holds, which only rows written outside the node's saves (an import) can
//! leave, is refused (409) rather than attested. The rows are read first;
//! the registry is read holding no connection, and 503 when it can't tell.

use std::collections::BTreeMap;

use serde::Serialize;
use serde_json::{Value, json};
use uuid::Uuid;

use crate::State;
use crate::access::ApprovalRead;
use crate::cheques::{self, Cheque};
use crate::http::ApiError;
use crate::third_parties;

const UNAVAILABLE: &str = "The third-party registry is not available. Try again later.";

/// What the route answers.
#[derive(Debug, Serialize)]
pub struct ApprovalData {
    pub cheque: Cheque,
    pub root: Value,
    pub content_hashes: BTreeMap<String, String>,
    #[serde(rename = "chequeWasConsumed")]
    pub cheque_was_consumed: bool,
}

/// The approval data of the collection `read` names.
pub async fn approval_data(state: &State, read: &ApprovalRead) -> Result<ApprovalData, ApiError> {
    let id = read.id();
    let provider: Option<Option<String>> =
        sqlx::query_scalar("SELECT third_party_id FROM builder.collections WHERE id = $1")
            .bind(id)
            .fetch_optional(state.items.pool())
            .await?;
    let Some(Some(provider)) = provider else {
        return Err(
            ApiError::conflict("Collection is not Third Party").with_data(json!({ "id": id }))
        );
    };
    let hashes: Vec<(Uuid, String)> = sqlx::query_as(sqlx::AssertSqlSafe(format!(
        "SELECT i.id, {} FROM builder.items i WHERE i.collection_id = $1 AND {}",
        crate::curation::newest_item_curation("i.id", "k.content_hash"),
        crate::curation::ITEM_CURATED
    )))
    .bind(id)
    .fetch_all(state.items.pool())
    .await?;
    let cheque: Option<(Cheque, String)> = sqlx::query_as::<_, (i32, String, String, String)>(
        "SELECT qty, salt, signature, third_party_id FROM builder.slot_usage_cheques \
         WHERE collection_id = $1 ORDER BY created_at DESC, id DESC LIMIT 1",
    )
    .bind(id)
    .fetch_optional(state.items.pool())
    .await?
    .map(|(qty, salt, signature, third_party)| {
        (
            Cheque {
                qty,
                salt,
                signature,
            },
            third_party,
        )
    });
    let (Some((cheque, cheque_provider)), false) = (cheque, hashes.is_empty()) else {
        return Err(ApiError::unauthorized("The collection is not published.")
            .with_data(json!({ "id": id })));
    };
    let changed: Vec<Uuid> = sqlx::query_scalar(sqlx::AssertSqlSafe(format!(
        "SELECT i.id FROM builder.items i WHERE i.collection_id = $1 \
           AND {} = 'pending' AND {} IS DISTINCT FROM i.local_content_hash ORDER BY i.id",
        crate::curation::newest_item_curation("i.id", "k.status"),
        crate::curation::newest_item_curation("i.id", "k.content_hash"),
    )))
    .bind(id)
    .fetch_all(state.items.pool())
    .await?;
    if !changed.is_empty() {
        return Err(
            ApiError::conflict("The items changed since they were submitted for review")
                .with_data(json!({ "id": id, "itemIds": changed })),
        );
    }
    let virtual_party: bool = sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM builder.virtual_third_parties WHERE id = $1)",
    )
    .bind(&provider)
    .fetch_one(state.items.pool())
    .await?;
    let receipt = cheques::receipt_id(&state.settings.network, &cheque_provider, &cheque)
        .ok_or_else(|| ApiError::internal(format!("stored cheque of {id} unreadable")))?;
    let registry = state
        .third_parties
        .registry(state, crate::kept::Since::write())
        .await?;
    let root = match (third_parties::provider(&registry, &provider), virtual_party) {
        (Some(record), _) => record.root.clone().map_or(Value::Null, Value::String),
        (None, true) => json!(""),
        (None, false) => return Err(ApiError::http(200, "The Third Party doesn't exists.")),
    };
    let consumed = third_parties::cheque_consumed(state, &cheque_provider, &receipt)
        .await
        .map_err(|error| {
            tracing::warn!(%error, "the third-party registry's receipts were not read");
            ApiError::service_unavailable(UNAVAILABLE)
        })?;
    Ok(ApprovalData {
        cheque,
        root,
        content_hashes: hashes
            .into_iter()
            .map(|(item, hash)| (item.to_string(), hash))
            .collect(),
        cheque_was_consumed: consumed,
    })
}

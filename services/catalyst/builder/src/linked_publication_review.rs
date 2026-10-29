//! The linked publication's finish: the client hands over its own
//! signed-fetch headers for three Foundation reads (the collection, its
//! items, its item curations), this replays them against the fixed
//! Foundation host with NO redirects (a redirect would forward the owner's
//! signed chain to a third party), checks that each set was signed by the
//! collection owner over exactly that path, and verifies what the
//! Foundation serves against the frozen `snapshot` before marking the
//! attempt `submitted` and the items published with the Foundation's urn,
//! content hash and curation status.
//!
//! The Foundation host is `BUILDER_FOUNDATION_URL` (the mirror posture's:
//! Decentraland's builder API); unset, verification answers 503 and nothing
//! is sent. The client is a guarded [`dcl_http::Outbound`] of its own,
//! not the process's: that one follows up to two redirects by design
//! (re-guarded per hop, but with the request headers kept), and the old
//! crate's contract for this hop is no redirect at all (20 s budget).

use std::{collections::BTreeMap, time::Duration};

use axum::http::{HeaderMap, HeaderName, HeaderValue};
use serde::Deserialize;
use serde_json::Value;
use uuid::Uuid;

use crate::shared::signed_fetch::{
    AUTH_CHAIN_HEADER_PREFIX, AUTH_METADATA_HEADER, AUTH_TIMESTAMP_HEADER,
};
use dcl_auth::MAX_AUTH_CHAIN_LINKS;
use dcl_http::HttpClientCfg;

use super::auth::{SignedPath, require_signer};
use super::http::ApiError;
use super::items::ItemsComponent;
use super::linked_publication_store::{
    LinkedPublicationState, STATE_QUERY, decode, lock_collection,
};

const MAX_RESPONSE: usize = 8 * 1024 * 1024;
const REQUEST_BUDGET: Duration = Duration::from_secs(20);

#[derive(Deserialize)]
pub struct FoundationReviewSignatures {
    pub collection: BTreeMap<String, String>,
    pub items: BTreeMap<String, String>,
    pub curations: BTreeMap<String, String>,
}

/// The guarded no-redirect client for the Foundation reads, built once per process.
pub fn foundation_client() -> dcl_http::Outbound {
    dcl_http::Outbound::new(HttpClientCfg::default().with_total_timeout(REQUEST_BUDGET))
        .expect("the Foundation client builds")
}

pub struct FoundationLinkedReview {
    pub client: dcl_http::Outbound,
    pub base: String,
}

impl FoundationLinkedReview {
    /// `None` when this node names no Foundation (`BUILDER_FOUNDATION_URL`).
    pub fn new(client: dcl_http::Outbound, base: Option<&str>) -> Option<Self> {
        Some(Self {
            client,
            base: base?.to_string(),
        })
    }

    async fn fetch(
        &self,
        owner: &str,
        path: &str,
        signed: &BTreeMap<String, String>,
    ) -> Result<Option<Value>, ApiError> {
        let headers = signed_headers(owner, path, signed).await?;
        let mut response = self.client.get(format!("{}{path}", self.base)).headers(headers).send().await
            .map_err(|_| ApiError::service_unavailable("Foundation review verification is unavailable. Retry to check the same submission."))?;
        if response.status() == reqwest::StatusCode::NOT_FOUND {
            return Ok(None);
        }
        if !response.status().is_success() {
            return Err(ApiError::http(
                502,
                "Foundation could not verify this submission. Check provider access and retry.",
            ));
        }
        if response
            .content_length()
            .is_some_and(|size| size > MAX_RESPONSE as u64)
        {
            return Err(ApiError::http(
                502,
                "Foundation review response is too large",
            ));
        }
        let mut bytes = Vec::new();
        while let Some(chunk) = response
            .chunk()
            .await
            .map_err(|_| ApiError::http(502, "Foundation review response was interrupted"))?
        {
            if bytes.len() + chunk.len() > MAX_RESPONSE {
                return Err(ApiError::http(
                    502,
                    "Foundation review response is too large",
                ));
            }
            bytes.extend_from_slice(&chunk);
        }
        let body: Value = serde_json::from_slice(&bytes)
            .map_err(|_| ApiError::http(502, "Invalid Foundation review response"))?;
        if body["ok"] == false || !body.as_object().is_some_and(|obj| obj.contains_key("data")) {
            return Err(ApiError::http(502, "Invalid Foundation review response"));
        }
        Ok(Some(body["data"].clone()))
    }

    pub async fn verify(
        &self,
        store: &ItemsComponent,
        owner: &str,
        id: Uuid,
        signatures: &FoundationReviewSignatures,
    ) -> Result<LinkedPublicationState, ApiError> {
        let state = store
            .linked_publication_state(owner, id)
            .await?
            .ok_or_else(|| ApiError::not_found("Linked publication not found"))?;
        if state.status == "submitted" {
            return Ok(state);
        }
        if state.status != "authorized" {
            return Err(ApiError::http(
                409,
                "Save the slot authorization before checking submission",
            ));
        }
        let snapshot: Value = sqlx::query_scalar(
            "SELECT snapshot FROM builder.linked_collection_publications WHERE collection_id=$1",
        )
        .bind(id)
        .fetch_one(&store.pool)
        .await?;
        let path = format!("/v1/collections/{id}");
        let items_path = format!("{path}/items");
        let curations_path = format!("{path}/itemCurations");
        let (collection, items, curations) = tokio::try_join!(
            self.fetch(owner, &path, &signatures.collection),
            self.fetch(owner, &items_path, &signatures.items),
            self.fetch(owner, &curations_path, &signatures.curations),
        )?;
        let (Some(collection), Some(items), Some(curations)) = (collection, items, curations)
        else {
            return Ok(state);
        };
        let Some(review) = verify_review(&snapshot, &collection, &items, &curations)? else {
            return Ok(state);
        };
        store
            .finish_linked_publication(owner, id, &state.preparation.revision, review)
            .await
    }
}

/// Only the `x-identity-*` headers are forwarded, and only once they verify
/// as the owner's signature over exactly `path` (the shared verifier, no
/// `x-original-path`: the map refuses that key).
async fn signed_headers(
    owner: &str,
    path: &str,
    signed: &BTreeMap<String, String>,
) -> Result<HeaderMap, ApiError> {
    if signed.len() > MAX_AUTH_CHAIN_LINKS + 2 {
        return Err(ApiError::bad_request(
            "Too many Foundation signature headers",
        ));
    }
    let mut headers = HeaderMap::new();
    for (name, value) in signed {
        let name = HeaderName::from_bytes(name.as_bytes())
            .map_err(|_| ApiError::bad_request("Invalid Foundation signature header"))?;
        let allowed = [AUTH_TIMESTAMP_HEADER, AUTH_METADATA_HEADER].contains(&name.as_str())
            || name
                .as_str()
                .strip_prefix(AUTH_CHAIN_HEADER_PREFIX)
                .is_some_and(|index| {
                    !index.is_empty() && index.bytes().all(|c| c.is_ascii_digit())
                });
        if !allowed || value.len() > 8192 {
            return Err(ApiError::bad_request("Invalid Foundation signature header"));
        }
        let value = HeaderValue::from_str(value)
            .map_err(|_| ApiError::bad_request("Invalid Foundation signature header"))?;
        if headers.insert(name, value).is_some() {
            return Err(ApiError::bad_request(
                "Duplicate Foundation signature header",
            ));
        }
    }
    let signer = require_signer(&headers, "get", &SignedPath::local(path), None).await?;
    if !signer.as_str().eq_ignore_ascii_case(owner) {
        return Err(ApiError::forbidden(
            "Foundation requests must be signed by the collection owner",
        ));
    }
    Ok(headers)
}

struct ReviewedItem {
    id: Uuid,
    urn: String,
    content_hash: String,
    status: String,
    approved: bool,
}
struct VerifiedReview {
    items: Vec<ReviewedItem>,
    forum_url: Option<String>,
}

fn verify_review(
    snapshot: &Value,
    collection: &Value,
    items: &Value,
    curations: &Value,
) -> Result<Option<VerifiedReview>, ApiError> {
    let mismatch = || {
        ApiError::http(
            409,
            "Foundation's submission does not match the reviewed collection and content",
        )
    };
    let local = &snapshot["collection"];
    let owner = local["eth_address"].as_str().ok_or_else(mismatch)?;
    let urn = local["urn_suffix"].as_str().ok_or_else(mismatch)?;
    if collection["id"] != local["id"]
        || collection["name"] != local["name"]
        || collection["urn"] != urn
        || !collection["contract_address"].is_null()
        || !collection["eth_address"]
            .as_str()
            .is_some_and(|s| s.eq_ignore_ascii_case(owner))
    {
        return Err(mismatch());
    }
    let expected = snapshot["items"].as_array().ok_or_else(mismatch)?;
    let items = items.as_array().ok_or_else(mismatch)?;
    let curations = curations.as_array().ok_or_else(mismatch)?;
    if expected.is_empty() || items.len() != expected.len() {
        return Err(mismatch());
    }
    let mut reviewed = Vec::with_capacity(expected.len());
    for saved in expected {
        let id = saved["id"].as_str().ok_or_else(mismatch)?;
        let matching: Vec<_> = items.iter().filter(|item| item["id"] == id).collect();
        if matching.len() != 1 {
            return Err(mismatch());
        }
        let item = matching[0];
        let item_urn = format!("{urn}:{id}");
        if item["urn"] != item_urn
            || item["collection_id"] != local["id"]
            || !item["eth_address"]
                .as_str()
                .is_some_and(|s| s.eq_ignore_ascii_case(owner))
            || ["name", "type", "thumbnail", "data", "metrics", "contents"]
                .iter()
                .any(|key| item[*key] != saved[*key])
            || item["description"].as_str().unwrap_or("")
                != saved["description"].as_str().unwrap_or("")
        {
            return Err(mismatch());
        }
        if curations.is_empty() {
            continue;
        }
        let matching: Vec<_> = curations
            .iter()
            .filter(|curation| curation["item_id"] == id)
            .collect();
        if matching.len() != 1 {
            return Err(mismatch());
        }
        let curation = matching[0];
        let hash = item["local_content_hash"]
            .as_str()
            .filter(|h| !h.is_empty())
            .ok_or_else(mismatch)?;
        let status = curation["status"]
            .as_str()
            .filter(|s| matches!(*s, "pending" | "approved" | "rejected"))
            .ok_or_else(mismatch)?;
        if curation["content_hash"] != hash || item["is_published"] != true {
            return Err(mismatch());
        }
        reviewed.push(ReviewedItem {
            id: id.parse().map_err(|_| mismatch())?,
            urn: item_urn,
            content_hash: hash.into(),
            status: status.into(),
            approved: item["is_approved"] == true && status == "approved",
        });
    }
    if curations.is_empty() {
        if collection["is_published"] == true
            || items.iter().any(|item| item["is_published"] == true)
        {
            return Err(mismatch());
        }
        return Ok(None);
    }
    if curations.len() != expected.len() || collection["is_published"] != true {
        return Err(mismatch());
    }
    let forum_url = match collection["forum_link"]
        .as_str()
        .filter(|url| !url.is_empty())
    {
        None => None,
        Some(raw) => {
            let url = reqwest::Url::parse(raw).map_err(|_| mismatch())?;
            if url.scheme() != "https"
                || url.host_str() != Some("forum.decentraland.org")
                || !url.path().starts_with("/t/")
                || !url.username().is_empty()
                || url.password().is_some()
                || url.port_or_known_default() != Some(443)
            {
                return Err(mismatch());
            }
            Some(url.to_string())
        }
    };
    Ok(Some(VerifiedReview {
        items: reviewed,
        forum_url,
    }))
}

impl ItemsComponent {
    /// Marks the items published with the Foundation's urn, content hash and
    /// curation status (`is_approved` is the declared projection of the
    /// status; the old rows could carry an approved status with the
    /// Foundation's `is_approved: false`, which the `items_approved_projection_check`
    /// forbids, so that combination approves the item's status and keeps the
    /// collection unapproved), the collection published, the attempt
    /// `submitted`. Idempotent once submitted.
    async fn finish_linked_publication(
        &self,
        owner: &str,
        id: Uuid,
        revision: &str,
        review: VerifiedReview,
    ) -> Result<LinkedPublicationState, ApiError> {
        let mut tx = self.pool.begin().await?;
        lock_collection(&mut tx, owner, id).await?;
        let value: Value = sqlx::query_scalar(STATE_QUERY)
            .bind(id)
            .bind(owner)
            .fetch_one(&mut *tx)
            .await?;
        let mut state = decode(value)?;
        if state.preparation.revision != revision {
            return Err(ApiError::http(409, "Linked publication revision changed"));
        }
        if state.status == "submitted" {
            return Ok(state);
        }
        if state.status != "authorized" || state.cheque.is_none() {
            return Err(ApiError::http(409, "Linked publication is not authorized"));
        }
        let approved = review.items.iter().all(|item| item.approved);
        for item in review.items {
            let changed = sqlx::query("UPDATE builder.items SET is_published=true,urn_suffix=$2,blockchain_item_id=$3,local_content_hash=$4,curation_status=$5,is_approved=($5='approved'),price='0',beneficiary='0x0000000000000000000000000000000000000000',rarity=NULL,total_supply=0,updated_at=timezone('UTC', now()) \
                WHERE id=$1 AND collection_id=$6 AND eth_address=$7 AND NOT is_published")
                .bind(item.id).bind(item.urn).bind(item.id.to_string()).bind(item.content_hash).bind(item.status).bind(id).bind(owner)
                .execute(&mut *tx).await?.rows_affected();
            if changed != 1 {
                return Err(ApiError::http(
                    409,
                    "Linked items changed during publication",
                ));
            }
        }
        sqlx::query("UPDATE builder.collections SET is_published=true,is_approved=$2,publication_pending=false,updated_at=timezone('UTC', now()) WHERE id=$1").bind(id).bind(approved).execute(&mut *tx).await?;
        sqlx::query("UPDATE builder.linked_collection_publications SET status='submitted',forum_url=$2,updated_at=timezone('UTC', now()) WHERE collection_id=$1")
            .bind(id).bind(&review.forum_url).execute(&mut *tx).await?;
        state.status = "submitted".into();
        state.forum_url = review.forum_url;
        tx.commit().await?;
        Ok(state)
    }
}

#[cfg(test)]
mod tests;

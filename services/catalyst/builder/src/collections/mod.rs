//! Collections as builder-server answers them: the body of `GET
//! /v1/collections/:id`, `PUT /v1/collections/:id` and the two listings,
//! the save ([`save`]) and the listings ([`listing`]).
//!
//! The body is builder-server's `FullCollection`: the stored row, with a
//! standard collection the chain copy holds merged as its `Bridge` merges the
//! subgraph (name, creator, roles, approval and dates from the copy), and
//! `lock` the owner's last lock, and a third-party collection with an item
//! curation merged as its `Bridge.mergeTPCollection` merges the curation of
//! its first curated item (by item id): `created_at`, `updated_at` and
//! `reviewed_at` from that curation. A listing row adds the status of the
//! collection's newest curation. A caller who may read only the public detail
//! gets the keys [`PUBLIC_DETAIL`] names; the public listing, those
//! [`PUBLIC_LISTING`] names. A key not named there is never answered to them.

pub mod approval;
pub mod contract;
pub mod deployment;
pub mod lifecycle;
pub mod listing;
pub mod publish_third_party;
pub mod save;

use chrono::{DateTime, NaiveDateTime, Utc};
use serde::Serialize;
use serde_json::Value;
use sqlx::PgConnection;
use uuid::Uuid;

use crate::access::{CollectionGrant, CollectionRead};
use crate::chain_state::{self, ReadView};
use crate::http::{ApiError, iso};
use crate::items::ItemsComponent;

/// The keys of a published collection's public detail
/// (`toPublicCollectionDetail`).
pub const PUBLIC_DETAIL: &[&str] = &[
    "id",
    "name",
    "eth_address",
    "contract_address",
    "is_published",
    "created_at",
    "updated_at",
    "managers",
    "minters",
    "is_approved",
    "reviewed_at",
    "linked_contract_address",
    "linked_contract_network",
    "is_mapping_complete",
    "item_count",
    "is_programmatic",
    "urn",
];

/// The keys of a row of the public search (`toPublicCollection`).
pub const PUBLIC_LISTING: &[&str] = &[
    "name",
    "eth_address",
    "contract_address",
    "is_published",
    "created_at",
    "updated_at",
    "managers",
    "minters",
    "is_approved",
    "reviewed_at",
    "linked_contract_address",
    "linked_contract_network",
    "is_mapping_complete",
    "item_count",
    "is_programmatic",
    "urn",
];

/// A collection's body (builder-server's `FullCollection`).
#[derive(Debug, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, export_to = "builder/"))]
pub struct CollectionOut {
    pub id: String,
    pub name: String,
    pub eth_address: String,
    pub salt: Option<String>,
    pub contract_address: Option<String>,
    pub is_published: bool,
    pub created_at: String,
    pub updated_at: String,
    pub managers: Vec<String>,
    pub minters: Vec<String>,
    pub is_approved: bool,
    pub reviewed_at: Option<String>,
    pub forum_link: Option<String>,
    pub lock: Option<String>,
    #[cfg_attr(feature = "ts", ts(type = "number | null"))]
    pub forum_id: Option<i64>,
    pub linked_contract_address: Option<String>,
    pub linked_contract_network: Option<String>,
    pub is_mapping_complete: bool,
    pub item_count: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub is_programmatic: Option<bool>,
    pub urn: String,
}

/// A row of a listing: the body, its latest curation's status and when the
/// collection, its items or that curation last changed.
#[derive(Debug, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, export_to = "builder/"))]
pub struct CollectionListingRowOut {
    #[serde(flatten)]
    pub collection: CollectionOut,
    pub curation_status: Option<String>,
    pub last_activity_at: String,
}

/// The body's columns over [`ReadView::collections`].
pub(crate) fn body_columns(view: &ReadView) -> String {
    format!(
        "c.id, COALESCE(chain.name, c.name) AS name, \
         CASE WHEN c.third_party_id IS NOT NULL THEN '' ELSE COALESCE(chain.creator, c.eth_address) END AS eth_address, \
         c.salt, c.contract_address, {} AS is_published, c.created_at, c.updated_at, c.lock, \
         chain.created_at AS chain_created_at, chain.updated_at AS chain_updated_at, \
         chain.reviewed_at AS chain_reviewed_at, \
         ARRAY(SELECT lower(m) FROM unnest(chain.managers) m) AS managers, \
         ARRAY(SELECT lower(m) FROM unnest(chain.minters) m) AS minters, \
         {} AS is_approved, c.linked_contract_address, c.linked_contract_network, \
         c.third_party_id, c.urn_suffix, \
         (SELECT count(*) FROM builder.items i WHERE i.collection_id = c.id) AS item_count, \
         NOT EXISTS (SELECT 1 FROM builder.items i WHERE i.collection_id = c.id \
           AND (i.mappings IS NULL OR {} = false)) AS is_mapping_complete, \
         c.forum_link, c.forum_id, \
         CASE WHEN c.third_party_id IS NOT NULL THEN {} END AS curation_dates",
        view.published(),
        view.approved(),
        crate::curation::newest_item_curation("i.id", "COALESCE(k.is_mapping_complete, false)"),
        crate::curation::first_curated_item_curation("c.id", "ARRAY[k.created_at, k.updated_at]")
    )
}

#[derive(Debug, sqlx::FromRow)]
pub(crate) struct CollectionRow {
    id: Uuid,
    name: String,
    eth_address: String,
    salt: Option<String>,
    contract_address: Option<String>,
    is_published: bool,
    created_at: NaiveDateTime,
    updated_at: NaiveDateTime,
    lock: Option<NaiveDateTime>,
    chain_created_at: Option<i64>,
    chain_updated_at: Option<i64>,
    chain_reviewed_at: Option<i64>,
    managers: Vec<String>,
    minters: Vec<String>,
    is_approved: bool,
    linked_contract_address: Option<String>,
    linked_contract_network: Option<String>,
    third_party_id: Option<String>,
    urn_suffix: Option<String>,
    item_count: i64,
    is_mapping_complete: bool,
    forum_link: Option<String>,
    forum_id: Option<i32>,
    curation_dates: Option<Vec<NaiveDateTime>>,
}

fn unix(seconds: i64) -> String {
    iso(&DateTime::<Utc>::from_timestamp(seconds, 0).unwrap_or_default())
}

impl CollectionRow {
    pub(crate) fn third_party(&self) -> bool {
        self.third_party_id.is_some()
    }

    pub(crate) fn provider(&self) -> Option<&str> {
        self.third_party_id.as_deref()
    }

    pub(crate) fn contract_address(&self) -> Option<&str> {
        self.contract_address.as_deref()
    }

    pub(crate) fn out(
        self,
        network: &crate::network::Network,
        is_programmatic: Option<bool>,
    ) -> CollectionOut {
        let urn = match (&self.third_party_id, self.urn_suffix) {
            (Some(_), Some(urn)) => urn,
            _ => network.collection_urn(self.contract_address.as_deref().unwrap_or("null")),
        };
        let curated = match self.curation_dates.as_deref() {
            Some(&[created, updated]) => Some((created, updated)),
            _ => None,
        };
        CollectionOut {
            id: self.id.to_string(),
            name: self.name,
            eth_address: self.eth_address,
            salt: self.salt,
            contract_address: self.contract_address,
            is_published: self.is_published,
            created_at: match curated {
                Some((at, _)) => iso(&at.and_utc()),
                None => self
                    .chain_created_at
                    .map(unix)
                    .unwrap_or_else(|| iso(&self.created_at.and_utc())),
            },
            updated_at: match curated {
                Some((_, at)) => iso(&at.and_utc()),
                None => self
                    .chain_updated_at
                    .map(unix)
                    .unwrap_or_else(|| iso(&self.updated_at.and_utc())),
            },
            managers: self.managers,
            minters: self.minters,
            is_approved: self.is_approved,
            reviewed_at: match curated {
                Some((_, at)) => Some(iso(&at.and_utc())),
                None => self.chain_reviewed_at.map(unix),
            },
            forum_link: self.forum_link,
            lock: self.lock.map(|lock| iso(&lock.and_utc())),
            forum_id: self.forum_id.map(i64::from),
            linked_contract_address: self.linked_contract_address,
            linked_contract_network: self.linked_contract_network,
            is_mapping_complete: self.is_mapping_complete,
            item_count: self.item_count.to_string(),
            is_programmatic,
            urn,
        }
    }
}

/// `body` with only the `keys` named.
pub(crate) fn project(body: impl Serialize, keys: Option<&[&str]>) -> Result<Value, ApiError> {
    let mut value = serde_json::to_value(body).map_err(|e| ApiError::internal(e.to_string()))?;
    if let (Some(keys), Some(fields)) = (keys, value.as_object_mut()) {
        fields.retain(|key, _| keys.contains(&key.as_str()));
    }
    Ok(value)
}

pub(crate) async fn row(
    conn: &mut PgConnection,
    view: &ReadView,
    grant: &impl CollectionGrant,
) -> Result<Option<CollectionRow>, ApiError> {
    Ok(
        sqlx::query_as::<_, CollectionRow>(sqlx::AssertSqlSafe(format!(
            "SELECT {} FROM {} WHERE c.id = $1",
            body_columns(view),
            view.collections()
        )))
        .bind(grant.collection())
        .fetch_optional(conn)
        .await?,
    )
}

impl ItemsComponent {
    /// `GET /v1/collections/:id`: the full body, or the public detail for a
    /// caller `read` admits to that only.
    pub async fn collection(&self, read: &CollectionRead) -> Result<Value, ApiError> {
        let mut conn = self.pool.acquire().await?;
        let view = chain_state::for_read(&mut conn).await?;
        let row = row(&mut conn, &view, read)
            .await?
            .ok_or_else(|| ApiError::not_found("Not found"))?;
        let programmatic = row.third_party().then_some(read.programmatic());
        project(
            row.out(&self.network, programmatic),
            (!read.full()).then_some(PUBLIC_DETAIL),
        )
    }
}

//! Items as builder-server answers them, and the draft store handle
//! ([`ItemsComponent`], over the ONE pool): the body ([`body`]), the save's
//! schema ([`schema`]), the save and delete ([`save`]), the listings
//! ([`listing`]), third-party item URNs ([`urn`]) and the content hash a
//! published item's body carries ([`hash`]). Also the collection projection
//! the drafts listing reads ([`CollectionMetaOut`]).
//! Every timestamp column is `timestamp without time zone` in UTC (the
//! schema convention), written as [`crate::http::iso`] writes it.

pub mod body;
#[cfg(test)]
mod catalyst_state_tests;
pub mod hash;
pub mod listing;
pub mod save;
pub mod schema;
pub mod urn;

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use chrono::{DateTime, NaiveDateTime, Utc};
use serde::Serialize;
use sqlx::PgPool;
use uuid::Uuid;

use crate::chain_state::ReadView;

use super::http::{ApiError, iso_timestamp};

/// The draft store: `builder.*` over the ONE pool.
pub struct ItemsComponent {
    pub(crate) pool: PgPool,
    pub(crate) denylist: Arc<crate::content::Denylist>,
    /// What one wallet's drafts may hold.
    pub documents: crate::limits::Documents,
    /// The Polygon chain its URNs name ([`crate::network`]).
    pub(crate) network: crate::network::Network,
    released: AtomicBool,
}

const DOCUMENTS_RELEASED: &str = "SELECT (SELECT count(*) FROM information_schema.columns \
     WHERE table_schema = 'builder' AND table_name = 'items' \
       AND column_name IN ('data', 'metrics') AND data_type = 'json') = 2 \
   AND (SELECT count(*) FROM pg_constraint c \
     JOIN pg_attribute a ON a.attrelid = c.conrelid AND a.attnum = ANY(c.conkey) \
     WHERE c.conname = 'uploaded_contents_pkey' \
       AND c.conrelid = 'builder.uploaded_contents'::regclass) = 2";

impl ItemsComponent {
    pub fn new(pool: PgPool) -> Self {
        Self {
            pool,
            denylist: Arc::default(),
            documents: crate::limits::Documents::default(),
            network: crate::network::Network::default(),
            released: AtomicBool::new(false),
        }
    }

    /// The store whose URNs name `network`'s chain.
    pub fn with_network(mut self, network: crate::network::Network) -> Self {
        self.network = network;
        self
    }

    /// The store whose item bodies forget a catalyst entity `denylist` withholds.
    pub fn with_denylist(mut self, denylist: Arc<crate::content::Denylist>) -> Self {
        self.denylist = denylist;
        self
    }

    /// 503 until `schemas/releases/builder-item-documents-and-uploaders.sql`
    /// has run: item saves and uploads need `json` documents and one upload
    /// record per hash and uploader. Asked of the database until it holds,
    /// then never again in this process.
    pub(crate) async fn require_documents_release(&self) -> Result<(), ApiError> {
        if self.released.load(Ordering::Relaxed) {
            return Ok(());
        }
        let released: bool = sqlx::query_scalar(DOCUMENTS_RELEASED)
            .fetch_one(&self.pool)
            .await?;
        if !released {
            return Err(ApiError::service_unavailable(
                "Item saves and uploads are closed until an operator runs \
                 schemas/releases/builder-item-documents-and-uploaders.sql on this node's database",
            ));
        }
        self.released.store(true, Ordering::Relaxed);
        Ok(())
    }

    pub(crate) fn pool(&self) -> &PgPool {
        &self.pool
    }
}

/// The [`CollectionMetaRow`] columns over [`ReadView::collections`],
/// published and approved derived through `view`.
pub(crate) fn collection_meta_columns(view: &ReadView) -> String {
    format!(
        "c.id, c.name, c.eth_address, c.contract_address, c.urn_suffix, c.third_party_id, \
         {} AS is_published, {} AS is_approved, c.created_at, c.updated_at",
        view.published(),
        view.approved()
    )
}

#[derive(Debug, sqlx::FromRow)]
pub struct CollectionMetaRow {
    pub id: Uuid,
    pub name: String,
    pub eth_address: String,
    pub contract_address: Option<String>,
    pub urn_suffix: Option<String>,
    pub third_party_id: Option<String>,
    pub is_published: bool,
    pub is_approved: bool,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

#[derive(Debug, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, export_to = "builder/"))]
pub struct CollectionMetaOut {
    pub id: String,
    pub name: String,
    pub eth_address: String,
    pub contract_address: Option<String>,
    pub urn: Option<String>,
    pub third_party_id: Option<String>,
    pub is_published: bool,
    pub is_approved: bool,
    #[serde(serialize_with = "iso_timestamp")]
    #[cfg_attr(feature = "ts", ts(type = "string"))]
    pub created_at: DateTime<Utc>,
    #[serde(serialize_with = "iso_timestamp")]
    #[cfg_attr(feature = "ts", ts(type = "string"))]
    pub updated_at: DateTime<Utc>,
}

impl From<&CollectionMetaRow> for CollectionMetaOut {
    fn from(r: &CollectionMetaRow) -> Self {
        Self {
            id: r.id.to_string(),
            name: r.name.clone(),
            eth_address: r.eth_address.clone(),
            contract_address: r.contract_address.clone(),
            urn: r.urn_suffix.clone(),
            third_party_id: r.third_party_id.clone(),
            is_published: r.is_published,
            is_approved: r.is_approved,
            created_at: r.created_at.and_utc(),
            updated_at: r.updated_at.and_utc(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{Value, json};

    fn ts(ms: i64) -> NaiveDateTime {
        DateTime::from_timestamp_millis(ms).unwrap().naive_utc()
    }

    fn meta_row(with_opts: bool) -> CollectionMetaRow {
        CollectionMetaRow {
            id: Uuid::nil(),
            name: "Hats".into(),
            eth_address: "0xowner".into(),
            contract_address: with_opts.then(|| "0xcontract".into()),
            urn_suffix: with_opts.then(|| "urn:decentraland:matic:collections-v2:0xc0ffee".into()),
            third_party_id: with_opts
                .then(|| "urn:decentraland:matic:collections-thirdparty:tp".into()),
            is_published: true,
            is_approved: false,
            created_at: ts(1_700_000_000_000),
            updated_at: ts(1_700_000_100_000),
        }
    }

    fn old_meta_json(r: &CollectionMetaRow) -> Value {
        json!({
            "id": r.id.to_string(),
            "name": r.name,
            "eth_address": r.eth_address,
            "contract_address": r.contract_address,
            "urn": r.urn_suffix,
            "third_party_id": r.third_party_id,
            "is_published": r.is_published,
            "is_approved": r.is_approved,
            "created_at": "2023-11-14T22:13:20.000Z",
            "updated_at": "2023-11-14T22:15:00.000Z",
        })
    }

    #[test]
    fn collection_meta_wire_bytes_carry_iso_timestamps() {
        for with_opts in [true, false] {
            let r = meta_row(with_opts);
            assert_eq!(
                serde_json::to_value(CollectionMetaOut::from(&r)).unwrap(),
                old_meta_json(&r),
            );
        }
    }
}

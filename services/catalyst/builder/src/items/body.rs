//! An item as builder-server answers it (`FullItem`): the stored row, merged
//! with the chain copy's item once a standard item is minted and the copy
//! holds it (`Bridge.mergeItem`), and with its third-party collection for a
//! third-party item (`Bridge.mergeTPItem`). `data`, `metrics`, `contents`
//! and `mappings` are answered as stored, keys in the order they were saved.
//!
//! What a standard item's body says depends on whether the copy holds it:
//! minted (`blockchain_item_id`), its collection's contract in the copy and
//! the copy's item at that contract and id. Then the chain decides `urn`,
//! `price`, `beneficiary`, `rarity` (a wearable's), `total_supply`,
//! `content_hash`, `eth_address` (the collection's creator), `is_approved`
//! (the collection's) and `is_published` (true); an item the node published
//! itself reads as published, with the URN it recorded, until the copy holds
//! it. A third-party item's `urn`
//! is its own; it is published once curated (`is_published`), read by
//! itself when an item of its collection has a curation, in a list when it
//! has one, and that curation's `is_mapping_complete` answers
//! `isMappingComplete`; and
//! `price`, `beneficiary`, `total_supply` and `blockchain_item_id` are what a
//! third-party item has on no chain. The catalyst sources (`in_catalyst`,
//! `catalyst_content_hash`) answer what builder-server answers when the
//! catalyst holds no entity for the item.

use chrono::NaiveDateTime;
use serde::Serialize;
use serde_json::value::RawValue;
use uuid::Uuid;

use crate::chain_state::ReadView;
use crate::http::{ApiError, iso};

const ZERO_ADDRESS: &str = "0x0000000000000000000000000000000000000000";

/// The `FROM` items a body reads: the item `i`, its collection `c`, the
/// copy's collection `chain`, the copy's item `ci` and `cat`, the active
/// wearable or emote entity this node's content holds at the item's URN (a
/// third-party item's own, a minted one's the copy names).
pub(crate) fn from(_view: &ReadView) -> &'static str {
    "builder.items i \
     LEFT JOIN builder.collections c ON c.id = i.collection_id \
     LEFT JOIN shop.collections chain ON chain.network = 'matic' \
       AND chain.contract_address = lower(c.contract_address) AND c.third_party_id IS NULL \
     LEFT JOIN shop.items ci ON ci.network = 'matic' AND ci.contract_address = chain.contract_address \
       AND ci.item_id = CASE WHEN i.blockchain_item_id ~ '^[0-9]{1,78}$' THEN i.blockchain_item_id::numeric END \
     LEFT JOIN LATERAL (SELECT d.entity_id, d.entity_metadata -> 'v' ->> 'id' AS id, \
         d.entity_metadata -> 'v' -> 'merkleProof' ->> 'entityHash' AS proof \
       FROM content.active_pointers ap JOIN content.deployments d ON d.entity_id = ap.entity_id \
       WHERE ap.pointer = lower(CASE WHEN c.third_party_id IS NOT NULL THEN i.urn_suffix ELSE ci.urn END) \
         AND d.deleter_deployment IS NULL AND d.entity_type IN ('wearable', 'emote')) cat ON true"
}

/// The columns of [`ItemRow`] over [`from`] for a list: one probe of each
/// item's curations, nothing read per collection.
pub(crate) fn columns() -> String {
    format!(
        "i.id, i.name, i.description, i.eth_address, i.collection_id, \
    i.blockchain_item_id, i.price, i.beneficiary, i.rarity, i.type AS item_type, i.data::text AS data, \
    i.created_at, i.updated_at, i.metrics::text AS metrics, i.thumbnail, \
    (SELECT COALESCE(json_object_agg(f.file, f.hash ORDER BY f.position, f.file), '{{}}'::json)::text \
       FROM builder.item_contents f WHERE f.item_id = i.id) AS contents, \
    i.local_content_hash, i.video, i.utility, i.mappings::text AS mappings, i.urn_suffix, \
    i.is_published AS submitted, \
    {} AS own_curation_mapped, \
    c.third_party_id, c.urn_suffix AS collection_urn, c.contract_address, \
    chain.creator AS chain_creator, \
    chain.is_approved AS chain_approved, ci.urn AS chain_urn, ci.item_id::text AS chain_item_id, \
    ci.price::text AS chain_price, ci.beneficiary AS chain_beneficiary, ci.rarity AS chain_rarity, \
    ci.item_type AS chain_item_type, ci.total_supply::text AS chain_supply, \
    ci.content_hash AS chain_content_hash, \
    cat.entity_id AS catalyst_entity, cat.id AS catalyst_id, cat.proof AS catalyst_proof",
        crate::curation::newest_item_curation("i.id", MAPPED)
    )
}

const MAPPED: &str = "COALESCE(k.is_mapping_complete, false)";

/// The columns of [`ItemRow`] for an item read by itself: [`columns`] and
/// the curation of its collection's first curated item.
pub(crate) fn by_id_columns() -> String {
    format!(
        "{}, {} AS collection_curation_mapped",
        columns(),
        crate::curation::first_curated_item_curation("i.collection_id", MAPPED)
    )
}

#[derive(Debug, sqlx::FromRow)]
pub(crate) struct ItemRow {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub eth_address: String,
    pub collection_id: Option<Uuid>,
    pub blockchain_item_id: Option<String>,
    pub price: Option<String>,
    pub beneficiary: Option<String>,
    pub rarity: Option<String>,
    pub item_type: String,
    pub data: String,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
    pub metrics: Option<String>,
    pub thumbnail: Option<String>,
    pub contents: String,
    pub local_content_hash: Option<String>,
    pub video: Option<String>,
    pub utility: Option<String>,
    pub mappings: Option<String>,
    pub urn_suffix: Option<String>,
    pub submitted: bool,
    pub own_curation_mapped: Option<bool>,
    #[sqlx(default)]
    pub collection_curation_mapped: Option<bool>,
    pub third_party_id: Option<String>,
    pub collection_urn: Option<String>,
    pub contract_address: Option<String>,
    pub chain_creator: Option<String>,
    pub chain_approved: Option<bool>,
    pub chain_urn: Option<String>,
    pub chain_item_id: Option<String>,
    pub chain_price: Option<String>,
    pub chain_beneficiary: Option<String>,
    pub chain_rarity: Option<String>,
    pub chain_item_type: Option<String>,
    pub chain_supply: Option<String>,
    pub chain_content_hash: Option<String>,
    pub catalyst_entity: Option<String>,
    pub catalyst_id: Option<String>,
    pub catalyst_proof: Option<String>,
}

/// An item's body (builder-server's `FullItem`; `PublicItem` without
/// `local_content_hash`).
#[derive(Debug, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, export_to = "builder/"))]
pub struct FullItemOut {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub eth_address: String,
    pub collection_id: Option<String>,
    pub blockchain_item_id: Option<String>,
    pub price: Option<String>,
    pub beneficiary: Option<String>,
    pub rarity: Option<String>,
    #[serde(rename = "type")]
    pub item_type: String,
    #[cfg_attr(feature = "ts", ts(type = "Record<string, unknown>"))]
    pub data: Box<RawValue>,
    pub created_at: String,
    pub updated_at: String,
    #[cfg_attr(feature = "ts", ts(type = "Record<string, unknown> | null"))]
    pub metrics: Option<Box<RawValue>>,
    pub thumbnail: Option<String>,
    #[cfg_attr(feature = "ts", ts(type = "Record<string, string>"))]
    pub contents: Box<RawValue>,
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub total_supply: serde_json::Number,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional, type = "string | null"))]
    pub local_content_hash: Option<Option<String>>,
    pub video: Option<String>,
    pub utility: Option<String>,
    #[cfg_attr(feature = "ts", ts(type = "Record<string, unknown> | null"))]
    pub mappings: Option<Box<RawValue>>,
    pub urn: Option<String>,
    pub in_catalyst: bool,
    pub is_approved: bool,
    pub is_published: bool,
    pub content_hash: Option<String>,
    pub catalyst_content_hash: Option<String>,
    #[serde(rename = "isMappingComplete", skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub is_mapping_complete: Option<bool>,
}

/// How builder-server reached the item: by itself (`getItem`, a save) it
/// names a minted standard item's URN even when the copy doesn't hold the
/// item; in a list (`consolidateItems`) it doesn't.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Reached {
    ById,
    InList,
}

fn raw(text: String) -> Result<Box<RawValue>, ApiError> {
    RawValue::from_string(text).map_err(|e| ApiError::internal(format!("stored item JSON: {e}")))
}

fn supply(text: Option<&str>) -> serde_json::Number {
    let value = text.unwrap_or("0");
    value
        .parse::<u64>()
        .map(serde_json::Number::from)
        .ok()
        .or_else(|| {
            value
                .parse::<f64>()
                .ok()
                .and_then(serde_json::Number::from_f64)
        })
        .unwrap_or_else(|| serde_json::Number::from(0))
}

impl ItemRow {
    /// Forgets the catalyst's entity when the node withholds it.
    pub(crate) fn withheld(mut self, denylist: &crate::content::Denylist) -> Self {
        if self
            .catalyst_entity
            .as_deref()
            .is_some_and(|entity| denylist.contains(entity))
        {
            self.catalyst_entity = None;
            self.catalyst_id = None;
            self.catalyst_proof = None;
        }
        self
    }

    fn third_party(&self) -> bool {
        self.third_party_id.is_some() && self.urn_suffix.is_some() && self.collection_urn.is_some()
    }

    /// The item part of a third-party item's URN.
    fn item_suffix(&self) -> Option<String> {
        let urn = self.urn_suffix.as_deref()?;
        let collection = self.collection_urn.as_deref()?;
        urn.strip_prefix(collection)?
            .strip_prefix(':')
            .map(str::to_string)
    }

    /// The body; `full` keeps what only the item's owner side reads.
    pub(crate) fn out(self, reached: Reached, full: bool) -> Result<FullItemOut, ApiError> {
        let third_party = self.third_party();
        let merged = !third_party && self.chain_item_id.is_some();
        let held = match (&self.catalyst_id, third_party) {
            (Some(id), true) => self.urn_suffix.as_deref() == Some(id.as_str()),
            (Some(id), false) => self
                .chain_urn
                .as_deref()
                .is_some_and(|urn| urn.eq_ignore_ascii_case(id)),
            (None, _) => false,
        };
        let dcl_urn = (!third_party
            && reached == Reached::ById
            && self.collection_id.is_some()
            && self.blockchain_item_id.is_some())
        .then(|| {
            format!(
                "urn:decentraland:matic:collections-v2:{}:{}",
                self.contract_address.as_deref().unwrap_or("null"),
                self.blockchain_item_id.as_deref().unwrap_or_default()
            )
        });
        let item_suffix = self.item_suffix();
        let mut out = FullItemOut {
            id: self.id.to_string(),
            name: self.name,
            description: self.description,
            eth_address: self.eth_address,
            collection_id: self.collection_id.map(|c| c.to_string()),
            blockchain_item_id: self.blockchain_item_id,
            price: self.price,
            beneficiary: self.beneficiary,
            rarity: self.rarity,
            item_type: self.item_type,
            data: raw(self.data)?,
            created_at: iso(&self.created_at.and_utc()),
            updated_at: iso(&self.updated_at.and_utc()),
            metrics: self.metrics.map(raw).transpose()?,
            thumbnail: self.thumbnail,
            contents: raw(self.contents)?,
            total_supply: serde_json::Number::from(0),
            local_content_hash: full.then_some(self.local_content_hash),
            video: self.video,
            utility: self.utility,
            mappings: self.mappings.as_ref().map(|m| raw(m.clone())).transpose()?,
            urn: dcl_urn,
            in_catalyst: false,
            is_approved: false,
            is_published: false,
            content_hash: None,
            catalyst_content_hash: None,
            is_mapping_complete: None,
        };
        if third_party {
            out.urn = self.urn_suffix;
            out.total_supply = serde_json::Number::from(0);
            out.price = Some("0".to_string());
            out.beneficiary = Some(ZERO_ADDRESS.to_string());
            out.blockchain_item_id = item_suffix;
            let curation = match reached {
                Reached::ById => self.collection_curation_mapped,
                Reached::InList => self.own_curation_mapped,
            };
            out.is_published = curation.is_some() || held;
            out.is_mapping_complete = Some(curation.unwrap_or(self.mappings.is_some()));
            if held {
                out.in_catalyst = true;
                out.is_approved = true;
                out.catalyst_content_hash = self.catalyst_proof;
            }
        } else if merged {
            if self.chain_item_type.as_deref() != Some("emote_v1") {
                out.rarity = self.chain_rarity.filter(|r| !r.is_empty()).or(out.rarity);
            }
            out.urn = if held {
                self.catalyst_id
            } else {
                self.chain_urn.or(out.urn)
            };
            out.in_catalyst = held;
            out.is_published = true;
            out.is_approved = self.chain_approved.unwrap_or(false);
            out.eth_address = self
                .chain_creator
                .unwrap_or(out.eth_address)
                .to_ascii_lowercase();
            out.price = self.chain_price;
            out.beneficiary = self.chain_beneficiary;
            out.blockchain_item_id = self.chain_item_id;
            out.total_supply = supply(self.chain_supply.as_deref());
            out.content_hash = self.chain_content_hash.filter(|h| !h.is_empty());
        } else if self.submitted {
            out.is_published = true;
            out.urn = self.urn_suffix.or(out.urn);
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row() -> ItemRow {
        ItemRow {
            id: Uuid::nil(),
            name: "Hat".into(),
            description: None,
            eth_address: "0xowner".into(),
            collection_id: Some(Uuid::nil()),
            blockchain_item_id: Some("0".into()),
            price: None,
            beneficiary: None,
            rarity: Some("common".into()),
            item_type: "wearable".into(),
            data: r#"{"tags":[],"category":"hat"}"#.into(),
            created_at: NaiveDateTime::default(),
            updated_at: NaiveDateTime::default(),
            metrics: None,
            thumbnail: None,
            contents: r#"{ "b.png" : "bafkb", "a.glb" : "bafka" }"#.into(),
            local_content_hash: Some("bafklocal".into()),
            video: None,
            utility: None,
            mappings: None,
            urn_suffix: None,
            submitted: false,
            own_curation_mapped: None,
            collection_curation_mapped: None,
            third_party_id: None,
            collection_urn: None,
            contract_address: Some("0xc0".into()),
            chain_creator: Some("0xCreator".into()),
            chain_approved: Some(true),
            chain_urn: Some("urn:decentraland:matic:collections-v2:0xc0:0".into()),
            chain_item_id: Some("0".into()),
            chain_price: Some("10".into()),
            chain_beneficiary: Some("0xbeneficiary".into()),
            chain_rarity: Some("rare".into()),
            chain_item_type: Some("wearable_v2".into()),
            chain_supply: Some("3".into()),
            chain_content_hash: Some(String::new()),
            catalyst_entity: None,
            catalyst_id: None,
            catalyst_proof: None,
        }
    }

    #[test]
    fn a_minted_item_the_copy_holds_takes_the_chain_values_and_keeps_its_documents_in_order() {
        let body = serde_json::to_string(&row().out(Reached::InList, false).unwrap()).unwrap();
        assert!(
            body.contains(r#""data":{"tags":[],"category":"hat"}"#),
            "{body}"
        );
        assert!(
            body.contains(r#""contents":{ "b.png" : "bafkb", "a.glb" : "bafka" }"#),
            "{body}"
        );
        let value: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(value["rarity"], "rare");
        assert_eq!(value["eth_address"], "0xcreator");
        assert_eq!(value["price"], "10");
        assert_eq!(value["total_supply"], 3);
        assert_eq!(value["content_hash"], serde_json::Value::Null);
        assert_eq!(value["is_published"], true);
        assert_eq!(value["is_approved"], true);
        assert!(value.get("local_content_hash").is_none());
        assert!(value.get("isMappingComplete").is_none());
    }

    #[test]
    fn a_minted_item_the_copy_lacks_has_a_urn_only_when_read_by_itself() {
        let mut r = row();
        r.chain_item_id = None;
        let mut s = row();
        s.chain_item_id = None;
        let by_id = r.out(Reached::ById, true).unwrap();
        assert_eq!(
            by_id.urn.as_deref(),
            Some("urn:decentraland:matic:collections-v2:0xc0:0")
        );
        assert!(!by_id.is_published);
        assert_eq!(by_id.local_content_hash, Some(Some("bafklocal".into())));
        assert_eq!(s.out(Reached::InList, true).unwrap().urn, None);
    }

    #[test]
    fn a_third_party_item_answers_its_urn_and_no_chain_values() {
        let mut r = row();
        r.third_party_id = Some("urn:decentraland:matic:collections-thirdparty:p".into());
        r.collection_urn = Some("urn:decentraland:matic:collections-thirdparty:p:c".into());
        r.urn_suffix = Some("urn:decentraland:matic:collections-thirdparty:p:c:hat".into());
        r.mappings = Some(r#"{"mainnet":{}}"#.into());
        let out = r.out(Reached::ById, true).unwrap();
        assert_eq!(out.blockchain_item_id.as_deref(), Some("hat"));
        assert_eq!(out.price.as_deref(), Some("0"));
        assert_eq!(out.beneficiary.as_deref(), Some(ZERO_ADDRESS));
        assert_eq!(out.is_mapping_complete, Some(true));
        assert!(!out.is_published);
        assert_eq!(out.rarity.as_deref(), Some("common"));
    }
}

//! The fold of the `collections` index's log (`shop.collection_event`) and
//! creation reads (`shop.collection_seed`) into the catalogue rows of
//! `shop.collections` and `shop.items`: a deterministic function of one
//! collection's walked events up to a block, its creation read, and two
//! aggregates of its mints (each item's highest issued id, the time of its
//! newest primary sale).
//!
//! On-chain state follows the contract (`ERC721BaseCollectionV2`):
//! - global and item minters and managers are sets: an account is one while
//!   its newest grant set it, listed in address order (so the rows are the
//!   same wherever a fold resumed);
//! - an item minter's allowance is its newest `SetItemMinter` value, less one
//!   for each later `Issue` it called while it was neither the creator nor a
//!   global minter, unless the allowance is `uint256` max (`_issueToken`);
//! - the store can sell an item while it is a global minter or holds an
//!   allowance for the item;
//! - an item's supply is its highest issued id (issues are numbered from 1).
//!
//! Item metadata follows the collections subgraph's format
//! (`version:type:name:description:category:bodyshapes[:…]`): `w`, `sw` and
//! `e` items, wearables with a known category and six or eight fields,
//! emotes with six to nine. Body shapes are taken as written (the subgraph's
//! check never runs), and an emote's unknown category is `dance`.

use std::collections::{BTreeMap, HashMap, HashSet};

use alloy_primitives::U256;

use super::collections_index::{
    CollectionEvent, KIND_ADD_ITEM, KIND_COMPLETE, KIND_CREATORSHIP_TRANSFERRED, KIND_ISSUE,
    KIND_OWNERSHIP_TRANSFERRED, KIND_PROXY_CREATED, KIND_RESCUE_ITEM, KIND_SET_APPROVED,
    KIND_SET_EDITABLE, KIND_SET_GLOBAL_MANAGER, KIND_SET_GLOBAL_MINTER, KIND_SET_ITEM_MINTER,
    KIND_UPDATE_ITEM_DATA, Seed,
};
use super::sales_index::STORE_MATIC;

pub const WEARABLE_V2: &str = "wearable_v2";
pub const SMART_WEARABLE_V1: &str = "smart_wearable_v1";
pub const EMOTE_V1: &str = "emote_v1";

pub const WEARABLE_CATEGORIES: [&str; 17] = [
    "eyebrows",
    "eyes",
    "facial_hair",
    "hair",
    "mouth",
    "upper_body",
    "lower_body",
    "feet",
    "earring",
    "eyewear",
    "hat",
    "helmet",
    "mask",
    "tiara",
    "top_head",
    "skin",
    "hands_wear",
];

pub const EMOTE_CATEGORIES: [&str; 8] = [
    "dance",
    "stunt",
    "greetings",
    "fun",
    "poses",
    "reactions",
    "horror",
    "miscellaneous",
];

/// The social emote outcome codes and the names stored for them.
pub const OUTCOMES: [(&str, &str); 3] = [
    ("so", "simple_outcome"),
    ("mo", "multiple_outcome"),
    ("ro", "random_outcome"),
];

/// An item's metadata string, decoded.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ItemMetadata {
    pub item_type: &'static str,
    pub name: String,
    pub description: String,
    pub category: String,
    pub body_shapes: Vec<String>,
    pub emote_loop: Option<bool>,
    pub emote_has_sound: Option<bool>,
    pub emote_has_geometry: Option<bool>,
    pub emote_outcome_type: Option<&'static str>,
}

/// `raw` as the catalogue reads it; `None` for a string no item type takes
/// (the subgraph's `undefined`, which the catalogue does not list).
pub fn parse_metadata(raw: &str) -> Option<ItemMetadata> {
    let data: Vec<&str> = raw.split(':').collect();
    let shapes = |s: &str| s.split(',').map(str::to_string).collect::<Vec<_>>();
    let item_type = match *data.get(1)? {
        "w" => WEARABLE_V2,
        "sw" => SMART_WEARABLE_V1,
        "e" => EMOTE_V1,
        _ => return None,
    };
    if item_type == EMOTE_V1 {
        if !(6..=9).contains(&data.len()) {
            return None;
        }
        let outcome = |s: &str| {
            OUTCOMES
                .iter()
                .find(|(code, _)| *code == s)
                .map(|(_, name)| *name)
        };
        let properties = data.get(7).copied().filter(|p| outcome(p).is_none());
        return Some(ItemMetadata {
            item_type,
            name: data[2].to_string(),
            description: data[3].to_string(),
            category: if EMOTE_CATEGORIES.contains(&data[4]) {
                data[4].to_string()
            } else {
                "dance".to_string()
            },
            body_shapes: shapes(data[5]),
            emote_loop: Some(data.get(6) == Some(&"1")),
            emote_has_sound: Some(properties.is_some_and(|p| p.contains('s'))),
            emote_has_geometry: Some(properties.is_some_and(|p| p.contains('g'))),
            emote_outcome_type: data
                .get(7)
                .and_then(|p| outcome(p))
                .or_else(|| data.get(8).and_then(|p| outcome(p))),
        });
    }
    if !(data.len() == 6 || data.len() == 8) || !WEARABLE_CATEGORIES.contains(&data[4]) {
        return None;
    }
    Some(ItemMetadata {
        item_type,
        name: data[2].to_string(),
        description: data[3].to_string(),
        category: data[4].to_string(),
        body_shapes: shapes(data[5]),
        emote_loop: None,
        emote_has_sound: None,
        emote_has_geometry: None,
        emote_outcome_type: None,
    })
}

/// One item's mints up to the fold's block, aggregated: its highest issued
/// id and the block time of its newest mint the store called.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Mints {
    pub max_issued: U256,
    pub primary_sold_at: Option<i64>,
}

/// One `shop.collections` row.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CollectionRow {
    pub contract_address: String,
    pub name: String,
    pub symbol: String,
    pub creator: String,
    pub owner: String,
    pub urn: String,
    pub is_completed: bool,
    pub is_approved: bool,
    pub is_editable: bool,
    pub items_count: i32,
    pub minters: Vec<String>,
    pub managers: Vec<String>,
    pub search_is_store_minter: bool,
    pub search_text: String,
    pub created_at: i64,
    pub updated_at: i64,
    pub reviewed_at: i64,
    pub first_listed_at: Option<i64>,
}

/// One `shop.items` row. `minters` are the item minters holding an
/// allowance, in the order first granted, and `minter_allowances` what each
/// holds; a later fold resumes from them.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ItemRow {
    pub item_id: U256,
    pub item_type: &'static str,
    pub urn: String,
    pub creator: String,
    pub beneficiary: String,
    pub rarity: String,
    pub total_supply: U256,
    pub max_supply: U256,
    pub available: U256,
    pub price: U256,
    pub image: String,
    pub name: String,
    pub description: String,
    pub category: String,
    pub body_shapes: Vec<String>,
    pub is_smart: bool,
    pub emote_loop: Option<bool>,
    pub emote_has_sound: Option<bool>,
    pub emote_has_geometry: Option<bool>,
    pub emote_outcome_type: Option<&'static str>,
    pub minters: Vec<String>,
    pub minter_allowances: Vec<U256>,
    pub search_is_store_minter: bool,
    pub search_is_collection_approved: bool,
    pub search_text: String,
    pub created_at: i64,
    pub updated_at: i64,
    pub reviewed_at: i64,
    pub sold_at: i64,
    pub first_listed_at: Option<i64>,
    pub content_hash: Option<String>,
}

/// Why a walked collection has no row at a block yet.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Pending {
    /// Its creation was not read, or was read past the block and an
    /// attribute only that read gives is needed.
    Seed,
    /// Its `ProxyCreated` is not in the events.
    NotCreated,
}

/// A resume that cannot go on from the rows: an event names an item with
/// no row (its metadata took no type, so its rarity and supply are not
/// kept). The collection is folded again from its creation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NeedsReplay;

/// An ordered set: insertion order, each member once.
#[derive(Clone, Default)]
struct Set(Vec<String>);

impl Set {
    fn put(&mut self, member: &str, on: bool) {
        let at = self.0.iter().position(|m| m == member);
        match (on, at) {
            (true, None) => self.0.push(member.to_string()),
            (false, Some(i)) => {
                self.0.remove(i);
            }
            _ => {}
        }
    }
    fn has(&self, member: &str) -> bool {
        self.0.iter().any(|m| m == member)
    }
}

/// An item being folded. `added_before` orders it against the events
/// applied: an item resumed from its row was added before all of them.
#[derive(Clone)]
struct ItemState {
    added_at: Option<(i64, i32)>,
    created_at: i64,
    updated_at: i64,
    reviewed_at: i64,
    rarity: String,
    max_supply: U256,
    price: U256,
    beneficiary: String,
    meta: Option<ItemMetadata>,
    max_issued: U256,
    sold_at: Option<i64>,
    /// Allowance per item minter, in the order first granted.
    allowances: Vec<(String, U256)>,
    /// The first block time the store could mint it.
    first_listed_at: Option<i64>,
    content_hash: Option<String>,
}

impl ItemState {
    fn added_before(&self, position: (i64, i32)) -> bool {
        self.added_at.is_none_or(|at| at < position)
    }
}

/// A collection being folded: the state the contract keeps, as the events
/// applied so far left it.
struct CollectionState {
    contract: String,
    name: String,
    symbol: String,
    creator: String,
    owner: String,
    approved: bool,
    editable: bool,
    completed: bool,
    items_count: i32,
    minters: Set,
    managers: Set,
    store_global: bool,
    created_at: i64,
    updated_at: i64,
    reviewed_at: i64,
    first_listed_at: Option<i64>,
    items: BTreeMap<U256, ItemState>,
    /// Resumed from rows: an event naming an item it lacks cannot be applied.
    resumed: bool,
}

fn u256(v: &Option<String>) -> U256 {
    v.as_deref()
        .and_then(|s| U256::from_str_radix(s, 10).ok())
        .unwrap_or_default()
}

fn decimal(v: &Option<String>) -> Option<U256> {
    v.as_deref().and_then(|s| U256::from_str_radix(s, 10).ok())
}

/// The catalogue base of an item's thumbnail, as the subgraph writes it.
pub const IMAGE_BASE: &str = "https://peer.decentraland.org/lambdas/collections/contents";

pub fn collection_urn(contract: &str) -> String {
    format!("urn:decentraland:matic:collections-v2:{contract}")
}

impl CollectionState {
    fn item(&mut self, id: Option<U256>) -> Result<Option<&mut ItemState>, NeedsReplay> {
        let Some(id) = id else { return Ok(None) };
        match self.items.get_mut(&id) {
            Some(item) => Ok(Some(item)),
            None if self.resumed => Err(NeedsReplay),
            None => Ok(None),
        }
    }

    fn apply(&mut self, e: &CollectionEvent) -> Result<(), NeedsReplay> {
        let id = decimal(&e.item_id);
        let position = (e.block_number, e.log_index);
        let t = e.block_time;
        let store = STORE_MATIC;
        match e.kind {
            KIND_ADD_ITEM => {
                let Some(id) = id else { return Ok(()) };
                if self.items.contains_key(&id) {
                    return Ok(());
                }
                self.items_count += 1;
                let store_global = self.store_global;
                self.items.insert(
                    id,
                    ItemState {
                        added_at: Some(position),
                        created_at: t,
                        updated_at: t,
                        reviewed_at: t,
                        rarity: e.rarity.clone().unwrap_or_default().to_lowercase(),
                        max_supply: u256(&e.max_supply),
                        price: u256(&e.amount),
                        beneficiary: e.beneficiary.clone().unwrap_or_default(),
                        meta: parse_metadata(e.metadata.as_deref().unwrap_or_default()),
                        max_issued: U256::ZERO,
                        sold_at: None,
                        allowances: Vec::new(),
                        first_listed_at: store_global.then_some(t),
                        content_hash: e.content_hash.clone(),
                    },
                );
            }
            KIND_RESCUE_ITEM => {
                if let Some(item) = self.item(id)? {
                    item.meta = parse_metadata(e.metadata.as_deref().unwrap_or_default());
                    item.content_hash = e.content_hash.clone();
                    item.updated_at = t;
                }
                self.updated_at = t;
                self.reviewed_at = t;
            }
            KIND_UPDATE_ITEM_DATA => {
                if let Some(item) = self.item(id)? {
                    item.price = u256(&e.amount);
                    item.beneficiary = e.beneficiary.clone().unwrap_or_default();
                    item.meta = parse_metadata(e.metadata.as_deref().unwrap_or_default());
                    item.updated_at = t;
                }
            }
            KIND_ISSUE => {
                let caller = e.account.clone().unwrap_or_default();
                let exempt = caller == self.creator || self.minters.has(&caller);
                if let Some(item) = self.item(id)? {
                    item.max_issued = item.max_issued.max(u256(&e.amount));
                    if caller == store {
                        item.sold_at = Some(item.sold_at.map_or(t, |s| s.max(t)));
                    }
                    if !exempt
                        && let Some((_, left)) =
                            item.allowances.iter_mut().find(|(m, _)| *m == caller)
                        && *left != U256::MAX
                    {
                        *left = left.saturating_sub(U256::from(1));
                    }
                }
            }
            KIND_SET_GLOBAL_MINTER => {
                let (Some(who), Some(on)) = (e.account.as_deref(), e.flag) else {
                    return Ok(());
                };
                self.minters.put(who, on);
                if who == store {
                    self.store_global = on;
                    if on {
                        self.first_listed_at.get_or_insert(t);
                        for item in self.items.values_mut() {
                            item.first_listed_at.get_or_insert(t);
                        }
                    }
                }
            }
            KIND_SET_GLOBAL_MANAGER => {
                if let (Some(who), Some(on)) = (e.account.as_deref(), e.flag) {
                    self.managers.put(who, on);
                }
            }
            KIND_SET_ITEM_MINTER => {
                let who = e.account.clone().unwrap_or_default();
                let value = u256(&e.amount);
                if let Some(item) = self.item(id)? {
                    match item.allowances.iter_mut().find(|(m, _)| *m == who) {
                        Some((_, left)) => *left = value,
                        None => item.allowances.push((who.clone(), value)),
                    }
                    if who == store && !value.is_zero() {
                        item.first_listed_at.get_or_insert(t);
                    }
                }
            }
            KIND_SET_APPROVED => {
                if let Some(on) = e.flag {
                    self.approved = on;
                }
                self.updated_at = t;
                self.reviewed_at = t;
                for item in self.items.values_mut() {
                    if item.added_before(position) {
                        item.reviewed_at = t;
                    }
                }
            }
            KIND_SET_EDITABLE => {
                if let Some(on) = e.flag {
                    self.editable = on;
                }
            }
            KIND_COMPLETE => self.completed = true,
            KIND_CREATORSHIP_TRANSFERRED => {
                if let Some(next) = &e.account {
                    self.creator = next.clone();
                }
                self.updated_at = t;
                for item in self.items.values_mut() {
                    if item.added_before(position) {
                        item.updated_at = t;
                    }
                }
            }
            KIND_OWNERSHIP_TRANSFERRED => {
                if let Some(next) = &e.account {
                    self.owner = next.clone();
                }
                self.updated_at = t;
            }
            _ => {}
        }
        Ok(())
    }

    fn rows(self) -> (CollectionRow, Vec<ItemRow>) {
        let urn = collection_urn(&self.contract);
        let store = STORE_MATIC;
        let (creator, approved, store_global) =
            (self.creator.clone(), self.approved, self.store_global);
        let items = self
            .items
            .into_iter()
            .filter_map(|(id, item)| {
                let meta = item.meta?;
                let item_urn = format!("{urn}:{id}");
                let mut held: Vec<(String, U256)> = item
                    .allowances
                    .into_iter()
                    .filter(|(_, left)| !left.is_zero())
                    .collect();
                // In address order: the same rows wherever a fold resumed.
                held.sort();
                Some(ItemRow {
                    item_id: id,
                    item_type: meta.item_type,
                    image: format!("{IMAGE_BASE}/{item_urn}/thumbnail"),
                    urn: item_urn,
                    creator: creator.clone(),
                    beneficiary: item.beneficiary,
                    rarity: item.rarity,
                    total_supply: item.max_issued,
                    available: item.max_supply.saturating_sub(item.max_issued),
                    max_supply: item.max_supply,
                    price: item.price,
                    search_text: format!("{} {}", meta.name, meta.description).to_ascii_lowercase(),
                    name: meta.name,
                    description: meta.description,
                    category: meta.category,
                    body_shapes: meta.body_shapes,
                    is_smart: meta.item_type == SMART_WEARABLE_V1,
                    emote_loop: meta.emote_loop,
                    emote_has_sound: meta.emote_has_sound,
                    emote_has_geometry: meta.emote_has_geometry,
                    emote_outcome_type: meta.emote_outcome_type,
                    search_is_store_minter: store_global || held.iter().any(|(m, _)| m == store),
                    minters: held.iter().map(|(m, _)| m.clone()).collect(),
                    minter_allowances: held.iter().map(|(_, left)| *left).collect(),
                    search_is_collection_approved: approved,
                    created_at: item.created_at,
                    updated_at: item.updated_at,
                    reviewed_at: item.reviewed_at,
                    sold_at: item.sold_at.unwrap_or(0),
                    first_listed_at: item.first_listed_at,
                    content_hash: item.content_hash,
                })
            })
            .collect();
        let (mut minters, mut managers) = (self.minters.0, self.managers.0);
        minters.sort();
        managers.sort();
        let collection = CollectionRow {
            urn,
            search_text: self.name.to_ascii_lowercase(),
            contract_address: self.contract,
            name: self.name,
            symbol: self.symbol,
            creator: self.creator,
            owner: self.owner,
            is_completed: self.completed,
            is_approved: self.approved,
            is_editable: self.editable,
            items_count: self.items_count,
            search_is_store_minter: store_global,
            minters,
            managers,
            created_at: self.created_at,
            updated_at: self.updated_at,
            reviewed_at: self.reviewed_at,
            first_listed_at: self.first_listed_at,
        };
        (collection, items)
    }
}

/// Folds `contract` from its creation: its `events` (every kind but
/// `issue`, plus the issues of item minters holding a finite allowance, in
/// chain order, all at or below `upto`), its creation read and its items'
/// mints. Items whose metadata no type takes have no row.
pub fn fold(
    contract: &str,
    events: &[CollectionEvent],
    seed: Option<&Seed>,
    mints: &HashMap<U256, Mints>,
    upto: i64,
) -> Result<(CollectionRow, Vec<ItemRow>), Pending> {
    let created = events
        .iter()
        .find(|e| e.kind == KIND_PROXY_CREATED)
        .ok_or(Pending::NotCreated)?;
    let seed = seed.ok_or(Pending::Seed)?;
    let seed_usable = seed.read_block <= upto;
    let has = |kind: &str| events.iter().any(|e| e.kind == kind);
    if !seed_usable
        && !(has(KIND_CREATORSHIP_TRANSFERRED) && has(KIND_SET_APPROVED) && has(KIND_SET_EDITABLE))
    {
        return Err(Pending::Seed);
    }
    // The creator before the first transfer is the one it names: a read
    // taken later may already hold a newer one.
    let creator = events
        .iter()
        .find(|e| e.kind == KIND_CREATORSHIP_TRANSFERRED)
        .and_then(|e| e.previous_account.clone())
        .unwrap_or_else(|| seed.creator.clone());
    let mut state = CollectionState {
        contract: contract.to_string(),
        name: seed.name.clone(),
        symbol: seed.symbol.clone(),
        creator,
        owner: String::from("0x0000000000000000000000000000000000000000"),
        approved: seed.is_approved,
        editable: seed.is_editable,
        completed: false,
        items_count: 0,
        minters: Set::default(),
        managers: Set::default(),
        store_global: false,
        created_at: created.block_time,
        updated_at: created.block_time,
        reviewed_at: created.block_time,
        first_listed_at: None,
        items: BTreeMap::new(),
        resumed: false,
    };
    for e in events {
        // A replay knows every item, so no event asks for one.
        let _ = state.apply(e);
    }
    for (id, m) in mints {
        if let Some(item) = state.items.get_mut(id) {
            item.max_issued = item.max_issued.max(m.max_issued);
            item.sold_at = match (item.sold_at, m.primary_sold_at) {
                (Some(a), Some(b)) => Some(a.max(b)),
                (a, b) => a.or(b),
            };
        }
    }
    Ok(state.rows())
}

/// Folds `events` (every kind, mints included, in chain order, all past the
/// block `collection` and `items` were folded to) onto those rows: the rows
/// of the collection at the newest event. An event naming an item without a
/// row asks for a fold from the creation instead.
pub fn resume(
    collection: CollectionRow,
    items: Vec<ItemRow>,
    events: &[CollectionEvent],
) -> Result<(CollectionRow, Vec<ItemRow>), NeedsReplay> {
    let mut state = CollectionState {
        contract: collection.contract_address,
        name: collection.name,
        symbol: collection.symbol,
        creator: collection.creator,
        owner: collection.owner,
        approved: collection.is_approved,
        editable: collection.is_editable,
        completed: collection.is_completed,
        items_count: collection.items_count,
        minters: Set(collection.minters),
        managers: Set(collection.managers),
        store_global: collection.search_is_store_minter,
        created_at: collection.created_at,
        updated_at: collection.updated_at,
        reviewed_at: collection.reviewed_at,
        first_listed_at: collection.first_listed_at,
        items: items
            .into_iter()
            .map(|row| {
                let item = ItemState {
                    added_at: None,
                    created_at: row.created_at,
                    updated_at: row.updated_at,
                    reviewed_at: row.reviewed_at,
                    rarity: row.rarity,
                    max_supply: row.max_supply,
                    price: row.price,
                    beneficiary: row.beneficiary,
                    meta: Some(ItemMetadata {
                        item_type: row.item_type,
                        name: row.name,
                        description: row.description,
                        category: row.category,
                        body_shapes: row.body_shapes,
                        emote_loop: row.emote_loop,
                        emote_has_sound: row.emote_has_sound,
                        emote_has_geometry: row.emote_has_geometry,
                        emote_outcome_type: row.emote_outcome_type,
                    }),
                    max_issued: row.total_supply,
                    sold_at: (row.sold_at != 0).then_some(row.sold_at),
                    allowances: row.minters.into_iter().zip(row.minter_allowances).collect(),
                    first_listed_at: row.first_listed_at,
                    content_hash: row.content_hash,
                };
                (row.item_id, item)
            })
            .collect(),
        resumed: true,
    };
    for e in events {
        state.apply(e)?;
    }
    Ok(state.rows())
}

/// The (item, minter) pairs whose issues a fold from the creation must
/// replay: every item minter granted a finite allowance in `events`.
pub fn tracked_minters(events: &[CollectionEvent]) -> HashSet<(String, String)> {
    events
        .iter()
        .filter(|e| e.kind == KIND_SET_ITEM_MINTER && u256(&e.amount) != U256::MAX)
        .filter_map(|e| Some((e.item_id.clone()?, e.account.clone()?)))
        .collect()
}

#[cfg(test)]
mod tests {
    #![allow(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        clippy::cast_possible_wrap,
        clippy::cast_precision_loss,
        reason = "test fixtures use small literal values"
    )]

    use super::*;
    use crate::chain_index::rpc::RpcLog;

    const C: &str = "0x00000000000000000000000000000000000000c0";
    const CREATOR: &str = "0x00000000000000000000000000000000000000c7";
    const MINTER: &str = "0x00000000000000000000000000000000000000a1";
    const OTHER: &str = "0x00000000000000000000000000000000000000a2";
    const HAT: &str = "1:w:Hat:a hat:hat:BaseMale";

    fn ev(kind: &'static str, block: i64) -> CollectionEvent {
        CollectionEvent {
            block_number: block,
            log_index: 0,
            block_time: 1_000 + block,
            kind,
            contract_address: C.into(),
            ..Default::default()
        }
    }

    fn add(block: i64, item: u64, metadata: &str) -> CollectionEvent {
        CollectionEvent {
            item_id: Some(item.to_string()),
            rarity: Some("Epic".into()),
            max_supply: Some("1000".into()),
            amount: Some("5".into()),
            beneficiary: Some(CREATOR.into()),
            metadata: Some(metadata.into()),
            content_hash: Some(String::new()),
            ..ev(KIND_ADD_ITEM, block)
        }
    }

    fn grant(kind: &'static str, block: i64, who: &str, on: bool) -> CollectionEvent {
        CollectionEvent {
            account: Some(who.into()),
            flag: Some(on),
            ..ev(kind, block)
        }
    }

    fn item_minter(block: i64, item: u64, who: &str, value: U256) -> CollectionEvent {
        CollectionEvent {
            item_id: Some(item.to_string()),
            account: Some(who.into()),
            amount: Some(value.to_string()),
            ..ev(KIND_SET_ITEM_MINTER, block)
        }
    }

    fn issue(block: i64, item: u64, caller: &str) -> CollectionEvent {
        CollectionEvent {
            item_id: Some(item.to_string()),
            account: Some(caller.into()),
            amount: Some("1".into()),
            ..ev(KIND_ISSUE, block)
        }
    }

    fn created(block: i64) -> Vec<CollectionEvent> {
        vec![
            CollectionEvent {
                account: Some(super::super::collections_index::FACTORIES[1].0.into()),
                ..ev(KIND_PROXY_CREATED, block)
            },
            CollectionEvent {
                account: Some("0x00000000000000000000000000000000000000f0".into()),
                previous_account: Some("0x0000000000000000000000000000000000000000".into()),
                ..ev(KIND_OWNERSHIP_TRANSFERRED, block)
            },
        ]
    }

    fn seed(read_block: i64) -> Seed {
        Seed {
            contract_address: C.into(),
            name: "My Collection".into(),
            symbol: "DCL-MC".into(),
            creator: CREATOR.into(),
            is_approved: false,
            is_editable: true,
            read_block,
        }
    }

    fn folded(events: Vec<CollectionEvent>) -> (CollectionRow, Vec<ItemRow>) {
        fold(C, &events, Some(&seed(1)), &HashMap::new(), 1_000).unwrap()
    }

    #[test]
    fn metadata_follows_the_subgraph_format() {
        let hat = parse_metadata("1:w:Hat:a hat:hat:BaseMale,BaseFemale").unwrap();
        assert_eq!(
            (
                hat.item_type,
                hat.name.as_str(),
                hat.description.as_str(),
                hat.category.as_str()
            ),
            (WEARABLE_V2, "Hat", "a hat", "hat")
        );
        assert_eq!(hat.body_shapes, ["BaseMale", "BaseFemale"]);
        assert_eq!((hat.emote_loop, hat.emote_outcome_type), (None, None));
        assert!(
            parse_metadata("1:w:Hat:d:hat:BaseMale:prevhash:1700000000").is_some(),
            "a rescued wearable's 8 fields"
        );
        assert!(
            parse_metadata("1:w:Hat:d:hat:BaseMale:extra").is_none(),
            "7 fields"
        );
        assert!(
            parse_metadata("1:w:Hat:d:cape:BaseMale").is_none(),
            "unknown category"
        );
        assert!(
            parse_metadata("1:w:Ha:t:d:hat:BaseMale").is_none(),
            "a colon in the name"
        );
        assert_eq!(
            parse_metadata("1:sw:Wand:d:hands_wear:Other")
                .unwrap()
                .item_type,
            SMART_WEARABLE_V1
        );
        assert_eq!(
            parse_metadata("1:sw:Wand:d:hands_wear:Other")
                .unwrap()
                .body_shapes,
            ["Other"],
            "shapes as written"
        );
        assert!(parse_metadata("1:x:A:d:hat:BaseMale").is_none());
        assert!(parse_metadata("").is_none());

        let plain = parse_metadata("1:e:Wave:hi:greetings:BaseMale").unwrap();
        assert_eq!(
            (
                plain.item_type,
                plain.emote_loop,
                plain.emote_has_sound,
                plain.emote_has_geometry,
                plain.emote_outcome_type
            ),
            (EMOTE_V1, Some(false), Some(false), Some(false), None)
        );
        assert_eq!(
            parse_metadata("1:e:Wave:hi:greetings:BaseMale:1")
                .unwrap()
                .emote_loop,
            Some(true)
        );
        assert_eq!(
            parse_metadata("1:e:Wave:hi:greetings:BaseMale:2")
                .unwrap()
                .emote_loop,
            Some(false)
        );
        let props = parse_metadata("1:e:Wave:hi:greetings:BaseMale:0:gs").unwrap();
        assert_eq!(
            (
                props.emote_has_geometry,
                props.emote_has_sound,
                props.emote_outcome_type
            ),
            (Some(true), Some(true), None)
        );
        let social = parse_metadata("1:e:Hug:hi:fun:BaseMale:0:mo").unwrap();
        assert_eq!(
            (
                social.emote_has_geometry,
                social.emote_has_sound,
                social.emote_outcome_type
            ),
            (Some(false), Some(false), Some("multiple_outcome"))
        );
        let both = parse_metadata("1:e:Hug:hi:fun:BaseMale:0:s:ro").unwrap();
        assert_eq!(
            (both.emote_has_sound, both.emote_outcome_type),
            (Some(true), Some("random_outcome"))
        );
        assert_eq!(
            parse_metadata("1:e:Old:hi:tricks:BaseMale")
                .unwrap()
                .category,
            "dance",
            "an old category"
        );
        assert!(parse_metadata("1:e:Wave:hi:greetings").is_none());
        assert!(parse_metadata("1:e:Wave:hi:greetings:BaseMale:1:so:x:y").is_none());
    }

    /// The real collection of the walk's fixture, folded from its creation
    /// to block 97,895,000.
    #[test]
    fn the_fixture_collection_folds_to_its_catalogue_rows() {
        let logs: Vec<RpcLog> = serde_json::from_str(include_str!(concat!(
            env!("DCL_ONE_ROOT"),
            "/services/catalyst/tests/fixtures/shop/collections-polygon-logs.json"
        )))
        .unwrap();
        let contract = "0x5f255679f7192bae8440746311757c4a5f752107";
        let creator = "0x23be90335e79d3245615985d91f69814f98a0fab";
        let events: Vec<CollectionEvent> = logs
            .iter()
            .filter_map(|l| {
                let block = super::super::chain_walk::log_position(l).0;
                super::super::collections_index::decode_log(l, 1_600_000_000 + block).unwrap()
            })
            .filter(|e| e.contract_address == contract)
            .collect();
        let mut mints: HashMap<U256, Mints> = HashMap::new();
        for e in events.iter().filter(|e| e.kind == KIND_ISSUE) {
            let m = mints.entry(u256(&e.item_id)).or_default();
            m.max_issued = m.max_issued.max(u256(&e.amount));
        }
        let walked: Vec<CollectionEvent> = events
            .iter()
            .filter(|e| e.kind != KIND_ISSUE)
            .cloned()
            .collect();
        let read = Seed {
            contract_address: contract.into(),
            name: "World Track Memorabilia".into(),
            symbol: "DCL-WRLDTRCKMMRBL".into(),
            creator: creator.into(),
            is_approved: true,
            is_editable: true,
            read_block: 97_800_000,
        };
        let (c, items) = fold(contract, &walked, Some(&read), &mints, 97_895_000).unwrap();
        assert_eq!(c.name, "World Track Memorabilia");
        assert_eq!(c.search_text, "world track memorabilia");
        assert_eq!(
            (c.creator.as_str(), c.owner.as_str()),
            (creator, "0xbf6755a83c0dcdbb2933a96ea778e00b717d7004")
        );
        assert_eq!(
            c.urn,
            format!("urn:decentraland:matic:collections-v2:{contract}")
        );
        assert!(c.is_completed && c.is_approved && c.is_editable);
        assert_eq!(c.items_count, 2);
        assert_eq!(
            c.minters,
            [
                "0xc36f78a5a475576edaf6c9010ddb5c968fb0a0de",
                "0xe982914373e6c7b77c86f8be80a9c29fafc8e911"
            ]
        );
        assert!(!c.search_is_store_minter && c.first_listed_at.is_none());
        assert_eq!(c.created_at, 1_600_000_000 + 97_707_994);
        assert_eq!(
            c.reviewed_at,
            1_600_000_000 + 97_780_435,
            "the newest rescue"
        );
        assert_eq!(items.len(), 2);
        let helmet = &items[0];
        assert_eq!(
            helmet.urn,
            format!("urn:decentraland:matic:collections-v2:{contract}:0")
        );
        assert_eq!(
            helmet.image,
            format!(
                "https://peer.decentraland.org/lambdas/collections/contents/{}/thumbnail",
                helmet.urn
            )
        );
        assert_eq!(
            (
                helmet.item_type,
                helmet.name.as_str(),
                helmet.category.as_str(),
                helmet.rarity.as_str()
            ),
            (WEARABLE_V2, "Vintage Leather Helmet", "helmet", "epic")
        );
        assert_eq!(
            helmet.search_text,
            "vintage leather helmet vintage racing leather with stitched detail and warm nickel."
        );
        assert_eq!(
            (helmet.total_supply, helmet.max_supply, helmet.available),
            (U256::from(3), U256::from(1000), U256::from(997))
        );
        assert_eq!(helmet.price, U256::MAX);
        assert!(helmet.search_is_collection_approved && !helmet.search_is_store_minter);
        assert_eq!(
            helmet.reviewed_at,
            1_600_000_000 + 97_732_808,
            "approval after the add"
        );
        assert_eq!(
            helmet.updated_at,
            1_600_000_000 + 97_780_435,
            "the rescue of item 0"
        );
        let goggles = &items[1];
        assert_eq!(
            (
                goggles.rarity.as_str(),
                goggles.total_supply,
                goggles.available
            ),
            ("legendary", U256::from(2), U256::from(98))
        );
        assert_eq!(goggles.updated_at, 1_600_000_000 + 97_732_792);
    }

    /// An item's content hash is the one its newest `AddItem` or
    /// `RescueItem` set, as the subgraph's `contentHash`; a data update
    /// keeps it.
    #[test]
    fn an_items_content_hash_is_its_newest_add_or_rescue() {
        let mut events = created(10);
        events.extend([
            CollectionEvent {
                content_hash: Some("bafkreiadded".into()),
                ..add(11, 0, HAT)
            },
            CollectionEvent {
                item_id: Some("0".into()),
                metadata: Some(HAT.into()),
                content_hash: Some("bafkreirescued".into()),
                ..ev(KIND_RESCUE_ITEM, 12)
            },
            CollectionEvent {
                item_id: Some("0".into()),
                amount: Some("9".into()),
                beneficiary: Some(OTHER.into()),
                metadata: Some(HAT.into()),
                ..ev(KIND_UPDATE_ITEM_DATA, 13)
            },
        ]);
        let (_, items) = folded(events);
        assert_eq!(items[0].content_hash.as_deref(), Some("bafkreirescued"));
    }

    /// Intended difference: a minter or manager is listed once while its
    /// newest grant set it, in address order (the subgraph appends on every
    /// grant, in grant order).
    #[test]
    fn minters_and_managers_are_sets() {
        let mut events = created(10);
        events.extend([
            grant(KIND_SET_GLOBAL_MINTER, 11, MINTER, true),
            grant(KIND_SET_GLOBAL_MINTER, 12, OTHER, true),
            grant(KIND_SET_GLOBAL_MINTER, 13, MINTER, false),
            grant(KIND_SET_GLOBAL_MINTER, 14, MINTER, true),
            grant(KIND_SET_GLOBAL_MANAGER, 15, OTHER, true),
            grant(KIND_SET_GLOBAL_MANAGER, 16, OTHER, true),
        ]);
        let (c, _) = folded(events);
        assert_eq!(c.minters, [MINTER, OTHER], "in address order");
        assert_eq!(c.managers, [OTHER]);
    }

    /// Intended difference: the store sells an item while it is a global
    /// minter or holds an allowance for it; a global revoke leaves an item
    /// it still holds an allowance for on sale (the subgraph's revoke reads
    /// the wrong index), and an item added while the store is a global
    /// minter is on sale and listed from its addition.
    #[test]
    fn the_store_flag_follows_the_contract() {
        let mut events = created(10);
        events.extend([
            add(11, 0, HAT),
            add(12, 1, HAT),
            item_minter(13, 0, STORE_MATIC, U256::MAX),
        ]);
        let (c, items) = folded(events.clone());
        assert!(!c.search_is_store_minter);
        assert_eq!(
            (
                items[0].search_is_store_minter,
                items[1].search_is_store_minter
            ),
            (true, false)
        );
        assert_eq!(items[0].minters, [STORE_MATIC]);
        assert_eq!(items[0].first_listed_at, Some(1_013));

        events.push(grant(KIND_SET_GLOBAL_MINTER, 14, STORE_MATIC, true));
        events.push(add(15, 2, HAT));
        let (c, items) = folded(events.clone());
        assert!(c.search_is_store_minter);
        assert_eq!(c.first_listed_at, Some(1_014));
        assert!(items.iter().all(|i| i.search_is_store_minter));
        assert_eq!(
            items.iter().map(|i| i.first_listed_at).collect::<Vec<_>>(),
            [Some(1_013), Some(1_014), Some(1_015)]
        );

        events.push(grant(KIND_SET_GLOBAL_MINTER, 16, STORE_MATIC, false));
        let (c, items) = folded(events);
        assert!(!c.search_is_store_minter);
        assert_eq!(
            items
                .iter()
                .map(|i| i.search_is_store_minter)
                .collect::<Vec<_>>(),
            [true, false, false]
        );
    }

    /// Intended difference: each Issue an item minter calls spends one of a
    /// finite allowance, as `_issueToken` does, unless it called while it was
    /// the creator or a global minter; an exhausted minter is no item minter.
    #[test]
    fn item_allowances_are_spent_per_issue() {
        let mut events = created(10);
        events.extend([
            add(11, 0, HAT),
            item_minter(12, 0, MINTER, U256::from(3)),
            item_minter(13, 0, OTHER, U256::MAX),
            issue(14, 0, MINTER),
            issue(15, 0, OTHER),
            grant(KIND_SET_GLOBAL_MINTER, 16, MINTER, true),
            issue(17, 0, MINTER),
            grant(KIND_SET_GLOBAL_MINTER, 18, MINTER, false),
            CollectionEvent {
                account: Some(MINTER.into()),
                previous_account: Some(CREATOR.into()),
                ..ev(KIND_CREATORSHIP_TRANSFERRED, 19)
            },
            issue(20, 0, MINTER),
            CollectionEvent {
                account: Some(CREATOR.into()),
                previous_account: Some(MINTER.into()),
                ..ev(KIND_CREATORSHIP_TRANSFERRED, 21)
            },
            issue(22, 0, MINTER),
        ]);
        assert_eq!(
            tracked_minters(&events),
            HashSet::from([("0".to_string(), MINTER.to_string())])
        );
        let (_, items) = folded(events.clone());
        assert_eq!(items[0].minters, [MINTER, OTHER], "3 - 2: one left");
        events.push(issue(23, 0, MINTER));
        let (_, items) = folded(events);
        assert_eq!(items[0].minters, [OTHER], "spent");
    }

    /// Values initialize set without an event come from the creation read
    /// only when it was read at or below the fold's block; an event that
    /// sets the value makes the read unnecessary for it.
    #[test]
    fn creation_reads_serve_only_at_or_below_their_block() {
        let events = created(10);
        let mints = HashMap::new();
        assert_eq!(
            fold(C, &events, None, &mints, 1_000).unwrap_err(),
            Pending::Seed
        );
        assert_eq!(
            fold(C, &events, Some(&seed(1_001)), &mints, 1_000).unwrap_err(),
            Pending::Seed
        );
        let (c, _) = fold(C, &events, Some(&seed(1_000)), &mints, 1_000).unwrap();
        assert_eq!(
            (c.creator.as_str(), c.is_approved, c.is_editable),
            (CREATOR, false, true)
        );

        let mut set = events.clone();
        set.extend([
            CollectionEvent {
                account: Some(MINTER.into()),
                previous_account: Some(OTHER.into()),
                ..ev(KIND_CREATORSHIP_TRANSFERRED, 11)
            },
            grant(KIND_SET_APPROVED, 12, MINTER, true),
            grant(KIND_SET_EDITABLE, 13, MINTER, false),
        ]);
        let (c, _) = fold(C, &set, Some(&seed(5_000)), &mints, 1_000).unwrap();
        assert_eq!(
            (c.creator.as_str(), c.is_approved, c.is_editable),
            (MINTER, true, false)
        );
        assert_eq!(
            fold(C, &events[1..], Some(&seed(1)), &mints, 1_000).unwrap_err(),
            Pending::NotCreated
        );
    }

    /// The fold of `all` (mints included) from the creation, as a projection
    /// computes it: every kind but the mints, the tracked minters' mints,
    /// and the mints aggregated per item.
    fn fold_all(all: &[CollectionEvent], read: &Seed, upto: i64) -> (CollectionRow, Vec<ItemRow>) {
        let tracked = tracked_minters(all);
        let mut mints: HashMap<U256, Mints> = HashMap::new();
        for e in all.iter().filter(|e| e.kind == KIND_ISSUE) {
            let m = mints.entry(u256(&e.item_id)).or_default();
            m.max_issued = m.max_issued.max(u256(&e.amount));
            if e.account.as_deref() == Some(STORE_MATIC) {
                m.primary_sold_at = Some(
                    m.primary_sold_at
                        .map_or(e.block_time, |s| s.max(e.block_time)),
                );
            }
        }
        let walked: Vec<CollectionEvent> = all
            .iter()
            .filter(|e| {
                e.kind != KIND_ISSUE
                    || tracked.contains(&(
                        e.item_id.clone().unwrap_or_default(),
                        e.account.clone().unwrap_or_default(),
                    ))
            })
            .cloned()
            .collect();
        fold(C, &walked, Some(read), &mints, upto).unwrap()
    }

    fn numbered(kinds: Vec<CollectionEvent>) -> Vec<CollectionEvent> {
        kinds
            .into_iter()
            .enumerate()
            .map(|(i, e)| CollectionEvent {
                block_number: 10 + i as i64,
                block_time: 1_010 + i as i64,
                ..e
            })
            .collect()
    }

    /// A projection resumed from its rows with the events past them equals
    /// the fold from the creation, at every cut of a history that uses every
    /// kind: the rows carry all the state the next events need.
    #[test]
    fn resuming_from_the_rows_equals_folding_from_the_creation() {
        let mut history = created(0);
        history.extend([
            add(0, 0, HAT),
            add(0, 1, "1:e:Wave:hi:greetings:BaseMale:1:so"),
            CollectionEvent {
                flag: Some(true),
                ..ev(KIND_COMPLETE, 0)
            },
            grant(KIND_SET_GLOBAL_MANAGER, 0, OTHER, true),
            item_minter(0, 0, MINTER, U256::from(2)),
            item_minter(0, 1, STORE_MATIC, U256::MAX),
            issue(0, 1, STORE_MATIC),
            issue(0, 0, MINTER),
            grant(KIND_SET_APPROVED, 0, MINTER, true),
            grant(KIND_SET_GLOBAL_MINTER, 0, STORE_MATIC, true),
            issue(0, 0, STORE_MATIC),
            CollectionEvent {
                item_id: Some("0".into()),
                amount: Some("9".into()),
                beneficiary: Some(OTHER.into()),
                metadata: Some("1:w:Cap:a cap:hat:BaseFemale".into()),
                ..ev(KIND_UPDATE_ITEM_DATA, 0)
            },
            CollectionEvent {
                account: Some(MINTER.into()),
                previous_account: Some(CREATOR.into()),
                ..ev(KIND_CREATORSHIP_TRANSFERRED, 0)
            },
            issue(0, 0, MINTER),
            grant(KIND_SET_GLOBAL_MINTER, 0, STORE_MATIC, false),
            CollectionEvent {
                account: Some(OTHER.into()),
                previous_account: Some("0x00000000000000000000000000000000000000f0".into()),
                ..ev(KIND_OWNERSHIP_TRANSFERRED, 0)
            },
            grant(KIND_SET_EDITABLE, 0, MINTER, false),
            CollectionEvent {
                item_id: Some("1".into()),
                metadata: Some("1:e:Wave:hi:greetings:BaseMale:0:gs".into()),
                ..ev(KIND_RESCUE_ITEM, 0)
            },
        ]);
        let history = numbered(history);
        let mut issued = HashMap::<String, u64>::new();
        let history: Vec<CollectionEvent> = history
            .into_iter()
            .map(|e| {
                if e.kind != KIND_ISSUE {
                    return e;
                }
                let n = issued.entry(e.item_id.clone().unwrap()).or_default();
                *n += 1;
                CollectionEvent {
                    amount: Some(n.to_string()),
                    ..e
                }
            })
            .collect();
        let read = seed(1);
        let end = history.last().unwrap().block_number;
        let whole = fold_all(&history, &read, end);
        assert_eq!(
            (
                whole.1[0].minters.clone(),
                whole.1[0].minter_allowances.clone()
            ),
            (vec![MINTER.to_string()], vec![U256::from(1)]),
            "one of two spent: the mint MINTER called as the creator was exempt"
        );
        for cut in 2..history.len() {
            let (done, rest) = history.split_at(cut);
            let (c, items) = fold_all(done, &read, done.last().unwrap().block_number);
            let resumed = resume(c, items, rest).unwrap();
            assert_eq!(resumed, whole, "cut after {} events", cut);
        }
    }

    /// Grants, a revoke and a regrant give the same minters and managers
    /// whether the fold resumes before or after any of them.
    #[test]
    fn grants_list_the_same_minters_at_any_cut() {
        let a = "0x00000000000000000000000000000000000000d1";
        let b = "0x00000000000000000000000000000000000000d2";
        let mut history = created(0);
        for kind in [KIND_SET_GLOBAL_MINTER, KIND_SET_GLOBAL_MANAGER] {
            history.extend([
                grant(kind, 0, a, true),
                grant(kind, 0, b, true),
                grant(kind, 0, a, false),
                grant(kind, 0, a, true),
            ]);
        }
        let history = numbered(history);
        let read = seed(1);
        let end = history.last().unwrap().block_number;
        let whole = fold_all(&history, &read, end);
        assert_eq!(
            (whole.0.minters.clone(), whole.0.managers.clone()),
            (
                vec![a.to_string(), b.to_string()],
                vec![a.to_string(), b.to_string()]
            )
        );
        for cut in 2..history.len() {
            let (done, rest) = history.split_at(cut);
            let (c, items) = fold_all(done, &read, done.last().unwrap().block_number);
            assert_eq!(
                resume(c, items, rest).unwrap(),
                whole,
                "cut after {cut} events"
            );
        }
    }

    /// Search text folds ASCII letters alone, as the subgraph's helper does.
    #[test]
    fn search_text_folds_ascii_letters_alone() {
        let mut events = created(10);
        events.push(add(11, 0, "1:w:Élan Hat:Ö Cap:hat:BaseMale"));
        let read = Seed {
            name: "Élan Vital".into(),
            ..seed(1)
        };
        let (c, items) = fold(C, &events, Some(&read), &HashMap::new(), 1_000).unwrap();
        assert_eq!(c.search_text, "Élan vital");
        assert_eq!(items[0].search_text, "Élan hat Ö cap");
    }

    /// An event naming an item the rows lack (its metadata took no type)
    /// asks for a fold from the creation.
    #[test]
    fn an_item_without_a_row_asks_for_a_replay() {
        let mut history = created(0);
        history.extend([add(0, 0, HAT), add(0, 1, "1:w:broken")]);
        let history = numbered(history);
        let (c, items) = fold_all(&history, &seed(1), 20);
        let rescue = CollectionEvent {
            item_id: Some("1".into()),
            metadata: Some(HAT.into()),
            ..ev(KIND_RESCUE_ITEM, 30)
        };
        assert_eq!(
            resume(c.clone(), items.clone(), &[rescue]),
            Err(NeedsReplay)
        );
        let mint = issue(31, 0, CREATOR);
        let (_, items) = resume(c, items, &[mint]).unwrap();
        assert_eq!(items[0].total_supply, U256::from(1));
    }

    /// An item whose metadata no type takes has no row but counts in the
    /// collection's items; a rescue that fixes the metadata gives it one.
    #[test]
    fn an_undefined_item_is_counted_and_not_listed() {
        let mut events = created(10);
        events.extend([add(11, 0, HAT), add(12, 1, "1:w:broken")]);
        let (c, items) = folded(events.clone());
        assert_eq!((c.items_count, items.len()), (2, 1));
        events.push(CollectionEvent {
            item_id: Some("1".into()),
            metadata: Some("1:e:Wave:hi:greetings:BaseMale:1:so".into()),
            ..ev(KIND_RESCUE_ITEM, 13)
        });
        let (_, items) = folded(events);
        assert_eq!(items.len(), 2);
        assert_eq!(
            (
                items[1].item_type,
                items[1].emote_outcome_type,
                items[1].updated_at
            ),
            (EMOTE_V1, Some("simple_outcome"), 1_013)
        );
    }
}

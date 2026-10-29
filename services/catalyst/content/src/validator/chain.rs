//! The live access checker: [`Sources`] as the [`BlockchainChecker`]. The old
//! `SquidBlockchainChecker` read one squid database (`squid_marketplace.*`)
//! and a separately configured LAND operator resolver; v2 has no squid, so
//! each question goes where the market slice moved it:
//!
//! | check | source | freshness | missing source |
//! | --- | --- | --- | --- |
//! | LAND rights of a scene | `land_authz::can_deploy_batch`: the index once ready, the marketplace subgraph's owner leg before that (operator legs denied) | index: folded within 25 blocks, at most 30 min old; subgraph: 30 s memo | `Unavailable` |
//! | claimed names | `marketplace::graph::owned_names` (the marketplace subgraph, `MARKETPLACE_SUBGRAPH_URL`) | 30 s memo per owner | `Unavailable` |
//! | owned wearables/emotes | the collections subgraph of the URN's layer, three tiers (token, exact, prefix), plus the escrow lease overlay `shop.usage_grants` | live query | `Unavailable` |
//! | collection access of a wearable/emote deploy | the collections subgraph of the URN's layer | live query | `Unavailable` |
//! | third-party items | the registry root at the deploy's L2 block ([`super::tp_subgraph`]) | per finalized block, cached | `Unavailable` |
//!
//! Every subgraph call goes through the ONE `http::Outbound` (via
//! `marketplace::graph::graph_query`: the SSRF guard, the retry ladder, the
//! body cap). A source that answers "no" is a rejection with the old text; a
//! source that is unset or fails is a [`ValidatorError`], which the access
//! step turns into `Unavailable` (the old checker rejected with the error).
//!
//! Deviations from the squid reads: an item's collection access looks at
//! the deployed item (`blockchainId`), where the squid query took whichever
//! item of the collection `LIMIT 1` returned; URNs are compared lowercased
//! with `mainnet` spelled `ethereum` (the subgraphs' spelling), where the
//! squid compared the bytes as given; the overlay is read unconditionally
//! (the old `to_regclass('marketplace.usage_grants')` probe is gone: the
//! table is in the schema).

use std::collections::HashSet;

use async_trait::async_trait;
use serde_json::{Value, json};
use tracing::warn;

use super::Sources;
use super::checker::{BlockchainChecker, BlockchainLayer};
use super::error::{PermissionResult, ValidatorError};
use super::types::*;
use crate::chain_index::ChainIndex;
use crate::chain_index::graph::{
    Network, OwnedName, THE_GRAPH_PAGE_SIZE, graph_query, normalize_urn,
};

pub const DECENTRALAND_ADDRESS: &str = "0x1337e0507eb4ab47e08a179573ed4533d9e22a7b";

/// Pages of 1000 copies walked per network for one ownership tier.
const MAX_PAGES: usize = 10;
/// Aliased `urn_starts_with` probes in one prefix-tier document.
const PREFIXES_PER_QUERY: usize = 50;

fn addresses_match(a: &str, b: &str) -> bool {
    a.to_lowercase() == b.to_lowercase()
}

fn address_in_list(address: &str, list: &[String]) -> bool {
    list.iter().any(|a| addresses_match(address, a))
}

/// The form a URN is compared in: lowercase, L1 spelled `ethereum`.
fn urn_key(urn: &str) -> String {
    normalize_urn(&urn.to_lowercase())
}

/// A decimal token id without leading zeros (the squid compared `::numeric`).
fn token_key(token_id: &str) -> String {
    let trimmed = token_id.trim_start_matches('0');
    if trimmed.is_empty() {
        "0".to_string()
    } else {
        trimmed.to_string()
    }
}

/// The collections subgraph that indexes a URN: its third segment's layer.
fn network_of_urn(urn: &str) -> Option<Network> {
    let network = urn.split(':').nth(2)?.to_lowercase();
    if is_l1_network(&network) {
        Some(Network::Ethereum)
    } else if is_l2_network(&network) || network == "polygon" {
        Some(Network::Matic)
    } else {
        None
    }
}

/// The names of `names` the owner does not hold, in input order; exact
/// comparison, as the squid's `subdomain = ANY($1)`.
fn failing_names(names: &[String], owned: &[OwnedName]) -> Vec<String> {
    names
        .iter()
        .filter(|name| !owned.iter().any(|o| &o.name == *name))
        .cloned()
        .collect()
}

impl Sources {
    fn market(&self) -> Result<&ChainIndex, ValidatorError> {
        self.market.as_deref().ok_or_else(|| {
            ValidatorError::Unconfigured(
                "no marketplace readers are attached to this node".to_string(),
            )
        })
    }

    fn collections_reader(&self, network: Network) -> Result<(&ChainIndex, &str), ValidatorError> {
        let market = self.market()?;
        let url = match network {
            Network::Ethereum => market.settings.eth_collections_subgraph_url.as_deref(),
            Network::Matic => market.settings.matic_collections_subgraph_url.as_deref(),
        };
        let var = match network {
            Network::Ethereum => "ETH_COLLECTIONS_SUBGRAPH_URL",
            Network::Matic => "MATIC_COLLECTIONS_SUBGRAPH_URL",
        };
        url.map(|u| (market, u)).ok_or_else(|| {
            ValidatorError::Unconfigured(format!(
                "no {} collections subgraph is configured ({var})",
                network.db_name()
            ))
        })
    }

    /// Every copy of the requested item URNs the owner holds on `network`,
    /// as `(urn key, token id)`. One paged `urn_in` query per network.
    async fn owned_copies(
        &self,
        owner: &str,
        urns: &[String],
    ) -> Result<HashSet<(String, String)>, ValidatorError> {
        const QUERY: &str = r#"
query ownedCopies($owner: String!, $urns: [String!]!, $idFrom: ID!, $first: Int!) {
  nfts(where: { owner: $owner, urn_in: $urns, id_gt: $idFrom }, orderBy: id, orderDirection: asc, first: $first) { id urn tokenId }
}"#;
        let mut out = HashSet::new();
        if !crate::shared::identity::is_valid_eth_address(owner) {
            return Ok(out);
        }
        let owner = owner.to_lowercase();
        for (network, keys) in by_network(urns) {
            let (market, url) = self.collections_reader(network)?;
            let mut id_from = String::new();
            for _ in 0..MAX_PAGES {
                let data = graph_query(
                    market,
                    url,
                    QUERY,
                    json!({ "owner": owner, "urns": keys, "idFrom": id_from, "first": THE_GRAPH_PAGE_SIZE }),
                )
                .await
                .map_err(ValidatorError::SubgraphQuery)?;
                let rows = nfts(&data);
                for nft in &rows {
                    if let Some(urn) = nft.get("urn").and_then(Value::as_str) {
                        let token = match nft.get("tokenId") {
                            Some(Value::String(s)) => token_key(s),
                            Some(Value::Number(n)) => token_key(&n.to_string()),
                            _ => String::new(),
                        };
                        out.insert((urn_key(urn), token));
                    }
                }
                match rows
                    .last()
                    .and_then(|n| n.get("id"))
                    .and_then(Value::as_str)
                {
                    Some(id) if rows.len() as i64 >= THE_GRAPH_PAGE_SIZE => {
                        id_from = id.to_string()
                    }
                    _ => break,
                }
            }
        }
        Ok(out)
    }

    /// Which of `prefixes` (URN keys) have at least one copy the owner holds
    /// under `<prefix>:`, per network in aliased documents of
    /// [`PREFIXES_PER_QUERY`] probes.
    async fn owned_prefixes(
        &self,
        owner: &str,
        prefixes: &[String],
    ) -> Result<HashSet<String>, ValidatorError> {
        let mut out = HashSet::new();
        if !crate::shared::identity::is_valid_eth_address(owner) {
            return Ok(out);
        }
        let owner = owner.to_lowercase();
        for (network, keys) in by_network(prefixes) {
            let (market, url) = self.collections_reader(network)?;
            for chunk in keys.chunks(PREFIXES_PER_QUERY) {
                let mut doc = String::from("query prefixed($owner: String!) {");
                for (i, key) in chunk.iter().enumerate() {
                    let pattern = serde_json::to_string(&format!("{key}:"))
                        .unwrap_or_else(|_| "\"\"".to_string());
                    doc.push_str(&format!(" p{i}: nfts(first: 1, where: {{ owner: $owner, urn_starts_with: {pattern} }}) {{ urn }}"));
                }
                doc.push_str(" }");
                let data = graph_query(market, url, &doc, json!({ "owner": owner }))
                    .await
                    .map_err(ValidatorError::SubgraphQuery)?;
                for (i, key) in chunk.iter().enumerate() {
                    if data
                        .get(format!("p{i}"))
                        .and_then(Value::as_array)
                        .is_some_and(|a| !a.is_empty())
                    {
                        out.insert(key.clone());
                    }
                }
            }
        }
        Ok(out)
    }

    /// The owner's active escrow leases (`shop.usage_grants`) on the
    /// requested URNs (`exact`, compared as stored) or under the requested
    /// `LIKE` patterns: `(urn key, token id)`. Bounded by the request; the
    /// grantee's partial index serves it (the urn clause is a filter).
    async fn granted(
        &self,
        owner: &str,
        exact: &[String],
        patterns: &[String],
    ) -> Result<Vec<(String, Option<String>)>, ValidatorError> {
        if exact.is_empty() && patterns.is_empty() {
            return Ok(Vec::new());
        }
        let rows: Vec<(String, Option<String>)> = sqlx::query_as(
            "SELECT urn, token_id FROM shop.usage_grants \
             WHERE grantee_address = lower($1) AND status = 'active' \
               AND (urn = ANY($2) OR urn LIKE ANY($3))",
        )
        .bind(owner)
        .bind(exact)
        .bind(patterns)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| ValidatorError::BlockchainQuery(format!("usage grants query failed: {e}")))?;
        Ok(rows
            .into_iter()
            .map(|(urn, token)| (urn_key(&urn), token.map(|t| token_key(&t))))
            .collect())
    }
}

/// The URN keys per collections subgraph, deduplicated; a URN no subgraph
/// indexes (off-chain, an unknown network) has no copy anywhere.
fn by_network(urns: &[String]) -> Vec<(Network, Vec<String>)> {
    let mut eth = Vec::new();
    let mut matic = Vec::new();
    let mut seen = HashSet::new();
    for urn in urns {
        let key = urn_key(urn);
        if !seen.insert(key.clone()) {
            continue;
        }
        match network_of_urn(&key) {
            Some(Network::Ethereum) => eth.push(key),
            Some(Network::Matic) => matic.push(key),
            None => {}
        }
    }
    [(Network::Ethereum, eth), (Network::Matic, matic)]
        .into_iter()
        .filter(|(_, keys)| !keys.is_empty())
        .collect()
}

fn nfts(data: &Value) -> Vec<Value> {
    data.get("nfts")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
}

/// The raw URNs and their keys, for the overlay's `urn = ANY`.
fn exact_forms<'a>(urns: impl Iterator<Item = &'a String>) -> Vec<String> {
    let mut out: Vec<String> = urns.flat_map(|u| [u.clone(), urn_key(u)]).collect();
    out.sort();
    out.dedup();
    out
}

fn like_pattern(urn: &str) -> String {
    let esc = urn
        .replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_");
    format!("{esc}:%")
}

/// Three ownership tiers, each a single batched round trip over the URNs still
/// unresolved after the prior tier. Every method returns the input indices it
/// resolved to `true`.
#[async_trait]
trait NftBatchSource {
    /// `items` are `(index, item_urn, token_id)`.
    async fn tier_token(
        &self,
        address: &str,
        items: &[(usize, String, String)],
    ) -> Result<Vec<usize>, ValidatorError>;
    /// `items` are `(index, urn)`.
    async fn tier_exact(
        &self,
        address: &str,
        items: &[(usize, String)],
    ) -> Result<Vec<usize>, ValidatorError>;
    /// `items` are `(index, urn)`; the `{urn}:` prefix is built inside the
    /// implementation.
    async fn tier_prefix(
        &self,
        address: &str,
        items: &[(usize, String)],
    ) -> Result<Vec<usize>, ValidatorError>;
}

/// The squid's three tiers over the collections subgraphs, each unioned with
/// the escrow lease overlay (the old `usage_grants` branch of every tier).
struct LiveNftSource<'a>(&'a Sources);

#[async_trait]
impl NftBatchSource for LiveNftSource<'_> {
    async fn tier_token(
        &self,
        address: &str,
        items: &[(usize, String, String)],
    ) -> Result<Vec<usize>, ValidatorError> {
        let item_urns: Vec<String> = items.iter().map(|(_, u, _)| u.clone()).collect();
        let mut owned = self.0.owned_copies(address, &item_urns).await?;
        for (urn, token) in self
            .0
            .granted(address, &exact_forms(item_urns.iter()), &[])
            .await?
        {
            if let Some(token) = token {
                owned.insert((urn, token));
            }
        }
        Ok(items
            .iter()
            .filter(|(_, urn, token)| owned.contains(&(urn_key(urn), token_key(token))))
            .map(|(i, ..)| *i)
            .collect())
    }

    async fn tier_exact(
        &self,
        address: &str,
        items: &[(usize, String)],
    ) -> Result<Vec<usize>, ValidatorError> {
        let urns: Vec<String> = items.iter().map(|(_, u)| u.clone()).collect();
        let mut owned: HashSet<String> = self
            .0
            .owned_copies(address, &urns)
            .await?
            .into_iter()
            .map(|(urn, _)| urn)
            .collect();
        owned.extend(
            self.0
                .granted(address, &exact_forms(urns.iter()), &[])
                .await?
                .into_iter()
                .map(|(urn, _)| urn),
        );
        Ok(items
            .iter()
            .filter(|(_, urn)| owned.contains(&urn_key(urn)))
            .map(|(i, _)| *i)
            .collect())
    }

    async fn tier_prefix(
        &self,
        address: &str,
        items: &[(usize, String)],
    ) -> Result<Vec<usize>, ValidatorError> {
        let urns: Vec<String> = items.iter().map(|(_, u)| u.clone()).collect();
        let owned = self.0.owned_prefixes(address, &urns).await?;
        let patterns: Vec<String> = urns
            .iter()
            .flat_map(|u| [like_pattern(u), like_pattern(&urn_key(u))])
            .collect();
        let granted = self.0.granted(address, &[], &patterns).await?;
        Ok(items
            .iter()
            .filter(|(_, urn)| {
                let key = urn_key(urn);
                owned.contains(&key)
                    || granted
                        .iter()
                        .any(|(g, _)| g.starts_with(&format!("{key}:")))
            })
            .map(|(i, _)| *i)
            .collect())
    }
}

/// Positional `owns`-per-URN. A URN reaches tier 2 only if tier 1 did not
/// resolve it, and tier 3 only if tier 2 did not, so at most 3 round trips run.
/// Tier 1 eligibility is a 7-part `:collections-` URN with an all-digit token
/// id; non-eligible URNs fall straight to tier 2 carrying their FULL original
/// URN, never a truncated one.
async fn nft_ownership_batch<S: NftBatchSource + ?Sized>(
    src: &S,
    address: &str,
    urns: &[String],
) -> Result<Vec<bool>, ValidatorError> {
    let mut resolved = vec![false; urns.len()];

    let tier1: Vec<(usize, String, String)> = urns
        .iter()
        .enumerate()
        .filter_map(|(i, urn)| {
            let parts: Vec<&str> = urn.split(':').collect();
            let eligible = parts.len() == 7
                && urn.contains(":collections-")
                && parts[6].chars().all(|c| c.is_ascii_digit());
            eligible.then(|| (i, parts[..6].join(":"), parts[6].to_string()))
        })
        .collect();
    if !tier1.is_empty() {
        for i in src.tier_token(address, &tier1).await? {
            resolved[i] = true;
        }
    }

    let pending = |resolved: &[bool]| -> Vec<(usize, String)> {
        urns.iter()
            .enumerate()
            .filter(|(i, _)| !resolved[*i])
            .map(|(i, urn)| (i, urn.clone()))
            .collect()
    };
    let tier2 = pending(&resolved);
    if !tier2.is_empty() {
        for i in src.tier_exact(address, &tier2).await? {
            resolved[i] = true;
        }
    }

    let tier3 = pending(&resolved);
    if !tier3.is_empty() {
        for i in src.tier_prefix(address, &tier3).await? {
            resolved[i] = true;
        }
    }

    Ok(resolved)
}

const QUERY_COLLECTION: &str = r#"
query collectionAccess($collection: String!) {
  collections(where: { id: $collection }, first: 1) { creator owner managers minters }
}"#;

const QUERY_COLLECTION_AND_ITEM: &str = r#"
query collectionAccess($collection: String!, $itemId: BigInt!) {
  collections(where: { id: $collection }, first: 1) { creator owner managers minters }
  items(where: { collection: $collection, blockchainId: $itemId }, first: 1) { creator managers minters }
}"#;

fn strings_at(v: &Value, key: &str) -> Vec<String> {
    v.get(key)
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}

fn str_at<'a>(v: &'a Value, key: &str) -> &'a str {
    v.get(key).and_then(Value::as_str).unwrap_or("")
}

/// The squid rule over one subgraph answer: the collection's creator, owner,
/// managers or minters, else the item's creator, managers or minters.
fn collection_grants(address: &str, data: &Value) -> bool {
    let first = |field: &str| {
        data.get(field)
            .and_then(Value::as_array)
            .and_then(|a| a.first())
            .cloned()
    };
    let Some(collection) = first("collections") else {
        return false;
    };
    if addresses_match(address, str_at(&collection, "creator"))
        || addresses_match(address, str_at(&collection, "owner"))
        || address_in_list(address, &strings_at(&collection, "managers"))
        || address_in_list(address, &strings_at(&collection, "minters"))
    {
        return true;
    }
    first("items").is_some_and(|item| {
        addresses_match(address, str_at(&item, "creator"))
            || address_in_list(address, &strings_at(&item, "managers"))
            || address_in_list(address, &strings_at(&item, "minters"))
    })
}

#[async_trait]
impl BlockchainChecker for Sources {
    async fn check_land_access(
        &self,
        eth_address: &str,
        parcels: &[(i32, i32)],
        _timestamp: Timestamp,
    ) -> Result<Vec<bool>, ValidatorError> {
        let land = self.land.as_deref().ok_or_else(|| {
            ValidatorError::Unconfigured(
                "no LAND rights source: the land_authz state is not attached".to_string(),
            )
        })?;
        let decisions = crate::land_authz::can_deploy_batch(land, parcels, eth_address).await?;
        Ok(decisions.iter().map(|d| d.verdict.allows()).collect())
    }

    async fn check_names_ownership(
        &self,
        eth_address: &str,
        names: &[String],
        _timestamp: Timestamp,
    ) -> Result<PermissionResult, ValidatorError> {
        let market = self.market()?;
        if market.settings.marketplace_subgraph_url.is_none() {
            return Err(ValidatorError::Unconfigured(
                "no marketplace subgraph is configured (MARKETPLACE_SUBGRAPH_URL)".to_string(),
            ));
        }
        let owned = crate::chain_index::graph::owned_names(market, eth_address)
            .await
            .map_err(ValidatorError::SubgraphQuery)?;
        Ok(PermissionResult::from_failing(failing_names(names, &owned)))
    }

    async fn check_items_ownership(
        &self,
        eth_address: &str,
        urns: &[String],
        _timestamp: Timestamp,
    ) -> Result<PermissionResult, ValidatorError> {
        let owned = nft_ownership_batch(&LiveNftSource(self), eth_address, urns).await?;
        Ok(PermissionResult::from_failing(
            urns.iter()
                .zip(owned)
                .filter(|(_, o)| !o)
                .map(|(u, _)| u.clone())
                .collect(),
        ))
    }

    async fn check_collection_access(
        &self,
        eth_address: &str,
        contract_address: &str,
        item_id: &str,
        _entity: &Entity,
        _timestamp: Timestamp,
        layer: BlockchainLayer,
    ) -> Result<bool, ValidatorError> {
        // A v1 collection named by its name rather than its contract has no
        // collection row keyed by it (the squid found none either).
        if !crate::shared::identity::is_valid_eth_address(contract_address) {
            return Ok(false);
        }
        let network = match layer {
            BlockchainLayer::L1 => Network::Ethereum,
            BlockchainLayer::L2 => Network::Matic,
        };
        let (market, url) = self.collections_reader(network)?;
        let collection = contract_address.to_lowercase();
        let data = if !item_id.is_empty() && item_id.bytes().all(|b| b.is_ascii_digit()) {
            graph_query(
                market,
                url,
                QUERY_COLLECTION_AND_ITEM,
                json!({ "collection": collection, "itemId": item_id }),
            )
            .await
        } else {
            graph_query(
                market,
                url,
                QUERY_COLLECTION,
                json!({ "collection": collection }),
            )
            .await
        }
        .map_err(ValidatorError::SubgraphQuery)?;
        Ok(collection_grants(eth_address, &data))
    }

    async fn check_third_party_access(
        &self,
        asset_urn: &str,
        entity: &Entity,
        _deployment: &Deployment<'_>,
        timestamp: Timestamp,
    ) -> Result<bool, ValidatorError> {
        let Some(tp) = self.third_party.as_ref() else {
            return Err(ValidatorError::Unconfigured(
                "no third-party registry is configured (THIRD_PARTY_REGISTRY_L2_SUBGRAPH_URL, BLOCKS_L2_SUBGRAPH_URL)".to_string(),
            ));
        };
        let Some(metadata) = &entity.metadata else {
            return Ok(false);
        };
        let tp_props: super::third_party::ThirdPartyProps =
            match serde_json::from_value(metadata.clone()) {
                Ok(p) => p,
                Err(e) => {
                    warn!(asset_urn, error = %e, "could not parse third-party metadata");
                    return Ok(false);
                }
            };
        let Some(tp_id) = super::third_party::get_third_party_id(asset_urn) else {
            warn!(asset_urn, "could not derive third-party id from urn");
            return Ok(false);
        };
        let block = tp
            .block_for_timestamp(timestamp)
            .await
            .map_err(|e| ValidatorError::SubgraphQuery(format!("L2 blocks: {e}")))?
            .ok_or_else(|| {
                ValidatorError::SubgraphQuery(
                    "the L2 blocks subgraph has no block for the deployment timestamp yet"
                        .to_string(),
                )
            })?;
        let root = tp
            .third_party_root(&tp_id, block)
            .await
            .map_err(|e| ValidatorError::SubgraphQuery(format!("third-party registry: {e}")))?;
        let Some(root) = root else {
            warn!(tp_id, block, "third-party not approved or root unavailable");
            return Ok(false);
        };
        Ok(super::third_party::verify_third_party_merkle_proof(
            &tp_props.merkle_proof,
            &root,
        ))
    }

    fn is_address_owned_by_decentraland(&self, address: &str) -> bool {
        let lower = address.to_lowercase();
        lower == DECENTRALAND_ADDRESS
            || self
                .additional_dcl_address
                .as_deref()
                .is_some_and(|a| a.to_lowercase() == lower)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::land_authz::resolve::{DEPLOY_GRANTING_LEGS, operator_flags};
    use crate::land_authz::{LandOperators, LandRightLeg, ParcelPermissionFlags};
    use std::collections::HashSet;
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct CountingNft {
        token: AtomicUsize,
        exact: AtomicUsize,
        prefix: AtomicUsize,
        owned: HashSet<String>,
    }

    impl CountingNft {
        fn new(owned: impl IntoIterator<Item = String>) -> Self {
            Self {
                token: AtomicUsize::new(0),
                exact: AtomicUsize::new(0),
                prefix: AtomicUsize::new(0),
                owned: owned.into_iter().collect(),
            }
        }
        fn total_calls(&self) -> usize {
            self.token.load(Ordering::SeqCst)
                + self.exact.load(Ordering::SeqCst)
                + self.prefix.load(Ordering::SeqCst)
        }
        fn hits<T>(
            &self,
            counter: &AtomicUsize,
            items: &[T],
            key: impl Fn(&T) -> (usize, String),
        ) -> Vec<usize> {
            counter.fetch_add(1, Ordering::SeqCst);
            items
                .iter()
                .map(key)
                .filter(|(_, k)| self.owned.contains(k))
                .map(|(i, _)| i)
                .collect()
        }
    }

    #[async_trait]
    impl NftBatchSource for CountingNft {
        async fn tier_token(
            &self,
            _address: &str,
            items: &[(usize, String, String)],
        ) -> Result<Vec<usize>, ValidatorError> {
            Ok(self.hits(&self.token, items, |(i, u, t)| {
                (*i, format!("token:{u}:{t}"))
            }))
        }
        async fn tier_exact(
            &self,
            _address: &str,
            items: &[(usize, String)],
        ) -> Result<Vec<usize>, ValidatorError> {
            Ok(self.hits(&self.exact, items, |(i, u)| (*i, format!("exact:{u}"))))
        }
        async fn tier_prefix(
            &self,
            _address: &str,
            items: &[(usize, String)],
        ) -> Result<Vec<usize>, ValidatorError> {
            Ok(self.hits(&self.prefix, items, |(i, u)| (*i, format!("prefix:{u}"))))
        }
    }

    #[tokio::test]
    async fn nft_ownership_batch_query_count() {
        let urns: Vec<String> = (0..10)
            .map(|k| format!("urn:decentraland:matic:collections-v2:0xabc{k}:0:{k}"))
            .chain((0..10).map(|k| format!("urn:decentraland:matic:collections-v2:0xdef{k}:{k}")))
            .collect();
        assert_eq!(urns.len(), 20);

        let item0 = "urn:decentraland:matic:collections-v2:0xabc0:0";
        let src = CountingNft::new([
            format!("token:{item0}:0"),
            format!("exact:{}", urns[5]),
            format!("prefix:{}", urns[12]),
        ]);

        let result = nft_ownership_batch(&src, "0xowner", &urns).await.unwrap();

        assert!(
            src.total_calls() <= 3,
            "expected <= 3 tier round trips, got {}",
            src.total_calls()
        );
        assert_eq!(result.len(), 20);
        assert!(result[0], "index 0 owned via tier-1 token");
        assert!(result[5], "index 5 owned via tier-2 exact");
        assert!(result[12], "index 12 owned via tier-3 prefix");
        for (i, owned) in result.iter().enumerate() {
            if ![0usize, 5, 12].contains(&i) {
                assert!(!owned, "index {i} must be unowned");
            }
        }
    }

    #[test]
    fn address_list_membership() {
        let list = vec!["0xabc123".to_string(), "0xDEF456".to_string()];
        assert!(address_in_list("0xABC123", &list));
        assert!(address_in_list("0xdef456", &list));
        assert!(!address_in_list("0x999999", &list));
    }

    #[test]
    fn urn_and_token_keys_follow_the_subgraph_spelling() {
        assert_eq!(
            urn_key("urn:decentraland:MAINNET:collections-v1:0xABC:Hat"),
            "urn:decentraland:ethereum:collections-v1:0xabc:hat"
        );
        assert_eq!(token_key("007"), "7");
        assert_eq!(token_key("0"), "0");
        assert_eq!(token_key("000"), "0");
        assert_eq!(
            network_of_urn("urn:decentraland:sepolia:collections-v2:0x1:0"),
            Some(Network::Ethereum)
        );
        assert_eq!(
            network_of_urn("urn:decentraland:amoy:collections-v2:0x1:0"),
            Some(Network::Matic)
        );
        assert_eq!(
            network_of_urn("urn:decentraland:off-chain:base-avatars:x"),
            None
        );
        assert_eq!(network_of_urn("invalid-urn"), None);
        assert_eq!(like_pattern("a_b%c"), "a\\_b\\%c:%");
    }

    #[test]
    fn urns_group_per_subgraph_once() {
        let groups = by_network(&[
            "urn:decentraland:matic:collections-v2:0xabc:0".to_string(),
            "urn:decentraland:MATIC:collections-v2:0xABC:0".to_string(),
            "urn:decentraland:mainnet:collections-v1:x:y".to_string(),
            "urn:decentraland:off-chain:base-avatars:eyes_00".to_string(),
        ]);
        assert_eq!(
            groups,
            vec![
                (
                    Network::Ethereum,
                    vec!["urn:decentraland:ethereum:collections-v1:x:y".to_string()]
                ),
                (
                    Network::Matic,
                    vec!["urn:decentraland:matic:collections-v2:0xabc:0".to_string()]
                ),
            ]
        );
    }

    #[test]
    fn names_are_compared_exactly_in_input_order() {
        let owned = |n: &str| OwnedName {
            name: n.to_string(),
            contract_address: String::new(),
            token_id: String::new(),
            price: None,
        };
        let names: Vec<String> = ["alice", "Bob", "carol"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        assert_eq!(
            failing_names(&names, &[owned("carol"), owned("bob")]),
            vec!["alice".to_string(), "Bob".to_string()]
        );
        assert!(failing_names(&names, &[owned("alice"), owned("Bob"), owned("carol")]).is_empty());
    }

    #[test]
    fn collection_access_follows_the_squid_legs() {
        let data = json!({
            "collections": [{ "creator": "0xCreator", "owner": "0xowner", "managers": ["0xManager"], "minters": ["0xminter"] }],
            "items": [{ "creator": "0xitemcreator", "managers": ["0xitemmanager"], "minters": [] }]
        });
        for granted in [
            "0xcreator",
            "0xOWNER",
            "0xmanager",
            "0xMinter",
            "0xItemCreator",
            "0xitemmanager",
        ] {
            assert!(collection_grants(granted, &data), "{granted}");
        }
        assert!(!collection_grants("0xstranger", &data));
        assert!(
            !collection_grants("0xcreator", &json!({ "collections": [], "items": [] })),
            "no collection, no access"
        );
        let without_item = json!({ "collections": [{ "creator": "0xa", "owner": "0xb", "managers": [], "minters": [] }] });
        assert!(!collection_grants("0xitemmanager", &without_item));
    }

    #[test]
    fn operator_grants_each_leg() {
        let ops = LandOperators {
            operator: Some("0xAAA1".into()),
            update_operator: Some("0xbbb2".into()),
            update_managers: vec!["0xccc3".into()],
            approved_for_all: vec!["0xDDD4".into()],
        };
        assert!(operator_flags("0xaaa1", &ops).grants_deploy());
        assert!(operator_flags("0xBBB2", &ops).grants_deploy());
        assert!(operator_flags("0xCCC3", &ops).grants_deploy());
        assert!(operator_flags("0xddd4", &ops).grants_deploy());
        assert!(!operator_flags("0xeee5", &ops).grants_deploy());
    }

    #[test]
    fn operator_grants_denies_on_empty() {
        assert!(!operator_flags("0xaaa1", &LandOperators::default()).grants_deploy());
    }

    #[test]
    fn no_flags_denies() {
        assert!(!ParcelPermissionFlags::default().grants_deploy());
    }

    #[test]
    fn each_granting_leg_grants_alone() {
        for &leg in &DEPLOY_GRANTING_LEGS {
            let mut flags = ParcelPermissionFlags::default();
            match leg {
                LandRightLeg::Owner => flags.owner = true,
                LandRightLeg::Operator => flags.operator = true,
                LandRightLeg::UpdateOperator => flags.update_operator = true,
                LandRightLeg::UpdateManager => flags.update_manager = true,
                LandRightLeg::ApprovedForAll => flags.approved_for_all = true,
            }
            assert!(flags.grants_deploy(), "{leg:?} should grant deploy");
        }
    }

    #[test]
    fn serialized_shape_matches_lambdas_permissions_response() {
        let json = serde_json::to_string(&ParcelPermissionFlags::default()).unwrap();
        assert_eq!(
            json,
            "{\"owner\":false,\"operator\":false,\"updateOperator\":false,\"updateManager\":false,\"approvedForAll\":false}"
        );
    }

    #[test]
    fn legs_serialize_to_flag_keys() {
        for &leg in &DEPLOY_GRANTING_LEGS {
            let key = serde_json::to_value(leg).unwrap();
            let flags = serde_json::to_value(ParcelPermissionFlags::default()).unwrap();
            assert!(
                flags
                    .as_object()
                    .unwrap()
                    .contains_key(key.as_str().unwrap())
            );
        }
    }
}

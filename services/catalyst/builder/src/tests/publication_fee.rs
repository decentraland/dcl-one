//! The fee of a publication another payer co-signs
//! ([`crate::publication_fee::for_call`]): the call a creator's wallet would
//! send for its own draft is priced from the rarities, and every other call
//! is refused.

use std::sync::Arc;

use alloy_primitives::{Address, U256};
use alloy_sol_types::SolCall;
use serde_json::json;
use uuid::Uuid;

use super::support::{
    FakeCommittee, FakeOracle, OWNER_KEY, address, committee_index, contents_of, save_collection,
    saved_item, scratch_pool, temp_storage, unique, upload,
};
use crate::collections::contract::{self, InitializeItem, initializeCall};
use crate::network::POLYGON;
use crate::publication_chain::{ContractItem, createCollectionCall};
use crate::publication_fee::for_call;
use crate::shared::signed_fetch::test_support::signer;

const STRANGER_KEY: &str = "0x2222222222222222222222222222222222222222222222222222222222222222";
const TEN_USD: &str = "10000000000000000000";
const HALF_CENT_OVER_TWO_USD: &str = "2005000000000000000";

struct Node {
    pool: sqlx::PgPool,
    state: crate::SharedState,
    oracle: FakeOracle,
}

async fn node() -> Option<Node> {
    let pool = scratch_pool(8).await?;
    let subgraph = FakeCommittee::with(&[]);
    subgraph.set_rarities(Some(&[
        ("common", TEN_USD, "100000"),
        ("rare", HALF_CENT_OVER_TWO_USD, "10"),
    ]));
    let oracle = FakeOracle::with(&[("common", "4000000000000000000"), ("rare", "5")]);
    let state = Arc::new(
        crate::State::new(
            pool.clone(),
            temp_storage().await,
            Arc::default(),
            dcl_http::Outbound::for_tests(),
            Some(committee_index(&subgraph).await),
            None,
            crate::Settings {
                polygon_rpc_url: Some(oracle.url().await),
                ..crate::Settings::default()
            },
        )
        .unwrap(),
    );
    Some(Node {
        pool,
        state,
        oracle,
    })
}

/// What a creator's wallet sends to publish a draft: the collection
/// manager's arguments and the `initialize` calldata its factory deploys
/// with.
struct Publication {
    id: Uuid,
    name: String,
    creator: Address,
    rarities: Vec<&'static str>,
}

impl Publication {
    fn of(creator: &str, rarities: &[&'static str]) -> Self {
        let id = Uuid::new_v4();
        Self {
            id,
            name: format!("Fee {}", &id.simple().to_string()[..12]),
            creator: creator.parse().unwrap(),
            rarities: rarities.to_vec(),
        }
    }

    fn initialize(&self) -> String {
        let call = initializeCall {
            name: self.name.clone(),
            symbol: "DCL-F".into(),
            baseURI: POLYGON.base_uri.into(),
            creator: self.creator,
            shouldComplete: true,
            isApproved: false,
            rarities: POLYGON.rarities_with_oracle.parse().unwrap(),
            items: self
                .rarities
                .iter()
                .map(|rarity| InitializeItem {
                    rarity: rarity.to_string(),
                    price: U256::ZERO,
                    beneficiary: Address::ZERO,
                    metadata: "1:w:Item::hat:BaseMale".into(),
                })
                .collect(),
        };
        format!("0x{}", alloy_primitives::hex::encode(call.abi_encode()))
    }

    fn call(&self) -> createCollectionCall {
        createCollectionCall {
            forwarder: POLYGON.forwarder.parse().unwrap(),
            factory: POLYGON.collection_factory.parse().unwrap(),
            salt: contract::salt(&self.id.to_string()),
            name: self.name.clone(),
            symbol: "DCL-F".into(),
            baseURI: POLYGON.base_uri.into(),
            creator: self.creator,
            items: self
                .rarities
                .iter()
                .map(|rarity| ContractItem {
                    rarity: rarity.to_string(),
                    price: U256::ZERO,
                    beneficiary: Address::ZERO,
                    metadata: "1:w:Item::hat:BaseMale".into(),
                })
                .collect(),
        }
    }

    fn data(&self) -> Vec<u8> {
        arguments(&self.call())
    }
}

fn arguments(call: &createCollectionCall) -> Vec<u8> {
    call.abi_encode()[4..].to_vec()
}

/// Saves the draft of `publication` for the owner, with one stored item for
/// each of its items.
async fn draft(node: &Node, publication: &Publication) {
    let owner = address(OWNER_KEY);
    save_collection(
        &node.state.items,
        &owner,
        json!({ "id": publication.id, "name": publication.name, "data": publication.initialize() }),
    )
    .await
    .unwrap();
    for rarity in &publication.rarities {
        let item = Uuid::new_v4();
        let bytes = unique("fee");
        saved_item(
            &node.state.items,
            &owner,
            &json!({ "id": item, "name": "Item", "collection_id": publication.id, "rarity": rarity,
                     "contents": contents_of(&[("model.glb", &bytes)]) }),
        )
        .await;
        upload(&node.state.items, &owner, item, &[&bytes])
            .await
            .unwrap();
    }
}

async fn refusal(node: &Node, key: &str, data: &[u8]) -> (u16, String) {
    let error = for_call(&node.state, &signer(&address(key)), data)
        .await
        .expect_err("the call is refused");
    (error.status(), error.to_string())
}

#[tokio::test]
async fn a_creators_own_draft_costs_its_items_rarities_rounded_up_to_a_cent() {
    let Some(node) = node().await else { return };
    let owner = address(OWNER_KEY);
    let publication = Publication::of(&owner, &["common", "common", "rare"]);
    draft(&node, &publication).await;

    let fee = for_call(&node.state, &signer(&owner), &publication.data())
        .await
        .unwrap();

    let stored: String =
        sqlx::query_scalar("SELECT contract_address FROM builder.collections WHERE id = $1")
            .bind(publication.id)
            .fetch_one(&node.pool)
            .await
            .unwrap();
    assert_eq!(
        (
            fee.collection,
            fee.contract.as_str(),
            fee.items,
            fee.usd_cents
        ),
        (publication.id, stored.as_str(), 3, 2201),
        "two rarities of 10 USD and one of 2.005 USD"
    );
}

#[tokio::test]
async fn a_call_that_is_not_the_drafts_own_publication_is_refused() {
    let Some(node) = node().await else { return };
    let owner = address(OWNER_KEY);
    let publication = Publication::of(&owner, &["common"]);
    draft(&node, &publication).await;
    let own = publication.call();

    let mut renamed = own.clone();
    renamed.name = "Another name".into();
    let mut another_item = own.clone();
    another_item.items[0].rarity = "rare".into();
    let mut fewer_items = own.clone();
    fewer_items.items.clear();
    let mut another_salt = own.clone();
    another_salt.salt = contract::salt("another");
    let mut another_factory = own.clone();
    another_factory.factory = Address::repeat_byte(0x11);
    let mut another_forwarder = own.clone();
    another_forwarder.forwarder = Address::repeat_byte(0x12);
    let mut trailing = arguments(&own);
    trailing.extend([0u8; 32]);
    let statuses = [
        ("another name", arguments(&renamed), 404),
        ("another item", arguments(&another_item), 404),
        ("no items", arguments(&fewer_items), 400),
        ("another salt", arguments(&another_salt), 404),
        ("another factory", arguments(&another_factory), 400),
        ("another forwarder", arguments(&another_forwarder), 400),
        ("trailing bytes", trailing, 400),
        ("no arguments", Vec::new(), 400),
    ];
    for (case, data, status) in statuses {
        assert_eq!(refusal(&node, OWNER_KEY, &data).await.0, status, "{case}");
    }
}

#[tokio::test]
async fn only_the_drafts_creator_may_have_its_publication_priced() {
    let Some(node) = node().await else { return };
    let owner = address(OWNER_KEY);
    let publication = Publication::of(&owner, &["common"]);
    draft(&node, &publication).await;

    assert_eq!(
        refusal(&node, STRANGER_KEY, &publication.data()).await.0,
        403,
        "the owner's call, signed by a stranger"
    );
    let mut strangers = publication.call();
    strangers.creator = address(STRANGER_KEY).parse().unwrap();
    assert_eq!(
        refusal(&node, STRANGER_KEY, &arguments(&strangers)).await.0,
        404,
        "a stranger's call for the owner's draft deploys at an address no draft of the stranger's holds"
    );
}

#[tokio::test]
async fn a_published_or_deployed_collection_is_refused_and_a_locked_one_is_not() {
    let Some(node) = node().await else { return };
    let owner = address(OWNER_KEY);

    let deployed = Publication::of(&owner, &["common"]);
    draft(&node, &deployed).await;
    let contract: String =
        sqlx::query_scalar("SELECT contract_address FROM builder.collections WHERE id = $1")
            .bind(deployed.id)
            .fetch_one(&node.pool)
            .await
            .unwrap();
    node.oracle.deploy(&contract, 1);
    let (status, error) = refusal(&node, OWNER_KEY, &deployed.data()).await;
    assert_eq!(
        (status, error.as_str()),
        (400, "Collection has already been published"),
        "the chain holds the contract"
    );

    let published = Publication::of(&owner, &["common"]);
    draft(&node, &published).await;
    sqlx::query(
        "INSERT INTO shop.collections (network, contract_address, chain_id, name, creator, owner, urn, \
           is_completed, is_approved, is_editable, created_at, updated_at, reviewed_at, mirrored_at) \
         SELECT 'matic', contract_address, 137, 'On chain', eth_address, eth_address, 'urn:c', true, false, true, 1, 1, 1, \
           timezone('UTC', now()) FROM builder.collections WHERE id = $1",
    )
    .bind(published.id)
    .execute(&node.pool)
    .await
    .unwrap();
    let (status, error) = refusal(&node, OWNER_KEY, &published.data()).await;
    assert_eq!(
        (status, error.as_str()),
        (400, "Collection has already been published"),
        "the copy holds the collection"
    );

    let locked = Publication::of(&owner, &["common"]);
    draft(&node, &locked).await;
    sqlx::query("UPDATE builder.collections SET lock = timezone('UTC', now()) WHERE id = $1")
        .bind(locked.id)
        .execute(&node.pool)
        .await
        .unwrap();
    assert!(
        for_call(&node.state, &signer(&owner), &locked.data())
            .await
            .is_ok(),
        "a first transaction may have reverted, so its owner retries under the lock"
    );
}

#[tokio::test]
async fn a_rarity_the_chain_does_not_list_or_prices_it_cannot_read_are_refused() {
    let Some(node) = node().await else { return };
    let owner = address(OWNER_KEY);
    let unlisted = Publication::of(&owner, &["mythic"]);
    draft(&node, &unlisted).await;
    assert_eq!(refusal(&node, OWNER_KEY, &unlisted.data()).await.0, 400);

    let Some(blind) = node_without_rarities().await else {
        return;
    };
    let publication = Publication::of(&owner, &["common"]);
    draft(&blind, &publication).await;
    assert_eq!(refusal(&blind, OWNER_KEY, &publication.data()).await.0, 503);
}

async fn node_without_rarities() -> Option<Node> {
    let node = node().await?;
    node.oracle.set(None);
    Some(node)
}

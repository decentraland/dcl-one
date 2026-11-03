//! The checks against a chain on the loopback ([`fake_chain`]): each
//! granted and denied, the five-minute fallback, an RPC that cannot decide,
//! the block search; the encodings against the vendored ABI; and, ignored by
//! default, one LAND parcel and one approved item against a public RPC.

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use alloy_primitives::{Address, U256, keccak256};
use alloy_sol_types::SolCall;

use super::*;
use crate::validator::fake_chain::{self, Query, Reply, at_deployment, five_min_before};

const OWNER: &str = "0xaaaa000000000000000000000000000000000001";
const STRANGER: &str = "0xcccc000000000000000000000000000000000003";
const TS: i64 = 1_700_000_000_123;

fn addr(s: &str) -> Address {
    s.parse().unwrap()
}

fn access(l1: Option<&str>, l2: Option<&str>) -> OnChainAccess {
    OnChainAccess::new(
        Outbound::for_tests(),
        "mainnet",
        l1.map(str::to_string),
        l2.map(str::to_string),
    )
}

#[test]
fn the_encodings_match_the_vendored_abi() {
    let abi: Vec<serde_json::Value> =
        serde_json::from_str(include_str!("contracts/checker.abi.json")).unwrap();
    let signature = |name: &str| {
        let f = abi.iter().find(|f| f["name"] == name).unwrap();
        let inputs: Vec<&str> = f["inputs"]
            .as_array()
            .unwrap()
            .iter()
            .map(|i| i["type"].as_str().unwrap())
            .collect();
        format!("{name}({})", inputs.join(","))
    };
    for (name, selector, ours) in [
        (
            "checkLAND",
            Checker::checkLANDCall::SELECTOR,
            Checker::checkLANDCall::SIGNATURE,
        ),
        (
            "checkName",
            Checker::checkNameCall::SELECTOR,
            Checker::checkNameCall::SIGNATURE,
        ),
        (
            "multicall",
            Checker::multicallCall::SELECTOR,
            Checker::multicallCall::SIGNATURE,
        ),
        (
            "validateThirdParty",
            Checker::validateThirdPartyCall::SELECTOR,
            Checker::validateThirdPartyCall::SIGNATURE,
        ),
        (
            "validateWearables",
            Checker::validateWearablesCall::SELECTOR,
            Checker::validateWearablesCall::SIGNATURE,
        ),
    ] {
        assert_eq!(signature(name), ours);
        assert_eq!(keccak256(ours.as_bytes())[..4], selector, "{name}");
    }
    let outputs: Vec<&str> = abi.iter().find(|f| f["name"] == "multicall").unwrap()["outputs"]
        .as_array()
        .unwrap()
        .iter()
        .map(|o| o["type"].as_str().unwrap())
        .collect();
    assert_eq!(outputs, ["bool[]"]);
    assert!(checker_bytecode().starts_with("0x6080"));
    for network in ["mainnet", "sepolia"] {
        assert!(CONTRACTS.l1.contains_key(network));
        assert!(CONTRACTS.l2.contains_key(l2_network(network).unwrap()));
    }
}

#[test]
fn the_bounds_reach_8_s_after_and_5_min_7_s_before() {
    assert_eq!(
        timestamp_bounds(1_700_000_000_000),
        (1_700_000_008, 1_699_999_701)
    );
    assert_eq!(
        timestamp_bounds(1_700_000_000_001),
        (1_700_000_009, 1_699_999_702)
    );
    assert_eq!(timestamp_bounds(0), (8, 0));
}

#[tokio::test]
async fn land_is_granted_per_parcel_at_the_deploy_block_or_five_minutes_before() {
    let at = at_deployment(TS);
    let before = five_min_before(TS);
    let chain = fake_chain::serve(move |q, block| match q {
        Query::Land {
            sender,
            land,
            estate,
            x,
            y,
        } => {
            assert_eq!(*land, CONTRACTS.l1["mainnet"].land.0);
            assert_eq!(*estate, CONTRACTS.l1["mainnet"].estate.0);
            assert!(block == at || block == before, "{block}");
            // (1,1) is the owner's now; (2,2) was until shortly before the deploy.
            Reply::Yes(
                *sender == addr(OWNER)
                    && ((*x, *y) == (1, 1) || ((*x, *y) == (2, 2) && block == before)),
            )
        }
        other => panic!("{other:?}"),
    })
    .await;
    let a = access(Some(&chain.url), None);
    assert_eq!(
        a.land(OWNER, &[(1, 1), (2, 2), (3, 3)], TS).await.unwrap(),
        vec![true, true, false]
    );
    let calls = chain.calls();
    assert!(calls.contains(&format!("eth_call@{at}")), "{calls:?}");
    assert!(calls.contains(&format!("eth_call@{before}")), "{calls:?}");
    chain.clear();
    assert_eq!(a.land(OWNER, &[(1, 1)], TS).await.unwrap(), vec![true]);
    assert_eq!(
        chain.calls(),
        vec![format!("eth_call@{at}")],
        "granted at the deploy block: no fallback, and the blocks are known"
    );
    assert_eq!(a.land(STRANGER, &[(1, 1)], TS).await.unwrap(), vec![false]);
    assert_eq!(
        a.land("not-an-address", &[(1, 1)], TS).await.unwrap(),
        vec![false]
    );
}

#[tokio::test]
async fn names_are_asked_of_the_registrar() {
    let chain = fake_chain::serve(|q, _| match q {
        Query::Name {
            sender,
            registrar,
            name,
        } => {
            assert_eq!(*registrar, CONTRACTS.l1["mainnet"].registrar.0);
            Reply::Yes(*sender == addr(OWNER) && name == "alice")
        }
        other => panic!("{other:?}"),
    })
    .await;
    let a = access(Some(&chain.url), None);
    assert_eq!(
        a.names(OWNER, &["alice".into(), "bob".into()], TS)
            .await
            .unwrap(),
        vec![true, false]
    );
    assert!(a.names(OWNER, &[], TS).await.unwrap().is_empty());
}

#[tokio::test]
async fn collection_items_pass_the_hashes_and_the_contracts_active_at_the_block() {
    let at = at_deployment(TS);
    let collection = "0xc011000000000000000000000000000000000001";
    let chain = fake_chain::serve(move |q, block| match q {
        Query::Wearable {
            sender,
            factories,
            collection: c,
            item,
            hash,
            committees,
        } => {
            let l2 = &CONTRACTS.l2["polygon"];
            let active = |list: &[SinceBlock]| -> Vec<Address> {
                list.iter()
                    .filter(|s| block >= s.since_block)
                    .map(|s| s.address.0)
                    .collect()
            };
            assert_eq!(*factories, active(&l2.factories));
            assert_eq!(*committees, active(&l2.committees));
            assert_eq!(factories.len(), 2, "both factories are active by then");
            assert_eq!(*c, addr(collection));
            assert_eq!(*item, U256::from(3));
            // Only the CIDv1 and only at the deploy block.
            Reply::Yes(*sender == addr(OWNER) && hash == "bafkreinew" && block == at)
        }
        other => panic!("{other:?}"),
    })
    .await;
    let a = access(None, Some(&chain.url));
    let hashes = ["QmOld".to_string(), "bafkreinew".to_string()];
    assert!(
        a.collection_item(OWNER, collection, "3", &hashes, TS)
            .await
            .unwrap()
    );
    assert!(
        !a.collection_item(STRANGER, collection, "3", &hashes, TS)
            .await
            .unwrap()
    );
    assert!(
        !a.collection_item(OWNER, collection, "3", &hashes[..1], TS)
            .await
            .unwrap()
    );
    assert!(
        !a.collection_item(OWNER, "community_contest", "3", &hashes, TS)
            .await
            .unwrap(),
        "a collection named without its address"
    );
}

#[tokio::test]
async fn a_third_party_root_is_asked_of_the_registry() {
    let root = [7u8; 32];
    let chain = fake_chain::serve(move |q, _| match q {
        Query::ThirdParty {
            registry,
            tp_id,
            root: r,
        } => {
            assert_eq!(*registry, CONTRACTS.l2["polygon"].third_party.0);
            Reply::Yes(
                tp_id == "urn:decentraland:matic:collections-thirdparty:woodies" && r.0 == root,
            )
        }
        other => panic!("{other:?}"),
    })
    .await;
    let a = access(None, Some(&chain.url));
    let tp = "urn:decentraland:matic:collections-thirdparty:woodies";
    assert!(a.third_party_root(tp, root, TS).await.unwrap());
    assert!(!a.third_party_root(tp, [8u8; 32], TS).await.unwrap());
}

#[tokio::test]
async fn items_are_owned_by_token_and_third_party_nfts_by_their_standard() {
    let erc721 = "0x7210000000000000000000000000000000000001";
    let erc1155 = "0x1155000000000000000000000000000000000001";
    let not_nft = "0x0000000000000000000000000000000000000bad";
    let v2 = "0x0002000000000000000000000000000000000001";
    let probes = Arc::new(AtomicUsize::new(0));
    let seen = probes.clone();
    let chain = fake_chain::serve(move |q, _| match q {
        Query::OwnerOf { contract, token }
            if *contract == addr(v2) || *contract == addr(erc721) =>
        {
            if token.is_zero() {
                seen.fetch_add(1, Ordering::SeqCst);
                return Reply::Revert;
            }
            Reply::Owner(if *token == U256::from(42) {
                addr(OWNER)
            } else {
                addr(STRANGER)
            })
        }
        Query::OwnerOf { .. } => Reply::Empty,
        Query::BalanceOf {
            contract,
            account,
            id,
        } => {
            if account == contract {
                seen.fetch_add(1, Ordering::SeqCst);
            }
            if *contract == addr(erc1155) {
                Reply::Balance(U256::from(u8::from(
                    *account == addr(OWNER) && *id == U256::from(5),
                )))
            } else if *contract == addr(not_nft) {
                Reply::Empty
            } else {
                Reply::Revert
            }
        }
        other => panic!("{other:?}"),
    })
    .await;
    let a = access(Some(&chain.url), Some(&chain.url));
    let urns: Vec<String> = [
        format!("urn:decentraland:matic:collections-v2:{v2}:0:42"),
        format!("urn:decentraland:matic:collections-v2:{v2}:0:43"),
        format!("urn:decentraland:matic:collections-v2:{v2}:0"),
        format!("urn:decentraland:matic:collections-thirdparty:tp:c:i:matic:{erc721}:42"),
        format!("urn:decentraland:matic:collections-thirdparty:tp:c:i:mainnet:{erc1155}:5"),
        format!("urn:decentraland:matic:collections-thirdparty:tp:c:i:matic:{erc1155}:6"),
        "urn:decentraland:off-chain:base-avatars:eyes_00".to_string(),
    ]
    .to_vec();
    assert_eq!(
        a.items(OWNER, &urns, TS).await.unwrap(),
        vec![true, false, true, true, true, false, true]
    );
    let first = probes.load(Ordering::SeqCst);
    assert_eq!(first, 4, "one probe per contract and chain");
    a.items(OWNER, &urns, TS).await.unwrap();
    assert_eq!(
        probes.load(Ordering::SeqCst),
        first,
        "the standard is kept once known"
    );
    // A contract that is neither: its items are not owned.
    let unknown = vec![format!(
        "urn:decentraland:matic:collections-thirdparty:tp:c:i:matic:{not_nft}:1"
    )];
    assert_eq!(a.items(OWNER, &unknown, TS).await.unwrap(), vec![false]);
}

#[tokio::test]
async fn a_check_the_rpc_cannot_answer_is_an_error_never_a_grant() {
    let at = at_deployment(TS);
    let chain = fake_chain::serve(move |q, block| match q {
        // The deploy block grants nothing and the earlier one is pruned.
        Query::Land { .. } if block == at => Reply::Yes(false),
        Query::Land { .. } => Reply::Fail,
        Query::Name { .. } => Reply::Revert,
        other => panic!("{other:?}"),
    })
    .await;
    let a = access(Some(&chain.url), None);
    let err = a.land(OWNER, &[(1, 1)], TS).await.unwrap_err().to_string();
    assert!(err.contains("missing trie node"), "{err}");
    assert_eq!(
        a.names(OWNER, &["alice".into()], TS).await.unwrap(),
        vec![false],
        "a revert is the chain's no"
    );

    chain.set_down(true);
    let fresh = access(Some(&chain.url), None);
    let err = fresh
        .land(OWNER, &[(1, 1)], TS)
        .await
        .unwrap_err()
        .to_string();
    assert!(err.contains("block lookup"), "{err}");
    assert!(
        !err.contains("127.0.0.1"),
        "the endpoint is never named: {err}"
    );

    for (a, needle) in [
        (access(None, None), "ETH_RPC_URL"),
        (
            OnChainAccess::new(
                Outbound::for_tests(),
                "goerli",
                Some(chain.url.clone()),
                None,
            ),
            "ETH_NETWORK goerli",
        ),
    ] {
        let err = a.land(OWNER, &[(1, 1)], TS).await.unwrap_err().to_string();
        assert!(err.contains(needle), "{err}");
    }
    let err = access(Some(&chain.url), None)
        .collection_item(OWNER, OWNER, "0", &[], TS)
        .await
        .unwrap_err()
        .to_string();
    assert!(err.contains("RPC_ENDPOINT_POLYGON"), "{err}");
}

#[tokio::test]
async fn the_deploy_blocks_come_from_a_search_that_remembers() {
    let chain = fake_chain::serve(|_, _| Reply::Yes(true)).await;
    let a = access(Some(&chain.url), None);
    let l1 = a.l1().unwrap();
    let blocks = l1.chain.blocks_for(TS).await.unwrap();
    assert_eq!(
        blocks,
        Blocks {
            at_deployment: Some(at_deployment(TS)),
            five_min_before: Some(five_min_before(TS)),
        }
    );
    let cold = chain.calls().len();
    assert!(cold < 70, "two logarithmic searches: {cold}");
    chain.clear();
    assert_eq!(l1.chain.blocks_for(TS).await.unwrap(), blocks);
    assert!(chain.calls().is_empty(), "{:?}", chain.calls());
    assert_eq!(
        l1.chain.blocks_for(GENESIS_MS_BEFORE).await.unwrap(),
        Blocks {
            at_deployment: None,
            five_min_before: None
        },
        "before the first block"
    );
}

const GENESIS_MS_BEFORE: i64 = (fake_chain::GENESIS - 3600) * 1000;

/// Read-only calls to a public archive RPC (`CHECKER_LIVE_L1_RPC` and
/// `CHECKER_LIVE_L2_RPC` override the defaults), at fixed past timestamps so
/// the answers do not move: a parcel owned directly, one in an estate, and
/// an item of an approved collection deployed by a committee member with the
/// content its on-chain hash pins.
#[tokio::test]
#[ignore = "reads a public RPC"]
async fn live_land_and_approved_item_on_mainnet() {
    let l1 = std::env::var("CHECKER_LIVE_L1_RPC")
        .unwrap_or_else(|_| "https://rpc.decentraland.org/mainnet".into());
    let l2 = std::env::var("CHECKER_LIVE_L2_RPC")
        .unwrap_or_else(|_| "https://rpc.decentraland.org/polygon".into());
    let http = Outbound::new(Default::default()).unwrap();
    let a = OnChainAccess::new(http, "mainnet", Some(l1), Some(l2));

    // Mainnet block 26336000.
    let at = 1_793_729_663_000;
    let direct = "0x5127bc7594839253bd204246b17a5a6ef60da540";
    let estate = "0x5188e308fee25ac49c10f9fd9270d953c4822ce5";
    assert_eq!(
        a.land(direct, &[(-30, -120), (0, 0)], at).await.unwrap(),
        vec![true, false]
    );
    assert_eq!(
        a.land(estate, &[(-30, -120), (0, 0)], at).await.unwrap(),
        vec![false, true],
        "the owner of the estate holding (0,0)"
    );
    assert_eq!(a.land(STRANGER, &[(0, 0)], at).await.unwrap(), vec![false]);

    let collection = "0xf18f33c4db6689595f25ea4af6dd8df3e3abf18b";
    let committee = "0x1dec5f50cb1467f505bb3ddfd408805114406b10";
    let creator = "0xfe2d424af0df49bb3316cb2e9f574b0d09cf98ad";
    let pinned = ["bafkreiewhubsoukrvwic5kn7o3ga3wdqxpd7neh7jyz3w3tgdmor6djr7a".to_string()];
    let deployed = 1_793_717_744_232;
    assert!(
        a.collection_item(committee, collection, "0", &pinned, deployed)
            .await
            .unwrap(),
        "the committee member deploying the pinned content"
    );
    assert!(
        !a.collection_item(
            committee,
            collection,
            "0",
            &["bafkreiother".to_string()],
            deployed
        )
        .await
        .unwrap(),
        "other content"
    );
    assert!(
        !a.collection_item(creator, collection, "0", &pinned, deployed)
            .await
            .unwrap(),
        "the creator of an approved collection"
    );
}

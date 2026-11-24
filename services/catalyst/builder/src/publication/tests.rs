use super::*;
use crate::network::{AMOY, POLYGON};
use serde_json::json;
use uuid::Uuid;

const OWNER: &str = "0x1111111111111111111111111111111111111111";

fn snapshot() -> Value {
    let hash = hash_bytes_v1(b"content");
    json!({
        "collection": {"id": Uuid::nil(), "name": "My collection", "eth_address": OWNER, "is_published": false},
        "items": [{
            "id": Uuid::nil(), "name": "Hat", "eth_address": OWNER, "type": "wearable",
            "description": "A blue hat", "rarity": "rare", "price": "1000000000000000001", "beneficiary": OWNER,
            "thumbnail": "thumbnail.png", "contents": {"model.glb": hash, "thumbnail.png": hash},
            "data": {"category": "hat", "representations": [{"mainFile": "model.glb", "contents": ["model.glb"],
                "bodyShapes": ["urn:decentraland:off-chain:base-avatars:BaseFemale", "urn:decentraland:off-chain:base-avatars:BaseMale"]}]}
        }]
    })
}

#[test]
fn matches_upstream_wearable_metadata_without_rounding_price() {
    let snapshot = snapshot();
    let result = prepare(snapshot.clone(), OWNER, &POLYGON).unwrap();
    assert_eq!(
        result.items[0].metadata,
        "1:w:Hat:A blue hat:hat:BaseFemale,BaseMale"
    );
    assert_eq!(result.items[0].price, "1000000000000000001");
    assert_eq!(result.symbol, "DCL-MYCLLCTN");
    assert_eq!(result.chain_id, 137);
    assert_eq!(
        result.salt,
        format!("{:#x}", keccak256(Uuid::nil().to_string()))
    );
    assert_eq!(
        result.salt,
        prepare(snapshot, OWNER, &POLYGON).unwrap().salt
    );
}

#[test]
fn an_amoy_node_prepares_amoys_contracts_and_base_uri() {
    let mainnet = prepare(snapshot(), OWNER, &POLYGON).unwrap();
    let amoy = prepare(snapshot(), OWNER, &AMOY).unwrap();
    assert_eq!(
        (
            amoy.chain_id,
            amoy.manager.as_str(),
            amoy.factory.as_str(),
            amoy.forwarder.as_str(),
            amoy.base_uri.as_str(),
        ),
        (
            80_002,
            "0x5309ae874fc4eb21adcd63f8b6c3f766cc3b1849",
            "0x802de0c509add2ee29de24de7225daaff4741c43",
            "0x7b1fe9de545b22cb553766817b84d655ce8121c9",
            "https://peer.decentraland.zone/lambdas/collections/standard/erc721/",
        )
    );
    assert_eq!(
        (
            mainnet.chain_id,
            mainnet.manager.as_str(),
            mainnet.base_uri.as_str()
        ),
        (
            137,
            "0x9d32aac179153a991e832550d9f96441ea27763a",
            "https://peer.decentraland.org/lambdas/collections/standard/erc721/",
        )
    );
    assert_eq!(
        (amoy.revision, amoy.salt, amoy.items[0].metadata.clone()),
        (
            mainnet.revision,
            mainnet.salt,
            mainnet.items[0].metadata.clone()
        ),
        "the snapshot and the items' metadata don't depend on the network"
    );
}

#[test]
fn revision_tracks_content_even_when_contract_metadata_does_not_change() {
    let mut snapshot = snapshot();
    let first = prepare(snapshot.clone(), OWNER, &POLYGON).unwrap();
    snapshot["items"][0]["contents"]["model.glb"] = json!(hash_bytes_v1(b"replacement"));
    let second = prepare(snapshot, OWNER, &POLYGON).unwrap();
    assert_ne!(first.revision, second.revision);
    assert_eq!(first.salt, second.salt);
    assert_eq!(first.items[0].metadata, second.items[0].metadata);
}

#[test]
fn refuses_unready_and_unsafe_publication_inputs() {
    for (pointer, value) in [
        ("/collection/third_party_id", json!("provider")),
        ("/collection/salt", json!("0x1234")),
        (
            "/collection/eth_address",
            json!("0x2222222222222222222222222222222222222222"),
        ),
        ("/items", json!([])),
        (
            "/items/0/eth_address",
            json!("0x2222222222222222222222222222222222222222"),
        ),
        ("/items/0/is_published", json!(true)),
        ("/items/0/missing_content", json!(true)),
        ("/items/0/name", json!("bad:name")),
        ("/items/0/name", json!("x".repeat(33))),
        ("/items/0/description", json!("x".repeat(65))),
        ("/items/0/description", json!("bad:description")),
        ("/items/0/rarity", json!(null)),
        ("/items/0/price", json!(null)),
        (
            "/items/0/price",
            json!("115792089237316195423570985008687907853269984665640564039457584007913129639936"),
        ),
        (
            "/items/0/beneficiary",
            json!("0x0000000000000000000000000000000000000000"),
        ),
        ("/items/0/thumbnail", json!("missing.png")),
        ("/items/0/data/category", json!("dance")),
        ("/items/0/data/representations", json!([])),
        (
            "/items/0/data/representations/0/mainFile",
            json!("missing.glb"),
        ),
        (
            "/items/0/data/representations/0/contents",
            json!(["model.glb", "missing.png"]),
        ),
        (
            "/items/0/data/representations/0/bodyShapes",
            json!(["BaseMale"]),
        ),
    ] {
        let mut snapshot = snapshot();
        let (parent, key) = pointer.rsplit_once('/').unwrap();
        snapshot.pointer_mut(parent).unwrap()[key] = value;
        assert!(
            prepare(snapshot, OWNER, &POLYGON).is_err(),
            "accepted {pointer}"
        );
    }
}

#[test]
fn emote_metadata_preserves_loop_audio_props_and_social_outcomes() {
    let mut snapshot = snapshot();
    let item = &mut snapshot["items"][0];
    item["type"] = json!("emote");
    item["data"]["category"] = json!("dance");
    item["data"]["loop"] = json!(true);
    item["contents"]["audio.mp3"] = json!(hash_bytes_v1(b"audio"));
    item["metrics"] = json!({"props": 1});
    item["data"]["outcomes"] = json!([{}, {}]);
    item["data"]["randomizeOutcomes"] = json!(true);
    assert_eq!(
        prepare(snapshot, OWNER, &POLYGON).unwrap().items[0].metadata,
        "1:e:Hat:A blue hat:dance:BaseFemale,BaseMale:1:sg:ro"
    );
}

#[test]
fn free_item_and_existing_salt_are_preserved() {
    let mut snapshot = snapshot();
    let salt = format!("0x{}", "ab".repeat(32));
    snapshot["collection"]["salt"] = json!(salt);
    snapshot["items"][0]["price"] = json!("0");
    snapshot["items"][0]["beneficiary"] = Value::Null;
    let result = prepare(snapshot, OWNER, &POLYGON).unwrap();
    assert_eq!(result.salt, salt);
    assert_eq!(result.items[0].beneficiary, format!("{:#x}", Address::ZERO));
}

#[test]
fn beneficiary_defaults_follow_the_contract_price_rules() {
    let mut snapshot = snapshot();
    snapshot["items"][0]["beneficiary"] = Value::Null;
    assert_eq!(
        prepare(snapshot.clone(), OWNER, &POLYGON).unwrap().items[0].beneficiary,
        OWNER
    );
    snapshot["items"][0]["beneficiary"] = json!(OWNER);
    snapshot["items"][0]["price"] = json!("0");
    assert_eq!(
        prepare(snapshot, OWNER, &POLYGON).unwrap().items[0].beneficiary,
        format!("{:#x}", Address::ZERO)
    );
}

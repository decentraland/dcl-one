//! The `local_content_hash` builder-server stores on a save
//! (`calculateItemContentHash`): for a minted standard item, the ADR-32 CID
//! of its files and the entity metadata the catalyst would receive; for a
//! third-party item, the keccak-256 of that metadata with its files and
//! mappings. Both serialize the metadata as `JSON.stringify` does, so the
//! documents keep the order they were saved in.

use std::cmp::Ordering;

use crate::content::{JsJson, js_stringify};

use super::schema::ItemType;

const VIDEO_PATH: &str = "video.mp4";

/// The parts of a saved item the hashes read.
pub(crate) struct Hashed<'a> {
    pub name: &'a str,
    pub description: Option<&'a str>,
    /// `None`: the save named no rarity, so the metadata has none.
    pub rarity: Option<Option<&'a str>>,
    pub item_type: ItemType,
    pub data: &'a JsJson,
    pub metrics: &'a JsJson,
    pub contents: &'a [(String, String)],
    pub mappings: Option<&'a JsJson>,
}

fn string(value: &str) -> JsJson {
    JsJson::String(value.to_string())
}

fn optional(value: Option<&str>) -> JsJson {
    value.map_or(JsJson::Null, string)
}

fn field(out: &mut Vec<(String, JsJson)>, key: &str, value: Option<&JsJson>) {
    if let Some(value) = value {
        out.push((key.to_string(), value.clone()));
    }
}

fn truthy(value: Option<&JsJson>) -> Option<&JsJson> {
    value.filter(|v| match v {
        JsJson::Null | JsJson::Bool(false) => false,
        JsJson::Number(n) => *n != 0.0 && !n.is_nan(),
        JsJson::String(s) => !s.is_empty(),
        _ => true,
    })
}

fn wearable_data(data: &JsJson) -> JsJson {
    let mut out = Vec::new();
    for key in [
        "replaces",
        "hides",
        "removesDefaultHiding",
        "tags",
        "category",
        "representations",
        "blockVrmExport",
        "outlineCompatible",
    ] {
        field(&mut out, key, data.get(key));
    }
    field(&mut out, "springBones", truthy(data.get("springBones")));
    JsJson::Object(out)
}

fn emote_data(data: &JsJson) -> JsJson {
    let mut out = Vec::new();
    for key in ["category", "representations", "tags", "loop"] {
        field(&mut out, key, data.get(key));
    }
    if data.get("startAnimation").is_some() && data.get("outcomes").is_some() {
        for key in ["startAnimation", "randomizeOutcomes", "outcomes"] {
            field(&mut out, key, data.get(key));
        }
    }
    JsJson::Object(out)
}

fn animation_metrics() -> JsJson {
    JsJson::Object(
        [
            ("triangles", 0.0),
            ("materials", 0.0),
            ("textures", 0.0),
            ("meshes", 0.0),
            ("bodies", 0.0),
            ("entities", 1.0),
        ]
        .into_iter()
        .map(|(k, v)| (k.to_string(), JsJson::Number(v)))
        .collect(),
    )
}

fn head(item: &Hashed, id: &str, collection_address: Option<&str>) -> Vec<(String, JsJson)> {
    let mut out = vec![
        ("id".to_string(), string(id)),
        ("name".to_string(), string(item.name)),
        ("description".to_string(), optional(item.description)),
    ];
    if let Some(address) = collection_address {
        out.push(("collectionAddress".to_string(), string(address)));
        if let Some(rarity) = item.rarity {
            out.push(("rarity".to_string(), optional(rarity)));
        }
    }
    out.push((
        "i18n".to_string(),
        JsJson::Array(vec![JsJson::Object(vec![
            ("code".to_string(), string("en")),
            ("text".to_string(), string(item.name)),
        ])]),
    ));
    out
}

/// JavaScript's string comparison: UTF-16 code units, a prefix first.
fn js_cmp(a: &str, b: &str) -> Ordering {
    a.encode_utf16().cmp(b.encode_utf16())
}

/// The entity metadata the catalyst would receive for a minted standard
/// item on `network` (builder-server's `buildEntityMetadata`).
pub(crate) fn standard_metadata(
    network: &crate::network::Network,
    item: &Hashed,
    contract: &str,
    blockchain_item_id: &str,
) -> JsJson {
    let id = network.item_urn(contract, blockchain_item_id);
    let mut metadata = head(item, &id, Some(contract));
    match item.item_type {
        ItemType::Emote => {
            metadata.push(("emoteDataADR74".to_string(), emote_data(item.data)));
            metadata.push(("image".to_string(), string("image.png")));
            metadata.push(("thumbnail".to_string(), string("thumbnail.png")));
            metadata.push(("metrics".to_string(), animation_metrics()));
        }
        ItemType::Wearable => {
            metadata.push(("data".to_string(), wearable_data(item.data)));
            metadata.push(("image".to_string(), string("image.png")));
            metadata.push(("thumbnail".to_string(), string("thumbnail.png")));
            metadata.push(("metrics".to_string(), item.metrics.clone()));
        }
    }
    JsJson::Object(metadata)
}

/// An item's files without its video, which is not deployed.
pub(crate) fn deployed_files<'a>(item: &'a Hashed) -> impl Iterator<Item = &'a (String, String)> {
    item.contents.iter().filter(|(file, _)| file != VIDEO_PATH)
}

/// The CID of a minted standard item's files and metadata on `network`
/// (ADR-32).
pub(crate) fn standard(
    network: &crate::network::Network,
    item: &Hashed,
    contract: &str,
    blockchain_item_id: &str,
) -> String {
    let metadata = standard_metadata(network, item, contract, blockchain_item_id);
    let mut files: Vec<&(String, String)> = deployed_files(item).collect();
    files.sort_by(|(fa, ha), (fb, hb)| js_cmp(ha, hb).then_with(|| js_cmp(fa, fb)));
    let content = JsJson::Array(
        files
            .into_iter()
            .map(|(file, hash)| {
                JsJson::Object(vec![
                    ("key".to_string(), string(file)),
                    ("hash".to_string(), string(hash)),
                ])
            })
            .collect(),
    );
    let payload = JsJson::Object(vec![
        ("content".to_string(), content),
        ("metadata".to_string(), metadata),
    ]);
    dcl_auth::hash_bytes_v1(js_stringify(&payload).as_bytes())
}

/// The keccak-256 of a third-party item's metadata, files and mappings.
pub(crate) fn third_party(item: &Hashed, urn: &str) -> String {
    let mut metadata = head(item, urn, None);
    metadata.push(("data".to_string(), wearable_data(item.data)));
    metadata.push(("image".to_string(), string("image.png")));
    metadata.push(("thumbnail".to_string(), string("thumbnail.png")));
    metadata.push(("metrics".to_string(), item.metrics.clone()));
    metadata.push((
        "content".to_string(),
        JsJson::Object(
            item.contents
                .iter()
                .map(|(file, hash)| (file.clone(), string(hash)))
                .collect(),
        ),
    ));
    field(&mut metadata, "mappings", truthy(item.mappings));
    hex::encode(alloy_primitives::keccak256(
        js_stringify(&JsJson::Object(metadata)).as_bytes(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::network::{AMOY, POLYGON};

    /// builder-server's `hashes.spec.ts` standard wearable: its expected hash
    /// is the one it computes on Sepolia (the checkout's `CHAIN_NAME`), and
    /// the mainnet one is what the same code computes on `Ethereum Mainnet`.
    #[test]
    fn a_minted_items_hash_names_the_networks_urn_as_builder_server_does() {
        let data: JsJson = serde_json::from_str(
            r#"{"replaces":["earring"],"hides":["eyewear"],"tags":[],"category":"earring",
            "representations":[{"bodyShapes":["urn:decentraland:off-chain:base-avatars:BaseMale"],
            "mainFile":"male/F_3LAU_Hat_Blue.glb","contents":["male/F_3LAU_Hat_Blue.glb"],
            "overrideHides":["eyewear"],"overrideReplaces":["earring"]}]}"#,
        )
        .unwrap();
        let metrics: JsJson = serde_json::from_str(
            r#"{"triangles":468,"materials":2,"textures":2,"meshes":1,"bodies":2,"entities":1}"#,
        )
        .unwrap();
        let contents = [
            (
                "thumbnail.png",
                "QmeSfHFqSk73esyE5ZsW4yRqWsr5eJ8vXLx7v7L2dsXTmM",
            ),
            (
                "male/F_3LAU_Hat_Blue.glb",
                "Qmf7dnGi5fyF9AwdJGzVnFCUUGBB8w2mW1v6AZAWh7rJVd",
            ),
            (
                "image.png",
                "QmXga5BnDE16XR6UH5Tgw3rDNLgA1RN8PkGZWpw7aQsUyN",
            ),
        ]
        .map(|(file, hash)| (file.to_string(), hash.to_string()));
        let item = Hashed {
            name: "F 3LAU Hat Blue",
            description: Some(""),
            rarity: Some(Some("unique")),
            item_type: ItemType::Wearable,
            data: &data,
            metrics: &metrics,
            contents: &contents,
            mappings: None,
        };
        let contract = "0x6319d66715faf411f8c37a2f5858e0bce90da5ae";
        assert_eq!(
            standard(&AMOY, &item, contract, "0"),
            "bafkreigblh3xd4r57xvpvokhyhzydmmb5wt6ri7tyf25hpm7flknjkyqma"
        );
        assert_eq!(
            standard(&POLYGON, &item, contract, "0"),
            "bafkreihav73zvqoxx32g2v7q4alu5o6q25ilgb4l34umist5mpifk3xole"
        );
    }
}

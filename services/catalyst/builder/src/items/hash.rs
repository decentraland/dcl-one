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
/// item (builder-server's `buildEntityMetadata`).
pub(crate) fn standard_metadata(item: &Hashed, contract: &str, blockchain_item_id: &str) -> JsJson {
    let id = format!("urn:decentraland:matic:collections-v2:{contract}:{blockchain_item_id}");
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

/// The CID of a minted standard item's files and metadata (ADR-32).
pub(crate) fn standard(item: &Hashed, contract: &str, blockchain_item_id: &str) -> String {
    let metadata = standard_metadata(item, contract, blockchain_item_id);
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

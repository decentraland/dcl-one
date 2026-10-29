//! Wearable and emote definitions as the lambdas serve them: the entity's
//! metadata with every content file name rewritten to this node's
//! `contents/<hash>` URL, plus the rarity ranking and the UCA collation the
//! sorted lists share.

use serde_json::{Map, Value, json};

pub const SORTED_RARITIES: &[&str] = &[
    "common",
    "uncommon",
    "rare",
    "epic",
    "legendary",
    "exotic",
    "mythic",
    "unique",
];

pub fn validate_rarity(rarity: &str) -> Result<(), String> {
    if SORTED_RARITIES.contains(&rarity) {
        Ok(())
    } else {
        Err(format!("Invalid rarity requested: '{rarity}'."))
    }
}

pub const SORT_FIELDS: &[&str] = &["rarity", "name", "date"];

/// `name` reads most naturally ascending; rarity and date most-valuable/newest
/// first. Callers that disagree pass `direction` explicitly.
pub fn default_sort_direction(sort: &str) -> &'static str {
    if sort == "name" { "ASC" } else { "DESC" }
}

pub fn validate_sort(sort: &str, direction: &str) -> Result<(), String> {
    if SORT_FIELDS.contains(&sort) && matches!(direction, "ASC" | "DESC") {
        Ok(())
    } else {
        Err(format!(
            "Invalid sorting requested: '{sort} {direction}'. Valid options are '[rarity, name, date] [ASC, DESC]'."
        ))
    }
}

pub fn rarity_rank(rarity: &str) -> i64 {
    SORTED_RARITIES
        .iter()
        .position(|r| *r == rarity)
        .map(|p| p as i64)
        .unwrap_or(-1)
}

pub fn content_url(entity: &Value, file_name: &str, content_public_url: &str) -> Option<String> {
    let hash = entity
        .get("content")
        .and_then(|c| c.as_array())
        .and_then(|arr| {
            arr.iter().find_map(|item| {
                let f = item.get("file").and_then(|v| v.as_str())?;
                if f == file_name {
                    item.get("hash").and_then(|v| v.as_str())
                } else {
                    None
                }
            })
        })?;
    let base = if content_public_url.ends_with('/') {
        content_public_url.to_string()
    } else {
        format!("{content_public_url}/")
    };
    Some(format!("{base}contents/{hash}"))
}

pub fn map_representations(reps: &mut Value, entity: &Value, content_public_url: &str) {
    if let Some(arr) = reps.as_array_mut() {
        for rep in arr.iter_mut() {
            let new_contents: Vec<Value> = rep
                .get("contents")
                .and_then(|c| c.as_array())
                .map(|files| {
                    files
                        .iter()
                        .filter_map(|f| f.as_str())
                        .map(|file_name| {
                            let url = content_url(entity, file_name, content_public_url)
                                .unwrap_or_default();
                            json!({ "key": file_name, "url": url })
                        })
                        .collect()
                })
                .unwrap_or_default();
            if let Some(obj) = rep.as_object_mut() {
                obj.insert("contents".into(), Value::Array(new_contents));
            }
        }
    }
}

pub fn rewrite_image_thumbnail(
    def: &mut Map<String, Value>,
    entity: &Value,
    content_public_url: &str,
) {
    for key in ["image", "thumbnail"] {
        if let Some(file) = def.get(key).and_then(|v| v.as_str()).map(|s| s.to_string())
            && let Some(url) = content_url(entity, &file, content_public_url)
        {
            def.insert(key.into(), Value::String(url));
        }
    }
}

pub fn extract_wearable_definition(entity: &Value, content_public_url: &str) -> Option<Value> {
    let mut def = entity.get("metadata")?.as_object()?.clone();
    if let Some(data) = def.get_mut("data").and_then(|d| d.as_object_mut())
        && let Some(reps) = data.get_mut("representations")
    {
        map_representations(reps, entity, content_public_url);
    }
    rewrite_image_thumbnail(&mut def, entity, content_public_url);
    Some(Value::Object(def))
}

pub fn extract_emote_definition(entity: &Value, content_public_url: &str) -> Option<Value> {
    let mut meta = entity.get("metadata")?.as_object()?.clone();

    let emote_data_adr74: Value = if meta.contains_key("emoteDataADR74") {
        let mut adr = meta.remove("emoteDataADR74").unwrap();
        if let Some(reps) = adr.get_mut("representations") {
            map_representations(reps, entity, content_public_url);
        }
        adr
    } else {
        let data = meta.remove("data").unwrap_or_else(|| json!({}));
        let emote_data_v0 = meta.remove("emoteDataV0");
        let loop_val = emote_data_v0
            .as_ref()
            .and_then(|v| v.get("loop"))
            .cloned()
            .unwrap_or(Value::Bool(false));
        let tags = data.get("tags").cloned().unwrap_or_else(|| json!([]));
        let mut reps = data
            .get("representations")
            .cloned()
            .unwrap_or_else(|| json!([]));
        map_representations(&mut reps, entity, content_public_url);
        json!({
            "category": "dance",
            "tags": tags,
            "loop": loop_val,
            "representations": reps,
        })
    };

    rewrite_image_thumbnail(&mut meta, entity, content_public_url);
    meta.insert("emoteDataADR74".into(), emote_data_adr74);
    Some(Value::Object(meta))
}

thread_local! {
    static COLLATOR: std::cell::RefCell<feruca::Collator> = std::cell::RefCell::new(feruca::Collator::default());
}

/// Unicode Collation Algorithm order, the `localeCompare` the reference
/// lambdas sorted names and URNs with.
pub fn locale_cmp(a: &str, b: &str) -> std::cmp::Ordering {
    COLLATOR.with(|c| c.borrow_mut().collate(a, b))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entity() -> Value {
        json!({
            "id": "Qm1",
            "content": [
                { "file": "male/hat.glb", "hash": "bafyhat" },
                { "file": "thumbnail.png", "hash": "bafythumb" },
                { "file": "image.png", "hash": "bafyimage" }
            ],
            "metadata": {
                "id": "urn:decentraland:matic:collections-v2:0xc0ffee:1",
                "name": "Hat",
                "image": "image.png",
                "thumbnail": "thumbnail.png",
                "data": {
                    "category": "hat",
                    "representations": [
                        { "bodyShapes": ["urn:decentraland:off-chain:base-avatars:BaseMale"], "mainFile": "male/hat.glb", "contents": ["male/hat.glb", "missing.glb"] }
                    ]
                }
            }
        })
    }

    #[test]
    fn wearable_definition_rewrites_files_to_content_urls() {
        let def = extract_wearable_definition(&entity(), "http://localhost/content").unwrap();
        assert_eq!(def["image"], "http://localhost/content/contents/bafyimage");
        assert_eq!(
            def["thumbnail"],
            "http://localhost/content/contents/bafythumb"
        );
        let contents = &def["data"]["representations"][0]["contents"];
        assert_eq!(
            contents[0],
            json!({ "key": "male/hat.glb", "url": "http://localhost/content/contents/bafyhat" })
        );
        assert_eq!(
            contents[1],
            json!({ "key": "missing.glb", "url": "" }),
            "an unlisted file keeps its key with an empty url"
        );
        assert_eq!(def["name"], "Hat");
        assert!(extract_wearable_definition(&json!({ "id": "x" }), "u").is_none());
    }

    #[test]
    fn emote_definition_builds_adr74_from_v0_and_keeps_an_existing_one() {
        let mut e = entity();
        e["metadata"]["emoteDataV0"] = json!({ "loop": true });
        let def = extract_emote_definition(&e, "http://localhost/content/").unwrap();
        assert_eq!(def["emoteDataADR74"]["loop"], true);
        assert_eq!(def["emoteDataADR74"]["category"], "dance");
        assert_eq!(
            def["emoteDataADR74"]["representations"][0]["contents"][0]["url"],
            "http://localhost/content/contents/bafyhat"
        );
        assert!(
            def.get("data").is_none(),
            "the v0 data block is folded into emoteDataADR74"
        );

        let mut e = entity();
        e["metadata"].as_object_mut().unwrap().remove("data");
        e["metadata"]["emoteDataADR74"] = json!({ "category": "fun", "loop": false, "representations": [{ "contents": ["male/hat.glb"] }] });
        let def = extract_emote_definition(&e, "http://localhost/content").unwrap();
        assert_eq!(def["emoteDataADR74"]["category"], "fun");
        assert_eq!(
            def["emoteDataADR74"]["representations"][0]["contents"][0]["key"],
            "male/hat.glb"
        );
    }

    #[test]
    fn rarity_rank_matches_reference_order() {
        assert_eq!(rarity_rank("common"), 0);
        assert_eq!(rarity_rank("legendary"), 4);
        assert_eq!(rarity_rank("unique"), 7);
        assert_eq!(rarity_rank("not-a-rarity"), -1);
        assert_eq!(rarity_rank(""), -1);
    }

    #[test]
    fn locale_cmp_places_basefemale_between_balbo_and_basketball() {
        use std::cmp::Ordering;
        assert_eq!(locale_cmp("balbo_beard", "BaseFemale"), Ordering::Less);
        assert_eq!(
            locale_cmp("BaseFemale", "basketball_shorts"),
            Ordering::Less
        );
        assert_eq!(
            locale_cmp("apple", "Apple"),
            Ordering::Less,
            "lowercase first on a case tiebreak"
        );
        assert_eq!(locale_cmp("same", "same"), Ordering::Equal);
    }
}

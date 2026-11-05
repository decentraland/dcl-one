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

/// lamb2's `contentServerUrl`: `CONTENT_URL` lowercased, `http://` added
/// when it has no scheme, trailing `/` stripped
/// (decentraland/lamb2:src/adapters/content-server-url.ts:8-17).
fn content_base(content_public_url: &str) -> String {
    let mut base = content_public_url.to_lowercase();
    if !base.starts_with("http") {
        base = format!("http://{base}");
    }
    while base.ends_with('/') {
        base.pop();
    }
    base
}

/// `<base>/contents/<hash>` of the entity file named `file_name`, `None`
/// when the entity lists no such file.
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
    Some(format!(
        "{}/contents/{hash}",
        content_base(content_public_url)
    ))
}

/// Each representation's `contents` file names as `{key, url}`, the `url`
/// left out for a file the entity does not list. `None` when the
/// representations or a `contents` list are not arrays, where lamb2's
/// `.map` throws and the item gets no definition
/// (decentraland/lamb2:src/adapters/definitions.ts:27-40;
/// decentraland/lamb2:src/adapters/definitions-fetcher.ts:49-60).
pub fn map_representations(
    reps: &Value,
    entity: &Value,
    content_public_url: &str,
) -> Option<Value> {
    let mapped = reps
        .as_array()?
        .iter()
        .map(|rep| {
            let contents: Vec<Value> = rep
                .get("contents")?
                .as_array()?
                .iter()
                .map(|file| {
                    let mut entry = Map::new();
                    entry.insert("key".into(), file.clone());
                    if let Some(url) = file
                        .as_str()
                        .and_then(|f| content_url(entity, f, content_public_url))
                    {
                        entry.insert("url".into(), Value::String(url));
                    }
                    Value::Object(entry)
                })
                .collect();
            let mut rep = rep.as_object()?.clone();
            rep.insert("contents".into(), Value::Array(contents));
            Some(Value::Object(rep))
        })
        .collect::<Option<Vec<Value>>>()?;
    Some(Value::Array(mapped))
}

/// `image` becomes its content URL when the entity lists the file and is
/// kept as it is otherwise; `thumbnail` becomes its content URL or is left
/// out (lamb2's `undefined`).
pub fn rewrite_image_thumbnail(
    def: &mut Map<String, Value>,
    entity: &Value,
    content_public_url: &str,
) {
    let url_of = |def: &Map<String, Value>, key: &str| {
        def.get(key)
            .and_then(Value::as_str)
            .and_then(|file| content_url(entity, file, content_public_url))
    };
    if let Some(url) = url_of(def, "image") {
        def.insert("image".into(), Value::String(url));
    }
    match url_of(def, "thumbnail") {
        Some(url) => {
            def.insert("thumbnail".into(), Value::String(url));
        }
        None => {
            def.remove("thumbnail");
        }
    }
}

/// JavaScript truthiness of a JSON value.
fn truthy(v: Option<&Value>) -> bool {
    match v {
        None | Some(Value::Null) | Some(Value::Bool(false)) => false,
        Some(Value::String(s)) => !s.is_empty(),
        Some(Value::Number(n)) => n.as_f64().is_some_and(|f| f != 0.0),
        Some(_) => true,
    }
}

/// The entity's metadata object when it has a truthy `id`; lamb2 skips an
/// entity without one (decentraland/lamb2:src/adapters/definitions-fetcher.ts:44-47).
fn definition_metadata(entity: &Value) -> Option<Map<String, Value>> {
    let metadata = entity.get("metadata")?.as_object()?;
    truthy(metadata.get("id")).then(|| metadata.clone())
}

/// `extractWearableDefinitionFromEntity`
/// (decentraland/lamb2:src/adapters/definitions.ts:4-25); `None` where it
/// throws (no `data.representations` array).
pub fn extract_wearable_definition(entity: &Value, content_public_url: &str) -> Option<Value> {
    let mut def = definition_metadata(entity)?;
    let data = def.get_mut("data")?.as_object_mut()?;
    let reps = map_representations(data.get("representations")?, entity, content_public_url)?;
    data.insert("representations".into(), reps);
    rewrite_image_thumbnail(&mut def, entity, content_public_url);
    Some(Value::Object(def))
}

/// `extractEmoteDefinitionFromEntity`
/// (decentraland/lamb2:src/adapters/definitions.ts:61-102): an ADR-74 emote
/// keeps its `emoteDataADR74`; a legacy one folds `data` and `emoteDataV0`
/// into `{category: "dance", tags?, loop?, representations}`, `tags` left
/// out when `data` has none and `loop` read from `emoteDataV0` when it is
/// there (`false` otherwise). `None` where lamb2 throws.
pub fn extract_emote_definition(entity: &Value, content_public_url: &str) -> Option<Value> {
    let mut meta = definition_metadata(entity)?;

    let emote_data_adr74: Value = if let Some(adr) = meta.remove("emoteDataADR74") {
        let reps = map_representations(adr.get("representations")?, entity, content_public_url)?;
        let mut adr = adr.as_object()?.clone();
        adr.insert("representations".into(), reps);
        Value::Object(adr)
    } else {
        let data = meta.remove("data")?;
        let reps = map_representations(data.get("representations")?, entity, content_public_url)?;
        let mut adr = Map::new();
        adr.insert("category".into(), json!("dance"));
        if let Some(tags) = data.get("tags") {
            adr.insert("tags".into(), tags.clone());
        }
        // `emoteDataV0.loop`: a `null` throws in lamb2, another non-object
        // reads `undefined` and leaves `loop` out.
        match meta.remove("emoteDataV0") {
            Some(Value::Null) => return None,
            Some(Value::Object(v0)) => {
                if let Some(l) = v0.get("loop") {
                    adr.insert("loop".into(), l.clone());
                }
            }
            Some(_) => {}
            None => {
                adr.insert("loop".into(), Value::Bool(false));
            }
        }
        adr.insert("representations".into(), reps);
        Value::Object(adr)
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
            json!({ "key": "missing.glb" }),
            "an unlisted file keeps its key and has no url"
        );
        assert_eq!(def["name"], "Hat");
        assert!(extract_wearable_definition(&json!({ "id": "x" }), "u").is_none());
    }

    #[test]
    fn emote_definition_reads_a_non_object_v0_as_lamb2() {
        let mut e = entity();
        e["metadata"]["emoteDataV0"] = json!("legacy");
        let def = extract_emote_definition(&e, "http://localhost/content/").unwrap();
        assert!(def["emoteDataADR74"].get("loop").is_none(), "{def}");
        assert_eq!(def["emoteDataADR74"]["category"], "dance");
        e["metadata"]["emoteDataV0"] = Value::Null;
        assert_eq!(
            extract_emote_definition(&e, "http://localhost/content/"),
            None,
            "lamb2 throws on a null emoteDataV0"
        );
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
    fn definitions_follow_lamb2_on_the_edges() {
        // The base URL is lowercased, without its trailing slashes.
        let def = extract_wearable_definition(&entity(), "HTTPS://Peer.Example/Content//").unwrap();
        assert_eq!(
            def["image"],
            "https://peer.example/content/contents/bafyimage"
        );

        // A thumbnail the entity does not list is left out; an image is kept.
        let mut e = entity();
        e["metadata"]["thumbnail"] = json!("gone.png");
        e["metadata"]["image"] = json!("gone.png");
        let def = extract_wearable_definition(&e, "http://localhost/content").unwrap();
        assert!(def.get("thumbnail").is_none());
        assert_eq!(def["image"], "gone.png");

        // No representations or no id: no definition.
        let mut e = entity();
        e["metadata"]["data"]
            .as_object_mut()
            .unwrap()
            .remove("representations");
        assert!(extract_wearable_definition(&e, "u").is_none());
        let mut e = entity();
        e["metadata"]["id"] = json!("");
        assert!(extract_wearable_definition(&e, "u").is_none());
        assert!(extract_emote_definition(&e, "u").is_none());

        // A legacy emote: `tags` only when `data` has them, `loop` from
        // `emoteDataV0` when present (left out when it has none), else false.
        let def = extract_emote_definition(&entity(), "u").unwrap();
        assert!(def["emoteDataADR74"].get("tags").is_none());
        assert_eq!(def["emoteDataADR74"]["loop"], false);
        let mut e = entity();
        e["metadata"]["data"]["tags"] = json!(["a"]);
        e["metadata"]["emoteDataV0"] = json!({});
        let def = extract_emote_definition(&e, "u").unwrap();
        assert_eq!(def["emoteDataADR74"]["tags"], json!(["a"]));
        assert!(def["emoteDataADR74"].get("loop").is_none());
        assert!(def.get("emoteDataV0").is_none());
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

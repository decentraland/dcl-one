//! The `POST /entities/active` element, written member by member. Upstream answers
//! `{...row.entity, id, metadata: {...row.entity.metadata, owner}}` over node-postgres' parse of
//! the `jsonb` column, so its members come in jsonb's order (key length, then bytes) with `id`,
//! `metadata` and `owner` spliced as the JS spread does (WR-15 in
//! `tests/fixtures/conformance/worlds/worlds-reads.md`). `serde_json::Value` sorts its keys
//! (the node builds without `preserve_order`, see the workspace `Cargo.toml`), so the stored
//! text is walked instead.

use serde_json::value::RawValue;

/// One JSON object's members in the order written, each value as written.
struct Members<'a>(Vec<(String, &'a RawValue)>);

impl<'de> serde::Deserialize<'de> for Members<'de> {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct Visit;
        impl<'de> serde::de::Visitor<'de> for Visit {
            type Value = Members<'de>;
            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("a JSON object")
            }
            fn visit_map<A: serde::de::MapAccess<'de>>(
                self,
                mut map: A,
            ) -> Result<Self::Value, A::Error> {
                let mut members = Vec::new();
                while let Some(member) = map.next_entry::<String, &'de RawValue>()? {
                    members.push(member);
                }
                Ok(Members(members))
            }
        }
        deserializer.deserialize_map(Visit)
    }
}

/// JSON text without the whitespace outside strings (`jsonb::text` writes
/// `", "` and `": "`; `JSON.stringify` writes none).
fn compact_json(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let (mut in_string, mut escaped) = (false, false);
    for c in text.chars() {
        if in_string {
            out.push(c);
            if escaped {
                escaped = false;
            } else if c == '\\' {
                escaped = true;
            } else if c == '"' {
                in_string = false;
            }
        } else if c == '"' {
            in_string = true;
            out.push(c);
        } else if !c.is_ascii_whitespace() {
            out.push(c);
        }
    }
    out
}

/// `{...object, [key]: value}` over written members: `key` keeps its place
/// when present and is appended otherwise.
fn spread_with(members: &mut Vec<(String, String)>, key: &str, value: String) {
    match members.iter_mut().find(|(k, _)| k == key) {
        Some(member) => member.1 = value,
        None => members.push((key.to_string(), value)),
    }
}

fn write_object(members: &[(String, String)]) -> serde_json::Result<String> {
    let mut out = String::from("{");
    for (i, (key, value)) in members.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        out.push_str(&serde_json::to_string(key)?);
        out.push(':');
        out.push_str(value);
    }
    out.push('}');
    Ok(out)
}

/// One `getEntityForWorlds` element from the stored entity's `jsonb::text`.
/// A metadata that is not an object spreads to nothing, as in JS.
fn active_entity(stored: &str, id: &str, owner: Option<&str>) -> serde_json::Result<Box<RawValue>> {
    let compact = compact_json(stored);
    let Members(members) = serde_json::from_str(&compact)?;
    let mut metadata: Vec<(String, String)> = members
        .iter()
        .find(|(k, _)| k == "metadata")
        .and_then(|(_, raw)| serde_json::from_str::<Members>(raw.get()).ok())
        .map(|Members(m)| {
            m.into_iter()
                .map(|(k, v)| (k, v.get().to_string()))
                .collect()
        })
        .unwrap_or_default();
    spread_with(&mut metadata, "owner", serde_json::to_string(&owner)?);
    let mut entity: Vec<(String, String)> = members
        .into_iter()
        .map(|(k, v)| (k, v.get().to_string()))
        .collect();
    spread_with(&mut entity, "id", serde_json::to_string(id)?);
    spread_with(&mut entity, "metadata", write_object(&metadata)?);
    RawValue::from_string(write_object(&entity)?)
}

/// The batch from `(entity_id, stored entity, owner)` rows: a stored entity that is not a JSON
/// object is left out and logged, so the other worlds still answer.
pub(super) fn active_entities(
    rows: impl IntoIterator<Item = (String, String, Option<String>)>,
) -> Vec<Box<RawValue>> {
    rows.into_iter()
        .filter_map(|(entity_id, entity, owner)| {
            active_entity(&entity, &entity_id, owner.as_deref())
                .inspect_err(|error| {
                    tracing::warn!(entity = %entity_id, %error, "stored entity is not an object; left out of /entities/active");
                })
                .ok()
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `{...row.entity, id, metadata: {...metadata, owner}}` over the text
    /// `jsonb` writes (members ordered by key length, then bytes): members keep
    /// their order, `id`, `metadata` and `owner` take their places, and nothing
    /// inside strings is touched.
    #[test]
    fn active_entity_spreads_in_the_stored_member_order() {
        let stored = r#"{"id": "old", "type": "scene", "content": [{"file": "a b.png", "hash": "h"}], "version": "v3", "metadata": {"main": "bin/index.js", "owner": "0xold", "scene": {"base": "0,0", "parcels": ["0,0"]}, "display": {"title": "Say \"hi\": \\ ok"}}, "pointers": ["0,0"], "timestamp": 1}"#;
        let got = active_entity(stored, "bafy", Some("0xnew")).unwrap();
        assert_eq!(
            got.get(),
            r#"{"id":"bafy","type":"scene","content":[{"file":"a b.png","hash":"h"}],"version":"v3","metadata":{"main":"bin/index.js","owner":"0xnew","scene":{"base":"0,0","parcels":["0,0"]},"display":{"title":"Say \"hi\": \\ ok"}},"pointers":["0,0"],"timestamp":1}"#
        );
    }

    #[test]
    fn active_entity_appends_what_the_entity_lacks() {
        let got = active_entity(r#"{"type": "scene", "timestamp": 2}"#, "bafy", None).unwrap();
        assert_eq!(
            got.get(),
            r#"{"type":"scene","timestamp":2,"id":"bafy","metadata":{"owner":null}}"#
        );
        let got = active_entity(
            r#"{"metadata": null, "type": "scene"}"#,
            "bafy",
            Some("0xa"),
        )
        .unwrap();
        assert_eq!(
            got.get(),
            r#"{"metadata":{"owner":"0xa"},"type":"scene","id":"bafy"}"#
        );
    }

    #[test]
    fn active_entities_leave_out_a_stored_entity_that_is_not_an_object() {
        let got = active_entities([
            ("a".to_string(), r#"{"type": "scene"}"#.to_string(), None),
            ("b".to_string(), "[]".to_string(), None),
            (
                "c".to_string(),
                "not json".to_string(),
                Some("0xc".to_string()),
            ),
        ]);
        let got: Vec<&str> = got.iter().map(|e| e.get()).collect();
        assert_eq!(
            got,
            vec![r#"{"type":"scene","id":"a","metadata":{"owner":null}}"#]
        );
    }
}

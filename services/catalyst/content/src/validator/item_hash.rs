//! The hashes an item's on-chain `contentHash` is compared with: the CIDv0
//! and CIDv1 of `JSON.stringify({ content, metadata })` (ADR-32), where
//! `content` is the entity's `{ key, hash }` pairs sorted by hash then key and
//! `metadata` is the entity file's metadata as JavaScript re-serializes it
//! ([`super::jsjson`]).

use std::cmp::Ordering;

use super::jsjson::{self, JsValue};
use super::types::{ContentMapping, Deployment};

/// JS `<` / `>` on strings: UTF-16 code unit order.
fn js_cmp(a: &str, b: &str) -> Ordering {
    a.encode_utf16().cmp(b.encode_utf16())
}

/// The bytes ADR-32 hashes, from the content mappings and the entity file.
fn hashed_payload(content: &[ContentMapping], entity_file: &[u8]) -> Option<Vec<u8>> {
    let entity: JsValue = serde_json::from_slice(entity_file).ok()?;
    entity.as_object()?;
    let mut sorted: Vec<&ContentMapping> = content.iter().collect();
    sorted.sort_by(|a, b| js_cmp(&a.hash, &b.hash).then_with(|| js_cmp(&a.file, &b.file)));
    let mut out = String::from("{\"content\":[");
    for (i, c) in sorted.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        out.push_str("{\"key\":");
        jsjson::write_string(&c.file, &mut out);
        out.push_str(",\"hash\":");
        jsjson::write_string(&c.hash, &mut out);
        out.push('}');
    }
    out.push(']');
    if let Some(metadata) = entity.get("metadata") {
        out.push_str(",\"metadata\":");
        jsjson::write(metadata, &mut out);
    }
    out.push('}');
    Some(out.into_bytes())
}

/// The CIDv0 and CIDv1 an item's `contentHash` may equal; `None` when the
/// entity file is not among the deployment's files or does not parse.
pub(super) fn deployment_hashes(deployment: &Deployment<'_>) -> Option<[String; 2]> {
    let file = deployment.files.get(&deployment.entity.id)?;
    let payload = hashed_payload(&deployment.entity.content, file)?;
    Some([
        crate::auth::hash_bytes(&payload),
        crate::auth::hash_bytes_v1(&payload),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mapping(file: &str, hash: &str) -> ContentMapping {
        ContentMapping {
            file: file.to_string(),
            hash: hash.to_string(),
        }
    }

    fn payload(content: &[ContentMapping], file: &str) -> String {
        String::from_utf8(hashed_payload(content, file.as_bytes()).unwrap()).unwrap()
    }

    #[test]
    fn content_is_sorted_by_hash_then_key_and_metadata_keeps_document_order() {
        let content = [
            mapping("b.png", "bafy2"),
            mapping("z.glb", "bafy1"),
            mapping("a.png", "bafy2"),
        ];
        let file = r#"{"type":"wearable","metadata":{"name":"hat","id":"x","data":{"tags":[],"2":1,"1":true}},"content":[]}"#;
        assert_eq!(
            payload(&content, file),
            r#"{"content":[{"key":"z.glb","hash":"bafy1"},{"key":"a.png","hash":"bafy2"},{"key":"b.png","hash":"bafy2"}],"metadata":{"name":"hat","id":"x","data":{"1":true,"2":1,"tags":[]}}}"#
        );
    }

    #[test]
    fn numbers_and_strings_print_as_javascript_does() {
        let file = r#"{"metadata":{"a":1.0,"b":-0.0,"c":1e21,"d":1.5e-7,"e":0.000001,"f":100,"g":"q\"é\n/","a":2}}"#;
        assert_eq!(
            payload(&[], file),
            "{\"content\":[],\"metadata\":{\"a\":2,\"b\":0,\"c\":1e+21,\"d\":1.5e-7,\"e\":0.000001,\"f\":100,\"g\":\"q\\\"é\\n/\"}}"
        );
    }

    /// Approved mainnet items: the entity file as the peers serve it and the
    /// `contentHash` the collections-matic-mainnet subgraph holds for it.
    #[test]
    fn approved_items_hash_to_their_on_chain_content_hash() {
        for (id, file, content_hash) in [
            (
                "QmWT5vttQgVPwDf3vyDmEBSi9FASFbcW9zRcedym7wMQgb",
                &include_bytes!(
                    "item_hash_fixtures/QmWT5vttQgVPwDf3vyDmEBSi9FASFbcW9zRcedym7wMQgb.entity.json"
                )[..],
                "QmTytsdBzL5Brr1JrWxwNvSvc2fWeXeTvvBbwT5qCVs7KF",
            ),
            (
                "bafkreicxp2hlkzqvax4dwm2bqic6owqv4qipd4a3zs5zjsfs2l2kas5fhm",
                &include_bytes!(
                    "item_hash_fixtures/bafkreicxp2hlkzqvax4dwm2bqic6owqv4qipd4a3zs5zjsfs2l2kas5fhm.entity.json"
                )[..],
                "bafkreidhcy4k5qpz6dwqcufld3vo27axr7vfzgj3tkyx4jgj53zsxvrj7i",
            ),
        ] {
            let entity = super::super::entity_parser::parse_entity_from_bytes(file, id).unwrap();
            let files = std::collections::HashMap::from([(
                id.to_string(),
                bytes::Bytes::copy_from_slice(file),
            )]);
            let auth_chain = Default::default();
            let deployment = Deployment {
                entity: &entity,
                files: &files,
                auth_chain: &auth_chain,
            };
            let hashes = deployment_hashes(&deployment).unwrap();
            assert!(
                hashes.contains(&content_hash.to_string()),
                "{id}: {hashes:?}"
            );
        }
    }

    #[test]
    fn missing_metadata_is_omitted_and_null_is_kept() {
        assert_eq!(payload(&[], r#"{"type":"wearable"}"#), r#"{"content":[]}"#);
        assert_eq!(
            payload(&[], r#"{"metadata":null}"#),
            r#"{"content":[],"metadata":null}"#
        );
    }
}

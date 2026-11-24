//! The metadata object: its parse, the predicates a service composes into
//! its `metadata_validator`, and the key guard of the pre-6.0.0 payload.

use std::sync::Arc;

use serde_json::{Map, Value};

use super::error::{RequestAuthError, malformed, safe};
use super::options::OptionsError;
use crate::js_date::js_trim;

pub type Metadata = Map<String, Value>;

/// A `metadata_validator`: `false` refuses the request with 400.
pub type MetadataPredicate = Arc<dyn Fn(&Metadata) -> bool + Send + Sync>;

const SIGNER: &str = "signer";

/// The metadata header as an object: absent reads as `{}`, JSON `null` as
/// `{}`; anything but an object is refused.
pub(crate) fn parse_metadata(raw: Option<&str>) -> Result<Metadata, RequestAuthError> {
    let refused = || {
        malformed(format!(
            "Invalid chain metadata: \"{}\"",
            safe(raw.unwrap_or(""))
        ))
    };
    match serde_json::from_str::<Value>(raw.unwrap_or("{}")).map_err(|_| refused())? {
        Value::Null => Ok(Map::new()),
        Value::Object(object) => Ok(object),
        _ => Err(refused()),
    }
}

fn is_canonical(value: &str) -> bool {
    value == js_trim(value).to_lowercase()
}

fn has_folded_variant(metadata: &Metadata, field: &str) -> bool {
    let folded = field.to_lowercase();
    metadata
        .keys()
        .any(|key| key != field && key.to_lowercase() == folded)
}

fn check_arguments(function: &str, values: &[&str]) -> Result<Vec<String>, OptionsError> {
    if values.is_empty() {
        return Err(OptionsError::new(format!(
            "{function}() requires at least one value"
        )));
    }
    for value in values {
        if value.is_empty() {
            return Err(OptionsError::new(format!(
                "{function}() expects non-empty string values"
            )));
        }
        if !is_canonical(value) {
            return Err(OptionsError::new(format!(
                "{function}() expects canonical (trimmed, lowercase) values, got: {value}"
            )));
        }
    }
    Ok(values.iter().map(|v| v.to_string()).collect())
}

/// `field`, when present, is a trimmed lowercase string; absent passes. A key
/// that folds to `field` without being spelled exactly that is refused.
pub fn canonical_field(field: &str) -> MetadataPredicate {
    let field = field.to_string();
    Arc::new(move |metadata| {
        if has_folded_variant(metadata, &field) {
            return false;
        }
        match metadata.get(&field) {
            None => true,
            Some(Value::String(value)) => is_canonical(value),
            Some(_) => false,
        }
    })
}

/// `field` is present, canonical, and one of `values`.
pub fn require_canonical_field(
    field: &str,
    values: &[&str],
) -> Result<MetadataPredicate, OptionsError> {
    require_field("requireCanonicalField", field, values)
}

fn require_field(
    function: &str,
    field: &str,
    values: &[&str],
) -> Result<MetadataPredicate, OptionsError> {
    let values = check_arguments(function, values)?;
    let field = field.to_string();
    Ok(Arc::new(move |metadata| {
        if has_folded_variant(metadata, &field) {
            return false;
        }
        matches!(metadata.get(&field), Some(Value::String(value)) if is_canonical(value) && values.contains(value))
    }))
}

/// Refuses a request whose `signer` is one of `signers`, or is not canonical;
/// one with no `signer` passes.
pub fn reject_if_signer(signers: &[&str]) -> Result<MetadataPredicate, OptionsError> {
    let signers = check_arguments("rejectIfSigner", signers)?;
    let canonical = canonical_field(SIGNER);
    Ok(Arc::new(move |metadata| {
        canonical(metadata)
            && !matches!(metadata.get(SIGNER), Some(Value::String(value)) if signers.contains(value))
    }))
}

/// Requires `signer` to be one of `signers`.
pub fn require_signer(signers: &[&str]) -> Result<MetadataPredicate, OptionsError> {
    require_field("requireSigner", SIGNER, signers)
}

/// The objects a path segment applies to: arrays are flattened, nested ones too.
fn objects_to_inspect<'a>(value: &'a Value, out: &mut Vec<&'a Metadata>) {
    match value {
        Value::Array(items) => items.iter().for_each(|item| objects_to_inspect(item, out)),
        Value::Object(object) => out.push(object),
        _ => {}
    }
}

/// Refuses metadata signed with the pre-6.0.0 payload whose keys are not in
/// the declared spelling: that payload folds the metadata, so a key differing
/// only in case keeps a valid signature. Dotted paths address nested fields.
pub(crate) fn assert_legacy_metadata_keys(
    metadata: &Metadata,
    canonical_keys: &[String],
) -> Result<(), RequestAuthError> {
    let root = Value::Object(metadata.clone());
    for declared in canonical_keys {
        let mut containers: Vec<&Value> = vec![&root];
        for segment in declared.split('.') {
            let folded = segment.to_lowercase();
            let mut next = Vec::new();
            let mut objects = Vec::new();
            for container in &containers {
                objects_to_inspect(container, &mut objects);
            }
            for object in objects {
                let delivered: Vec<&String> = object
                    .keys()
                    .filter(|key| key.to_lowercase() == folded)
                    .collect();
                match delivered.as_slice() {
                    [] => continue,
                    [key] if key.as_str() == segment => next.push(&object[segment]),
                    [key] => {
                        return Err(malformed(format!(
                            "Invalid chain metadata: expected \"{}\", got \"{}\"",
                            safe(segment),
                            safe(key)
                        )));
                    }
                    many => {
                        return Err(malformed(format!(
                            "Invalid chain metadata: \"{}\" delivered under {} spellings",
                            safe(segment),
                            many.len()
                        )));
                    }
                }
            }
            if next.is_empty() {
                break;
            }
            containers = next;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn meta(raw: &str) -> Metadata {
        parse_metadata(Some(raw)).unwrap()
    }

    #[test]
    fn metadata_is_an_object_null_or_absent() {
        assert_eq!(parse_metadata(None).unwrap(), Map::new());
        assert_eq!(parse_metadata(Some("null")).unwrap(), Map::new());
        assert_eq!(meta(r#"{"a":1,"a":2}"#)["a"], 2);
        for bad in ["", "[]", "1", "\"x\"", "{", "{}, {}"] {
            assert_eq!(
                parse_metadata(Some(bad)).unwrap_err().status(),
                400,
                "{bad:?}"
            );
        }
    }

    #[test]
    fn signer_predicates_refuse_respellings() {
        let reject = reject_if_signer(&["decentraland-kernel-scene"]).unwrap();
        assert!(reject(&meta("{}")));
        assert!(reject(&meta(r#"{"signer":"dcl:explorer"}"#)));
        assert!(!reject(&meta(r#"{"signer":"decentraland-kernel-scene"}"#)));
        assert!(!reject(&meta(r#"{"signer":"Decentraland-Kernel-Scene"}"#)));
        assert!(!reject(&meta(r#"{"Signer":"dcl:explorer"}"#)));
        assert!(!reject(&meta(r#"{"signer":"dcl:explorer﻿"}"#)));
        assert!(!reject(&meta(r#"{"signer":1}"#)));
        // JS `trim` keeps U+0085, so a value ending in it is still canonical.
        assert!(reject(&meta("{\"signer\":\"dcl\u{85}\"}")));

        let require = require_signer(&["decentraland-kernel-scene"]).unwrap();
        assert!(require(&meta(r#"{"signer":"decentraland-kernel-scene"}"#)));
        assert!(!require(&meta("{}")));
        assert!(!require(&meta(r#"{"signer":"dcl:explorer"}"#)));
        assert!(!require(&meta(
            r#"{"signer":"decentraland-kernel-scene","SIGNER":"x"}"#
        )));

        let intent = require_canonical_field("intent", &["dcl:explorer:comms-handshake"]).unwrap();
        assert!(intent(&meta(
            r#"{"intent":"dcl:explorer:comms-handshake"}"#
        )));
        assert!(!intent(&meta(
            r#"{"intent":" dcl:explorer:comms-handshake"}"#
        )));

        assert!(canonical_field("signer")(&meta("{}")));
        assert!(!canonical_field("signer")(&meta(r#"{"signer":"A"}"#)));

        assert!(require_signer(&[]).is_err());
        assert!(require_signer(&[""]).is_err());
        assert!(reject_if_signer(&["Scene"]).is_err());
    }

    #[test]
    fn the_legacy_key_guard_follows_paths_through_arrays() {
        let keys = |list: &[&str]| list.iter().map(|k| k.to_string()).collect::<Vec<_>>();
        let guard = |raw: &str, list: &[&str]| assert_legacy_metadata_keys(&meta(raw), &keys(list));
        assert!(guard(r#"{"signer":"x"}"#, &["signer"]).is_ok());
        assert!(guard(r#"{"other":"x"}"#, &["signer"]).is_ok());
        assert!(guard(r#"{"Signer":"x"}"#, &["signer"]).is_err());
        assert!(guard(r#"{"signer":"x","SIGNER":"y"}"#, &["signer"]).is_err());
        assert!(guard(r#"{"realm":{"serverName":"a"}}"#, &["realm.serverName"]).is_ok());
        assert!(guard(r#"{"realm":{"servername":"a"}}"#, &["realm.serverName"]).is_err());
        assert!(guard(r#"{"Realm":{"serverName":"a"}}"#, &["realm.serverName"]).is_err());
        assert!(
            guard(
                r#"{"items":[{"sceneId":"a"},[{"SceneId":"b"}]]}"#,
                &["items.sceneId"]
            )
            .is_err()
        );
        assert!(guard(r#"{"items":[1,"x",{"sceneId":"a"}]}"#, &["items.sceneId"]).is_ok());
        let err = guard(r#"{"Signer":"x"}"#, &["signer"]).unwrap_err();
        assert_eq!(
            err.to_string(),
            "Invalid chain metadata: expected \"signer\", got \"Signer\""
        );
    }
}

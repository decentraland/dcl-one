//! The shape checks of a world publish: canonical parcel sets, bounds, the
//! entity file cap, the auth chain fields of the multipart form and the
//! navmap thumbnail rule.

use std::collections::BTreeMap;

use serde_json::{Value, json};

use crate::auth::{
    MAX_AUTH_CHAIN_LINKS, canonicalize_pointer, is_canonical_pointer, parse_pointer,
};

pub(super) const MAX_ENTITY_FILE_SIZE_BYTES: usize = 5 * 1024 * 1024;

pub const MAX_DEPLOY_FILES: usize = 1000;
pub const MAX_DEPLOY_FILE_BYTES: usize = 50 * 1024 * 1024;

const MIN_PARCEL_COORDINATE: i64 = -150;
const MAX_PARCEL_COORDINATE: i64 = 150;

pub(crate) fn canon_pointer(s: &str) -> String {
    canonicalize_pointer(s)
}

pub(super) fn canon_pointer_set(values: &[Value]) -> Vec<String> {
    let mut out: Vec<String> = values
        .iter()
        .filter_map(|v| v.as_str().map(canon_pointer))
        .collect();
    out.sort();
    out.dedup();
    out
}

/// True iff `values` is a non-empty array of unique, already-canonical parcel coordinate
/// strings -- the shape `@dcl/schemas` `SceneParcels` enforces. Non-canonical spellings and
/// duplicates are rejected rather than normalized, so a deployment is authorized and sized
/// against exactly the literal set of parcels it is placed on.
pub(super) fn is_canonical_parcel_set(values: &[Value]) -> bool {
    if values.is_empty() {
        return false;
    }
    let mut seen = std::collections::HashSet::with_capacity(values.len());
    for v in values {
        match v.as_str() {
            Some(s) if is_canonical_pointer(s) => {
                if !seen.insert(s) {
                    return false;
                }
            }
            _ => return false,
        }
    }
    true
}

pub(super) fn validate_parcel_in_bounds(parcel: &str) -> Result<(), String> {
    let (x, y) = match parse_pointer(parcel) {
        Some(xy) => xy,
        None => return Err(format!("Invalid coordinate format: {parcel}")),
    };
    if !(MIN_PARCEL_COORDINATE..=MAX_PARCEL_COORDINATE).contains(&x) {
        return Err(format!(
            "Coordinate X value {x} is out of bounds. Must be between {MIN_PARCEL_COORDINATE} and {MAX_PARCEL_COORDINATE}."
        ));
    }
    if !(MIN_PARCEL_COORDINATE..=MAX_PARCEL_COORDINATE).contains(&y) {
        return Err(format!(
            "Coordinate Y value {y} is out of bounds. Must be between {MIN_PARCEL_COORDINATE} and {MAX_PARCEL_COORDINATE}."
        ));
    }
    Ok(())
}

fn too_long_message() -> String {
    format!("Auth chain is too long; the maximum allowed is {MAX_AUTH_CHAIN_LINKS} elements")
}

fn indexed_field<'a>(
    fields: &'a BTreeMap<String, String>,
    i: i64,
    part: &str,
) -> Result<&'a String, String> {
    fields
        .get(&format!("authChain[{i}][{part}]"))
        .ok_or_else(|| format!("Missing auth chain element at index {i}"))
}

/// The `authChain` JSON field, or the `authChain[i][type|payload|signature]`
/// indexed fields the older CLIs send.
pub(super) fn extract_auth_chain_from_fields(
    fields: &BTreeMap<String, String>,
) -> Result<Value, String> {
    if let Some(chain_str) = fields.get("authChain") {
        let chain: Value =
            serde_json::from_str(chain_str).map_err(|_| "Invalid auth chain".to_string())?;
        let arr = chain
            .as_array()
            .ok_or_else(|| "Invalid auth chain".to_string())?;
        if arr.len() > MAX_AUTH_CHAIN_LINKS {
            return Err(too_long_message());
        }
        return Ok(chain);
    }

    let mut biggest_index: i64 = -1;
    for key in fields.keys() {
        if let Some(rest) = key.strip_prefix("authChain[")
            && let Some(idx_str) = rest.split(']').next()
            && let Ok(idx) = idx_str.parse::<i64>()
            && idx > biggest_index
        {
            biggest_index = idx;
        }
    }

    if biggest_index == -1 {
        return Err("No auth chain can be derived".to_string());
    }
    if biggest_index >= MAX_AUTH_CHAIN_LINKS as i64 {
        return Err(too_long_message());
    }

    let mut chain = Vec::new();
    for i in 0..=biggest_index {
        let payload = indexed_field(fields, i, "payload")?;
        let signature = indexed_field(fields, i, "signature")?;
        let link_type = indexed_field(fields, i, "type")?;
        chain.push(json!({ "type": link_type, "payload": payload, "signature": signature }));
    }
    Ok(Value::Array(chain))
}

pub(super) fn entity_file_too_large_error() -> String {
    format!(
        "The entity file is too large. The maximum allowed size is {MAX_ENTITY_FILE_SIZE_BYTES} bytes."
    )
}

fn has_uri_scheme(path: &str) -> bool {
    let mut chars = path.chars();
    match chars.next() {
        Some(c) if c.is_ascii_alphabetic() => {}
        _ => return false,
    }
    for c in chars {
        match c {
            ':' => return true,
            c if c.is_ascii_alphanumeric() || matches!(c, '+' | '.' | '-') => {}
            _ => return false,
        }
    }
    false
}

fn is_relative_thumbnail_path(path: &str) -> bool {
    if has_uri_scheme(path) || path.starts_with('/') {
        return false;
    }
    let js_ws = |c: char| c.is_whitespace() || c == '\u{FEFF}';
    if path.starts_with(js_ws) || path.ends_with(js_ws) {
        return false;
    }
    if path.chars().any(|c| c.is_control()) {
        return false;
    }
    !path.contains(['<', '>', '"'])
}

pub(super) fn validate_navmap_thumbnail(entity: &Value, errors: &mut Vec<String>) {
    let thumb = match entity
        .get("metadata")
        .and_then(|m| m.get("display"))
        .and_then(|d| d.get("navmapThumbnail"))
    {
        None | Some(Value::Null) => return,
        Some(Value::String(s)) if s.is_empty() => return,
        Some(Value::Bool(false)) => return,
        Some(Value::Number(n)) if n.as_f64() == Some(0.0) => return,
        Some(Value::String(s)) => s.as_str(),
        Some(other) => {
            errors.push(format!("Scene thumbnail '{other}' must be a relative path to a file included in the deployment."));
            return;
        }
    };
    if !is_relative_thumbnail_path(thumb) {
        errors.push(format!("Scene thumbnail '{thumb}' must be a relative path to a file included in the deployment."));
        return;
    }
    let file_present = matches!(
        entity.get("content"),
        Some(Value::Array(items)) if items.iter().any(|item| item.get("file").and_then(|f| f.as_str()) == Some(thumb))
    );
    if !file_present {
        errors.push(format!(
            "Scene thumbnail '{thumb}' must be a file included in the deployment."
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canon_pointer_set_normalizes_and_sorts() {
        let a = canon_pointer_set(&[json!("1,2"), json!("0,0"), json!(" 0,0 ")]);
        assert_eq!(a, vec!["0,0".to_string(), "1,2".to_string()]);
    }

    #[test]
    fn canon_pointer_numerically_normalizes() {
        assert_eq!(canon_pointer("00,00"), "0,0");
        assert_eq!(canon_pointer("-0,-0"), "0,0");
        assert_eq!(canon_pointer(" 01 , 002 "), "1,2");
        assert_eq!(canon_pointer("-05,10"), "-5,10");
        assert_eq!(canon_pointer("00,00"), canon_pointer("0,0"));
        assert_eq!(canon_pointer("not-a-parcel"), "not-a-parcel");
        assert_eq!(canon_pointer("1,2,3"), "1,2,3");
        assert_eq!(canon_pointer("1e2,3"), "1e2,3");
    }

    #[test]
    fn canon_pointer_set_treats_leading_zeros_as_equal() {
        let pointers = canon_pointer_set(&[json!("00,00"), json!("01,00")]);
        let parcels = canon_pointer_set(&[json!("0,0"), json!("1,0")]);
        assert_eq!(pointers, parcels);
    }

    #[test]
    fn parcel_bounds_validation_matches_upstream() {
        assert!(validate_parcel_in_bounds("0,0").is_ok());
        assert!(validate_parcel_in_bounds("-150,150").is_ok());
        assert!(validate_parcel_in_bounds("150,-150").is_ok());
        assert!(
            validate_parcel_in_bounds("151,0")
                .unwrap_err()
                .contains("Coordinate X value 151 is out of bounds")
        );
        assert!(
            validate_parcel_in_bounds("0,-151")
                .unwrap_err()
                .contains("Coordinate Y value -151 is out of bounds")
        );
        assert!(
            validate_parcel_in_bounds("garbage")
                .unwrap_err()
                .contains("Invalid coordinate format")
        );
    }

    #[test]
    fn is_canonical_parcel_set_rejects_noncanonical_and_duplicates() {
        assert!(is_canonical_parcel_set(&[json!("0,0"), json!("0,1")]));
        assert!(is_canonical_parcel_set(&[json!("-5,10")]));
        assert!(!is_canonical_parcel_set(&[]));
        assert!(!is_canonical_parcel_set(&[json!("0,0"), json!("0,0")]));
        assert!(!is_canonical_parcel_set(&[json!("00,0")]));
        assert!(!is_canonical_parcel_set(&[json!(" 0,1 ")]));
        assert!(!is_canonical_parcel_set(&[json!("-0,0")]));
        assert!(!is_canonical_parcel_set(&[json!(0)]));
    }

    #[test]
    fn pointers_equal_scene_parcels_after_canonicalization() {
        let pointers = canon_pointer_set(&[json!("0,0"), json!(" 1,1 "), json!("0,0")]);
        let parcels = canon_pointer_set(&[json!("1,1"), json!("0,0")]);
        assert_eq!(pointers, parcels);
        let mismatch = canon_pointer_set(&[json!("0,0"), json!("2,2")]);
        assert_ne!(pointers, mismatch);
    }

    #[test]
    fn extract_auth_chain_indexed_and_json() {
        let mut f = BTreeMap::new();
        f.insert("authChain[0][type]".into(), "SIGNER".into());
        f.insert("authChain[0][payload]".into(), "0xabc".into());
        f.insert("authChain[0][signature]".into(), "".into());
        let v = extract_auth_chain_from_fields(&f).unwrap();
        assert_eq!(v.as_array().unwrap().len(), 1);
        assert_eq!(v[0]["type"], "SIGNER");

        let mut g = BTreeMap::new();
        g.insert(
            "authChain".into(),
            r#"[{"type":"SIGNER","payload":"0x1"}]"#.into(),
        );
        let v = extract_auth_chain_from_fields(&g).unwrap();
        assert_eq!(v[0]["payload"], "0x1");
    }

    #[test]
    fn extract_auth_chain_requires_something() {
        assert!(extract_auth_chain_from_fields(&BTreeMap::new()).is_err());
        let mut f = BTreeMap::new();
        f.insert("authChain".to_string(), "not-json".to_string());
        assert_eq!(
            extract_auth_chain_from_fields(&f),
            Err("Invalid auth chain".to_string())
        );
        f.insert("authChain".to_string(), r#"{"a":1}"#.to_string());
        assert_eq!(
            extract_auth_chain_from_fields(&f),
            Err("Invalid auth chain".to_string())
        );
    }

    #[test]
    fn rejects_over_length_chains_and_missing_indexed_elements() {
        let mut f = BTreeMap::new();
        for i in 0..=MAX_AUTH_CHAIN_LINKS {
            f.insert(format!("authChain[{i}][type]"), "SIGNER".to_string());
            f.insert(format!("authChain[{i}][payload]"), format!("0xp{i}"));
            f.insert(format!("authChain[{i}][signature]"), format!("0xs{i}"));
        }
        assert!(
            extract_auth_chain_from_fields(&f)
                .unwrap_err()
                .starts_with("Auth chain is too long")
        );
        let mut g = BTreeMap::new();
        let long: Vec<Value> = (0..=MAX_AUTH_CHAIN_LINKS)
            .map(|_| json!({"type":"SIGNER","payload":"p","signature":"s"}))
            .collect();
        g.insert(
            "authChain".to_string(),
            serde_json::to_string(&long).unwrap(),
        );
        assert!(
            extract_auth_chain_from_fields(&g)
                .unwrap_err()
                .starts_with("Auth chain is too long")
        );
        let mut h = BTreeMap::new();
        h.insert("authChain[0][type]".to_string(), "SIGNER".to_string());
        h.insert("authChain[0][payload]".to_string(), "0xp".to_string());
        assert_eq!(
            extract_auth_chain_from_fields(&h),
            Err("Missing auth chain element at index 0".to_string())
        );
    }

    #[test]
    fn thumbnail_relative_paths_are_accepted() {
        assert!(is_relative_thumbnail_path("thumb.png"));
        assert!(is_relative_thumbnail_path("images/thumbnail.png"));
        assert!(is_relative_thumbnail_path("dir/na:me.png"));
    }

    #[test]
    fn thumbnail_non_relative_paths_are_rejected() {
        for value in [
            "https://example.com/image.png",
            "https://example.com/x\"><script>alert(1)</script><meta name=\"y",
            "//evil.example/x.png",
            "/thumb.png",
            "data:text/html,<b>x</b>",
            "javascript:alert(1)",
            "HtTpS://evil.example/x.png",
            " thumb.png",
            "thumb.png ",
            "thumb\nname.png",
            "thumb\".png",
            "thumb<img>.png",
            "\u{FEFF}thumb.png",
            "thumb.png\u{FEFF}",
        ] {
            assert!(
                !is_relative_thumbnail_path(value),
                "expected rejection: {value:?}"
            );
        }
    }

    #[test]
    fn thumbnail_validation_pushes_upstream_error_strings() {
        let mut errors = Vec::new();
        let entity = json!({
            "content": [{ "file": "https://example.com/image.png", "hash": "bafyx" }],
            "metadata": { "display": { "navmapThumbnail": "https://example.com/image.png" } }
        });
        validate_navmap_thumbnail(&entity, &mut errors);
        assert_eq!(
            errors,
            vec!["Scene thumbnail 'https://example.com/image.png' must be a relative path to a file included in the deployment.".to_string()]
        );

        let mut errors = Vec::new();
        let entity = json!({
            "content": [{ "file": "other.png", "hash": "bafyx" }],
            "metadata": { "display": { "navmapThumbnail": "thumb.png" } }
        });
        validate_navmap_thumbnail(&entity, &mut errors);
        assert_eq!(
            errors,
            vec![
                "Scene thumbnail 'thumb.png' must be a file included in the deployment."
                    .to_string()
            ]
        );

        let mut errors = Vec::new();
        let entity = json!({
            "content": [{ "file": "images/thumbnail.png", "hash": "bafyx" }],
            "metadata": { "display": { "navmapThumbnail": "images/thumbnail.png" } }
        });
        validate_navmap_thumbnail(&entity, &mut errors);
        assert!(errors.is_empty());

        let mut errors = Vec::new();
        validate_navmap_thumbnail(&json!({ "metadata": {} }), &mut errors);
        validate_navmap_thumbnail(
            &json!({ "metadata": { "display": { "navmapThumbnail": null } } }),
            &mut errors,
        );
        validate_navmap_thumbnail(
            &json!({ "metadata": { "display": { "navmapThumbnail": "" } } }),
            &mut errors,
        );
        assert!(errors.is_empty());
    }

    #[test]
    fn thumbnail_bom_prefixed_path_is_rejected_even_when_content_matches() {
        let mut errors = Vec::new();
        let entity = json!({
            "content": [{ "file": "\u{FEFF}thumb.png", "hash": "bafyx" }],
            "metadata": { "display": { "navmapThumbnail": "\u{FEFF}thumb.png" } }
        });
        validate_navmap_thumbnail(&entity, &mut errors);
        assert_eq!(
            errors,
            vec!["Scene thumbnail '\u{FEFF}thumb.png' must be a relative path to a file included in the deployment.".to_string()]
        );
    }

    #[test]
    fn thumbnail_non_string_values_fail_validation() {
        let mut errors = Vec::new();
        let entity = json!({
            "content": [{ "file": "thumb.png", "hash": "bafyx" }],
            "metadata": { "display": { "navmapThumbnail": ["https://x\"><script>alert(1)</script><meta name=\"y"] } }
        });
        validate_navmap_thumbnail(&entity, &mut errors);
        assert_eq!(
            errors,
            vec![
                "Scene thumbnail '[\"https://x\\\"><script>alert(1)</script><meta name=\\\"y\"]' must be a relative path to a file included in the deployment."
                    .to_string()
            ]
        );

        for (value, rendered) in [
            (json!(5), "5"),
            (json!({"a": 1}), "{\"a\":1}"),
            (json!(true), "true"),
        ] {
            let mut errors = Vec::new();
            validate_navmap_thumbnail(
                &json!({ "metadata": { "display": { "navmapThumbnail": value } } }),
                &mut errors,
            );
            assert_eq!(
                errors,
                vec![format!(
                    "Scene thumbnail '{rendered}' must be a relative path to a file included in the deployment."
                )]
            );
        }

        let mut errors = Vec::new();
        for falsy in [json!(false), json!(0), json!(0.0)] {
            validate_navmap_thumbnail(
                &json!({ "metadata": { "display": { "navmapThumbnail": falsy } } }),
                &mut errors,
            );
        }
        assert!(errors.is_empty(), "falsy values must skip: {errors:?}");
    }

    #[test]
    fn entity_file_cap_matches_upstream_constant_and_message() {
        assert_eq!(MAX_ENTITY_FILE_SIZE_BYTES, 5 * 1024 * 1024);
        assert_eq!(
            entity_file_too_large_error(),
            "The entity file is too large. The maximum allowed size is 5242880 bytes."
        );
    }
}

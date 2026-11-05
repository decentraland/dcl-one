use serde_json::{Value, json};

use super::errors::AppError;

pub const MAX_IDS_OR_POINTERS: usize = 1000;

/// The lamb2 sentence for POST /entities/active; the multi-clause text is part
/// of the contract.
pub const ACTIVE_ENTITIES_SELECTOR_MESSAGE: &str = "ids or pointers must be present, but not both. \
    They must be arrays and contain at least one element. \
    None of the elements can be empty.";

/// GET /entities/{type} answers with the short form.
pub const ENTITIES_SELECTOR_MESSAGE: &str = "ids or pointers must be present, but not both";

#[derive(Debug, PartialEq, Eq)]
pub enum SelectorError {
    Malformed,
    TooMany(usize),
}

impl SelectorError {
    pub fn into_app_error(self, malformed_message: &str) -> AppError {
        match self {
            SelectorError::Malformed => AppError::invalid(malformed_message),
            SelectorError::TooMany(max) => AppError::invalid(format!(
                "Too many ids or pointers; the maximum allowed is {max}"
            )),
        }
    }
}

/// Exactly one of `ids`/`pointers`, non-empty, every element non-empty and
/// NUL-free; returns whether ids were selected.
pub fn validate_ids_or_pointers(
    ids: Option<&[String]>,
    pointers: Option<&[String]>,
    max: usize,
) -> Result<bool, SelectorError> {
    let is_valid = |s: &String| !s.is_empty() && !s.contains('\0');
    let (len, use_ids) = match (ids, pointers) {
        (Some(ids), None) if !ids.is_empty() && ids.iter().all(is_valid) => (ids.len(), true),
        (None, Some(pointers)) if !pointers.is_empty() && pointers.iter().all(is_valid) => {
            (pointers.len(), false)
        }
        _ => return Err(SelectorError::Malformed),
    };

    if len > max {
        return Err(SelectorError::TooMany(max));
    }

    Ok(use_ids)
}

/// Why a POST /entities/active body was refused: upstream's
/// `@dcl/schema-validator-component` 400 body, or (for a NUL byte in an
/// element, which upstream passes to PostgreSQL) the selector error.
#[derive(Debug)]
pub enum BodyRefusal {
    Schema(Value),
    Selector(AppError),
}

/// Parses a POST /entities/active body whatever its `Content-Type`
/// (upstream registers the validator with `ensureJsonContentType: false`) and
/// checks it against upstream's `activeEntitiesBodySchema`; returns whether
/// ids were selected, and the values.
///
/// A body that is not JSON answers `{ok: false, message}` with the parse
/// error; one that fails the schema answers `{ok: false, message: "Invalid
/// JSON body", data}` with the errors AJV 8 (`allErrors: false`) reports for
/// the `oneOf`: the first error of each branch, then the `oneOf` error.
pub fn active_entities_body(bytes: &[u8]) -> Result<(bool, Vec<String>), BodyRefusal> {
    let body: Value = serde_json::from_slice(bytes).map_err(|e| {
        // V8's message for an empty body; any other parse error carries
        // serde's wording.
        let message = if bytes.iter().all(u8::is_ascii_whitespace) {
            "Unexpected end of JSON input".to_string()
        } else {
            e.to_string()
        };
        BodyRefusal::Schema(json!({ "ok": false, "message": message }))
    })?;
    // JSON.parse keeps keys in body order; AJV names the first extra one.
    let keys = match body {
        Value::Object(_) => serde_json::from_slice::<OrderedKeys>(bytes)
            .map(|k| k.0)
            .unwrap_or_default(),
        _ => Vec::new(),
    };
    let mut errors = Vec::new();
    for (branch, key) in ["ids", "pointers"].into_iter().enumerate() {
        match selector_branch_error(&body, &keys, branch, key) {
            Some(error) => errors.push(error),
            None => {
                let values: Vec<String> = body[key]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(|v| v.as_str().map(str::to_string))
                    .collect();
                if values.iter().any(|v| v.contains('\0')) {
                    return Err(BodyRefusal::Selector(AppError::invalid(
                        ACTIVE_ENTITIES_SELECTOR_MESSAGE,
                    )));
                }
                return Ok((branch == 0, values));
            }
        }
    }
    errors.push(json!({
        "instancePath": "",
        "schemaPath": "#/oneOf",
        "keyword": "oneOf",
        "params": { "passingSchemas": null },
        "message": "must match exactly one schema in oneOf",
    }));
    Err(BodyRefusal::Schema(json!({
        "ok": false,
        "message": "Invalid JSON body",
        "data": errors,
    })))
}

/// The first AJV error of `oneOf` branch `branch` (`{key: string[1..=1000]}`,
/// `required`, `additionalProperties: false`), in AJV's keyword order: type,
/// required, additionalProperties, then the property's type, item count and
/// items.
fn selector_branch_error(body: &Value, keys: &[String], branch: usize, key: &str) -> Option<Value> {
    let error = |instance: String, schema: &str, keyword: &str, params: Value, message: String| {
        Some(json!({
            "instancePath": instance,
            "schemaPath": format!("#/oneOf/{branch}/{schema}"),
            "keyword": keyword,
            "params": params,
            "message": message,
        }))
    };
    let Some(object) = body.as_object() else {
        return error(
            String::new(),
            "type",
            "type",
            json!({ "type": "object" }),
            "must be object".into(),
        );
    };
    let Some(value) = object.get(key) else {
        return error(
            String::new(),
            "required",
            "required",
            json!({ "missingProperty": key }),
            format!("must have required property '{key}'"),
        );
    };
    if let Some(extra) = keys.iter().find(|k| k.as_str() != key) {
        return error(
            String::new(),
            "additionalProperties",
            "additionalProperties",
            json!({ "additionalProperty": extra }),
            "must NOT have additional properties".into(),
        );
    }
    let property = format!("properties/{key}");
    let Some(items) = value.as_array() else {
        return error(
            format!("/{key}"),
            &format!("{property}/type"),
            "type",
            json!({ "type": "array" }),
            "must be array".into(),
        );
    };
    if items.len() > MAX_IDS_OR_POINTERS {
        return error(
            format!("/{key}"),
            &format!("{property}/maxItems"),
            "maxItems",
            json!({ "limit": MAX_IDS_OR_POINTERS }),
            format!("must NOT have more than {MAX_IDS_OR_POINTERS} items"),
        );
    }
    if items.is_empty() {
        return error(
            format!("/{key}"),
            &format!("{property}/minItems"),
            "minItems",
            json!({ "limit": 1 }),
            "must NOT have fewer than 1 items".into(),
        );
    }
    items
        .iter()
        .enumerate()
        .find_map(|(i, item)| match item.as_str() {
            None => error(
                format!("/{key}/{i}"),
                &format!("{property}/items/type"),
                "type",
                json!({ "type": "string" }),
                "must be string".into(),
            ),
            Some("") => error(
                format!("/{key}/{i}"),
                &format!("{property}/items/minLength"),
                "minLength",
                json!({ "limit": 1 }),
                "must NOT have fewer than 1 characters".into(),
            ),
            Some(_) => None,
        })
}

/// A JSON object's keys in body order, each once (where JSON.parse keeps a
/// repeated key).
#[derive(Default)]
struct OrderedKeys(Vec<String>);

impl<'de> serde::Deserialize<'de> for OrderedKeys {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct Keys;
        impl<'de> serde::de::Visitor<'de> for Keys {
            type Value = OrderedKeys;
            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("a JSON object")
            }
            fn visit_map<A: serde::de::MapAccess<'de>>(
                self,
                mut map: A,
            ) -> Result<OrderedKeys, A::Error> {
                // The set keeps a body of many distinct keys linear.
                let mut keys: Vec<String> = Vec::new();
                let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
                while let Some((key, _)) = map.next_entry::<String, serde::de::IgnoredAny>()? {
                    if seen.insert(key.clone()) {
                        keys.push(key);
                    }
                }
                Ok(OrderedKeys(keys))
            }
        }
        deserializer.deserialize_map(Keys)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn strings(items: &[&str]) -> Vec<String> {
        items.iter().map(|s| s.to_string()).collect()
    }

    /// About 900k distinct keys, a 10 MiB body: the key dedup stays linear
    /// and AJV's first extra key is still named.
    #[test]
    fn many_distinct_keys_are_read_in_linear_time() {
        let mut body = String::from("{");
        for i in 0..900_000 {
            body.push_str(&format!("\"a{i:07}\":0,"));
        }
        body.push_str("\"ids\":[\"x\"]}");
        let started = std::time::Instant::now();
        let Err(BodyRefusal::Schema(answer)) = active_entities_body(body.as_bytes()) else {
            panic!("extra keys fail the schema");
        };
        assert!(started.elapsed() < std::time::Duration::from_secs(20));
        assert_eq!(
            answer["data"][0]["params"]["additionalProperty"], "a0000000",
            "{answer}"
        );
    }

    fn validate(
        ids: Option<&[String]>,
        pointers: Option<&[String]>,
    ) -> Result<bool, SelectorError> {
        validate_ids_or_pointers(ids, pointers, MAX_IDS_OR_POINTERS)
    }

    #[test]
    fn accepts_ids_only() {
        let ids = strings(&["a", "b"]);
        assert!(validate(Some(&ids), None).unwrap());
    }

    #[test]
    fn accepts_pointers_only() {
        let ptrs = strings(&["0,0"]);
        assert!(!validate(None, Some(&ptrs)).unwrap());
    }

    #[test]
    fn rejects_both_present() {
        let ids = strings(&["a"]);
        let ptrs = strings(&["0,0"]);
        assert_eq!(
            validate(Some(&ids), Some(&ptrs)),
            Err(SelectorError::Malformed)
        );
    }

    #[test]
    fn rejects_neither_present() {
        assert_eq!(validate(None, None), Err(SelectorError::Malformed));
    }

    #[test]
    fn rejects_empty_ids_array() {
        let empty: Vec<String> = Vec::new();
        assert_eq!(validate(Some(&empty), None), Err(SelectorError::Malformed));
    }

    #[test]
    fn rejects_empty_string_element_in_ids() {
        let ids = strings(&["a", "", "c"]);
        assert_eq!(validate(Some(&ids), None), Err(SelectorError::Malformed));
    }

    #[test]
    fn rejects_empty_string_element_in_pointers() {
        let ptrs = strings(&[""]);
        assert_eq!(validate(None, Some(&ptrs)), Err(SelectorError::Malformed));
    }

    #[test]
    fn rejects_nul_byte_element_in_ids() {
        let ids = strings(&["a", "\0", "c"]);
        assert_eq!(validate(Some(&ids), None), Err(SelectorError::Malformed));
    }

    #[test]
    fn rejects_nul_byte_element_in_pointers() {
        let ptrs = strings(&["0,0\0"]);
        assert_eq!(validate(None, Some(&ptrs)), Err(SelectorError::Malformed));
    }

    #[test]
    fn accepts_exactly_1000_ids() {
        let ids: Vec<String> = (0..MAX_IDS_OR_POINTERS).map(|i| i.to_string()).collect();
        assert_eq!(ids.len(), 1000);
        assert!(validate(Some(&ids), None).unwrap());
    }

    #[test]
    fn rejects_over_1000_ids() {
        let ids: Vec<String> = (0..=MAX_IDS_OR_POINTERS).map(|i| i.to_string()).collect();
        assert_eq!(ids.len(), 1001);
        let err = validate(Some(&ids), None).unwrap_err();
        assert_eq!(err, SelectorError::TooMany(1000));
        assert_eq!(
            err.into_app_error("x").to_string(),
            "Too many ids or pointers; the maximum allowed is 1000"
        );
    }

    #[test]
    fn rejects_over_1000_pointers() {
        let ptrs: Vec<String> = (0..=MAX_IDS_OR_POINTERS).map(|i| i.to_string()).collect();
        assert_eq!(
            validate(None, Some(&ptrs)),
            Err(SelectorError::TooMany(1000))
        );
    }

    #[test]
    fn malformed_selector_carries_the_endpoint_message() {
        let err = SelectorError::Malformed.into_app_error(ACTIVE_ENTITIES_SELECTOR_MESSAGE);
        assert_eq!(err.to_string(), ACTIVE_ENTITIES_SELECTOR_MESSAGE);
        assert!(ACTIVE_ENTITIES_SELECTOR_MESSAGE.starts_with(ENTITIES_SELECTOR_MESSAGE));
    }

    fn schema_refusal(raw: &str) -> Value {
        match active_entities_body(raw.as_bytes()) {
            Err(BodyRefusal::Schema(body)) => body,
            other => panic!("not a schema refusal for {raw}: {other:?}"),
        }
    }

    fn one_of(first: Value, second: Value) -> Value {
        json!({ "ok": false, "message": "Invalid JSON body", "data": [first, second, {
            "instancePath": "", "schemaPath": "#/oneOf", "keyword": "oneOf",
            "params": { "passingSchemas": null },
            "message": "must match exactly one schema in oneOf" }] })
    }

    fn missing(branch: usize, key: &str) -> Value {
        json!({ "instancePath": "", "schemaPath": format!("#/oneOf/{branch}/required"),
            "keyword": "required", "params": { "missingProperty": key },
            "message": format!("must have required property '{key}'") })
    }

    fn extra(branch: usize, key: &str) -> Value {
        json!({ "instancePath": "", "schemaPath": format!("#/oneOf/{branch}/additionalProperties"),
            "keyword": "additionalProperties", "params": { "additionalProperty": key },
            "message": "must NOT have additional properties" })
    }

    /// Upstream `routes.ts` registers the validator with
    /// `ensureJsonContentType: false`: the body is the only input.
    #[test]
    fn active_entities_body_selects_ids_or_pointers() {
        let (use_ids, values) = active_entities_body(br#"{"ids":["a","b"]}"#).unwrap();
        assert!(use_ids);
        assert_eq!(values, strings(&["a", "b"]));
        let (use_ids, values) = active_entities_body(br#"{"pointers":["0,0"]}"#).unwrap();
        assert!(!use_ids);
        assert_eq!(values, strings(&["0,0"]));
        let ids: Vec<String> = (0..MAX_IDS_OR_POINTERS).map(|i| i.to_string()).collect();
        let body = json!({ "ids": ids }).to_string();
        assert_eq!(active_entities_body(body.as_bytes()).unwrap().1.len(), 1000);
    }

    #[test]
    fn active_entities_body_that_is_not_json_answers_the_parse_error() {
        assert_eq!(
            schema_refusal(""),
            json!({ "ok": false, "message": "Unexpected end of JSON input" })
        );
        let body = schema_refusal("ids=a");
        assert_eq!(body["ok"], false);
        assert!(body.get("data").is_none(), "{body}");
    }

    #[test]
    fn active_entities_body_failing_the_schema_answers_as_the_schema_validator() {
        let object = |branch: usize| {
            json!({ "instancePath": "", "schemaPath": format!("#/oneOf/{branch}/type"),
                "keyword": "type", "params": { "type": "object" }, "message": "must be object" })
        };
        let property = |branch: usize,
                        key: &str,
                        path: &str,
                        keyword: &str,
                        params: Value,
                        message: &str| {
            json!({ "instancePath": path, "schemaPath": format!("#/oneOf/{branch}/properties/{key}/{keyword}"),
                "keyword": keyword.rsplit('/').next().unwrap(), "params": params, "message": message })
        };
        let over: Vec<String> = (0..=MAX_IDS_OR_POINTERS).map(|i| i.to_string()).collect();
        let over = json!({ "ids": over }).to_string();
        let cases = [
            ("[]", one_of(object(0), object(1))),
            ("null", one_of(object(0), object(1))),
            ("{}", one_of(missing(0, "ids"), missing(1, "pointers"))),
            (
                r#"{"pointers":["0,0"],"ids":["a"]}"#,
                one_of(extra(0, "pointers"), extra(1, "ids")),
            ),
            (
                r#"{"ids":["a"],"pointers":null}"#,
                one_of(extra(0, "pointers"), extra(1, "ids")),
            ),
            (
                r#"{"ids":["a"],"fields":"x"}"#,
                one_of(extra(0, "fields"), missing(1, "pointers")),
            ),
            (
                r#"{"pointers":[]}"#,
                one_of(
                    missing(0, "ids"),
                    property(
                        1,
                        "pointers",
                        "/pointers",
                        "minItems",
                        json!({ "limit": 1 }),
                        "must NOT have fewer than 1 items",
                    ),
                ),
            ),
            (
                r#"{"ids":"a"}"#,
                one_of(
                    property(
                        0,
                        "ids",
                        "/ids",
                        "type",
                        json!({ "type": "array" }),
                        "must be array",
                    ),
                    missing(1, "pointers"),
                ),
            ),
            (
                r#"{"ids":["a",""]}"#,
                one_of(
                    property(
                        0,
                        "ids",
                        "/ids/1",
                        "items/minLength",
                        json!({ "limit": 1 }),
                        "must NOT have fewer than 1 characters",
                    ),
                    missing(1, "pointers"),
                ),
            ),
            (
                r#"{"ids":[1]}"#,
                one_of(
                    property(
                        0,
                        "ids",
                        "/ids/0",
                        "items/type",
                        json!({ "type": "string" }),
                        "must be string",
                    ),
                    missing(1, "pointers"),
                ),
            ),
            (
                over.as_str(),
                one_of(
                    property(
                        0,
                        "ids",
                        "/ids",
                        "maxItems",
                        json!({ "limit": 1000 }),
                        "must NOT have more than 1000 items",
                    ),
                    missing(1, "pointers"),
                ),
            ),
        ];
        for (raw, expected) in cases {
            assert_eq!(
                schema_refusal(raw),
                expected,
                "{}",
                &raw[..raw.len().min(40)]
            );
        }
    }

    #[test]
    fn active_entities_body_with_a_nul_element_is_the_selector_error() {
        match active_entities_body(br#"{"ids":["a\u0000"]}"#) {
            Err(BodyRefusal::Selector(e)) => {
                assert_eq!(e.to_string(), ACTIVE_ENTITIES_SELECTOR_MESSAGE)
            }
            other => panic!("{other:?}"),
        }
    }
}

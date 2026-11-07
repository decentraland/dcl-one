use serde_json::{Value, json};

use super::errors::AppError;

pub const MAX_IDS_OR_POINTERS: usize = 1000;

/// GET /entities/{type} answers with the short form.
pub const ENTITIES_SELECTOR_MESSAGE: &str = "ids or pointers must be present, but not both";

/// GET /entities/{type} with more than [`MAX_IDS_OR_POINTERS`] values.
pub fn too_many_ids_or_pointers() -> AppError {
    AppError::invalid(format!(
        "Too many ids or pointers; the maximum allowed is {MAX_IDS_OR_POINTERS}"
    ))
}

/// Why a POST /entities/active body was refused: upstream's
/// `@dcl/schema-validator-component` 400 body, or an element with a NUL
/// byte, which upstream passes to PostgreSQL and answers with its bare 500.
#[derive(Debug)]
pub enum BodyRefusal {
    Schema(Value),
    NulElement,
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
        BodyRefusal::Schema(json!({ "ok": false, "message": json_parse_message(bytes, &e) }))
    })?;
    let keys = object_keys_in_order(&body, bytes);
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
                // Upstream hands the NUL to PostgreSQL, which refuses it:
                // its 500 (`CR-2-nul-element.json`).
                if values.iter().any(|v| v.contains('\0')) {
                    return Err(BodyRefusal::NulElement);
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

/// The message `JSON.parse` throws for a body that is not JSON: V8's text
/// for the failures below, else serde's. The schema validator answers it as
/// `{ok: false, message}`.
pub fn json_parse_message(bytes: &[u8], error: &serde_json::Error) -> String {
    v8_parse_message(bytes).unwrap_or_else(|| error.to_string())
}

/// An object body's keys in body order, as `JSON.parse` keeps them (AJV names
/// the first extra one); empty for any other value.
pub fn object_keys_in_order(body: &Value, bytes: &[u8]) -> Vec<String> {
    match body {
        Value::Object(_) => serde_json::from_slice::<OrderedKeys>(bytes)
            .map(|k| k.0)
            .unwrap_or_default(),
        _ => Vec::new(),
    }
}

/// V8's `JSON.parse` message for the parse failures the production peer was
/// captured answering (`captures/content-reads/CR-2-empty-body.json`,
/// `CR-2-not-json.json`, `CR-2-invalid-token.json`); `None` for any other,
/// which keeps serde's wording.
fn v8_parse_message(bytes: &[u8]) -> Option<String> {
    let source = std::str::from_utf8(bytes).ok()?;
    let is_ws = |c: char| matches!(c, ' ' | '\t' | '\n' | '\r');
    let mut chars = source.char_indices().skip_while(|(_, c)| is_ws(*c));
    let Some((_, first)) = chars.next() else {
        return Some("Unexpected end of JSON input".into());
    };
    if first == '{' {
        // `{` not followed by a property name or `}`: V8 names the position
        // (UTF-16 units) of the offending character, or the end.
        let (at, next) = match chars.find(|(_, c)| !is_ws(*c)) {
            Some((at, c)) => (at, Some(c)),
            None => (source.len(), None),
        };
        if matches!(next, Some('"' | '}')) {
            return None;
        }
        let before = &source[..at];
        let position = before.encode_utf16().count();
        let line = before.matches('\n').count() + 1;
        let column = before
            .rsplit('\n')
            .next()
            .map_or(0, |l| l.encode_utf16().count())
            + 1;
        return Some(format!(
            "Expected property name or '}}' in JSON at position {position} (line {line} column {column})"
        ));
    }
    // A first character no JSON value starts with, in a source short enough
    // that V8 quotes it whole (it quotes an excerpt from 21 characters on).
    let starts_value = matches!(first, '[' | '"' | '-' | '0'..='9' | 't' | 'f' | 'n');
    if !starts_value && source.encode_utf16().count() < 21 {
        return Some(format!(
            "Unexpected token '{first}', \"{source}\" is not valid JSON"
        ));
    }
    None
}

/// Serializes a schema refusal with the key order upstream's validator
/// writes: `ok,message,data`, and each error's
/// `instancePath,schemaPath,keyword,params,message` (`CR-2-*.json`).
pub fn ordered_refusal_json(value: &Value) -> String {
    const ORDER: [&str; 7] = [
        "ok",
        "instancePath",
        "schemaPath",
        "keyword",
        "params",
        "message",
        "data",
    ];
    match value {
        Value::Object(map) => {
            let mut keys: Vec<&String> = map.keys().collect();
            keys.sort_by_key(|k| ORDER.iter().position(|o| o == k).unwrap_or(ORDER.len()));
            let fields: Vec<String> = keys
                .into_iter()
                .map(|k| {
                    format!(
                        "{}:{}",
                        Value::from(k.as_str()),
                        ordered_refusal_json(&map[k])
                    )
                })
                .collect();
            format!("{{{}}}", fields.join(","))
        }
        Value::Array(items) => {
            let items: Vec<String> = items.iter().map(ordered_refusal_json).collect();
            format!("[{}]", items.join(","))
        }
        other => other.to_string(),
    }
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

    #[test]
    fn too_many_carries_the_maximum() {
        assert_eq!(
            too_many_ids_or_pointers().to_string(),
            "Too many ids or pointers; the maximum allowed is 1000"
        );
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
    fn active_entities_body_with_a_nul_element_is_upstreams_500() {
        match active_entities_body(br#"{"ids":["a\u0000"]}"#) {
            Err(BodyRefusal::NulElement) => {}
            other => panic!("{other:?}"),
        }
    }

    /// CR-2: V8's messages for the captured parse failures.
    #[test]
    fn parse_failures_answer_v8s_captured_messages() {
        for (raw, message) in [
            ("", "Unexpected end of JSON input"),
            ("  ", "Unexpected end of JSON input"),
            (
                "{",
                "Expected property name or '}' in JSON at position 1 (line 1 column 2)",
            ),
            (
                "{\n x",
                "Expected property name or '}' in JSON at position 3 (line 2 column 2)",
            ),
            ("hello", "Unexpected token 'h', \"hello\" is not valid JSON"),
        ] {
            assert_eq!(schema_refusal(raw)["message"], message, "{raw:?}");
        }
    }

    /// CR-2: the refusal's keys in the captured order.
    #[test]
    fn refusals_keep_the_captured_key_order() {
        let body = schema_refusal(r#"{"pointers":[]}"#);
        let text = ordered_refusal_json(&body);
        assert!(
            text.starts_with(r##"{"ok":false,"message":"Invalid JSON body","data":[{"instancePath":"","schemaPath":"#/oneOf/0/required","keyword":"required","params":{"missingProperty":"ids"},"message":"must have required property 'ids'"}"##),
            "{text}"
        );
        assert_eq!(serde_json::from_str::<Value>(&text).unwrap(), body);
    }
}

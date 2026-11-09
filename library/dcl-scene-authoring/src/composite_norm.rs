use crate::jsjson::{self, JsValue, array_index, ordered_entries};
use crate::schema_crdt::component_number_from_name;
use anyhow::{Result, bail};
use base64::Engine;
use std::borrow::Cow;
use std::collections::{BTreeMap, HashSet};
use std::sync::OnceLock;

/// The composite namespace of the smart-item components `@dcl/smart-items`
/// defines (`smart-items::Actions`, `smart-items::Triggers`, ...).
pub const SMART_ITEMS_PREFIX: &str = "smart-items::";

/// The namespace composites carried while the smart-item runtime was the
/// registry package `@dcl/asset-packs`. Scenes saved by the Creator Hub before
/// the move still name their components this way, so every reader renames the
/// prefix on load; the runtime defines only [`SMART_ITEMS_PREFIX`] names, and
/// a component's number is hashed from its name.
pub const LEGACY_PREFIX: &str = "asset-packs::";

/// A composite component name as the runtime defines it: a [`LEGACY_PREFIX`]
/// name moved under [`SMART_ITEMS_PREFIX`], every other name unchanged.
pub fn smart_item_name(name: &str) -> Cow<'_, str> {
    match name.strip_prefix(LEGACY_PREFIX) {
        Some(rest) => Cow::Owned(format!("{SMART_ITEMS_PREFIX}{rest}")),
        None => Cow::Borrowed(name),
    }
}

/// The smart-item components `@dcl/smart-items` defines, by bare name:
/// `smart-items::<Name>` in a composite the runtime wrote, `asset-packs::<Name>`
/// in one the registry `@dcl/inspector` wrote.
pub const SMART_ITEM_NAMES: [&str; 13] = [
    "ActionTypes",
    "Actions",
    "Counter",
    "Triggers",
    "States",
    "CounterBar",
    "AdminTools",
    "VideoScreen",
    "Rewards",
    "TextAnnouncements",
    "VideoControlState",
    "Script",
    "Placeholder",
];

/// The component whose `componentIds` value names other components by number.
pub const SYNC_COMPONENTS: &str = "core-schema::Sync-Components";

/// Each smart-item component's number under its [`LEGACY_PREFIX`] name and
/// under its [`SMART_ITEMS_PREFIX`] name, in [`SMART_ITEM_NAMES`] order. A
/// component's number is hashed from its name, so the rename moves every one.
pub fn legacy_component_numbers() -> &'static [(u32, u32); 13] {
    static TABLE: OnceLock<[(u32, u32); 13]> = OnceLock::new();
    TABLE.get_or_init(|| {
        SMART_ITEM_NAMES.map(|name| {
            (
                component_number_from_name(&format!("{LEGACY_PREFIX}{name}")),
                component_number_from_name(&format!("{SMART_ITEMS_PREFIX}{name}")),
            )
        })
    })
}

/// A component number as the runtime defines it: a smart-item component's
/// [`LEGACY_PREFIX`] number moved to its [`SMART_ITEMS_PREFIX`] number, every
/// other number unchanged.
pub fn current_component_number(id: u32) -> u32 {
    legacy_component_numbers()
        .iter()
        .find(|(legacy, _)| *legacy == id)
        .map_or(id, |(_, current)| *current)
}

fn as_u32(n: f64) -> Option<u32> {
    (n.fract() == 0.0 && (0.0..=f64::from(u32::MAX)).contains(&n)).then_some(n as u32)
}

fn get_mut<'a>(value: &'a mut JsValue, key: &str) -> Option<&'a mut JsValue> {
    match value {
        JsValue::Object(entries) => entries.iter_mut().find(|(k, _)| k == key).map(|(_, v)| v),
        _ => None,
    }
}

/// A [`SYNC_COMPONENTS`] value lists components by number: each legacy
/// smart-item number becomes the current one ([`current_component_number`]);
/// other numbers and strings pass through.
fn remap_sync_component_ids(data: &mut [(String, JsValue)]) {
    for (_, entry) in data.iter_mut() {
        let Some(JsValue::Array(ids)) =
            get_mut(entry, "json").and_then(|j| get_mut(j, "componentIds"))
        else {
            continue;
        };
        for id in ids.iter_mut() {
            if let JsValue::Number(n) = id
                && let Some(number) = as_u32(*n)
            {
                *n = f64::from(current_component_number(number));
            }
        }
    }
}

fn get_set<'a>(value: &'a JsValue, key: &str) -> Option<&'a JsValue> {
    value.get(key).filter(|v| !matches!(v, JsValue::Null))
}

fn truthy(value: &JsValue) -> bool {
    match value {
        JsValue::Null => false,
        JsValue::Bool(b) => *b,
        JsValue::Number(n) => *n != 0.0 && !n.is_nan(),
        JsValue::String(s) => !s.is_empty(),
        JsValue::Array(_) | JsValue::Object(_) => true,
    }
}

fn obj(entries: Vec<(&str, JsValue)>) -> JsValue {
    JsValue::Object(
        entries
            .into_iter()
            .map(|(k, v)| (k.to_owned(), v))
            .collect(),
    )
}

/// `JSON.stringify` with full JS number formatting (`NaN`, `1e+21`, ...).
pub fn write_json(value: &JsValue, out: &mut String) {
    jsjson::write_with(value, out, &|n| Ok(js_number_string(n))).expect("infallible");
}

fn js_number_string(x: f64) -> String {
    if x.is_nan() {
        return "NaN".into();
    }
    if x.is_infinite() {
        return if x > 0.0 { "Infinity" } else { "-Infinity" }.into();
    }
    if x == 0.0 {
        return "0".into();
    }
    let mag = x.abs();
    if (1e-6..1e21).contains(&mag) {
        return format!("{x}");
    }
    let exp = format!("{x:e}");
    match exp.find('e') {
        Some(p) if !exp[p + 1..].starts_with('-') => format!("{}e+{}", &exp[..p], &exp[p + 1..]),
        _ => exp,
    }
}

fn js_string(value: &JsValue) -> String {
    match value {
        JsValue::Null => "null".into(),
        JsValue::Bool(b) => b.to_string(),
        JsValue::Number(n) => js_number_string(*n),
        JsValue::String(s) => s.clone(),
        JsValue::Array(items) => items
            .iter()
            .map(|item| match item {
                JsValue::Null => String::new(),
                other => js_string(other),
            })
            .collect::<Vec<_>>()
            .join(","),
        JsValue::Object(_) => "[object Object]".into(),
    }
}

fn js_number(value: &JsValue) -> f64 {
    match value {
        JsValue::Null => 0.0,
        JsValue::Bool(b) => f64::from(u8::from(*b)),
        JsValue::Number(n) => *n,
        JsValue::String(s) => js_parse_number(s),
        JsValue::Array(_) => js_parse_number(&js_string(value)),
        JsValue::Object(_) => f64::NAN,
    }
}

fn js_parse_number(s: &str) -> f64 {
    let t = s.trim_matches(|c: char| c.is_whitespace() || c == '\u{feff}');
    if t.is_empty() {
        return 0.0;
    }
    match t {
        "Infinity" | "+Infinity" => return f64::INFINITY,
        "-Infinity" => return f64::NEG_INFINITY,
        _ => {}
    }
    for (prefix, radix) in [("0x", 16), ("0o", 8), ("0b", 2)] {
        if let Some(rest) = t
            .strip_prefix(prefix)
            .or_else(|| t.strip_prefix(prefix.to_ascii_uppercase().as_str()))
        {
            return js_parse_radix(rest, radix);
        }
    }
    if is_decimal_literal(t) {
        t.parse::<f64>().unwrap_or(f64::NAN)
    } else {
        f64::NAN
    }
}

fn js_parse_radix(digits: &str, radix: u32) -> f64 {
    if digits.is_empty() {
        return f64::NAN;
    }
    let mut acc = 0.0_f64;
    for c in digits.chars() {
        match c.to_digit(radix) {
            Some(d) => acc = acc * f64::from(radix) + f64::from(d),
            None => return f64::NAN,
        }
    }
    acc
}

fn is_decimal_literal(t: &str) -> bool {
    let b = t.as_bytes();
    let mut i = 0;
    if matches!(b.first(), Some(b'+') | Some(b'-')) {
        i = 1;
    }
    let int_start = i;
    while i < b.len() && b[i].is_ascii_digit() {
        i += 1;
    }
    let int_len = i - int_start;
    let mut frac_len = 0;
    if i < b.len() && b[i] == b'.' {
        i += 1;
        let frac_start = i;
        while i < b.len() && b[i].is_ascii_digit() {
            i += 1;
        }
        frac_len = i - frac_start;
    }
    if int_len == 0 && frac_len == 0 {
        return false;
    }
    if i < b.len() && (b[i] == b'e' || b[i] == b'E') {
        i += 1;
        if i < b.len() && (b[i] == b'+' || b[i] == b'-') {
            i += 1;
        }
        let exp_start = i;
        while i < b.len() && b[i].is_ascii_digit() {
            i += 1;
        }
        if i == exp_start {
            return false;
        }
    }
    i == b.len()
}

fn js_math_round(x: f64) -> f64 {
    if !x.is_finite() {
        return x;
    }
    let f = x.floor();
    if x - f >= 0.5 { f + 1.0 } else { f }
}

/// Re-encode as `atob`/`btoa` would: url-safe alphabet accepted, whitespace
/// skipped, decoding stops at the first foreign character.
fn canonical_base64(input: &str) -> String {
    let mut sextets: Vec<u8> = Vec::with_capacity(input.len());
    for c in input.chars() {
        let v = match c {
            'A'..='Z' => c as u8 - b'A',
            'a'..='z' => c as u8 - b'a' + 26,
            '0'..='9' => c as u8 - b'0' + 52,
            '+' | '-' => 62,
            '/' | '_' => 63,
            c if c.is_ascii_whitespace() => continue,
            _ => break,
        };
        sextets.push(v);
    }
    let mut bytes = Vec::with_capacity(sextets.len() * 3 / 4 + 2);
    for chunk in sextets.chunks(4) {
        if chunk.len() >= 2 {
            bytes.push((chunk[0] << 2) | (chunk[1] >> 4));
        }
        if chunk.len() >= 3 {
            bytes.push((chunk[1] << 4) | (chunk[2] >> 2));
        }
        if chunk.len() == 4 {
            bytes.push((chunk[2] << 6) | chunk[3]);
        }
    }
    base64::engine::general_purpose::STANDARD.encode(bytes)
}

fn normalize_data_entry(value: &JsValue) -> Result<JsValue> {
    if matches!(value, JsValue::Null) {
        bail!("data entry is null");
    }
    if let Some(json) = get_set(value, "json") {
        return Ok(obj(vec![("json", json.clone())]));
    }
    if let Some(binary) = get_set(value, "binary") {
        let JsValue::String(b64) = binary else {
            bail!("data entry binary is not a base64 string");
        };
        return Ok(obj(vec![(
            "binary",
            JsValue::String(canonical_base64(b64)),
        )]));
    }
    Ok(obj(Vec::new()))
}

/// Entity keys are re-spelled through `Number` → `String`, so `"512"`, `"5.12e2"`
/// and `" 512 "` all land on the index 512; non-index survivors keep insertion order.
fn normalize_data(pairs: &[(String, &JsValue)]) -> Result<Vec<(String, JsValue)>> {
    let mut indexed: BTreeMap<u32, JsValue> = BTreeMap::new();
    let mut plain: Vec<(String, JsValue)> = Vec::new();
    for (key, value) in ordered_entries(pairs) {
        let entry = normalize_data_entry(value)?;
        let out_key = js_number_string(js_parse_number(key));
        match array_index(&out_key) {
            Some(n) => {
                indexed.insert(n, entry);
            }
            None => jsjson::set(&mut plain, out_key, entry),
        }
    }
    let mut entries: Vec<(String, JsValue)> = indexed
        .into_iter()
        .map(|(n, e)| (n.to_string(), e))
        .collect();
    entries.extend(plain);
    Ok(entries)
}

/// One normalized component block.
struct Block {
    name: String,
    schema: Option<JsValue>,
    data: Vec<(String, JsValue)>,
    /// The file spelled the name `smart-items::` itself (not renamed here).
    current: bool,
    /// `schema` came from a block the file spelled `smart-items::`.
    schema_current: bool,
    /// Entities whose entry came from a block the file spelled `smart-items::`.
    current_entities: HashSet<String>,
}

impl Block {
    /// Fold a later block of the same name in: per entity the block the file
    /// already spelled `smart-items::` wins, and so does its `jsonSchema`
    /// (else the first block that has one).
    fn merge(&mut self, other: Block) {
        if other.schema.is_some()
            && (self.schema.is_none() || (other.current && !self.schema_current))
        {
            self.schema = other.schema;
            self.schema_current = other.current;
        }
        for (key, value) in other.data {
            match self.data.iter_mut().find(|(k, _)| *k == key) {
                Some(slot) if other.current && !self.current_entities.contains(&key) => {
                    slot.1 = value;
                }
                Some(_) => {}
                None => self.data.push((key.clone(), value)),
            }
            if other.current {
                self.current_entities.insert(key);
            }
        }
        self.data = ordered_entries(&self.data).into_iter().cloned().collect();
    }

    fn into_value(self) -> JsValue {
        let mut entries = vec![("name", JsValue::String(self.name))];
        if let Some(schema) = self.schema {
            entries.push(("jsonSchema", schema));
        }
        entries.push(("data", JsValue::Object(self.data)));
        obj(entries)
    }
}

fn normalize_component(component: &JsValue) -> Result<Block> {
    if matches!(component, JsValue::Null) {
        bail!("component is null");
    }
    let name = get_set(component, "name").map_or_else(String::new, js_string);
    let current = name.starts_with(SMART_ITEMS_PREFIX);
    let name = smart_item_name(&name).into_owned();
    let pairs: Vec<(String, &JsValue)> = match component.get("data") {
        Some(JsValue::Object(entries)) => entries.iter().map(|(k, v)| (k.clone(), v)).collect(),
        Some(JsValue::Array(items)) => items
            .iter()
            .enumerate()
            .map(|(i, v)| (i.to_string(), v))
            .collect(),
        _ => Vec::new(),
    };
    let schema = get_set(component, "jsonSchema").cloned();
    let mut data = normalize_data(&pairs)?;
    if name == SYNC_COMPONENTS {
        remap_sync_component_ids(&mut data);
    }
    let current_entities = match current {
        true => data.iter().map(|(k, _)| k.clone()).collect(),
        false => HashSet::new(),
    };
    Ok(Block {
        schema_current: current && schema.is_some(),
        name,
        schema,
        data,
        current,
        current_entities,
    })
}

/// Blocks that end with the same `smart-items::` name after the rename
/// become one block where the first stood ([`Block::merge`]).
fn normalize_definition(root: &JsValue) -> Result<JsValue> {
    if matches!(root, JsValue::Null) {
        bail!("composite root is null");
    }
    let version = js_math_round(get_set(root, "version").map_or(0.0, js_number));
    let version_value = if version.is_finite() {
        JsValue::Number(version)
    } else {
        JsValue::Null
    };
    let mut blocks: Vec<Block> = Vec::new();
    if let Some(JsValue::Array(items)) = root.get("components") {
        for item in items {
            let block = normalize_component(item)?;
            if block.name.starts_with(SMART_ITEMS_PREFIX)
                && let Some(first) = blocks.iter_mut().find(|b| b.name == block.name)
            {
                first.merge(block);
                continue;
            }
            blocks.push(block);
        }
    }
    let components = blocks.into_iter().map(Block::into_value).collect();
    Ok(obj(vec![
        ("version", version_value),
        ("components", JsValue::Array(components)),
    ]))
}

fn static_core_table() -> &'static HashSet<String> {
    static TABLE: OnceLock<HashSet<String>> = OnceLock::new();
    TABLE.get_or_init(|| {
        let raw = include_str!("../docs/composite-component-schemas.json");
        let parsed: serde_json::Value = serde_json::from_str(raw).unwrap_or_default();
        parsed["components"]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter(|c| c["inStaticTable"].as_bool() == Some(true))
                    .filter_map(|c| c["name"].as_str().map(str::to_owned))
                    .collect()
            })
            .unwrap_or_default()
    })
}

/// Normalizes composites the way `@dcl/ecs` `Composite.toJson` does, tracking
/// which component names earlier composites have already defined. Smart-item
/// components come out under [`SMART_ITEMS_PREFIX`] whichever prefix the file
/// carries ([`smart_item_name`]), [`SYNC_COMPONENTS`] values point at their
/// current numbers ([`current_component_number`]), and blocks that end with
/// the same smart-item name merge into one; the result
/// normalizes to itself.
pub struct CompositeNormalizer {
    defined: HashSet<String>,
}

impl Default for CompositeNormalizer {
    fn default() -> Self {
        Self::new()
    }
}

impl CompositeNormalizer {
    pub fn new() -> Self {
        let defined = [
            "core::Transform",
            "core-schema::Network-Entity",
            "core-schema::Network-Parent",
            "composite::root",
        ]
        .into_iter()
        .map(str::to_owned)
        .collect();
        Self { defined }
    }

    pub fn normalize(&mut self, raw: &str) -> Result<String> {
        let normalized = normalize_definition(&jsjson::parse(raw)?)?;
        self.check_instanceable(&normalized)?;
        let mut out = String::new();
        write_json(&normalized, &mut out);
        Ok(out)
    }

    fn check_instanceable(&mut self, normalized: &JsValue) -> Result<()> {
        let Some(JsValue::Array(components)) = normalized.get("components") else {
            return Ok(());
        };
        for component in components {
            let Some(JsValue::String(name)) = component.get("name") else {
                continue;
            };
            if self.defined.contains(name) {
                continue;
            }
            if name.starts_with("core::") {
                if static_core_table().contains(name) {
                    self.defined.insert(name.clone());
                    continue;
                }
                bail!("the core component {name} was not found");
            }
            if component.get("jsonSchema").is_some_and(truthy) {
                self.defined.insert(name.clone());
                continue;
            }
            bail!("{name} is not defined and there is no schema to define it");
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The table is regenerated from the blob's `@dcl/ecs`
    /// (`scripts/dump-composite-schema-table.mts`, `DCL_ECS_PATH` at the
    /// install tree) whenever the vendored line moves; the version pin here
    /// is what makes a forgotten regeneration fail.
    #[test]
    fn schema_table_tracks_the_vendored_ecs() {
        let raw = include_str!("../docs/composite-component-schemas.json");
        let parsed: serde_json::Value = serde_json::from_str(raw).unwrap();
        assert_eq!(
            parsed["ecsVersion"],
            serde_json::json!("7.29.1-34986384248.commit-bb45080")
        );
        let table = static_core_table();
        for name in [
            "core::ExplorerUiEventsResult",
            "core::TouchScreenControls",
            "core::UiInputBinding",
            "core::AvatarEmoteCommand",
            // new on the auth-server line
            "core::AvatarNametag",
        ] {
            assert!(table.contains(name), "missing {name}");
        }
        // CreatedBy is a manual component, registered but not a static-table
        // entry: composites never carry it
        assert!(!table.contains("core-schema::Created-By"));
    }

    fn edge_cases() -> Vec<(String, String)> {
        let raw = include_str!("../docs/composite-tojson-edge-cases.json");
        let JsValue::Array(cases) = jsjson::parse(raw).expect("edge cases parse") else {
            panic!("edge cases must be an array");
        };
        cases
            .iter()
            .map(|case| {
                let input = case.get("input").expect("input");
                let Some(JsValue::String(expected)) = case.get("output") else {
                    panic!("output must be a string");
                };
                let mut input_text = String::new();
                write_json(input, &mut input_text);
                (input_text, expected.clone())
            })
            .collect()
    }

    fn normalize_text(input: &str) -> String {
        let parsed = jsjson::parse(input).expect("parse");
        let normalized = normalize_definition(&parsed).expect("normalize");
        let mut out = String::new();
        write_json(&normalized, &mut out);
        out
    }

    #[test]
    fn edge_cases_match_upstream() {
        let cases = edge_cases();
        assert!(cases.len() >= 20);
        for (input, expected) in cases {
            assert_eq!(normalize_text(&input), expected, "input: {input}");
        }
    }

    #[test]
    fn normalization_is_idempotent() {
        for (_, expected) in edge_cases() {
            assert_eq!(normalize_text(&expected), expected);
        }
    }

    #[test]
    fn unknown_core_component_is_rejected() {
        let mut n = CompositeNormalizer::new();
        let raw = r#"{"version":1,"components":[{"name":"core::NotAThing","data":{}}]}"#;
        assert!(n.normalize(raw).is_err());
    }

    #[test]
    fn custom_component_needs_schema_until_defined() {
        let mut n = CompositeNormalizer::new();
        let no_schema = r#"{"version":1,"components":[{"name":"my::Thing","data":{}}]}"#;
        assert!(n.normalize(no_schema).is_err());
        let with_schema = r#"{"version":1,"components":[{"name":"my::Thing","jsonSchema":{"type":"object"},"data":{}}]}"#;
        assert!(n.normalize(with_schema).is_ok());
        assert!(n.normalize(no_schema).is_ok());
    }

    #[test]
    fn known_components_pass() {
        let mut n = CompositeNormalizer::new();
        let raw = r#"{"version":1,"components":[{"name":"core::MeshRenderer","data":{}},{"name":"core::Transform","data":{}},{"name":"composite::root","data":{}}]}"#;
        assert!(n.normalize(raw).is_ok());
    }

    #[test]
    fn null_entries_reject_the_composite() {
        let mut n = CompositeNormalizer::new();
        assert!(n.normalize("null").is_err());
        assert!(n.normalize(r#"{"components":[null]}"#).is_err());
        assert!(
            n.normalize(r#"{"components":[{"name":"core::Transform","data":{"512":null}}]}"#)
                .is_err()
        );
    }

    #[test]
    fn legacy_smart_item_names_normalize_to_the_runtime_namespace() {
        assert_eq!(
            smart_item_name("asset-packs::Actions"),
            "smart-items::Actions"
        );
        assert_eq!(
            smart_item_name("smart-items::Actions"),
            "smart-items::Actions"
        );
        assert_eq!(smart_item_name("core::Transform"), "core::Transform");
        let mut n = CompositeNormalizer::new();
        let raw = r#"{"version":1,"components":[{"name":"asset-packs::Actions","jsonSchema":{"type":"object"},"data":{"512":{"json":{"value":[]}}}}]}"#;
        let out = n.normalize(raw).unwrap();
        assert!(out.contains(r#""name":"smart-items::Actions""#), "{out}");
        assert!(
            !out.contains(&format!("\"name\":\"{LEGACY_PREFIX}")),
            "{out}"
        );
        // The first definition registered the renamed name, so a later composite
        // may use either spelling without carrying the schema again.
        let later = r#"{"version":1,"components":[{"name":"asset-packs::Actions","data":{}}]}"#;
        assert!(n.normalize(later).is_ok());
    }

    /// A Creator Hub capture from before the move: every component it inlines
    /// for the runtime has to come out under the namespace the runtime defines,
    /// and its `Sync-Components` under the numbers of those names. Other values
    /// are not touched — its `inspector::Config` data names the components its
    /// fields edit, and that stays as written.
    #[test]
    fn creator_hub_capture_normalizes_without_a_legacy_component_name_left() {
        let raw = include_str!("../testdata/gather-main.composite");
        assert!(raw.contains("\"asset-packs::Actions\""));
        assert!(raw.contains("4149214247"), "Script under its legacy number");
        let out = CompositeNormalizer::new().normalize(raw).unwrap();
        assert!(out.contains(r#""name":"smart-items::Actions""#));
        assert!(out.contains(r#""name":"smart-items::Script""#));
        assert!(!out.contains(&format!("\"name\":\"{LEGACY_PREFIX}")));
        assert!(out.contains(r#""type":"asset-packs::AdminTools""#));
        assert!(
            out.contains(r#""componentIds":[2884627400]"#) && out.contains(r#"[1043,1498732997]"#),
            "{out}"
        );
        assert!(
            !out.contains("99929642") && !out.contains("4149214247"),
            "{out}"
        );
        assert_eq!(CompositeNormalizer::new().normalize(&out).unwrap(), out);
    }

    /// The numbers the renumbering moves, as the component-number hash gives
    /// them (and as the review of the rename listed them).
    #[test]
    fn legacy_numbers_follow_the_component_number_hash() {
        let table = legacy_component_numbers();
        assert_eq!(table.len(), SMART_ITEM_NAMES.len());
        for (name, (legacy, current)) in SMART_ITEM_NAMES.iter().zip(table) {
            assert_eq!(
                *legacy,
                component_number_from_name(&format!("asset-packs::{name}"))
            );
            assert_eq!(
                *current,
                component_number_from_name(&format!("smart-items::{name}"))
            );
            assert_eq!(current_component_number(*legacy), *current, "{name}");
            assert_eq!(current_component_number(*current), *current, "{name}");
        }
        assert_eq!(current_component_number(2709697014), 261723156, "States");
        assert_eq!(current_component_number(2193198513), 748857427, "Actions");
        assert_eq!(current_component_number(1043), 1043);
    }

    const SYNC_SCHEMA: &str = r#"{"type":"object","properties":{"componentIds":{"type":"array","items":{"type":"integer","serializationType":"int64"},"serializationType":"array"}},"serializationType":"map"}"#;

    #[test]
    fn sync_components_point_at_the_runtime_numbers() {
        let raw = format!(
            r#"{{"version":1,"components":[{{"name":"core-schema::Sync-Components","jsonSchema":{SYNC_SCHEMA},"data":{{"512":{{"json":{{"componentIds":[2709697014,1043,"asset-packs::Actions",261723156]}}}},"513":{{"binary":"AQID"}}}}}}]}}"#
        );
        let out = CompositeNormalizer::new().normalize(&raw).unwrap();
        assert!(
            out.contains(r#""componentIds":[261723156,1043,"asset-packs::Actions",261723156]"#),
            "{out}"
        );
        assert_eq!(CompositeNormalizer::new().normalize(&out).unwrap(), out);
    }

    const STATES_SCHEMA: &str = r#"{"type":"object","properties":{"value":{"type":"string","serializationType":"utf8-string"}},"serializationType":"map"}"#;

    #[test]
    fn same_named_blocks_merge_into_the_first_and_the_current_spelling_wins() {
        let raw = format!(
            r#"{{"version":1,"components":[{{"name":"core::Transform","data":{{"512":{{"json":{{}}}}}}}},{{"name":"asset-packs::States","jsonSchema":{{"legacy":true}},"data":{{"513":{{"json":"legacy-513"}},"512":{{"json":"legacy-512"}}}}}},{{"name":"smart-items::States","jsonSchema":{STATES_SCHEMA},"data":{{"513":{{"json":"current-513"}},"514":{{"json":"current-514"}}}}}},{{"name":"asset-packs::States","data":{{"514":{{"json":"legacy-514"}},"515":{{"json":"legacy-515"}}}}}}]}}"#
        );
        let out = CompositeNormalizer::new().normalize(&raw).unwrap();
        let expected = format!(
            r#"{{"version":1,"components":[{{"name":"core::Transform","data":{{"512":{{"json":{{}}}}}}}},{{"name":"smart-items::States","jsonSchema":{STATES_SCHEMA},"data":{{"512":{{"json":"legacy-512"}},"513":{{"json":"current-513"}},"514":{{"json":"current-514"}},"515":{{"json":"legacy-515"}}}}}}]}}"#
        );
        assert_eq!(out, expected);
        assert_eq!(CompositeNormalizer::new().normalize(&out).unwrap(), out);
    }

    #[test]
    fn a_current_block_first_keeps_its_entries_and_schema() {
        let raw = format!(
            r#"{{"version":1,"components":[{{"name":"smart-items::Counter","jsonSchema":{STATES_SCHEMA},"data":{{"512":{{"json":"current"}}}}}},{{"name":"asset-packs::Counter","jsonSchema":{{"legacy":true}},"data":{{"512":{{"json":"legacy"}},"600":{{"json":"legacy-600"}}}}}}]}}"#
        );
        let out = CompositeNormalizer::new().normalize(&raw).unwrap();
        assert_eq!(
            out,
            format!(
                r#"{{"version":1,"components":[{{"name":"smart-items::Counter","jsonSchema":{STATES_SCHEMA},"data":{{"512":{{"json":"current"}},"600":{{"json":"legacy-600"}}}}}}]}}"#
            )
        );
    }

    /// Two legacy blocks: the first keeps its entries, the first schema wins,
    /// and only a block without a schema borrows one.
    #[test]
    fn legacy_blocks_merge_first_wins() {
        let raw = r#"{"version":1,"components":[{"name":"asset-packs::Counter","data":{"512":{"json":"first"}}},{"name":"asset-packs::Counter","jsonSchema":{"type":"object"},"data":{"512":{"json":"second"},"513":{"json":"second-513"}}}]}"#;
        let out = CompositeNormalizer::new().normalize(raw).unwrap();
        assert_eq!(
            out,
            r#"{"version":1,"components":[{"name":"smart-items::Counter","jsonSchema":{"type":"object"},"data":{"512":{"json":"first"},"513":{"json":"second-513"}}}]}"#
        );
    }

    #[test]
    fn other_components_never_merge() {
        let raw = r#"{"version":1,"components":[{"name":"my::Thing","jsonSchema":{"type":"object"},"data":{"512":{"json":1}}},{"name":"my::Thing","data":{"512":{"json":2}}}]}"#;
        let out = CompositeNormalizer::new().normalize(raw).unwrap();
        assert_eq!(out.matches(r#""name":"my::Thing""#).count(), 2, "{out}");
    }

    #[test]
    fn number_formatting_matches_js() {
        assert_eq!(js_number_string(0.0), "0");
        assert_eq!(js_number_string(-0.0), "0");
        assert_eq!(js_number_string(5.0), "5");
        assert_eq!(js_number_string(0.1), "0.1");
        assert_eq!(js_number_string(1e20), "100000000000000000000");
        assert_eq!(js_number_string(1e21), "1e+21");
        assert_eq!(js_number_string(1.5e-7), "1.5e-7");
        assert_eq!(js_number_string(f64::NAN), "NaN");
    }
}

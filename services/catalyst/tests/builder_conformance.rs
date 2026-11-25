//! The builder's parity ledger (`tests/fixtures/conformance/builder/parity.json`)
//! checked against the route ledger (`builder/src/tests/upstream_routes.rs`),
//! and its captures replayed against the builder routers as `serve` composes
//! them, with the social family's replay rules
//! (`social::test_fixtures::replay_signed`). Every capture is evidence of
//! exactly one operation and carries the replay notes it must produce: none
//! for a match, the recorded difference for a known gap. The replay needs
//! CATALYST_TEST_DATABASE_URL; one scratch database for the file.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use alloy::signers::{Signer, local::PrivateKeySigner};
use alloy_primitives::keccak256;
use axum::Router;
use axum::http::{HeaderMap, HeaderName, HeaderValue};
use catalyst_testing::conformance::diff::REDACTION_MARK;
use catalyst_testing::conformance::fixture::{Fixture, VOLATILE_SENTINEL};
use catalyst_testing::conformance::volatility::Volatility;
use serde::Deserialize;
use serde_json::Value;

use catalyst::auth::{AuthLink, AuthLinkType};
use catalyst::builder::curation::forum::fake::FakeForum;
use catalyst::builder::{Settings, State, root_router, router};
use catalyst::http::Outbound;
use catalyst::shared::signed_fetch::{
    AUTH_CHAIN_HEADER_PREFIX, AUTH_METADATA_HEADER, AUTH_TIMESTAMP_HEADER, build_payload_v6,
};
use catalyst::social::test_fixtures::replay_with;
use catalyst::storage::Storage;

#[path = "support/db.rs"]
mod db;

const FIXTURES: &str = "services/catalyst/tests/fixtures/conformance/builder";
const ROUTE_LEDGER: &str = "services/catalyst/builder/src/tests/upstream_routes.rs";
const GATEWAY_PREFIX: &str = "/builder-api";
const STATUSES: &[&str] = &["not_reviewed", "reviewed", "known_gap"];

#[derive(Deserialize)]
struct Ledger {
    schema_version: u32,
    #[serde(default)]
    volatile_fields: Vec<String>,
    #[serde(default)]
    deliberate_gaps: BTreeMap<String, DeliberateGap>,
    #[serde(default)]
    open_gaps: BTreeMap<String, OpenGap>,
    operations: Vec<Operation>,
}

#[derive(Deserialize)]
struct Operation {
    id: String,
    owner: String,
    status: String,
    result: String,
    evidence: Vec<Evidence>,
    gaps: Vec<String>,
    remaining_verification: String,
}

/// A difference the node keeps on purpose: its statement, the captures it
/// explains notes on, and the notes (`anchors`) a capture must record before
/// the gap explains any other note there.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DeliberateGap {
    text: String,
    captures: Vec<String>,
    #[serde(default)]
    anchors: Vec<String>,
}

/// A difference the node doesn't keep on purpose: as [`DeliberateGap`], and
/// the behavior of the node that closes it.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct OpenGap {
    text: String,
    closes: String,
    captures: Vec<String>,
    #[serde(default)]
    anchors: Vec<String>,
}

#[derive(Deserialize)]
struct Evidence {
    path: String,
    #[serde(default)]
    test: Option<String>,
    scope: String,
    #[serde(default)]
    replay: Option<Vec<String>>,
    #[serde(default)]
    explained_by: BTreeMap<String, String>,
}

#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct Defect {
    offset: Option<i64>,
    path: Option<String>,
    method: Option<String>,
    metadata: Option<String>,
    sent_metadata: Option<String>,
    #[serde(default)]
    untimestamped: bool,
    #[serde(default)]
    with_headers: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Seed {
    wallets: BTreeMap<String, String>,
    third_parties: Vec<SeedThirdParty>,
    registry: Vec<SeedThirdParty>,
    collections: Vec<SeedCollection>,
    items: Vec<SeedItem>,
    chain: SeedChain,
    collection_curations: Vec<SeedCollectionCuration>,
    item_curations: Vec<SeedItemCuration>,
    #[serde(default)]
    receipts: Vec<SeedReceipt>,
    #[serde(default)]
    entities: Vec<SeedEntity>,
}

/// An entity the catalyst holds, active at its pointer.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SeedEntity {
    entity_id: String,
    #[serde(rename = "type")]
    entity_type: String,
    pointer: String,
    metadata: Value,
}

/// A slot cheque the registry consumed: its receipt is the cheque's digest.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SeedReceipt {
    provider: String,
    qty: i32,
    salt: String,
    signer: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SeedCollectionCuration {
    id: uuid::Uuid,
    collection_id: uuid::Uuid,
    status: String,
    assignee: Option<String>,
    created_at: chrono::DateTime<chrono::Utc>,
    updated_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SeedItemCuration {
    id: uuid::Uuid,
    item_id: uuid::Uuid,
    status: String,
    content_hash: String,
    is_mapping_complete: Option<bool>,
    created_at: chrono::DateTime<chrono::Utc>,
    updated_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SeedThirdParty {
    id: String,
    name: String,
    managers: Vec<String>,
    #[serde(default)]
    programmatic: bool,
    approved: Option<bool>,
    max_items: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SeedItem {
    id: uuid::Uuid,
    name: String,
    owner: String,
    collection_id: Option<uuid::Uuid>,
    blockchain_item_id: Option<String>,
    thumbnail: String,
    #[serde(default)]
    rarity: Option<String>,
    #[serde(default)]
    urn_suffix: Option<String>,
    #[serde(default)]
    local_content_hash: Option<String>,
    #[serde(default)]
    contents: BTreeMap<String, String>,
    data: Value,
    metrics: Value,
    created_at: chrono::DateTime<chrono::Utc>,
    updated_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SeedChain {
    committee: Vec<String>,
    collections: Vec<ChainCollection>,
    items: Vec<ChainItem>,
    rarities: Vec<ChainRarity>,
}

#[derive(Deserialize, Clone)]
#[serde(deny_unknown_fields)]
struct ChainRarity {
    name: String,
    price: String,
    max_supply: String,
    mana_price: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ChainCollection {
    contract_address: String,
    name: String,
    creator: String,
    owner: String,
    is_approved: bool,
    managers: Vec<String>,
    minters: Vec<String>,
    created_at: i64,
    updated_at: i64,
    reviewed_at: i64,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ChainItem {
    contract_address: String,
    blockchain_id: String,
    name: String,
    category: String,
    rarity: String,
    price: String,
    beneficiary: String,
    total_supply: String,
    content_hash: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SeedCollection {
    id: uuid::Uuid,
    name: String,
    owner: String,
    salt: String,
    contract_address: Option<String>,
    third_party_id: Option<String>,
    urn_suffix: Option<String>,
    #[serde(default)]
    linked_contract_address: Option<String>,
    #[serde(default)]
    linked_contract_network: Option<String>,
    #[serde(default)]
    forum_link: Option<String>,
    #[serde(default)]
    forum_id: Option<i32>,
    created_at: chrono::DateTime<chrono::Utc>,
    updated_at: chrono::DateTime<chrono::Utc>,
}

fn parse_seed(text: &str) -> Result<Seed, String> {
    serde_json::from_str(text).map_err(|e| e.to_string())
}

fn root() -> PathBuf {
    PathBuf::from(env!("DCL_ONE_ROOT"))
}

fn ledger() -> Ledger {
    let path = root().join(FIXTURES).join("parity.json");
    serde_json::from_slice(&std::fs::read(&path).unwrap_or_else(|e| panic!("{path:?}: {e}")))
        .unwrap()
}

fn in_scope_routes() -> BTreeSet<String> {
    let source = std::fs::read_to_string(root().join(ROUTE_LEDGER)).unwrap();
    let table = &source[source.find("const UPSTREAM").unwrap()..];
    let table = &table[..table.find("];").unwrap()];
    let compact: String = table.chars().filter(|c| !c.is_whitespace()).collect();
    let rows: Vec<(String, String, String)> = compact
        .split("(\"")
        .skip(1)
        .map(|row| {
            let (method, rest) = row.split_once("\",\"").unwrap();
            let (path, rest) = rest.split_once("\",").unwrap();
            let status = rest.split(')').next().unwrap().trim_end_matches(',');
            assert!(
                ["Routed", "Todo", "Removed"].contains(&status),
                "unparsed route ledger row {row}"
            );
            (method.to_string(), path.to_string(), status.to_string())
        })
        .collect();
    assert!(
        rows.len() > 70,
        "the route ledger parsed to {} rows",
        rows.len()
    );
    rows.into_iter()
        .filter(|(_, _, status)| status != "Removed")
        .map(|(method, path, _)| format!("api_http:{method}:/v1{path}"))
        .collect()
}

fn raw_capture(path: &str) -> Value {
    let bytes = std::fs::read(root().join(path)).unwrap_or_else(|e| panic!("{path}: {e}"));
    serde_json::from_slice(&bytes).unwrap_or_else(|e| panic!("{path}: {e}"))
}

fn checked_capture(path: &str, volatile_fields: &[String]) -> Value {
    let raw = raw_capture(path);
    let relative = path.strip_prefix(&format!("{FIXTURES}/")).unwrap_or(path);
    let problems = capture_errors(relative, &raw, volatile_fields);
    assert!(problems.is_empty(), "{}", problems.join("\n"));
    raw
}

fn fixture_files() -> BTreeSet<String> {
    fn walk(dir: &Path, out: &mut BTreeSet<String>) {
        for entry in std::fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                walk(&path, out);
            } else {
                let relative = path.strip_prefix(root()).unwrap();
                out.insert(relative.to_str().unwrap().to_string());
            }
        }
    }
    let mut out = BTreeSet::new();
    walk(&root().join(FIXTURES), &mut out);
    out
}

fn exercises(id: &str, method: &str, path: &str) -> bool {
    let Some((id_method, template)) = id
        .strip_prefix("api_http:")
        .and_then(|rest| rest.split_once(':'))
    else {
        return false;
    };
    let path = path.split('?').next().unwrap_or_default();
    let path = path.strip_prefix(GATEWAY_PREFIX).unwrap_or(path);
    let path = if path.len() > 1 {
        path.strip_suffix('/').unwrap_or(path)
    } else {
        path
    };
    let segments: Vec<&str> = path.split('/').collect();
    let expected: Vec<&str> = template.split('/').collect();
    id_method == method
        && segments.len() == expected.len()
        && segments.iter().zip(&expected).all(|(segment, want)| {
            if want.starts_with(':') {
                !segment.is_empty()
            } else {
                segment == want
            }
        })
}

fn volatile_notes(raw: &Value, candidate: &[u8]) -> Vec<String> {
    let Ok(body) = serde_json::from_slice::<Value>(candidate) else {
        return Vec::new();
    };
    let at = |path: &str| values_at(&body, &path.split('.').collect::<Vec<_>>());
    let mut notes = Vec::new();
    for path in strings(raw, "volatile_paths") {
        if at(&path).iter().any(|v| !(v.is_string() || v.is_number())) {
            notes.push(format!("{path} volatile value is not a timestamp"));
        }
    }
    for paths in equalities(raw) {
        let values: Vec<Vec<&Value>> = paths.iter().map(|p| at(p)).collect();
        if values.windows(2).any(|pair| pair[0] != pair[1]) {
            notes.push(format!("{} differ", paths.join(" and ")));
        }
    }
    notes
}

fn request_of(raw: &Value) -> Option<(String, String)> {
    let method = raw["request"]["method"].as_str()?;
    let method = match raw["request"]["headers"]["access-control-request-method"].as_str() {
        Some(asked) if method == "OPTIONS" => asked,
        _ => method,
    };
    Some((
        method.to_string(),
        raw["request"]["path"].as_str()?.to_string(),
    ))
}

fn strings(raw: &Value, key: &str) -> Vec<String> {
    serde_json::from_value(raw[key].clone()).unwrap_or_default()
}

fn equalities(raw: &Value) -> Vec<Vec<String>> {
    serde_json::from_value(raw["volatile_equal"].clone()).unwrap_or_default()
}

struct Marker {
    path: String,
    at: *const Value,
    redacted: bool,
}

fn markers(value: &Value, path: String, out: &mut Vec<Marker>) {
    match value {
        Value::String(text)
            if text.contains(REDACTION_MARK) || text.contains(VOLATILE_SENTINEL) =>
        {
            out.push(Marker {
                path,
                at: value,
                redacted: text.contains(REDACTION_MARK),
            })
        }
        Value::Array(items) => {
            for (i, item) in items.iter().enumerate() {
                markers(item, format!("{path}[{i}]"), out);
            }
        }
        Value::Object(map) => {
            for (key, child) in map {
                let child_path = if path.is_empty() {
                    key.clone()
                } else {
                    format!("{path}.{key}")
                };
                markers(child, child_path, out);
            }
        }
        _ => {}
    }
}

fn capture_errors(relative: &str, raw: &Value, volatile_fields: &[String]) -> Vec<String> {
    let mut errors = Vec::new();
    let volatile_paths = strings(raw, "volatile_paths");
    let body = &raw["response"]["body_json"];
    let mut reached = BTreeSet::new();
    for volatile in &volatile_paths {
        let field = volatile.rsplit('.').next().unwrap_or_default();
        if !volatile_fields.iter().any(|allowed| allowed == field) {
            errors.push(format!(
                "{relative}: volatile path {volatile} is not one of the volatile_fields"
            ));
            continue;
        }
        let found = values_at(body, &volatile.split('.').collect::<Vec<_>>());
        if found.is_empty() || found.iter().any(|v| v.as_str() != Some(VOLATILE_SENTINEL)) {
            errors.push(format!(
                "{relative}: {volatile} is not recorded as {VOLATILE_SENTINEL}"
            ));
        }
        reached.extend(found.into_iter().map(|v| v as *const Value));
    }
    let mut found = Vec::new();
    markers(raw, String::new(), &mut found);
    for marker in found {
        let path = marker.path;
        if marker.redacted {
            errors.push(format!("{relative}: <<REDACTED>> at {path}"));
        } else if !reached.contains(&marker.at) {
            errors.push(format!(
                "{relative}: {VOLATILE_SENTINEL} at {path} is not reached by a volatile path"
            ));
        }
    }
    for path in equalities(raw).iter().flatten() {
        if !volatile_paths.contains(path) {
            errors.push(format!(
                "{relative}: volatile_equal names {path}, which is not a volatile path"
            ));
        }
    }
    if let Some(sign) = raw.get("sign") {
        match serde_json::from_value::<Defect>(sign.clone()) {
            Ok(defect) => errors.extend(
                defect_problems(&defect)
                    .into_iter()
                    .map(|problem| format!("{relative}: sign {problem}")),
            ),
            Err(e) => errors.push(format!("{relative}: sign: {e}")),
        }
        if raw.get("auth").is_some() {
            errors.push(format!(
                "{relative}: a capture is re-signed with its defect or replayed as recorded, not both"
            ));
        }
        if signer_of(raw).is_none() {
            errors.push(format!("{relative}: sign names no signer"));
        }
    }
    if let Some(sign) = raw.get("validator_sign") {
        match serde_json::from_value::<ValidatorSign>(sign.clone()) {
            Ok(ValidatorSign {
                secret: Some(secret),
                ..
            }) if secret != "wrong" => errors.push(format!(
                "{relative}: validator_sign names a secret other than \"wrong\""
            )),
            Ok(_) => {}
            Err(e) => errors.push(format!("{relative}: validator_sign: {e}")),
        }
        if raw.get("sign").is_some() || raw.get("auth").is_some() || signer_of(raw).is_some() {
            errors.push(format!(
                "{relative}: a validator's capture carries no wallet signature"
            ));
        }
    }
    if raw.get("validation_of").is_some() && raw.get("validator_sign").is_none() {
        errors.push(format!(
            "{relative}: validation_of names the claim of a validator's capture only"
        ));
    }
    if raw
        .get("review_flow")
        .is_some_and(|flow| flow != &Value::Bool(true))
    {
        errors.push(format!("{relative}: review_flow is true or absent"));
    }
    match raw.get("auth") {
        None => {}
        Some(Value::String(mode)) if mode == "recorded" => {
            if recorded_auth(raw).is_empty() {
                errors.push(format!(
                    "{relative}: auth is recorded but the request carries no x-identity- header"
                ));
            }
        }
        Some(other) => errors.push(format!(
            "{relative}: auth must be \"recorded\", not {other}"
        )),
    }
    errors
}

fn recorded_auth(raw: &Value) -> HeaderMap {
    let mut headers = HeaderMap::new();
    for (name, value) in raw["request"]["headers"].as_object().into_iter().flatten() {
        if name.starts_with("x-identity-") {
            headers.insert(
                HeaderName::from_bytes(name.as_bytes()).unwrap(),
                HeaderValue::from_str(value.as_str().unwrap_or_default()).unwrap(),
            );
        }
    }
    headers
}

fn values_at<'a>(value: &'a Value, parts: &[&str]) -> Vec<&'a Value> {
    match value {
        Value::Array(items) => items
            .iter()
            .flat_map(|item| values_at(item, parts))
            .collect(),
        _ if parts.is_empty() => vec![value],
        Value::Object(map) => map
            .get(parts[0])
            .map(|child| values_at(child, &parts[1..]))
            .unwrap_or_default(),
        _ => Vec::new(),
    }
}

fn captures(files: &BTreeSet<String>) -> BTreeSet<String> {
    files
        .iter()
        .filter(|path| {
            ["source", "live"].iter().any(|dir| {
                path.strip_prefix(&format!("{FIXTURES}/{dir}/"))
                    .is_some_and(|name| name.ends_with(".json") && !name.contains('/'))
            })
        })
        .cloned()
        .collect()
}

/// The captures the gap `reference` (`deliberate:<name>` or `open:<name>`)
/// lists and its anchors, when the ledger states it.
fn stated_gap<'a>(
    ledger: &'a Ledger,
    reference: &str,
) -> Option<(&'a Vec<String>, &'a Vec<String>)> {
    if let Some(name) = reference.strip_prefix("deliberate:") {
        return ledger
            .deliberate_gaps
            .get(name)
            .map(|g| (&g.captures, &g.anchors));
    }
    let name = reference.strip_prefix("open:")?;
    ledger
        .open_gaps
        .get(name)
        .map(|g| (&g.captures, &g.anchors))
}

/// Captures in the order both sides run them: `live/` then `source/`, each
/// by the number its name starts with. A name without a number, or two
/// captures of one folder with the same number, is refused.
fn replay_order(paths: BTreeSet<String>) -> Result<Vec<String>, String> {
    let mut keyed = BTreeMap::new();
    for path in paths {
        let relative = path.strip_prefix(&format!("{FIXTURES}/")).unwrap_or(&path);
        let (dir, name) = relative.rsplit_once('/').unwrap_or(("", relative));
        let number: u32 = name
            .split_once('-')
            .and_then(|(n, _)| n.parse().ok())
            .ok_or_else(|| format!("{relative}: a capture name starts with its number"))?;
        if let Some(other) = keyed.insert((dir.to_string(), number), path.clone()) {
            return Err(format!("{relative} and {other} share the number {number}"));
        }
    }
    Ok(keyed.into_values().collect())
}

fn ledger_errors(
    ledger: &Ledger,
    routes: &BTreeSet<String>,
    files: &BTreeSet<String>,
    captured: &BTreeMap<String, Value>,
) -> Vec<String> {
    let mut errors = Vec::new();
    if let Err(e) = replay_order(captures(files)) {
        errors.push(e);
    }
    if ledger.schema_version != 1 {
        errors.push(format!("schema_version {}", ledger.schema_version));
    }
    let named: BTreeSet<String> = ledger.operations.iter().map(|o| o.id.clone()).collect();
    if named.len() != ledger.operations.len() {
        errors.push("duplicate operations in parity.json".to_string());
    }
    for id in routes.difference(&named) {
        errors.push(format!("{id}: missing from parity.json"));
    }
    for id in named.difference(routes) {
        errors.push(format!("{id}: not an in-scope route"));
    }
    let replayed = captures(files);
    let mut cited = BTreeMap::new();
    let mut attributed = BTreeSet::new();
    let mut deliberate = BTreeSet::new();
    let mut open = BTreeSet::new();
    for op in &ledger.operations {
        let id = &op.id;
        for gap in &op.gaps {
            if let Some(name) = gap.strip_prefix("deliberate:") {
                if !ledger.deliberate_gaps.contains_key(name) {
                    errors.push(format!("{id}: no deliberate gap named {name}"));
                }
                deliberate.insert(name);
            } else if let Some(name) = gap.strip_prefix("open:") {
                if !ledger.open_gaps.contains_key(name) {
                    errors.push(format!("{id}: no open gap named {name}"));
                }
                open.insert(name);
            } else {
                errors.push(format!("{id}: gap {gap} is neither deliberate: nor open:"));
            }
        }
        let has_open = op.gaps.iter().any(|g| g.starts_with("open:"));
        if op.status == "reviewed" && has_open {
            errors.push(format!("{id}: reviewed with an open gap"));
        }
        if op.status == "known_gap" && !has_open {
            errors.push(format!("{id}: known_gap names no open gap"));
        }
        if !STATUSES.contains(&op.status.as_str()) {
            errors.push(format!("{id}: status {}", op.status));
        }
        if op.owner.is_empty() || op.result.is_empty() || op.remaining_verification.is_empty() {
            errors.push(format!(
                "{id}: owner, result and remaining_verification are required"
            ));
        }
        if op.status != "not_reviewed" && op.evidence.is_empty() {
            errors.push(format!("{id}: a review needs evidence"));
        }
        for evidence in &op.evidence {
            let path = &evidence.path;
            let exists = if path.starts_with(FIXTURES) {
                files.contains(path)
            } else {
                root().join(path).is_file()
            };
            if !exists {
                errors.push(format!("{id}: missing evidence {path}"));
                continue;
            }
            if evidence.scope.is_empty() {
                errors.push(format!("{id}: evidence {path} states no scope"));
            }
            if let Some(test) = &evidence.test {
                let text = std::fs::read_to_string(root().join(path)).unwrap_or_default();
                if !text.contains(&format!("fn {test}(")) {
                    errors.push(format!("{id}: no test {test} in {path}"));
                }
            }
            let under_fixtures = path.strip_prefix(&format!("{FIXTURES}/"));
            if let Some(relative) = under_fixtures {
                if !replayed.contains(path) {
                    errors.push(format!("{relative} is not a replayed capture"));
                }
                if evidence.replay.is_none() {
                    errors.push(format!("{id} cites {relative} without its replay"));
                }
            }
            if let (Some(_), Some((method, request_path))) =
                (&evidence.replay, captured.get(path).and_then(request_of))
                && !exercises(id, &method, &request_path)
            {
                let relative = under_fixtures.unwrap_or(path);
                errors.push(format!(
                    "{relative} requests {method} {request_path}, not {id}"
                ));
            }
            if let Some(notes) = &evidence.replay {
                if under_fixtures.is_none() {
                    errors.push(format!("{id}: replay of a non-capture {path}"));
                }
                let relative = under_fixtures.unwrap_or(path);
                for (note, gap) in &evidence.explained_by {
                    if !notes.contains(note) {
                        errors.push(format!(
                            "{relative}: explains {note}, which it does not record"
                        ));
                    }
                    if !op.gaps.contains(gap) {
                        errors.push(format!("{relative}: {gap}, which {id} does not list"));
                    }
                    attributed.insert((gap.as_str(), relative));
                    match stated_gap(ledger, gap) {
                        None => errors.push(format!("{relative}: {gap} is no stated gap")),
                        Some((captures, _)) if !captures.iter().any(|c| c == relative) => {
                            errors.push(format!("{relative}: {gap} does not list this capture"))
                        }
                        Some((_, anchors)) => {
                            if !anchors.contains(note)
                                && let Some(missing) = anchors.iter().find(|a| !notes.contains(a))
                            {
                                errors.push(format!(
                                    "{relative}: {gap} explains {note} only beside {missing}"
                                ));
                            }
                        }
                    }
                }
                for note in notes
                    .iter()
                    .filter(|n| !evidence.explained_by.contains_key(*n))
                {
                    errors.push(format!("{relative}: {note} is explained by no gap"));
                }
                if let Some(other) = cited.insert(path.clone(), id.clone()) {
                    errors.push(format!("{path} is evidence of {other} and {id}"));
                }
            }
        }
        let replays_a_capture = op
            .evidence
            .iter()
            .any(|e| e.replay.is_some() && replayed.contains(&e.path));
        if op.status == "reviewed" && !replays_a_capture {
            errors.push(format!("{id}: reviewed without a replayed capture"));
        }
    }
    let stated = ledger
        .deliberate_gaps
        .iter()
        .map(|(name, g)| {
            (
                "deliberate",
                name,
                &g.text,
                &g.captures,
                deliberate.contains(name.as_str()),
            )
        })
        .chain(ledger.open_gaps.iter().map(|(name, g)| {
            if g.closes.trim().is_empty() {
                errors.push(format!("open gap {name}: says nothing of what closes it"));
            }
            (
                "open",
                name,
                &g.text,
                &g.captures,
                open.contains(name.as_str()),
            )
        }))
        .collect::<Vec<_>>();
    for (kind, name, text, listed, referenced) in stated {
        let label = format!("{kind} gap {name}");
        if text.trim().is_empty() {
            errors.push(format!("{label}: states nothing"));
        }
        if !referenced {
            errors.push(format!("{label}: no operation references it"));
        }
        let reference = format!("{kind}:{name}");
        for capture in listed {
            if !replayed.contains(&format!("{FIXTURES}/{capture}")) {
                errors.push(format!("{label}: lists {capture}, which is no capture"));
            } else if !attributed.contains(&(reference.as_str(), capture.as_str())) {
                errors.push(format!(
                    "{label}: lists {capture}, which attributes no note to it"
                ));
            }
        }
    }
    for path in files {
        let relative = path.strip_prefix(&format!("{FIXTURES}/")).unwrap_or(path);
        let data = ["parity.json", "seed.json"].contains(&relative);
        if path.ends_with(".json") && !data && !replayed.contains(path) {
            errors.push(format!("{relative} is not a replayed capture"));
        }
    }
    for path in &replayed {
        let relative = path.strip_prefix(&format!("{FIXTURES}/")).unwrap_or(path);
        let Some(raw) = captured.get(path) else {
            errors.push(format!("{relative}: not parsed"));
            continue;
        };
        if let Err(e) = serde_json::from_value::<Fixture>(raw.clone()) {
            errors.push(format!("{relative}: {e}"));
        }
        errors.extend(capture_errors(relative, raw, &ledger.volatile_fields));
        if let Some((method, request_path)) = request_of(raw) {
            let matching: Vec<&str> = routes
                .iter()
                .filter(|route| exercises(route, &method, &request_path))
                .map(String::as_str)
                .collect();
            if matching.len() > 1 {
                errors.push(format!(
                    "{relative} requests {method} {request_path}, which matches several operations: {}",
                    matching.join(", ")
                ));
            }
        }
        if !cited.contains_key(path) {
            errors.push(format!("{path}: no operation cites this capture"));
        }
    }
    errors.sort();
    errors.dedup();
    errors
}

#[test]
fn the_parity_ledger_names_every_in_scope_route() {
    let files = fixture_files();
    let fixtures = captures(&files)
        .into_iter()
        .map(|path| {
            let raw = raw_capture(&path);
            (path, raw)
        })
        .collect();
    let errors = ledger_errors(&ledger(), &in_scope_routes(), &files, &fixtures);
    assert!(errors.is_empty(), "parity.json:\n{}", errors.join("\n"));
}

const INFO: &str = "api_http:GET:/v1/info";

fn fixture(name: &str) -> String {
    format!("{FIXTURES}/{name}")
}

fn capture(method: &str, path: &str, body: Value, volatile: &[&str]) -> Value {
    serde_json::json!({
        "request": {"method": method, "path": path},
        "response": {"status": 200, "body_json": body},
        "volatile_paths": volatile
    })
}

fn info_captures(files: &BTreeSet<String>) -> BTreeMap<String, Value> {
    captures(files)
        .into_iter()
        .map(|path| (path, capture("GET", "/v1/info", serde_json::json!({}), &[])))
        .collect()
}

fn ledger_value(operations: Value) -> Ledger {
    serde_json::from_value(serde_json::json!({
        "schema_version": 1,
        "volatile_fields": ["created_at", "updated_at"],
        "operations": operations
    }))
    .unwrap()
}

fn checked(operations: Value, files: &[&str]) -> Vec<String> {
    let files = files.iter().map(|name| fixture(name)).collect();
    let routes = BTreeSet::from([INFO.to_string()]);
    ledger_errors(
        &ledger_value(operations),
        &routes,
        &files,
        &info_captures(&files),
    )
}

fn operation(status: &str, evidence: Value) -> Value {
    serde_json::json!({
        "id": INFO, "owner": "small-reads", "status": status, "result": "compared",
        "evidence": evidence, "gaps": if status == "known_gap" { vec!["differs"] } else { vec![] },
        "remaining_verification": "more cases"
    })
}

fn refused(errors: &[String], expected: &str) {
    assert!(
        errors.iter().any(|e| e.contains(expected)),
        "expected an error containing {expected:?}, got {errors:#?}"
    );
}

#[test]
fn captures_replay_in_the_order_of_their_number() {
    let paths = [
        "source/99-last.json",
        "source/100-after.json",
        "source/2-first.json",
        "live/1-public.json",
    ]
    .map(fixture);
    assert_eq!(
        replay_order(paths.iter().cloned().collect()).unwrap(),
        [
            "live/1-public.json",
            "source/2-first.json",
            "source/99-last.json",
            "source/100-after.json"
        ]
        .map(fixture)
        .to_vec()
    );
    let error = replay_order(
        ["source/05-one.json", "source/5-two.json"]
            .map(fixture)
            .into_iter()
            .collect(),
    )
    .unwrap_err();
    assert!(error.contains("number 5"), "{error}");
    assert!(
        replay_order(BTreeSet::from([fixture("source/unnumbered.json")]))
            .unwrap_err()
            .contains("unnumbered.json")
    );
}

#[test]
fn a_reviewed_operation_with_a_matching_capture_is_accepted() {
    let evidence =
        serde_json::json!([{"path": fixture("source/01-info.json"), "scope": "s", "replay": []}]);
    let errors = checked(
        serde_json::json!([operation("reviewed", evidence)]),
        &["parity.json", "seed.json", "source/01-info.json"],
    );
    assert!(errors.is_empty(), "{errors:#?}");
}

const STATUS_NOTE: &str = "status: fixture=200 candidate=400";

fn deliberate_gaps() -> Value {
    serde_json::json!({
        "stricter-input": {"text": "The node refuses an input upstream accepts.",
            "captures": [], "anchors": [STATUS_NOTE]},
        "fixed-message": {"text": "The node's message is fixed.", "captures": []}
    })
}

fn open_gaps() -> Value {
    serde_json::json!({
        "registry": {"text": "The node reads no registry.", "closes": "The node reads the registry.",
            "captures": []}
    })
}

/// The checker's errors for the committed ledger after `edit`, against the
/// committed captures.
fn parity_errors_after(edit: impl FnOnce(&mut Value)) -> Vec<String> {
    let path = root().join(FIXTURES).join("parity.json");
    let mut value: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    edit(&mut value);
    let files = fixture_files();
    let fixtures = captures(&files)
        .into_iter()
        .map(|path| {
            let raw = raw_capture(&path);
            (path, raw)
        })
        .collect();
    ledger_errors(
        &serde_json::from_value(value).unwrap(),
        &in_scope_routes(),
        &files,
        &fixtures,
    )
}

#[test]
fn a_gap_explains_nothing_on_a_capture_it_does_not_list() {
    let errors = parity_errors_after(|ledger| {
        let evidence = ledger["operations"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .flat_map(|op| op["evidence"].as_array_mut().unwrap())
            .find(|e| {
                e["path"]
                    .as_str()
                    .unwrap()
                    .ends_with("source/190-item-save-draft-owner.json")
            })
            .unwrap();
        let notes = [
            "status: fixture=200 candidate=401",
            "data.name missing",
            "error extra",
            "ok value",
        ];
        evidence["replay"] = serde_json::json!(notes);
        evidence["explained_by"] = notes
            .iter()
            .map(|n| (n.to_string(), Value::from("deliberate:stored-name-trimmed")))
            .collect::<serde_json::Map<_, _>>()
            .into();
    });
    refused(
        &errors,
        "source/190-item-save-draft-owner.json: deliberate:stored-name-trimmed does not list this capture",
    );
}

#[test]
fn a_deliberate_gap_explains_nothing_on_a_capture_it_does_not_list() {
    refused(
        &ledger_with(
            "reviewed",
            serde_json::json!(["deliberate:fixed-message"]),
            serde_json::json!([STATUS_NOTE]),
            serde_json::json!({ STATUS_NOTE: "deliberate:fixed-message" }),
            deliberate_gaps(),
            open_gaps(),
        ),
        "source/01-info.json: deliberate:fixed-message does not list this capture",
    );
}

#[test]
fn a_gap_listing_a_capture_that_attributes_nothing_to_it_is_refused() {
    let mut deliberate = deliberate_gaps();
    deliberate["stricter-input"]["captures"] = serde_json::json!(["source/01-info.json"]);
    deliberate["fixed-message"]["captures"] = serde_json::json!(["source/01-info.json"]);
    refused(
        &ledger_with(
            "reviewed",
            serde_json::json!(["deliberate:stricter-input", "deliberate:fixed-message"]),
            serde_json::json!([STATUS_NOTE]),
            serde_json::json!({ STATUS_NOTE: "deliberate:stricter-input" }),
            deliberate,
            open_gaps(),
        ),
        "deliberate gap fixed-message: lists source/01-info.json, which attributes no note to it",
    );
}

#[test]
fn a_gap_listing_no_capture_is_refused() {
    let mut deliberate = deliberate_gaps();
    deliberate["stricter-input"]["captures"] =
        serde_json::json!(["source/01-info.json", "source/99-gone.json"]);
    refused(
        &ledger_with(
            "reviewed",
            serde_json::json!(["deliberate:stricter-input"]),
            serde_json::json!([STATUS_NOTE]),
            serde_json::json!({ STATUS_NOTE: "deliberate:stricter-input" }),
            deliberate,
            open_gaps(),
        ),
        "deliberate gap stricter-input: lists source/99-gone.json, which is no capture",
    );
}

/// The checker's errors for one operation with `status` and `gaps` whose
/// capture replays with `notes`, explained by `explained_by`, against the
/// stated `deliberate` and `open` gaps.
fn ledger_with(
    status: &str,
    gaps: Value,
    notes: Value,
    explained_by: Value,
    deliberate: Value,
    open: Value,
) -> Vec<String> {
    let mut op = operation(
        status,
        serde_json::json!([{"path": fixture("source/01-info.json"), "scope": "s",
            "replay": notes, "explained_by": explained_by}]),
    );
    op["gaps"] = gaps;
    let ledger: Ledger = serde_json::from_value(serde_json::json!({
        "schema_version": 1,
        "volatile_fields": ["created_at", "updated_at"],
        "deliberate_gaps": deliberate,
        "open_gaps": open,
        "operations": [op]
    }))
    .unwrap();
    let files = ["parity.json", "seed.json", "source/01-info.json"]
        .iter()
        .map(|name| fixture(name))
        .collect();
    ledger_errors(
        &ledger,
        &BTreeSet::from([INFO.to_string()]),
        &files,
        &info_captures(&files),
    )
    .into_iter()
    .filter(|e| !e.contains("no operation references it"))
    .collect()
}

/// As [`ledger_with`] with one recorded difference, and each stated gap
/// `explained_by` names listing the capture.
fn with_difference(status: &str, gaps: Value, explained_by: Value) -> Vec<String> {
    let (mut deliberate, mut open) = (deliberate_gaps(), open_gaps());
    for gap in explained_by.as_object().unwrap().values() {
        let (kind, name) = gap.as_str().unwrap().split_once(':').unwrap();
        let stated = if kind == "open" {
            &mut open
        } else {
            &mut deliberate
        };
        if let Some(stated) = stated.get_mut(name) {
            stated["captures"] = serde_json::json!(["source/01-info.json"]);
        }
    }
    ledger_with(
        status,
        gaps,
        serde_json::json!([STATUS_NOTE]),
        explained_by,
        deliberate,
        open,
    )
}

#[test]
fn a_reviewed_operation_whose_differences_its_deliberate_gaps_explain_is_accepted() {
    let errors = with_difference(
        "reviewed",
        serde_json::json!(["deliberate:stricter-input"]),
        serde_json::json!({ STATUS_NOTE: "deliberate:stricter-input" }),
    );
    assert!(errors.is_empty(), "{errors:#?}");
}

#[test]
fn a_reviewed_difference_no_gap_explains_is_refused() {
    refused(
        &with_difference(
            "reviewed",
            serde_json::json!(["deliberate:stricter-input"]),
            serde_json::json!({}),
        ),
        "source/01-info.json: status: fixture=200 candidate=400 is explained by no gap",
    );
}

#[test]
fn a_known_gap_difference_no_gap_explains_is_refused() {
    refused(
        &with_difference(
            "known_gap",
            serde_json::json!(["open:registry"]),
            serde_json::json!({}),
        ),
        "source/01-info.json: status: fixture=200 candidate=400 is explained by no gap",
    );
}

#[test]
fn an_open_gap_explains_a_known_gap_difference() {
    let errors = with_difference(
        "known_gap",
        serde_json::json!(["open:registry"]),
        serde_json::json!({ STATUS_NOTE: "open:registry" }),
    );
    assert!(errors.is_empty(), "{errors:#?}");
}

#[test]
fn a_reviewed_operation_with_an_open_gap_is_refused() {
    refused(
        &with_difference(
            "reviewed",
            serde_json::json!(["open:registry"]),
            serde_json::json!({ STATUS_NOTE: "open:registry" }),
        ),
        "api_http:GET:/v1/info: reviewed with an open gap",
    );
}

#[test]
fn a_known_gap_without_an_open_gap_is_refused() {
    refused(
        &with_difference(
            "known_gap",
            serde_json::json!(["deliberate:stricter-input"]),
            serde_json::json!({ STATUS_NOTE: "deliberate:stricter-input" }),
        ),
        "api_http:GET:/v1/info: known_gap names no open gap",
    );
}

#[test]
fn a_gap_that_is_neither_deliberate_nor_open_is_refused() {
    refused(
        &with_difference(
            "known_gap",
            serde_json::json!(["open:registry", "the node differs"]),
            serde_json::json!({ STATUS_NOTE: "open:registry" }),
        ),
        "api_http:GET:/v1/info: gap the node differs is neither deliberate: nor open:",
    );
}

#[test]
fn a_difference_explained_by_a_gap_its_operation_does_not_list_is_refused() {
    refused(
        &with_difference(
            "reviewed",
            serde_json::json!(["deliberate:stricter-input"]),
            serde_json::json!({ STATUS_NOTE: "deliberate:fixed-message" }),
        ),
        "source/01-info.json: deliberate:fixed-message, which api_http:GET:/v1/info does not list",
    );
}

#[test]
fn a_refusal_gap_explains_nothing_on_a_capture_without_its_anchor() {
    let mut deliberate = deliberate_gaps();
    deliberate["stricter-input"]["captures"] = serde_json::json!(["source/01-info.json"]);
    refused(
        &ledger_with(
            "reviewed",
            serde_json::json!(["deliberate:stricter-input"]),
            serde_json::json!(["data.salt missing"]),
            serde_json::json!({ "data.salt missing": "deliberate:stricter-input" }),
            deliberate,
            open_gaps(),
        ),
        "source/01-info.json: deliberate:stricter-input explains data.salt missing only beside status: fixture=200 candidate=400",
    );
}

#[test]
fn a_difference_explained_by_an_unstated_gap_is_refused() {
    refused(
        &with_difference(
            "reviewed",
            serde_json::json!(["deliberate:unstated"]),
            serde_json::json!({ STATUS_NOTE: "deliberate:unstated" }),
        ),
        "no deliberate gap named unstated",
    );
}

#[test]
fn an_explanation_of_a_difference_the_capture_does_not_record_is_refused() {
    refused(
        &with_difference(
            "reviewed",
            serde_json::json!(["deliberate:stricter-input"]),
            serde_json::json!({ STATUS_NOTE: "deliberate:stricter-input", "data.x missing": "deliberate:stricter-input" }),
        ),
        "source/01-info.json: explains data.x missing, which it does not record",
    );
}

#[test]
fn a_capture_cited_without_its_replay_is_refused() {
    let mut gap = operation(
        "known_gap",
        serde_json::json!([{"path": fixture("source/01-info.json"), "scope": "s", "replay": ["status: fixture=200 candidate=404"]}]),
    );
    gap["id"] = "api_http:GET:/v1/rarities".into();
    let reviewed = operation(
        "reviewed",
        serde_json::json!([{"path": fixture("source/01-info.json"), "scope": "s"}]),
    );
    let ledger = ledger_value(serde_json::json!([reviewed, gap]));
    let routes = BTreeSet::from([INFO.to_string(), "api_http:GET:/v1/rarities".to_string()]);
    let files = BTreeSet::from([fixture("source/01-info.json")]);
    refused(
        &ledger_errors(&ledger, &routes, &files, &info_captures(&files)),
        "cites source/01-info.json without its replay",
    );
}

#[test]
fn a_replay_of_a_file_that_is_not_a_capture_is_refused() {
    let evidence = serde_json::json!([{"path": fixture("seed.json"), "scope": "s", "replay": []}]);
    let errors = checked(
        serde_json::json!([operation("reviewed", evidence)]),
        &["parity.json", "seed.json"],
    );
    refused(&errors, "seed.json is not a replayed capture");
}

#[test]
fn a_capture_outside_source_and_live_is_refused() {
    let evidence = serde_json::json!([{"path": fixture("source/extra/01-info.json"), "scope": "s", "replay": []}]);
    let errors = checked(
        serde_json::json!([operation("reviewed", evidence)]),
        &[
            "parity.json",
            "seed.json",
            "source/extra/01-info.json",
            "captures/02-info.json",
        ],
    );
    refused(
        &errors,
        "source/extra/01-info.json is not a replayed capture",
    );
    refused(&errors, "captures/02-info.json is not a replayed capture");
}

#[test]
fn a_reviewed_operation_without_a_replayed_capture_is_refused() {
    let evidence = serde_json::json!([{
        "path": "services/catalyst/tests/builder_conformance.rs",
        "test": "the_parity_ledger_names_every_in_scope_route",
        "scope": "s"
    }]);
    let errors = checked(
        serde_json::json!([operation("reviewed", evidence)]),
        &["parity.json", "seed.json"],
    );
    refused(&errors, "reviewed without a replayed capture");
}

#[test]
fn a_capture_of_another_operation_is_refused() {
    let collections = "api_http:GET:/v1/collections";
    let path = fixture("source/01-collections-public-search.json");
    let mut moved = operation(
        "reviewed",
        serde_json::json!([{"path": path, "scope": "s", "replay": []}]),
    );
    moved["id"] = INFO.into();
    let mut left = operation("not_reviewed", serde_json::json!([]));
    left["id"] = collections.into();
    let routes = BTreeSet::from([INFO.to_string(), collections.to_string()]);
    let files = BTreeSet::from([path.clone()]);
    let fixtures = BTreeMap::from([(
        path,
        capture(
            "GET",
            "/v1/collections",
            serde_json::json!({"ok": true, "data": []}),
            &[],
        ),
    )]);
    let ledger = ledger_value(serde_json::json!([moved, left]));
    refused(
        &ledger_errors(&ledger, &routes, &files, &fixtures),
        "source/01-collections-public-search.json requests GET /v1/collections, not api_http:GET:/v1/info",
    );
}

#[test]
fn a_capture_matches_its_operation_by_method_and_path_template() {
    let id = "api_http:GET:/v1/collections/:id/items";
    assert!(exercises(id, "GET", "/v1/collections/c011ec71/items"));
    assert!(exercises(id, "GET", "/v1/collections/c011ec71/items/"));
    assert!(exercises(
        id,
        "GET",
        "/builder-api/v1/collections/c011ec71/items"
    ));
    assert!(exercises(
        id,
        "GET",
        "/v1/collections/c011ec71/items?page=1"
    ));
    assert!(!exercises(id, "PUT", "/v1/collections/c011ec71/items"));
    assert!(!exercises(id, "GET", "/v1/collections//items"));
    assert!(!exercises(
        id,
        "GET",
        "/v1/collections/c011ec71/items/extra"
    ));
    assert!(!exercises(id, "GET", "/v1/collections/c011ec71"));
    assert!(!exercises(
        id,
        "GET",
        "/builder-api/collections/c011ec71/items"
    ));
    assert!(exercises(
        "api_http:GET:/v1/collections",
        "GET",
        "/v1/collections/"
    ));
    assert!(!exercises(
        "api_http:GET:/v1/collections",
        "GET",
        "/v1/collectionsx"
    ));
    assert!(exercises(
        "api_http:HEAD:/v1/storage/contents/:filename",
        "HEAD",
        "/v1/storage/contents/Qm1"
    ));
}

#[test]
fn a_volatile_path_outside_the_allowed_fields_is_refused() {
    let path = fixture("source/01-info.json");
    let evidence = serde_json::json!([{"path": path, "scope": "s", "replay": []}]);
    let ledger = ledger_value(serde_json::json!([operation("reviewed", evidence)]));
    let routes = BTreeSet::from([INFO.to_string()]);
    let files = BTreeSet::from([path.clone()]);
    let check = |fx: Value| {
        ledger_errors(
            &ledger,
            &routes,
            &files,
            &BTreeMap::from([(path.clone(), fx)]),
        )
    };
    let masked =
        serde_json::json!({"data": "<<VOLATILE>>", "created": {"created_at": "<<VOLATILE>>"}});
    let only_created = serde_json::json!({"created": {"created_at": "<<VOLATILE>>"}});
    let allowed = check(capture(
        "GET",
        "/v1/info",
        only_created,
        &["created.created_at"],
    ));
    assert!(allowed.is_empty(), "{allowed:#?}");
    refused(
        &check(capture("GET", "/v1/info", masked.clone(), &["data"])),
        "source/01-info.json: volatile path data is not one of the volatile_fields",
    );
    refused(
        &check(capture("GET", "/v1/info", masked, &["data[]"])),
        "source/01-info.json: volatile path data[] is not one of the volatile_fields",
    );
    let recorded = serde_json::json!({"created": {"created_at": "2025-12-01T00:00:00.000Z"}});
    refused(
        &check(capture(
            "GET",
            "/v1/info",
            recorded,
            &["created.created_at"],
        )),
        "source/01-info.json: created.created_at is not recorded as <<VOLATILE>>",
    );
    let missing = serde_json::json!({"created": {}});
    refused(
        &check(capture("GET", "/v1/info", missing, &["created.created_at"])),
        "source/01-info.json: created.created_at is not recorded as <<VOLATILE>>",
    );
}

fn one_capture(fx: Value, extra_routes: &[&str]) -> Vec<String> {
    let path = fixture("source/01-info.json");
    let evidence = serde_json::json!([{"path": path, "scope": "s", "replay": []}]);
    let ledger = ledger_value(serde_json::json!([operation("reviewed", evidence)]));
    let mut routes = BTreeSet::from([INFO.to_string()]);
    routes.extend(extra_routes.iter().map(|r| r.to_string()));
    let mut ledger = ledger;
    for route in extra_routes {
        let mut other = operation("not_reviewed", serde_json::json!([]));
        other["id"] = (*route).into();
        ledger
            .operations
            .push(serde_json::from_value(other).unwrap());
    }
    let files = BTreeSet::from([path.clone()]);
    let fixtures = BTreeMap::from([(path, fx)]);
    ledger_errors(&ledger, &routes, &files, &fixtures)
}

#[test]
fn a_review_flow_or_claim_marker_the_replay_cannot_apply_is_refused() {
    let base = |extra: Value| {
        let mut fx = serde_json::json!({
            "request": {"method": "GET", "path": "/v1/info"},
            "response": {"status": 200, "body_json": {"ok": true}}
        });
        fx.as_object_mut()
            .unwrap()
            .extend(extra.as_object().unwrap().clone());
        fx
    };
    refused(
        &one_capture(
            base(serde_json::json!({ "validation_of": "c011ec71-0000-4000-8000-000000000302" })),
            &[],
        ),
        "source/01-info.json: validation_of names the claim of a validator's capture only",
    );
    refused(
        &one_capture(base(serde_json::json!({ "review_flow": "true" })), &[]),
        "source/01-info.json: review_flow is true or absent",
    );
    assert!(one_capture(base(serde_json::json!({ "review_flow": true })), &[]).is_empty());
}

#[test]
fn a_marker_written_into_a_capture_is_refused() {
    let unreached = serde_json::json!({
        "request": {"method": "GET", "path": "/v1/info"},
        "response": {"status": 200, "body_json": {"ok": true, "data": "<<VOLATILE>>"}},
        "volatile_paths": []
    });
    refused(
        &one_capture(unreached, &[]),
        "source/01-info.json: <<VOLATILE>> at response.body_json.data is not reached by a volatile path",
    );
    let redacted = serde_json::json!({
        "request": {"method": "GET", "path": "/v1/info"},
        "response": {"status": 200, "body_json": {"ok": true, "data": "<<REDACTED>>"}}
    });
    refused(
        &one_capture(redacted, &[]),
        "source/01-info.json: <<REDACTED>> at response.body_json.data",
    );
    let in_request = serde_json::json!({
        "request": {"method": "GET", "path": "/v1/info", "headers": {"x-identity-auth-chain-0": "<<REDACTED:jwt>>"}},
        "response": {"status": 200, "body_json": {"ok": true}}
    });
    refused(
        &one_capture(in_request, &[]),
        "source/01-info.json: <<REDACTED>> at request.headers.x-identity-auth-chain-0",
    );
}

#[test]
fn a_deliberate_gap_is_stated_once_and_referenced_by_name() {
    let mut op = operation("known_gap", serde_json::json!([]));
    op["gaps"] = serde_json::json!(["deliberate:unknown"]);
    let mut ledger = ledger_value(serde_json::json!([op]));
    ledger.deliberate_gaps.insert(
        "unused".into(),
        DeliberateGap {
            text: "a decision nothing cites".into(),
            captures: vec![],
            anchors: vec![],
        },
    );
    let routes = BTreeSet::from([INFO.to_string()]);
    let errors = ledger_errors(&ledger, &routes, &BTreeSet::new(), &BTreeMap::new());
    refused(
        &errors,
        "api_http:GET:/v1/info: no deliberate gap named unknown",
    );
    refused(&errors, "deliberate gap unused: no operation references it");
}

#[test]
fn a_capture_replays_its_recorded_headers_only_when_it_has_some() {
    let mode = |auth: Value| {
        one_capture(
            serde_json::json!({
                "request": {"method": "GET", "path": "/v1/info", "headers": {}},
                "response": {"status": 200, "body_json": {"ok": true}},
                "auth": auth
            }),
            &[],
        )
    };
    refused(
        &mode("fresh".into()),
        "source/01-info.json: auth must be \"recorded\", not \"fresh\"",
    );
    refused(
        &mode("recorded".into()),
        "source/01-info.json: auth is recorded but the request carries no x-identity- header",
    );
}

#[test]
fn a_capture_signature_defect_is_one_the_replay_reproduces() {
    let chain = serde_json::json!({"x-identity-auth-chain-0": r#"{"type":"SIGNER","payload":"0xc1aba94a79ca07d9d15a62ff5c9b382777e0c70e","signature":""}"#});
    let defect = |headers: Value, extra: Value| {
        let mut fx = serde_json::json!({
            "request": {"method": "GET", "path": "/v1/info", "headers": headers},
            "response": {"status": 401, "body_json": {"ok": false}}
        });
        for (key, value) in extra.as_object().unwrap() {
            fx[key] = value.clone();
        }
        one_capture(fx, &[])
    };
    refused(
        &defect(chain.clone(), serde_json::json!({"sign": {"skew": 1}})),
        "source/01-info.json: sign: unknown field `skew`",
    );
    refused(
        &defect(
            chain.clone(),
            serde_json::json!({"sign": {"offset": 1}, "auth": "recorded"}),
        ),
        "source/01-info.json: a capture is re-signed with its defect or replayed as recorded, not both",
    );
    refused(
        &defect(
            serde_json::json!({}),
            serde_json::json!({"sign": {"offset": 1}}),
        ),
        "source/01-info.json: sign names no signer",
    );
    assert_eq!(
        defect(
            chain,
            serde_json::json!({"sign": {"untimestamped": true, "withHeaders": true}})
        ),
        Vec::<String>::new()
    );
}

fn defect_errors(sign: Value) -> Vec<String> {
    one_capture(
        serde_json::json!({
            "request": {"method": "GET", "path": "/v1/info", "headers": {
                "x-identity-auth-chain-0": r#"{"type":"SIGNER","payload":"0xc1aba94a79ca07d9d15a62ff5c9b382777e0c70e","signature":""}"#
            }},
            "response": {"status": 401, "body_json": {"ok": false}},
            "sign": sign
        }),
        &[],
    )
}

#[test]
fn a_sign_without_a_defect_is_refused() {
    refused(
        &defect_errors(serde_json::json!({})),
        "source/01-info.json: sign names no defect",
    );
    refused(
        &defect_errors(serde_json::json!({"metadata": "{}"})),
        "source/01-info.json: sign names no defect",
    );
}

#[test]
fn a_zero_offset_is_refused() {
    refused(
        &defect_errors(serde_json::json!({"offset": 0, "metadata": "{}"})),
        "source/01-info.json: sign offset 0 is no defect",
    );
}

#[test]
fn with_headers_without_untimestamped_is_refused() {
    refused(
        &defect_errors(
            serde_json::json!({"withHeaders": true, "path": "/v1/other", "metadata": "{}"}),
        ),
        "source/01-info.json: sign withHeaders needs untimestamped",
    );
}

#[test]
fn untimestamped_with_a_timestamped_defect_is_refused() {
    for field in [
        serde_json::json!({"path": "/v1/other"}),
        serde_json::json!({"offset": 1}),
        serde_json::json!({"metadata": "{}"}),
        serde_json::json!({"sentMetadata": "{}"}),
    ] {
        let mut sign = serde_json::json!({"untimestamped": true});
        sign.as_object_mut()
            .unwrap()
            .extend(field.as_object().unwrap().clone());
        refused(
            &defect_errors(sign),
            "source/01-info.json: sign untimestamped takes no path, offset, metadata or sentMetadata",
        );
    }
}

#[test]
fn a_capture_that_matches_two_operations_is_refused() {
    let by_id = "api_http:GET:/v1/collections/:id";
    let by_address = "api_http:GET:/v1/:address/items";
    let path = fixture("source/01-collection.json");
    let mut cited = operation(
        "reviewed",
        serde_json::json!([{"path": path, "scope": "s", "replay": []}]),
    );
    cited["id"] = by_id.into();
    let mut other = operation("not_reviewed", serde_json::json!([]));
    other["id"] = by_address.into();
    let ledger = ledger_value(serde_json::json!([cited, other]));
    let routes = BTreeSet::from([by_id.to_string(), by_address.to_string()]);
    let files = BTreeSet::from([path.clone()]);
    let fixtures = BTreeMap::from([(
        path,
        capture(
            "GET",
            "/v1/collections/items",
            serde_json::json!({"ok": true}),
            &[],
        ),
    )]);
    refused(
        &ledger_errors(&ledger, &routes, &files, &fixtures),
        "source/01-collection.json requests GET /v1/collections/items, which matches several operations: api_http:GET:/v1/:address/items, api_http:GET:/v1/collections/:id",
    );
}

#[test]
fn a_volatile_value_must_be_a_timestamp_and_keep_recorded_equalities() {
    let raw = serde_json::json!({
        "volatile_paths": ["data.created_at", "data.updated_at"],
        "volatile_equal": [["data.created_at", "data.updated_at"]]
    });
    let candidate = |body: Value| volatile_notes(&raw, &serde_json::to_vec(&body).unwrap());
    let same = "2026-11-18T00:00:00.000Z";
    assert!(
        candidate(serde_json::json!({"data": {"created_at": same, "updated_at": same}})).is_empty()
    );
    assert!(candidate(serde_json::json!({"data": {"created_at": 1, "updated_at": 1}})).is_empty());
    assert_eq!(
        candidate(
            serde_json::json!({"data": {"created_at": {"nested": true}, "updated_at": same}})
        ),
        vec![
            "data.created_at volatile value is not a timestamp".to_string(),
            "data.created_at and data.updated_at differ".to_string()
        ]
    );
    assert_eq!(
        candidate(
            serde_json::json!({"data": {"created_at": same, "updated_at": "2026-11-18T00:00:01.000Z"}})
        ),
        vec!["data.created_at and data.updated_at differ".to_string()]
    );
}

#[test]
fn a_seed_field_the_replay_cannot_apply_is_refused() {
    let collection = r#"{"id": "c011ec71-0000-4000-8000-000000000001", "name": "n", "owner": "owner",
        "salt": "0x00", "created_at": "2025-12-01T00:00:00Z", "updated_at": "2025-12-01T00:00:00Z""#;
    let committee =
        format!(r#"{{"wallets": {{}}, "committee": ["owner"], "collections": [{collection}}}]}}"#);
    let error = parse_seed(&committee).err().unwrap_or_default();
    assert!(error.contains("committee"), "{error:?}");
    let locked = format!(r#"{{"wallets": {{}}, "collections": [{collection}, "lock": null}}]}}"#);
    let error = parse_seed(&locked).err().unwrap_or_default();
    assert!(error.contains("lock"), "{error:?}");
}

struct Wallet {
    root: PrivateKeySigner,
    ephemeral: PrivateKeySigner,
}

impl Wallet {
    fn derived(role: &str) -> Self {
        let key = |label: String| {
            PrivateKeySigner::from_bytes(&keccak256(
                format!("dcl-one builder conformance {label}").as_bytes(),
            ))
            .unwrap()
        };
        Self {
            root: key(role.to_string()),
            ephemeral: key(format!("{role} ephemeral")),
        }
    }

    fn address(&self) -> String {
        format!("{:#x}", self.root.address())
    }

    async fn chain(&self, payload: &str) -> HeaderMap {
        let ephemeral_payload = format!(
            "Decentraland Login\nEphemeral address: {:#x}\nExpiration: 2099-01-01T00:00:00.000Z",
            self.ephemeral.address()
        );
        let chain = [
            AuthLink {
                link_type: AuthLinkType::SIGNER,
                payload: self.address(),
                signature: None,
            },
            AuthLink {
                link_type: AuthLinkType::EcdsaEphemeral,
                signature: Some(
                    self.root
                        .sign_message(ephemeral_payload.as_bytes())
                        .await
                        .unwrap()
                        .to_string(),
                ),
                payload: ephemeral_payload,
            },
            AuthLink {
                link_type: AuthLinkType::EcdsaSignedEntity,
                signature: Some(
                    self.ephemeral
                        .sign_message(payload.as_bytes())
                        .await
                        .unwrap()
                        .to_string(),
                ),
                payload: payload.to_string(),
            },
        ];
        let mut headers = HeaderMap::new();
        for (i, link) in chain.iter().enumerate() {
            headers.insert(
                HeaderName::from_bytes(format!("{AUTH_CHAIN_HEADER_PREFIX}{i}").as_bytes())
                    .unwrap(),
                HeaderValue::from_str(&serde_json::to_string(link).unwrap()).unwrap(),
            );
        }
        headers
    }
}

fn defect_problems(defect: &Defect) -> Vec<&'static str> {
    let mut problems = Vec::new();
    let timestamped = defect.offset.is_some()
        || defect.path.is_some()
        || defect.metadata.is_some()
        || defect.sent_metadata.is_some();
    if !defect.untimestamped
        && defect.offset.is_none_or(|offset| offset == 0)
        && defect.path.is_none()
        && defect.method.is_none()
        && defect.sent_metadata.is_none()
    {
        problems.push(if defect.offset == Some(0) {
            "offset 0 is no defect"
        } else {
            "names no defect"
        });
    }
    if defect.with_headers && !defect.untimestamped {
        problems.push("withHeaders needs untimestamped");
    }
    if defect.untimestamped && timestamped {
        problems.push("untimestamped takes no path, offset, metadata or sentMetadata");
    }
    problems
}

fn signer_of(raw: &Value) -> Option<String> {
    let first = raw["request"]["headers"][format!("{AUTH_CHAIN_HEADER_PREFIX}0")].as_str()?;
    let link: Value = serde_json::from_str(first).ok()?;
    link["payload"].as_str().map(str::to_lowercase)
}

async fn fresh_signature(
    raw: &Value,
    fx: &Fixture,
    wallets: &BTreeMap<String, Wallet>,
    prefix: &str,
) -> HeaderMap {
    let Some(signer) = signer_of(raw) else {
        return HeaderMap::new();
    };
    let wallet = wallets
        .get(&signer)
        .unwrap_or_else(|| panic!("{signer} is not a seed.json wallet"));
    let recorded = fx
        .request
        .headers
        .get(AUTH_METADATA_HEADER)
        .map_or("{}", String::as_str);
    let defect: Defect = raw
        .get("sign")
        .map(|sign| serde_json::from_value(sign.clone()).unwrap())
        .unwrap_or_default();
    let method = defect.method.as_deref().unwrap_or(&fx.request.method);
    let now = chrono::Utc::now().timestamp_millis();
    if defect.untimestamped {
        let route = fx
            .request
            .path
            .strip_prefix("/v1")
            .unwrap_or(&fx.request.path);
        let mut headers = wallet
            .chain(&format!("{method}:{route}").to_lowercase())
            .await;
        if defect.with_headers {
            headers.insert(AUTH_TIMESTAMP_HEADER, HeaderValue::from(now));
            headers.insert(AUTH_METADATA_HEADER, HeaderValue::from_static("{}"));
        }
        return headers;
    }
    let path = format!(
        "{prefix}{}",
        defect.path.as_deref().unwrap_or(&fx.request.path)
    );
    let signed = defect.metadata.as_deref().unwrap_or(recorded);
    let timestamp = (now + defect.offset.unwrap_or(0)).to_string();
    let mut headers = wallet
        .chain(&build_payload_v6(method, &path, &timestamp, signed))
        .await;
    headers.insert(
        AUTH_TIMESTAMP_HEADER,
        HeaderValue::from_str(&timestamp).unwrap(),
    );
    let sent = defect.sent_metadata.as_deref().unwrap_or(signed);
    headers.insert(AUTH_METADATA_HEADER, HeaderValue::from_str(sent).unwrap());
    headers
}

fn roles(seed: &Seed, roles: &[String]) -> Vec<String> {
    roles
        .iter()
        .map(|role| seed.wallets[role].clone())
        .collect()
}

/// The shop's third-party registry index holding `seed.registry`, each
/// provider as the capture's fake serves it, and `seed.receipts`, at a
/// current publication.
async fn registry_index(pool: &sqlx::PgPool, seed: &Seed) {
    const VERSION: &str = "INSERT INTO shop.third_party_registry_event \
           (block_number, log_index, block_time, provider_id, state, receipt) \
         VALUES (1, (SELECT count(*) FROM shop.third_party_registry_event), 0, $1, $2, $3)";
    for p in &seed.registry {
        let managers: Vec<String> = roles(seed, &p.managers)
            .iter()
            .map(|m| m.to_ascii_lowercase())
            .collect();
        let state = serde_json::json!({
            "id": p.id,
            "approved": p.approved.unwrap_or(true),
            "root": null,
            "resolver": null,
            "raw_metadata": "",
            "metadata": { "name": p.name, "description": "", "contracts": [] },
            "managers": managers,
            "programmatic": p.programmatic,
            "max_items": p.max_items.as_deref().unwrap_or("100"),
            "consumed_slots": "0",
        });
        for r in seed.receipts.iter().filter(|r| r.provider == p.id) {
            let hash = format!(
                "0x{}",
                hex::encode(catalyst_builder::cheques::digest(
                    &catalyst_builder::network::POLYGON,
                    &r.provider,
                    r.qty,
                    r.salt.parse().expect("seed.json receipt salt"),
                ))
            );
            sqlx::query(VERSION)
                .bind(&p.id)
                .bind(&state)
                .bind(serde_json::json!({
                    "hash": hash, "quantity": r.qty.to_string(),
                    "signer": seed.wallets[&r.signer], "curator": "",
                }))
                .execute(pool)
                .await
                .unwrap();
        }
        sqlx::query(VERSION)
            .bind(&p.id)
            .bind(&state)
            .bind(None::<Value>)
            .execute(pool)
            .await
            .unwrap();
    }
    for r in &seed.receipts {
        assert!(
            seed.registry.iter().any(|p| p.id == r.provider),
            "seed.json receipt of {}, which the registry does not list",
            r.provider
        );
    }
    sqlx::query(
        "INSERT INTO shop.chain_cursor \
           (indexer, network, start_block, last_block, full_history, folded_block, folded_at, chain_id) \
         VALUES ($1, 'matic', 0, 1, true, 1, timezone('UTC', now()), 137)",
    )
    .bind(catalyst::chain_index::third_party_index::INDEXER)
    .execute(pool)
    .await
    .unwrap();
}

/// A loopback Polygon collections subgraph that answers `seed.chain`'s
/// `rarities` and its `committee` as `accounts` (ordered by id, paged by
/// `first` and `skip`); returns its URL.
async fn collections_subgraph(seed: &Seed) -> String {
    let rarities: Vec<Value> = seed
        .chain
        .rarities
        .iter()
        .map(|r| serde_json::json!({ "id": r.name, "name": r.name, "price": r.price, "maxSupply": r.max_supply }))
        .collect();
    let listed = serde_json::json!({ "data": { "rarities": rarities } });
    let mut committee: Vec<String> = roles(seed, &seed.chain.committee)
        .iter()
        .map(|m| m.to_ascii_lowercase())
        .collect();
    committee.sort();
    let app = Router::new().route(
        "/graph/collections",
        axum::routing::post(move |axum::Json(body): axum::Json<Value>| {
            let (listed, committee) = (listed.clone(), committee.clone());
            async move {
                if !body["query"]
                    .as_str()
                    .is_some_and(|q| q.contains("accounts"))
                {
                    return axum::Json(listed);
                }
                let skip = body["variables"]["skip"].as_u64().unwrap_or(0) as usize;
                let first = body["variables"]["first"].as_u64().unwrap_or(100) as usize;
                let page: Vec<Value> = committee
                    .into_iter()
                    .skip(skip)
                    .take(first)
                    .map(|m| serde_json::json!({ "id": m, "address": m }))
                    .collect();
                axum::Json(serde_json::json!({ "data": { "accounts": page } }))
            }
        }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    format!("http://{addr}/graph/collections")
}

alloy_sol_types::sol! {
    struct OracleRarity { string name; uint256 maxSupply; uint256 price; }
    function getRarityByName(string rarity) external view returns (OracleRarity);
    function itemsCount() external view returns (uint256);
}

/// A loopback Polygon JSON-RPC that answers, the way the capture's fake
/// does: `eth_chainId` Polygon's, `eth_getCode` code only at a `seed.chain`
/// collection's contract, a collection contract's `itemsCount()` with its
/// `seed.chain` items, and `getRarityByName` with `seed.chain`'s MANA prices.
async fn oracle(seed: &Seed) -> String {
    use alloy_sol_types::SolCall;
    let rarities = seed.chain.rarities.clone();
    let deployed: BTreeMap<String, u64> = seed
        .chain
        .collections
        .iter()
        .map(|c| {
            let items = seed
                .chain
                .items
                .iter()
                .filter(|i| i.contract_address == c.contract_address)
                .count();
            (c.contract_address.to_ascii_lowercase(), items as u64)
        })
        .collect();
    let app = Router::new().route(
        "/rpc",
        axum::routing::post(move |axum::Json(body): axum::Json<Value>| {
            let (rarities, deployed) = (rarities.clone(), deployed.clone());
            async move {
                let reply = |result: Value| {
                    axum::Json(serde_json::json!({ "jsonrpc": "2.0", "id": body["id"].clone(), "result": result }))
                };
                let at = |value: &Value| value.as_str().map(str::to_ascii_lowercase).and_then(|a| deployed.get(&a).copied());
                match body["method"].as_str().unwrap_or_default() {
                    "eth_chainId" => return reply("0x89".into()),
                    "eth_getCode" => {
                        return reply(if at(&body["params"][0]).is_some() { "0x6080" } else { "0x" }.into());
                    }
                    "eth_call"
                        if body["params"][0]["data"].as_str()
                            == Some(alloy_primitives::hex::encode_prefixed(itemsCountCall::SELECTOR).as_str()) =>
                    {
                        return reply(match at(&body["params"][0]["to"]) {
                            Some(items) => format!("0x{items:064x}").into(),
                            None => "0x".into(),
                        });
                    }
                    _ => {}
                }
                let call = alloy_primitives::hex::decode(body["params"][0]["data"].as_str().unwrap_or_default())
                    .ok()
                    .and_then(|bytes| getRarityByNameCall::abi_decode(&bytes).ok());
                let answer = call
                    .and_then(|call| rarities.into_iter().find(|r| r.name == call.rarity))
                    .map(|r| {
                        serde_json::json!({ "result": alloy_primitives::hex::encode_prefixed(
                            getRarityByNameCall::abi_encode_returns(&OracleRarity {
                                name: r.name,
                                maxSupply: r.max_supply.parse().unwrap(),
                                price: r.mana_price.parse().unwrap(),
                            }),
                        ) })
                    })
                    .unwrap_or_else(|| serde_json::json!({ "error": { "code": 3, "message": "execution reverted" } }));
                let mut answer = answer;
                answer["jsonrpc"] = "2.0".into();
                answer["id"] = body["id"].clone();
                axum::Json(answer)
            }
        }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    format!("http://{addr}/rpc")
}

/// The seed's providers: a registry provider names its managers (unless it
/// is programmatic), a virtual third party its name and managers, and only a
/// registry provider says whether it is approved.
fn check_providers(seed: &Seed) {
    for registered in &seed.registry {
        assert!(
            registered.programmatic || !registered.managers.is_empty(),
            "seed.json registry provider {} has no manager",
            registered.id
        );
    }
    for provider in &seed.third_parties {
        assert!(
            !provider.name.is_empty()
                && !provider.managers.is_empty()
                && provider.approved.is_none(),
            "seed.json virtual third party {}",
            provider.id
        );
    }
}

async fn seed(pool: &sqlx::PgPool, seed: &Seed) {
    for e in &seed.entities {
        let pointer = e.pointer.to_lowercase();
        sqlx::query(
            "INSERT INTO content.deployments (deployer_address, version, entity_type, entity_id, entity_metadata, \
               entity_timestamp, entity_pointers, local_timestamp, auth_chain) \
             VALUES ('0x0000000000000000000000000000000000000000', 'v3', $1, $2, $3::json, '2026-01-01', ARRAY[$4], \
               '2026-01-01', '[]'::json)",
        )
        .bind(&e.entity_type)
        .bind(&e.entity_id)
        .bind(serde_json::json!({ "v": e.metadata }))
        .bind(&pointer)
        .execute(pool)
        .await
        .unwrap_or_else(|err| panic!("seed.json entity {}: {err}", e.entity_id));
        sqlx::query(
            "INSERT INTO content.active_pointers (pointer, entity_id, entity_type) VALUES ($1, $2, $3)",
        )
        .bind(&pointer)
        .bind(&e.entity_id)
        .bind(&e.entity_type)
        .execute(pool)
        .await
        .unwrap_or_else(|err| panic!("seed.json entity {} pointer: {err}", e.entity_id));
    }
    check_providers(seed);
    for provider in &seed.third_parties {
        sqlx::query(
            "INSERT INTO builder.virtual_third_parties (id, managers, raw_metadata) VALUES ($1, $2, $3)",
        )
        .bind(&provider.id)
        .bind(roles(seed, &provider.managers))
        .bind(format!("tp:1:{}:", provider.name))
        .execute(pool)
        .await
        .unwrap_or_else(|e| panic!("seed.json virtual third party {}: {e}", provider.id));
    }
    for c in &seed.collections {
        let urn = c
            .third_party_id
            .as_ref()
            .zip(c.urn_suffix.as_ref())
            .map(|(provider, suffix)| format!("{provider}:{suffix}"));
        sqlx::query(
            "INSERT INTO builder.collections \
         (id, name, eth_address, salt, contract_address, third_party_id, urn_suffix, \
          linked_contract_address, linked_contract_network, forum_link, forum_id, created_at, updated_at) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13)",
        )
        .bind(c.id)
        .bind(&c.name)
        .bind(&seed.wallets[&c.owner])
        .bind(&c.salt)
        .bind(&c.contract_address)
        .bind(&c.third_party_id)
        .bind(urn)
        .bind(&c.linked_contract_address)
        .bind(&c.linked_contract_network)
        .bind(&c.forum_link)
        .bind(c.forum_id)
        .bind(c.created_at.naive_utc())
        .bind(c.updated_at.naive_utc())
        .execute(pool)
        .await
        .unwrap_or_else(|e| panic!("seed.json collection {}: {e}", c.id));
    }
    for i in &seed.items {
        let urn = i.urn_suffix.as_ref().map(|suffix| {
            let c = seed
                .collections
                .iter()
                .find(|c| Some(c.id) == i.collection_id)
                .unwrap_or_else(|| panic!("seed.json item {} has a URN but no collection", i.id));
            format!(
                "{}:{}:{suffix}",
                c.third_party_id.as_deref().unwrap_or_default(),
                c.urn_suffix.as_deref().unwrap_or_default()
            )
        });
        sqlx::query(
            "INSERT INTO builder.items (id, name, eth_address, collection_id, blockchain_item_id, type, thumbnail, rarity, urn_suffix, data, metrics, created_at, updated_at, local_content_hash) \
             VALUES ($1, $2, $3, $4, $5, 'wearable', $6, $7, $8, $9::text::json, $10::text::json, $11, $12, $13)",
        )
        .bind(i.id)
        .bind(&i.name)
        .bind(&seed.wallets[&i.owner])
        .bind(i.collection_id)
        .bind(&i.blockchain_item_id)
        .bind(&i.thumbnail)
        .bind(&i.rarity)
        .bind(urn)
        .bind(i.data.to_string())
        .bind(i.metrics.to_string())
        .bind(i.created_at.naive_utc())
        .bind(i.updated_at.naive_utc())
        .bind(&i.local_content_hash)
        .execute(pool)
        .await
        .unwrap_or_else(|e| panic!("seed.json item {}: {e}", i.id));
        for (position, (file, hash)) in i.contents.iter().enumerate() {
            sqlx::query("INSERT INTO builder.item_contents (item_id, file, hash, position, named_by) VALUES ($1, $2, $3, $4, $5)")
                .bind(i.id)
                .bind(file)
                .bind(hash)
                .bind(position as i32)
                .bind(&seed.wallets[&i.owner])
                .execute(pool)
                .await
                .unwrap_or_else(|e| panic!("seed.json item {} contents: {e}", i.id));
        }
    }
    for k in &seed.collection_curations {
        sqlx::query(
            "INSERT INTO builder.collection_curations (id, collection_id, status, assignee, created_at, updated_at) \
             VALUES ($1, $2, $3, $4, $5, $6)",
        )
        .bind(k.id)
        .bind(k.collection_id)
        .bind(&k.status)
        .bind(k.assignee.as_ref().map(|role| &seed.wallets[role]))
        .bind(k.created_at.naive_utc())
        .bind(k.updated_at.naive_utc())
        .execute(pool)
        .await
        .unwrap_or_else(|e| panic!("seed.json collection curation {}: {e}", k.id));
    }
    for k in &seed.item_curations {
        sqlx::query(
            "INSERT INTO builder.item_curations (id, item_id, status, content_hash, is_mapping_complete, created_at, updated_at) \
             VALUES ($1, $2, $3, $4, $5, $6, $7)",
        )
        .bind(k.id)
        .bind(k.item_id)
        .bind(&k.status)
        .bind(&k.content_hash)
        .bind(k.is_mapping_complete)
        .bind(k.created_at.naive_utc())
        .bind(k.updated_at.naive_utc())
        .execute(pool)
        .await
        .unwrap_or_else(|e| panic!("seed.json item curation {}: {e}", k.id));
    }
    for c in &seed.chain.collections {
        sqlx::query(
            "INSERT INTO shop.collections (network, contract_address, chain_id, name, creator, owner, urn, \
               is_completed, is_approved, is_editable, managers, minters, created_at, updated_at, reviewed_at, mirrored_at) \
             VALUES ('matic', $1, 137, $2, $3, $4, 'urn:decentraland:matic:collections-v2:' || $1, true, $5, true, \
               $6, $7, $8, $9, $10, timezone('UTC', now()))",
        )
        .bind(&c.contract_address)
        .bind(&c.name)
        .bind(&seed.wallets[&c.creator])
        .bind(&seed.wallets[&c.owner])
        .bind(c.is_approved)
        .bind(roles(seed, &c.managers))
        .bind(roles(seed, &c.minters))
        .bind(c.created_at)
        .bind(c.updated_at)
        .bind(c.reviewed_at)
        .execute(pool)
        .await
        .unwrap_or_else(|e| panic!("seed.json chain collection {}: {e}", c.contract_address));
    }
    for i in &seed.chain.items {
        let creator = seed
            .chain
            .collections
            .iter()
            .find(|c| c.contract_address == i.contract_address)
            .map(|c| &seed.wallets[&c.creator])
            .unwrap_or_else(|| {
                panic!(
                    "seed.json chain item of an unknown collection {}",
                    i.contract_address
                )
            });
        sqlx::query(
            "INSERT INTO shop.items (network, contract_address, item_id, chain_id, item_type, urn, creator, beneficiary, \
               rarity, total_supply, max_supply, available, price, name, category, body_shapes, created_at, updated_at, \
               reviewed_at, mirrored_at, content_hash) \
             VALUES ('matic', $1, $2::numeric, 137, 'wearable_v2', \
               'urn:decentraland:matic:collections-v2:' || $1 || ':' || $2, $3, $4, $5, $6::numeric, 100, \
               100 - $6::numeric, $7::numeric, $8, $9, ARRAY['BaseMale'], 0, 0, 0, timezone('UTC', now()), $10)",
        )
        .bind(&i.contract_address)
        .bind(&i.blockchain_id)
        .bind(creator)
        .bind(&seed.wallets[&i.beneficiary])
        .bind(&i.rarity)
        .bind(&i.total_supply)
        .bind(&i.price)
        .bind(&i.name)
        .bind(&i.category)
        .bind(&i.content_hash)
        .execute(pool)
        .await
        .unwrap_or_else(|e| panic!("seed.json chain item {}: {e}", i.contract_address));
    }
    sqlx::query(
        "INSERT INTO shop.mirror_state (mirror, network, source_url, last_complete_pass_at, pass_started_at) \
         VALUES ('collections', 'matic', 'https://subgraph.example', timezone('UTC', now()), timezone('UTC', now())), \
                ('items', 'matic', 'https://subgraph.example', timezone('UTC', now()), timezone('UTC', now()))",
    )
    .execute(pool)
    .await
    .unwrap();
}

struct TempDir(PathBuf);

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn mounted(state: Arc<State>) -> Router {
    Router::new()
        .merge(root_router(state.clone()))
        .nest("/builder-api", router(state))
}

struct Replay {
    scratch: db::ScratchDb,
    app: Router,
    state: Arc<State>,
    slack: FakeSlack,
    forum: FakeForum,
    newsletter: FakeNewsletter,
    wallets: BTreeMap<String, Wallet>,
    validator_calls: std::sync::atomic::AtomicI64,
    claims: std::sync::Mutex<BTreeMap<String, (Value, Value)>>,
    _dir: TempDir,
}

/// The secret the capture configures upstream's validator callback with,
/// and the one a `validator_sign` of `{"secret": "wrong"}` signs with.
const VALIDATOR_SECRET: &str = "example-validator-secret-0123456789abcdef";
const WRONG_VALIDATOR_SECRET: &str = "example-wrong-secret-0123456789abcdefghij";

/// A capture's `validator_sign`: the defect its signature carries.
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct ValidatorSign {
    #[serde(default)]
    offset: i64,
    secret: Option<String>,
}

impl Replay {
    /// The validator's headers for `method` at `path` with `body`, signed
    /// with the node's scheme at the capture's clock (each call a
    /// millisecond before the last, so none repeats) plus `sign`'s defect.
    fn validator_headers(
        &self,
        method: &str,
        path: &str,
        body: &[u8],
        sign: &ValidatorSign,
    ) -> HeaderMap {
        use catalyst::shared::validator_signature::{
            SIGNATURE_HEADER, TIMESTAMP_HEADER, sign as signed,
        };
        let n = self
            .validator_calls
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let timestamp = (capture_clock().timestamp_millis() + sign.offset - n).to_string();
        let secret = match sign.secret.as_deref() {
            Some(_) => WRONG_VALIDATOR_SECRET,
            None => VALIDATOR_SECRET,
        };
        let mut headers = HeaderMap::new();
        headers.insert(TIMESTAMP_HEADER, HeaderValue::from_str(&timestamp).unwrap());
        headers.insert(
            SIGNATURE_HEADER,
            HeaderValue::from_str(&signed(secret.as_bytes(), &timestamp, method, path, body))
                .unwrap(),
        );
        headers
    }

    /// What the node queues for the validator after a capture: each request
    /// it recorded, claimed through its route, as upstream's manifest
    /// without the `validationId`.
    async fn claimed(&self) -> Vec<Value> {
        let open: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM builder.validation_requests WHERE settled_at IS NULL AND claims = 0",
        )
        .fetch_one(&self.scratch.pool)
        .await
        .unwrap();
        let mut claimed = Vec::new();
        for _ in 0..open {
            let path = "/v1/validator/claims";
            let mut request = axum::http::Request::builder().method("POST").uri(path);
            for (name, value) in
                &self.validator_headers("POST", path, b"{}", &ValidatorSign::default())
            {
                request = request.header(name, value);
            }
            let response = tower::ServiceExt::oneshot(
                self.app.clone(),
                request.body(axum::body::Body::from("{}")).unwrap(),
            )
            .await
            .unwrap();
            let status = response.status();
            let bytes = axum::body::to_bytes(response.into_body(), 8 << 20)
                .await
                .unwrap();
            let body: Value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
            assert_eq!(status, 200, "a recorded request is claimed: {body}");
            self.claims.lock().unwrap().insert(
                body["data"]["collectionId"]
                    .as_str()
                    .unwrap_or_default()
                    .to_string(),
                (
                    body["data"]["validationId"].clone(),
                    body["data"]["claimId"].clone(),
                ),
            );
            claimed.push(serde_json::json!({
                "collectionId": body["data"]["collectionId"],
                "items": body["data"]["items"],
            }));
        }
        claimed
    }
}

/// The bot token the capture configures upstream's Slack client with.
const SLACK_TOKEN: &str = "xoxb-example123";
/// The channel the capture configures upstream's curation notices for.
const SLACK_CHANNEL: &str = "curation-example";

/// A loopback Slack Web API that takes `chat.postMessage` from the bot and
/// records each message's JSON body.
#[derive(Clone, Default)]
struct FakeSlack(Arc<std::sync::Mutex<Vec<Value>>>);

#[derive(Clone, Default)]
struct FakeNewsletter(Arc<std::sync::Mutex<Vec<Value>>>);

const NEWSLETTER_KEY: &str = "example-newsletter-key";
const NEWSLETTER_PUBLICATION: &str = "pub-example";
const NEWSLETTER_FAILING_EMAIL: &str = "fail@example.com";
const CAPTURE_CORS_ORIGIN: &str =
    r"regex:^https://(.{1,50}\.)?decentraland\.example$;https://partner.example";

impl FakeNewsletter {
    async fn start(&self) -> String {
        let received = self.0.clone();
        let path = format!("/v2/publications/{NEWSLETTER_PUBLICATION}/subscriptions");
        let recorded = path.clone();
        let app = Router::new().route(
            &path,
            axum::routing::post(move |headers: HeaderMap, body: axum::body::Bytes| {
                let received = received.clone();
                let path = recorded.clone();
                async move {
                    let header = |name: &str| {
                        headers
                            .get(name)
                            .and_then(|v| v.to_str().ok())
                            .map_or(Value::Null, |v| Value::from(v.to_string()))
                    };
                    let body: Value = serde_json::from_slice(&body).unwrap_or(Value::Null);
                    let failing = body["email"] == NEWSLETTER_FAILING_EMAIL;
                    received.lock().unwrap().push(serde_json::json!({
                        "method": "POST", "path": path, "authorization": header("authorization"),
                        "accept": header("accept"), "content_type": header("content-type"), "body": body,
                    }));
                    if failing {
                        axum::http::StatusCode::INTERNAL_SERVER_ERROR
                    } else {
                        axum::http::StatusCode::CREATED
                    }
                }
            }),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        format!("http://{addr}/v2")
    }

    fn take(&self) -> Vec<Value> {
        std::mem::take(&mut *self.0.lock().unwrap())
    }
}

impl FakeSlack {
    async fn start(&self) -> String {
        let posted = self.0.clone();
        let app = Router::new().route(
            "/api/chat.postMessage",
            axum::routing::post(
                move |headers: HeaderMap, axum::Json(body): axum::Json<Value>| {
                    let posted = posted.clone();
                    async move {
                        let bearer = headers.get("authorization").and_then(|v| v.to_str().ok());
                        if bearer != Some(&format!("Bearer {SLACK_TOKEN}")) {
                            return axum::Json(
                                serde_json::json!({ "ok": false, "error": "invalid_auth" }),
                            );
                        }
                        posted.lock().unwrap().push(body.clone());
                        axum::Json(serde_json::json!({
                            "ok": true, "channel": body["channel"], "ts": "1767225600.000100",
                            "message": { "type": "message", "text": body["text"] },
                        }))
                    }
                },
            ),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        format!("http://{addr}/api")
    }

    fn take(&self) -> Vec<Value> {
        std::mem::take(&mut *self.0.lock().unwrap())
    }
}

async fn replay_setup() -> Option<Replay> {
    let seed_data =
        parse_seed(&std::fs::read_to_string(root().join(FIXTURES).join("seed.json")).unwrap())
            .unwrap_or_else(|e| panic!("seed.json: {e}"));
    let scratch = db::scratch_database().await?;
    let mut wallets = BTreeMap::new();
    for (role, address) in &seed_data.wallets {
        let wallet = Wallet::derived(role);
        assert_eq!(&wallet.address(), address, "seed.json wallet {role}");
        wallets.insert(address.clone(), wallet);
    }
    seed(&scratch.pool, &seed_data).await;
    sqlx::query(
        "INSERT INTO explore_api.feature_flags (app, document, fetched_at) \
         VALUES ('builder', '{\"flags\":{\"builder-auto-curation\":true},\"variants\":{}}', timezone('UTC', now()))",
    )
    .execute(&scratch.pool)
    .await
    .unwrap();
    registry_index(&scratch.pool, &seed_data).await;
    let collections_subgraph = collections_subgraph(&seed_data).await;
    let index = catalyst::chain_index::ChainIndex::new(
        Outbound::for_tests(),
        catalyst::chain_index::IndexSettings {
            matic_collections_subgraph_url: Some(collections_subgraph),
            ..catalyst::chain_index::IndexSettings::for_tests()
        },
    );
    index.allow_loopback_for_tests();
    let dir = TempDir(std::env::temp_dir().join(format!(
        "catalyst-builder-conformance-{}",
        uuid::Uuid::new_v4()
    )));
    let storage = Arc::new(Storage::from_config(&dir.0).await.unwrap());
    let slack = FakeSlack::default();
    let slack_api = slack.start().await;
    let forum = FakeForum::default();
    for collection in &seed_data.collections {
        if let (Some(link), Some(post)) = (&collection.forum_link, collection.forum_id) {
            let url = url::Url::parse(link).unwrap();
            let parts: Vec<_> = url.path_segments().unwrap().collect();
            forum.seed(i64::from(post), parts[2].parse().unwrap(), parts[1]);
        }
    }
    let forum_api = forum.start().await;
    let newsletter = FakeNewsletter::default();
    let newsletter_api = newsletter.start().await;
    let mut forum_settings = FakeForum::forum(&forum_api);
    forum_settings.url = "https://forum.example.org".into();
    let mut state = State::new(
        scratch.pool.clone(),
        storage.clone(),
        storage.denylist().clone(),
        Outbound::for_tests(),
        Some(Arc::new(index)),
        None,
        Settings {
            writes_enabled: true,
            polygon_rpc_url: Some(oracle(&seed_data).await),
            slack: Some(catalyst::builder::curation::slack::Slack {
                token: SLACK_TOKEN.into(),
                channel: SLACK_CHANNEL.into(),
                api: slack_api,
            }),
            links: catalyst::builder::curation::slack::Links {
                creators_url: Some("https://creators.example.org".into()),
                builder_url: None,
            },
            forum: Some(forum_settings),
            builder_server_url: "https://builder-api.example.org".into(),
            validator_secret: Some(VALIDATOR_SECRET.into()),
            newsletter: Some(catalyst::builder::NewsletterForward {
                url: newsletter_api,
                publication_id: NEWSLETTER_PUBLICATION.into(),
                api_key: NEWSLETTER_KEY.into(),
            }),
            cors: catalyst::builder::cors::Cors::parse(Some(CAPTURE_CORS_ORIGIN), None).unwrap(),
            ..Settings::default()
        },
    )
    .unwrap();
    state.now = capture_clock;
    state.guarded = Outbound::for_tests();
    state.content_sync.set_own_content_authority(true);
    let state = Arc::new(state);
    Some(Replay {
        scratch,
        app: mounted(state.clone()),
        state,
        slack,
        forum,
        newsletter,
        wallets,
        validator_calls: std::sync::atomic::AtomicI64::new(0),
        claims: std::sync::Mutex::default(),
        _dir: dir,
    })
}

/// The capture's fixed clock, which the node's lock is set and compared by.
fn capture_clock() -> chrono::DateTime<chrono::Utc> {
    chrono::DateTime::from_timestamp_millis(1_767_225_600_000).unwrap()
}

async fn replayed(replay: &Replay, path: &str, raw: &Value, prefix: &str) -> Vec<String> {
    let mut fx: Fixture =
        serde_json::from_value(raw.clone()).unwrap_or_else(|e| panic!("{path}: {e}"));
    let name = Path::new(path).file_stem().unwrap().to_str().unwrap();
    if let Some(collection) = raw.get("validation_of").and_then(Value::as_str) {
        let (validation, claim) = replay
            .claims
            .lock()
            .unwrap()
            .get(collection)
            .cloned()
            .unwrap_or_else(|| panic!("{path}: the node holds no claim of {collection}"));
        let body = fx.request.body.as_mut().expect("a verdict has a body");
        body["validationId"] = validation;
        body["claimId"] = claim;
    }
    let review_flow = raw.get("review_flow") == Some(&Value::Bool(true));
    sqlx::query(if review_flow {
        "UPDATE explore_api.feature_flags SET document = jsonb_set(document, '{flags,builder-auto-curation-review-flow}', 'true') WHERE app = 'builder'"
    } else {
        "UPDATE explore_api.feature_flags SET document = document #- '{flags,builder-auto-curation-review-flow}' WHERE app = 'builder'"
    })
    .execute(&replay.scratch.pool)
    .await
    .unwrap();
    let auth = if let Some(sign) = raw.get("validator_sign") {
        let sign: ValidatorSign = serde_json::from_value(sign.clone()).unwrap();
        fx.request
            .headers
            .retain(|name, _| !name.starts_with("x-wearable-validator-"));
        let body = fx
            .request
            .body
            .as_ref()
            .map(|body| serde_json::to_vec(body).unwrap())
            .unwrap_or_default();
        replay.validator_headers(
            &fx.request.method,
            &format!("{prefix}{}", fx.request.path),
            &body,
            &sign,
        )
    } else if raw.get("auth").is_some() {
        recorded_auth(raw)
    } else {
        fresh_signature(raw, &fx, &replay.wallets, prefix).await
    };
    let every_key = Volatility::default().with_ignored_fields(&[]);
    let repeat = raw.get("repeat").and_then(Value::as_u64).unwrap_or(0);
    let fresh_budget = || {
        if let Some(signer) = signer_of(raw) {
            replay.state.limits.review_writes.clear(&signer);
            replay.state.limits.review_reads.clear(&signer);
        }
    };
    if repeat > 0 {
        fresh_budget();
    }
    for _ in 0..repeat {
        replay_with(&replay.app, prefix, name, &fx, &auth, &every_key).await;
    }
    let (mut notes, body) = replay_with(&replay.app, prefix, name, &fx, &auth, &every_key).await;
    if repeat > 0 {
        fresh_budget();
    }
    notes.extend(volatile_notes(raw, &body));
    replay.state.notices.settled().await;
    let sent = Value::Array(replay.slack.take());
    let recorded = raw
        .get("slack")
        .cloned()
        .unwrap_or_else(|| Value::Array(vec![]));
    if sent != recorded {
        notes.push(format!("slack: fixture={recorded} candidate={sent}"));
    }
    let subscribed = Value::Array(replay.newsletter.take());
    let recorded = raw
        .get("newsletter")
        .cloned()
        .unwrap_or_else(|| Value::Array(vec![]));
    if subscribed != recorded {
        notes.push(format!(
            "newsletter: fixture={recorded} candidate={subscribed}"
        ));
    }
    let sent = Value::Array(replay.forum.take());
    let recorded = raw
        .get("forum")
        .cloned()
        .unwrap_or_else(|| Value::Array(vec![]));
    notes.extend(
        catalyst_testing::conformance::diff::compare_json(
            "forum", "forum", &recorded, &sent, &every_key,
        )
        .into_iter()
        .map(|difference| difference.to_string()),
    );
    let queued = Value::Array(replay.claimed().await);
    let recorded = raw
        .get("queue")
        .cloned()
        .unwrap_or_else(|| Value::Array(vec![]));
    notes.extend(
        catalyst_testing::conformance::diff::compare_json(
            "queue", "queue", &recorded, &queued, &every_key,
        )
        .into_iter()
        .map(|difference| difference.to_string()),
    );
    notes
}

#[tokio::test]
async fn a_changed_id_fails_the_replay() {
    let path = fixture("source/02-collection-owner.json");
    let mut raw = checked_capture(&path, &ledger().volatile_fields);
    raw["response"]["body_json"]["data"]["id"] = "c011ec71-0000-4000-8000-0000000000ff".into();
    let Some(replay) = replay_setup().await else {
        return;
    };
    let notes = replayed(&replay, &path, &raw, "").await;
    replay.scratch.teardown().await;
    assert!(notes.contains(&"data.id value".to_string()), "{notes:#?}");
}

#[tokio::test]
async fn a_forum_call_missing_from_its_capture_fails_the_replay() {
    let path = fixture("source/447-curation-post-assigned.json");
    let mut raw = checked_capture(&path, &ledger().volatile_fields);
    raw.as_object_mut().unwrap().remove("forum");
    let Some(replay) = replay_setup().await else {
        return;
    };
    let notes = replayed(&replay, &path, &raw, "").await;
    replay.scratch.teardown().await;
    assert!(
        notes.contains(&"forum.length: baseline=0 candidate=2".to_string()),
        "{notes:#?}"
    );
}

#[tokio::test]
async fn a_queued_manifest_unlike_the_nodes_fails_the_replay() {
    let path = fixture("source/276-collection-publish-assigns-ids.json");
    let mut raw = checked_capture(&path, &ledger().volatile_fields);
    raw["queue"][0]["items"][1]["metadata"]["name"] = "Another name".into();
    let Some(replay) = replay_setup().await else {
        return;
    };
    let notes = replayed(&replay, &path, &raw, "").await;
    replay.scratch.teardown().await;
    assert!(
        notes
            .iter()
            .any(|note| note
                .starts_with("queue[0].items[1].metadata.name: baseline=\"Another name\"")),
        "{notes:#?}"
    );
}

#[tokio::test]
async fn the_builder_captures_replay_against_the_mounted_router() {
    let ledger = ledger();
    let expected: BTreeMap<String, (String, Vec<String>)> = ledger
        .operations
        .into_iter()
        .flat_map(|op| {
            let id = op.id;
            op.evidence
                .into_iter()
                .filter_map(move |e| e.replay.map(|notes| (e.path, (id.clone(), notes))))
        })
        .collect();
    let mut paths = captures(&fixture_files());
    paths.extend(expected.keys().cloned());
    let loaded: Vec<(String, Value)> = replay_order(paths)
        .unwrap_or_else(|e| panic!("{e}"))
        .into_iter()
        .map(|path| {
            let raw = checked_capture(&path, &ledger.volatile_fields);
            (path, raw)
        })
        .collect();
    let Some(replay) = replay_setup().await else {
        return;
    };
    let mut mismatches = Vec::new();
    for (path, raw) in loaded {
        let notes = replayed(&replay, &path, &raw, "").await;
        let Some((operation, recorded)) = expected.get(&path) else {
            mismatches.push(format!(
                "{path}: no operation cites it; it replays as {notes:#?}"
            ));
            continue;
        };
        if &notes != recorded {
            mismatches.push(format!(
                "{path} ({operation}): recorded {recorded:#?}, replayed {notes:#?}"
            ));
        }
        if ["GET", "HEAD"].contains(&raw["request"]["method"].as_str().unwrap_or_default()) {
            let prefixed = replayed(&replay, &path, &raw, GATEWAY_PREFIX).await;
            if &prefixed != recorded {
                mismatches.push(format!(
                    "{path} ({operation}) under {GATEWAY_PREFIX}: recorded {recorded:#?}, replayed {prefixed:#?}"
                ));
            }
        }
    }
    replay.scratch.teardown().await;
    assert!(mismatches.is_empty(), "{}", mismatches.join("\n"));
}

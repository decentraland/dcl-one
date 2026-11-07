//! The events-notifier parity fixtures (`tests/fixtures/notifier_parity`)
//! replayed against the Segment webhooks router as `serve` mounts it
//! (`social::segment_webhooks::router` behind `social::writes::guard`, nested
//! under `/events-notifier`): each entry's body is signed as Segment signs it,
//! sent, and its answer and effect (the `social.client_events` row, the
//! onboarding checkpoint) compared; `known_gaps.json` ratchets the recorded
//! differences. Needs CATALYST_TEST_DATABASE_URL; one scratch database for
//! the file, the table truncated and the onboarding state fresh per entry.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use axum::Router;
use axum::body::Body;
use axum::http::{Request, header};
use serde::Deserialize;
use serde_json::{Value, json};
use sqlx::PgPool;
use tower::ServiceExt;

use catalyst::auth::Wallet;
use catalyst::explore::api::onboarding::{CheckpointRequest, OnboardingState};
use catalyst::shared::segment_signature;
use catalyst::social::segment_webhooks::{self, Settings, State};
use catalyst::social::writes;

#[path = "support/db.rs"]
mod db;

const SIGNING_KEY: &[u8] = b"fixture-signing-key";
const PREFIX: &str = "/events-notifier";
const WEB_PROJECT_ID: &str = "ncCetmghaPEGFVVa1n8hKY";

// ── fixtures ───────────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct Entry {
    id: String,
    request: RequestSpec,
    #[serde(default)]
    harness: Harness,
    expected: Expected,
    #[serde(default)]
    effect: Effect,
    #[serde(default)]
    source: Source,
}

#[derive(Deserialize, Default)]
struct Source {
    /// `source` (the notifier's answer) or `accepted` (dcl-one's, `ACCEPTED_ENTRIES`).
    #[serde(default)]
    kind: String,
}

#[derive(Deserialize)]
struct RequestSpec {
    path: String,
    /// The JSON body, or `$name` for `bodies/<name>.json`.
    body: Value,
    /// Dotted paths set before sending; the string `$delete` removes one.
    #[serde(default)]
    patch: BTreeMap<String, Value>,
}

#[derive(Deserialize, Default)]
struct Harness {
    /// `valid` (default), `wrong` (another key) or `none` (no header).
    #[serde(default)]
    sign: Option<String>,
    /// `fresh`: a chain signed now, its owner in the body; `foreign`: the
    /// chain of one wallet under another's address.
    #[serde(default)]
    identity: Option<String>,
    /// `pretty`: the body is sent re-spaced, the signature over its compact form.
    #[serde(default)]
    body_form: Option<String>,
    #[serde(default)]
    force_context_validation: bool,
    #[serde(default)]
    denylist: Vec<String>,
    #[serde(default)]
    expected_realm: Option<String>,
    #[serde(default)]
    expected_realm_url: Option<String>,
    #[serde(default)]
    writes_enabled: Option<bool>,
    #[serde(default)]
    signing_key: Option<bool>,
    #[serde(default)]
    repeat: Option<u32>,
    /// A checkpoint body recorded before the request.
    #[serde(default)]
    seed_checkpoint: Option<Value>,
}

#[derive(Deserialize)]
struct Expected {
    status: u16,
    #[serde(default)]
    body: Option<Value>,
    #[serde(default)]
    body_subset: Option<Value>,
}

#[derive(Deserialize, Default)]
struct Effect {
    #[serde(default)]
    rows: Option<i64>,
    /// A subset of the stored row's columns (`metadata` compared as a subset too).
    #[serde(default)]
    row: Option<Value>,
    /// `{user, checkpoint, recorded: false}` or a subset of the recorded state.
    #[serde(default)]
    checkpoint: Option<Value>,
}

#[derive(Deserialize, Default)]
struct KnownGaps {
    #[serde(default)]
    entries: Vec<KnownGap>,
}

#[derive(Deserialize, serde::Serialize)]
struct KnownGap {
    id: String,
    observed: String,
}

fn fixtures_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/notifier_parity")
}

fn load_entries() -> Vec<Entry> {
    let path = fixtures_dir().join("forwarders.fixture.json");
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let entries: Vec<Entry> = serde_json::from_str(&text)
        .unwrap_or_else(|e| panic!("{}: not a JSON array of entries: {e}", path.display()));
    let mut seen = BTreeSet::new();
    for entry in &entries {
        assert!(
            seen.insert(entry.id.clone()),
            "duplicate fixture id {}",
            entry.id
        );
    }
    entries
}

fn load_known_gaps() -> BTreeMap<String, String> {
    let path = fixtures_dir().join("known_gaps.json");
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let gaps: KnownGaps =
        serde_json::from_str(&text).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    gaps.entries
        .into_iter()
        .map(|g| (g.id, g.observed))
        .collect()
}

/// Entries whose expectation is dcl-one's accepted answer rather than the
/// notifier's (README "Accepted entries"): the notifier's 500s and the events
/// it published broken or unverified. Checked like every other entry, never a
/// known gap: a failure is a regression.
const ACCEPTED_ENTRIES: [&str; 5] = [
    "forward.expired_chain",
    "forward.no_received_at",
    "forward.no_message_id",
    "forward-web.no_traits",
    "forward-web.no_page",
];

fn flag(name: &str) -> bool {
    std::env::var(name).ok().as_deref() == Some("1")
}

// ── the request ────────────────────────────────────────────────────────────

struct Identity {
    address: String,
    chain: String,
}

/// An identity chain signed now (SIGNER, ECDSA_EPHEMERAL), as the explorer's.
fn identity(kind: &str) -> Result<Identity, String> {
    let owner = Wallet::random().map_err(|e| e.to_string())?;
    let ephemeral = Wallet::random().map_err(|e| e.to_string())?;
    let expiration = (chrono::Utc::now() + chrono::Duration::days(1))
        .to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
    let payload = format!(
        "Decentraland Login\nEphemeral address: {}\nExpiration: {expiration}",
        ephemeral.address()
    );
    let signature = owner
        .sign_message(payload.as_bytes())
        .map_err(|e| e.to_string())?;
    let chain = json!([
        { "payload": owner.address(), "signature": "", "type": "SIGNER" },
        { "payload": payload, "signature": signature, "type": "ECDSA_EPHEMERAL" },
    ])
    .to_string();
    let address = match kind {
        "fresh" => owner.address(),
        "foreign" => Wallet::random().map_err(|e| e.to_string())?.address(),
        other => return Err(format!("unknown harness.identity {other}")),
    };
    Ok(Identity { address, chain })
}

/// `$identity` in any string of `value` replaced by the identity's address.
fn substitute(value: &Value, identity: Option<&Identity>) -> Value {
    match (value, identity) {
        (Value::String(s), Some(id)) if s.contains("$identity") => {
            Value::String(s.replace("$identity", &id.address))
        }
        (Value::Array(items), _) => {
            Value::Array(items.iter().map(|v| substitute(v, identity)).collect())
        }
        (Value::Object(map), _) => Value::Object(
            map.iter()
                .map(|(k, v)| (k.clone(), substitute(v, identity)))
                .collect(),
        ),
        _ => value.clone(),
    }
}

fn set_path(root: &mut Value, path: &str, value: Value) -> Result<(), String> {
    let mut parts = path.split('.').peekable();
    let mut cursor = root;
    while let Some(part) = parts.next() {
        let map = cursor
            .as_object_mut()
            .ok_or_else(|| format!("patch {path}: {part} is not inside an object"))?;
        if parts.peek().is_none() {
            if value == json!("$delete") {
                map.remove(part);
            } else {
                map.insert(part.to_string(), value);
            }
            return Ok(());
        }
        cursor = map
            .entry(part.to_string())
            .or_insert_with(|| Value::Object(Default::default()));
    }
    Ok(())
}

fn build_body(entry: &Entry, identity: Option<&Identity>) -> Result<Value, String> {
    let mut body = match &entry.request.body {
        Value::String(name) if name.starts_with('$') => {
            let path = fixtures_dir()
                .join("bodies")
                .join(format!("{}.json", &name[1..]));
            let text =
                std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
            serde_json::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))?
        }
        other => other.clone(),
    };
    if let Some(id) = identity {
        let context = body
            .get_mut("context")
            .and_then(Value::as_object_mut)
            .ok_or("harness.identity needs a context object")?;
        context.insert("dcl_eth_address".into(), json!(id.address));
        context.insert("auth_chain".into(), json!(id.chain));
        if body.get("userId").is_some() {
            body["userId"] = json!(id.address);
        }
    }
    for (path, value) in &entry.request.patch {
        set_path(&mut body, path, substitute(value, identity))?;
    }
    Ok(body)
}

async fn send(router: &Router, path: &str, body: Vec<u8>, signature: Option<&str>) -> (u16, Value) {
    let mut request = Request::post(path).header(header::CONTENT_TYPE, "application/json");
    if let Some(signature) = signature {
        request = request.header("x-signature", signature);
    }
    let response = router
        .clone()
        .oneshot(request.body(Body::from(body)).unwrap())
        .await
        .unwrap();
    let status = response.status().as_u16();
    let bytes = axum::body::to_bytes(response.into_body(), 1 << 20)
        .await
        .unwrap();
    let body = serde_json::from_slice(&bytes)
        .unwrap_or_else(|_| Value::String(String::from_utf8_lossy(&bytes).into_owned()));
    (status, body)
}

// ── comparison ─────────────────────────────────────────────────────────────

/// Every member of `expected` present in `actual` with the same value;
/// objects recurse, everything else compares whole.
fn subset(expected: &Value, actual: &Value, path: &str, out: &mut Vec<String>) {
    match (expected, actual) {
        (Value::Object(e), Value::Object(a)) => {
            for (key, value) in e {
                let path = if path.is_empty() {
                    key.clone()
                } else {
                    format!("{path}.{key}")
                };
                match a.get(key) {
                    Some(actual) => subset(value, actual, &path, out),
                    None => out.push(format!("{path} missing (expected {value})")),
                }
            }
        }
        _ if expected == actual => {}
        _ => out.push(format!("{path} = {actual} (expected {expected})")),
    }
}

struct Effects {
    rows: i64,
    row: Option<Value>,
    onboarding: Arc<OnboardingState>,
}

fn compare_effects(entry: &Entry, identity: Option<&Identity>, effects: &Effects) -> Vec<String> {
    let mut out = Vec::new();
    if let Some(rows) = entry.effect.rows
        && rows != effects.rows
    {
        out.push(format!("rows = {} (expected {rows})", effects.rows));
    }
    if let Some(expected) = &entry.effect.row {
        match &effects.row {
            Some(row) => subset(&substitute(expected, identity), row, "row", &mut out),
            None => out.push("row missing".into()),
        }
    }
    if let Some(expected) = &entry.effect.checkpoint {
        let expected = substitute(expected, identity);
        let user = expected["user"].as_str().unwrap_or_default();
        let checkpoint = expected["checkpoint"].as_i64().unwrap_or_default();
        let state = effects.onboarding.checkpoint_state(user, checkpoint);
        if expected.get("recorded") == Some(&json!(false)) {
            if let Some(state) = state {
                out.push(format!(
                    "checkpoint {checkpoint} of {user} recorded: {}",
                    json!(state)
                ));
            }
        } else {
            match state {
                Some(state) => {
                    let mut wanted = expected.clone();
                    wanted.as_object_mut().unwrap().remove("user");
                    wanted.as_object_mut().unwrap().remove("checkpoint");
                    subset(&wanted, &json!(state), "checkpoint", &mut out);
                }
                None => out.push(format!("checkpoint {checkpoint} of {user} not recorded")),
            }
        }
    }
    out
}

enum Outcome {
    Pass,
    Fail(String),
    /// A defect of the fixture itself, never a gap.
    Invalid(String),
}

async fn run_entry(entry: &Entry, pool: &PgPool) -> Outcome {
    sqlx::query("TRUNCATE social.client_events")
        .execute(pool)
        .await
        .unwrap();
    let kind = entry.source.kind.as_str();
    if !(kind == "source" || (kind == "accepted" && ACCEPTED_ENTRIES.contains(&entry.id.as_str())))
    {
        return Outcome::Invalid(format!(
            "source.kind {kind:?}: only entries derived from the pinned source (or the ACCEPTED_ENTRIES as \"accepted\") are tracked"
        ));
    }
    let harness = &entry.harness;
    let identity = match harness.identity.as_deref().map(identity) {
        Some(Ok(identity)) => Some(identity),
        Some(Err(reason)) => return Outcome::Invalid(reason),
        None => None,
    };
    let onboarding = Arc::new(OnboardingState::default());
    if let Some(seed) = &harness.seed_checkpoint {
        let seed: CheckpointRequest =
            match serde_json::from_value(substitute(seed, identity.as_ref())) {
                Ok(seed) => seed,
                Err(e) => return Outcome::Invalid(format!("seed_checkpoint: {e}")),
            };
        if let Err(e) = onboarding.record_checkpoint(seed) {
            return Outcome::Invalid(format!("seed_checkpoint refused: {e}"));
        }
    }
    let settings = Settings {
        signing_key: (harness.signing_key != Some(false))
            .then(|| String::from_utf8(SIGNING_KEY.to_vec()).unwrap()),
        force_context_validation: harness.force_context_validation,
        expected_realm: harness.expected_realm.clone(),
        expected_realm_url: harness.expected_realm_url.clone(),
        anon_id_denylist: Settings::parse_list(&harness.denylist.join(",")),
        web_project_id: WEB_PROJECT_ID.to_string(),
        retention: Duration::from_secs(30 * 24 * 3600),
    };
    let state = Arc::new(State::new(pool.clone(), settings, onboarding.clone()));
    let router = Router::new().nest(
        PREFIX,
        writes::guard(
            segment_webhooks::router(state),
            harness.writes_enabled.unwrap_or(true),
        ),
    );
    let body = match build_body(entry, identity.as_ref()) {
        Ok(body) => body,
        Err(reason) => return Outcome::Invalid(reason),
    };
    let compact = serde_json::to_vec(&body).unwrap();
    let sent = match harness.body_form.as_deref() {
        Some("pretty") => serde_json::to_vec_pretty(&body).unwrap(),
        None => compact.clone(),
        Some(other) => return Outcome::Invalid(format!("unknown harness.body_form {other}")),
    };
    let signature = match harness.sign.as_deref() {
        None | Some("valid") => Some(segment_signature::sign(SIGNING_KEY, &compact)),
        Some("wrong") => Some(segment_signature::sign(b"another-key", &compact)),
        Some("none") => None,
        Some(other) => return Outcome::Invalid(format!("unknown harness.sign {other}")),
    };
    let path = format!("{PREFIX}{}", entry.request.path);
    let mut diffs = Vec::new();
    let repeat = harness.repeat.unwrap_or(1).max(1);
    for attempt in 1..=repeat {
        let (status, answer) = send(&router, &path, sent.clone(), signature.as_deref()).await;
        let mut this = Vec::new();
        if status != entry.expected.status {
            this.push(format!(
                "status {status} (expected {})",
                entry.expected.status
            ));
        }
        if let Some(expected) = &entry.expected.body
            && *expected != answer
        {
            this.push(format!("body = {answer} (expected {expected})"));
        }
        if let Some(expected) = &entry.expected.body_subset {
            subset(expected, &answer, "body", &mut this);
        }
        if repeat > 1 {
            this = this
                .into_iter()
                .map(|d| format!("attempt {attempt}: {d}"))
                .collect();
        }
        diffs.extend(this);
    }
    let rows: i64 = sqlx::query_scalar("SELECT count(*) FROM social.client_events")
        .fetch_one(pool)
        .await
        .unwrap();
    let row: Option<Value> = sqlx::query_scalar(
        "SELECT row_to_json(e) FROM (SELECT key, source, sub_type, address, session_id, anonymous_id, realm, ip, metadata \
         FROM social.client_events ORDER BY created_at, key LIMIT 1) e",
    )
    .fetch_optional(pool)
    .await
    .unwrap();
    diffs.extend(compare_effects(
        entry,
        identity.as_ref(),
        &Effects {
            rows,
            row,
            onboarding,
        },
    ));
    if diffs.is_empty() {
        Outcome::Pass
    } else {
        let observed = diffs.join("; ");
        let observed = match &identity {
            Some(id) => observed.replace(&id.address, "<identity>"),
            None => observed,
        };
        Outcome::Fail(observed)
    }
}

// ── the gate ───────────────────────────────────────────────────────────────

#[tokio::test]
async fn segment_webhooks_match_the_notifier_fixtures_except_the_known_gaps() {
    let Some(db) = db::scratch_database().await else {
        use std::io::Write;
        let _ = writeln!(
            std::io::stderr(),
            "SKIPPED notifier parity: CATALYST_TEST_DATABASE_URL is unset, no fixture ran"
        );
        return;
    };
    let pool = db.pool.clone();
    let entries = load_entries();
    let mut known = load_known_gaps();
    for id in known.keys() {
        assert!(
            entries.iter().any(|e| e.id == *id),
            "known_gaps.json names {id}, which no fixture defines"
        );
    }
    // An accepted entry is never a gap: listed, it is treated as unlisted (so
    // the run still reports it), dropped by a write and an error otherwise.
    let misfiled: Vec<String> = known
        .keys()
        .filter(|id| ACCEPTED_ENTRIES.contains(&id.as_str()))
        .cloned()
        .collect();
    for id in &misfiled {
        known.remove(id);
    }
    let write = flag("NOTIFIER_PARITY_WRITE_GAPS");
    let allow_new = flag("NOTIFIER_PARITY_ALLOW_NEW_GAPS");
    assert!(
        write || !allow_new,
        "NOTIFIER_PARITY_ALLOW_NEW_GAPS=1 needs NOTIFIER_PARITY_WRITE_GAPS=1"
    );

    let mut rows: Vec<(String, &'static str, String)> = Vec::new();
    let mut unexpected_failures: Vec<(String, String)> = Vec::new();
    let mut unexpected_passes = Vec::new();
    let mut changed = Vec::new();
    let mut invalid = Vec::new();
    let mut still_failing: BTreeMap<String, String> = BTreeMap::new();
    let (mut passed, mut failed) = (0, 0);
    for entry in &entries {
        let id = entry.id.clone();
        match run_entry(entry, &pool).await {
            Outcome::Pass => {
                passed += 1;
                if known.contains_key(&id) {
                    unexpected_passes.push(id.clone());
                    rows.push((id, "FIXED", String::new()));
                } else {
                    rows.push((id, "PASS", String::new()));
                }
            }
            Outcome::Fail(observed) => {
                failed += 1;
                match known.get(&id) {
                    Some(recorded) if *recorded == observed => {
                        still_failing.insert(id.clone(), observed.clone());
                        rows.push((id, "GAP", observed));
                    }
                    Some(recorded) => {
                        still_failing.insert(id.clone(), observed.clone());
                        changed.push(format!("{id}: now {observed} (was {recorded})"));
                        rows.push((id, "CHANGED", observed));
                    }
                    None => {
                        still_failing.insert(id.clone(), observed.clone());
                        unexpected_failures.push((id.clone(), observed.clone()));
                        rows.push((id, "FAIL", observed));
                    }
                }
            }
            Outcome::Invalid(reason) => {
                invalid.push(format!("{id}: {reason}"));
                rows.push((id, "BAD", reason));
            }
        }
    }
    db.teardown().await;

    if write {
        let mut kept: Vec<KnownGap> = still_failing
            .iter()
            .filter(|(id, _)| {
                (allow_new && !ACCEPTED_ENTRIES.contains(&id.as_str())) || known.contains_key(*id)
            })
            .map(|(id, observed)| KnownGap {
                id: id.clone(),
                observed: observed.clone(),
            })
            .collect();
        kept.sort_by(|a, b| a.id.cmp(&b.id));
        for id in &misfiled {
            eprintln!("removed {id}: an accepted entry is never a gap");
        }
        for gap in &kept {
            if !known.contains_key(&gap.id) {
                eprintln!("added {}: {}", gap.id, gap.observed);
            }
        }
        let about = serde_json::from_str::<Value>(
            &std::fs::read_to_string(fixtures_dir().join("known_gaps.json")).unwrap(),
        )
        .unwrap()["about"]
            .clone();
        let text =
            serde_json::to_string_pretty(&json!({ "about": about, "entries": kept })).unwrap();
        std::fs::write(fixtures_dir().join("known_gaps.json"), text + "\n").unwrap();
    }

    eprintln!(
        "notifier parity: {passed} passed, {failed} failed ({} known gaps), {} invalid of {} entries",
        still_failing.len() - unexpected_failures.len(),
        invalid.len(),
        entries.len()
    );
    for (id, verdict, reason) in &rows {
        if reason.is_empty() {
            eprintln!("{verdict:7} {id}");
        } else {
            eprintln!("{verdict:7} {id}: {reason}");
        }
    }
    assert!(
        invalid.is_empty(),
        "fixture defects (BAD above; never known gaps): {invalid:#?}"
    );
    // An accepted entry is never written as a gap, with or without the flag.
    let unblessable: Vec<String> = unexpected_failures
        .iter()
        .filter(|(id, _)| !allow_new || ACCEPTED_ENTRIES.contains(&id.as_str()))
        .map(|(id, observed)| format!("{id}: {observed}"))
        .collect();
    assert!(
        unblessable.is_empty(),
        "entries failing but not in known_gaps.json (fix the router or the fixture; a real gap is added with NOTIFIER_PARITY_WRITE_GAPS=1 NOTIFIER_PARITY_ALLOW_NEW_GAPS=1 and recorded in the fixtures README under 'Known gaps' with the pinned source lines it diverges from; ACCEPTED_ENTRIES are never gaps): {unblessable:#?}"
    );
    if !write {
        assert!(
            misfiled.is_empty(),
            "known_gaps.json lists accepted entries, which are never gaps; remove them (NOTIFIER_PARITY_WRITE_GAPS=1): {misfiled:#?}"
        );
        assert!(
            unexpected_passes.is_empty(),
            "entries in known_gaps.json that now pass; remove them (NOTIFIER_PARITY_WRITE_GAPS=1): {unexpected_passes:#?}"
        );
        assert!(
            changed.is_empty(),
            "known gaps whose observed difference changed (CHANGED above); check none got worse, then refresh with NOTIFIER_PARITY_WRITE_GAPS=1: {changed:#?}"
        );
    }
}

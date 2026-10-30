//! The onboarding checkpoints a client reports (`reached`, `completed`, 1-7)
//! and the nudges a mailer pulls (`pending-nudges?sequence=1..3`: 12, 24, 36 h
//! after the latest checkpoint of a user with an email). Process state, as
//! the old service kept it: a restart forgets the checkpoints. Both routes
//! need `ONBOARDING_API_KEY` as a bearer (compared in constant time); without
//! the key they answer 401.

use axum::body::Bytes;
use axum::extract::{Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use chrono::{DateTime, Duration, Utc};
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

use crate::api::Shared as AppState;

const SEQUENCE_HOURS: [i64; 4] = [0, 12, 24, 36];
const MIN_CHECKPOINT: i64 = 1;
const MAX_CHECKPOINT: i64 = 7;
const MAX_USER_IDENTIFIER_LEN: usize = 255;
const MAX_EMAIL_LEN: usize = 255;
const MAX_WALLET_LEN: usize = 255;
const MAX_SOURCE_LEN: usize = 50;

#[derive(Debug, Clone)]
struct CheckpointRow {
    user_id: String,
    email: Option<String>,
    wallet: Option<String>,
    checkpoint: i64,
    reached_at: DateTime<Utc>,
    completed_at: Option<DateTime<Utc>>,
    metadata: Option<serde_json::Value>,
}

#[derive(Debug, Clone)]
struct CheckpointPayload {
    user_identifier: String,
    identifier_type: String,
    checkpoint_id: i64,
    action: String,
    email: Option<String>,
    wallet: Option<String>,
    metadata: Option<serde_json::Value>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
struct PendingNudge {
    user_id: String,
    checkpoint_id: i64,
    email: String,
}

#[derive(Serialize)]
struct CheckpointAck {
    success: bool,
}

#[derive(Serialize)]
struct PendingNudgesResponse {
    sequence: i64,
    count: u64,
    nudges: Vec<PendingNudge>,
}

/// Rows are append-only and unique per (user_id, checkpoint); `by_key` finds the one row
/// an upsert touches, `by_wallet` the rows a wallet-only checkpoint resolves through, and
/// `latest` each user's highest checkpoint, the only row a nudge can target.
#[derive(Default)]
struct OnboardingStore {
    checkpoints: Rows,
    by_key: HashMap<(String, i64), usize>,
    by_wallet: HashMap<String, Vec<usize>>,
    latest: HashMap<String, usize>,
    sent_nudges: HashSet<(String, i64, i64)>,
}

/// The checkpoint rows, reached only by index, so a test can count how many rows
/// an operation touches: a constant number per upsert and one per user for the
/// nudge list, never a walk of every row.
#[derive(Default)]
struct Rows {
    rows: Vec<CheckpointRow>,
    #[cfg(test)]
    touched: std::sync::atomic::AtomicUsize,
}

impl Rows {
    fn len(&self) -> usize {
        self.rows.len()
    }

    fn push(&mut self, row: CheckpointRow) {
        self.rows.push(row);
    }

    #[cfg(test)]
    fn iter(&self) -> std::slice::Iter<'_, CheckpointRow> {
        self.rows.iter()
    }

    /// The rows indexed since the last call.
    #[cfg(test)]
    fn take_touched(&self) -> usize {
        self.touched.swap(0, std::sync::atomic::Ordering::Relaxed)
    }
}

impl std::ops::Index<usize> for Rows {
    type Output = CheckpointRow;

    fn index(&self, i: usize) -> &CheckpointRow {
        #[cfg(test)]
        self.touched
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        &self.rows[i]
    }
}

impl std::ops::IndexMut<usize> for Rows {
    fn index_mut(&mut self, i: usize) -> &mut CheckpointRow {
        #[cfg(test)]
        {
            *self.touched.get_mut() += 1;
        }
        &mut self.rows[i]
    }
}

impl OnboardingStore {
    fn resolve_wallet_identity(&self, wallet_address: &str) -> Option<(String, String)> {
        self.by_wallet
            .get(wallet_address)?
            .iter()
            .map(|&i| (i, &self.checkpoints[i]))
            .filter(|(_, r)| r.wallet.as_deref() == Some(wallet_address) && r.email.is_some())
            .min_by_key(|(i, r)| (r.checkpoint, *i))
            .map(|(_, r)| (r.user_id.clone(), r.email.clone().unwrap_or_default()))
    }

    fn set_wallet(&mut self, i: usize, wallet: &Option<String>) {
        let Some(w) = wallet else { return };
        let row = &mut self.checkpoints[i];
        let changed = row.wallet.as_deref() != Some(w.as_str());
        row.wallet = Some(w.clone());
        if changed {
            self.by_wallet.entry(w.clone()).or_default().push(i);
        }
    }

    fn push_row(&mut self, row: CheckpointRow) {
        let i = self.checkpoints.len();
        self.by_key.insert((row.user_id.clone(), row.checkpoint), i);
        if let Some(w) = &row.wallet {
            self.by_wallet.entry(w.clone()).or_default().push(i);
        }
        match self.latest.get(&row.user_id) {
            Some(&latest) if self.checkpoints[latest].checkpoint >= row.checkpoint => {}
            _ => {
                self.latest.insert(row.user_id.clone(), i);
            }
        }
        self.checkpoints.push(row);
    }

    fn record_checkpoint(&mut self, payload: CheckpointPayload, now: DateTime<Utc>) {
        let CheckpointPayload {
            mut user_identifier,
            identifier_type,
            checkpoint_id,
            action,
            mut email,
            mut wallet,
            metadata,
        } = payload;

        if let Some(w) = wallet.as_mut() {
            *w = w.to_lowercase();
        }

        if identifier_type == "wallet" && email.is_none() {
            let lowered = user_identifier.to_lowercase();
            if let Some((resolved_user, resolved_email)) = self.resolve_wallet_identity(&lowered) {
                wallet = Some(wallet.unwrap_or(lowered));
                user_identifier = resolved_user;
                email = Some(resolved_email);
            } else {
                wallet = Some(wallet.unwrap_or(lowered));
            }
        }

        let key = (user_identifier.clone(), checkpoint_id);
        if action == "completed" {
            if let Some(&i) = self.by_key.get(&key)
                && self.checkpoints[i].completed_at.is_none()
            {
                self.checkpoints[i].completed_at = Some(now);
                if email.is_some() {
                    self.checkpoints[i].email = email.clone();
                }
                self.set_wallet(i, &wallet);
            }
            return;
        }

        if let Some(&i) = self.by_key.get(&key) {
            if email.is_some() {
                self.checkpoints[i].email = email.clone();
            }
            self.set_wallet(i, &wallet);
            if metadata.is_some() {
                self.checkpoints[i].metadata = metadata.clone();
            }
        } else {
            self.push_row(CheckpointRow {
                user_id: user_identifier.clone(),
                email: email.clone(),
                wallet: wallet.clone(),
                checkpoint: checkpoint_id,
                reached_at: now,
                completed_at: None,
                metadata: metadata.clone(),
            });
        }

        if checkpoint_id > 1
            && let Some(&i) = self.by_key.get(&(user_identifier, checkpoint_id - 1))
            && self.checkpoints[i].completed_at.is_none()
        {
            self.checkpoints[i].completed_at = Some(now);
        }
    }

    fn pending_nudges(&self, sequence: i64, now: DateTime<Utc>) -> Vec<PendingNudge> {
        let hours = SEQUENCE_HOURS[sequence as usize];
        let threshold = now - Duration::hours(hours);

        let mut latest: Vec<usize> = self.latest.values().copied().collect();
        latest.sort_unstable();
        latest
            .into_iter()
            .filter_map(|i| {
                let oc = &self.checkpoints[i];
                let email = oc.email.as_ref()?;
                if oc.completed_at.is_some() {
                    return None;
                }
                if oc.reached_at >= threshold {
                    return None;
                }
                if self
                    .sent_nudges
                    .contains(&(oc.user_id.clone(), oc.checkpoint, sequence))
                {
                    return None;
                }
                Some(PendingNudge {
                    user_id: oc.user_id.clone(),
                    checkpoint_id: oc.checkpoint,
                    email: email.clone(),
                })
            })
            .collect()
    }
}

#[derive(Default)]
pub struct OnboardingState {
    inner: Mutex<OnboardingStore>,
}

impl OnboardingState {
    fn record(&self, payload: CheckpointPayload, now: DateTime<Utc>) {
        self.inner.lock().record_checkpoint(payload, now);
    }

    fn pending_nudges(&self, sequence: i64, now: DateTime<Utc>) -> Vec<PendingNudge> {
        self.inner.lock().pending_nudges(sequence, now)
    }

    #[cfg(test)]
    fn mark_nudge_sent(&self, user_id: &str, checkpoint: i64, sequence: i64) {
        self.inner
            .lock()
            .sent_nudges
            .insert((user_id.to_string(), checkpoint, sequence));
    }
}

/// Under the `/onboarding` prefix: `POST /onboarding/checkpoint`,
/// `GET /onboarding/pending-nudges`, both behind `ONBOARDING_API_KEY`.
pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/checkpoint", post(post_checkpoint))
        .route("/pending-nudges", get(get_pending_nudges))
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CheckpointRequestBody {
    #[serde(rename = "checkpointId")]
    checkpoint_id: i64,
    #[serde(rename = "userIdentifier")]
    user_identifier: String,
    #[serde(rename = "identifierType")]
    identifier_type: String,
    action: String,
    #[serde(default)]
    email: Option<String>,
    #[serde(default)]
    wallet: Option<String>,
    #[serde(default)]
    source: Option<String>,
    #[serde(default)]
    metadata: Option<serde_json::Value>,
}

fn validate_checkpoint(body: &CheckpointRequestBody) -> Result<(), String> {
    if body.checkpoint_id < MIN_CHECKPOINT || body.checkpoint_id > MAX_CHECKPOINT {
        return Err("checkpointId must be an integer between 1 and 7".to_string());
    }
    let ulen = body.user_identifier.chars().count();
    if !(1..=MAX_USER_IDENTIFIER_LEN).contains(&ulen) {
        return Err("userIdentifier must be between 1 and 255 characters".to_string());
    }
    if body.identifier_type != "email" && body.identifier_type != "wallet" {
        return Err("identifierType must be one of 'email', 'wallet'".to_string());
    }
    if body.action != "reached" && body.action != "completed" {
        return Err("action must be one of 'reached', 'completed'".to_string());
    }
    if let Some(email) = &body.email
        && email.chars().count() > MAX_EMAIL_LEN
    {
        return Err("email must be at most 255 characters".to_string());
    }
    if let Some(wallet) = &body.wallet
        && wallet.chars().count() > MAX_WALLET_LEN
    {
        return Err("wallet must be at most 255 characters".to_string());
    }
    if let Some(source) = &body.source
        && source.chars().count() > MAX_SOURCE_LEN
    {
        return Err("source must be at most 50 characters".to_string());
    }
    if let Some(metadata) = &body.metadata
        && !metadata.is_object()
    {
        return Err("metadata must be an object".to_string());
    }
    Ok(())
}

async fn post_checkpoint(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    if let Err(resp) = require_onboarding_bearer(&state, &headers) {
        return *resp;
    }

    let req: CheckpointRequestBody = match serde_json::from_slice(&body) {
        Ok(req) => req,
        Err(e) => return bad_request(&e.to_string()),
    };
    if let Err(msg) = validate_checkpoint(&req) {
        return bad_request(&msg);
    }

    state.onboarding.record(
        CheckpointPayload {
            user_identifier: req.user_identifier,
            identifier_type: req.identifier_type,
            checkpoint_id: req.checkpoint_id,
            action: req.action,
            email: req.email,
            wallet: req.wallet,
            metadata: req.metadata,
        },
        Utc::now(),
    );

    (StatusCode::OK, Json(CheckpointAck { success: true })).into_response()
}

#[derive(Debug, Deserialize)]
struct PendingNudgesQuery {
    #[serde(default)]
    sequence: Option<i64>,
}

async fn get_pending_nudges(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<PendingNudgesQuery>,
) -> Response {
    if let Err(resp) = require_onboarding_bearer(&state, &headers) {
        return *resp;
    }

    let sequence = query.sequence.unwrap_or(1);
    if !(1..=3).contains(&sequence) {
        return bad_request("sequence must be 1, 2, or 3");
    }

    let nudges = state.onboarding.pending_nudges(sequence, Utc::now());
    (
        StatusCode::OK,
        Json(PendingNudgesResponse {
            sequence,
            count: nudges.len() as u64,
            nudges,
        }),
    )
        .into_response()
}

fn require_onboarding_bearer(state: &AppState, headers: &HeaderMap) -> Result<(), Box<Response>> {
    crate::auth::admin_bearer::check(state.settings.onboarding_api_key.as_deref(), headers)
        .map_err(|_| Box::new(unauthorized()))
}

fn unauthorized() -> Response {
    (
        StatusCode::UNAUTHORIZED,
        Json(serde_json::json!({ "error": "Unauthorized" })),
    )
        .into_response()
}

fn bad_request(msg: &str) -> Response {
    (
        StatusCode::BAD_REQUEST,
        Json(serde_json::json!({ "error": msg })),
    )
        .into_response()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn t(secs: i64) -> DateTime<Utc> {
        DateTime::<Utc>::from_timestamp(1_700_000_000 + secs, 0).unwrap()
    }

    fn reached(
        user: &str,
        id_type: &str,
        checkpoint: i64,
        email: Option<&str>,
        wallet: Option<&str>,
    ) -> CheckpointPayload {
        CheckpointPayload {
            user_identifier: user.to_string(),
            identifier_type: id_type.to_string(),
            checkpoint_id: checkpoint,
            action: "reached".to_string(),
            email: email.map(str::to_string),
            wallet: wallet.map(str::to_string),
            metadata: None,
        }
    }

    #[test]
    fn reached_then_pending_after_interval() {
        let mut store = OnboardingStore::default();
        store.record_checkpoint(reached("a@b.com", "email", 1, Some("a@b.com"), None), t(0));

        assert!(store.pending_nudges(1, t(60)).is_empty());
        assert_eq!(
            store.pending_nudges(1, t(13 * 3600)),
            vec![PendingNudge {
                user_id: "a@b.com".to_string(),
                checkpoint_id: 1,
                email: "a@b.com".to_string(),
            }]
        );

        assert!(store.pending_nudges(2, t(13 * 3600)).is_empty());
        assert_eq!(store.pending_nudges(2, t(25 * 3600)).len(), 1);
    }

    #[test]
    fn checkpoint_without_email_is_not_pending() {
        let mut store = OnboardingStore::default();
        store.record_checkpoint(reached("0xabc", "wallet", 1, None, None), t(0));
        assert!(store.pending_nudges(1, t(13 * 3600)).is_empty());
    }

    #[test]
    fn reaching_next_checkpoint_completes_previous_and_supersedes_it() {
        let mut store = OnboardingStore::default();
        store.record_checkpoint(reached("a@b.com", "email", 1, Some("a@b.com"), None), t(0));
        store.record_checkpoint(reached("a@b.com", "email", 2, Some("a@b.com"), None), t(10));

        let pending = store.pending_nudges(1, t(13 * 3600));
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].checkpoint_id, 2);
    }

    #[test]
    fn completed_action_marks_completion() {
        let mut store = OnboardingStore::default();
        store.record_checkpoint(reached("a@b.com", "email", 3, Some("a@b.com"), None), t(0));
        assert_eq!(store.pending_nudges(1, t(13 * 3600)).len(), 1);

        store.record_checkpoint(
            CheckpointPayload {
                action: "completed".to_string(),
                ..reached("a@b.com", "email", 3, None, None)
            },
            t(20),
        );
        assert!(store.pending_nudges(1, t(13 * 3600)).is_empty());
    }

    #[test]
    fn wallet_checkpoint_resolves_to_email_user_id() {
        let mut store = OnboardingStore::default();
        store.record_checkpoint(
            reached("a@b.com", "email", 1, Some("a@b.com"), Some("0xABC")),
            t(0),
        );
        store.record_checkpoint(reached("0xAbC", "wallet", 2, None, None), t(10));

        let uid2 = store
            .checkpoints
            .iter()
            .find(|r| r.checkpoint == 2)
            .map(|r| r.user_id.clone());
        assert_eq!(uid2.as_deref(), Some("a@b.com"));

        let pending = store.pending_nudges(1, t(13 * 3600));
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].checkpoint_id, 2);
        assert_eq!(pending[0].user_id, "a@b.com");
    }

    #[test]
    fn upsert_coalesces_email_and_wallet() {
        let mut store = OnboardingStore::default();
        store.record_checkpoint(reached("u", "wallet", 1, None, Some("0xabc")), t(0));
        store.record_checkpoint(reached("u", "wallet", 1, Some("late@b.com"), None), t(5));

        let row = store
            .checkpoints
            .iter()
            .find(|r| r.checkpoint == 1)
            .unwrap();
        assert_eq!(row.email.as_deref(), Some("late@b.com"));
        assert_eq!(row.wallet.as_deref(), Some("0xabc"));
    }

    #[test]
    fn sent_nudge_is_excluded() {
        let state = OnboardingState::default();
        state.record(reached("a@b.com", "email", 1, Some("a@b.com"), None), t(0));
        assert_eq!(state.pending_nudges(1, t(13 * 3600)).len(), 1);
        state.mark_nudge_sent("a@b.com", 1, 1);
        assert!(state.pending_nudges(1, t(13 * 3600)).is_empty());
        assert_eq!(state.pending_nudges(2, t(25 * 3600)).len(), 1);
    }

    #[test]
    fn pending_nudges_is_linear_not_quadratic() {
        let mut store = OnboardingStore::default();
        for i in 0..20_000 {
            let user = format!("user-{i:05}");
            let email = format!("u{i}@example.com");
            store.record_checkpoint(reached(&user, "email", 1, Some(&email), None), t(0));
        }
        store.checkpoints.take_touched();
        let nudges = store.pending_nudges(1, t(13 * 3600));
        assert_eq!(nudges.len(), 20_000);
        assert_eq!(nudges[0].user_id, "user-00000", "insertion order is kept");
        assert_eq!(
            store.checkpoints.take_touched(),
            20_000,
            "pending_nudges reads each user's latest row once"
        );
    }

    #[test]
    fn record_checkpoint_is_indexed_not_a_scan() {
        let mut store = OnboardingStore::default();
        for i in 0..20_000 {
            let user = format!("user-{i:05}");
            store.record_checkpoint(reached(&user, "email", 1, Some(&user), None), t(0));
        }
        store.checkpoints.take_touched();
        for i in 0..20_000 {
            let user = format!("user-{i:05}");
            store.record_checkpoint(reached(&user, "email", 2, Some(&user), None), t(10));
        }
        let touched = store.checkpoints.take_touched();
        assert!(
            touched <= 3 * 20_000,
            "20k upserts touched {touched} rows: an upsert walks the rows instead of its index"
        );
        assert_eq!(store.by_key.len(), 40_000);
        let pending = store.pending_nudges(1, t(13 * 3600));
        assert_eq!(pending.len(), 20_000);
        assert!(pending.iter().all(|n| n.checkpoint_id == 2));
    }

    #[test]
    fn wallet_learned_late_still_resolves_and_prefers_the_lowest_checkpoint() {
        let mut store = OnboardingStore::default();
        store.record_checkpoint(reached("b@b.com", "email", 3, Some("b@b.com"), None), t(0));
        store.record_checkpoint(reached("a@b.com", "email", 2, Some("a@b.com"), None), t(1));
        store.record_checkpoint(reached("b@b.com", "email", 3, None, Some("0xABC")), t(2));
        store.record_checkpoint(reached("a@b.com", "email", 2, None, Some("0xabc")), t(3));

        assert_eq!(
            store.resolve_wallet_identity("0xabc"),
            Some(("a@b.com".to_string(), "a@b.com".to_string()))
        );
        store.record_checkpoint(reached("0xABC", "wallet", 4, None, None), t(10));
        let row = store
            .checkpoints
            .iter()
            .find(|r| r.checkpoint == 4)
            .unwrap();
        assert_eq!(row.user_id, "a@b.com");
        assert_eq!(row.email.as_deref(), Some("a@b.com"));
    }

    #[test]
    fn validation_rejects_out_of_range_and_bad_enums() {
        let base = |cp: i64, id_type: &str, action: &str| CheckpointRequestBody {
            checkpoint_id: cp,
            user_identifier: "u".to_string(),
            identifier_type: id_type.to_string(),
            action: action.to_string(),
            email: None,
            wallet: None,
            source: None,
            metadata: None,
        };
        assert!(validate_checkpoint(&base(1, "email", "reached")).is_ok());
        assert!(validate_checkpoint(&base(7, "wallet", "completed")).is_ok());
        assert!(validate_checkpoint(&base(0, "email", "reached")).is_err());
        assert!(validate_checkpoint(&base(8, "email", "reached")).is_err());
        assert!(validate_checkpoint(&base(1, "phone", "reached")).is_err());
        assert!(validate_checkpoint(&base(1, "email", "skipped")).is_err());

        let mut empty_user = base(1, "email", "reached");
        empty_user.user_identifier = String::new();
        assert!(validate_checkpoint(&empty_user).is_err());
    }
}

#[cfg(test)]
mod route_tests {
    use super::*;
    use axum::body::Body;
    use axum::http::Request;
    use tower::ServiceExt;

    async fn app(key: Option<&str>) -> Router {
        let mut settings = crate::api::Settings::for_tests(crate::api::Sources::defaults());
        settings.onboarding_api_key = key.map(str::to_string);
        let state = crate::api::tests::node_without_db_state(settings).await;
        Router::new().nest("/onboarding", routes().with_state(state))
    }

    async fn send(
        app: &Router,
        method: &str,
        path: &str,
        bearer: Option<&str>,
        body: &str,
    ) -> (StatusCode, serde_json::Value) {
        let mut b = Request::builder()
            .method(method)
            .uri(path)
            .header("content-type", "application/json");
        if let Some(t) = bearer {
            b = b.header("authorization", format!("Bearer {t}"));
        }
        let resp = app
            .clone()
            .oneshot(b.body(Body::from(body.to_string())).unwrap())
            .await
            .unwrap();
        let status = resp.status();
        let bytes = axum::body::to_bytes(resp.into_body(), 1 << 20)
            .await
            .unwrap();
        (
            status,
            serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null),
        )
    }

    const BODY: &str = r#"{"checkpointId":1,"userIdentifier":"a@b.com","identifierType":"email","action":"reached","email":"a@b.com"}"#;

    #[tokio::test]
    async fn both_routes_need_the_bearer_and_the_key_configured() {
        let unconfigured = app(None).await;
        assert_eq!(
            send(
                &unconfigured,
                "POST",
                "/onboarding/checkpoint",
                Some(""),
                BODY
            )
            .await
            .0,
            StatusCode::UNAUTHORIZED
        );
        let app = app(Some("test-onboarding-key")).await;
        assert_eq!(
            send(&app, "POST", "/onboarding/checkpoint", None, BODY)
                .await
                .0,
            StatusCode::UNAUTHORIZED
        );
        assert_eq!(
            send(&app, "POST", "/onboarding/checkpoint", Some("wrong"), BODY)
                .await
                .0,
            StatusCode::UNAUTHORIZED
        );
        assert_eq!(
            send(&app, "GET", "/onboarding/pending-nudges", None, "")
                .await
                .0,
            StatusCode::UNAUTHORIZED
        );
    }

    #[tokio::test]
    async fn a_checkpoint_is_recorded_and_validated() {
        let app = app(Some("k")).await;
        let (status, body) = send(&app, "POST", "/onboarding/checkpoint", Some("k"), BODY).await;
        assert_eq!(
            (status, body),
            (StatusCode::OK, serde_json::json!({ "success": true }))
        );
        let (status, body) = send(&app, "POST", "/onboarding/checkpoint", Some("k"), r#"{"checkpointId":9,"userIdentifier":"u","identifierType":"email","action":"reached"}"#).await;
        assert_eq!(
            (status, body["error"].as_str()),
            (
                StatusCode::BAD_REQUEST,
                Some("checkpointId must be an integer between 1 and 7")
            )
        );
        let (status, _) = send(&app, "POST", "/onboarding/checkpoint", Some("k"), r#"{"checkpointId":1,"userIdentifier":"u","identifierType":"email","action":"reached","extra":1}"#).await;
        assert_eq!(
            status,
            StatusCode::BAD_REQUEST,
            "unknown fields are refused"
        );
        let (status, body) = send(
            &app,
            "GET",
            "/onboarding/pending-nudges?sequence=1",
            Some("k"),
            "",
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["count"], 0, "reached a moment ago: nothing due yet");
        let (status, body) = send(
            &app,
            "GET",
            "/onboarding/pending-nudges?sequence=4",
            Some("k"),
            "",
        )
        .await;
        assert_eq!(
            (status, body["error"].as_str()),
            (StatusCode::BAD_REQUEST, Some("sequence must be 1, 2, or 3"))
        );
    }
}

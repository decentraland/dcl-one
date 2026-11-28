//! The onboarding checkpoints a client reports (`reached`, `completed`, 1-7)
//! and the nudges a mailer pulls (`pending-nudges?sequence=1..3`: 12, 24, 36 h
//! after the latest checkpoint of a user with an email), as the auth server's
//! onboarding component keeps them (`decentraland/auth-server`,
//! `src/ports/onboarding/component.ts`). The rows live in
//! `explore_api.onboarding_checkpoints`, so every replica reads what any one
//! wrote and a restart keeps them; a row is kept [`RETENTION_DAYS`] after it
//! was first reached (DATABASE.md, "Onboarding checkpoints workload") and a
//! checkpoint's metadata is at most [`MAX_METADATA_BYTES`] as JSON. Both
//! routes need `ONBOARDING_API_KEY` as a bearer (compared in constant time);
//! without the key they answer 401.

use crate::shared::routing::{get, post};
use axum::body::Bytes;
use axum::extract::{Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::{Json, Router};
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;

use crate::api::Shared as AppState;

const SEQUENCE_HOURS: [i64; 4] = [0, 12, 24, 36];
const MIN_CHECKPOINT: i64 = 1;
const MAX_CHECKPOINT: i64 = 7;
const MAX_USER_IDENTIFIER_LEN: usize = 255;
const MAX_EMAIL_LEN: usize = 255;
const MAX_WALLET_LEN: usize = 255;
const MAX_SOURCE_LEN: usize = 50;
/// A checkpoint's `metadata`, serialized compact.
pub const MAX_METADATA_BYTES: usize = 4 * 1024;
/// A row is deleted this long after its checkpoint was first reached.
pub const RETENTION_DAYS: i64 = 30;
/// The oldest rows a write looks at for expiry: more than one per insert, so
/// the deletions keep up with the rows the inserts of a retention ago left.
const SWEEP_BATCH: i64 = 16;

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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, sqlx::FromRow)]
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

/// A recorded checkpoint as [`OnboardingState::checkpoint_state`] reports it.
#[cfg(any(test, feature = "test-support"))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CheckpointState {
    pub completed: bool,
    pub email: Option<String>,
    pub wallet: Option<String>,
    pub metadata: Option<serde_json::Value>,
}

/// Why a checkpoint was not recorded.
#[derive(Debug)]
pub enum CheckpointError {
    /// The body breaks a rule of `POST /onboarding/checkpoint` (a 400).
    Invalid(String),
    /// The store failed (a 500).
    Store(sqlx::Error),
}

impl std::fmt::Display for CheckpointError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Invalid(msg) => f.write_str(msg),
            Self::Store(error) => write!(f, "onboarding store: {error}"),
        }
    }
}

impl std::error::Error for CheckpointError {}

/// The checkpoints in `explore_api.onboarding_checkpoints`, over the explore
/// pool: no process state, so any replica answers for every other.
pub struct OnboardingState {
    pool: PgPool,
}

/// The lowest checkpoint (then the oldest row) a wallet is bound to with an
/// email, through `onboarding_checkpoints_wallet_idx`.
const RESOLVE_WALLET: &str = "SELECT user_id, email FROM explore_api.onboarding_checkpoints \
     WHERE wallet = $1 AND email IS NOT NULL ORDER BY checkpoint, id LIMIT 1";

/// A `completed` marks the row once; email and wallet fill in when given.
const COMPLETE: &str = "UPDATE explore_api.onboarding_checkpoints \
     SET completed_at = $3, email = COALESCE($4, email), wallet = COALESCE($5, wallet) \
     WHERE user_id = $1 AND checkpoint = $2 AND completed_at IS NULL";

/// A `reached` inserts the row or fills in the email, wallet and metadata
/// given; `reached_at` keeps the first.
const UPSERT: &str = "INSERT INTO explore_api.onboarding_checkpoints \
     (user_id, checkpoint, email, wallet, reached_at, metadata) VALUES ($1, $2, $3, $4, $5, $6) \
     ON CONFLICT (user_id, checkpoint) DO UPDATE SET \
     email = COALESCE(EXCLUDED.email, onboarding_checkpoints.email), \
     wallet = COALESCE(EXCLUDED.wallet, onboarding_checkpoints.wallet), \
     metadata = COALESCE(EXCLUDED.metadata, onboarding_checkpoints.metadata)";

/// A `reached` of a wallet no email is bound to updates a row it already has
/// and inserts none: a row no nudge can reach.
const UPDATE_ONLY: &str = "UPDATE explore_api.onboarding_checkpoints \
     SET wallet = COALESCE($3, wallet), metadata = COALESCE($4, metadata) \
     WHERE user_id = $1 AND checkpoint = $2";

/// Reaching checkpoint N completes N-1.
const COMPLETE_PREVIOUS: &str = "UPDATE explore_api.onboarding_checkpoints \
     SET completed_at = $3 WHERE user_id = $1 AND checkpoint = $2 AND completed_at IS NULL";

/// Each user's highest checkpoint, open, with an email and reached before
/// `$1`, in insertion order: `onboarding_checkpoints_pending_idx` for the
/// candidates, the `(user_id, checkpoint)` key for the later-checkpoint probe.
const PENDING: &str = "SELECT oc.user_id, oc.checkpoint::bigint AS checkpoint_id, oc.email \
     FROM explore_api.onboarding_checkpoints oc \
     WHERE oc.email IS NOT NULL AND oc.completed_at IS NULL AND oc.reached_at < $1 \
     AND NOT EXISTS (SELECT 1 FROM explore_api.onboarding_checkpoints later \
                     WHERE later.user_id = oc.user_id AND later.checkpoint > oc.checkpoint) \
     ORDER BY oc.id";

/// The oldest rows by insertion order, deleted when past the retention.
/// `reached_at` follows `id` (both are taken at the insert), so the head of
/// the primary key is where expired rows are; a row another writer holds is
/// skipped, never waited for.
const SWEEP: &str = "DELETE FROM explore_api.onboarding_checkpoints WHERE reached_at < $1 \
     AND id IN (SELECT id FROM explore_api.onboarding_checkpoints ORDER BY id LIMIT $2 \
                FOR UPDATE SKIP LOCKED)";

impl OnboardingState {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// One checkpoint in one transaction: the wallet's resolution, the row's
    /// upsert and the previous checkpoint's completion commit together. The
    /// retention sweep follows, outside it.
    async fn record(&self, payload: CheckpointPayload, now: DateTime<Utc>) -> sqlx::Result<()> {
        let CheckpointPayload {
            mut user_identifier,
            identifier_type,
            checkpoint_id,
            action,
            mut email,
            wallet,
            metadata,
        } = payload;
        let checkpoint =
            i16::try_from(checkpoint_id).map_err(|e| sqlx::Error::Encode(Box::new(e)))?;
        let mut wallet = wallet.map(|w| w.to_lowercase());
        let by_wallet = identifier_type == "wallet";

        let mut tx = self.pool.begin().await?;
        if by_wallet {
            let lowered = user_identifier.to_lowercase();
            if email.is_none()
                && let Some((user, bound)) = sqlx::query_as::<_, (String, String)>(RESOLVE_WALLET)
                    .bind(&lowered)
                    .fetch_optional(&mut *tx)
                    .await?
            {
                user_identifier = user;
                email = Some(bound);
            }
            // The identifier is the wallet: a body without the `wallet` field
            // binds it too, so a later wallet-identified checkpoint finds the row.
            wallet = Some(wallet.unwrap_or(lowered));
        }

        if action == "completed" {
            sqlx::query(COMPLETE)
                .bind(&user_identifier)
                .bind(checkpoint)
                .bind(now)
                .bind(&email)
                .bind(&wallet)
                .execute(&mut *tx)
                .await?;
            return tx.commit().await;
        }

        let metadata = metadata.map(sqlx::types::Json);
        if by_wallet && email.is_none() {
            sqlx::query(UPDATE_ONLY)
                .bind(&user_identifier)
                .bind(checkpoint)
                .bind(&wallet)
                .bind(&metadata)
                .execute(&mut *tx)
                .await?;
        } else {
            sqlx::query(UPSERT)
                .bind(&user_identifier)
                .bind(checkpoint)
                .bind(&email)
                .bind(&wallet)
                .bind(now)
                .bind(&metadata)
                .execute(&mut *tx)
                .await?;
        }
        if checkpoint > 1 {
            sqlx::query(COMPLETE_PREVIOUS)
                .bind(&user_identifier)
                .bind(checkpoint - 1)
                .bind(now)
                .execute(&mut *tx)
                .await?;
        }
        tx.commit().await?;

        if let Err(error) = self.sweep(now).await {
            tracing::warn!(%error, "onboarding checkpoints: retention sweep failed");
        }
        Ok(())
    }

    /// Deletes up to [`SWEEP_BATCH`] of the oldest rows past the retention.
    async fn sweep(&self, now: DateTime<Utc>) -> sqlx::Result<u64> {
        let cutoff = now - Duration::days(RETENTION_DAYS);
        Ok(sqlx::query(SWEEP)
            .bind(cutoff)
            .bind(SWEEP_BATCH)
            .execute(&self.pool)
            .await?
            .rows_affected())
    }

    /// Validates `req` as `POST /onboarding/checkpoint` does, then records it.
    pub async fn record_checkpoint(&self, req: CheckpointRequest) -> Result<(), CheckpointError> {
        validate_checkpoint(&req).map_err(CheckpointError::Invalid)?;
        self.record(
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
        )
        .await
        .map_err(CheckpointError::Store)
    }

    /// The recorded checkpoint of `user_id`, for the tests of what records one.
    #[cfg(any(test, feature = "test-support"))]
    pub async fn checkpoint_state(
        &self,
        user_id: &str,
        checkpoint: i64,
    ) -> Option<CheckpointState> {
        type Row = (
            bool,
            Option<String>,
            Option<String>,
            Option<sqlx::types::Json<serde_json::Value>>,
        );
        let row: Option<Row> = sqlx::query_as(
            "SELECT completed_at IS NOT NULL, email, wallet, metadata \
             FROM explore_api.onboarding_checkpoints WHERE user_id = $1 AND checkpoint = $2",
        )
        .bind(user_id)
        .bind(i16::try_from(checkpoint).ok()?)
        .fetch_optional(&self.pool)
        .await
        .expect("reading an onboarding checkpoint");
        row.map(|(completed, email, wallet, metadata)| CheckpointState {
            completed,
            email,
            wallet,
            metadata: metadata.map(|m| m.0),
        })
    }

    async fn pending_nudges(
        &self,
        sequence: i64,
        now: DateTime<Utc>,
    ) -> sqlx::Result<Vec<PendingNudge>> {
        let index = usize::try_from(sequence).map_err(|e| sqlx::Error::Encode(Box::new(e)))?;
        let threshold = now - Duration::hours(SEQUENCE_HOURS[index]);
        sqlx::query_as(PENDING)
            .bind(threshold)
            .fetch_all(&self.pool)
            .await
    }
}

/// Under the `/onboarding` prefix: `POST /onboarding/checkpoint`,
/// `GET /onboarding/pending-nudges`, both behind `ONBOARDING_API_KEY`.
pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/checkpoint", post(post_checkpoint))
        .route("/pending-nudges", get(get_pending_nudges))
}

/// The body of `POST /onboarding/checkpoint`, also what the Segment webhooks
/// lift from a forwarded checkpoint event (`social::segment_webhooks`).
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CheckpointRequest {
    #[serde(rename = "checkpointId")]
    pub checkpoint_id: i64,
    #[serde(rename = "userIdentifier")]
    pub user_identifier: String,
    #[serde(rename = "identifierType")]
    pub identifier_type: String,
    pub action: String,
    #[serde(default)]
    pub email: Option<String>,
    #[serde(default)]
    pub wallet: Option<String>,
    #[serde(default)]
    pub source: Option<String>,
    #[serde(default)]
    pub metadata: Option<serde_json::Value>,
}

/// Whether a key or a string anywhere in `value` holds a NUL.
fn json_has_nul(value: &serde_json::Value) -> bool {
    match value {
        serde_json::Value::String(text) => text.contains('\0'),
        serde_json::Value::Array(items) => items.iter().any(json_has_nul),
        serde_json::Value::Object(fields) => fields
            .iter()
            .any(|(key, value)| key.contains('\0') || json_has_nul(value)),
        _ => false,
    }
}

fn validate_checkpoint(body: &CheckpointRequest) -> Result<(), String> {
    if body.checkpoint_id < MIN_CHECKPOINT || body.checkpoint_id > MAX_CHECKPOINT {
        return Err("checkpointId must be an integer between 1 and 7".to_string());
    }
    if body.identifier_type != "email" && body.identifier_type != "wallet" {
        return Err("identifierType must be one of 'email', 'wallet'".to_string());
    }
    // PostgreSQL stores no NUL in text or jsonb: refused here, not a 500 at the insert.
    let texts = [
        Some(&body.user_identifier),
        body.email.as_ref(),
        body.wallet.as_ref(),
        body.source.as_ref(),
    ];
    if texts.into_iter().flatten().any(|text| text.contains('\0')) {
        return Err("text fields must not contain NUL characters".to_string());
    }
    // A wallet identifier is stored lowercased, and lowercasing can lengthen a
    // string ('İ' is two characters lowercased): the stored form is measured.
    let ulen = if body.identifier_type == "wallet" {
        body.user_identifier.to_lowercase().chars().count()
    } else {
        body.user_identifier.chars().count()
    };
    if !(1..=MAX_USER_IDENTIFIER_LEN).contains(&ulen) {
        return Err("userIdentifier must be between 1 and 255 characters".to_string());
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
        && wallet.to_lowercase().chars().count() > MAX_WALLET_LEN
    {
        return Err("wallet must be at most 255 characters".to_string());
    }
    if let Some(source) = &body.source
        && source.chars().count() > MAX_SOURCE_LEN
    {
        return Err("source must be at most 50 characters".to_string());
    }
    if let Some(metadata) = &body.metadata {
        if !metadata.is_object() {
            return Err("metadata must be an object".to_string());
        }
        // jsonb refuses a NUL, in a key or a value alike.
        if json_has_nul(metadata) {
            return Err("metadata must not contain NUL characters".to_string());
        }
        if metadata.to_string().len() > MAX_METADATA_BYTES {
            return Err(format!(
                "metadata must be at most {MAX_METADATA_BYTES} bytes as JSON"
            ));
        }
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

    let req: CheckpointRequest = match serde_json::from_slice(&body) {
        Ok(req) => req,
        Err(e) => return bad_request(&e.to_string()),
    };
    match state.onboarding.record_checkpoint(req).await {
        Ok(()) => {}
        Err(CheckpointError::Invalid(msg)) => return bad_request(&msg),
        Err(error) => {
            tracing::error!(%error, "onboarding checkpoint not recorded");
            return internal_error("Internal server error");
        }
    }

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

    let nudges = match state.onboarding.pending_nudges(sequence, Utc::now()).await {
        Ok(nudges) => nudges,
        Err(error) => {
            tracing::error!(%error, "onboarding pending nudges not read");
            return internal_error("Failed to query pending nudges");
        }
    };
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

fn internal_error(msg: &str) -> Response {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(serde_json::json!({ "error": msg })),
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

    const H: i64 = 3600;

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

    fn completed(user: &str, checkpoint: i64) -> CheckpointPayload {
        CheckpointPayload {
            action: "completed".to_string(),
            ..reached(user, "email", checkpoint, None, None)
        }
    }

    /// A store over a database of its own (the nudge read is table-wide);
    /// `None` skips without `CATALYST_TEST_DATABASE_URL`.
    async fn store() -> Option<OnboardingState> {
        crate::db::scratch::isolated_pool(4)
            .await
            .map(OnboardingState::new)
    }

    impl OnboardingState {
        async fn put(&self, payload: CheckpointPayload, now: DateTime<Utc>) {
            self.record(payload, now).await.unwrap();
        }

        async fn nudges(&self, sequence: i64, now: DateTime<Utc>) -> Vec<PendingNudge> {
            self.pending_nudges(sequence, now).await.unwrap()
        }

        /// The user id and email of the one row at `checkpoint`.
        async fn row_at(&self, checkpoint: i64) -> Option<(String, Option<String>)> {
            sqlx::query_as(
                "SELECT user_id, email FROM explore_api.onboarding_checkpoints WHERE checkpoint = $1",
            )
            .bind(i16::try_from(checkpoint).unwrap())
            .fetch_optional(&self.pool)
            .await
            .unwrap()
        }

        async fn resolve(&self, wallet: &str) -> Option<(String, String)> {
            sqlx::query_as(RESOLVE_WALLET)
                .bind(wallet)
                .fetch_optional(&self.pool)
                .await
                .unwrap()
        }

        async fn rows(&self) -> i64 {
            sqlx::query_scalar("SELECT count(*) FROM explore_api.onboarding_checkpoints")
                .fetch_one(&self.pool)
                .await
                .unwrap()
        }
    }

    #[tokio::test]
    async fn reached_then_pending_after_interval() {
        let Some(store) = store().await else { return };
        store
            .put(reached("a@b.com", "email", 1, Some("a@b.com"), None), t(0))
            .await;

        assert!(store.nudges(1, t(60)).await.is_empty());
        assert_eq!(
            store.nudges(1, t(13 * H)).await,
            vec![PendingNudge {
                user_id: "a@b.com".to_string(),
                checkpoint_id: 1,
                email: "a@b.com".to_string(),
            }]
        );
        assert!(store.nudges(2, t(13 * H)).await.is_empty());
        assert_eq!(store.nudges(2, t(25 * H)).await.len(), 1);
        assert!(store.nudges(3, t(25 * H)).await.is_empty());
        assert_eq!(store.nudges(3, t(37 * H)).await.len(), 1);
    }

    #[tokio::test]
    async fn checkpoint_without_email_is_not_pending() {
        let Some(store) = store().await else { return };
        store.put(reached("u", "email", 1, None, None), t(0)).await;
        assert_eq!(store.rows().await, 1);
        assert!(store.nudges(1, t(13 * H)).await.is_empty());
    }

    #[tokio::test]
    async fn reaching_next_checkpoint_completes_previous_and_supersedes_it() {
        let Some(store) = store().await else { return };
        store
            .put(reached("a@b.com", "email", 1, Some("a@b.com"), None), t(0))
            .await;
        store
            .put(reached("a@b.com", "email", 2, Some("a@b.com"), None), t(10))
            .await;

        assert!(
            store
                .checkpoint_state("a@b.com", 1)
                .await
                .unwrap()
                .completed
        );
        let pending = store.nudges(1, t(13 * H)).await;
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].checkpoint_id, 2);
    }

    /// A user's highest checkpoint is the only one a nudge can target, even
    /// when a lower one is still open (reached out of order).
    #[tokio::test]
    async fn a_lower_open_checkpoint_is_superseded_by_a_higher_one() {
        let Some(store) = store().await else { return };
        store
            .put(reached("a@b.com", "email", 3, Some("a@b.com"), None), t(0))
            .await;
        store
            .put(reached("a@b.com", "email", 1, Some("a@b.com"), None), t(1))
            .await;
        let pending = store.nudges(1, t(13 * H)).await;
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].checkpoint_id, 3);
    }

    #[tokio::test]
    async fn completed_action_marks_completion() {
        let Some(store) = store().await else { return };
        store
            .put(reached("a@b.com", "email", 3, Some("a@b.com"), None), t(0))
            .await;
        assert_eq!(store.nudges(1, t(13 * H)).await.len(), 1);
        store.put(completed("a@b.com", 3), t(20)).await;
        assert!(store.nudges(1, t(13 * H)).await.is_empty());
        store.put(completed("nobody", 3), t(21)).await;
        assert_eq!(store.rows().await, 1, "a completed inserts no row");
    }

    #[tokio::test]
    async fn wallet_checkpoint_resolves_to_email_user_id() {
        let Some(store) = store().await else { return };
        store
            .put(
                reached("a@b.com", "email", 1, Some("a@b.com"), Some("0xABC")),
                t(0),
            )
            .await;
        store
            .put(reached("0xAbC", "wallet", 2, None, None), t(10))
            .await;

        assert_eq!(
            store.row_at(2).await,
            Some(("a@b.com".to_string(), Some("a@b.com".to_string())))
        );
        let pending = store.nudges(1, t(13 * H)).await;
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].checkpoint_id, 2);
        assert_eq!(pending[0].user_id, "a@b.com");
    }

    #[tokio::test]
    async fn upsert_coalesces_email_wallet_and_metadata() {
        let Some(store) = store().await else { return };
        let mut first = reached("u", "wallet", 1, Some("late@b.com"), None);
        first.metadata = Some(serde_json::json!({ "step": "profile" }));
        store.put(first, t(0)).await;
        store
            .put(reached("u", "wallet", 1, None, Some("0xabc")), t(5))
            .await;

        let row = store.checkpoint_state("u", 1).await.unwrap();
        assert_eq!(row.email.as_deref(), Some("late@b.com"));
        assert_eq!(row.wallet.as_deref(), Some("0xabc"));
        assert_eq!(row.metadata, Some(serde_json::json!({ "step": "profile" })));
        assert_eq!(
            store.nudges(1, t(12 * H + 1)).await.len(),
            1,
            "reached_at is the first reached, not the upsert's"
        );
    }

    /// An email body identified by the wallet (`identifierType: "wallet"`, no
    /// `wallet` field) binds that wallet, so the explorer's CP7 for it lands
    /// on the user's row.
    #[tokio::test]
    async fn an_email_body_identified_by_a_wallet_binds_that_wallet() {
        let Some(store) = store().await else { return };
        store
            .put(
                reached("0xABC", "wallet", 1, Some("late@b.com"), None),
                t(0),
            )
            .await;
        assert_eq!(
            store
                .checkpoint_state("0xABC", 1)
                .await
                .unwrap()
                .wallet
                .as_deref(),
            Some("0xabc")
        );
        store
            .put(reached("0xabc", "wallet", 7, None, None), t(5))
            .await;
        assert_eq!(
            store.row_at(7).await,
            Some(("0xABC".to_string(), Some("late@b.com".to_string())))
        );
    }

    #[tokio::test]
    async fn an_explicit_wallet_field_wins_over_the_identifier() {
        let Some(store) = store().await else { return };
        store
            .put(
                reached("0xabc", "wallet", 1, Some("a@b.com"), Some("0xDEF")),
                t(0),
            )
            .await;
        assert_eq!(
            store
                .checkpoint_state("0xabc", 1)
                .await
                .unwrap()
                .wallet
                .as_deref(),
            Some("0xdef")
        );
        assert_eq!(store.resolve("0xabc").await, None);
        assert_eq!(
            store.resolve("0xdef").await,
            Some(("0xabc".to_string(), "a@b.com".to_string()))
        );
    }

    #[tokio::test]
    async fn a_wallet_checkpoint_with_no_bound_email_leaves_no_row() {
        let Some(store) = store().await else { return };
        store
            .put(reached("0xabc", "wallet", 7, None, None), t(0))
            .await;
        assert_eq!(store.rows().await, 0, "a row no nudge can reach");
        store
            .put(
                reached("a@b.com", "email", 1, Some("a@b.com"), Some("0xabc")),
                t(1),
            )
            .await;
        store
            .put(reached("0xabc", "wallet", 7, None, None), t(2))
            .await;
        assert_eq!(
            store.row_at(7).await,
            Some(("a@b.com".to_string(), Some("a@b.com".to_string())))
        );
    }

    #[tokio::test]
    async fn wallet_learned_late_still_resolves_and_prefers_the_lowest_checkpoint() {
        let Some(store) = store().await else { return };
        store
            .put(reached("b@b.com", "email", 3, Some("b@b.com"), None), t(0))
            .await;
        store
            .put(reached("a@b.com", "email", 2, Some("a@b.com"), None), t(1))
            .await;
        store
            .put(reached("b@b.com", "email", 3, None, Some("0xABC")), t(2))
            .await;
        store
            .put(reached("a@b.com", "email", 2, None, Some("0xabc")), t(3))
            .await;

        assert_eq!(
            store.resolve("0xabc").await,
            Some(("a@b.com".to_string(), "a@b.com".to_string()))
        );
        store
            .put(reached("0xABC", "wallet", 4, None, None), t(10))
            .await;
        assert_eq!(
            store.row_at(4).await,
            Some(("a@b.com".to_string(), Some("a@b.com".to_string())))
        );
    }

    #[tokio::test]
    async fn nudges_keep_insertion_order() {
        let Some(store) = store().await else { return };
        for i in (0..40).rev() {
            let user = format!("user-{i:02}");
            store
                .put(reached(&user, "email", 1, Some(&user), None), t(40 - i))
                .await;
        }
        let users: Vec<String> = store
            .nudges(1, t(13 * H))
            .await
            .into_iter()
            .map(|n| n.user_id)
            .collect();
        let expected: Vec<String> = (0..40).rev().map(|i| format!("user-{i:02}")).collect();
        assert_eq!(users, expected);
    }

    /// The same checkpoint reported at once by many requests (replicas) is
    /// one row, and every request succeeds.
    #[tokio::test]
    async fn concurrent_reports_of_one_checkpoint_upsert_one_row() {
        let Some(store) = store().await else { return };
        let store = std::sync::Arc::new(store);
        let mut tasks = Vec::new();
        for i in 0..16 {
            let store = store.clone();
            tasks.push(tokio::spawn(async move {
                let mut payload = reached("a@b.com", "email", 2, Some("a@b.com"), None);
                payload.metadata = Some(serde_json::json!({ "n": i }));
                store.record(payload, t(i)).await
            }));
        }
        for task in tasks {
            task.await.unwrap().unwrap();
        }
        assert_eq!(store.rows().await, 1);
    }

    #[tokio::test]
    async fn rows_past_the_retention_are_swept_by_later_writes() {
        let Some(store) = store().await else { return };
        for i in 0..20 {
            let user = format!("old-{i:02}");
            store
                .put(reached(&user, "email", 1, Some(&user), None), t(i))
                .await;
        }
        let later = t(RETENTION_DAYS * 24 * H + 100);
        store
            .put(reached("new", "email", 1, Some("new@b.com"), None), later)
            .await;
        assert_eq!(store.rows().await, 21 - SWEEP_BATCH);
        store
            .put(reached("new", "email", 2, Some("new@b.com"), None), later)
            .await;
        assert_eq!(store.rows().await, 2, "only the rows within the retention");
        assert!(store.checkpoint_state("new", 1).await.is_some());
    }

    /// The nudge read and the wallet resolution reach their rows through the
    /// indexes (with sequential scans priced out, as on a table too large for
    /// one).
    #[tokio::test]
    async fn the_nudge_read_and_the_wallet_resolution_are_indexed() {
        let Some(store) = store().await else { return };
        let mut conn = store.pool.acquire().await.unwrap();
        sqlx::query("SET enable_seqscan = off")
            .execute(&mut *conn)
            .await
            .unwrap();
        let lines: Vec<String> =
            sqlx::query_scalar(sqlx::AssertSqlSafe(format!("EXPLAIN {PENDING}")))
                .bind(t(0))
                .fetch_all(&mut *conn)
                .await
                .unwrap();
        let pending = lines.join("\n");
        assert!(
            pending.contains("onboarding_checkpoints_pending_idx")
                && pending.contains("onboarding_checkpoints_user_checkpoint_key"),
            "{pending}"
        );
        let lines: Vec<String> =
            sqlx::query_scalar(sqlx::AssertSqlSafe(format!("EXPLAIN {RESOLVE_WALLET}")))
                .bind("0xabc")
                .fetch_all(&mut *conn)
                .await
                .unwrap();
        let resolve = lines.join("\n");
        assert!(
            resolve.contains("onboarding_checkpoints_wallet_idx"),
            "{resolve}"
        );
    }

    #[test]
    fn validation_caps_the_metadata_size() {
        let request = |metadata: serde_json::Value| CheckpointRequest {
            checkpoint_id: 1,
            user_identifier: "u".to_string(),
            identifier_type: "email".to_string(),
            action: "reached".to_string(),
            email: None,
            wallet: None,
            source: None,
            metadata: Some(metadata),
        };
        assert!(validate_checkpoint(&request(serde_json::json!({ "step": "profile" }))).is_ok());
        let padded = serde_json::json!({ "pad": "x".repeat(MAX_METADATA_BYTES) });
        assert_eq!(
            validate_checkpoint(&request(padded)).unwrap_err(),
            format!("metadata must be at most {MAX_METADATA_BYTES} bytes as JSON")
        );
    }

    #[test]
    fn validation_rejects_out_of_range_and_bad_enums() {
        let base = |cp: i64, id_type: &str, action: &str| CheckpointRequest {
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

    /// What PostgreSQL would refuse at the insert is a 400 here, not a 500 there.
    #[test]
    fn validation_refuses_what_the_table_cannot_store() {
        let base = || CheckpointRequest {
            checkpoint_id: 1,
            user_identifier: "u".to_string(),
            identifier_type: "wallet".to_string(),
            action: "reached".to_string(),
            email: None,
            wallet: None,
            source: None,
            metadata: None,
        };
        let nul = "a\0b".to_string();
        for set in [
            (|r: &mut CheckpointRequest, v: String| r.user_identifier = v)
                as fn(&mut CheckpointRequest, String),
            |r, v| r.email = Some(v),
            |r, v| r.wallet = Some(v),
            |r, v| r.source = Some(v),
        ] {
            let mut request = base();
            set(&mut request, nul.clone());
            assert_eq!(
                validate_checkpoint(&request).unwrap_err(),
                "text fields must not contain NUL characters"
            );
        }
        for metadata in [
            serde_json::json!({ "k": "a\0b" }),
            serde_json::json!({ "a\0b": 1 }),
            serde_json::json!({ "k": ["a\0"] }),
        ] {
            let mut request = base();
            request.metadata = Some(metadata);
            assert_eq!(
                validate_checkpoint(&request).unwrap_err(),
                "metadata must not contain NUL characters"
            );
        }
        // a literal backslash-u text is no NUL
        let mut escaped = base();
        escaped.metadata = Some(serde_json::json!({ "k": "\\u0000" }));
        assert!(validate_checkpoint(&escaped).is_ok());

        // 'İ' lowercases to two characters: 255 of them store as 510
        let mut wallet = base();
        wallet.wallet = Some("İ".repeat(MAX_WALLET_LEN));
        assert_eq!(
            validate_checkpoint(&wallet).unwrap_err(),
            "wallet must be at most 255 characters"
        );
        let mut identifier = base();
        identifier.user_identifier = "İ".repeat(MAX_USER_IDENTIFIER_LEN);
        assert!(validate_checkpoint(&identifier).is_err());
        identifier.identifier_type = "email".to_string();
        assert!(
            validate_checkpoint(&identifier).is_ok(),
            "an email identifier is stored as sent"
        );
    }
}

#[cfg(test)]
mod route_tests {
    use super::*;
    use axum::body::Body;
    use axum::http::Request;
    use sqlx::postgres::PgPoolOptions;
    use tower::ServiceExt;

    fn settings(key: Option<&str>) -> crate::api::Settings {
        let mut settings = crate::api::Settings::for_tests(crate::api::Sources::defaults());
        settings.onboarding_api_key = key.map(str::to_string);
        settings
    }

    fn mount(state: AppState) -> Router {
        Router::new().nest("/onboarding", routes().with_state(state))
    }

    async fn app(key: Option<&str>) -> Router {
        mount(crate::api::tests::node_without_db_state(settings(key)).await)
    }

    /// A replica: its own pool and state over the database `pool` names.
    async fn replica(pool: &PgPool, key: &str) -> (Router, AppState) {
        let pool = PgPoolOptions::new()
            .max_connections(4)
            .connect_with((*pool.connect_options()).clone())
            .await
            .unwrap();
        let state = crate::api::tests::node_with_db_state(settings(Some(key)), pool).await;
        (mount(state.clone()), state)
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

    /// Moves every checkpoint `hours` into the past, as time would.
    async fn age(pool: &PgPool, hours: i32) {
        sqlx::query(
            "UPDATE explore_api.onboarding_checkpoints SET reached_at = reached_at - make_interval(hours => $1)",
        )
        .bind(hours)
        .execute(pool)
        .await
        .unwrap();
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

    /// Validation answers before the store is reached: these hold with no
    /// database behind the node.
    #[tokio::test]
    async fn a_checkpoint_and_a_sequence_are_validated() {
        let app = app(Some("k")).await;
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

    /// The store unreachable is a 500 with the auth server's bodies.
    #[tokio::test]
    async fn a_store_failure_is_a_500() {
        let app = app(Some("k")).await;
        let (status, body) = send(&app, "POST", "/onboarding/checkpoint", Some("k"), BODY).await;
        assert_eq!(
            (status, body),
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                serde_json::json!({ "error": "Internal server error" })
            )
        );
        let (status, body) = send(&app, "GET", "/onboarding/pending-nudges", Some("k"), "").await;
        assert_eq!(
            (status, body),
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                serde_json::json!({ "error": "Failed to query pending nudges" })
            )
        );
    }

    /// The gateway sends each request to any replica: a checkpoint one
    /// records is a nudge the other lists, and a completion the other
    /// records clears it for the first.
    #[tokio::test]
    async fn replicas_share_the_checkpoints() {
        let Some(pool) = crate::db::scratch::isolated_pool(2).await else {
            return;
        };
        let (a, _) = replica(&pool, "k").await;
        let (b, _) = replica(&pool, "k").await;

        let (status, body) = send(&a, "POST", "/onboarding/checkpoint", Some("k"), BODY).await;
        assert_eq!(
            (status, body),
            (StatusCode::OK, serde_json::json!({ "success": true }))
        );
        let pending = "/onboarding/pending-nudges?sequence=1";
        let (status, body) = send(&b, "GET", pending, Some("k"), "").await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["count"], 0, "reached a moment ago: nothing due yet");

        age(&pool, 13).await;
        let (status, body) = send(&b, "GET", pending, Some("k"), "").await;
        assert_eq!(
            (status, body),
            (
                StatusCode::OK,
                serde_json::json!({
                    "sequence": 1,
                    "count": 1,
                    "nudges": [{ "userId": "a@b.com", "checkpointId": 1, "email": "a@b.com" }],
                })
            )
        );
        let (_, body) = send(&a, "GET", "/onboarding/pending-nudges", Some("k"), "").await;
        assert_eq!(body["sequence"], 1, "sequence defaults to 1");
        assert_eq!(body["count"], 1);

        let completed = r#"{"checkpointId":1,"userIdentifier":"a@b.com","identifierType":"email","action":"completed"}"#;
        assert_eq!(
            send(&b, "POST", "/onboarding/checkpoint", Some("k"), completed)
                .await
                .0,
            StatusCode::OK
        );
        let (_, body) = send(&a, "GET", pending, Some("k"), "").await;
        assert_eq!(body["count"], 0);
    }

    /// A new process (a restart, a new replica) reads what an earlier one wrote.
    #[tokio::test]
    async fn checkpoints_survive_a_new_state() {
        let Some(pool) = crate::db::scratch::isolated_pool(2).await else {
            return;
        };
        let (first, state) = replica(&pool, "k").await;
        let body = r#"{"checkpointId":2,"userIdentifier":"a@b.com","identifierType":"email","action":"reached","email":"a@b.com","wallet":"0xABC","metadata":{"step":"name"}}"#;
        assert_eq!(
            send(&first, "POST", "/onboarding/checkpoint", Some("k"), body)
                .await
                .0,
            StatusCode::OK
        );
        state.pool.close().await;
        drop((first, state));

        let (second, state) = replica(&pool, "k").await;
        assert_eq!(
            state.onboarding.checkpoint_state("a@b.com", 2).await,
            Some(CheckpointState {
                completed: false,
                email: Some("a@b.com".into()),
                wallet: Some("0xabc".into()),
                metadata: Some(serde_json::json!({ "step": "name" })),
            })
        );
        let cp7 = r#"{"checkpointId":7,"userIdentifier":"0xAbc","identifierType":"wallet","action":"reached"}"#;
        assert_eq!(
            send(&second, "POST", "/onboarding/checkpoint", Some("k"), cp7)
                .await
                .0,
            StatusCode::OK
        );
        age(&pool, 25).await;
        let (_, body) = send(
            &second,
            "GET",
            "/onboarding/pending-nudges?sequence=2",
            Some("k"),
            "",
        )
        .await;
        assert_eq!(
            body["nudges"],
            serde_json::json!([{ "userId": "a@b.com", "checkpointId": 7, "email": "a@b.com" }]),
            "the wallet bound before the restart resolves after it"
        );
    }
}

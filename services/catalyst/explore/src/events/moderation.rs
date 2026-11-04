//! Operator moderation of an event (the console's `events/moderate`,
//! `ops::admin::explore`): approve, reject/archive, feature/unfeature, or set
//! `approved`, `rejected`, `highlighted`, `trending`, `name`, `description`
//! directly, the old admin `PATCH /api/events/{id}` body and rules.
//!
//! The decision lands in `explore.event` (the `raw` document and the columns
//! the lists filter on: `approved`, `highlighted`, `trending`, `name`,
//! `description`), the row locked, and in one transaction with its record:
//! for an event created on this node (`origin = 'local'`) `explore.events_local`,
//! its audit (`signer` the operator, the console's signed-in wallet;
//! `signed_payload` the moderation as received); for a mirrored one
//! `explore.event_moderation`, the operator's flags (each with its actor and
//! reason) the mirror re-applies on every pass. A mirrored event's own fields (`name`, `description`) are the
//! upstream's, so a patch that sets them answers 409 naming the upstream. A
//! soft-deleted event answers not found, as before.
//!
//! An operator's event creation (the console's `events/create`, the old
//! admin `POST /api/events`): the old body (unknown fields refused), its
//! validation (name <= 150, description <= 5000, safe url and images, the
//! featured-item urn, start in RFC 3339, duration within 24 h and not
//! finished, Genesis coordinates unless a world) and document (pending
//! approval, created by `admin` as the old admin path wrote it), with the
//! place resolved by a query over `explore.place_indexed` instead of the old
//! HTTP lookup; the row (`origin = 'local'`) and its `events_local` audit
//! (the operator as signer) in one transaction. The users' own create, edit and delete are the
//! explore write surface's, not this module's.

use chrono::{DateTime, Duration, Utc};
use serde::Deserialize;
use serde_json::{Map, Value, json};
use sqlx::{PgPool, Postgres, Transaction};

use crate::shared::errors::ApiError;

/// The old admin patch body.
#[derive(Debug, Default, Clone, Deserialize, serde::Serialize)]
pub struct AdminPatch {
    #[serde(default)]
    pub action: Option<String>,
    #[serde(default)]
    pub approved: Option<bool>,
    #[serde(default)]
    pub rejected: Option<bool>,
    #[serde(default)]
    pub highlighted: Option<bool>,
    #[serde(default)]
    pub trending: Option<bool>,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    #[serde(alias = "rejection_reason")]
    pub reason: Option<String>,
    /// The old HTTP admin path's audit actor (the console passes its wallet instead).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub actor: Option<String>,
}

/// Both HTTP and console moderation accept the historical reason spelling.
/// An explicit `reason` key takes precedence, including an explicit null.
pub fn parse_admin_patch(mut value: Value) -> Result<AdminPatch, ApiError> {
    if let Some(fields) = value.as_object_mut() {
        normalize_admin_reason(fields);
    }
    serde_json::from_value(value)
        .map_err(|error| ApiError::bad_request(format!("invalid patch body: {error}")))
}

pub(crate) fn normalize_admin_reason(fields: &mut Map<String, Value>) {
    if fields.contains_key("reason") {
        fields.remove("rejection_reason");
    }
}

impl AdminPatch {
    /// Resolve actions once for both moderation-only and content edits.
    /// Explicit approval follows the action, then rejection, then feature flags.
    pub(crate) fn resolved(&self) -> Result<Self, ApiError> {
        if self
            .reason
            .as_ref()
            .is_some_and(|reason| reason.chars().count() > 500)
        {
            return Err(ApiError::bad_request("reason too long"));
        }
        let mut resolved = Self {
            name: self.name.clone(),
            description: self.description.clone(),
            reason: self.reason.clone(),
            actor: self.actor.clone(),
            ..Self::default()
        };
        match self.action.as_deref() {
            Some("approve") => {
                resolved.approved = Some(true);
                resolved.rejected = Some(false);
            }
            Some("reject" | "archive") => {
                resolved.approved = Some(false);
                resolved.rejected = Some(true);
            }
            Some("feature") => resolved.highlighted = Some(true),
            Some("unfeature") => resolved.highlighted = Some(false),
            Some(other) => {
                return Err(ApiError::bad_request(format!(
                    "unknown action \"{other}\" (expected approve|reject|feature|unfeature|archive)"
                )));
            }
            None => {}
        }
        if let Some(approved) = self.approved {
            resolved.approved = Some(approved);
            if approved {
                resolved.rejected = Some(false);
            }
        }
        if let Some(rejected) = self.rejected {
            resolved.rejected = Some(rejected);
            if rejected {
                resolved.approved = Some(false);
            }
        }
        if self.highlighted.is_some() {
            resolved.highlighted = self.highlighted;
        }
        if self.trending.is_some() {
            resolved.trending = self.trending;
        }
        Ok(resolved)
    }
}

fn set_approved(raw: &mut Map<String, Value>, actor: &str) {
    raw.insert("approved".into(), json!(true));
    raw.insert("approved_by".into(), json!(actor));
    raw.insert("rejected".into(), json!(false));
    raw.insert("rejected_by".into(), Value::Null);
    raw.insert("rejection_reason".into(), Value::Null);
}

fn set_rejected(raw: &mut Map<String, Value>, actor: &str, reason: Option<String>) {
    raw.insert("rejected_by".into(), json!(actor));
    raw.insert("approved".into(), json!(false));
    raw.insert("approved_by".into(), Value::Null);
    raw.insert("rejected".into(), json!(true));
    raw.insert(
        "rejection_reason".into(),
        reason.map(Value::from).unwrap_or(Value::Null),
    );
}

/// The old `apply_admin_state`: the action first, then the explicit fields;
/// nothing to apply is a 400.
pub fn apply_admin_state(
    raw: &mut Map<String, Value>,
    body: &AdminPatch,
    actor: &str,
) -> Result<(), ApiError> {
    let resolved = body.resolved()?;
    let body = &resolved;
    let mut handled = false;
    if let Some(v) = body.approved {
        if v {
            set_approved(raw, actor);
        } else {
            raw.insert("approved".into(), json!(false));
            raw.insert("approved_by".into(), Value::Null);
        }
        handled = true;
    }
    if let Some(v) = body.rejected {
        if v {
            set_rejected(raw, actor, body.reason.clone());
        } else {
            raw.insert("rejected".into(), json!(false));
            raw.insert("rejected_by".into(), Value::Null);
            raw.insert("rejection_reason".into(), Value::Null);
        }
        handled = true;
    }
    if let Some(v) = body.highlighted {
        raw.insert("highlighted".into(), json!(v));
        handled = true;
    }
    if let Some(v) = body.trending {
        raw.insert("trending".into(), json!(v));
        handled = true;
    }
    if let Some(name) = &body.name {
        raw.insert("name".into(), json!(name));
        handled = true;
    }
    if let Some(description) = &body.description {
        raw.insert("description".into(), json!(description));
        handled = true;
    }
    // Editing an existing rejection's explanation is independent of deciding
    // the verdict again, so retain its original actor and feature decisions.
    if body.reason.is_some() && flag(raw, "rejected") {
        raw.insert("rejection_reason".into(), json!(body.reason));
        handled = true;
    }
    if !handled {
        return Err(ApiError::bad_request("no moderation fields provided"));
    }
    Ok(())
}

pub(crate) fn is_soft_deleted(raw: &Map<String, Value>) -> bool {
    ["deleted_by_user", "deleted_by_admin"]
        .iter()
        .any(|k| raw.get(*k).and_then(Value::as_bool).unwrap_or(false))
}

pub(crate) fn flag(raw: &Map<String, Value>, key: &str) -> bool {
    raw.get(key).and_then(Value::as_bool).unwrap_or(false)
}

/// The answer to a write of a mirrored event's own fields: 409 naming the
/// upstream. The next pass writes the upstream's fields back, so a local
/// edit would either vanish or, kept by an overlay the mirror skips, freeze
/// the copy.
pub(crate) async fn mirrored_event(pool: &PgPool, event_id: &str) -> ApiError {
    let upstream: Option<String> =
        sqlx::query_scalar("SELECT source_url FROM explore.mirror_state WHERE mirror = $1")
            .bind(super::mirror::EVENTS_MIRROR)
            .fetch_optional(pool)
            .await
            .ok()
            .flatten();
    let upstream = upstream.unwrap_or_else(|| "its upstream".to_string());
    ApiError::conflict(format!(
        "The event {event_id} is mirrored from {upstream} and read-only on this node; edit it there"
    ))
}

/// Applies `body` to the event as `actor`. `Ok(false)` when there is no such
/// event (or it was deleted): the old route's 404; a patch of a mirrored
/// event's own fields answers [`mirrored_event`].
pub async fn moderate(
    pool: &PgPool,
    event_id: &str,
    body: &AdminPatch,
    actor: &str,
    settings: &crate::Settings,
) -> Result<bool, ApiError> {
    super::mirror::reclaim_local(pool).await?;
    let mut tx = pool.begin().await?;
    let row: Option<(Value, String, String)> =
        sqlx::query_as("SELECT raw, name, origin FROM explore.event WHERE id = $1 FOR UPDATE")
            .bind(event_id)
            .fetch_optional(&mut *tx)
            .await?;
    let Some((raw, current_name, origin)) = row else {
        return Ok(false);
    };
    let before = raw.clone();
    let mut raw = match raw {
        Value::Object(m) => m,
        _ => Map::new(),
    };
    if is_soft_deleted(&raw) {
        return Ok(false);
    }
    let mirrored = origin != LOCAL;
    if mirrored && (body.name.is_some() || body.description.is_some()) {
        drop(tx);
        return Err(mirrored_event(pool, event_id).await);
    }
    apply_admin_state(&mut raw, body, actor)?;
    if !mirrored {
        begin_metric_approval(&mut tx, before.as_object().unwrap_or(&Map::new()), &mut raw).await?;
    }
    if mirrored && body.reason.is_some() && flag(&raw, "rejected") {
        let resolved = body.resolved()?;
        if resolved.rejected != Some(true) {
            // Keep the ownership check and local moderation update in the
            // same row-locked transaction. A feature-only overlay does not
            // grant authority over the upstream's rejection decision.
            let locally_rejected: bool = sqlx::query_scalar(
                "SELECT COALESCE(rejected, false) FROM explore.event_moderation WHERE event_id=$1",
            )
            .bind(event_id)
            .fetch_optional(&mut *tx)
            .await?
            .unwrap_or(false);
            if !locally_rejected {
                return Err(ApiError::conflict(
                    "This rejection belongs to the upstream; make an explicit local rejection decision before editing its reason",
                ));
            }
        }
    }
    let decision = mirrored.then(|| Decision::of(body, &raw));
    raw.insert("updated_at".into(), json!(Utc::now().to_rfc3339()));
    let name = raw
        .get("name")
        .and_then(Value::as_str)
        .map(str::to_string)
        .unwrap_or(current_name);
    let description = raw
        .get("description")
        .and_then(Value::as_str)
        .map(str::to_string);
    let (approved, highlighted, trending) = (
        flag(&raw, "approved"),
        flag(&raw, "highlighted"),
        flag(&raw, "trending"),
    );
    super::notifications::record_transition(
        &mut tx,
        event_id,
        Some(&before),
        &Value::Object(raw.clone()),
        !mirrored,
        settings,
    )
    .await?;
    sqlx::query(
        "UPDATE explore.event SET raw = $2, name = $3, description = $4, approved = $5, highlighted = $6, trending = $7 \
         WHERE id = $1",
    )
    .bind(event_id)
    .bind(Value::Object(raw))
    .bind(&name)
    .bind(description)
    .bind(approved)
    .bind(highlighted)
    .bind(trending)
    .execute(&mut *tx)
    .await?;
    match decision {
        Some(d) => d.record(&mut tx, event_id, actor).await?,
        None => {
            sqlx::query(
                "INSERT INTO explore.events_local (id, signer, signed_payload) VALUES ($1, $2, $3) \
                 ON CONFLICT (id) DO UPDATE SET signer = EXCLUDED.signer, signed_payload = EXCLUDED.signed_payload, \
                     updated_at = timezone('UTC', now())",
            )
            .bind(event_id)
            .bind(actor)
            .bind(json!({ "moderation": body }))
            .execute(&mut *tx)
            .await?;
        }
    }
    tx.commit().await?;
    tracing::info!(event = %event_id, actor, mirrored, "event moderation recorded");
    Ok(true)
}

/// The flags a patch decided on a mirrored event, as the patch left them in
/// `raw`; `None`: not touched, the upstream's value keeps showing. Each
/// decided flag carries its actor and reason as `raw` holds them (an
/// approval its approver, a rejection its rejecter and reason, a delete its
/// actor and reason); a flag decided the other way carries none.
#[derive(Debug, Default, PartialEq)]
pub(crate) struct Decision {
    approved: Option<bool>,
    rejected: Option<bool>,
    highlighted: Option<bool>,
    trending: Option<bool>,
    hidden: Option<bool>,
    approved_by: Option<String>,
    rejected_by: Option<String>,
    rejection_reason: Option<String>,
    deleted_by: Option<String>,
    deleted_reason: Option<String>,
}

fn text(raw: &Map<String, Value>, key: &str) -> Option<String> {
    raw.get(key).and_then(Value::as_str).map(str::to_string)
}

impl Decision {
    /// What [`apply_admin_state`] touched: approve and reject set both
    /// `approved` and `rejected`, as does `approved: true` or
    /// `rejected: true`.
    fn of(body: &AdminPatch, raw: &Map<String, Value>) -> Self {
        let action = body.action.as_deref();
        let verdict = matches!(action, Some("approve" | "reject" | "archive"))
            || body.approved == Some(true)
            || body.rejected == Some(true);
        let pick = |touched: bool, key: &str| touched.then(|| flag(raw, key));
        let reason_edit = body.reason.is_some() && flag(raw, "rejected");
        let approved = pick(
            verdict || body.approved.is_some() || reason_edit,
            "approved",
        );
        let rejected = pick(
            verdict || body.rejected.is_some() || reason_edit,
            "rejected",
        );
        let with = |decided: Option<bool>, key: &str| {
            decided.unwrap_or(false).then(|| text(raw, key)).flatten()
        };
        Self {
            approved,
            rejected,
            highlighted: pick(
                matches!(action, Some("feature" | "unfeature")) || body.highlighted.is_some(),
                "highlighted",
            ),
            trending: pick(body.trending.is_some(), "trending"),
            hidden: None,
            approved_by: with(approved, "approved_by"),
            rejected_by: with(rejected, "rejected_by"),
            rejection_reason: with(rejected, "rejection_reason"),
            deleted_by: None,
            deleted_reason: None,
        }
    }

    /// Merged into the event's `event_moderation` row: a flag this decision
    /// leaves `None` keeps the earlier one, with its actor and reason.
    async fn record(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        event_id: &str,
        actor: &str,
    ) -> Result<(), ApiError> {
        sqlx::query(
            "INSERT INTO explore.event_moderation AS m \
                 (event_id, approved, rejected, highlighted, trending, hidden, set_by, \
                  approved_by, rejected_by, rejection_reason, deleted_by, deleted_reason) \
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12) \
             ON CONFLICT (event_id) DO UPDATE SET \
                 approved = COALESCE(EXCLUDED.approved, m.approved), \
                 rejected = COALESCE(EXCLUDED.rejected, m.rejected), \
                 highlighted = COALESCE(EXCLUDED.highlighted, m.highlighted), \
                 trending = COALESCE(EXCLUDED.trending, m.trending), \
                 hidden = COALESCE(EXCLUDED.hidden, m.hidden), \
                 approved_by = CASE WHEN EXCLUDED.approved IS NULL THEN m.approved_by ELSE EXCLUDED.approved_by END, \
                 rejected_by = CASE WHEN EXCLUDED.rejected IS NULL THEN m.rejected_by ELSE EXCLUDED.rejected_by END, \
                 rejection_reason = CASE WHEN EXCLUDED.rejected IS NULL THEN m.rejection_reason ELSE EXCLUDED.rejection_reason END, \
                 deleted_by = CASE WHEN EXCLUDED.hidden IS NULL THEN m.deleted_by ELSE EXCLUDED.deleted_by END, \
                 deleted_reason = CASE WHEN EXCLUDED.hidden IS NULL THEN m.deleted_reason ELSE EXCLUDED.deleted_reason END, \
                 set_by = EXCLUDED.set_by, set_at = timezone('UTC', now())",
        )
        .bind(event_id)
        .bind(self.approved)
        .bind(self.rejected)
        .bind(self.highlighted)
        .bind(self.trending)
        .bind(self.hidden)
        .bind(actor)
        .bind(&self.approved_by)
        .bind(&self.rejected_by)
        .bind(&self.rejection_reason)
        .bind(&self.deleted_by)
        .bind(&self.deleted_reason)
        .execute(&mut **tx)
        .await?;
        Ok(())
    }
}

/// An operator's delete of a mirrored event: `raw` (the soft-delete keys
/// set) projected onto the row and `hidden` recorded, which keeps it from
/// every public read across passes; the console still sees it.
pub(super) async fn hide_mirrored_locked(
    tx: &mut Transaction<'_, Postgres>,
    event_id: &str,
    before: &Map<String, Value>,
    raw: Map<String, Value>,
    actor: &str,
    settings: &crate::Settings,
) -> Result<(), ApiError> {
    let decision = Decision {
        hidden: Some(true),
        deleted_by: text(&raw, "deleted_by"),
        deleted_reason: text(&raw, "deleted_reason"),
        ..Default::default()
    };
    super::notifications::record_transition(
        tx,
        event_id,
        Some(&Value::Object(before.clone())),
        &Value::Object(raw.clone()),
        false,
        settings,
    )
    .await?;
    sqlx::query("UPDATE explore.event SET raw = $2 WHERE id = $1")
        .bind(event_id)
        .bind(Value::Object(raw))
        .execute(&mut **tx)
        .await?;
    decision.record(tx, event_id, actor).await?;
    tracing::info!(event = %event_id, actor, "mirrored event hidden");
    Ok(())
}

/// `explore.event.origin` of an event created on this node; every other row
/// is the mirror's (`'upstream'`).
pub(crate) const LOCAL: &str = "local";

/// The creator the old admin path wrote (`ADMIN_SIGNER`); the operator's
/// wallet is the overlay's signer and the audit row's actor.
pub const ADMIN_CREATOR: &str = "admin";
pub(crate) const MAX_EVENT_DESCRIPTION_CHARS: usize = 5000;
pub(crate) const MAX_EVENT_CONTACT_CHARS: usize = 100;
pub(crate) const MAX_EVENT_DURATION_MS: i64 = 1000 * 60 * 60 * 24;
pub(crate) const PAST_FINISH_AT_MESSAGE: &str = "The event end date is already in the past";
pub(crate) const PAST_RECURRENT_UNTIL_MESSAGE: &str =
    "The recurrence end date (Until) must be in the future";
const MAX_FEATURED_ITEM_LEN: usize = 160;
const FEATURED_ITEM_CHAINS: [&str; 4] = ["matic", "ethereum", "amoy", "sepolia"];

/// The old admin create body.
#[derive(Debug, Clone, Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub struct CreateEvent {
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub image: Option<String>,
    #[serde(default)]
    pub image_vertical: Option<String>,
    pub start_at: String,
    pub duration: i64,
    #[serde(default)]
    pub all_day: Option<bool>,
    #[serde(default)]
    pub recurrent: Option<bool>,
    #[serde(default)]
    pub recurrent_frequency: Option<String>,
    #[serde(default)]
    pub recurrent_setpos: Option<i64>,
    #[serde(default)]
    pub recurrent_monthday: Option<i64>,
    #[serde(default)]
    pub recurrent_weekday_mask: Option<i64>,
    #[serde(default)]
    pub recurrent_month_mask: Option<i64>,
    #[serde(default)]
    pub recurrent_interval: Option<i64>,
    #[serde(default)]
    pub recurrent_count: Option<i64>,
    #[serde(default)]
    pub recurrent_until: Option<String>,
    pub x: i32,
    pub y: i32,
    #[serde(default)]
    pub server: Option<String>,
    #[serde(default)]
    pub contact: Option<String>,
    #[serde(default)]
    pub details: Option<String>,
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub categories: Option<Vec<String>>,
    #[serde(default)]
    pub schedules: Option<Vec<String>>,
    #[serde(default)]
    pub world: Option<bool>,
    #[serde(default)]
    pub community_id: Option<String>,
    #[serde(default)]
    pub featured_item: Option<String>,
}

pub(crate) fn parse_rfc3339(s: &str) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(s)
        .ok()
        .map(|d| d.with_timezone(&Utc))
}

pub fn validate_featured_item(urn: &str) -> Result<(), String> {
    if urn.chars().count() > MAX_FEATURED_ITEM_LEN {
        return Err("featured_item must be at most 160 characters".into());
    }
    let invalid = || "featured_item must be a collections-v2 item or collection urn".to_string();
    let rest = urn.strip_prefix("urn:decentraland:").ok_or_else(invalid)?;
    let (chain, rest) = rest.split_once(':').ok_or_else(invalid)?;
    if !FEATURED_ITEM_CHAINS.contains(&chain) {
        return Err(invalid());
    }
    let rest = rest.strip_prefix("collections-v2:0x").ok_or_else(invalid)?;
    let (address, item) = match rest.split_once(':') {
        Some((address, item)) => (address, Some(item)),
        None => (rest, None),
    };
    if address.len() != 40 || !address.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(invalid());
    }
    if let Some(item) = item
        && (item.is_empty() || !item.bytes().all(|b| b.is_ascii_digit()))
    {
        return Err(invalid());
    }
    Ok(())
}

pub(crate) fn max_duration_message() -> String {
    format!(
        "Maximum allowed duration {}Hrs",
        MAX_EVENT_DURATION_MS / 3_600_000
    )
}

pub(crate) fn finish_at_of(start_at: DateTime<Utc>, duration_ms: i64) -> Option<DateTime<Utc>> {
    Duration::try_milliseconds(duration_ms.max(0)).and_then(|d| start_at.checked_add_signed(d))
}

pub(crate) fn normalized_series(
    recurrent: bool,
    frequency: Option<&str>,
    count: Option<i64>,
    until: Option<DateTime<Utc>>,
) -> bool {
    recurrent && frequency.is_some_and(|f| !f.is_empty()) && (count.is_some() || until.is_some())
}

pub(crate) fn series_may_have_future_dates(
    recurrent: bool,
    frequency: Option<&str>,
    count: Option<i64>,
    until: Option<DateTime<Utc>>,
    now: DateTime<Utc>,
) -> bool {
    normalized_series(recurrent, frequency, count, until) && until.is_none_or(|u| u > now)
}

/// The old `validate_create`: the start instant, or the 400.
pub fn validate_create(body: &CreateEvent, now: DateTime<Utc>) -> Result<DateTime<Utc>, ApiError> {
    if body.name.chars().count() > 150 {
        return Err(ApiError::bad_request("name must be at most 150 characters"));
    }
    if body
        .description
        .as_ref()
        .is_some_and(|d| d.chars().count() > MAX_EVENT_DESCRIPTION_CHARS)
    {
        return Err(ApiError::bad_request("description too long"));
    }
    if body
        .details
        .as_ref()
        .is_some_and(|value| value.chars().count() > MAX_EVENT_DESCRIPTION_CHARS)
        || body
            .contact
            .as_ref()
            .is_some_and(|value| value.chars().count() > MAX_EVENT_CONTACT_CHARS)
    {
        return Err(ApiError::bad_request(
            "organizer text exceeds the maximum length",
        ));
    }
    if let Some(u) = &body.url {
        crate::sanitize::validate_event_url(u).map_err(ApiError::bad_request)?;
    }
    for img in [&body.image, &body.image_vertical].into_iter().flatten() {
        crate::sanitize::sanitize_asset_url(img).map_err(ApiError::bad_request)?;
    }
    if let Some(urn) = body.featured_item.as_deref().filter(|v| !v.is_empty()) {
        validate_featured_item(urn).map_err(ApiError::bad_request)?;
    }
    if body.duration < 0 {
        return Err(ApiError::bad_request("duration must be positive"));
    }
    let start_at =
        parse_rfc3339(&body.start_at).ok_or_else(|| ApiError::bad_request("invalid start_at"))?;
    if !(-170..=170).contains(&body.x) || !(-170..=170).contains(&body.y) {
        return Err(ApiError::bad_request("coordinates out of range"));
    }
    let world = body.world.unwrap_or(false);
    if !world && !super::handlers::is_inside_world_limits(body.x, body.y) {
        return Err(ApiError::bad_request("coordinates outside world limits"));
    }
    let recurrent = body.recurrent.unwrap_or(false);
    let frequency = body.recurrent_frequency.as_deref();
    let until = body.recurrent_until.as_deref().and_then(parse_rfc3339);
    if !series_may_have_future_dates(recurrent, frequency, body.recurrent_count, until, now)
        && finish_at_of(start_at, body.duration).is_some_and(|finish_at| finish_at <= now)
    {
        return Err(ApiError::bad_request(PAST_FINISH_AT_MESSAGE));
    }
    if normalized_series(recurrent, frequency, body.recurrent_count, until)
        && until.is_some_and(|u| u <= now)
    {
        return Err(ApiError::bad_request(PAST_RECURRENT_UNTIL_MESSAGE));
    }
    if body.duration > MAX_EVENT_DURATION_MS {
        return Err(ApiError::bad_request(max_duration_message()));
    }
    Ok(start_at)
}

/// The old `build_create_raw`: the event document a create stores, pending
/// approval, its one occurrence the start.
pub fn build_create_raw(
    body: &CreateEvent,
    creator: &str,
    start_at: DateTime<Utc>,
    place_id: Option<&str>,
    world: bool,
    now: DateTime<Utc>,
) -> Result<Value, ApiError> {
    let span = Duration::try_milliseconds(body.duration.max(0))
        .ok_or_else(|| ApiError::bad_request(max_duration_message()))?;
    let finish_at = finish_at_of(start_at, body.duration)
        .ok_or_else(|| ApiError::bad_request(max_duration_message()))?;
    let next_finish = start_at
        .checked_add_signed(span)
        .ok_or_else(|| ApiError::bad_request(max_duration_message()))?;
    let url = body
        .url
        .clone()
        .filter(|u| !u.is_empty())
        .unwrap_or_else(|| {
            super::metadata::jump_in_url(world, body.server.as_deref(), body.x, body.y)
        });
    let mut seen = std::collections::HashSet::new();
    let schedules: Vec<String> = body
        .schedules
        .clone()
        .unwrap_or_default()
        .into_iter()
        .filter(|s| seen.insert(s.clone()))
        .collect();
    let core = json!({
        "id": null,
        "name": body.name,
        "image": body.image,
        "image_vertical": body.image_vertical,
        "description": body.description,
        "start_at": start_at.to_rfc3339(),
        "finish_at": finish_at.to_rfc3339(),
        "next_start_at": start_at.to_rfc3339(),
        "next_finish_at": next_finish.to_rfc3339(),
        "duration": body.duration,
        "all_day": body.all_day.unwrap_or(false),
        "x": body.x,
        "y": body.y,
        "server": body.server,
        "url": url,
        "user": creator,
        "user_name": null,
        "estate_id": null,
        "estate_name": null,
        "scene_name": null,
    });
    let recurrence = json!({
        "approved": false,
        "rejected": false,
        "highlighted": false,
        "trending": false,
        "world": world,
        "recurrent": body.recurrent.unwrap_or(false),
        "recurrent_frequency": body.recurrent_frequency,
        "recurrent_setpos": body.recurrent_setpos,
        "recurrent_monthday": body.recurrent_monthday,
        "recurrent_weekday_mask": body.recurrent_weekday_mask.unwrap_or(0),
        "recurrent_month_mask": body.recurrent_month_mask.unwrap_or(0),
        "recurrent_interval": body.recurrent_interval.unwrap_or(1),
        "recurrent_count": body.recurrent_count,
        "recurrent_until": body.recurrent_until,
        "recurrent_dates": [start_at.to_rfc3339()],
        "categories": body.categories.clone().unwrap_or_default(),
        "schedules": schedules,
        "contact": body.contact,
        "details": body.details,
    });
    let attendance = json!({
        "total_attendees": 0,
        "latest_attendees": [],
        "coordinates": [body.x, body.y],
        "position": [body.x, body.y],
        "live": false,
        "attending": false,
        "place_id": place_id,
        "community_id": body.community_id,
        "featured_item": body.featured_item.as_deref().filter(|v| !v.is_empty()),
        "created_at": now.to_rfc3339(),
        "updated_at": now.to_rfc3339(),
        "approved_by": null,
        "rejected_by": null,
        "rejection_reason": null,
        "deleted_by_user": false,
        "deleted_by_admin": false,
        "deleted_by": null,
        "deleted_at": null,
        "deleted_reason": null,
        "previous_place_id": null,
    });
    let mut doc = Map::new();
    for part in [core, recurrence, attendance] {
        if let Value::Object(m) = part {
            doc.extend(m);
        }
    }
    super::recurrence::recompute(&mut doc, now)?;
    Ok(Value::Object(doc))
}

fn metric_starts(raw: &Map<String, Value>) -> Vec<(String, DateTime<Utc>)> {
    let values: Vec<&str> = match raw.get("recurrent_dates").and_then(Value::as_array) {
        Some(dates) if !dates.is_empty() => dates.iter().filter_map(Value::as_str).collect(),
        _ => raw
            .get("start_at")
            .and_then(Value::as_str)
            .into_iter()
            .collect(),
    };
    values
        .into_iter()
        .filter_map(|value| {
            DateTime::parse_from_rfc3339(value)
                .ok()
                .map(|date| (value.to_owned(), date.with_timezone(&Utc)))
        })
        .collect()
}

/// Start a create/approval window without importing already-ended occurrences.
/// A first occurrence already in progress remains eligible when it later ends.
fn begin_metric_window(
    raw: &mut Map<String, Value>,
    at: DateTime<Utc>,
    already_eligible: Option<&std::collections::BTreeSet<DateTime<Utc>>>,
) {
    let duration =
        chrono::Duration::milliseconds(raw.get("duration").and_then(Value::as_i64).unwrap_or(0));
    let preserved: Map<String, Value> = metric_starts(raw)
        .into_iter()
        .filter(|(_, start)| {
            *start < at
                && already_eligible.is_none_or(|eligible| eligible.contains(start))
                && start
                    .checked_add_signed(duration)
                    .is_some_and(|end| end > at)
        })
        .map(|(key, _)| (key, Value::Bool(true)))
        .collect();
    raw.insert("_metrics_schedule_after".into(), json!(at.to_rfc3339()));
    raw.insert("_metrics_preserved_starts".into(), Value::Object(preserved));
}

async fn begin_metric_approval(
    tx: &mut Transaction<'_, Postgres>,
    before: &Map<String, Value>,
    raw: &mut Map<String, Value>,
) -> Result<(), sqlx::Error> {
    if !flag(before, "approved")
        && flag(raw, "approved")
        && !flag(raw, "rejected")
        && !is_soft_deleted(raw)
    {
        let at: DateTime<Utc> = sqlx::query_scalar("SELECT clock_timestamp()")
            .fetch_one(&mut **tx)
            .await?;
        let eligible = eligible_metric_starts(raw);
        begin_metric_window(raw, at, Some(&eligible));
    }
    Ok(())
}

fn eligible_metric_starts(raw: &Map<String, Value>) -> std::collections::BTreeSet<DateTime<Utc>> {
    let old_cutoff = raw
        .get("_metrics_schedule_after")
        .and_then(Value::as_str)
        .and_then(|value| DateTime::parse_from_rfc3339(value).ok());
    let old_preserved = raw
        .get("_metrics_preserved_starts")
        .and_then(Value::as_object);
    metric_starts(raw)
        .into_iter()
        .filter(|(key, date)| {
            old_cutoff.is_none_or(|at| *date >= at)
                || old_preserved.is_some_and(|old| old.contains_key(key))
        })
        .map(|(_, date)| date)
        .collect()
}

/// Preserve eligible occurrence identities shared by the old and new schedules.
/// The private lookup is bounded by the current materialized schedule; it never
/// accumulates removed dates across edits or grants eligibility to new past dates.
pub(super) fn preserve_metric_starts(
    before: &Map<String, Value>,
    after: &mut Map<String, Value>,
    cutoff: DateTime<Utc>,
) {
    let eligible = eligible_metric_starts(before);
    let preserved: Map<String, Value> = metric_starts(after)
        .into_iter()
        .filter(|(_, date)| *date < cutoff && eligible.contains(date))
        .map(|(key, _)| (key, Value::Bool(true)))
        .collect();
    after.insert("_metrics_schedule_after".into(), json!(cutoff.to_rfc3339()));
    after.insert("_metrics_preserved_starts".into(), Value::Object(preserved));
}

/// Event identifiers stay unique even when concurrent creates share a clock tick.
fn generate_id() -> String {
    uuid::Uuid::new_v4().to_string()
}

/// The old `resolve_location` as a query: a world event names its world by
/// `server` (the world's name, lowercased, when the catalogue knows it); a
/// parcel event takes the enabled place whose positions hold the parcel
/// (`place_positions_path_gin`), the newest deployment first.
pub(crate) async fn resolve_location<'e, E>(
    executor: E,
    world: bool,
    server: Option<&str>,
    x: i32,
    y: i32,
) -> Result<(Option<String>, bool), ApiError>
where
    E: sqlx::PgExecutor<'e>,
{
    match server.filter(|s| world && !s.is_empty()) {
        Some(name) => {
            let found: Option<(String,)> = sqlx::query_as(
                "SELECT lower(world_name) FROM explore.place_indexed WHERE world IS TRUE AND lower(world_name) = lower($1) LIMIT 1",
            )
            .bind(name)
            .fetch_optional(executor)
            .await?;
            Ok((found.map(|(n,)| n), true))
        }
        None => {
            let found: Option<(String,)> = sqlx::query_as(
                "SELECT id FROM explore.place_indexed WHERE world IS FALSE AND disabled IS FALSE AND raw->'positions' @> $1::jsonb \
                 ORDER BY deployed_at DESC NULLS LAST, id LIMIT 1",
            )
            .bind(json!([format!("{x},{y}")]))
            .fetch_optional(executor)
            .await?;
            Ok((found.map(|(id,)| id), false))
        }
    }
}

/// A community event may only be linked by an owner or moderator of that
/// active community. This uses the shared membership authority, not a claim
/// supplied by the event form.
pub(super) async fn validate_community<'e, E>(
    executor: E,
    id: &str,
    signer: &str,
) -> Result<(), ApiError>
where
    E: sqlx::PgExecutor<'e>,
{
    let id =
        uuid::Uuid::parse_str(id).map_err(|_| ApiError::bad_request("invalid community_id"))?;
    let managed: bool = sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM social.communities c \
         LEFT JOIN social.community_members m ON m.community_id = c.id AND m.member_address = $2 \
         WHERE c.id = $1 AND c.active AND NOT c.suspended \
           AND (c.owner_address = $2 OR m.role IN ('owner', 'moderator')))",
    )
    .bind(id)
    .bind(signer.to_ascii_lowercase())
    .fetch_one(executor)
    .await?;
    if !managed {
        return Err(ApiError::forbidden(
            "You don't have permission to link this community",
        ));
    }
    Ok(())
}

/// Validate catalogue references before committing an event. Arrays are
/// bounded at the boundary, and the schedule check is one set query.
pub(super) async fn validate_references<'e, E>(
    executor: E,
    raw: &Map<String, Value>,
) -> Result<(), ApiError>
where
    E: sqlx::PgExecutor<'e>,
{
    if let Some(categories) = raw.get("categories").and_then(Value::as_array) {
        let known = super::categories::list();
        if categories.len() > 25
            || categories.iter().any(|category| {
                !known
                    .iter()
                    .any(|known| category.as_str() == Some(&known.name))
            })
        {
            return Err(ApiError::bad_request("Invalid category tag supplied"));
        }
    }
    if let Some(schedules) = raw.get("schedules").and_then(Value::as_array) {
        if schedules.len() > 100 {
            return Err(ApiError::bad_request("too many schedules"));
        }
        let schedules: Vec<String> = schedules
            .iter()
            .filter_map(Value::as_str)
            .map(str::to_owned)
            .collect();
        if !schedules.is_empty() {
            let count: i64 = sqlx::query_scalar(
                "SELECT count(*) FROM explore.schedules_local WHERE id = ANY($1)",
            )
            .bind(&schedules)
            .fetch_one(executor)
            .await?;
            if count != schedules.len() as i64 {
                return Err(ApiError::bad_request("Schedule not found"));
            }
        }
    }
    Ok(())
}

/// Creates the event as the old admin path did, the operator as the
/// overlay's signer; answers its id.
pub async fn create(
    pool: &PgPool,
    body: &CreateEvent,
    operator: &str,
    settings: &crate::Settings,
) -> Result<String, ApiError> {
    create_as(pool, body, ADMIN_CREATOR, operator, settings).await
}

/// Creates the event with `creator` as its `user` and `signer` as its
/// overlay's signer (the old user path: both the signed wallet).
pub async fn create_as(
    pool: &PgPool,
    body: &CreateEvent,
    creator: &str,
    signer: &str,
    settings: &crate::Settings,
) -> Result<String, ApiError> {
    let mut body = body.clone();
    for (image, kind) in [
        (&mut body.image, "poster"),
        (&mut body.image_vertical, "poster-vertical"),
    ] {
        if let Some(value) = image {
            *value = super::posters::resolve(pool, value, settings, kind).await?;
        }
    }
    let body = &body;
    let operator = signer;
    let now = Utc::now();
    let start_at = validate_create(body, now)?;
    let (place_id, world) = resolve_location(
        pool,
        body.world.unwrap_or(false),
        body.server.as_deref(),
        body.x,
        body.y,
    )
    .await?;
    let id = generate_id();
    let mut doc = build_create_raw(body, creator, start_at, place_id.as_deref(), world, now)?;
    doc["id"] = json!(id);
    if let Some(raw) = doc.as_object_mut() {
        let mut conn = pool.acquire().await?;
        super::metadata::enrich(&mut conn, raw, None, true).await?;
    }
    if creator != ADMIN_CREATOR {
        if let Some(community) = body.community_id.as_deref() {
            validate_community(pool, community, creator).await?;
        }
    }
    if let Some(raw) = doc.as_object() {
        validate_references(pool, raw).await?;
    }
    let mut tx = pool.begin().await?;
    let created_at: DateTime<Utc> = sqlx::query_scalar("SELECT clock_timestamp()")
        .fetch_one(&mut *tx)
        .await?;
    if let Some(raw) = doc.as_object_mut() {
        begin_metric_window(raw, created_at, None);
    }
    doc["created_at"] = json!(created_at.to_rfc3339());
    doc["updated_at"] = json!(created_at.to_rfc3339());
    let f = super::mirror::extract_fields(&doc)
        .ok_or_else(|| ApiError::internal("event document without an id"))?;
    sqlx::query(
        "INSERT INTO explore.event (id, name, start_at, finish_at, next_start_at, next_finish_at, duration_ms, recurrent, \
             highlighted, trending, approved, attending, community_id, user_creator, coordinates_x, coordinates_y, description, raw, origin) \
         VALUES ($1, $2, $3::timestamptz AT TIME ZONE 'UTC', $4::timestamptz AT TIME ZONE 'UTC', $5::timestamptz AT TIME ZONE 'UTC', \
             $6::timestamptz AT TIME ZONE 'UTC', $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18, 'local')",
    )
    .bind(f.id)
    .bind(f.name)
    .bind(f.start_at)
    .bind(f.finish_at)
    .bind(f.next_start_at)
    .bind(f.next_finish_at)
    .bind(f.duration_ms)
    .bind(f.recurrent)
    .bind(f.highlighted)
    .bind(f.trending)
    .bind(f.approved)
    .bind(f.attending)
    .bind(f.community_id)
    .bind(f.user)
    .bind(f.x)
    .bind(f.y)
    .bind(f.description)
    .bind(&doc)
    .execute(&mut *tx)
    .await?;
    sqlx::query(
        "INSERT INTO explore.events_local (id, signer, signed_payload) VALUES ($1, $2, $3)",
    )
    .bind(&id)
    .bind(operator)
    .bind(json!({ "create": body }))
    .execute(&mut *tx)
    .await?;
    super::notifications::record_transition(&mut tx, &id, None, &doc, true, settings).await?;
    tx.commit().await?;
    tracing::info!(event = %id, signer = operator, creator, "event created");
    Ok(id)
}

/// Store an edit/delete using the caller's already locked row and transaction.
/// `before` is that locked snapshot, not a document read outside the lock.
pub(super) async fn store_locked(
    tx: &mut Transaction<'_, Postgres>,
    event_id: &str,
    before: &Map<String, Value>,
    mut raw: Map<String, Value>,
    signer: &str,
    payload: Value,
    settings: &crate::Settings,
) -> Result<(), ApiError> {
    begin_metric_approval(tx, before, &mut raw).await?;
    let mut doc = Value::Object(raw);
    doc["id"] = json!(event_id);
    let f = super::mirror::extract_fields(&doc)
        .ok_or_else(|| ApiError::internal("event document without an id"))?;
    // The caller loaded and locked this local event before applying edits.
    super::notifications::record_transition(
        tx,
        event_id,
        Some(&Value::Object(before.clone())),
        &doc,
        true,
        settings,
    )
    .await?;
    sqlx::query(
        "UPDATE explore.event SET name = $2, start_at = $3::timestamptz AT TIME ZONE 'UTC', finish_at = $4::timestamptz AT TIME ZONE 'UTC',
             next_start_at = $5::timestamptz AT TIME ZONE 'UTC', next_finish_at = $6::timestamptz AT TIME ZONE 'UTC', duration_ms = $7,
             recurrent = $8, highlighted = $9, trending = $10, approved = $11, community_id = $12, user_creator = $13,
             coordinates_x = $14, coordinates_y = $15, description = $16, raw = $17
         WHERE id = $1",
    )
    .bind(f.id)
    .bind(f.name)
    .bind(f.start_at)
    .bind(f.finish_at)
    .bind(f.next_start_at)
    .bind(f.next_finish_at)
    .bind(f.duration_ms)
    .bind(f.recurrent)
    .bind(f.highlighted)
    .bind(f.trending)
    .bind(f.approved)
    .bind(f.community_id)
    .bind(f.user)
    .bind(f.x)
    .bind(f.y)
    .bind(f.description)
    .bind(&doc)
    .execute(&mut **tx)
    .await?;
    sqlx::query(
        "INSERT INTO explore.events_local (id, signer, signed_payload) VALUES ($1, $2, $3) \
         ON CONFLICT (id) DO UPDATE SET signer = EXCLUDED.signer, signed_payload = EXCLUDED.signed_payload, \
             updated_at = timezone('UTC', now())",
    )
    .bind(event_id)
    .bind(signer)
    .bind(payload)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn raw() -> Map<String, Value> {
        json!({ "name": "e", "approved": false })
            .as_object()
            .unwrap()
            .clone()
    }

    #[test]
    fn shared_admin_parser_gives_reason_precedence_including_null() {
        for (reason, expected) in [(json!("primary"), Some("primary")), (Value::Null, None)] {
            let parsed = parse_admin_patch(
                json!({"action":"reject","reason":reason,"rejection_reason":"alias"}),
            )
            .unwrap();
            assert_eq!(parsed.reason.as_deref(), expected);
            let mut raw = json!({"rejected":true,"rejection_reason":"old"})
                .as_object()
                .unwrap()
                .clone();
            apply_admin_state(&mut raw, &parsed, "operator").unwrap();
            assert_eq!(raw["rejection_reason"], json!(expected));
        }
        let parsed =
            parse_admin_patch(json!({"action":"reject","rejection_reason":"alias"})).unwrap();
        assert_eq!(parsed.reason.as_deref(), Some("alias"));
    }

    #[test]
    fn the_old_admin_rules_hold() {
        let actor = "0xabc";
        let mut r = raw();
        apply_admin_state(
            &mut r,
            &AdminPatch {
                action: Some("approve".into()),
                ..Default::default()
            },
            actor,
        )
        .unwrap();
        assert_eq!(
            (
                r["approved"].clone(),
                r["approved_by"].clone(),
                r["rejected"].clone()
            ),
            (json!(true), json!(actor), json!(false))
        );
        apply_admin_state(
            &mut r,
            &AdminPatch {
                action: Some("reject".into()),
                reason: Some("spam".into()),
                ..Default::default()
            },
            actor,
        )
        .unwrap();
        assert_eq!(
            (
                r["approved"].clone(),
                r["rejected"].clone(),
                r["rejection_reason"].clone()
            ),
            (json!(false), json!(true), json!("spam"))
        );
        apply_admin_state(
            &mut r,
            &AdminPatch {
                action: Some("feature".into()),
                trending: Some(true),
                ..Default::default()
            },
            actor,
        )
        .unwrap();
        assert_eq!(
            (r["highlighted"].clone(), r["trending"].clone()),
            (json!(true), json!(true))
        );
        assert!(
            apply_admin_state(&mut raw(), &AdminPatch::default(), actor).is_err(),
            "nothing to apply"
        );
        assert!(
            apply_admin_state(
                &mut raw(),
                &AdminPatch {
                    action: Some("delete".into()),
                    ..Default::default()
                },
                actor
            )
            .is_err()
        );
        let long = "x".repeat(501);
        assert!(
            apply_admin_state(
                &mut raw(),
                &AdminPatch {
                    action: Some("reject".into()),
                    reason: Some(long),
                    ..Default::default()
                },
                actor
            )
            .is_err()
        );
    }

    /// The decision lands in the row and its audit; on a mirrored event it
    /// lands in the row and `event_moderation`, which the next pass re-applies
    /// over the upstream's flags while it writes back the upstream's fields,
    /// and a patch of the mirrored event's own fields answers 409.
    #[tokio::test]
    async fn a_local_event_takes_its_moderation_and_a_mirrored_one_keeps_it_across_passes() {
        let Some(pool) = crate::db::scratch::pool(2).await else {
            return;
        };
        let id = format!("console-event-{}", uuid::Uuid::new_v4());
        let mirrored = format!("console-mirrored-{}", uuid::Uuid::new_v4());
        let upstream = |name: &str| {
            json!({ "id": mirrored, "name": name, "approved": true, "highlighted": false,
                    "trending": true, "start_at": "2099-06-01T18:00:00Z", "duration": 3_600_000 })
        };
        sqlx::query(
            "INSERT INTO explore.event (id, name, approved, raw, origin) VALUES ($1, 'local', true, $2, 'local')",
        )
        .bind(&id)
        .bind(json!({ "id": id, "name": "local", "approved": true }))
        .execute(&pool)
        .await
        .unwrap();
        assert!(
            super::super::mirror::upsert_event(&pool, &upstream("upstream"))
                .await
                .unwrap()
        );
        let actor = "0x00000000000000000000000000000000000000e1";
        let body = AdminPatch {
            action: Some("reject".into()),
            reason: Some("off-topic".into()),
            highlighted: Some(true),
            ..Default::default()
        };
        let rename = AdminPatch {
            name: Some("renamed here".into()),
            highlighted: Some(true),
            ..Default::default()
        };
        match moderate(
            &pool,
            &mirrored,
            &rename,
            actor,
            &crate::Settings::for_tests(),
        )
        .await
        {
            Err(ApiError::Http { status, message }) => {
                assert_eq!(status, 409);
                assert!(message.contains("mirrored from"), "{message}");
            }
            other => panic!("a mirrored event's own fields are the upstream's: {other:?}"),
        }
        let overlay = || {
            let pool = pool.clone();
            let mirrored = mirrored.clone();
            async move {
                sqlx::query_as::<
                    _,
                    (
                        Option<bool>,
                        Option<bool>,
                        Option<bool>,
                        Option<bool>,
                        String,
                    ),
                >(
                    "SELECT approved, rejected, highlighted, trending, set_by \
                     FROM explore.event_moderation WHERE event_id = $1",
                )
                .bind(mirrored)
                .fetch_optional(&pool)
                .await
                .unwrap()
            }
        };
        assert_eq!(overlay().await, None, "a refused patch records nothing");

        assert!(
            moderate(
                &pool,
                &mirrored,
                &body,
                actor,
                &crate::Settings::for_tests()
            )
            .await
            .unwrap()
        );
        assert_eq!(
            overlay().await,
            Some((Some(false), Some(true), Some(true), None, actor.to_string()))
        );
        let row = || {
            let pool = pool.clone();
            let mirrored = mirrored.clone();
            async move {
                sqlx::query_as::<_, (String, bool, bool, bool, Value, i64)>(
                    "SELECT name, approved, highlighted, trending, raw->'rejected', \
                            (SELECT count(*) FROM explore.events_local WHERE id = $1) \
                     FROM explore.event WHERE id = $1",
                )
                .bind(mirrored)
                .fetch_one(&pool)
                .await
                .unwrap()
            }
        };
        assert_eq!(
            row().await,
            ("upstream".into(), false, true, true, json!(true), 0)
        );
        assert!(
            super::super::mirror::upsert_event(&pool, &upstream("renamed upstream"))
                .await
                .unwrap()
        );
        assert_eq!(
            row().await,
            ("renamed upstream".into(), false, true, true, json!(true), 0),
            "the pass writes the upstream's name and keeps the operator's flags"
        );
        let approve = AdminPatch {
            action: Some("approve".into()),
            ..Default::default()
        };
        assert!(
            moderate(
                &pool,
                &mirrored,
                &approve,
                actor,
                &crate::Settings::for_tests()
            )
            .await
            .unwrap()
        );
        assert_eq!(
            overlay().await,
            Some((Some(true), Some(false), Some(true), None, actor.to_string())),
            "a later decision merges into the earlier one"
        );

        assert!(
            moderate(&pool, &id, &body, actor, &crate::Settings::for_tests())
                .await
                .unwrap()
        );
        let (approved, highlighted, raw): (bool, bool, Value) =
            sqlx::query_as("SELECT approved, highlighted, raw FROM explore.event WHERE id = $1")
                .bind(&id)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!((approved, highlighted), (false, true));
        assert_eq!(
            (
                raw["rejected"].clone(),
                raw["rejected_by"].clone(),
                raw["rejection_reason"].clone()
            ),
            (json!(true), json!(actor), json!("off-topic"))
        );
        let (signer, payload): (String, Value) =
            sqlx::query_as("SELECT signer, signed_payload FROM explore.events_local WHERE id = $1")
                .bind(&id)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(
            (signer.as_str(), payload["moderation"]["action"].as_str()),
            (actor, Some("reject"))
        );
        assert!(
            !moderate(
                &pool,
                "no-such-event",
                &body,
                actor,
                &crate::Settings::for_tests()
            )
            .await
            .unwrap()
        );

        sqlx::query(
            "UPDATE explore.event SET raw = raw || '{\"deleted_by_admin\": true}' WHERE id = $1",
        )
        .bind(&id)
        .execute(&pool)
        .await
        .unwrap();
        assert!(
            !moderate(&pool, &id, &body, actor, &crate::Settings::for_tests())
                .await
                .unwrap(),
            "a deleted event is not found"
        );
        sqlx::query("DELETE FROM explore.events_local WHERE id = $1")
            .bind(&id)
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("DELETE FROM explore.event_moderation WHERE event_id = $1")
            .bind(&mirrored)
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("DELETE FROM explore.event WHERE id = ANY($1)")
            .bind(vec![id, mirrored])
            .execute(&pool)
            .await
            .unwrap();
    }

    /// An operator's rejection of a mirrored event keeps its actor and reason
    /// across passes, and clears the upstream's approver; a later approval
    /// carries its approver and clears the rejection's.
    #[tokio::test]
    async fn a_mirrored_rejection_keeps_its_reason_and_actor_across_passes() {
        let Some(pool) = crate::db::scratch::pool(2).await else {
            return;
        };
        let id = format!("console-mirrored-{}", uuid::Uuid::new_v4());
        let upstream = json!({ "id": id, "name": "upstream", "approved": true,
                               "approved_by": "upstream-moderator", "rejected": false,
                               "start_at": "2099-06-01T18:00:00Z", "duration": 3_600_000 });
        let pass = || {
            let pool = pool.clone();
            let upstream = upstream.clone();
            async move {
                assert!(
                    super::super::mirror::upsert_event(&pool, &upstream)
                        .await
                        .unwrap()
                );
            }
        };
        let shown = || {
            let pool = pool.clone();
            let id = id.clone();
            async move {
                let (approved, raw): (bool, Value) =
                    sqlx::query_as("SELECT approved, raw FROM explore.event WHERE id = $1")
                        .bind(id)
                        .fetch_one(&pool)
                        .await
                        .unwrap();
                (
                    approved,
                    raw["approved_by"].clone(),
                    raw["rejected"].clone(),
                    raw["rejected_by"].clone(),
                    raw["rejection_reason"].clone(),
                )
            }
        };
        pass().await;
        let actor = "0x00000000000000000000000000000000000000e2";
        let reject = AdminPatch {
            action: Some("reject".into()),
            reason: Some("spam".into()),
            ..Default::default()
        };
        assert!(
            moderate(&pool, &id, &reject, actor, &crate::Settings::for_tests())
                .await
                .unwrap()
        );
        pass().await;
        assert_eq!(
            shown().await,
            (false, Value::Null, json!(true), json!(actor), json!("spam")),
            "the pass keeps the rejection's actor and reason"
        );
        let approve = AdminPatch {
            action: Some("approve".into()),
            ..Default::default()
        };
        assert!(
            moderate(&pool, &id, &approve, actor, &crate::Settings::for_tests())
                .await
                .unwrap()
        );
        pass().await;
        assert_eq!(
            shown().await,
            (true, json!(actor), json!(false), Value::Null, Value::Null),
            "an approval carries its approver and clears the rejection's"
        );
        sqlx::query("DELETE FROM explore.event_moderation WHERE event_id = $1")
            .bind(&id)
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("DELETE FROM explore.event WHERE id = $1")
            .bind(&id)
            .execute(&pool)
            .await
            .unwrap();
    }

    fn body(start: DateTime<Utc>) -> CreateEvent {
        serde_json::from_value(json!({ "name": "operator event", "start_at": start.to_rfc3339(), "duration": 3_600_000, "x": 10, "y": 20 })).unwrap()
    }

    #[test]
    fn the_old_create_rules_hold() {
        let now = Utc::now();
        let start = now + Duration::hours(2);
        assert_eq!(
            validate_create(&body(start), now).unwrap().timestamp(),
            start.timestamp()
        );
        let past = CreateEvent {
            start_at: (now - Duration::hours(5)).to_rfc3339(),
            ..body(start)
        };
        assert!(validate_create(&past, now).is_err(), "finished already");
        assert!(
            validate_create(
                &CreateEvent {
                    duration: 25 * 3_600_000,
                    ..body(start)
                },
                now
            )
            .is_err(),
            "over 24 h"
        );
        assert!(
            validate_create(
                &CreateEvent {
                    x: 400,
                    ..body(start)
                },
                now
            )
            .is_err()
        );
        assert!(
            validate_create(
                &CreateEvent {
                    x: 160,
                    y: 0,
                    ..body(start)
                },
                now
            )
            .is_err(),
            "outside Genesis"
        );
        assert!(
            validate_create(
                &CreateEvent {
                    x: 160,
                    y: 0,
                    world: Some(true),
                    ..body(start)
                },
                now
            )
            .is_ok(),
            "a world event is not bound to Genesis"
        );
        assert!(
            validate_create(
                &CreateEvent {
                    url: Some("javascript:alert(1)".into()),
                    ..body(start)
                },
                now
            )
            .is_err()
        );
        assert!(
            validate_create(
                &CreateEvent {
                    featured_item: Some("urn:decentraland:matic:collections-v2:0xnope".into()),
                    ..body(start)
                },
                now
            )
            .is_err()
        );
        assert!(
            validate_featured_item(
                "urn:decentraland:matic:collections-v2:0x0000000000000000000000000000000000000001:7"
            )
            .is_ok()
        );
        assert!(serde_json::from_value::<CreateEvent>(json!({ "name": "n", "start_at": "x", "duration": 1, "x": 0, "y": 0, "admin": true })).is_err(), "unknown fields are refused");
        let doc =
            build_create_raw(&body(start), ADMIN_CREATOR, start, Some("p"), false, now).unwrap();
        assert_eq!(
            (
                doc["approved"].clone(),
                doc["user"].clone(),
                doc["url"].clone()
            ),
            (
                json!(false),
                json!("admin"),
                json!("https://decentraland.org/play/?position=10%2C20")
            )
        );
    }

    #[test]
    fn concurrent_event_creates_get_unique_canonical_uuid_ids() {
        let barrier = std::sync::Barrier::new(8);
        let ids = std::thread::scope(|scope| {
            let workers: Vec<_> = (0..8)
                .map(|_| {
                    scope.spawn(|| {
                        barrier.wait();
                        (0..1000).map(|_| generate_id()).collect::<Vec<_>>()
                    })
                })
                .collect();
            workers
                .into_iter()
                .flat_map(|worker| worker.join().unwrap())
                .collect::<Vec<_>>()
        });
        let mut seen = std::collections::HashSet::new();
        for id in ids {
            let parsed = uuid::Uuid::parse_str(&id).unwrap();
            assert_eq!(parsed.get_version_num(), 4);
            assert_eq!(parsed.to_string(), id);
            assert!(
                seen.insert(id),
                "concurrent creates must not share an event ID"
            );
        }
    }

    #[tokio::test]
    async fn an_operator_event_is_created_pending_with_its_overlay_and_its_place() {
        let Some(pool) = crate::db::scratch::pool(2).await else {
            return;
        };
        let place = format!("console-create-place-{}", uuid::Uuid::new_v4());
        sqlx::query("INSERT INTO explore.place (id, base_position, raw) VALUES ($1, '-31,77', $2)")
            .bind(&place)
            .bind(json!({ "id": place, "positions": ["-31,77", "-31,78"] }))
            .execute(&pool)
            .await
            .unwrap();
        let start = Utc::now() + Duration::hours(1);
        let b = CreateEvent {
            x: -31,
            y: 78,
            ..body(start)
        };
        let operator = "0x00000000000000000000000000000000000000e2";
        let id = create(&pool, &b, operator, &crate::Settings::for_tests())
            .await
            .unwrap();
        let (approved, user, raw, origin): (bool, Option<String>, Value, String) = sqlx::query_as(
            "SELECT approved, user_creator, raw, origin FROM explore.event WHERE id = $1",
        )
        .bind(&id)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(
            (approved, user.as_deref(), raw["place_id"].as_str()),
            (false, Some(ADMIN_CREATOR), Some(place.as_str()))
        );
        assert_eq!(origin, LOCAL, "an event created here is this node's");
        let signer: String =
            sqlx::query_scalar("SELECT signer FROM explore.events_local WHERE id = $1")
                .bind(&id)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(signer, operator);
        assert!(
            moderate(
                &pool,
                &id,
                &AdminPatch {
                    action: Some("approve".into()),
                    ..Default::default()
                },
                operator,
                &crate::Settings::for_tests()
            )
            .await
            .unwrap()
        );
        sqlx::query("DELETE FROM explore.events_local WHERE id = $1")
            .bind(&id)
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("DELETE FROM explore.event WHERE id = $1")
            .bind(&id)
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("DELETE FROM explore.place WHERE id = $1")
            .bind(&place)
            .execute(&pool)
            .await
            .unwrap();
    }
    #[tokio::test]
    async fn rejecting_does_not_pin_upstream_feature_flags_after_approval() {
        let Some(pool) = crate::db::scratch::pool(2).await else {
            return;
        };
        let id = format!("mirror-features-{}", uuid::Uuid::new_v4());
        let upstream = |featured: bool| json!({"id":id,"name":"Mirror","approved":true,"highlighted":featured,"trending":featured,"start_at":"2099-06-01T18:00:00Z","duration":3600000});
        super::super::mirror::upsert_event(&pool, &upstream(false))
            .await
            .unwrap();
        moderate(
            &pool,
            &id,
            &AdminPatch {
                action: Some("reject".into()),
                ..Default::default()
            },
            "operator",
            &crate::Settings::for_tests(),
        )
        .await
        .unwrap();
        super::super::mirror::upsert_event(&pool, &upstream(true))
            .await
            .unwrap();
        moderate(
            &pool,
            &id,
            &AdminPatch {
                action: Some("approve".into()),
                ..Default::default()
            },
            "operator",
            &crate::Settings::for_tests(),
        )
        .await
        .unwrap();
        super::super::mirror::upsert_event(&pool, &upstream(true))
            .await
            .unwrap();
        let flags: (bool, bool) =
            sqlx::query_as("SELECT highlighted,trending FROM explore.event WHERE id=$1")
                .bind(&id)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(flags, (true, true));
        moderate(
            &pool,
            &id,
            &AdminPatch {
                action: Some("unfeature".into()),
                ..Default::default()
            },
            "operator",
            &crate::Settings::for_tests(),
        )
        .await
        .unwrap();
        moderate(
            &pool,
            &id,
            &AdminPatch {
                action: Some("approve".into()),
                ..Default::default()
            },
            "operator",
            &crate::Settings::for_tests(),
        )
        .await
        .unwrap();
        super::super::mirror::upsert_event(&pool, &upstream(true))
            .await
            .unwrap();
        let highlighted: bool =
            sqlx::query_scalar("SELECT highlighted FROM explore.event WHERE id=$1")
                .bind(&id)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert!(
            !highlighted,
            "an intentional operator unfeature survives approval and mirror refresh"
        );
    }
}

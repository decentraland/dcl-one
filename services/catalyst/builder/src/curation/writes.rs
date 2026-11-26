//! The curation writes: a collection's or an item's newest curation
//! updated, or a new review request opened. Each runs in one transaction
//! that locks the collection row, decides again who may write
//! ([`access::confirm_curator`]), reads the newest curation under the lock
//! and asks the transition table ([`super::transitions`]), so two curators
//! acting at once end in one consistent state. A committee write records its
//! `ops.admin_audit` row in the same transaction; a Slack notice
//! ([`super::slack`]) is sent only after the commit.

use serde_json::{Map, Value, json};
use sqlx::PgConnection;
use uuid::Uuid;

use super::rows::{
    self, CollectionCurationOpened, CollectionCurationOut, ItemCurationOpened, ItemCurationOut,
};
use super::slack::Notice;
use super::transitions::{self, Actor, Change, Curation, Refusal, Request, Status};
use crate::SharedState;
use crate::access::{self, Curator, Operator};
use crate::chain_state;
use crate::http::ApiError;
use crate::shared::audit;
use crate::validator;

/// The rejection reasons builder-server accepts.
pub const REJECTION_REASONS: [&str; 12] = [
    "clipping",
    "thumbnail",
    "category_hides",
    "rigging",
    "triangle_count",
    "emote",
    "file_size",
    "textures_materials",
    "reversed_faces",
    "smart_wearable_files",
    "content_policy_ip",
    "other",
];
/// The longest rejection message.
pub const MAX_MESSAGE: usize = 3000;
/// The longest note.
pub const MAX_NOTE: usize = 2000;

fn invalid_schema() -> ApiError {
    ApiError::bad_request("Invalid schema")
}

/// A `PATCH` body's `curation`.
#[derive(Debug, Default)]
pub struct Update {
    pub status: Option<Status>,
    /// Present when the body names an assignee; `None` inside unassigns.
    pub assignee: Option<Option<String>>,
    pub reasons: Option<Vec<String>>,
    pub message: Option<String>,
    pub note: Option<String>,
    /// The item content an item approval approves.
    pub content_hash: Option<String>,
}

/// A `POST` body's `curation`.
#[derive(Debug, Default)]
pub struct Open {
    pub assignee: Option<String>,
    /// What changed, as the creator says it, trimmed.
    pub note: String,
}

fn object(value: &Value) -> Result<&Map<String, Value>, ApiError> {
    value.as_object().ok_or_else(invalid_schema)
}

fn bounded(value: &Value, max: usize, min: usize) -> Result<String, ApiError> {
    let text = value.as_str().ok_or_else(invalid_schema)?;
    let length = text.chars().count();
    if length < min || length > max {
        return Err(invalid_schema());
    }
    Ok(text.to_string())
}

/// builder-server's `patchCurationSchema`, refusing a key it doesn't name.
pub fn parse_update(curation: &Value) -> Result<Update, ApiError> {
    let fields = object(curation)?;
    let mut update = Update::default();
    for (key, value) in fields {
        match key.as_str() {
            "status" => {
                update.status = Some(
                    value
                        .as_str()
                        .and_then(Status::parse)
                        .ok_or_else(invalid_schema)?,
                )
            }
            "assignee" => {
                update.assignee = Some(match value {
                    Value::Null => None,
                    Value::String(s) if s.is_empty() => None,
                    Value::String(s) => Some(s.to_ascii_lowercase()),
                    _ => return Err(invalid_schema()),
                })
            }
            "rejectionReasons" => {
                let reasons = value.as_array().ok_or_else(invalid_schema)?;
                let mut seen = Vec::new();
                for reason in reasons {
                    let reason = reason
                        .as_str()
                        .filter(|r| REJECTION_REASONS.contains(r))
                        .ok_or_else(invalid_schema)?;
                    if seen.iter().any(|s| s == reason) {
                        return Err(invalid_schema());
                    }
                    seen.push(reason.to_string());
                }
                if seen.is_empty() {
                    return Err(invalid_schema());
                }
                update.reasons = Some(seen);
            }
            "rejectionMessage" => update.message = Some(bounded(value, MAX_MESSAGE, 1)?),
            "note" => update.note = Some(bounded(value, MAX_NOTE, 0)?),
            "contentHash" => {
                update.content_hash = Some(
                    value
                        .as_str()
                        .filter(|h| super::content_hash(h))
                        .ok_or_else(invalid_schema)?
                        .to_string(),
                )
            }
            _ => return Err(invalid_schema()),
        }
    }
    if update.status.is_none() && update.assignee.is_none() {
        return Err(invalid_schema());
    }
    Ok(update)
}

/// builder-server's `postCurationSchema` (a note) and the assignee it
/// reads, refusing a key it doesn't name.
pub fn parse_open(curation: Option<&Value>) -> Result<Open, ApiError> {
    let Some(curation) = curation else {
        return Ok(Open::default());
    };
    let mut open = Open::default();
    for (key, value) in object(curation)? {
        match key.as_str() {
            "assignee" => {
                open.assignee = match value {
                    Value::Null => None,
                    Value::String(s) if s.is_empty() => None,
                    Value::String(s) => Some(s.to_ascii_lowercase()),
                    _ => return Err(invalid_schema()),
                }
            }
            "note" => open.note = bounded(value, MAX_NOTE, 0)?.trim().to_string(),
            _ => return Err(invalid_schema()),
        }
    }
    Ok(open)
}

fn refused(refusal: Refusal, id: Uuid) -> ApiError {
    let data = json!({ "id": id });
    match refusal {
        Refusal::StatusIsCommittees => {
            ApiError::forbidden("Only committee members can approve or reject a curation")
        }
        Refusal::AssigneeIsCommittees => {
            ApiError::forbidden("Only committee members can modify the assignee")
        }
        Refusal::NoCuration => ApiError::not_found("Curation does not exist"),
        Refusal::Ongoing => ApiError::bad_request("There is already an ongoing review request"),
        Refusal::NeverCurated => ApiError::bad_request(
            "Item curations can't be created for items that weren't curated before",
        ),
        Refusal::ItemAssignee => return invalid_schema(),
        Refusal::VerdictOnly | Refusal::Decided | Refusal::ItemStatusOnly => {
            return ApiError::internal(format!("a curation request refused as {refusal:?}"));
        }
    }
    .with_data(data)
}

fn actor(curator: &Curator) -> Actor {
    if curator.committee() {
        Actor::Committee
    } else {
        Actor::Creator
    }
}

/// Asks the table as the curator stands; a refusal only the committee
/// lifts admits the signer as the committee when it is on it.
async fn admitted(
    state: &SharedState,
    curator: Curator,
    curation: Curation,
    from: Option<Status>,
    request: Request,
) -> Result<Curator, ApiError> {
    match transitions::decide(curation, from, request, actor(&curator)) {
        Err(Refusal::StatusIsCommittees | Refusal::AssigneeIsCommittees) => {
            let curator = access::as_committee(state, curator).await?;
            Ok(curator)
        }
        _ => Ok(curator),
    }
}

fn subject(curator: &Curator) -> Uuid {
    curator.item().unwrap_or(curator.collection())
}

async fn newest_status(state: &SharedState, curator: &Curator) -> Result<Option<Status>, ApiError> {
    let mut conn = state.items.pool().acquire().await?;
    Ok(match curator.item() {
        Some(item) => rows::newest_of_item(&mut conn, item)
            .await?
            .map(|k| k.status),
        None => rows::newest_of_collection(&mut conn, curator.collection())
            .await?
            .map(|k| k.status),
    }
    .as_deref()
    .and_then(Status::parse))
}

/// 403 when a committee read that started after the decision no longer
/// lists `assignee`.
async fn unassignable(
    state: &crate::State,
    assignee: Option<&str>,
    id: Uuid,
) -> Result<(), ApiError> {
    if let Some(assignee) = assignee
        && !access::assignable(state, assignee).await?
    {
        return Err(
            ApiError::forbidden("The assignee must be a committee member")
                .with_data(json!({ "id": id })),
        );
    }
    Ok(())
}

async fn lock(conn: &mut PgConnection, collection: Uuid) -> Result<(), ApiError> {
    sqlx::query("SELECT 1 FROM builder.collections WHERE id = $1 FOR UPDATE")
        .bind(collection)
        .execute(conn)
        .await?;
    Ok(())
}

async fn audited(
    conn: &mut PgConnection,
    curator: &Curator,
    action: &str,
    detail: Value,
) -> Result<(), ApiError> {
    let target = subject(curator).to_string();
    audit::record(
        conn,
        &audit::Entry::new(curator.signer(), "create", action)
            .target(Some(&target))
            .detail(detail),
    )
    .await?;
    Ok(())
}

/// Appends a curator's event to the collection's review timeline.
async fn event(
    conn: &mut PgConnection,
    collection: Uuid,
    kind: &str,
    actor: &str,
    payload: Value,
) -> Result<(), ApiError> {
    super::events::record(
        conn,
        collection,
        kind,
        super::events::Actor::Curator,
        Some(actor),
        payload,
        None,
    )
    .await?;
    Ok(())
}

#[derive(Clone, Copy, PartialEq)]
enum Decision {
    New,
    Edit,
    Repeat,
}

/// builder-server's `classifyDecision`: a decision is new unless the newest
/// curator decision on the timeline is the same one, taken since this
/// curation was opened; a rejection resent with other feedback edits it.
async fn classify(
    conn: &mut PgConnection,
    previous: &CollectionCurationOut,
    status: Status,
    update: &Update,
) -> Result<Decision, ApiError> {
    let wanted = match status {
        Status::Approved => "review.approved",
        _ => "review.rejected",
    };
    let latest: Option<(String, chrono::NaiveDateTime)> = sqlx::query_as(
        "SELECT type, created_at FROM builder.collection_events WHERE collection_id = $1 \
         AND type IN ('review.approved', 'review.rejected') ORDER BY created_at DESC, id DESC LIMIT 1",
    )
    .bind(previous.collection_id)
    .fetch_optional(conn)
    .await?;
    let same = latest.is_some_and(|(kind, at)| kind == wanted && at >= previous.created_at);
    if !same {
        return Ok(Decision::New);
    }
    let sorted = |reasons: &[String]| {
        let mut r = reasons.to_vec();
        r.sort();
        r
    };
    let same_reasons = update.reasons.as_ref().is_none_or(|r| {
        sorted(r) == sorted(previous.rejection_reasons.as_deref().unwrap_or_default())
    });
    let message = update
        .message
        .as_deref()
        .map(str::trim)
        .filter(|m| !m.is_empty());
    let same_message =
        message.is_none_or(|m| m == previous.rejection_message.as_deref().unwrap_or_default());
    Ok(
        if status == Status::Rejected && (!same_reasons || !same_message) {
            Decision::Edit
        } else {
            Decision::Repeat
        },
    )
}

/// Sets each smart wearable's kept video of the collection to the
/// `video.mp4` its contents name, as an approval pins it.
async fn pin_videos(conn: &mut PgConnection, collection: Uuid) -> Result<(), ApiError> {
    let replaced: Vec<Option<String>> = sqlx::query_scalar(
        "WITH named AS ( \
           SELECT i.id, v.hash, v.named_by, i.video AS before FROM builder.items i \
           JOIN builder.item_contents v ON v.item_id = i.id AND v.file = 'video.mp4' \
           WHERE i.collection_id = $1 AND i.type = 'wearable' AND i.video IS DISTINCT FROM v.hash \
             AND EXISTS (SELECT 1 FROM builder.item_contents s WHERE s.item_id = i.id AND s.file LIKE '%.js') \
             AND EXISTS (SELECT 1 FROM builder.uploaded_contents u \
               WHERE u.hash = v.hash AND u.uploaded_by = v.named_by AND u.video)) \
         UPDATE builder.items i SET video = named.hash, video_named_by = named.named_by \
         FROM named WHERE i.id = named.id RETURNING named.before",
    )
    .bind(collection)
    .fetch_all(&mut *conn)
    .await?;
    let before: Vec<String> = replaced.into_iter().flatten().collect();
    crate::items::save::forget_unnamed(conn, &before).await
}

/// `PATCH /v1/collections/:id/curation`.
pub async fn update_collection(
    state: &SharedState,
    curator: Curator,
    update: Update,
) -> Result<CollectionCurationOut, ApiError> {
    let request = Request::Update {
        status: update.status,
        assignee: update.assignee.is_some(),
    };
    let from = newest_status(state, &curator).await?;
    let curator = admitted(state, curator, Curation::Collection, from, request).await?;
    let id = curator.collection();
    if let Err(refusal) = transitions::decide(Curation::Collection, from, request, actor(&curator))
        && refusal != Refusal::NoCuration
    {
        return Err(refused(refusal, id));
    }
    let assignee_of = update.assignee.clone().flatten();
    let assignee_of = assignee_of.as_deref();
    if let Some(assignee) = assignee_of
        && !access::assignable(state, assignee).await?
    {
        return Err(
            ApiError::forbidden("The assignee must be a committee member")
                .with_data(json!({ "id": id })),
        );
    }
    let mut tx = state.items.pool().begin().await?;
    lock(&mut tx, id).await?;
    let view = chain_state::for_decision(&mut tx).await?;
    let curated = access::confirm_curator(&mut tx, &view, state, &curator).await?;
    unassignable(state, assignee_of, id).await?;
    let previous = rows::newest_of_collection(&mut tx, id).await?;
    let from = previous.as_ref().and_then(|k| Status::parse(&k.status));
    transitions::decide(Curation::Collection, from, request, actor(&curator))
        .map_err(|r| refused(r, id))?;
    let previous = previous.ok_or_else(|| refused(Refusal::NoCuration, id))?;
    let decision = match update.status {
        Some(status @ (Status::Approved | Status::Rejected)) => {
            Some((status, classify(&mut tx, &previous, status, &update).await?))
        }
        _ => None,
    };
    let reviewed = decision.filter(|(_, d)| *d != Decision::Repeat);
    let (reasons, message): (Option<Option<Vec<String>>>, Option<Option<String>>) = match reviewed {
        Some((Status::Approved, _)) => (Some(None), Some(None)),
        Some(_) => (
            update.reasons.clone().map(Some),
            update
                .message
                .as_deref()
                .map(str::trim)
                .filter(|m| !m.is_empty())
                .map(|m| Some(m.to_string())),
        ),
        None => (None, None),
    };
    if update.status == Some(Status::Approved) {
        pin_videos(&mut tx, id).await?;
    }
    let updated: CollectionCurationOut = sqlx::query_as(
        "UPDATE builder.collection_curations SET \
           assignee = CASE WHEN $2 THEN $3 ELSE assignee END, \
           status = COALESCE($4, status), \
           reviewed_by = CASE WHEN $5 THEN $6 ELSE reviewed_by END, \
           rejection_reasons = CASE WHEN $7 THEN $8 ELSE rejection_reasons END, \
           rejection_message = CASE WHEN $9 THEN $10 ELSE rejection_message END, \
           updated_at = GREATEST(updated_at, timezone('UTC', clock_timestamp())) \
         WHERE id = $1 RETURNING id, collection_id, status, created_at, updated_at, assignee, reviewed_by, \
           rejection_reasons, rejection_message",
    )
    .bind(previous.id)
    .bind(update.assignee.is_some())
    .bind(update.assignee.clone().flatten())
    .bind(update.status.map(Status::as_str))
    .bind(reviewed.is_some())
    .bind(curator.signer())
    .bind(reasons.is_some())
    .bind(reasons.clone().flatten())
    .bind(message.is_some())
    .bind(message.clone().flatten())
    .fetch_one(&mut *tx)
    .await?;
    let reassigned = update
        .assignee
        .as_ref()
        .filter(|a| **a != previous.assignee);
    if let Some(assignee) = reassigned {
        event(
            &mut tx,
            id,
            "review.assigned",
            curator.signer(),
            json!({ "assignee": assignee }),
        )
        .await?;
    }
    match reviewed {
        Some((Status::Approved, _)) => {
            let note = update.note.as_deref().map(str::trim).unwrap_or_default();
            let payload = if note.is_empty() {
                json!({})
            } else {
                json!({ "note": note })
            };
            event(&mut tx, id, "review.approved", curator.signer(), payload).await?;
        }
        Some(_) => {
            let payload = json!({
                "rejectionReasons": update.reasons.clone()
                    .or_else(|| previous.rejection_reasons.clone())
                    .unwrap_or_default(),
                "rejectionMessage": message.clone().flatten()
                    .or_else(|| previous.rejection_message.clone()),
            });
            event(&mut tx, id, "review.rejected", curator.signer(), payload).await?;
        }
        None => {}
    }
    if curator.committee() {
        audited(
            &mut tx,
            &curator,
            "builder.curation.update",
            json!({
                "from": previous.status,
                "status": update.status.map(Status::as_str),
                "assignee": update.assignee,
                "rejectionReasons": update.reasons,
                "rejectionMessage": update.message,
            }),
        )
        .await?;
    }
    tx.commit().await?;
    let notices = &state.notices;
    if let Some(assignee) = reassigned {
        notices.send(
            &state.guarded,
            id,
            &curated.name,
            curator.signer(),
            Notice::Assigned {
                assignee: assignee.clone(),
            },
        );
    }
    if let Some((status, Decision::New)) = decision {
        notices.send(
            &state.guarded,
            id,
            &curated.name,
            curator.signer(),
            Notice::Verdict {
                approved: status == Status::Approved,
            },
        );
    }
    Ok(updated)
}

/// `POST /v1/collections/:id/curation`.
pub async fn open_collection(
    state: &SharedState,
    curator: Curator,
    open: Open,
) -> Result<CollectionCurationOpened, ApiError> {
    let request = Request::Open {
        assignee: open.assignee.is_some(),
    };
    let from = newest_status(state, &curator).await?;
    let curator = admitted(state, curator, Curation::Collection, from, request).await?;
    let id = curator.collection();
    transitions::decide(Curation::Collection, from, request, actor(&curator))
        .map_err(|r| refused(r, id))?;
    let assignee_of = open.assignee.as_deref();
    if let Some(assignee) = assignee_of
        && !access::assignable(state, assignee).await?
    {
        return Err(
            ApiError::forbidden("The assignee must be a committee member")
                .with_data(json!({ "id": id })),
        );
    }
    let known_committee = access::known_curator(state, &curator).await;
    let mut tx = state.items.pool().begin().await?;
    lock(&mut tx, id).await?;
    let view = chain_state::for_decision(&mut tx).await?;
    let curated = access::confirm_curator(&mut tx, &view, state, &curator).await?;
    unassignable(state, assignee_of, id).await?;
    let previous = rows::newest_of_collection(&mut tx, id).await?;
    let from = previous.as_ref().and_then(|k| Status::parse(&k.status));
    transitions::decide(Curation::Collection, from, request, actor(&curator))
        .map_err(|r| refused(r, id))?;
    let notice = match &open.assignee {
        Some(assignee) => Some(Notice::Assigned {
            assignee: Some(assignee.clone()),
        }),
        None if !curated.creator && known_committee => None,
        None => {
            let submission = match (&previous, &curated.contract) {
                (Some(_), _) => true,
                (None, _) if curated.third_party => false,
                (None, None) => false,
                (None, Some(contract)) => {
                    let published: bool = sqlx::query_scalar(
                        "SELECT EXISTS (SELECT 1 FROM builder.collection_events \
                         WHERE collection_id = $1 AND type = 'collection.published')",
                    )
                    .bind(id)
                    .fetch_one(&mut *tx)
                    .await?;
                    !published
                        && chain_state::collection(&mut tx, &view, contract)
                            .await?
                            .is_none_or(|facts| facts.is_approved)
                }
            };
            submission.then(|| Notice::ReviewRequested {
                after_rejection: from == Some(Status::Rejected),
            })
        }
    };
    let validation = match (&notice, &curated.contract) {
        (Some(Notice::ReviewRequested { .. }), Some(contract)) if !curated.third_party => {
            if crate::review::enabled(&mut tx, state).await? {
                match crate::review::submit(
                    &mut tx,
                    state,
                    id,
                    contract,
                    previous.as_ref(),
                    curator.signer(),
                    &open.note,
                )
                .await?
                {
                    crate::review::Submitted::Answered(existing) => {
                        tx.rollback().await?;
                        return Ok(CollectionCurationOpened {
                            id: existing.id,
                            status: existing.status,
                            created_at: existing.created_at,
                            updated_at: existing.updated_at,
                            collection_id: id,
                            assignee: existing.assignee,
                        });
                    }
                    crate::review::Submitted::Opened(notice) => notice,
                }
            } else {
                validator::on_changes(&mut tx, state, &view, id, contract).await?
            }
        }
        _ => None,
    };
    let opened = super::record_collection_curation(
        &mut tx,
        &curator,
        Status::Pending.as_str(),
        open.assignee.as_deref(),
    )
    .await?;
    let (created_at, updated_at): (chrono::NaiveDateTime, chrono::NaiveDateTime) = sqlx::query_as(
        "SELECT created_at, updated_at FROM builder.collection_curations WHERE id = $1",
    )
    .bind(opened)
    .fetch_one(&mut *tx)
    .await?;
    if curator.committee() {
        audited(
            &mut tx,
            &curator,
            "builder.curation.open",
            json!({ "from": previous.map(|k| k.status), "assignee": open.assignee }),
        )
        .await?;
    }
    tx.commit().await?;
    for notice in notice.into_iter().chain(validation) {
        state
            .notices
            .send(&state.guarded, id, &curated.name, curator.signer(), notice);
    }
    Ok(CollectionCurationOpened {
        id: opened,
        status: Status::Pending.as_str().to_string(),
        created_at,
        updated_at,
        collection_id: id,
        assignee: open.assignee,
    })
}

/// The item's content hash and whether its mappings are set, as a new or
/// refreshed curation records them.
async fn submitted(conn: &mut PgConnection, item: Uuid) -> Result<(String, bool), ApiError> {
    let (hash, mapped): (Option<String>, bool) = sqlx::query_as(
        "SELECT local_content_hash, mappings IS NOT NULL FROM builder.items WHERE id = $1",
    )
    .bind(item)
    .fetch_one(conn)
    .await?;
    let hash = hash.filter(|h| super::content_hash(h)).ok_or_else(|| {
        ApiError::conflict("The item has no content hash to curate")
            .with_data(json!({ "id": item }))
    })?;
    Ok((hash, mapped))
}

/// `PATCH /v1/items/:id/curation`.
pub async fn update_item(
    state: &SharedState,
    curator: Curator,
    update: Update,
) -> Result<ItemCurationOut, ApiError> {
    let request = Request::Update {
        status: update.status,
        assignee: update.assignee.is_some(),
    };
    let from = newest_status(state, &curator).await?;
    let curator = admitted(state, curator, Curation::Item, from, request).await?;
    let item = subject(&curator);
    transitions::decide(Curation::Item, from, request, actor(&curator))
        .map_err(|r| refused(r, item))?;
    let approved = update.status == Some(Status::Approved);
    if approved && update.content_hash.is_none() {
        return Err(
            ApiError::bad_request("An item approval names the content hash it approves")
                .with_data(json!({ "id": item })),
        );
    }
    let mut tx = state.items.pool().begin().await?;
    lock(&mut tx, curator.collection()).await?;
    let view = chain_state::for_decision(&mut tx).await?;
    access::confirm_curator(&mut tx, &view, state, &curator).await?;
    let previous = rows::newest_of_item(&mut tx, item).await?;
    let from = previous.as_ref().and_then(|k| Status::parse(&k.status));
    let Change::Update { status, .. } =
        transitions::decide(Curation::Item, from, request, actor(&curator))
            .map_err(|r| refused(r, item))?
    else {
        return Err(refused(Refusal::NoCuration, item));
    };
    let previous = previous.ok_or_else(|| refused(Refusal::NoCuration, item))?;
    let approves = update.content_hash.as_deref().filter(|_| approved);
    let (updated, hash) = set_item_curation(
        &mut tx,
        curator.collection(),
        item,
        previous.id,
        status,
        approves,
    )
    .await?;
    if curator.committee() {
        audited(
            &mut tx,
            &curator,
            "builder.item_curation.update",
            json!({ "from": previous.status, "status": status.map(Status::as_str), "content_hash": hash }),
        )
        .await?;
    }
    tx.commit().await?;
    Ok(updated)
}

async fn set_item_curation(
    conn: &mut PgConnection,
    collection: Uuid,
    item: Uuid,
    id: Uuid,
    status: Option<Status>,
    approves: Option<&str>,
) -> Result<(ItemCurationOut, String), ApiError> {
    let (hash, mapped) = submitted(&mut *conn, item).await?;
    if approves.is_some_and(|approved| approved != hash) {
        return Err(ApiError::conflict("The item changed since it was reviewed")
            .with_data(json!({ "id": item, "contentHash": hash })));
    }
    let updated: ItemCurationOut = sqlx::query_as(
        "UPDATE builder.item_curations SET status = COALESCE($2, status), content_hash = $3, \
           is_mapping_complete = $4, updated_at = GREATEST(updated_at, timezone('UTC', clock_timestamp())) \
         WHERE id = $1 RETURNING id, item_id, status, created_at, updated_at, content_hash, is_mapping_complete",
    )
    .bind(id)
    .bind(status.map(Status::as_str))
    .bind(&hash)
    .bind(mapped)
    .fetch_one(&mut *conn)
    .await?;
    sqlx::query(
        "UPDATE builder.collection_curations SET \
           updated_at = GREATEST(updated_at, timezone('UTC', clock_timestamp())) WHERE collection_id = $1",
    )
    .bind(collection)
    .execute(&mut *conn)
    .await?;
    Ok((updated, hash))
}

/// The operator's item status routes: the newest curation of each of
/// `items` in the operator's collection set to `status`, in one transaction
/// that locks the collection and decides again, each move asked of the table
/// as [`Actor::Operator`] and recorded in `ops.admin_audit`. An item with no
/// curation, or of another collection, is left alone. Answers how many were
/// set.
pub async fn operate_items(
    state: &SharedState,
    operator: Operator,
    items: &[Uuid],
    status: Status,
) -> Result<u64, ApiError> {
    let request = Request::Update {
        status: Some(status),
        assignee: false,
    };
    let collection = operator.collection();
    let mut tx = state.items.pool().begin().await?;
    lock(&mut tx, collection).await?;
    let view = chain_state::for_decision(&mut tx).await?;
    access::confirm_operator(&mut tx, &view, &operator).await?;
    let members: Vec<Uuid> = sqlx::query_scalar(
        "SELECT id FROM builder.items WHERE collection_id = $1 AND id = ANY($2) ORDER BY id",
    )
    .bind(collection)
    .bind(items)
    .fetch_all(&mut *tx)
    .await?;
    let mut set = 0;
    for item in members {
        let Some(previous) = rows::newest_of_item(&mut tx, item).await? else {
            continue;
        };
        let from = Status::parse(&previous.status);
        let Change::Update { status, .. } =
            transitions::decide(Curation::Item, from, request, Actor::Operator)
                .map_err(|r| refused(r, item))?
        else {
            return Err(refused(Refusal::ItemStatusOnly, item));
        };
        let (_, hash) =
            set_item_curation(&mut tx, collection, item, previous.id, status, None).await?;
        let target = item.to_string();
        audit::record(
            &mut *tx,
            &audit::Entry::new(operator.actor(), "create", "builder.item_curation.update")
                .target(Some(&target))
                .detail(json!({
                    "from": previous.status,
                    "status": status.map(Status::as_str),
                    "content_hash": hash,
                })),
        )
        .await?;
        set += 1;
    }
    tx.commit().await?;
    Ok(set)
}

/// `POST /v1/items/:id/curation`.
pub async fn open_item(
    state: &SharedState,
    curator: Curator,
) -> Result<ItemCurationOpened, ApiError> {
    let request = Request::Open { assignee: false };
    let item = subject(&curator);
    let from = newest_status(state, &curator).await?;
    transitions::decide(Curation::Item, from, request, actor(&curator))
        .map_err(|r| refused(r, item))?;
    let mut tx = state.items.pool().begin().await?;
    lock(&mut tx, curator.collection()).await?;
    let view = chain_state::for_decision(&mut tx).await?;
    access::confirm_curator(&mut tx, &view, state, &curator).await?;
    let previous = rows::newest_of_item(&mut tx, item).await?;
    let from = previous.as_ref().and_then(|k| Status::parse(&k.status));
    transitions::decide(Curation::Item, from, request, actor(&curator))
        .map_err(|r| refused(r, item))?;
    let (hash, mapped) = submitted(&mut tx, item).await?;
    let opened =
        super::record_item_curation(&mut tx, item, Status::Pending.as_str(), &hash, mapped).await?;
    let (created_at, updated_at): (chrono::NaiveDateTime, chrono::NaiveDateTime) =
        sqlx::query_as("SELECT created_at, updated_at FROM builder.item_curations WHERE id = $1")
            .bind(opened)
            .fetch_one(&mut *tx)
            .await?;
    if curator.committee() {
        audited(
            &mut tx,
            &curator,
            "builder.item_curation.open",
            json!({ "from": previous.map(|k| k.status), "content_hash": hash }),
        )
        .await?;
    }
    tx.commit().await?;
    Ok(ItemCurationOpened {
        id: opened,
        status: Status::Pending.as_str().to_string(),
        created_at,
        updated_at,
        item_id: item,
        content_hash: hash,
        is_mapping_complete: mapped,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_patch_body_follows_builder_servers_schema_and_names_nothing_else() {
        let update = parse_update(&json!({
            "status": "rejected",
            "assignee": "0xABC",
            "rejectionReasons": ["clipping", "other"],
            "rejectionMessage": "Needs work",
            "note": ""
        }))
        .unwrap();
        assert_eq!(update.status, Some(Status::Rejected));
        assert_eq!(update.assignee, Some(Some("0xabc".into())));
        assert_eq!(
            parse_update(&json!({ "assignee": null })).unwrap().assignee,
            Some(None)
        );
        for refused in [
            json!({}),
            json!({ "note": "only a note" }),
            json!({ "status": "bogus" }),
            json!({ "status": "approved", "extra": 1 }),
            json!({ "assignee": 1 }),
            json!({ "status": "rejected", "rejectionReasons": [] }),
            json!({ "status": "rejected", "rejectionReasons": ["clipping", "clipping"] }),
            json!({ "status": "rejected", "rejectionReasons": ["<script>"] }),
            json!({ "status": "rejected", "rejectionMessage": "" }),
            json!({ "status": "rejected", "rejectionMessage": "x".repeat(MAX_MESSAGE + 1) }),
            json!({ "status": "approved", "note": "x".repeat(MAX_NOTE + 1) }),
            json!([]),
            json!("approved"),
        ] {
            assert!(parse_update(&refused).is_err(), "{refused}");
        }
        assert!(
            parse_update(
                &json!({ "status": "rejected", "rejectionMessage": "é".repeat(MAX_MESSAGE) })
            )
            .is_ok()
        );
    }

    #[test]
    fn a_post_body_takes_an_assignee_and_a_bounded_note() {
        assert_eq!(parse_open(None).unwrap().assignee, None);
        assert_eq!(
            parse_open(Some(&json!({ "assignee": "0xABC", "note": "hi" })))
                .unwrap()
                .assignee,
            Some("0xabc".into())
        );
        assert_eq!(
            parse_open(Some(&json!({ "assignee": null })))
                .unwrap()
                .assignee,
            None
        );
        for refused in [
            json!({ "note": "x".repeat(MAX_NOTE + 1) }),
            json!({ "assignee": 1 }),
            json!({ "status": "approved" }),
            json!(null),
        ] {
            assert!(parse_open(Some(&refused)).is_err(), "{refused}");
        }
    }
}

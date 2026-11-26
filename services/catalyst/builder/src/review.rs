//! The review flow: a standard collection's review as creators, curators
//! and the wearable validator share it, on with `builder-auto-curation-review-flow`
//! as well as the automated review ([`enabled`]).
//!
//! With it on, the collection's timeline (`builder.collection_events`)
//! records each automated review's start and verdict, a creator's appeals,
//! submitted changes and comments; the validator's verdict on its current
//! claim rejects the pending review, or reopens its own rejection, through
//! the transition table as [`transitions::Actor::Validator`], and never
//! approves; creators ask for another automated review ([`retry`]), appeal
//! a rejection to a curator ([`appeal`]) and write to the curators
//! ([`comment`]). Off, none of the four routes answers and a verdict only
//! settles its request.
//!
//! Each write locks the collection row, decides again who may write, and
//! sends its notice after the commit. A creator's text is bounded in length
//! ([`MAX_TEXT`]) and in count per collection and per wallet per UTC day.

use std::collections::{BTreeMap, HashMap};

use chrono::{NaiveDateTime, NaiveTime, TimeDelta};
use serde::Serialize;
use serde_json::value::{RawValue, to_raw_value};
use serde_json::{Value, json};
use sqlx::PgConnection;
use uuid::Uuid;

use crate::access::{self, CurationGrant, Curator, ReviewRead, ValidatorVerdict};
use crate::chain_state;
use crate::curation::events::{self, Actor, EventOut};
use crate::curation::rows::{self, CollectionCurationOut};
use crate::curation::slack::Notice;
use crate::curation::transitions::{self, Change, Curation, Request, Status};
use crate::http::{ApiError, iso};
use crate::shared::audit;
use crate::validator::{self, Timeline, Trigger};
use crate::{SharedState, State};

/// The feature whose flag (`builder-auto-curation-review-flow`) turns the
/// review flow on.
pub const FEATURE: &str = "auto-curation-review-flow";
/// The most comments one collection takes in a UTC day.
pub const COMMENTS_PER_COLLECTION: usize = 200;
/// The most comments one wallet writes in a UTC day.
pub const COMMENTS_PER_CALLER: usize = 200;
/// The most appeals one collection takes in a UTC day.
pub const APPEALS_PER_COLLECTION: usize = 5;
/// The most appeals one wallet makes in a UTC day.
pub const APPEALS_PER_CALLER: usize = 20;
/// The longest comment or appeal note, in characters.
pub const MAX_TEXT: usize = 2000;
/// A timeline page's default length.
pub const DEFAULT_EVENTS: i64 = 50;
/// A timeline page's longest length.
pub const MAX_EVENTS: i64 = 200;
/// The last timeline page asked for.
pub const MAX_PAGE: i64 = 10_000;
/// The validator's name where it decides a curation.
pub const VALIDATOR: &str = "validator";

/// Whether the review flow is on: the automated review is
/// ([`validator::enabled`]) and so is `builder-auto-curation-review-flow`,
/// as the boolean `true`; read on `conn`.
pub async fn enabled(conn: &mut PgConnection, state: &State) -> Result<bool, ApiError> {
    validator::features_on(conn, state, &[validator::FEATURE, FEATURE]).await
}

fn conflict(message: &str, collection: Uuid) -> ApiError {
    ApiError::conflict(message).with_data(json!({ "id": collection }))
}

fn not_published(collection: Uuid) -> ApiError {
    conflict(
        "The collection is not published, so it has no review yet",
        collection,
    )
}

fn in_progress_refusal(collection: Uuid) -> ApiError {
    conflict(
        "A validation is already in progress for this collection",
        collection,
    )
}

fn now(state: &State) -> NaiveDateTime {
    state.now().naive_utc()
}

fn next_day(at: NaiveDateTime) -> NaiveDateTime {
    at.date().and_time(NaiveTime::MIN) + TimeDelta::days(1)
}

fn limit_refusal(message: &str, collection: Uuid, retry_at: NaiveDateTime) -> ApiError {
    ApiError::too_many(message)
        .with_data(json!({ "id": collection, "retryAt": iso(&retry_at.and_utc()) }))
}

async fn lock(conn: &mut PgConnection, collection: Uuid) -> Result<(), ApiError> {
    sqlx::query("SELECT 1 FROM builder.collections WHERE id = $1 FOR UPDATE")
        .bind(collection)
        .execute(conn)
        .await?;
    Ok(())
}

/// A page of the timeline: builder-server's paginated answer.
#[derive(Debug, Serialize)]
pub struct EventsPage {
    pub total: i64,
    pub limit: i64,
    pub pages: i64,
    pub page: i64,
    pub results: Vec<EventOut>,
}

/// The page and length a query asks for, as builder-server reads and bounds
/// them.
pub fn pagination(query: Option<&str>) -> (i64, i64) {
    let mut page = None;
    let mut limit = None;
    for (key, value) in url::form_urlencoded::parse(query.unwrap_or_default().as_bytes()) {
        match key.as_ref() {
            "page" if page.is_none() => page = Some(value.into_owned()),
            "limit" if limit.is_none() => limit = Some(value.into_owned()),
            _ => {}
        }
    }
    let page = page
        .as_deref()
        .and_then(crate::collections::listing::js_int)
        .unwrap_or(1);
    let limit = limit
        .as_deref()
        .and_then(crate::collections::listing::js_int)
        .filter(|l| *l < 100_000)
        .unwrap_or(DEFAULT_EVENTS);
    (page.clamp(1, MAX_PAGE), limit.clamp(1, MAX_EVENTS))
}

/// `GET /v1/collections/:id/events`: a page of the timeline, newest first;
/// redacted unless the committee reads it.
pub async fn timeline(
    state: &SharedState,
    read: &ReviewRead,
    page: i64,
    limit: i64,
) -> Result<EventsPage, ApiError> {
    let mut conn = state.items.pool().acquire().await?;
    let (rows, total) = events::page(&mut conn, read.id(), limit, limit * (page - 1)).await?;
    let results = if read.committee() {
        rows
    } else {
        rows.into_iter().map(EventOut::redacted).collect()
    };
    Ok(EventsPage {
        total,
        limit,
        pages: (total + limit - 1) / limit,
        page,
        results,
    })
}

/// Refuses a creator's text past the day's bound for its collection or its
/// writer, with the writer's texts serialized while the caller's
/// transaction holds them.
async fn bounded(
    conn: &mut PgConnection,
    state: &State,
    kind: &str,
    collection: Uuid,
    address: &str,
    (per_collection, per_caller): (usize, usize),
    message: &str,
) -> Result<(), ApiError> {
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1, 0))")
        .bind(format!("builder.review-text:{address}"))
        .execute(&mut *conn)
        .await?;
    let at = now(state);
    let from = at.date().and_time(NaiveTime::MIN);
    let (on_collection, by_caller) =
        events::written(conn, kind, collection, address, from, next_day(at)).await?;
    if on_collection >= per_collection as i64 || by_caller >= per_caller as i64 {
        return Err(limit_refusal(message, collection, next_day(at)));
    }
    Ok(())
}

/// `POST /v1/collections/:id/comments`: a comment on an open review of a
/// published collection, by a role holder (as the creator) or the committee
/// (as a curator), about the collection or one of its items.
pub async fn comment(
    state: &SharedState,
    curator: Curator,
    message: String,
    item: Option<Uuid>,
) -> Result<EventOut, ApiError> {
    let id = curator.collection();
    let mut tx = state.items.pool().begin().await?;
    lock(&mut tx, id).await?;
    let view = chain_state::for_decision(&mut tx, &state.items.clock).await?;
    let curated = access::confirm_reviewer(&mut tx, &view, state, &curator).await?;
    if !curated.published {
        return Err(not_published(id));
    }
    let newest = rows::newest_of_collection(&mut tx, id).await?;
    if newest
        .as_ref()
        .is_none_or(|k| k.status == Status::Approved.as_str())
    {
        return Err(conflict(
            "The collection has no open review to comment on",
            id,
        ));
    }
    if let Some(item) = item {
        let belongs: bool = sqlx::query_scalar(
            "SELECT EXISTS (SELECT 1 FROM builder.items WHERE id = $1 AND collection_id = $2)",
        )
        .bind(item)
        .bind(id)
        .fetch_one(&mut *tx)
        .await?;
        if !belongs {
            return Err(
                ApiError::bad_request("The item does not belong to the collection")
                    .with_data(json!({ "id": id, "itemId": item })),
            );
        }
    }
    bounded(
        &mut tx,
        state,
        "review.comment",
        id,
        curator.signer(),
        (COMMENTS_PER_COLLECTION, COMMENTS_PER_CALLER),
        "The daily limit of comments was reached",
    )
    .await?;
    let mut payload = json!({ "message": message });
    if let Some(item) = item {
        payload["itemId"] = json!(item);
    }
    let actor = if curator.committee() {
        Actor::Curator
    } else {
        Actor::Creator
    };
    let event = events::record(
        &mut tx,
        id,
        "review.comment",
        actor,
        Some(curator.signer()),
        payload,
        Some(now(state)),
    )
    .await?;
    tx.commit().await?;
    if curator.committee() {
        return Ok(event);
    }
    state.notices.send(
        &state.guarded,
        id,
        &curated.name,
        curator.signer(),
        Notice::CommentPosted { message },
    );
    Ok(event.redacted())
}

/// `POST /v1/collections/:id/curation/appeal`: a role holder asks a curator
/// to look again at a rejected collection: the appeal goes on the timeline
/// and a new pending review opens.
pub async fn appeal(
    state: &SharedState,
    curator: Curator,
    note: String,
) -> Result<CollectionCurationOut, ApiError> {
    let id = curator.collection();
    let mut tx = state.items.pool().begin().await?;
    lock(&mut tx, id).await?;
    let view = chain_state::for_decision(&mut tx, &state.items.clock).await?;
    let curated = access::confirm_reviewer(&mut tx, &view, state, &curator).await?;
    let latest: Option<String> = sqlx::query_scalar(
        "SELECT type FROM builder.collection_events WHERE collection_id = $1 AND type <> 'review.comment' \
         ORDER BY created_at DESC, id DESC LIMIT 1",
    )
    .bind(id)
    .fetch_optional(&mut *tx)
    .await?;
    let newest = rows::newest_of_collection(&mut tx, id).await?;
    let from = newest.as_ref().and_then(|k| Status::parse(&k.status));
    let recorded = latest.as_deref() == Some("review.appeal_requested");
    if recorded && from == Some(Status::Pending) {
        return Err(conflict(
            "There is already an open appeal for this collection",
            id,
        ));
    }
    if from != Some(Status::Rejected) {
        return Err(conflict("Only rejected collections can be appealed", id));
    }
    transitions::decide(
        Curation::Collection,
        from,
        Request::Open { assignee: false },
        transitions::Actor::Creator,
    )
    .map_err(|_| conflict("Only rejected collections can be appealed", id))?;
    if !recorded {
        appeal_event(&mut tx, state, id, curator.signer(), &note).await?;
    }
    let opened = crate::curation::record_collection_curation(
        &mut tx,
        &curator,
        Status::Pending.as_str(),
        None,
    )
    .await?;
    let row = rows::collection_curation(&mut tx, opened).await?;
    tx.commit().await?;
    state.notices.send(
        &state.guarded,
        id,
        &curated.name,
        curator.signer(),
        Notice::AppealRequested { note },
    );
    Ok(row)
}

async fn appeal_event(
    conn: &mut PgConnection,
    state: &State,
    collection: Uuid,
    signer: &str,
    note: &str,
) -> Result<(), ApiError> {
    bounded(
        conn,
        state,
        "review.appeal_requested",
        collection,
        signer,
        (APPEALS_PER_COLLECTION, APPEALS_PER_CALLER),
        "The daily limit of appeals was reached",
    )
    .await?;
    events::record(
        conn,
        collection,
        "review.appeal_requested",
        Actor::Creator,
        Some(signer),
        json!({ "note": note }),
        Some(now(state)),
    )
    .await?;
    Ok(())
}

/// Whether `collection`'s automated review is running: it has an open
/// request no curator decided after.
async fn in_progress(conn: &mut PgConnection, collection: Uuid) -> Result<bool, ApiError> {
    let open: Option<(Uuid, NaiveDateTime)> = sqlx::query_as(
        "SELECT id, created_at FROM builder.validation_requests WHERE collection_id = $1 AND settled_at IS NULL",
    )
    .bind(collection)
    .fetch_optional(&mut *conn)
    .await?;
    let Some((request, created_at)) = open else {
        return Ok(false);
    };
    let since = events::started_at(conn, collection, request, created_at).await?;
    Ok(!events::decided_since(conn, collection, since).await?)
}

async fn within_daily_requests(
    conn: &mut PgConnection,
    state: &State,
    collection: Uuid,
) -> Result<(), ApiError> {
    let (today, next) = validator::requests_today(conn, collection, now(state)).await?;
    if today >= validator::DAILY_REQUESTS {
        return Err(limit_refusal(
            "The daily validation limit for this collection was reached",
            collection,
            next,
        ));
    }
    Ok(())
}

/// The items a retry sends: every minted item when nothing ran; the last
/// run's again after it failed without judging them; else the items changed
/// since they were judged and then the last run's others.
async fn retry_items(
    conn: &mut PgConnection,
    network: &crate::network::Network,
    collection: Uuid,
    contract: &str,
) -> Result<Vec<Uuid>, ApiError> {
    let minted: Vec<(Uuid, String)> = validator::saved_items(conn, collection, None)
        .await?
        .iter()
        .map(|item| (item.id, item.content_hash(network, contract)))
        .collect();
    let Some((validation, previous, _)) = events::latest_start(conn, collection).await? else {
        return Ok(minted.into_iter().map(|(id, _)| id).collect());
    };
    let previous: Vec<Uuid> = previous
        .iter()
        .filter_map(|id| Uuid::parse_str(id).ok())
        .filter(|id| minted.iter().any(|(m, _)| m == id))
        .collect();
    if let Some((kind, payload)) = events::verdict_of(conn, collection, &validation).await?
        && kind == "review.automated_error"
    {
        if payload.get("reason").and_then(Value::as_str) == Some("too_large") {
            return Err(conflict(
                "The collection is too large for an automated review: a curator will review it",
                collection,
            ));
        }
        return Ok(previous);
    }
    let judged = events::judged_hashes(conn, collection).await?;
    let changed: Vec<Uuid> = minted
        .iter()
        .filter(|(id, hash)| judged.get(&id.to_string()) != Some(hash))
        .map(|(id, _)| *id)
        .collect();
    if changed.is_empty() {
        return Err(conflict(
            "Nothing changed since the last validation: edit an item or appeal the decision",
            collection,
        ));
    }
    let rest: Vec<Uuid> = previous
        .into_iter()
        .filter(|id| !changed.contains(id))
        .collect();
    Ok(changed.into_iter().chain(rest).collect())
}

/// `POST /v1/collections/:id/validations`: a role holder asks for another
/// automated review of a published standard collection, refused while one
/// runs, past the day's requests, after a curator's decision, and when
/// nothing changed since the last verdict; a validator's rejection reopens.
/// Answers the start, redacted.
pub async fn retry(state: &SharedState, curator: Curator) -> Result<EventOut, ApiError> {
    let id = curator.collection();
    let mut tx = state.items.pool().begin().await?;
    lock(&mut tx, id).await?;
    let view = chain_state::for_decision(&mut tx, &state.items.clock).await?;
    let curated = access::confirm_reviewer(&mut tx, &view, state, &curator).await?;
    if !curated.published {
        return Err(not_published(id));
    }
    let Some(contract) = curated.contract.clone().filter(|_| !curated.third_party) else {
        return Err(conflict(
            "Only standard collections can be validated automatically",
            id,
        ));
    };
    let newest = rows::newest_of_collection(&mut tx, id).await?;
    let from = newest.as_ref().and_then(|k| Status::parse(&k.status));
    if from == Some(Status::Approved) {
        return Err(conflict(
            "There is nothing to validate again: submit your changes for review",
            id,
        ));
    }
    if from == Some(Status::Rejected)
        && newest.as_ref().and_then(|k| k.reviewed_by.as_deref()) != Some(VALIDATOR)
    {
        return Err(conflict(
            "A curator rejected this collection: submit your changes for review instead of validating again",
            id,
        ));
    }
    if in_progress(&mut tx, id).await? {
        return Err(in_progress_refusal(id));
    }
    within_daily_requests(&mut tx, state, id).await?;
    let items = retry_items(&mut tx, &state.settings.network, id, &contract).await?;
    let recorded = validator::record(
        &mut tx,
        state,
        id,
        Trigger::Retry,
        &items,
        Timeline::On {
            by: Some(curator.signer()),
        },
    )
    .await?;
    let start = recorded
        .start
        .ok_or_else(|| ApiError::internal(format!("collection {id}: a retry recorded no start")))?;
    if from == Some(Status::Rejected) {
        transitions::decide(
            Curation::Collection,
            from,
            Request::Open { assignee: false },
            transitions::Actor::Creator,
        )
        .map_err(|_| ApiError::internal("a creator's retry may reopen a rejection"))?;
        crate::curation::record_collection_curation(
            &mut tx,
            &curator,
            Status::Pending.as_str(),
            None,
        )
        .await?;
    }
    tx.commit().await?;
    if let Some(notice) = recorded.notice {
        state
            .notices
            .send(&state.guarded, id, &curated.name, curator.signer(), notice);
    }
    Ok(start.redacted())
}

/// What a creator's changes submission does with the review flow on.
pub(crate) enum Submitted {
    /// Nothing changed since the validator's own rejection, which nobody
    /// touched: it answers the request and nothing is written.
    Answered(CollectionCurationOut),
    /// The request opens, with the curators' notice about its automated
    /// review, if any.
    Opened(Option<Notice>),
}

/// A creator's changes submission on the published standard collection
/// `collection` at `contract`, on the submission's transaction (which holds
/// the collection's row lock), with the review flow on: the items changed
/// on chain and since they were judged go to the validator, refused while a
/// review runs or past the day's requests; nothing changed after a
/// rejection is an appeal.
pub(crate) async fn submit(
    conn: &mut PgConnection,
    state: &State,
    collection: Uuid,
    contract: &str,
    previous: Option<&CollectionCurationOut>,
    signer: &str,
    note: &str,
) -> Result<Submitted, ApiError> {
    let view = chain_state::for_decision(conn, &state.items.clock).await?;
    let changed =
        match validator::changed(conn, &view, &state.settings.network, collection, contract).await?
        {
            Ok(changed) => changed,
            Err(_) => validator::saved_items(conn, collection, None)
                .await?
                .iter()
                .map(|item| {
                    (
                        item.id,
                        item.content_hash(&state.settings.network, contract),
                    )
                })
                .collect(),
        };
    let judged: HashMap<String, String> = events::judged_hashes(conn, collection).await?;
    let items: Vec<Uuid> = changed
        .into_iter()
        .filter(|(id, hash)| judged.get(&id.to_string()) != Some(hash))
        .map(|(id, _)| id)
        .collect();
    if items.is_empty() {
        if let Some(previous) = previous.filter(|k| {
            k.status == Status::Rejected.as_str()
                && k.reviewed_by.as_deref() == Some(VALIDATOR)
                && k.created_at == k.updated_at
        }) {
            return Ok(Submitted::Answered(
                rows::collection_curation(conn, previous.id).await?,
            ));
        }
        if previous.is_some_and(|k| k.status == Status::Rejected.as_str()) {
            appeal_event(conn, state, collection, signer, note).await?;
        }
        return Ok(Submitted::Opened(None));
    }
    if in_progress(conn, collection).await? {
        return Err(in_progress_refusal(collection));
    }
    within_daily_requests(conn, state, collection).await?;
    let mut payload = json!({ "itemIds": items });
    if !note.is_empty() {
        payload["note"] = json!(note);
    }
    events::record(
        conn,
        collection,
        "changes.submitted",
        Actor::Creator,
        Some(signer),
        payload,
        Some(now(state)),
    )
    .await?;
    Ok(Submitted::Opened(
        validator::record(
            conn,
            state,
            collection,
            Trigger::Changes,
            &items,
            Timeline::On { by: None },
        )
        .await?
        .notice,
    ))
}

/// A third-party collection's publication, on its transaction: with the
/// review flow on, the timeline records once that a curator reviews its
/// items by hand.
pub(crate) async fn on_third_party_publish(
    conn: &mut PgConnection,
    state: &State,
    collection: Uuid,
) -> Result<(), ApiError> {
    if !enabled(conn, state).await? {
        return Ok(());
    }
    let handed: bool = sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM builder.collection_events WHERE collection_id = $1 \
         AND type = 'review.human_required' AND payload->>'reason' = 'third_party')",
    )
    .bind(collection)
    .fetch_one(&mut *conn)
    .await?;
    if !handed {
        events::record(
            conn,
            collection,
            "review.human_required",
            Actor::System,
            None,
            json!({ "reason": "third_party" }),
            Some(now(state)),
        )
        .await?;
    }
    Ok(())
}

/// The validator's verdict in `result` (builder-server's validation result
/// as it came, without the claim) on the request `validation`, from its
/// current claim, on the verdict's transaction, which holds the collection's
/// row lock, with the review flow on. Unless a curator decided since the
/// request started, a rejection rejects the pending review (or opens a
/// rejected one) and a pass reopens the validator's own rejection; the
/// verdict goes on the timeline once, with `stale` (the items changed since
/// they were sent); an error the validator can't recover hands the review
/// to a curator. The curators' notices to send after the commit.
pub(crate) async fn on_verdict(
    conn: &mut PgConnection,
    state: &State,
    grant: &ValidatorVerdict,
    validation: Uuid,
    requested_at: NaiveDateTime,
    result: &BTreeMap<String, Box<RawValue>>,
    stale: &[&str],
) -> Result<Vec<Notice>, ApiError> {
    let collection = grant.curated();
    let at = now(state);
    let since = events::started_at(conn, collection, validation, requested_at).await?;
    let decided = events::decided_since(conn, collection, since).await?;
    let field = |key: &str| -> Value {
        result
            .get(key)
            .and_then(|raw| serde_json::from_str(raw.get()).ok())
            .unwrap_or(Value::Null)
    };
    let raw = |payload: &BTreeMap<String, Box<RawValue>>| -> Result<Box<RawValue>, ApiError> {
        to_raw_value(payload).map_err(|e| ApiError::internal(format!("verdict payload: {e}")))
    };
    let judged = || -> Result<Box<RawValue>, ApiError> {
        let mut payload = result.clone();
        if !stale.is_empty() {
            payload.insert(
                "staleItemIds".into(),
                to_raw_value(stale).map_err(|e| ApiError::internal(e.to_string()))?,
            );
        }
        raw(&payload)
    };
    let mut notices = Vec::new();
    match field("verdict").as_str() {
        Some("passed") => {
            if !decided {
                decide(conn, grant, Request::Reopen, validation).await?;
            }
            let recorded = events::record_once(
                conn,
                collection,
                "review.automated_passed",
                Actor::Validator,
                &judged()?,
                at,
            )
            .await?;
            if recorded && !decided {
                notices.push(Notice::AutomatedPassed { stale: stale.len() });
            }
        }
        Some("rejected") => {
            if !decided {
                decide(conn, grant, Request::Reject, validation).await?;
            }
            events::record_once(
                conn,
                collection,
                "review.automated_rejected",
                Actor::Validator,
                &judged()?,
                at,
            )
            .await?;
        }
        _ => {
            let handed = |reason: &str, items: Value| {
                json!({
                    "reason": reason,
                    "validationId": validation,
                    "rulesVersion": field("rulesVersion"),
                    "items": items,
                })
            };
            if field("reason").as_str() == Some("unsupported") {
                let unsupported: Vec<Value> = field("items")
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter(|item| item.get("unsupported") == Some(&Value::Bool(true)))
                    .cloned()
                    .collect();
                let ids: Vec<String> = unsupported
                    .iter()
                    .filter_map(|item| item.get("itemId").and_then(Value::as_str))
                    .map(str::to_string)
                    .collect();
                let payload = to_raw_value(&handed("unsupported_items", Value::Array(unsupported)))
                    .map_err(|e| ApiError::internal(e.to_string()))?;
                if events::record_once(
                    conn,
                    collection,
                    "review.human_required",
                    Actor::System,
                    &payload,
                    at,
                )
                .await?
                {
                    notices.push(Notice::Unsupported { unsupported: ids });
                }
            } else if field("retryable") != Value::Bool(true) {
                let payload = to_raw_value(&handed("validator_error", field("items")))
                    .map_err(|e| ApiError::internal(e.to_string()))?;
                if events::record_once(
                    conn,
                    collection,
                    "review.human_required",
                    Actor::System,
                    &payload,
                    at,
                )
                .await?
                {
                    notices.push(Notice::ValidatorFailed {
                        validation: validation.to_string(),
                    });
                }
            } else if events::record_once(
                conn,
                collection,
                "review.automated_error",
                Actor::Validator,
                &raw(result)?,
                at,
            )
            .await?
            {
                notices.push(Notice::AutomatedFailed {
                    validation: validation.to_string(),
                });
            }
        }
    }
    Ok(notices)
}

/// The validator's `request` on its grant's collection, as the transition
/// table allows it; a reopening only of its own rejection. Audited.
async fn decide(
    conn: &mut PgConnection,
    grant: &ValidatorVerdict,
    request: Request,
    validation: Uuid,
) -> Result<(), ApiError> {
    let collection = grant.curated();
    let newest = rows::newest_of_collection(conn, collection).await?;
    let from = newest.as_ref().and_then(|k| Status::parse(&k.status));
    if request == Request::Reopen
        && newest.as_ref().and_then(|k| k.reviewed_by.as_deref()) != Some(VALIDATOR)
    {
        return Ok(());
    }
    let to = match transitions::decide(
        Curation::Collection,
        from,
        request,
        transitions::Actor::Validator,
    ) {
        Err(_) => return Ok(()),
        Ok(Change::OpenRejected) => {
            let opened = crate::curation::record_collection_curation(
                conn,
                grant,
                Status::Rejected.as_str(),
                None,
            )
            .await?;
            sqlx::query("UPDATE builder.collection_curations SET reviewed_by = $2 WHERE id = $1")
                .bind(opened)
                .bind(VALIDATOR)
                .execute(&mut *conn)
                .await?;
            Status::Rejected
        }
        Ok(Change::Update {
            status: Some(Status::Rejected),
            ..
        }) => {
            let Some(newest) = &newest else {
                return Ok(());
            };
            sqlx::query(
                "UPDATE builder.collection_curations SET status = 'rejected', reviewed_by = $2, \
                   rejection_reasons = NULL, rejection_message = NULL, \
                   updated_at = GREATEST(updated_at, timezone('UTC', clock_timestamp())) WHERE id = $1",
            )
            .bind(newest.id)
            .bind(VALIDATOR)
            .execute(&mut *conn)
            .await?;
            Status::Rejected
        }
        Ok(Change::Open { .. }) => {
            crate::curation::record_collection_curation(
                conn,
                grant,
                Status::Pending.as_str(),
                None,
            )
            .await?;
            Status::Pending
        }
        Ok(change) => {
            return Err(ApiError::internal(format!(
                "the validator may not {change:?}"
            )));
        }
    };
    let target = collection.to_string();
    audit::record(
        conn,
        &audit::Entry::new(VALIDATOR, "create", "builder.curation.validator")
            .target(Some(&target))
            .detail(json!({
                "validationId": validation,
                "from": from.map(Status::as_str),
                "status": to.as_str(),
            })),
    )
    .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_page_and_its_length_are_read_as_builder_server_reads_them() {
        for (query, expected) in [
            (None, (1, 50)),
            (Some("page=2&limit=10"), (2, 10)),
            (Some("page=-4&limit=999"), (1, 200)),
            (Some("page=99999&limit=abc"), (10_000, 50)),
            (Some("page=3x&limit=+7.9"), (3, 7)),
            (Some("limit=0"), (1, 1)),
            (Some("limit=100000"), (1, 50)),
            (Some("limit=99999"), (1, 200)),
            (Some("page=%20%205&page=9"), (5, 50)),
            (Some("page=99999999999999999999999"), (10_000, 50)),
        ] {
            assert_eq!(pagination(query), expected, "{query:?}");
        }
    }
}

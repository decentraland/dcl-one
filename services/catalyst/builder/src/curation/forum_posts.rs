//! A collection's one forum topic and the committee's replies in it.
//!
//! A collection gets its topic once it is published: the publish that makes
//! it so opens the topic after it commits ([`open`]), and a manager whose
//! client finds no topic asks for it again (`POST /v1/collections/:id/post`).
//! Nothing a request sends reaches the topic, and a collection that stores
//! its link is never posted again: asking again answers the stored link.
//!
//! An attempt first claims the topic: in one transaction that locks the
//! collection row (and, for a manager's request, decides access again) the
//! collection must be published, hold no link, and no other claim may be
//! younger than [`CLAIM_HOLDS`]; it records `forum_claimed_at` and commits.
//! Then, holding no connection, it asks the forum, within [`ATTEMPT`], to
//! create the topic, and stores the link and the first post's id in a second
//! transaction, which clears the claim. A request that finds a claim within
//! [`CLAIM_HOLDS`] is told to retry; one that finds an older claim takes it
//! over. When the forum made a topic whose link was never stored, its title
//! is taken: the next attempt finds the topic the node's account made under
//! that title in the category and keeps it when its first post names the
//! collection. A standard collection whose title another collection's topic
//! holds takes a title that names its contract; a third-party one is refused.

use std::sync::atomic::Ordering;
use std::time::Duration;

use chrono::TimeDelta;
use serde_json::{Value, json};
use uuid::Uuid;

use super::forum::{Created, Failure, Forum, Topic};
use super::forum_text::{self, PostCollection, PostItem};
use super::slack::shorten;
use crate::access::{self, CollectionManage, ForumReply};
use crate::chain_state;
use crate::http::ApiError;
use crate::{SharedState, State};

/// How long a claim keeps other requests from asking the forum.
pub const CLAIM_HOLDS: TimeDelta = TimeDelta::seconds(60);
/// How long one attempt may spend on the forum.
pub const ATTEMPT: Duration = Duration::from_secs(45);
/// The longest reply, in UTF-16 code units as builder-server counts.
pub const MAX_REPLY: usize = 10_000;

/// The topics being opened after a publish committed.
#[derive(Clone, Debug, Default)]
pub struct Topics {
    in_flight: std::sync::Arc<std::sync::atomic::AtomicUsize>,
}

impl Topics {
    /// Waits until every topic started so far has finished.
    pub async fn settled(&self) {
        while self.in_flight.load(std::sync::atomic::Ordering::SeqCst) > 0 {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    }
}

fn unavailable() -> ApiError {
    ApiError::service_unavailable("The forum is not available. Try again later.")
}

struct Claim {
    collection: PostCollection,
    third_party: Option<String>,
    owner: Option<String>,
}

#[derive(sqlx::FromRow)]
struct ClaimRow {
    name: String,
    third_party_id: Option<String>,
    forum_link: Option<String>,
    fresh: Option<bool>,
}

/// Who asks for a topic: a manager of the collection whose client found
/// none, or the publish of the collection `id` that just committed, listing
/// `items` when given.
enum Asker<'a> {
    Manager(&'a CollectionManage),
    Publish(Uuid, Option<&'a [Uuid]>),
}

/// What a claim found: the topic to make, or the link already stored.
enum Claimed {
    Topic(Claim),
    Stored(String),
}

/// The first [`forum_text::MAX_ITEMS`] items of the collection `id` a post
/// lists, of `only` when given, in creation order.
async fn post_items(
    conn: &mut sqlx::PgConnection,
    id: Uuid,
    only: Option<&[Uuid]>,
) -> Result<Vec<PostItem>, ApiError> {
    Ok(sqlx::query_as(
        "SELECT i.id, i.name, i.description, i.rarity, i.data->>'category' AS category, \
           (SELECT ic.hash FROM builder.item_contents ic WHERE ic.item_id = i.id AND ic.file = i.thumbnail) AS thumbnail_hash \
         FROM builder.items i WHERE i.collection_id = $1 AND ($3::uuid[] IS NULL OR i.id = ANY($3)) \
         ORDER BY i.created_at, i.id LIMIT $2",
    )
    .bind(id)
    .bind(forum_text::MAX_ITEMS as i64)
    .bind(only)
    .fetch_all(conn)
    .await?)
}

async fn claim(state: &State, asker: Asker<'_>) -> Result<Claimed, ApiError> {
    let id = match asker {
        Asker::Manager(manage) => manage.id(),
        Asker::Publish(id, _) => id,
    };
    let mut tx = state.items.pool().begin().await?;
    let view = chain_state::for_decision(&mut tx).await?;
    let row: Option<ClaimRow> = sqlx::query_as(
        "SELECT name, third_party_id, forum_link, forum_claimed_at > timezone('UTC', clock_timestamp()) - $2::interval AS fresh \
         FROM builder.collections WHERE id = $1 FOR UPDATE",
    )
    .bind(id)
    .bind(CLAIM_HOLDS)
    .fetch_optional(&mut *tx)
    .await?;
    let (published, contract, owner, only) = match asker {
        Asker::Manager(manage) => {
            let managed = access::confirm_collection_manage(&mut tx, &view, manage).await?;
            (managed.published, managed.contract, managed.owner, None)
        }
        Asker::Publish(_, items) => match access::topic_state(&mut tx, &view, id).await? {
            Some((published, contract, owner)) => (published, contract, owner, items),
            None => (false, None, None, items),
        },
    };
    let Some(ClaimRow {
        name,
        third_party_id: third_party,
        forum_link: link,
        fresh,
    }) = row
    else {
        return Err(ApiError::not_found("Collection not found").with_data(json!({ "id": id })));
    };
    if !published {
        return Err(
            ApiError::conflict("The collection is not published").with_data(json!({ "id": id }))
        );
    }
    if let Some(link) = link {
        return Ok(Claimed::Stored(link));
    }
    if state.settings.forum.is_none() {
        return Err(unavailable());
    }
    if fresh == Some(true) {
        return Err(ApiError::service_unavailable(
            "The forum post is being created. Try again later.",
        )
        .with_data(json!({ "id": id })));
    }
    let items = post_items(&mut tx, id, only).await?;
    sqlx::query(
        "UPDATE builder.collections SET forum_claimed_at = timezone('UTC', clock_timestamp()) WHERE id = $1",
    )
    .bind(id)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(Claimed::Topic(Claim {
        collection: PostCollection {
            id,
            name,
            contract,
            items,
        },
        third_party,
        owner,
    }))
}

async fn claimed_name(state: &State, address: &str) -> Result<Option<String>, ApiError> {
    let avatar: Option<Value> = sqlx::query_scalar(
        "SELECT COALESCE(d.entity_metadata::jsonb -> 'v', d.entity_metadata::jsonb) #> '{avatars,0}' \
         FROM content.active_pointers ap JOIN content.deployments d ON d.entity_id = ap.entity_id \
         WHERE ap.pointer = $1 AND d.entity_type = 'profile'",
    )
    .bind(address.to_ascii_lowercase())
    .fetch_optional(state.items.pool())
    .await?
    .flatten();
    Ok(avatar.and_then(|avatar| {
        (avatar["hasClaimedName"] == true)
            .then(|| avatar["name"].as_str().map(str::to_string))
            .flatten()
            .filter(|name| !name.is_empty())
    }))
}

/// The topic the node's account made under `title` for the claimed
/// collection: one whose first post names the collection.
async fn kept(
    state: &State,
    forum: &Forum,
    claim: &Claim,
    title: &str,
) -> Result<Option<Topic>, Failure> {
    let Some((id, slug)) = forum.own_topic(&state.guarded, title).await? else {
        return Ok(None);
    };
    let post = forum.first_post(&state.guarded, id).await?;
    let raw = forum.raw_of(&state.guarded, post).await?;
    Ok(raw
        .contains(&claim.collection.id.to_string())
        .then_some(Topic { post, id, slug }))
}

async fn make(
    state: &State,
    forum: &Forum,
    claim: &Claim,
    title: String,
) -> Result<Topic, Failure> {
    let raw = forum_text::body(
        &claim.collection,
        &state.settings.links,
        &state.settings.builder_server_url,
    );
    let first = forum_text::remove_emojis(&title);
    if let Created::Topic(topic) = forum.create(&state.guarded, &first, &raw).await? {
        return Ok(topic);
    }
    if let Some(topic) = kept(state, forum, claim, &first).await? {
        return Ok(topic);
    }
    let (None, Some(contract)) = (&claim.third_party, &claim.collection.contract) else {
        return Err(Failure::Refused(reqwest::StatusCode::UNPROCESSABLE_ENTITY));
    };
    let retried = forum_text::remove_emojis(&forum_text::retried_title(&title, contract));
    if let Created::Topic(topic) = forum.create(&state.guarded, &retried, &raw).await? {
        return Ok(topic);
    }
    kept(state, forum, claim, &retried)
        .await?
        .ok_or(Failure::Refused(reqwest::StatusCode::UNPROCESSABLE_ENTITY))
}

async fn title(state: &State, claim: &Claim) -> Result<String, ApiError> {
    Ok(match (&claim.third_party, &claim.owner) {
        (Some(provider), _) => format!(
            "Third Party collection {} with URN: {provider}",
            claim.collection.name
        ),
        (None, owner) => {
            let owner = owner.as_deref().unwrap_or_default();
            let creator = match claimed_name(state, owner).await? {
                Some(name) => name,
                None => shorten(owner),
            };
            forum_text::title(&claim.collection.name, &creator)
        }
    })
}

/// `POST /v1/collections/:id/post`: a manager's client found no topic for
/// the published collection; the topic is made and its link answered, or
/// the stored link answered without asking the forum. The whole attempt,
/// the creator's name included, ends within [`ATTEMPT`], before its claim
/// lapses.
pub async fn post(state: &State, manage: &CollectionManage) -> Result<String, ApiError> {
    match claim(state, Asker::Manager(manage)).await? {
        Claimed::Stored(link) => Ok(link),
        Claimed::Topic(claim) => {
            let forum = state.settings.forum.as_ref().ok_or_else(unavailable)?;
            make_topic(state, forum, &claim).await
        }
    }
}

/// Makes the claimed topic and stores its link and first post unless a
/// link was stored meanwhile; the stored link.
async fn make_topic(state: &State, forum: &Forum, claim: &Claim) -> Result<String, ApiError> {
    let id = claim.collection.id;
    let topic = tokio::time::timeout(ATTEMPT, async {
        let title = title(state, claim).await?;
        make(state, forum, claim, title)
            .await
            .map_err(|failure| failure.api_error(id))
    })
    .await
    .unwrap_or_else(|_| Err(Failure::Unreachable.api_error(id)))?;
    let link = forum.link(&topic);
    let kept = sqlx::query(
        "UPDATE builder.collections SET forum_link = $2, forum_id = $3, forum_claimed_at = NULL \
         WHERE id = $1 AND forum_link IS NULL",
    )
    .bind(id)
    .bind(&link)
    .bind(topic.post)
    .execute(state.items.pool())
    .await?
    .rows_affected()
        == 1;
    if kept {
        return Ok(link);
    }
    let stored: Option<Option<String>> =
        sqlx::query_scalar("SELECT forum_link FROM builder.collections WHERE id = $1")
            .bind(id)
            .fetch_optional(state.items.pool())
            .await?;
    Ok(stored.flatten().unwrap_or(link))
}

/// After the publish of the collection `id` commits, in the background: its
/// topic made, as [`post`] makes it, listing `items` when given; or, after
/// a third-party publish of `items`, once the collection stores its topic's
/// first post, that post's text with them appended. Within [`ATTEMPT`]; a
/// failure is logged, the publish stands, and the manager's client may ask
/// again.
pub fn open(state: &SharedState, id: Uuid, items: Option<Vec<Uuid>>) {
    if state.settings.forum.is_none() {
        return;
    }
    let state = state.clone();
    let in_flight = state.topics.in_flight.clone();
    in_flight.fetch_add(1, Ordering::SeqCst);
    tokio::spawn(async move {
        publish_topic(&state, id, items.as_deref()).await;
        in_flight.fetch_sub(1, Ordering::SeqCst);
    });
}

async fn publish_topic(state: &State, id: Uuid, items: Option<&[Uuid]>) {
    let Some(forum) = state.settings.forum.as_ref() else {
        return;
    };
    let attempt = async {
        let stored: Option<(String, Option<String>, Option<i32>)> = sqlx::query_as(
            "SELECT name, forum_link, forum_id FROM builder.collections WHERE id = $1",
        )
        .bind(id)
        .fetch_optional(state.items.pool())
        .await?;
        match (stored, items) {
            (Some((name, Some(_), Some(post))), Some(items)) => {
                let mut conn = state.items.pool().acquire().await?;
                let collection = PostCollection {
                    id,
                    name,
                    contract: None,
                    items: post_items(&mut conn, id, Some(items)).await?,
                };
                drop(conn);
                let old = forum
                    .raw_of(&state.guarded, post)
                    .await
                    .map_err(|failure| failure.api_error(id))?;
                let raw = forum_text::update(
                    &old,
                    &collection,
                    &state.settings.links,
                    &state.settings.builder_server_url,
                    &(state.now)().format("%-m/%-d/%Y").to_string(),
                );
                forum
                    .update(&state.guarded, post, &raw)
                    .await
                    .map_err(|failure| failure.api_error(id))
            }
            (Some((_, Some(_), None)), Some(_)) => {
                tracing::warn!(collection = %id, "a published collection's topic has no first post stored, so its new items are not added to it");
                Ok(())
            }
            (Some((_, None, _)), items) => match claim(state, Asker::Publish(id, items)).await? {
                Claimed::Topic(claim) => make_topic(state, forum, &claim).await.map(|_| ()),
                Claimed::Stored(_) => Ok(()),
            },
            _ => Ok(()),
        }
    };
    match tokio::time::timeout(ATTEMPT, attempt).await {
        Ok(Ok(())) => {}
        Ok(Err(error)) => {
            tracing::warn!(collection = %id, status = error.status(), "the forum topic of a publish was not written")
        }
        Err(_) => {
            tracing::warn!(collection = %id, "the forum topic of a publish timed out")
        }
    }
}

fn reply_text(raw: Option<&str>, id: Uuid) -> Result<String, ApiError> {
    raw.filter(|raw| (1..=MAX_REPLY).contains(&raw.encode_utf16().count()))
        .map(|raw| raw.replace('@', "@\u{200b}"))
        .ok_or_else(|| ApiError::bad_request("Invalid forum post").with_data(json!({ "id": id })))
}

fn not_replied(id: Uuid) -> ApiError {
    ApiError::http(200, "Error creating forum post").with_data(json!({ "id": id }))
}

/// `POST /v1/collections/:id/curation/post`: replies `raw` in the
/// collection's topic, which the forum names from the topic's first post.
/// An `@` is followed by a zero-width space, so a reply mentions nobody.
pub async fn reply(state: &State, grant: &ForumReply, raw: Option<&str>) -> Result<(), ApiError> {
    let id = grant.collection();
    let post: Option<Option<i32>> =
        sqlx::query_scalar("SELECT forum_id FROM builder.collections WHERE id = $1")
            .bind(id)
            .fetch_optional(state.items.pool())
            .await?;
    let Some(post) = post else {
        return Err(ApiError::not_found("Collection not found").with_data(json!({ "id": id })));
    };
    let Some(post) = post else {
        return Err(
            ApiError::conflict("The collection does not have a forum post yet")
                .with_data(json!({ "id": id })),
        );
    };
    let text = reply_text(raw, id)?;
    let forum = state.settings.forum.as_ref().ok_or_else(unavailable)?;
    let replied = tokio::time::timeout(ATTEMPT, async {
        let Some(topic) = forum.topic_of(&state.guarded, post).await? else {
            return Ok(false);
        };
        forum.reply(&state.guarded, topic, &text).await?;
        Ok::<_, Failure>(true)
    })
    .await
    .unwrap_or(Err(Failure::Unreachable));
    match replied {
        Ok(true) => Ok(()),
        Ok(false) => Err(ApiError::conflict("The collection forum post has no topic")
            .with_data(json!({ "id": id, "forumId": post }))),
        Err(_) => {
            tracing::warn!(collection = %id, member = grant.member(), "a curation reply did not reach the forum");
            Err(not_replied(id))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_attempt_ends_before_its_claim_lapses() {
        assert!(ATTEMPT < CLAIM_HOLDS.to_std().unwrap());
    }

    #[test]
    fn a_reply_is_one_to_ten_thousand_utf16_units_and_mentions_nobody() {
        let id = Uuid::nil();
        assert!(reply_text(None, id).is_err());
        assert!(reply_text(Some(""), id).is_err());
        assert_eq!(
            reply_text(Some(&"a".repeat(MAX_REPLY)), id).unwrap().len(),
            MAX_REPLY
        );
        assert!(reply_text(Some(&"a".repeat(MAX_REPLY + 1)), id).is_err());
        assert!(reply_text(Some(&"😀".repeat(MAX_REPLY / 2 + 1)), id).is_err());
        assert_eq!(
            reply_text(Some("The collection has been assigned to @admins"), id).unwrap(),
            "The collection has been assigned to @\u{200b}admins"
        );
        let assigned = "The collection has been assigned to <a target=\"_blank\" href=\"https://market.example.org/accounts/0x01eba3f0e992f53debaf51ed3738dc2f23e09aee\">Name</a>";
        assert_eq!(reply_text(Some(assigned), id).unwrap(), assigned);
    }
}

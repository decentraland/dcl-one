//! Community notifications: the rows notifications-workers writes for each
//! community event social-service-ea broadcasts, produced here in process.
//!
//! The recipients follow upstream's broadcaster
//! (`src/logic/community/broadcaster.ts` of `decentraland/social-service-ea`
//! at `b6b2cef`) and the metadata follows the processor's event parser
//! (`processor/src/logic/event-parser.ts` of
//! `decentraland/notifications-workers` at `0f113fb`). A recipient who opted
//! out of the community (`subscription_opt_outs`, scope `community`) gets no
//! row, as the processor's opt-out filter drops it. Each row's id is a v5 uuid
//! over (type, recipient, event key), so a repeated event (a re-ban, whose key
//! is stable upstream) is the same row. A failure is logged and never fails
//! the request that caused it, as upstream's fire-and-forget broadcast.

use std::collections::BTreeSet;

use serde_json::Value;
use sqlx::PgPool;
use uuid::Uuid;

use super::store::{NewNotification, insert_notifications};
use crate::{Address, CommunityId};

const COMMUNITY_NS: Uuid = Uuid::from_bytes([
    0x5b, 0x0c, 0x9e, 0x41, 0x2a, 0x7d, 0x4f, 0x18, 0x8e, 0x63, 0xc1, 0x0a, 0x97, 0x24, 0xd3, 0x6f,
]);

/// Who an event reaches (upstream's broadcasting registry).
pub enum Audience<'a> {
    /// One wallet (`directBroadcast` and the single-recipient events).
    One(&'a Address),
    /// Every member (`broadcastToAllMembers`: RENAMED).
    Members,
    /// Moderators and members (`broadcastToAllMembersButOwner`: DELETED).
    MembersButOwner,
    /// Owners and moderators (`broadcastToOwnersAndModerators`:
    /// REQUEST_TO_JOIN_RECEIVED).
    Managers,
    /// Every member but the author (`broadcastToAllMembersButPostAuthor`).
    MembersExcept(&'a Address),
    /// The online members but the opener (`broadcastVoiceChatStarted`).
    OnlineMembersExcept {
        online: &'a BTreeSet<String>,
        except: &'a str,
    },
}

/// One community event: its notification type, upstream's event key and the
/// metadata the processor stores.
pub struct CommunityEvent {
    pub kind: &'static str,
    pub key: String,
    pub community_id: CommunityId,
    pub metadata: Value,
}

/// Raw SNS metadata precedes the notifications worker's projection and opt-out
/// filter. Its field names are intentionally different from inbox metadata.
pub(crate) fn legacy_event(
    event: &CommunityEvent,
    audience: &Audience<'_>,
    timestamp: i64,
) -> Value {
    let id = event.community_id.as_uuid();
    let metadata = &event.metadata;
    let one = match audience {
        Audience::One(address) => address.as_str(),
        _ => "",
    };
    let community = || serde_json::json!({"communityId":id,"communityName":metadata["communityName"],"thumbnailUrl":metadata["thumbnailUrl"]});
    let mut raw = match event.kind {
        "community_deleted" => {
            serde_json::json!({"id":id,"name":metadata["communityName"],"thumbnailUrl":metadata["thumbnailUrl"]})
        }
        "community_deleted_content_violation" => {
            serde_json::json!({"id":id,"name":metadata["communityName"],"ownerAddress":one,"thumbnailUrl":metadata["thumbnailUrl"]})
        }
        "community_renamed" => {
            serde_json::json!({"id":id,"oldName":metadata["oldName"],"newName":metadata["newName"],"thumbnailUrl":metadata["thumbnailUrl"]})
        }
        "community_member_removed" | "community_member_banned" => {
            serde_json::json!({"id":id,"name":metadata["communityName"],"memberAddress":one,"thumbnailUrl":metadata["thumbnailUrl"]})
        }
        "community_member_left" => serde_json::json!({"id":id,"memberAddress":one}),
        "community_ownership_transferred" => {
            let mut raw = community();
            raw["oldOwnerAddress"] = metadata["oldOwnerAddress"].clone();
            raw["newOwnerAddress"] = Value::String(one.to_owned());
            raw
        }
        "community_invite_received"
        | "community_request_to_join_accepted"
        | "community_request_to_join_received" => {
            let mut raw = community();
            raw["memberAddress"] = if metadata["memberAddress"].is_string() {
                metadata["memberAddress"].clone()
            } else {
                Value::String(one.to_owned())
            };
            if event.kind == "community_request_to_join_received" {
                raw["memberName"] = metadata["memberName"].clone();
            }
            raw
        }
        "community_post_added" => {
            let mut raw = community();
            raw["postId"] = metadata["postId"].clone();
            raw["authorAddress"] = metadata["authorAddress"].clone();
            raw["addressesToNotify"] = serde_json::json!([]);
            raw
        }
        "community_voice_chat_started" => community(),
        _ => metadata.clone(),
    };
    // A source event may omit an undefined optional field; it never serializes
    // an invented null introduced by this local projection.
    if let Some(object) = raw.as_object_mut() {
        object.retain(|_, value| !value.is_null());
    }
    serde_json::json!({"type":"community","subType":event.kind.replace('_',"-"),"key":event.key,"timestamp":timestamp,"metadata":raw})
}

/// The source broadcaster reads unfiltered membership, then the remote worker
/// applies opt-out. The existing inbox writer remains a separate consumer.
pub(crate) async fn broadcast_legacy(
    pool: &PgPool,
    publisher: &crate::legacy_events::LegacyEvents,
    event: Value,
    online: Option<Vec<String>>,
    excluded: Vec<String>,
) {
    let result = broadcast_legacy_inner(pool, publisher, event, online, excluded).await;
    if let Err(error) = result {
        tracing::warn!(%error,"community SNS broadcast failed");
    }
}

async fn broadcast_legacy_inner(
    pool: &PgPool,
    publisher: &crate::legacy_events::LegacyEvents,
    mut event: Value,
    online: Option<Vec<String>>,
    mut excluded: Vec<String>,
) -> anyhow::Result<()> {
    let subtype = event["subType"].as_str().unwrap_or_default();
    let (roles, field, single): (Option<Vec<&str>>, &str, bool) = match subtype {
        "community-deleted" => (Some(vec!["moderator", "member"]), "memberAddresses", false),
        "community-renamed" => (None, "memberAddresses", false),
        "community-request-to-join-received" => {
            (Some(vec!["moderator", "owner"]), "addressesToNotify", true)
        }
        "community-post-added" => {
            excluded.push(
                event["metadata"]["authorAddress"]
                    .as_str()
                    .unwrap_or_default()
                    .to_lowercase(),
            );
            (None, "addressesToNotify", false)
        }
        "community-voice-chat-started" => (None, "addressesToNotify", false),
        _ => return publisher.publish(&event).await,
    };
    let community = event["metadata"]["communityId"]
        .as_str()
        .or_else(|| event["metadata"]["id"].as_str())
        .ok_or_else(|| anyhow::anyhow!("Community event lacks community id"))?;
    let community = Uuid::parse_str(community)?;
    let mut recipients = Vec::<String>::new();
    let mut offset = 0_i64;
    loop {
        let page:Vec<String>=sqlx::query_scalar("SELECT member_address FROM social.community_members WHERE community_id=$1 AND ($2::text[] IS NULL OR role=ANY($2)) AND ($3::text[] IS NULL OR member_address=ANY($3)) AND (cardinality($4::text[])=0 OR member_address<>ANY($4)) ORDER BY CASE role WHEN 'owner' THEN 1 WHEN 'moderator' THEN 2 WHEN 'member' THEN 3 ELSE 4 END,joined_at LIMIT 100 OFFSET $5")
            .bind(community).bind(&roles).bind(&online).bind(&excluded).bind(offset).fetch_all(pool).await?;
        let done = page.len() < 100;
        recipients.extend(page);
        if done {
            break;
        }
        offset += 100;
    }
    if single {
        event["metadata"][field] = serde_json::json!(recipients);
        return publisher.publish(&event).await;
    }
    let original_key = event["key"].as_str().unwrap_or_default().to_owned();
    let messages: Vec<Value> = recipients
        .chunks(100)
        .enumerate()
        .map(|(index, recipients)| {
            let mut event = event.clone();
            event["key"] = Value::String(format!("{original_key}-batch-{}", index + 1));
            event["metadata"][field] = serde_json::json!(recipients);
            event
        })
        .collect();
    // Source publishMessages returns failed events; this broadcaster does not
    // retry or promote those failures to the HTTP operation.
    let _ = publisher.publish_batch(&messages).await?;
    Ok(())
}

async fn recipients(
    pool: &PgPool,
    community: CommunityId,
    audience: &Audience<'_>,
) -> sqlx::Result<Vec<String>> {
    let (roles, except): (&[&str], Option<&str>) = match audience {
        Audience::One(address) => {
            return sqlx::query_scalar(
                "SELECT $1::text WHERE NOT EXISTS (SELECT 1 FROM social.subscription_opt_outs \
                 WHERE address = $1 AND scope = 'community' AND CASE WHEN pg_input_is_valid(scope_id,'uuid') THEN scope_id::uuid END = $2)",
            )
            .bind(address.as_str())
            .bind(community)
            .fetch_all(pool)
            .await;
        }
        Audience::Members => (&["owner", "moderator", "member"], None),
        Audience::MembersButOwner => (&["moderator", "member"], None),
        Audience::Managers => (&["owner", "moderator"], None),
        Audience::MembersExcept(author) => {
            (&["owner", "moderator", "member"], Some(author.as_str()))
        }
        Audience::OnlineMembersExcept { except, .. } => {
            (&["owner", "moderator", "member"], Some(*except))
        }
    };
    let roles: Vec<&str> = roles.to_vec();
    let rows: Vec<String> = sqlx::query_scalar(
        "SELECT m.member_address FROM social.community_members m \
         WHERE m.community_id = $1 AND m.role = ANY($2) \
           AND m.member_address IS DISTINCT FROM lower($3) \
           AND NOT EXISTS (SELECT 1 FROM social.subscription_opt_outs o \
                           WHERE o.address = m.member_address AND o.scope = 'community' AND CASE WHEN pg_input_is_valid(o.scope_id,'uuid') THEN o.scope_id::uuid END = $4) \
         ORDER BY m.member_address",
    )
    .bind(community)
    .bind(&roles)
    .bind(except)
    .bind(community)
    .fetch_all(pool)
    .await?;
    Ok(match audience {
        Audience::OnlineMembersExcept { online, .. } => rows
            .into_iter()
            .filter(|a| online.contains(a.as_str()))
            .collect(),
        _ => rows,
    })
}

/// Writes the event's notification rows; returns how many were new.
pub async fn publish(
    pool: &PgPool,
    event: &CommunityEvent,
    audience: Audience<'_>,
) -> sqlx::Result<u64> {
    let batch: Vec<NewNotification> = recipients(pool, event.community_id, &audience)
        .await?
        .into_iter()
        .filter_map(|address| Address::parse(&address).ok())
        .map(|address| {
            let identity = serde_json::to_vec(&(event.kind, address.as_str(), &event.key))
                .expect("the identity holds only strings");
            NewNotification {
                id: Uuid::new_v5(&COMMUNITY_NS, &identity),
                ..NewNotification::now(address, event.kind, event.metadata.clone())
            }
        })
        .collect();
    insert_notifications(pool, &batch).await
}

/// [`publish`], its failure logged: the request that caused the event has
/// already succeeded.
pub async fn notify(pool: &PgPool, event: CommunityEvent, audience: Audience<'_>) {
    if let Err(error) = publish(pool, &event, audience).await {
        tracing::warn!(%error, kind = event.kind, community_id = %event.community_id, "community notification not written");
    }
}

/// The milliseconds upstream suffixes onto event keys that are unique per
/// occurrence (`Date.now()`).
pub fn now_ms() -> i64 {
    chrono::Utc::now().timestamp_millis()
}

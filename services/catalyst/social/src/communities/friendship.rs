//! The friendship facts the community routes decorate with: the wire
//! `friendshipStatus` enum of a member row relative to the viewer, and the
//! viewer's friends inside a batch of communities. Reads
//! `social.friendships` / `social.friendship_actions` over the ONE pool (the
//! old service reached them through a second `MUTES_PG_CONNECTION_STRING`
//! pool). Decoration: a failed read defaults to NONE / no friends.

use std::collections::HashMap;

use sqlx::PgPool;

use crate::{Address, CommunityId};

pub mod friendship_status {
    pub const REQUEST_SENT: i32 = 0;
    pub const REQUEST_RECEIVED: i32 = 1;
    pub const CANCELED: i32 = 2;
    pub const ACCEPTED: i32 = 3;
    pub const REJECTED: i32 = 4;
    pub const DELETED: i32 = 5;
    pub const BLOCKED: i32 = 6;
    pub const NONE: i32 = 7;
    pub const BLOCKED_BY: i32 = 8;

    pub const UNRECOGNIZED: i32 = -1;
}

pub fn friendship_request_status(action: &str, acting_user: &str, context_address: &str) -> i32 {
    use friendship_status::*;
    let acting_is_context = acting_user.eq_ignore_ascii_case(context_address);
    match action {
        "accept" => ACCEPTED,
        "cancel" => CANCELED,
        "delete" => DELETED,
        "reject" => REJECTED,
        "request" => {
            if acting_is_context {
                REQUEST_SENT
            } else {
                REQUEST_RECEIVED
            }
        }
        "block" => {
            if acting_is_context {
                BLOCKED
            } else {
                BLOCKED_BY
            }
        }
        _ => UNRECOGNIZED,
    }
}

/// The latest action of the viewer's friendship with each address, as the
/// wire status; addresses with no friendship row are absent (NONE).
pub async fn friendship_statuses(
    pool: &PgPool,
    viewer: &Address,
    addresses: &[String],
) -> HashMap<String, i32> {
    let mut out = HashMap::new();
    if addresses.is_empty() {
        return out;
    }
    let rows: Vec<(String, String, String)> = match sqlx::query_as(
        "SELECT DISTINCT ON (f.id) \
           CASE WHEN f.address_requester = $1 THEN f.address_requested \
                ELSE f.address_requester END AS other_user, \
           fa.action, \
           fa.acting_user \
         FROM social.friendships f \
         INNER JOIN social.friendship_actions fa ON f.id = fa.friendship_id \
         WHERE (f.address_requester = $1 AND f.address_requested = ANY($2)) \
            OR (f.address_requested = $1 AND f.address_requester = ANY($2)) \
         ORDER BY f.id, fa.timestamp DESC, fa.id DESC",
    )
    .bind(viewer)
    .bind(addresses)
    .fetch_all(pool)
    .await
    {
        Ok(r) => r,
        Err(e) => {
            tracing::warn!(error = %e, "friendship status lookup failed; defaulting members to NONE");
            return out;
        }
    };
    for (other_user, action, acting_user) in rows {
        out.insert(
            other_user.to_lowercase(),
            friendship_request_status(&action, &acting_user, viewer.as_str()),
        );
    }
    out
}

/// Up to three of the viewer's friends per community (by address order),
/// for the `friends` decoration of the listing.
pub async fn community_friends(
    pool: &PgPool,
    viewer: &Address,
    community_ids: &[CommunityId],
) -> HashMap<CommunityId, Vec<String>> {
    let mut out: HashMap<CommunityId, Vec<String>> = HashMap::new();
    if community_ids.is_empty() {
        return out;
    }
    let friend_rows: Vec<(String,)> = match sqlx::query_as(
        "SELECT DISTINCT CASE WHEN f.address_requester = $1 \
                                THEN f.address_requested \
                                ELSE f.address_requester END AS address \
         FROM social.friendships f \
         WHERE f.is_active = TRUE \
           AND (f.address_requester = $1 OR f.address_requested = $1)",
    )
    .bind(viewer)
    .fetch_all(pool)
    .await
    {
        Ok(r) => r,
        Err(e) => {
            tracing::warn!(error = %e, "community friends: friend lookup failed; defaulting to no friends");
            return out;
        }
    };
    let friends: Vec<String> = friend_rows.into_iter().map(|(a,)| a).collect();
    if friends.is_empty() {
        return out;
    }
    let rows: Vec<(CommunityId, String)> = match sqlx::query_as(
        "SELECT community_id, member_address FROM ( \
           SELECT cm.community_id, cm.member_address, \
                  ROW_NUMBER() OVER (PARTITION BY cm.community_id ORDER BY cm.member_address) AS rn \
           FROM social.community_members cm \
           WHERE cm.community_id = ANY($1) AND cm.member_address = ANY($2) \
         ) ranked \
         WHERE rn <= 3",
    )
    .bind(community_ids)
    .bind(&friends)
    .fetch_all(pool)
    .await
    {
        Ok(r) => r,
        Err(e) => {
            tracing::warn!(error = %e, "community friends: membership lookup failed; defaulting to no friends");
            return out;
        }
    };
    for (community_id, address) in rows {
        out.entry(community_id).or_default().push(address);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::friendship_request_status;
    use super::friendship_status::*;

    const ME: &str = "0xaaa";
    const OTHER: &str = "0xbbb";

    #[test]
    fn request_direction_follows_the_acting_user() {
        assert_eq!(friendship_request_status("request", ME, ME), REQUEST_SENT);
        assert_eq!(
            friendship_request_status("request", OTHER, ME),
            REQUEST_RECEIVED
        );
        assert_eq!(
            friendship_request_status("request", "0xAAA", ME),
            REQUEST_SENT
        );
    }

    #[test]
    fn blocked_vs_blocked_by() {
        assert_eq!(friendship_request_status("block", ME, ME), BLOCKED);
        assert_eq!(friendship_request_status("block", OTHER, ME), BLOCKED_BY);
        assert_eq!(friendship_request_status("block", "0xAAA", ME), BLOCKED);
    }

    #[test]
    fn terminal_actions_ignore_acting_user() {
        assert_eq!(friendship_request_status("accept", OTHER, ME), ACCEPTED);
        assert_eq!(friendship_request_status("cancel", OTHER, ME), CANCELED);
        assert_eq!(friendship_request_status("delete", OTHER, ME), DELETED);
        assert_eq!(friendship_request_status("reject", OTHER, ME), REJECTED);
    }

    #[test]
    fn unknown_action_is_unrecognized_and_none_is_seven() {
        assert_eq!(
            friendship_request_status("frobnicate", ME, ME),
            UNRECOGNIZED
        );
        assert_eq!(NONE, 7);
    }
}

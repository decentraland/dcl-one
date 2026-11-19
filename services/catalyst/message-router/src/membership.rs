//! The router's one read of social's community tables per routed packet, by shared SQL on its
//! own small pool (DATABASE.md "Message router workload"; it never writes them). One statement
//! answers both questions:
//!
//! - is the sender a member: one primary-key probe of `social.community_members`
//!   (`community_id`, `member_address`), whatever the community's state, as upstream;
//! - who receives it: the members of the community when `social.communities` is `active` and
//!   not `suspended`, other than the sender, lowercased; the `community_members_pkey` prefix
//!   serves the scan.
//!
//! Member addresses are stored lowercase; the sender's LiveKit identity is lowercased before
//! it is compared.

use async_trait::async_trait;
use sqlx::PgPool;
use uuid::Uuid;

pub struct PgMembership {
    pool: PgPool,
}

impl PgMembership {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// `None` when `sender` is not a member of `community`; otherwise the other members of the
    /// community when it is active and not suspended, none when it is not.
    pub async fn audience(
        &self,
        community: Uuid,
        sender: &str,
    ) -> sqlx::Result<Option<Vec<String>>> {
        let (member, recipients): (bool, Vec<String>) = sqlx::query_as(
            "SELECT EXISTS (SELECT 1 FROM social.community_members \
                            WHERE community_id = $1 AND member_address = $2), \
                    ARRAY(SELECT lower(cm.member_address) FROM social.communities c \
                          JOIN social.community_members cm ON cm.community_id = c.id \
                          WHERE c.id = $1 AND c.active AND NOT c.suspended \
                            AND cm.member_address <> $2)",
        )
        .bind(community)
        .bind(sender.to_lowercase())
        .fetch_one(&self.pool)
        .await?;
        Ok(member.then_some(recipients))
    }
}

#[async_trait]
impl crate::routing::Membership for PgMembership {
    async fn audience(&self, community: Uuid, sender: &str) -> Result<Option<Vec<String>>, String> {
        PgMembership::audience(self, community, sender)
            .await
            .map_err(|e| e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const OWNER: &str = "0x00000000000000000000000000000000000000a1";
    const MEMBER: &str = "0x00000000000000000000000000000000000000b2";
    const OTHER: &str = "0x00000000000000000000000000000000000000c3";
    const STRANGER: &str = "0x00000000000000000000000000000000000000d4";

    async fn community(pool: &PgPool, active: bool, members: &[(&str, &str)]) -> Uuid {
        let id = Uuid::new_v4();
        sqlx::query(
            "INSERT INTO social.communities (id, name, owner_address, active) \
             VALUES ($1, 'router test', $2, $3)",
        )
        .bind(id)
        .bind(members[0].0)
        .bind(active)
        .execute(pool)
        .await
        .unwrap();
        for (address, role) in members {
            sqlx::query(
                "INSERT INTO social.community_members (community_id, member_address, role) \
                 VALUES ($1, $2, $3)",
            )
            .bind(id)
            .bind(address)
            .bind(role)
            .execute(pool)
            .await
            .unwrap();
        }
        id
    }

    /// Behind `CATALYST_TEST_DATABASE_URL`; skips without it.
    #[tokio::test]
    async fn members_and_recipients_of_active_inactive_and_suspended_communities() {
        let Some(pool) = catalyst_db::scratch::pool(2).await else {
            return;
        };
        let members = PgMembership::new(pool.clone());
        let roles = [(OWNER, "owner"), (MEMBER, "member"), (OTHER, "moderator")];
        let active = community(&pool, true, &roles).await;
        let inactive = community(&pool, false, &roles).await;
        let suspended = community(&pool, true, &roles).await;
        sqlx::query(
            "UPDATE social.communities SET suspended = TRUE, suspended_at = now(), \
             suspended_by = 'router test' WHERE id = $1",
        )
        .bind(suspended)
        .execute(&pool)
        .await
        .unwrap();

        let checksummed = MEMBER.to_uppercase().replace("0X", "0x");
        let sorted = |found: Option<Vec<String>>| {
            found.map(|mut recipients| {
                recipients.sort();
                recipients
            })
        };
        let others = Some(vec![OWNER.to_owned(), OTHER.to_owned()]);
        assert_eq!(
            sorted(members.audience(active, MEMBER).await.unwrap()),
            others
        );
        // the LiveKit identity may arrive checksummed
        assert_eq!(
            sorted(members.audience(active, &checksummed).await.unwrap()),
            others
        );
        assert_eq!(members.audience(active, STRANGER).await.unwrap(), None);
        // a member of an inactive or suspended community has no one to send to, as upstream
        assert_eq!(
            members.audience(inactive, MEMBER).await.unwrap(),
            Some(vec![])
        );
        assert_eq!(
            members.audience(suspended, MEMBER).await.unwrap(),
            Some(vec![])
        );
        assert_eq!(
            members.audience(Uuid::new_v4(), MEMBER).await.unwrap(),
            None
        );
        let lone = community(&pool, true, &[(OWNER, "owner")]).await;
        assert_eq!(members.audience(lone, OWNER).await.unwrap(), Some(vec![]));
    }
}

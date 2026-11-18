//! The router's two reads of social's community tables, by shared SQL on its own small pool
//! (DATABASE.md "Message router workload"; it never writes them):
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

    pub async fn is_member(&self, community: Uuid, address: &str) -> sqlx::Result<bool> {
        sqlx::query_scalar(
            "SELECT EXISTS (SELECT 1 FROM social.community_members \
             WHERE community_id = $1 AND member_address = $2)",
        )
        .bind(community)
        .bind(address.to_lowercase())
        .fetch_one(&self.pool)
        .await
    }

    pub async fn recipients(&self, community: Uuid, sender: &str) -> sqlx::Result<Vec<String>> {
        sqlx::query_scalar(
            "SELECT lower(cm.member_address) FROM social.communities c \
             JOIN social.community_members cm ON cm.community_id = c.id \
             WHERE c.id = $1 AND c.active AND NOT c.suspended AND cm.member_address <> $2",
        )
        .bind(community)
        .bind(sender.to_lowercase())
        .fetch_all(&self.pool)
        .await
    }
}

#[async_trait]
impl crate::routing::Membership for PgMembership {
    async fn is_member(&self, community: Uuid, address: &str) -> Result<bool, String> {
        PgMembership::is_member(self, community, address)
            .await
            .map_err(|e| e.to_string())
    }

    async fn recipients(&self, community: Uuid, sender: &str) -> Result<Vec<String>, String> {
        PgMembership::recipients(self, community, sender)
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

        assert!(members.is_member(active, MEMBER).await.unwrap());
        // the LiveKit identity may arrive checksummed
        assert!(
            members
                .is_member(active, &MEMBER.to_uppercase().replace("0X", "0x"))
                .await
                .unwrap()
        );
        assert!(!members.is_member(active, STRANGER).await.unwrap());
        // membership alone, as upstream: the recipients read is what checks the community
        assert!(members.is_member(inactive, MEMBER).await.unwrap());
        assert!(!members.is_member(Uuid::new_v4(), MEMBER).await.unwrap());

        let mut recipients = members.recipients(active, MEMBER).await.unwrap();
        recipients.sort();
        assert_eq!(recipients, [OWNER, OTHER]);
        let mut recipients = members
            .recipients(active, &MEMBER.to_uppercase().replace("0X", "0x"))
            .await
            .unwrap();
        recipients.sort();
        assert_eq!(recipients, [OWNER, OTHER]);
        assert!(
            members
                .recipients(inactive, MEMBER)
                .await
                .unwrap()
                .is_empty()
        );
        assert!(members.is_member(suspended, MEMBER).await.unwrap());
        assert!(
            members
                .recipients(suspended, MEMBER)
                .await
                .unwrap()
                .is_empty()
        );
        assert!(
            members
                .recipients(Uuid::new_v4(), MEMBER)
                .await
                .unwrap()
                .is_empty()
        );
        let lone = community(&pool, true, &[(OWNER, "owner")]).await;
        assert!(members.recipients(lone, OWNER).await.unwrap().is_empty());
    }
}

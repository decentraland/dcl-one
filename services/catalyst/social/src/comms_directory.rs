//! Social-owned implementation of the gatekeeper directory.
use crate::comms::interfaces::{
    ModerationNotification, SocialDirectory, StreamingNotification, UserNotification,
};
use crate::{Address, CommunityId};
use chrono::NaiveDateTime;
use sqlx::PgPool;

const STREAMING_NS: uuid::Uuid = uuid::Uuid::from_bytes([
    0x5b, 0x0e, 0x7c, 0x1a, 0x3d, 0x42, 0x4f, 0x8b, 0x9a, 0x61, 0x2c, 0xe4, 0x77, 0x08, 0xd3, 0x95,
]);

pub struct Directory {
    pool: PgPool,
}
impl Directory {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait::async_trait]
impl SocialDirectory for Directory {
    async fn notify_moderation_notification(
        &self,
        transaction: &mut sqlx::PgConnection,
        notification: &ModerationNotification,
    ) -> Result<(), sqlx::Error> {
        crate::notifications::moderation::insert(transaction, notification).await
    }
    async fn notify_user_notifications(
        &self,
        transaction: &mut sqlx::PgConnection,
        notifications: &[UserNotification],
    ) -> Result<(), sqlx::Error> {
        let batch: Vec<_> = notifications
            .iter()
            .map(|n| crate::notifications::store::NewNotification {
                id: n.id,
                address: n.recipient.clone(),
                kind: n.kind.to_string(),
                metadata: n.metadata.clone(),
                occurred_at: n.occurred_at,
            })
            .collect();
        crate::notifications::store::insert_notifications(transaction, &batch).await?;
        Ok(())
    }
    async fn community_members(&self, community: CommunityId) -> Result<Vec<Address>, sqlx::Error> {
        sqlx::query_scalar("SELECT member_address FROM social.community_members WHERE community_id = $1 ORDER BY member_address").bind(community).fetch_all(&self.pool).await
    }
    async fn private_messages_privacy(&self, address: &Address) -> Result<String, sqlx::Error> {
        sqlx::query_scalar("SELECT COALESCE((SELECT private_messages_privacy FROM social.user_settings WHERE address = $1), $2)").bind(address).bind(crate::rpc::db::ABSENT_PRIVATE_MESSAGES_PRIVACY).fetch_one(&self.pool).await
    }
    async fn set_private_messages_privacy(
        &self,
        address: &Address,
        privacy: &str,
    ) -> Result<(), sqlx::Error> {
        sqlx::query("INSERT INTO social.user_settings (address, private_messages_privacy, updated_at) VALUES ($1, $2, $3) ON CONFLICT (address) DO UPDATE SET private_messages_privacy = EXCLUDED.private_messages_privacy, updated_at = EXCLUDED.updated_at").bind(address).bind(privacy).bind(chrono::Utc::now().naive_utc()).execute(&self.pool).await?;
        Ok(())
    }
    async fn record_community_voice_ended(
        &self,
        community: CommunityId,
        event_key: &str,
        event: &serde_json::Value,
        occurred_at: NaiveDateTime,
    ) -> Result<(), sqlx::Error> {
        // Ranking only: upstream produced no member notification for this event.
        let mut tx = self.pool.begin().await?;
        crate::communities::ranking::record_stream(
            &mut tx,
            community.as_uuid(),
            event_key,
            event,
            occurred_at,
        )
        .await?;
        tx.commit().await
    }
    async fn notify_streaming_notification(
        &self,
        notification: &StreamingNotification,
    ) -> Result<(), sqlx::Error> {
        let notice = crate::notifications::store::NewNotification {
            id: uuid::Uuid::new_v5(
                &STREAMING_NS,
                format!(
                    "{}:{}",
                    notification.event_key,
                    notification.recipient.as_str()
                )
                .as_bytes(),
            ),
            address: notification.recipient.clone(),
            kind: notification.kind.as_str().to_string(),
            metadata: notification.metadata.clone(),
            occurred_at: notification.occurred_at,
        };
        crate::notifications::store::insert_keyed_notification(
            &self.pool,
            &notification.event_key,
            &notice,
        )
        .await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::comms::interfaces::StreamingNotificationKind;
    #[tokio::test]
    async fn directory_preserves_privacy_defaults_and_deduplicates_streaming_retries() {
        let Some(pool) = crate::db::scratch::pool(4).await else {
            return;
        };
        let directory = Directory::new(pool.clone());
        let address =
            Address::parse(&format!("0x00000000{}", uuid::Uuid::new_v4().simple())).unwrap();
        assert_eq!(
            directory.private_messages_privacy(&address).await.unwrap(),
            "all"
        );
        directory
            .set_private_messages_privacy(&address, "only_friends")
            .await
            .unwrap();
        assert_eq!(
            directory.private_messages_privacy(&address).await.unwrap(),
            "only_friends"
        );
        let notification = StreamingNotification {
            event_key: format!("test-{}", uuid::Uuid::new_v4()),
            recipient: address.clone(),
            kind: StreamingNotificationKind::PlaceUpdated,
            metadata: serde_json::json!({"title":"Stream Location Altered"}),
            occurred_at: chrono::Utc::now().naive_utc(),
        };
        directory
            .notify_streaming_notification(&notification)
            .await
            .unwrap();
        directory
            .notify_streaming_notification(&notification)
            .await
            .unwrap();
        let count: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM social.notifications WHERE address = $1 AND broadcast_key = $2",
        )
        .bind(&address)
        .bind(&notification.event_key)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(count, 1);
        let row: (String, serde_json::Value) = sqlx::query_as("SELECT type, metadata FROM social.notifications WHERE address = $1 AND broadcast_key = $2").bind(&address).bind(&notification.event_key).fetch_one(&pool).await.unwrap();
        assert_eq!(row.0, "streaming_place_updated");
        assert_eq!(row.1, notification.metadata);
        let deliveries: i64 = sqlx::query_scalar("SELECT count(*) FROM social.notifications n JOIN social.notification_deliveries d ON d.notification_id = n.id WHERE n.address = $1 AND n.broadcast_key = $2").bind(&address).bind(&notification.event_key).fetch_one(&pool).await.unwrap();
        assert_eq!(
            deliveries, 1,
            "streaming notices enter the email outbox once"
        );
    }

    #[tokio::test]
    async fn community_voice_end_ranks_once_and_notifies_no_member() {
        let Some(pool) = crate::db::scratch::pool(4).await else {
            return;
        };
        let directory = Directory::new(pool.clone());
        let community = uuid::Uuid::new_v4();
        let member =
            Address::parse(&format!("0x00000000{}", uuid::Uuid::new_v4().simple())).unwrap();
        sqlx::query(
            "INSERT INTO social.communities (id,name,owner_address) VALUES ($1,'Voice',$2)",
        )
        .bind(community)
        .bind(&member)
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query("INSERT INTO social.community_members (community_id,member_address,role) VALUES ($1,$2,'owner')")
            .bind(community).bind(&member).execute(&pool).await.unwrap();
        let key = format!("community-streaming-ended-{}", uuid::Uuid::new_v4());
        let event = serde_json::json!({"metadata":{"communityId":community,"totalParticipants":3}});
        for _ in 0..2 {
            directory
                .record_community_voice_ended(
                    CommunityId::from_uuid(community),
                    &key,
                    &event,
                    chrono::Utc::now().naive_utc(),
                )
                .await
                .unwrap();
        }
        let notices: i64 =
            sqlx::query_scalar("SELECT count(*) FROM social.notifications WHERE address = $1")
                .bind(&member)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(notices, 0);
        let streams: i32 = sqlx::query_scalar(
            "SELECT ranking_streams_count FROM social.communities WHERE id = $1",
        )
        .bind(community)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(streams, 1, "a replayed key ranks once");
    }

    #[tokio::test]
    async fn user_notices_land_in_the_producers_transaction_once() {
        let Some(pool) = crate::db::scratch::pool(4).await else {
            return;
        };
        let directory = Directory::new(pool.clone());
        let address =
            Address::parse(&format!("0x00000000{}", uuid::Uuid::new_v4().simple())).unwrap();
        let notice = UserNotification {
            id: uuid::Uuid::new_v4(),
            recipient: address.clone(),
            kind: "worlds_permission_granted",
            metadata: serde_json::json!({"title": "World Permission Granted"}),
            occurred_at: chrono::Utc::now().naive_utc(),
        };
        let count = || async {
            sqlx::query_scalar::<_, i64>(
                "SELECT count(*) FROM social.notifications n \
                   JOIN social.notification_deliveries d ON d.notification_id = n.id \
                  WHERE n.address = $1 AND n.type = 'worlds_permission_granted'",
            )
            .bind(&address)
            .fetch_one(&pool)
            .await
            .unwrap()
        };
        let mut tx = pool.begin().await.unwrap();
        directory
            .notify_user_notifications(&mut tx, std::slice::from_ref(&notice))
            .await
            .unwrap();
        tx.rollback().await.unwrap();
        assert_eq!(count().await, 0, "a rolled-back change leaves no notice");
        for _ in 0..2 {
            let mut tx = pool.begin().await.unwrap();
            directory
                .notify_user_notifications(&mut tx, std::slice::from_ref(&notice))
                .await
                .unwrap();
            tx.commit().await.unwrap();
        }
        assert_eq!(count().await, 1);
    }
}

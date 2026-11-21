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
        let (quota, other): (Vec<_>, Vec<_>) = notifications
            .iter()
            .map(|n| crate::notifications::store::NewNotification {
                id: n.id,
                address: n.recipient.clone(),
                kind: n.kind.to_string(),
                metadata: n.metadata.clone(),
                occurred_at: n.occurred_at,
            })
            .partition(|n| {
                matches!(
                    n.kind.as_str(),
                    "worlds_missing_resources"
                        | "worlds_access_restricted"
                        | "worlds_access_restored"
                )
            });
        crate::shared::notifications::insert_notifications_once(&mut *transaction, &quota).await?;
        crate::notifications::store::insert_notifications(transaction, &other).await?;
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

    #[tokio::test]
    async fn quota_repeats_preserve_the_first_notice_and_sent_email() {
        let Some(pool) = crate::db::scratch::pool(4).await else {
            return;
        };
        eprintln!("quota persistence regression: PostgreSQL assertions enabled");
        let address =
            Address::parse(&format!("0x00000000{}", uuid::Uuid::new_v4().simple())).unwrap();
        sqlx::query("INSERT INTO social.subscriptions (address,email) VALUES ($1,'notice-recipient@example.org')")
            .bind(&address).execute(&pool).await.unwrap();
        let first_at = chrono::DateTime::from_timestamp(1_800_000_000, 0)
            .unwrap()
            .naive_utc();
        let notices: Vec<_> = [
            "worlds_missing_resources",
            "worlds_access_restricted",
            "worlds_access_restored",
            "worlds_permission_granted",
        ]
        .into_iter()
        .map(|kind| UserNotification {
            id: uuid::Uuid::new_v4(),
            recipient: address.clone(),
            kind,
            metadata: serde_json::json!({"title":"first", "when":1234}),
            occurred_at: first_at,
        })
        .collect();
        let mut first_batch = notices.clone();
        let mut repeated_in_batch = notices[0].clone();
        repeated_in_batch.metadata = serde_json::json!({"title":"later in batch", "when":4567});
        first_batch.push(repeated_in_batch);
        let mut tx = pool.begin().await.unwrap();
        Directory::new(pool.clone())
            .notify_user_notifications(&mut tx, &first_batch)
            .await
            .unwrap();
        tx.commit().await.unwrap();
        sqlx::query("UPDATE social.notifications SET read=true,read_at=$2 WHERE address=$1")
            .bind(&address)
            .bind(first_at)
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("UPDATE social.notification_deliveries SET state='sent',attempts=1 WHERE notification_id IN (SELECT id FROM social.notifications WHERE address=$1)")
            .bind(&address).execute(&pool).await.unwrap();
        let repeats: Vec<_> = notices
            .iter()
            .cloned()
            .map(|mut notice| {
                notice.metadata = serde_json::json!({"title":"later", "when":9876});
                notice.occurred_at += chrono::TimeDelta::hours(1);
                notice
            })
            .collect();
        for _ in 0..2 {
            let mut tx = pool.begin().await.unwrap();
            Directory::new(pool.clone())
                .notify_user_notifications(&mut tx, &repeats)
                .await
                .unwrap();
            tx.commit().await.unwrap();
        }
        let count: i64 =
            sqlx::query_scalar("SELECT count(*) FROM social.notifications WHERE address=$1")
                .bind(&address)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(count, 4);
        for notice in &notices {
            let row:(serde_json::Value,NaiveDateTime,bool,Option<NaiveDateTime>,String,i32,i64)=sqlx::query_as(
                "SELECT n.metadata,n.occurred_at,n.read,n.read_at,d.state,d.attempts,count(*) OVER () FROM social.notifications n JOIN social.notification_deliveries d ON d.notification_id=n.id WHERE n.id=$1"
            ).bind(notice.id).fetch_one(&pool).await.unwrap();
            let expected = if notice.kind == "worlds_permission_granted" {
                &repeats[3]
            } else {
                notice
            };
            assert_eq!(
                row,
                (
                    expected.metadata.clone(),
                    expected.occurred_at,
                    true,
                    Some(first_at),
                    "sent".to_string(),
                    1,
                    1
                ),
                "{}",
                notice.kind
            );
        }
    }
}

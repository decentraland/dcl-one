//! Offered ban answer: comms owns platform/device/scene bans, worlds owns operator blocks.
use crate::access::Ban;
use crate::interfaces::AccessPolicy;
use crate::world_identity::SceneId;
use crate::{Address, AuthorityNotEstablished};
use chrono::NaiveDateTime;

#[async_trait::async_trait]
pub trait BanStatus: Send + Sync {
    async fn ban(
        &self,
        actor: &Address,
        scene: Option<&SceneId>,
        device: Option<&str>,
        now: NaiveDateTime,
    ) -> Result<Option<Ban>, AuthorityNotEstablished>;
}

/// Full admission answer. Never calls the worlds full permission/ban operation.
pub async fn admission_ban(
    policy: &dyn AccessPolicy,
    bans: &dyn BanStatus,
    actor: &Address,
    scene: Option<&SceneId>,
    device: Option<&str>,
    now: NaiveDateTime,
) -> Result<Option<Ban>, AuthorityNotEstablished> {
    if policy.is_blocked(actor).await? {
        return Ok(Some(Ban::Blocked));
    }
    bans.ban(actor, scene, device, now).await
}

#[cfg(any(test, feature = "test-support"))]
pub mod test_support {
    use super::*;
    pub fn state(
        policy: crate::interfaces::test_support::StubPolicy,
        bans: StubBanStatus,
    ) -> crate::gatekeeper::CommsState {
        use std::sync::Arc;
        crate::gatekeeper::CommsState::new(
            sqlx::PgPool::connect_lazy("postgres://postgres@127.0.0.1:1/postgres").unwrap(),
            crate::http::Outbound::for_tests(),
            crate::gatekeeper::Settings::for_tests(),
            None,
            None,
            Arc::new(policy),
            Arc::new(crate::interfaces::test_support::StubScenes::default()),
            Arc::new(crate::presence::Presence::new()),
            Arc::new(crate::interfaces::test_support::StubDirectory::default()),
        )
        .with_ban_status(Arc::new(bans))
    }
    pub struct StubBanStatus(pub Result<Option<Ban>, AuthorityNotEstablished>);
    impl Default for StubBanStatus {
        fn default() -> Self {
            Self(Ok(None))
        }
    }
    #[async_trait::async_trait]
    impl BanStatus for StubBanStatus {
        async fn ban(
            &self,
            _: &Address,
            _: Option<&SceneId>,
            _: Option<&str>,
            _: NaiveDateTime,
        ) -> Result<Option<Ban>, AuthorityNotEstablished> {
            self.0
                .as_ref()
                .copied()
                .map_err(crate::interfaces::test_support::copy)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::test_support::StubBanStatus;
    use super::*;
    use crate::interfaces::test_support::StubPolicy;
    use std::sync::Mutex;

    fn wallet() -> Address {
        Address::parse("0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa").unwrap()
    }
    fn unavailable() -> AuthorityNotEstablished {
        AuthorityNotEstablished::store_unavailable("test", &sqlx::Error::PoolClosed)
    }

    #[tokio::test]
    async fn full_answer_keeps_precedence_no_ban_and_both_authority_faults() {
        let actor = wallet();
        let now = chrono::Utc::now().naive_utc();
        let blocked = StubPolicy {
            blocked: Ok(true),
            ..Default::default()
        };
        // Raw-store failure must never override a definite operator block.
        assert_eq!(
            admission_ban(
                &blocked,
                &StubBanStatus(Err(unavailable())),
                &actor,
                None,
                None,
                now
            )
            .await
            .unwrap(),
            Some(Ban::Blocked)
        );
        for answer in [None, Some(Ban::Platform), Some(Ban::Scene)] {
            assert_eq!(
                admission_ban(
                    &StubPolicy::default(),
                    &StubBanStatus(Ok(answer)),
                    &actor,
                    None,
                    None,
                    now
                )
                .await
                .unwrap(),
                answer
            );
        }
        assert!(
            admission_ban(
                &StubPolicy {
                    blocked: Err(unavailable()),
                    ..Default::default()
                },
                &StubBanStatus::default(),
                &actor,
                None,
                None,
                now
            )
            .await
            .is_err()
        );
        assert!(
            admission_ban(
                &StubPolicy::default(),
                &StubBanStatus(Err(unavailable())),
                &actor,
                None,
                None,
                now
            )
            .await
            .is_err()
        );
    }

    struct Probe(Mutex<Vec<(Address, Option<SceneId>, Option<String>, NaiveDateTime)>>);
    #[async_trait::async_trait]
    impl BanStatus for Probe {
        async fn ban(
            &self,
            actor: &Address,
            scene: Option<&SceneId>,
            device: Option<&str>,
            now: NaiveDateTime,
        ) -> Result<Option<Ban>, AuthorityNotEstablished> {
            self.0.lock().unwrap().push((
                actor.clone(),
                scene.cloned(),
                device.map(str::to_owned),
                now,
            ));
            Ok(None)
        }
    }
    #[tokio::test]
    async fn full_answer_forwards_scope_device_and_clock_exactly_once() {
        let probe = Probe(Mutex::new(vec![]));
        let actor = wallet();
        let scene = SceneId::parse("scene-under-test").unwrap();
        let now = chrono::Utc::now().naive_utc();
        assert_eq!(
            admission_ban(
                &StubPolicy::default(),
                &probe,
                &actor,
                Some(&scene),
                Some("device"),
                now
            )
            .await
            .unwrap(),
            None
        );
        assert_eq!(
            *probe.0.lock().unwrap(),
            vec![(actor, Some(scene), Some("device".into()), now)]
        );
    }

    #[tokio::test]
    async fn raw_ban_store_fault_is_unavailable_not_no_ban() {
        let pool = sqlx::PgPool::connect_lazy("postgres://postgres@127.0.0.1:1/postgres").unwrap();
        pool.close().await;
        assert!(matches!(
            super::super::Moderation::new(pool)
                .ban(&wallet(), None, None, chrono::Utc::now().naive_utc())
                .await,
            Err(AuthorityNotEstablished::UndeterminedStoreUnavailable { .. })
        ));
    }

    #[tokio::test]
    async fn raw_owner_preserves_address_devices_scene_expiration_lift_and_order() {
        let Some(pool) = crate::db::scratch::pool(4).await else {
            return;
        };
        let actor =
            Address::parse(&format!("0x00000000{}", uuid::Uuid::new_v4().simple())).unwrap();
        let other =
            Address::parse(&format!("0x00000000{}", uuid::Uuid::new_v4().simple())).unwrap();
        let scene = SceneId::parse(&format!("scene-{}", uuid::Uuid::new_v4().simple())).unwrap();
        let device = uuid::Uuid::new_v4().to_string();
        let now = chrono::Utc::now().naive_utc();
        let bans = super::super::Moderation::new(pool.clone());
        assert_eq!(
            bans.ban(&actor, Some(&scene), None, now).await.unwrap(),
            None
        );
        sqlx::query("INSERT INTO social.scene_bans (place_id, banned_address, banned_by) VALUES ($1, $2, $2)").bind(scene.as_str()).bind(&actor).execute(&pool).await.unwrap();
        assert_eq!(
            bans.ban(&actor, Some(&scene), None, now).await.unwrap(),
            Some(Ban::Scene)
        );
        assert_eq!(bans.ban(&actor, None, None, now).await.unwrap(), None);
        sqlx::query("INSERT INTO social.user_bans (banned_address, banned_by, reason, banned_device_id, banned_at, expires_at) VALUES ($1, 'test', 'test', $2, $3, $4)").bind(&other).bind(&device).bind(now).bind(now + chrono::Duration::hours(1)).execute(&pool).await.unwrap();
        assert_eq!(
            bans.ban(&actor, Some(&scene), Some(&device), now)
                .await
                .unwrap(),
            Some(Ban::Platform)
        );
        sqlx::query("INSERT INTO social.player_connections (address, device_id) VALUES ($1, $2)")
            .bind(&actor)
            .bind(&device)
            .execute(&pool)
            .await
            .unwrap();
        assert_eq!(
            bans.ban(&actor, Some(&scene), None, now).await.unwrap(),
            Some(Ban::Platform)
        );
        assert_eq!(
            bans.ban(&actor, Some(&scene), Some("different-device"), now)
                .await
                .unwrap(),
            Some(Ban::Scene)
        );
        assert_eq!(
            bans.ban(&other, None, None, now).await.unwrap(),
            Some(Ban::Platform)
        );
        assert_eq!(
            bans.ban(&actor, Some(&scene), None, now + chrono::Duration::hours(2))
                .await
                .unwrap(),
            Some(Ban::Scene)
        );
        sqlx::query("UPDATE social.user_bans SET lifted_at = $2, lifted_by = 'test' WHERE banned_address = $1").bind(&other).bind(now).execute(&pool).await.unwrap();
        assert_eq!(
            bans.ban(&actor, Some(&scene), None, now).await.unwrap(),
            Some(Ban::Scene)
        );
    }
}

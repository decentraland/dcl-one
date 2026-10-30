//! The LiveKit participant removal the `/ws` connector's ban sweep performs, over this state's
//! credentials and RoomService client: the gatekeeper is the one holder of the LiveKit secret, and
//! it mints every island token itself (`cluster::gateway::island`). LiveKit not configured, or the
//! `devkey`/`devsecret` placeholder on either credential, evicts nobody.

#[cfg(test)]
use std::time::Duration;

use crate::gatekeeper::CommsState;
use crate::shared::livekit::{LiveKit, is_placeholder_cred};

/// The key and secret the kick authenticates with; `None` when LiveKit is not configured or either
/// credential is a placeholder, which no real SFU accepts.
fn armed_credentials(livekit: &LiveKit) -> Option<(&str, &str)> {
    (livekit.configured
        && !is_placeholder_cred(&livekit.api_key)
        && !is_placeholder_cred(&livekit.api_secret))
    .then_some((livekit.api_key.as_str(), livekit.api_secret.as_str()))
}

impl CommsState {
    /// Whether [`Self::kick_island_participant`] reaches LiveKit.
    pub fn connector_livekit_armed(&self) -> bool {
        armed_credentials(&self.livekit).is_some()
    }

    /// Persists an island enforcement request. The primary's existing removal
    /// job performs the checked RPC; successful enqueue is not SFU settlement.
    /// Replicas may enqueue but never send RemoveParticipant here.
    pub async fn kick_island_participant(&self, room: &str, identity: &str) -> bool {
        if armed_credentials(&self.livekit).is_none() {
            return true;
        }
        if !matches!(
            crate::rooms::room_scope(room),
            Some(crate::rooms::RoomScope::Island(_))
        ) {
            return false;
        }
        let Ok(wallet) = crate::Address::parse(identity) else {
            return false;
        };
        match crate::gatekeeper::removal::connection(&self.pool, room, &wallet).await {
            Ok(()) => true,
            Err(_) => {
                tracing::warn!("island removal request could not be persisted");
                false
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::io::Write;
    use std::sync::{Arc, Mutex};

    use super::*;
    use crate::interfaces::test_support::{StubPolicy, StubScenes};

    /// A gatekeeper over `livekit` and a pool that never connects.
    fn state(livekit: LiveKit) -> CommsState {
        let mut settings = crate::gatekeeper::Settings::for_tests();
        settings.livekit = livekit;
        CommsState::new(
            sqlx::postgres::PgPoolOptions::new()
                .acquire_timeout(Duration::from_millis(50))
                .connect_lazy("postgres://lazy@127.0.0.1:1/catalyst")
                .expect("a lazy pool never connects"),
            crate::http::Outbound::for_tests(),
            settings,
            None,
            None,
            Arc::new(StubPolicy::default()),
            Arc::new(StubScenes::default()),
            Arc::new(crate::presence::Presence::new()),
            Arc::new(crate::interfaces::test_support::StubDirectory::default()),
        )
    }

    fn armed(livekit: LiveKit) -> bool {
        state(livekit).connector_livekit_armed()
    }

    async fn kick(livekit: LiveKit, room: &str, identity: &str) {
        state(livekit).kick_island_participant(room, identity).await;
    }

    /// What a deployment with real credentials resolves to.
    fn configured() -> LiveKit {
        LiveKit {
            api_key: "APIabc".into(),
            api_secret: "supersecret".into(),
            client_url: "wss://lk.example".into(),
            api_url: "http://127.0.0.1:1".into(),
            configured: true,
        }
    }

    /// The dev placeholders on either credential, and LiveKit not configured at all.
    fn unarmed() -> Vec<(&'static str, LiveKit)> {
        vec![
            (
                "both placeholders",
                LiveKit {
                    api_key: "devkey".into(),
                    api_secret: "devsecret".into(),
                    ..configured()
                },
            ),
            (
                "placeholder key",
                LiveKit {
                    api_key: "DEVKEY".into(),
                    ..configured()
                },
            ),
            (
                "placeholder secret",
                LiveKit {
                    api_secret: " DevSecret ".into(),
                    ..configured()
                },
            ),
            (
                "not configured",
                LiveKit {
                    configured: false,
                    ..configured()
                },
            ),
        ]
    }

    #[tokio::test(flavor = "current_thread")]
    async fn configured_credentials_arm_the_kick() {
        assert!(armed(configured()));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn placeholder_credentials_arm_no_kick() {
        for (case, livekit) in unarmed()
            .into_iter()
            .filter(|(case, _)| *case != "not configured")
        {
            assert!(!armed(livekit), "{case}");
        }
    }

    #[tokio::test(flavor = "current_thread")]
    async fn unconfigured_livekit_arms_no_kick() {
        let livekit = LiveKit {
            configured: false,
            ..configured()
        };
        assert!(!armed(livekit));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn an_unarmed_kick_reaches_no_room_service() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let api_url = format!("http://{}", listener.local_addr().unwrap());
        for (_, livekit) in unarmed() {
            let livekit = LiveKit {
                api_url: api_url.clone(),
                ..livekit
            };
            kick(livekit, "I7", "0xpeer").await;
        }
        assert!(
            tokio::time::timeout(Duration::from_millis(200), listener.accept())
                .await
                .is_err(),
            "no RoomService request without armed credentials"
        );

        let livekit = LiveKit {
            api_url,
            ..configured()
        };
        let armed_kick = tokio::spawn(async move { kick(livekit, "I7", "0xpeer").await });
        assert!(
            tokio::time::timeout(Duration::from_millis(200), listener.accept())
                .await
                .is_err(),
            "even configured credentials enqueue only; this path cannot reach RoomService"
        );
        armed_kick.abort();
    }

    #[tokio::test(flavor = "current_thread")]
    async fn comms_dev_credentials_arm_no_kick() {
        // What comms' `Settings::for_tests` carries: the dev credentials, configured.
        let livekit = crate::gatekeeper::Settings::for_tests().livekit;
        assert!(livekit.configured);
        assert!(!armed(livekit));
    }

    #[tokio::test]
    async fn durable_connector_enqueues_real_island_intent_without_room_service_on_replica() {
        if std::env::var("CATALYST_TEST_DATABASE_URL")
            .ok()
            .is_none_or(|value| value.trim().is_empty())
        {
            assert_ne!(
                std::env::var("GITHUB_ACTIONS").as_deref(),
                Ok("true"),
                "durable island regressions require CATALYST_TEST_DATABASE_URL in CI"
            );
            eprintln!(
                "local skip: durable island PostgreSQL fixture requires CATALYST_TEST_DATABASE_URL"
            );
            return;
        }

        let pool = crate::db::scratch::pool(4)
            .await
            .expect("mandatory disposable PostgreSQL enqueue fixture");
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let livekit = LiveKit {
            api_url: format!("http://{}", listener.local_addr().unwrap()),
            ..configured()
        };
        let mut replica = state(livekit);
        replica.pool = pool.clone();
        let room = format!("island-{}", uuid::Uuid::new_v4());
        let wallet = "0x1111111111111111111111111111111111111111";
        assert!(replica.kick_island_participant(&room, wallet).await);
        assert_eq!(sqlx::query_scalar::<_,i64>("SELECT count(*) FROM social.livekit_removal_intents WHERE cause_kind='connection' AND wallet=$1").bind(wallet).fetch_one(&pool).await.unwrap(),1);
        assert!(
            tokio::time::timeout(Duration::from_millis(100), listener.accept())
                .await
                .is_err()
        );
        assert_eq!(
            sqlx::query_scalar::<_, i64>(
                "SELECT count(*) FROM social.island_cleanup_dispatches WHERE wallet=$1"
            )
            .bind(wallet)
            .fetch_one(&pool)
            .await
            .unwrap(),
            0,
            "enqueue is not dispatch/settlement"
        );
    }

    #[derive(Clone, Default)]
    struct LogBuffer(Arc<Mutex<Vec<u8>>>);

    impl Write for LogBuffer {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            self.0.lock().unwrap().extend_from_slice(bytes);
            Ok(bytes.len())
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    /// Everything logged at any level while `f` runs on this thread.
    ///
    /// `tracing` caches a callsite's interest when the callsite is first hit. While one dispatcher
    /// alone is live, a first hit on another test's thread asks only that thread's subscriber
    /// (none) and caches "never", hiding the event from this capture. A second live dispatcher
    /// makes every first hit ask all of them.
    async fn logs_of<F: std::future::Future<Output = ()>>(f: F) -> String {
        let _second = tracing::Dispatch::new(tracing::subscriber::NoSubscriber::default());
        let buffer = LogBuffer::default();
        let writer = buffer.clone();
        let subscriber = tracing_subscriber::fmt()
            .with_max_level(tracing::Level::TRACE)
            .without_time()
            .with_ansi(false)
            .with_writer(move || writer.clone())
            .finish();
        let _guard = tracing::subscriber::set_default(subscriber);
        f.await;
        String::from_utf8(buffer.0.lock().unwrap().clone()).unwrap()
    }

    #[tokio::test(flavor = "current_thread")]
    async fn participant_removal_logs_never_retain_the_configured_url() {
        const CANARY: &str = "archipelago-livekit-private-material-canary";
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let api_url = format!("http://{}/{CANARY}", listener.local_addr().unwrap());
        drop(listener);
        let livekit = LiveKit {
            api_url,
            ..configured()
        };

        let logs = logs_of(kick(
            livekit,
            "island-room",
            "0x1111111111111111111111111111111111111111",
        ))
        .await;

        assert!(
            logs.contains("island removal request could not be persisted"),
            "{logs}"
        );
        assert!(!logs.contains(CANARY), "configured URL survived in {logs}");
    }
}

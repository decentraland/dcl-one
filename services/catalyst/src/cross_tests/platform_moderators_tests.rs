//! The gatekeeper's moderator list over social's dapps flags, wired as `serve` wires them.

use std::sync::Arc;

use axum::Json;
use axum::routing::get;
use parking_lot::Mutex;
use serde_json::{Value, json};

use crate::comms::gatekeeper::{CommsState, Settings as GatekeeperSettings};
use crate::content::{ContentState, Settings as ContentSettings};
use crate::http::Outbound;
use crate::shared::identity::Address;
use crate::storage::Storage;

const ENV_WALLET: &str = "0x00000000000000000000000000000000000000e1";
const FLAG_WALLET: &str = "0x00000000000000000000000000000000000000f1";

/// A fresh database holds no dapps document, so the env list applies; the
/// first successful refresh replaces it with the flag's list and sets the
/// refresh gauge.
#[tokio::test]
async fn the_env_list_applies_until_the_first_dapps_document_then_the_flag_decides() {
    let Some(pool) = crate::db::scratch::isolated_pool(4).await else {
        return;
    };
    let dir = std::env::temp_dir().join(format!(
        "catalyst-platform-moderators-{}",
        uuid::Uuid::new_v4().simple()
    ));
    let storage = Arc::new(Storage::from_config(&dir).await.unwrap());

    let served: Arc<Mutex<Value>> = Arc::new(Mutex::new(json!({
        "flags": { "dapps-platform_user_moderators": true },
        "variants": { "dapps-platform_user_moderators": {
            "name": "list", "enabled": true,
            "payload": { "type": "string", "value": FLAG_WALLET }
        } }
    })));
    let reply = served.clone();
    let stub = axum::Router::new().route(
        "/dapps.json",
        get(move || {
            let reply = reply.clone();
            async move { Json(reply.lock().clone()) }
        }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let server = tokio::spawn(async move { axum::serve(listener, stub).await.unwrap() });

    let content = Arc::new(ContentState::new(
        pool.clone(),
        ContentSettings::for_tests(),
        storage.clone(),
    ));
    let mut api_settings =
        crate::explore::api::Settings::for_tests(crate::explore::api::Sources::defaults());
    api_settings.mirrors_enabled = false;
    api_settings.ab_cdn_enabled = false;
    api_settings.ab_cdn_cache_dir = dir.join("ab-cdn");
    let api =
        crate::explore::api::State::new(api_settings, pool.clone(), Outbound::for_tests(), content)
            .await
            .unwrap();

    let mut settings = crate::social::communities::Settings::for_tests();
    settings.features.base_url = format!("http://{addr}");
    let communities = Arc::new(crate::social::communities::State::new(
        pool.clone(),
        storage,
        settings,
        None,
        &Outbound::for_tests(),
        crate::social::communities::State::explore_for_tests(&pool),
    ));
    let gatekeeper = CommsState::new(
        pool.clone(),
        Outbound::for_tests(),
        GatekeeperSettings {
            moderator_addresses: vec![Address::parse(ENV_WALLET).unwrap()],
            ..GatekeeperSettings::for_tests()
        },
        None,
        None,
        Arc::new(crate::worlds::policy::Policy::new(
            pool.clone(),
            crate::worlds::policy::PersonalWorldsPolicy::disabled(),
            None,
        )),
        Arc::new(crate::worlds::rooms::SceneResolver::new(pool.clone())),
        Arc::new(crate::comms::presence::Presence::new()),
        Arc::new(crate::comms::interfaces::test_support::StubDirectory::default()),
    );
    let listed = |gatekeeper: &CommsState| -> Vec<String> {
        gatekeeper
            .platform_moderators()
            .iter()
            .map(|a| a.as_str().to_string())
            .collect()
    };

    communities.load_flags().await.unwrap();
    assert!(communities.flags_configured());
    gatekeeper.set_platform_moderator_flags(communities.flags.clone());
    assert_eq!(
        listed(&gatekeeper),
        [ENV_WALLET],
        "a fresh database: the env list"
    );

    let recorder = crate::comms::metrics::prometheus_builder()
        .unwrap()
        .build_recorder();
    let _recording = metrics::set_default_local_recorder(&recorder);
    communities.refresh_flags(&api).await.unwrap();
    assert_eq!(
        listed(&gatekeeper),
        [FLAG_WALLET],
        "the fetched flag decides"
    );
    assert!(
        recorder
            .handle()
            .render()
            .contains("social_dapps_flags_last_refresh_timestamp_seconds"),
        "a successful refresh sets the gauge"
    );

    *served.lock() = json!({
        "flags": { "dapps-platform_user_moderators": true },
        "variants": { "dapps-platform_user_moderators": {
            "name": "list", "enabled": true,
            "payload": { "type": "string", "value": ENV_WALLET }
        } }
    });
    communities.refresh_flags(&api).await.unwrap();
    assert_eq!(listed(&gatekeeper), [ENV_WALLET], "a later refresh applies");

    server.abort();
    let _ = std::fs::remove_dir_all(&dir);
}

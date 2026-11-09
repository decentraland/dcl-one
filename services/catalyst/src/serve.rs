//! `catalyst serve`: builds every domain's state and router and runs the HTTP server.
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use anyhow::Context;

use crate::config::Config;
use crate::content::sync::{SyncSettings, worker};
use crate::social::{
    self, badges, communities, media, messaging, notifications, profile_images, rpc as social_rpc,
};
use crate::{
    builder, camera_reel, comms, content, db, explore, http, jobs, land_authz, ops, presence,
    price, shared, shop, storage, worlds,
};

/// How long the workers get to stop (flush, persist cursors) before they are aborted.
const JOBS_SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(30);

#[cfg(test)]
mod worlds_comms_tests {
    use super::worlds_comms_online;
    use crate::shared::livekit::LiveKit;

    fn livekit(configured: bool, client_url: &str) -> LiveKit {
        LiveKit {
            api_key: "key".into(),
            api_secret: "secret".into(),
            client_url: client_url.into(),
            api_url: "https://livekit.example.org".into(),
            configured,
        }
    }

    #[test]
    fn worlds_comms_need_real_credentials_and_a_signaling_url() {
        assert!(worlds_comms_online(&livekit(
            true,
            "wss://livekit.example.org"
        )));
        assert!(
            !worlds_comms_online(&livekit(false, "wss://livekit.example.org")),
            "dev credentials"
        );
        assert!(!worlds_comms_online(&livekit(true, "")), "no signaling URL");
    }
}

#[cfg(test)]
mod metrics_tests {
    #[tokio::test]
    async fn serve_exporter_scrapes_and_rejects_duplicate_startup() {
        // A process-global recorder needs a dedicated child process, so this
        // fixture cannot alter the recorder observed by other root tests.
        const CHILD: &str = "CATALYST_SERVE_METRICS_TEST_CHILD";
        if std::env::var_os(CHILD).is_none() {
            let result = std::process::Command::new(std::env::current_exe().unwrap())
                .args([
                    "--exact",
                    "serve::metrics_tests::serve_exporter_scrapes_and_rejects_duplicate_startup",
                    "--nocapture",
                ])
                .env(CHILD, "1")
                .output()
                .unwrap();
            assert!(
                result.status.success(),
                "{}\n{}",
                String::from_utf8_lossy(&result.stdout),
                String::from_utf8_lossy(&result.stderr)
            );
            assert!(String::from_utf8_lossy(&result.stdout).contains("1 passed"));
            return;
        }
        let reserved = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let bind = reserved.local_addr().unwrap();
        drop(reserved);
        super::install_metrics(&bind.to_string()).unwrap();
        let response = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(2))
            .build()
            .unwrap()
            .get(format!("http://{bind}/metrics"))
            .send()
            .await
            .unwrap();
        assert!(response.status().is_success());
        let rendered = response.text().await.unwrap();
        for sample in ["comms_nats_connected 0", "comms_cluster_work_running 0"] {
            assert!(rendered.contains(sample), "missing {sample}: {rendered}");
        }
        let reserved = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let failed_bind = reserved.local_addr().unwrap();
        drop(reserved);
        assert!(super::install_metrics(&failed_bind.to_string()).is_err());
        // Duplicate rejection releases its unspawned HTTP listener. Dependency
        // upkeep lasts until this isolated child's runtime exits; no retry is supported.
        let _reused = std::net::TcpListener::bind(failed_bind).unwrap();
    }

    #[test]
    fn serve_registers_health_and_work_before_events() {
        let recorder = crate::comms::metrics::prometheus_builder()
            .unwrap()
            .build_recorder();
        let handle = recorder.handle();
        metrics::with_local_recorder(&recorder, super::register_metrics);
        let rendered = handle.render();
        for sample in [
            "comms_nats_connected 0",
            "comms_cluster_events_received_total 0",
            "comms_cluster_work_running 0",
            "comms_cluster_work_queued 0",
            "comms_cluster_work_rejected_total{reason=\"wallet_full\"} 0",
        ] {
            assert!(rendered.contains(sample), "missing {sample}: {rendered}");
        }
    }
}

/// The `BVIMPOSTERS_BAKE_*` / `BVIMPOSTERS_IMPOST_*` settings.
fn imposters_bake_settings(config: &Config) -> explore::imposters::RawBakeSettings {
    let non_empty = Config::non_empty;
    explore::imposters::RawBakeSettings {
        enabled: config.imposters.bvimposters_bake_enabled,
        wrapper: non_empty(&config.imposters.bvimposters_bake_wrapper),
        impost_bin: non_empty(&config.imposters.bvimposters_impost_bin),
        impost_server: non_empty(&config.imposters.bvimposters_impost_server),
        impost_content_server: non_empty(&config.imposters.bvimposters_impost_content_server),
        impost_sdk6_adaption_layer: non_empty(
            &config.imposters.bvimposters_impost_sdk6_adaption_layer,
        ),
        queue_depth: Some(config.imposters.bvimposters_bake_queue_depth),
        timeout_secs: Some(config.imposters.bvimposters_bake_timeout_secs),
        max_failures: Some(config.imposters.bvimposters_bake_max_failures),
        quarantine_secs: Some(config.imposters.bvimposters_bake_quarantine_secs),
    }
}

/// The six write switches as `GET /writes` reports them.
fn write_switches(config: &Config) -> shared::writes::Switches {
    shared::writes::Switches {
        content: config.writes.content_writes_enabled,
        create: config.create.create_writes_enabled,
        data: config.writes.data_writes_enabled,
        social: config.writes.social_writes_enabled,
        worlds: config.worlds.worlds_writes_enabled,
        ops: config.ops.ops_writes_enabled,
    }
}

/// `WRITES_RESTORE_STAMP_DIR`: from here on every write gate checks the
/// stamps live; the startup verdict is only logged, so a node that boots
/// with a stale stamp serves its reads and takes writes once a backup and a
/// drill land.
fn install_restore_stamps(config: &Config) -> anyhow::Result<()> {
    let Some(dir) = config.restore_stamp_dir() else {
        tracing::info!("WRITES_RESTORE_STAMP_DIR unset: the write switches alone decide");
        return Ok(());
    };
    let stamps = shared::writes::RestoreStamps::from_settings(
        dir,
        config.writes.writes_backup_max_age_hours,
        config.writes.writes_drill_max_age_days,
    )
    .map_err(anyhow::Error::msg)?;
    let on = config.write_switches_on().join(",");
    match stamps.check() {
        Ok(()) => tracing::info!(dir, switches_on = %on, "restore stamps are fresh"),
        Err(hold) if on.is_empty() => {
            tracing::info!(dir, "restore stamps: {hold} (every write switch is off)")
        }
        Err(hold) => tracing::warn!(dir, switches_on = %on, "writes are held: {hold}"),
    }
    if !shared::writes::restore::install(stamps) {
        anyhow::bail!("the restore stamps were installed twice in one process");
    }
    Ok(())
}

fn register_metrics() {
    comms::gatekeeper::admission::cluster::metrics::register_series();
}

fn install_metrics(raw: &str) -> anyhow::Result<comms::metrics::PrometheusHandle> {
    let bind = comms::metrics::serve_metrics_bind(raw)?;
    let handle = comms::metrics::install_prometheus_exporter(bind)?;
    register_metrics();
    tracing::info!(%bind, "serve /metrics listening");
    Ok(handle)
}

pub async fn serve(mut config: Config) -> anyhow::Result<()> {
    config.node.init_tracing();
    let metrics_handle = install_metrics(&config.node.serve_metrics_bind)?;
    tracing::info!("{}", catalyst_config::posture::startup_line(&config));
    config.resolve_secret_files()?;
    config.content.eth_rpc_url()?;
    install_restore_stamps(&config)?;
    let sync_settings = SyncSettings::from_config(&config)?;
    let pool = db::connect_pool(
        &config.node.database_url,
        &db::PoolSettings {
            max_connections: config.node.pg_pool_size,
            ..Default::default()
        },
    )
    .await
    .context("connecting to DATABASE_URL")?;
    db::bootstrap(&pool).await?;
    worlds::rooms::ensure_world_scene_ban_release(&pool).await?;

    let storage_root = Path::new(&config.node.storage_root_folder);
    let storage = Arc::new(
        storage::Storage::from_config(storage_root)
            .await
            .with_context(|| {
                format!(
                    "opening STORAGE_ROOT_FOLDER {}",
                    config.node.storage_root_folder
                )
            })?,
    );

    // ONE SyncStatus: the orchestrator writes it, /about and /status read it.
    let sync_status = Arc::new(content::SyncStatus::default());
    let content = Arc::new(content::ContentState::with_sync(
        pool.clone(),
        content::Settings::from_config(&config),
        storage.clone(),
        sync_status.clone(),
    ));

    // ONE outbound client: peer sync and the RPC validator use it as is
    // (`raw`), every mirror and upstream reader through the guard.
    let http_client = http::Outbound::new(http::HttpClientCfg::serve(
        config.content.connections_max_idle,
    ))
    .context("building the HTTP client")?;

    // Fleet-wide jobs run in one process at a time through leases in the
    // database; an API replica (FLEET_JOBS_ENABLED=false) stands them down.
    let jobs = if config.node.job_leases_enabled {
        jobs::Jobs::with_leases(
            pool.clone(),
            jobs::LeaseSettings {
                namespace: config.node.job_lease_namespace.clone(),
                ..jobs::LeaseSettings::default()
            },
        )
    } else {
        tracing::info!("JOB_LEASES_ENABLED=false: fleet jobs run unconditionally in this process");
        jobs::Jobs::new()
    };
    if !config.node.fleet_jobs_enabled {
        tracing::info!("FLEET_JOBS_ENABLED=false: fleet jobs stand down in this process");
    }
    let jobs = Arc::new(
        jobs.with_fleet_jobs(config.node.fleet_jobs_enabled)
            .with_first_pass_now(config.node.jobs_first_pass_now),
    );
    // Every process mirrors the sync leader's state from the database into its
    // own SyncStatus; the leader (the process holding the `sync` lease) writes it.
    content::sync::status_mirror::spawn_mirror(&jobs, pool.clone(), sync_status.clone());

    // The snapshot generator: the time-windowed files /snapshots lists, so
    // another catalyst can name this node in its SYNC_SOURCE. With the sync
    // worker attached it waits for Syncing; SNAPSHOT_GENERATION_INTERVAL_HOURS=0
    // turns it off (the console's regenerate then answers 501).
    let snapshot_gen = content::snapshot_gen::Settings::from_hours(
        config.content.snapshot_generation_interval_hours,
    )
    .map(|settings| {
        content::snapshot_gen::SnapshotGenerator::new(
            content.clone(),
            sync_settings.is_some(),
            settings,
        )
    });
    match &snapshot_gen {
        Some(generator) => generator.spawn_job(&jobs),
        None => tracing::info!(
            "SNAPSHOT_GENERATION_INTERVAL_HOURS=0: no snapshot generation; /snapshots serves what content.snapshots holds"
        ),
    }

    // The explore, explorer-boot, worlds and marketplace domains: every one
    // over the same pool, client and storage; each mirror runs only when its
    // upstream is configured, and every upstream is checked at startup to be
    // a public endpoint that is not this node.
    let local_urls: Vec<String> = [
        config.node.public_url.clone(),
        config.node.content_url.clone(),
        Some(format!("http://{}", config.node.bind_addr())),
    ]
    .into_iter()
    .flatten()
    .collect();
    let local: Vec<&str> = local_urls.iter().map(String::as_str).collect();

    // ONE EIP-1654 validator (contract-wallet signatures): explore, the
    // social routes and the social socket share it.
    let eip1654: Option<Arc<dyn crate::auth::eip1654::Eip1654Validator>> = config
        .content
        .eth_rpc_url()?
        .map(|url| crate::auth::cached_rpc_validator(url, http_client.raw().clone()));

    // The content validator of the local write path (src/content/validator):
    // the old catalyrst-validator rules over the ONE pool (the escrow
    // overlay), store and client, the shared EIP-1654 validator and the
    // chain access checks over ETH_RPC_URL and RPC_ENDPOINT_POLYGON.
    let validator_settings = content::validator::Settings::parse(
        config.validator.ignore_blockchain_access_checks,
        config.writes.content_writes_enabled,
        config.validator.additional_decentraland_address.as_deref(),
        &config.node.eth_network,
        config.content.eth_rpc_url()?,
        config.economy.rpc_endpoint_polygon.as_deref(),
    )
    .context("content validator configuration")?;
    // Sync and the read-through validate every entity a peer serves.
    let judges_peer_entities = sync_settings.is_some() || config.content.read_through_enabled;
    if judges_peer_entities {
        validator_settings
            .require_chain_for_peer_entities()
            .context("content validator configuration")?;
    }
    let content_validator = Arc::new(content::validator::Sources::new(
        pool.clone(),
        storage.clone(),
        http_client.clone(),
        eip1654.clone(),
        validator_settings,
    ));
    if (config.writes.content_writes_enabled || judges_peer_entities)
        && !config.validator.ignore_blockchain_access_checks
    {
        content_validator
            .verify_chain_ids()
            .await
            .context("deploy access check RPCs")?;
    }

    // READ_THROUGH_ENABLED: a local miss (entities, content files, profiles) is
    // filled from SYNC_SOURCE, verified and stored as a synced deployment; a
    // world not held is mirrored on first read when the worlds mirror is on.
    let read_through = content::read_through::ReadThrough::from_config(
        &config,
        &local_urls,
        pool.clone(),
        storage.clone(),
        http_client.raw().clone(),
        content_validator.clone(),
    )?
    .map(Arc::new);
    if let Some(read_through) = &read_through {
        content.attach_read_through(read_through.clone());
    }
    let source =
        |enabled: bool, url: &str, var: &str| -> anyhow::Result<Option<explore::MirrorSource>> {
            Ok(if enabled {
                Some(explore::MirrorSource::checked(url, &local).context(var.to_string())?)
            } else {
                None
            })
        };
    if config.explore.events_mirror_adopt && config.explore.events_mirror_upstream {
        anyhow::bail!(
            "EVENTS_MIRROR_ADOPT adopts the mirrored events once the mirror is off: unset EVENTS_MIRROR_UPSTREAM first"
        );
    }
    let explore_settings = explore::Settings {
        content_public_url: content.settings.content_public_url.clone(),
        asset_rewrite_domain: explore::Settings::asset_rewrite_domain_for(
            config.node.public_url.as_deref(),
        ),
        events_admin_token: config.explore.catalyrst_events_admin_token.clone(),
        events_slack_webhook: config.explore.events_slack_webhook.clone(),
        events_upcoming_summaries_enabled: config.explore.events_upcoming_summaries_enabled,
        places_upstream: source(
            config.explore.places_mirror_upstream,
            &config.explore.places_upstream_url,
            "PLACES_UPSTREAM_URL",
        )?,
        places_interval: Duration::from_secs(config.explore.places_mirror_interval_secs.max(1)),
        worlds_upstream: source(
            config.explore.worlds_mirror_upstream,
            &config.explore.worlds_upstream_url,
            "WORLDS_UPSTREAM_URL",
        )?,
        worlds_interval: Duration::from_secs(config.explore.worlds_mirror_interval_secs.max(1)),
        events_upstream: source(
            config.explore.events_mirror_upstream,
            &config.explore.events_upstream_url,
            "EVENTS_UPSTREAM_URL",
        )?,
        events_interval: Duration::from_secs(config.explore.events_mirror_interval_secs.max(1)),
        events_adopt: config.explore.events_mirror_adopt,
        places_derive: Some(explore::places::derive::DeriveScope::resolve(
            config.explore.places_derive_from_content,
            config.explore.places_mirror_upstream,
        )),
        places_derive_interval: Duration::from_secs(
            config.explore.places_derive_interval_secs.max(5),
        ),
        user_writes_enabled: config.writes.data_writes_enabled,
        operator_writes_enabled: config.ops.ops_writes_enabled,
        places_admin_token: config.explore.places_admin_auth_token.clone(),
        data_team_token: config.explore.data_team_auth_token.clone(),
        places_admin_addresses: config
            .explore
            .places_admin_addresses
            .iter()
            .map(|a| a.trim().to_lowercase())
            .filter(|a| !a.is_empty())
            .collect(),
        places_base_url: config.explore.places_base_url.trim().to_string(),
        places_moderation_webhook: config.explore.places_moderation_webhook.clone(),
        snapshot_score_url: Some(config.explore.snapshot_score_url.trim().to_string())
            .filter(|u| !u.is_empty()),
        events_base_url: config
            .explore
            .events_base_url
            .trim_end_matches('/')
            .to_string(),
        jump_in_site_url: config
            .explore
            .jump_in_site_url
            .trim()
            .trim_end_matches('/')
            .to_string(),
        events_asset_base_url: explore::Settings::events_asset_base_for(&config.node.own_url()),
        realm_name: config
            .node
            .realm_name
            .as_deref()
            .map(str::trim)
            .filter(|r| !r.is_empty())
            .unwrap_or("main")
            .to_string(),
    };
    let explore_cors = explore::cors::Policy::upstream_with(&config.explore.explore_cors_origins)
        .map_err(|e| anyhow::anyhow!("EXPLORE_CORS_ORIGINS: {e}"))?;
    let explore = Arc::new(
        explore::ExploreState::new(pool.clone(), explore_settings, eip1654.clone())
            .with_storage(storage.clone())
            .with_outbound(http_client.clone())
            .with_cors(explore_cors),
    );
    explore.spawn_jobs(&jobs, http_client.clone());

    // The explorer-api readers and mirrors, the asset-bundle CDN read-through
    // cache (`/ab-cdn`, the old catalyrst-ab-cdn serving half) and the map
    // metadata routes (the old catalyrst-map JSON documents).
    let api_sources = explore::api::Sources::from_config(&config);
    let mut api_settings = explore::api::Settings::from_config(&config, api_sources);
    api_settings.ab_cdn_enabled = config.ab_cdn.ab_cdn_enabled;
    if let Some(u) = Config::non_empty(&config.ab_cdn.map_image_base_url) {
        api_settings.map_image_base_url = u.trim_end_matches('/').to_string();
    }
    api_settings.map_external_base_url = Config::non_empty(&config.ab_cdn.map_external_base_url)
        .map(|u| u.trim_end_matches('/').to_string());
    api_settings.satellite_upstream =
        Config::non_empty(&config.explore.satellite_mirror_upstream_url)
            .map(|u| u.trim_end_matches('/').to_string());
    api_settings.satellite_interval =
        Duration::from_secs(config.explore.satellite_mirror_interval_secs.max(60));
    api_settings.satellite_allow_private = config.explore.satellite_mirror_allow_private_upstream;
    api_settings.satellite_source_budget_bytes =
        config.explore.satellite_source_budget_mb.max(1) << 20;
    api_settings.satellite_output_entries = config.explore.satellite_output_entries.max(1);
    let explorer_api = explore::api::State::new(
        api_settings,
        pool.clone(),
        http_client.clone(),
        content.clone(),
    )
    .await
    .context("starting the explorer-api readers and mirrors")?;
    explorer_api.attach_eip1654(eip1654.clone());
    explorer_api.spawn_jobs(&jobs);
    // An island's /about advertises its own minimap once the grid it draws is loaded.
    let grid = Arc::downgrade(&explorer_api);
    content.attach_parcel_grid(Arc::new(move || {
        grid.upgrade().is_some_and(|api| api.map.is_ready())
    }));

    // The /about comms block (the old COMMS_WS_CONNECTOR_URL/COMMS_STATS_URL
    // probes): archipelago's core status as a module call, advertised when
    // COMMS_FIXED_ADAPTER is set.
    let about_comms = content::about_comms::Settings::resolve(content::about_comms::RawSettings {
        protocol: &config.about_comms.comms_protocol,
        fixed_adapter: config.explore.comms_fixed_adapter.as_deref(),
        version: config.about_comms.comms_version.as_deref(),
        commit_hash: config.about_comms.comms_commit_hash.as_deref(),
        max_users: config.about_comms.max_users,
        offline_when_unreachable: config.about_comms.comms_offline_when_unreachable,
        v4_control_url: config.about_comms.comms_v4_control_url.as_deref(),
        v4_control_audience: config.about_comms.comms_v4_control_audience.as_deref(),
        v4_pulse_native_endpoint: config.about_comms.comms_v4_pulse_native_endpoint.as_deref(),
        v4_pulse_webtransport_url: config
            .about_comms
            .comms_v4_pulse_webtransport_url
            .as_deref(),
        v4_pulse_certificate_hash: config
            .about_comms
            .comms_v4_pulse_certificate_hash
            .as_deref(),
        v4_pulse_audience: config.about_comms.comms_v4_pulse_audience.as_deref(),
        v4_island_refresh_url: config.about_comms.comms_v4_island_refresh_url.as_deref(),
    });
    // Built before worlds so both /about routes share one SFU probe.
    let about_comms =
        about_comms.map(|settings| Arc::new(content::about_comms::AboutCommsState::new(settings)));

    let comms_settings = comms_settings(&config)?;
    let mut worlds_settings = worlds::Settings::from_config(&config)
        .with_global_scenes_urn(config.worlds_content.global_scenes_urn.as_deref());
    worlds_settings.realm_name_strip_ens = config.worlds_content.worlds_realm_name_strip_ens;
    worlds_settings.map_estate_view_url = config.map_urls().estate_view;
    worlds_settings.livekit_client_url = comms_settings.livekit.client_url.clone();
    worlds_settings.comms_online = worlds_comms_online(&comms_settings.livekit);
    if let Some(about) = &about_comms {
        worlds_settings.sfu = about.sfu.clone();
    }
    // Worlds' visitors use this node's content unless CONTENT_URL points elsewhere.
    if Config::non_empty(&config.node.content_url).is_none() {
        worlds_settings.content_readiness =
            worlds::about::ContentReadiness::local(sync_status.clone());
    }
    let worlds_mirror = worlds::MirrorSettings::from_config(&config, &local_urls)?;
    // The live world presence: ONE map the gatekeeper's LiveKit webhook
    // feeds, read by its world mints, its own routes, worlds' status and the
    // presence archive.
    let live_presence = Arc::new(comms::presence::Presence::new());
    let worlds = Arc::new(
        worlds::WorldsState::new(
            pool.clone(),
            storage.clone(),
            worlds_settings,
            worlds_mirror,
            http_client.clone(),
        )
        .with_presence(live_presence.clone()),
    );
    worlds.spawn_jobs(&jobs);

    // The market half of the data plane (README, "Data sources"): the marketplace surface over the subgraph readers and the
    // collections/items mirrors, the LAND authorization index, the economy
    // relay, the MANA spot archive, the rentals store and the presence
    // archive, every one over the same pool, client and job lifecycle. The
    // local writes among them sit behind DATA_WRITES_ENABLED
    // (`shop::market::DataWrites`, the social switch's twin).
    let self_hosts = content::own_hosts(&config);
    let market = Arc::new(
        shop::market::MarketplaceState::new(
            pool.clone(),
            http_client.clone(),
            crate::index::market_settings(&config, &self_hosts)?,
        )
        .with_eip1654(eip1654.clone())
        .with_read_through(read_through.clone())
        .with_denylist(storage.denylist().clone())
        .with_feature_flags(shop::market::features::Source {
            url: config.shop.shop_feature_flags_url.clone(),
            origin: if config.shop.shop_feature_flags_origin.is_empty() {
                // The service answers by Referer: an origin it cannot name asks for no site, and every flag reads off.
                match url::Url::parse(&config.shop.shop_base_url).map(|u| u.origin()) {
                    Ok(origin) if origin.is_tuple() => origin.ascii_serialization(),
                    _ => {
                        tracing::warn!(
                            shop_base_url = %config.shop.shop_base_url,
                            "SHOP_FEATURE_FLAGS_ORIGIN is empty and SHOP_BASE_URL names no origin: the dapps flags are asked for no site, so they read off"
                        );
                        String::new()
                    }
                }
            } else {
                config.shop.shop_feature_flags_origin.clone()
            },
            refresh: Duration::from_secs(config.shop.shop_feature_flags_refresh_secs),
        }),
    );
    market.spawn_api_jobs(&jobs);

    let land_settings = crate::index::land_settings(&config)?;
    let land = Arc::new(land_authz::LandAuthzState::new(
        pool.clone(),
        http_client.clone(),
        land_settings,
        Some(market.index.clone()),
    ));
    explore.places.set_land_authz(land.clone());

    // The content server's local write path (src/content/{create_entity,
    // unpublish_scene,land_publish}): the write deployer over the ONE pool
    // and store with the validator above, the LAND index (the unpublish
    // route's parcel check), the shared EIP-1654 validator and the durable
    // operator denylist (content.denylist, loaded now and reloaded every
    // 60 s), behind CONTENT_WRITES_ENABLED. With CONTENT_FORWARD_UPSTREAM_URL
    // every local deploy is also queued for that catalyst and sent on by the
    // `content-forward` job (src/content/forward.rs).
    let content_forwarder = forwarder(
        "CONTENT_FORWARD_UPSTREAM_URL",
        config.writes.content_forward_upstream_url.as_deref(),
        content::forward::CONTENT_LANE,
        &config,
        &pool,
        &storage,
        &self_hosts,
        &jobs,
    )?;
    let deployer = content::write_deployer::WriteDeployer::with_optional_eip1654(
        pool.clone(),
        storage.clone(),
        eip1654.clone(),
    )
    .with_land_authz(Some(land.clone()))
    .with_validator(content_validator.clone())
    .with_denylist(content.denylist.clone())
    .with_forwarder(content_forwarder);
    content.attach_writes(config.writes.content_writes_enabled, Arc::new(deployer));
    // Expired partial uploads (content::partial_deployments) are reclaimed
    // only where they can be made: a node taking local writes.
    if config.writes.content_writes_enabled {
        content::partial_deployments::spawn(
            &jobs,
            pool.clone(),
            storage.clone(),
            content::partial_deployments::Limits::from_config(&config),
        );
    }
    let denylisted = content
        .load_denylist()
        .await
        .context("loading content.denylist")?;
    content.spawn_jobs(&jobs);
    // The remote moderation lists (DENYLIST_URLS; Decentraland's on a
    // mirror), fetched once before the routes serve and every 120 s after,
    // into the same projection; a failed fetch is logged, never fatal.
    let denylist_urls = config.content.denylist_urls()?;
    if !denylist_urls.is_empty() {
        let (remote, errors) = content
            .reload_remote_denylist(&http_client, &denylist_urls)
            .await;
        for error in &errors {
            tracing::warn!(%error, "remote denylist fetch failed at boot");
        }
        tracing::info!(
            urls = denylist_urls.len(),
            remote,
            "remote denylists loaded"
        );
    }
    content.spawn_remote_denylist(&jobs, http_client.clone(), denylist_urls);
    if config.writes.content_writes_enabled {
        tracing::warn!(
            denylisted,
            "CONTENT_WRITES_ENABLED=true: this node is the authority for local deployments and unpublishes"
        );
    } else {
        tracing::info!(
            denylisted,
            "CONTENT_WRITES_ENABLED=false: POST /entities and DELETE /scenes/{{coord}} answer 503"
        );
    }
    // The content history prune (src/content/history_prune.rs): behind
    // CONTENT_HISTORY_PRUNE_ENABLED, and off beside the writes switch (an
    // authoritative node keeps its history); it logs once why it is not running.
    content::history_prune::spawn(
        &jobs,
        pool.clone(),
        storage.clone(),
        content::history_prune::PruneOptions::from_config(&config),
    );

    let economy_settings = shop::evm::Settings {
        min_sale_value_in_wei: config.economy.min_sale_value_in_wei.clone(),
        max_transactions_per_day: config.economy.max_transactions_per_day,
        contract_addresses_url: Config::non_empty(&config.economy.contract_addresses_url),
        contract_addresses_chain_key: config.economy.contract_addresses_chain_key.clone(),
        collections_chain_id: config.economy.collections_chain_id,
        collections_fetch_interval: Duration::from_millis(
            config.economy.collections_fetch_interval_ms,
        ),
        polygon_rpc_url: Config::non_empty(&config.economy.rpc_endpoint_polygon),
        max_gas_price_allowed_in_wei: config.economy.max_gas_price_allowed_in_wei,
        max_gas_limit: config.economy.max_gas_limit,
        oz_relayer_url: Config::non_empty(&config.economy.oz_relayer_url),
        oz_relayer_id: Config::non_empty(&config.economy.oz_relayer_id),
        oz_relayer_api_key: Config::non_empty(&config.economy.oz_relayer_api_key),
        oz_relayer_speed: config.economy.oz_relayer_speed.clone(),
        oz_max_status_checks: config.economy.oz_max_status_checks,
        oz_sleep_between_checks: Duration::from_millis(
            config.economy.oz_sleep_time_between_checks_ms,
        ),
        meta_tx_broadcast_enabled: config.economy.meta_tx_broadcast_enabled,
        relayer_private_key: Config::non_empty(&config.economy.relayer_private_key),
        transactions_upstream_url: Config::non_empty(&config.economy.transactions_upstream_url),
        transactions_upstream_timeout: Duration::from_millis(
            config.economy.transactions_upstream_timeout_ms,
        ),
        admin_token: Config::non_empty(&config.economy.catalyrst_economy_admin_token),
        landiler_escrow_address: Config::non_empty(&config.economy.landiler_escrow_address),
        names_chain_id: config.economy.names_chain_id,
        eth_rpc_url: config.content.eth_rpc_url()?,
        names_max_price_wei: Config::non_empty(&config.economy.names_max_price_wei)
            .map(|s| alloy_primitives::U256::from_str_radix(s.trim(), 10))
            .transpose()
            .context("NAMES_MAX_PRICE_WEI")?,
        receipt_poll_interval: Duration::from_millis(config.economy.receipt_poll_interval_ms),
        receipt_timeout: Duration::from_millis(config.economy.receipt_timeout_ms),
        broker_reconcile_interval: Duration::from_millis(
            config.economy.broker_reconcile_interval_ms,
        ),
        mana_usd_aggregator: Config::non_empty(&config.economy.mana_usd_aggregator_address)
            .map(|s| s.trim().parse())
            .transpose()
            .context("MANA_USD_AGGREGATOR_ADDRESS")?
            .unwrap_or(shop::evm::oracle::MANA_USD_AGGREGATOR_POLYGON),
        usd_pegged_oracle_max_age_secs: config.economy.usd_pegged_oracle_max_age_secs,
        usd_pegged_slippage_bps: config.economy.usd_pegged_slippage_bps,
        data_writes_enabled: config.writes.data_writes_enabled,
    };
    // Names the variable, never echoes a value.
    economy_settings
        .validate()
        .context("economy configuration")?;
    let economy_state = Arc::new(
        shop::evm::State::new(pool.clone(), http_client.clone(), economy_settings)
            .context("provisioning the economy relayers")?,
    );
    shop::checkout::spawn_jobs(&economy_state, &jobs);

    let price = Arc::new(price::State::new(
        pool.clone(),
        http_client.clone(),
        price::Settings::resolve(
            price::RawSettings {
                poll_enabled: config.price.price_poll_enabled,
                coingecko_url: config.price.coingecko_url.clone(),
                poll_interval_secs: config.price.price_poll_interval_secs,
                fallback_sources: config.price.price_fallback_sources.clone(),
                admin_token: Config::non_empty(&config.price.price_admin_token),
                writes_enabled: config.writes.data_writes_enabled,
            },
            &self_hosts,
        )
        .context("PRICE_* / COINGECKO_URL configuration")?,
    ));
    price.spawn_jobs(&jobs);

    let signatures = Arc::new(shop::rentals::State::new(
        pool.clone(),
        Some(market.clone()),
        shop::rentals::Settings {
            writes_enabled: config.writes.data_writes_enabled,
        },
        eip1654.clone(),
    ));

    tracing::info!(
        mirrors = config.marketplace.marketplace_mirrors_enabled,
        collections_networks = market.settings.collection_networks().len(),
        marketplace_subgraph = market.settings.marketplace_subgraph_url.is_some(),
        trade_rpc_chains = market.settings.market.trade_rpc_urls.iter().count(),
        market_admin = market.settings.market.admin_token.is_some(),
        land_index = land.settings.is_configured(),
        relay_mode = economy_state.settings.relay_mode(),
        economy_admin = economy_state.settings.admin_token.is_some(),
        price_poll = config.price.price_poll_enabled,
        rentals_reader = signatures.reader_configured(),
        "data-plane routes (market half)"
    );
    if config.writes.data_writes_enabled {
        tracing::warn!(
            "DATA_WRITES_ENABLED=true: this node is the authority for signed trades, lists, relays, rentals and the presence archive"
        );
    } else {
        tracing::info!(
            "DATA_WRITES_ENABLED=false: data-plane routes serve reads; every write answers 503"
        );
    }

    // The creator plane (the old catalyrst-create bundle): the builder and the
    // camera reel over the same pool, client, blob store and validator, both
    // behind CREATE_WRITES_ENABLED (builder::writes).
    let builder = Arc::new(
        builder::State::new(
            pool.clone(),
            storage.clone(),
            content.denylist.clone(),
            http_client.clone(),
            Some(market.index.clone()),
            eip1654.clone(),
            builder::Settings::parse(builder::RawSettings {
                writes_enabled: config.create.create_writes_enabled,
                content_bucket_url: config.create.builder_content_bucket_url.clone(),
                asset_packs_bucket_url: config.create.builder_asset_packs_bucket_url.clone(),
                polygon_rpc_url: config.create.builder_polygon_rpc_url.clone(),
                admin_addresses: config.create.builder_admin_addresses.clone(),
                admin_token: config.create.catalyrst_builder_admin_token.clone(),
                newsletter_service_url: config.create.newsletter_service_url.clone(),
                newsletter_publication_id: config.create.newsletter_publication_id.clone(),
                newsletter_api_key: config.create.newsletter_service_api_key.clone(),
                foundation_url: Some(config.create.builder_foundation_url.clone()),
            })
            .context("BUILDER_* / NEWSLETTER_* configuration")?,
        )
        .context("builder state")?,
    );
    let public_url = config
        .node
        .public_url
        .clone()
        .unwrap_or_else(|| format!("http://{}", config.node.bind_addr()));
    let camera_reel = Arc::new(camera_reel::State::new(
        pool.clone(),
        storage.clone(),
        camera_reel::Settings {
            api_url: Config::non_empty(&Some(config.create.camera_reel_api_url.clone()))
                .unwrap_or_else(|| {
                    format!("{}/camera-reel-service", public_url.trim_end_matches('/'))
                }),
            bucket_url: Config::non_empty(&config.create.camera_reel_bucket_url),
            max_images_per_user: config.create.camera_reel_max_images_per_user,
            admin_token: Config::non_empty(&config.create.catalyrst_camera_reel_admin_token),
            writes_enabled: config.create.create_writes_enabled,
        },
        eip1654.clone(),
    ));
    let withheld = camera_reel
        .load_withheld()
        .await
        .context("loading camera_reel.withheld_blobs")?;
    camera_reel.spawn_ranking_job(&jobs);
    tracing::info!(
        builder = %builder.sources_label(),
        camera_reel_api_url = %camera_reel.settings.api_url,
        camera_reel_admin = camera_reel.settings.admin_token.is_some(),
        withheld_blobs = withheld,
        "creator routes"
    );
    if config.create.create_writes_enabled {
        tracing::warn!("CREATE_WRITES_ENABLED=true: this node is the authority for creator writes");
    } else {
        tracing::info!(
            "CREATE_WRITES_ENABLED=false: creator routes serve reads; every write answers 503"
        );
    }

    // The operator plane and the remaining services (the old
    // catalyrst-credits, -rpc and -bvimposters units): every one over the
    // same pool, client, blob store, validator and job lifecycle. The credits
    // ledger is local authority behind OPS_WRITES_ENABLED (the social
    // switch's twin).
    let stage5_ops = ops_states(
        &config,
        &pool,
        &http_client,
        &jobs,
        OpsSources {
            market: &market,
            price: &price,
            economy: &economy_state,
            eip1654: eip1654.clone(),
            public_url: &public_url,
        },
    )
    .await?;

    // The worlds permission core: ONE instance for the worlds product
    // surface, the storage routes, the scene host, archipelago's ban check
    // and comms (the local call that replaced the old permission-document
    // HTTP hops). Its name authority is the marketplace reader.
    let personal_worlds = worlds::policy::PersonalWorldsPolicy::new(
        config.worlds.worlds_personal_worlds_max,
        config.worlds.worlds_personal_world_max_size_bytes,
        worlds::policy::MAX_SCENE_SIZE_BYTES,
    )
    .context("WORLDS_PERSONAL_WORLDS_MAX / WORLDS_PERSONAL_WORLD_MAX_SIZE_BYTES")?;
    let land_operators = comms_settings
        .land_operators_url
        .as_deref()
        .map(|url| worlds::policy::LandOperatorsReader::new(http_client.clone(), url));
    let ens = if config.worlds.allow_ens_domains {
        let rpc = config.content.eth_rpc_url().context("ETH_RPC_URL")?;
        Some(Arc::new(worlds::ens::EnsResolver::configured(
            http_client.clone(),
            rpc.as_deref(),
            &config.node.eth_network,
        )?))
    } else {
        None
    };
    let ban_status: Arc<dyn comms::gatekeeper::moderation::BanStatus> =
        Arc::new(comms::gatekeeper::moderation::Moderation::new(pool.clone()));
    let policy = Arc::new(
        worlds::policy::Policy::new(pool.clone(), personal_worlds, Some(market.index.clone()))
            .with_ban_status(ban_status.clone())
            .with_land_operators(land_operators)
            .with_land_authz(Some(land.clone()))
            .with_max_parcels(config.worlds.max_parcels)
            .with_ens(ens, i64::from(config.worlds.ens_max_size) * 1024 * 1024),
    );
    worlds.attach_policy(policy.clone());
    // A world read-through holds each mirrored scene's signer to this policy and shares the
    // content read-through's download bound.
    if let Some(read_through) = &read_through {
        worlds.attach_read_through(worlds::WorldReadThrough {
            fills: content::read_through::Fills::new(
                content::read_through::MAX_CONCURRENT_FILLS,
                content::read_through::DEFAULT_WAIT,
            ),
            downloads: read_through.downloads().clone(),
            policy: policy.clone(),
        });
    }
    let stage5 = worlds_states(
        &config,
        &pool,
        &storage,
        &http_client,
        &policy,
        eip1654.clone(),
        &local_urls,
        &jobs,
        live_presence.clone(),
    )
    .await?;
    // One process holds every connector socket: the primary mounts /ws, /ws/v4
    // and the routes that read them, and runs the island subscriber that mints
    // for them; an API replica keeps the feed job (/core-status, /about).
    let primary = config.node.fleet_jobs_enabled;
    if primary {
        tracing::info!(
            "FLEET_JOBS_ENABLED=true: this process serves /ws, /ws/v4 and the socket-reading connector routes and runs the island subscriber; run exactly one such process per node"
        );
    } else {
        tracing::info!(
            "FLEET_JOBS_ENABLED=false: this process serves no /ws, /ws/v4 or socket-reading connector route and mints no island; the primary serves them"
        );
    }
    let comms_stage = comms_states(
        &config,
        &pool,
        &http_client,
        &content,
        &market,
        comms_settings,
        &policy,
        ban_status.clone(),
        stage5.core.clone(),
        live_presence.clone(),
        eip1654.clone(),
        primary,
        &jobs,
    )
    .await?;

    match about_comms {
        Some(about) => {
            about.attach_core(comms_stage.archipelago.clone());
            about.spawn_jobs(&jobs, http_client.raw().clone());
            tracing::info!(adapter = %about.settings.fixed_adapter, sfu_probe = %about.sfu.target(), "/about advertises comms");
            content.attach_about_comms(about);
        }
        None => tracing::info!("COMMS_FIXED_ADAPTER unset: /about carries no comms block"),
    }

    let outfits = Arc::new(shop::outfits::State::new(
        pool.clone(),
        storage.clone(),
        shop::outfits::Settings {
            creators: shop::outfits::Settings::parse_creators(&config.shop.shop_outfit_creators),
            writes_enabled: config.writes.data_writes_enabled,
        },
        eip1654.clone(),
    ));
    let mana = Arc::new(
        shop::mana::State::new(
            pool.clone(),
            http_client.clone(),
            market.clone(),
            signatures.clone(),
            shop::mana::Settings {
                writes_enabled: config.writes.data_writes_enabled,
                track_interval: Duration::from_secs(
                    config.marketplace.marketplace_index_interval_secs,
                ),
                collections_chain_id: config.economy.collections_chain_id,
            },
            eip1654.clone(),
        )
        .context("the Shop's MANA settlement venues")?,
    );
    shop::mana::spawn_jobs(&mana, &jobs);
    // The Shop prices at the settlement chain's newest marketplace's own feed
    // (`shop::mana::feed_rate`), read in the background; without an endpoint or a
    // marketplace on that chain it keeps the spot.
    let settlement = mana.settings.collections_chain_id;
    match shop::mana::feed_rate::settlement_venue(&mana.venues, settlement) {
        Some((url, marketplace)) => shop::mana::feed_rate::install(
            Arc::new(
                shop::credits::feed::ChainFeed::new(
                    Arc::new(shop::evm::chain::ChainComponent::new(
                        http_client.clone(),
                        &url,
                        shop::evm::chain::DEFAULT_RPC_TIMEOUT,
                    )),
                    config.shop.credits_usd_pegged_feed_max_age_secs,
                )
                .reading_past_tolerance(),
            ),
            settlement,
            marketplace,
            config.shop.credits_usd_pegged_feed_max_age_secs,
        ),
        None => tracing::info!(
            chain_id = settlement,
            "shop MANA/USD: no endpoint or marketplace for the settlement chain; the Shop prices at the market spot"
        ),
    }
    let social = social_states(
        &config,
        &pool,
        &storage,
        &http_client,
        &content,
        &market,
        comms_stage.gatekeeper.clone(),
        eip1654,
        &explore,
        explorer_api.onboarding.clone(),
        metrics_handle,
    )?;
    social
        .rpc
        .start_topology(&jobs, comms_stage.archipelago.feed.topology.clone());
    social
        .rpc
        .start_fanout(&jobs, comms_stage.gatekeeper.bus.clone());
    market.attach_profile_images(social.profile_images.clone());
    let subscriptions = Arc::new(shop::subscriptions::State::new(
        pool.clone(),
        Arc::new(social.notifications.email.clone()),
        shop::subscriptions::Settings {
            shop_base_url: config.shop.shop_base_url.clone(),
            delivery_interval: Duration::from_secs(config.shop.shop_notify_delivery_interval_secs),
            watch_interval: Duration::from_secs(config.shop.shop_notify_watch_interval_secs),
            writes_enabled: config.writes.data_writes_enabled,
        },
        outfits.eip1654.clone(),
    ));
    subscriptions.spawn_jobs(&jobs);
    shop::coupons::favorites::spawn_job(&market, config.shop.shop_base_url.clone(), &jobs);
    explore
        .live
        .set_participants(comms_stage.gatekeeper.clone());
    social
        .communities
        .set_voice_eviction(comms_stage.gatekeeper.clone());
    social.communities.set_chain_index(market.index.clone());
    social.communities.load_flags().await?;
    if let Err(error) = social.communities.refresh_flags(&explorer_api).await {
        tracing::warn!(%error,"initial DAPPS flags refresh failed; retaining the stored document");
    }
    social
        .communities
        .spawn_flags_job(&jobs, explorer_api.clone());
    social
        .communities
        .referral
        .set_denylist(explorer_api.denylist.clone());
    communities::ranking::spawn_jobs(&social.communities, &jobs);
    communities::compliance::spawn_job(&social.communities, &jobs);
    // A permission, access-setting or block edit reaches the running rooms
    // at once: the host's sockets and comms' LiveKit participants.
    if let Some(host) = &stage5.scene_host {
        stage5.core.add_revocation_sink(host.clone());
    }
    stage5
        .core
        .add_revocation_sink(Arc::new(worlds::core::CommsRoomRevocations(
            comms_stage.gatekeeper.clone(),
        )));
    // A community exit re-decides admission to the worlds listing it.
    social
        .communities
        .set_world_access_revocation(Arc::new(WorldsCommunityAccess(stage5.core.clone())));
    // The presence archive reads archipelago, comms and the worlds core as
    // module calls (the old ARCHIPELAGO_URL / COMMS_URL / WORLDS_SERVER_URL hops).
    let presence = Arc::new(presence::PresenceState::new(
        pool.clone(),
        presence::Settings::from_switches(
            config.presence.presence_collector_enabled,
            config.writes.data_writes_enabled,
        )
        .with_knobs(
            config.presence.presence_snapshot_interval_secs,
            Some(&config.presence.presence_genesis_realm),
            config.presence.presence_retention_days,
        ),
        presence::sources::ModuleSources::new(
            comms_stage.archipelago.clone(),
            comms_stage.gatekeeper.clone(),
            stage5.core.clone(),
        ),
    ));
    presence.spawn_jobs(&jobs);
    social.notifications.spawn_jobs(&jobs);
    comms_stage.gatekeeper.spawn_jobs(&jobs);
    social.media.spawn_jobs(&jobs);
    social.rpc.spawn_jobs(&jobs);
    social.segment_webhooks.spawn_jobs(&jobs);
    if config.writes.social_writes_enabled {
        let communities = social.communities.clone();
        jobs.spawn_periodic_leased(
            "social-referral-rewards",
            jobs::PeriodicCfg::every(Duration::from_secs(60)),
            move |_ctx| {
                let communities = communities.clone();
                async move {
                    communities
                        .referral
                        .drain_pending_rewards()
                        .await
                        .map_err(|error| {
                            anyhow::anyhow!("referral reward drain failed: {error:?}")
                        })?;
                    Ok(())
                }
            },
        );
    }
    communities::fed::spawn_job(&social.communities, &jobs);
    // Copies of the upstream public communities; not local authority, so
    // not behind SOCIAL_WRITES_ENABLED (the explore mirrors' rule). A join or
    // leave is a local membership; every other write on them answers 409
    // naming the upstream (communities::mirror::refuse_mirrored_writes).
    if let Some(upstream) = source(
        config.social.communities_mirror_upstream,
        &config.social.communities_upstream_url,
        "COMMUNITIES_UPSTREAM_URL",
    )? {
        let thumbnail_base =
            explore::MirrorSource::checked(&config.social.communities_mirror_thumbnail_url, &local)
                .context("COMMUNITIES_MIRROR_THUMBNAIL_URL")?;
        social.communities.set_mirror_source(upstream.base());
        communities::mirror::spawn_job(
            social.communities.clone(),
            http_client.clone(),
            communities::mirror::MirrorConfig {
                source: upstream,
                thumbnail_base,
                interval: Duration::from_secs(
                    config.social.communities_mirror_interval_secs.max(1),
                ),
            },
            &jobs,
        );
    }
    if config.writes.social_writes_enabled {
        tracing::warn!("SOCIAL_WRITES_ENABLED=true: this node is the authority for social writes");
    } else {
        tracing::info!(
            "SOCIAL_WRITES_ENABLED=false: social routes serve reads; every write answers 503"
        );
    }
    if config.worlds.worlds_writes_enabled {
        tracing::warn!(
            "WORLDS_WRITES_ENABLED=true: this node is the authority for world publishes, permissions and storage"
        );
    } else {
        tracing::info!(
            "WORLDS_WRITES_ENABLED=false: worlds routes serve reads; every write answers 503"
        );
    }

    // The orchestrator's pause/resume/force handle goes to the operator
    // console; without a sync worker its controls answer 501.
    let sync_control = match &sync_settings {
        Some(settings) => Some(
            worker::start(
                settings,
                db::connect_pool(
                    &config.node.database_url,
                    &db::PoolSettings {
                        max_connections: config.node.sync_pool_size(),
                        ..Default::default()
                    },
                )
                .await
                .context("connecting the sync pool to DATABASE_URL")?,
                storage.clone(),
                sync_status,
                http_client.raw().clone(),
                content::sync_deployer::SyncValidation::Content(content_validator.clone()),
                &jobs,
            )
            .await
            .context("starting the sync worker")?
            .control,
        ),
        None if config.node.serve_unsynced => {
            tracing::warn!(
                "--serve-unsynced: reporting Syncing with no sync worker attached; \
                 development only, never a readiness signal"
            );
            worker::serve_unsynced(&config, &sync_status);
            worker::spawn_startup_repair(&jobs, pool.clone());
            None
        }
        None => {
            tracing::info!(
                "SYNC_ENABLED=false: no sync worker; the node reports the sync leader's state from the database, Bootstrapping until one has written it"
            );
            worker::spawn_startup_repair(&jobs, pool.clone());
            None
        }
    };
    // With sync off the read-through's undecided rows still need the retry and prune loops.
    if sync_settings.is_none()
        && let Some(read_through) = &read_through
    {
        worker::start_failed_deployments(
            &worker::FailedDeploymentSettings::from_config(&config),
            read_through.sources().to_vec(),
            pool.clone(),
            storage.clone(),
            http_client.raw().clone(),
            content::sync_deployer::SyncValidation::Content(content_validator.clone()),
            &jobs,
        );
    }

    // The operator console (the old catalyrst-live /admin API): SIWE sign-in
    // over ADMIN_ADDRESSES + SESSION_SECRET, the sync controls, the cache
    // flush, the failed-deployments re-arm/purge, the job/cache/sync/mirror
    // reports and the ops.admin_audit log.
    let abgen = Config::non_empty(&config.admin.abgen_admin_url)
        .map(|url| {
            ops::admin::console::Abgen::parse(
                &url,
                Config::non_empty(&config.admin.ab_registry_admin_token),
                &http_client,
            )
        })
        .transpose()
        .context("ABGEN_ADMIN_URL")?;
    let ops_admin = Arc::new(
        ops::admin::State::new(
            pool.clone(),
            ops::admin::Settings::parse(ops::admin::RawSettings {
                session_secret: Config::non_empty(&config.admin.session_secret),
                admin_addresses: config.admin.admin_addresses.clone(),
                session_ttl_secs: config.admin.admin_session_ttl_secs,
                cookie_insecure: config.admin.admin_cookie_insecure,
                eth_network: config.node.eth_network.clone(),
                writes_enabled: config.ops.ops_writes_enabled,
            }),
            content.clone(),
            jobs.clone(),
            sync_control,
        )
        // The console's /admin/api/<family>/* calls into these modules in
        // this process (the old console proxied to each over loopback HTTP).
        .with_domains(ops::admin::console::Domains {
            credits: Some(stage5_ops.credits.clone()),
            price: Some(price.clone()),
            comms: Some(comms_stage.gatekeeper.clone()),
            social_rpc: Some(social.rpc.clone()),
            communities: Some(social.communities.clone()),
            notifications: Some(social.notifications.clone()),
            badges: Some(social.badges.clone()),
            worlds: Some(stage5.core.clone()),
            scene_host: stage5.scene_host.clone(),
            camera_reel: Some(camera_reel.clone()),
            builder: Some(builder.clone()),
            rpc: Some(stage5_ops.rpc.clone()),
            explorer_api: Some(explorer_api.clone()),
            explore: Some(explore.clone()),
            abgen,
            social_writes_enabled: config.writes.social_writes_enabled,
            worlds_writes_enabled: stage5.writes_enabled,
        }),
    );
    tracing::info!(
        console = ops_admin.settings.admin_enabled(),
        sync_control = ops_admin.sync_control.is_some(),
        credits_admin = stage5_ops.credits.settings.admin_token.is_some(),
        stripe = stage5_ops.credits.settings.stripe_secret_key.is_some(),
        rpc_networks = stage5_ops.rpc.upstreams_snapshot().len(),
        rpc_admin = stage5_ops.rpc.settings.admin_token.is_some(),
        imposters_bake = stage5_ops.imposters.settings.bake.is_some(),
        abgen_admin = ops_admin.domains.abgen.is_some(),
        "operator-plane routes"
    );
    if config.ops.ops_writes_enabled {
        tracing::warn!(
            "OPS_WRITES_ENABLED=true: this node is the authority for the credits ledger and the operator console's writes"
        );
    } else {
        tracing::info!(
            "OPS_WRITES_ENABLED=false: operator-plane routes serve reads; every write answers 503"
        );
    }
    let addr = config.node.bind_addr();
    // The old gateway's location table (catalyrst/nixos/web-gateway.nix): the
    // content routes and /lambdas at the root, every other service under the
    // prefix nginx used to strip; the same routes also at the paths their own
    // vhost served, so a per-vhost proxy needs no rewrite. Anything else falls
    // through to the content 404. The canonical Shop tree (`shop::router`:
    // the market surface, the credits resources, the rentals store, the
    // MANA top-up reads, the operator routes) is under /shop-api; the
    // market surface alone answers again under /marketplace-api, its legacy
    // alias: its /v1/items and /v1/collections collide with the builder's at
    // the root, and the lambdas stay at the root. The economy routes answer at the root
    // (the old web tier's /v1/transactions location) and under
    // /transactions-api (the gateway's strip prefix, with the old /health and
    // /ping there only); the price feed at the root (/api/v3/simple/price,
    // as web.nix served it) and under /price; the rentals store at the root
    // and under /signatures-api; the LAND routes at their /lambdas paths; the
    // presence archive under /presence, the paths the sites call. The
    // connector answers at the root and under /archipelago-ea-stats; its
    // socket routes only on the primary.
    let socket_restart = comms_stage.archipelago.restart.clone();
    let socket_drain = comms_stage.archipelago.sockets.clone();
    let socket_cap = comms_stage.archipelago.settings.server.max_sockets;
    let archipelago = crate::comms::archipelago::mounts(comms_stage.archipelago, primary);
    let blobs = content.clone();
    let app = content::build_router(content)
        .merge(shop::market::router(market.clone()))
        .nest(
            "/shop-api",
            shop::router(shop::States {
                market: market.clone(),
                credits: stage5_ops.credits.clone(),
                economy: economy_state.clone(),
                signatures: signatures.clone(),
                outfits: outfits.clone(),
                subscriptions: subscriptions.clone(),
                mana,
            }),
        )
        .merge(shop::campaigns::builder_alias_router(market.clone()))
        // events-notifier.<domain> reaches /events-notifier (gateway subdomain rule).
        .nest(
            "/events-notifier",
            shop::market::tips::router(market.clone()),
        )
        .nest("/marketplace-api", shop::market::market_router(market))
        .merge(land_authz::router(land))
        .merge(economy_routes(&economy_state))
        .merge(shop::evm::contracts_document_router())
        .nest(
            "/transactions-api",
            economy_routes(&economy_state).merge(shop::evm::health_router(economy_state)),
        )
        .merge(price::router(price.clone()))
        .nest("/price", price::router(price))
        .merge(shop::rentals::router(signatures.clone()))
        .nest("/signatures-api", shop::rentals::router(signatures))
        .nest("/presence", presence::router(presence))
        .merge(explore::router(explore.clone()))
        .nest("/places", explore::router(explore.clone()))
        .nest("/events", explore::router(explore))
        .merge(worlds::app_router(worlds, blobs, &stage5))
        .merge(live_presence_routes(live_presence, stage5.writes_enabled))
        .merge(archipelago.root)
        .nest("/archipelago-ea-stats", archipelago.stats)
        .merge(explore::api::router(explorer_api))
        // The gatekeeper's mounts, where `social::router` held them: the Pulse
        // room authority, the world comms mints and the LiveKit webhook (the
        // worlds content server's root path) at the root, the mints and the
        // webhook under `/worlds-content-server`, and the gatekeeper's routes
        // with social's MLS routes under `/comms-gatekeeper` behind the
        // social writes switch.
        .merge(comms::gatekeeper::room_authority_router(
            &comms_stage.gatekeeper,
        ))
        .merge(comms::gatekeeper::admission::world_mint::router(
            comms_stage.gatekeeper.clone(),
        ))
        .merge(social::writes::guard(
            comms::gatekeeper::webhook_router(comms_stage.gatekeeper.clone()),
            config.writes.social_writes_enabled,
        ))
        .nest(
            "/worlds-content-server",
            comms::gatekeeper::admission::world_mint::router(comms_stage.gatekeeper.clone()).merge(
                social::writes::guard(
                    comms::gatekeeper::webhook_router(comms_stage.gatekeeper.clone()),
                    config.writes.social_writes_enabled,
                ),
            ),
        )
        .nest(
            "/comms-gatekeeper",
            social::writes::guard(
                comms::gatekeeper::router(comms_stage.gatekeeper.clone())
                    .merge(social::messaging::router(social.messaging.clone())),
                config.writes.social_writes_enabled,
            ),
        )
        .merge(social::router(&social, config.writes.social_writes_enabled))
        .merge(builder::root_router(builder.clone()))
        .nest("/builder-api", builder::router(builder))
        .nest("/camera-reel-service", camera_reel::router(camera_reel));
    // The operator plane under the old gateway's prefixes: credits under
    // /credits only (its /users, /cart and /checkout would collide at the root; the
    // handlers verify a signed fetch over the public path and the route-local
    // one), the relay under /rpc (rpc.<domain>/x -> /rpc/x),
    // the imposter zips under /bvimposters (the prefix baked into the explorer
    // wasm), the console API at the root (/admin/auth, /admin/api/*).
    let app = app
        .nest("/credits", shop::credits::router(stage5_ops.credits))
        .nest("/rpc", ops::rpc::router(stage5_ops.rpc))
        .nest(
            "/bvimposters",
            explore::imposters::router(stage5_ops.imposters),
        )
        .merge(ops::admin::content::router(ops_admin.clone()))
        .merge(ops::admin::snapshots::router(
            ops_admin.clone(),
            snapshot_gen,
        ))
        .merge(ops::admin::router(ops_admin))
        .merge(shared::writes::status_router(write_switches(&config)));
    // A request a gate, an auth check or a handler answers before reading
    // its body is drained, not reset: nginx writes a buffered upload after
    // the answer, and a reset fails that write, answers 502 and counts the
    // process down. Up to the largest body a route admits (a deployment).
    let app = shared::drain::drain_unread(
        app,
        content::create_entity::MAX_DEPLOYMENT_BYTES
            .max(worlds::core::deploy::MAX_UPLOAD_WIRE_SIZE_BYTES) as u64,
    );
    // Upstream's router under /content/: static segments matched whatever
    // their case, and its `Allow` list on a 405 (also under
    // /worlds-content-server/). Other paths pass untouched.
    let app = content::routing::upstream_routing(app);
    // The old executable trimmed trailing slashes outside its router
    // (`/api/events/` is what the explorer requests), so the layer wraps the
    // finished app rather than any one router.
    let app = tower_http::normalize_path::NormalizePath::trim_trailing_slash(app);
    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .with_context(|| format!("binding {addr} (HTTP_SERVER_HOST + HTTP_SERVER_PORT)"))?;
    let shutdown = shutdown_signal();
    tracing::info!(%addr, "catalyst listening");
    let shutdown_jobs = jobs.clone();
    axum::serve(
        listener,
        axum::ServiceExt::<axum::extract::Request>::into_make_service_with_connect_info::<
            std::net::SocketAddr,
        >(app),
    )
    .with_graceful_shutdown(async move {
        shutdown.await;
        socket_restart.cancel();
        // Workers first, so cursors and batches settle while the listener
        // still answers; then the listener drains.
        let aborted = shutdown_jobs.shutdown(JOBS_SHUTDOWN_TIMEOUT).await;
        if !aborted.is_empty() {
            tracing::warn!(?aborted, "jobs aborted at shutdown");
        }
        // Upgraded WebSockets outlive their HTTP requests. Wait for the writers themselves,
        // whose permits cover a 5s in-flight frame plus a 5s terminal message and 5s close.
        match tokio::time::timeout(
            Duration::from_secs(16),
            socket_drain.acquire_many_owned(socket_cap),
        )
        .await
        {
            Ok(Ok(_writers_stopped)) => {}
            _ => tracing::warn!("socket writer drain exceeded its shutdown budget"),
        }
        tracing::info!("workers stopped; closing the listener");
    })
    .await?;
    Ok(())
}

/// The old transactions-server surface: the relay (`economy`) plus the
/// Shop's checkout and treasury routes over the same state, at the root
/// and under `/transactions-api`.
fn economy_routes(state: &Arc<shop::evm::State>) -> axum::Router {
    shop::evm::router(state.clone())
        .merge(shop::checkout::router(state.clone()))
        .merge(shop::treasury::router(state.clone()))
}

/// The live presence's two routes where the worlds core routers held them:
/// at the root and under `/worlds-content-server`, each behind the worlds
/// writes gate (`WORLDS_WRITES_ENABLED`) as those routers were, applied
/// before nesting so the gate sees the route-local path.
pub(crate) fn live_presence_routes(
    presence: Arc<comms::presence::Presence>,
    writes_enabled: bool,
) -> axum::Router {
    axum::Router::new()
        .merge(worlds::writes::guard(
            comms::presence::router(presence.clone()),
            writes_enabled,
        ))
        .nest(
            "/worlds-content-server",
            worlds::writes::guard(comms::presence::router(presence), writes_enabled),
        )
}

/// What the operator-plane states read from the rest of `serve`.
struct OpsSources<'a> {
    market: &'a Arc<shop::market::MarketplaceState>,
    price: &'a Arc<price::State>,
    economy: &'a Arc<shop::evm::State>,
    eip1654: Option<Arc<dyn crate::auth::eip1654::Eip1654Validator>>,
    public_url: &'a str,
}

/// The operator-plane states, one per old unit, with their jobs attached.
struct OpsStates {
    credits: Arc<shop::credits::State>,
    rpc: ops::rpc::Shared,
    imposters: explore::imposters::Shared,
}

async fn ops_states(
    config: &Config,
    pool: &sqlx::PgPool,
    http_client: &http::Outbound,
    jobs: &Arc<jobs::Jobs>,
    sources: OpsSources<'_>,
) -> anyhow::Result<OpsStates> {
    let non_empty = Config::non_empty;

    let credits = Arc::new(shop::credits::State::new(
        pool.clone(),
        http_client.clone(),
        crate::index::credits_settings(config)?,
        shop::credits::Sources {
            market: Some(sources.market.clone()),
            price: Some(sources.price.clone()),
            economy: Some(sources.economy.clone()),
        },
        sources.eip1654.clone(),
    ));
    credits.spawn_jobs(jobs);

    let rpc = ops::rpc::State::new(
        ops::rpc::Settings::parse(
            [
                ("mainnet", config.rpc.rpc_upstream_mainnet.clone()),
                ("ethereum", config.rpc.rpc_upstream_ethereum.clone()),
                ("sepolia", config.rpc.rpc_upstream_sepolia.clone()),
                ("polygon", config.rpc.rpc_upstream_polygon.clone()),
                ("matic", config.rpc.rpc_upstream_matic.clone()),
                ("amoy", config.rpc.rpc_upstream_amoy.clone()),
                ("mumbai", config.rpc.rpc_upstream_mumbai.clone()),
                ("arbitrum", config.rpc.rpc_upstream_arbitrum.clone()),
                ("optimism", config.rpc.rpc_upstream_optimism.clone()),
                ("avalanche", config.rpc.rpc_upstream_avalanche.clone()),
                ("binance", config.rpc.rpc_upstream_binance.clone()),
                ("fantom", config.rpc.rpc_upstream_fantom.clone()),
            ],
            non_empty(&config.rpc.catalyrst_rpc_admin_token),
            config.rpc.catalyrst_rpc_allow_private_upstream,
            &format!("{}/rpc", sources.public_url.trim_end_matches('/')),
        )
        .context("RPC_UPSTREAM_* / CATALYRST_RPC_* configuration")?,
        http_client.clone(),
        pool.clone(),
    );

    let imposters = explore::imposters::State::new(
        explore::imposters::Settings::resolve(explore::imposters::RawSettings {
            store_root: non_empty(&Some(config.imposters.bvimposters_store_root.clone())),
            store_max_bytes: Some(config.imposters.bvimposters_store_max_bytes),
            mem_cache_bytes: Some(config.imposters.bvimposters_mem_cache_bytes),
            bake: imposters_bake_settings(config),
        })
        .context("BVIMPOSTERS_* configuration")?,
    )
    .await
    .context("opening the imposter store")?;
    imposters.spawn_jobs(jobs);

    Ok(OpsStates {
        credits,
        rpc,
        imposters,
    })
}

/// The comms gatekeeper's settings, resolved once: the gatekeeper reads the
/// LiveKit credentials from them (it also removes the participants the
/// archipelago connector kicks), and the worlds settings take the LiveKit
/// client URL.
/// Worlds advertise their comms mint only against a real LiveKit: dev
/// credentials mint tokens no cluster accepts, and no signaling URL has no room.
fn worlds_comms_online(livekit: &shared::livekit::LiveKit) -> bool {
    livekit.configured && !livekit.client_url.is_empty()
}

fn comms_settings(config: &Config) -> anyhow::Result<comms::gatekeeper::Settings> {
    let non_empty = Config::non_empty;
    comms::gatekeeper::Settings::resolve(comms::gatekeeper::RawSettings {
        livekit_host: config.social.livekit_host.clone(),
        livekit_api_host: non_empty(&config.social.livekit_api_host),
        livekit_ws_url: non_empty(&config.social.livekit_ws_url),
        livekit_api_key: non_empty(&config.social.livekit_api_key).unwrap_or_default(),
        livekit_api_secret: non_empty(&config.social.livekit_api_secret).unwrap_or_default(),
        livekit_allow_dev_creds: config.social.livekit_allow_dev_creds,
        private_messages_room_id: config.social.private_messages_room_id.clone(),
        authoritative_server_address: non_empty(&config.social.authoritative_server_address),
        moderator_token: non_empty(&config.social.moderator_token),
        moderator_addresses: config.social.platform_user_moderators.clone(),
        gatekeeper_auth_token: non_empty(&config.social.comms_gatekeeper_auth_token),
        land_operators_url: non_empty(&config.social.comms_land_operators_url),
        lease_authorizations_url: Some(config.social.lease_authorizations_url.clone()),
        voice_connection_interrupted_ttl_ms: config.social.voice_chat_connection_interrupted_ttl_ms,
        voice_initial_connection_ttl_ms: config.social.voice_chat_initial_connection_ttl_ms,
        community_voice_no_moderator_ttl_ms: config.social.community_voice_chat_no_moderator_ttl_ms,
        voice_expired_batch_size: config.social.voice_chat_expired_batch_size,
        commit_hash: config.node.commit_hash.clone(),
    })
    .context("LIVEKIT_* / COMMS_* configuration")
}

/// The upstream forwarder of one lane when `var` names an upstream (a public
/// http(s) base URL that is not this node), its job spawned; `None` when unset.
#[allow(clippy::too_many_arguments)]
fn forwarder(
    var: &str,
    url: Option<&str>,
    lane: content::forward::Lane,
    config: &Config,
    pool: &sqlx::PgPool,
    storage: &Arc<storage::Storage>,
    self_hosts: &[String],
    jobs: &Arc<jobs::Jobs>,
) -> anyhow::Result<Option<Arc<content::forward::Forwarder>>> {
    let Some(url) = url.map(str::trim).filter(|u| !u.is_empty()) else {
        return Ok(None);
    };
    crate::shared::upstream::check_upstream(var, url, self_hosts)?;
    content::forward::check_target(var, url).map_err(anyhow::Error::msg)?;
    let forwarder = Arc::new(content::forward::Forwarder::new(
        pool.clone(),
        storage.clone(),
        content::forward::client().context("building the forwarding HTTP client")?,
        lane,
        url,
        content::forward::Retry::new(
            config.writes.forward_max_attempts,
            config.writes.forward_retry_base_secs,
        ),
    ));
    forwarder.spawn(jobs);
    tracing::info!(var, lane = lane.job, "forwarding local deploys upstream");
    Ok(Some(forwarder))
}

/// The stage-5 worlds states: the product surface (publish, permissions,
/// settings, presence, operator routes), the world/player/env storage, the
/// authoritative scene host (behind `SCENE_HOST_ENABLED`) and the worlds
/// federation mirror, every one over the ONE pool, blob store, client,
/// permission core and job lifecycle; `worlds::app_router` owns their
/// routes. `catalyst pulse` is the realtime process beside this one; they
/// meet on `NATS_URL` and the room authority comms mounts.
#[allow(clippy::too_many_arguments)]
async fn worlds_states(
    config: &Config,
    pool: &sqlx::PgPool,
    storage: &Arc<storage::Storage>,
    http_client: &http::Outbound,
    policy: &Arc<worlds::policy::Policy>,
    eip1654: Option<Arc<dyn crate::auth::eip1654::Eip1654Validator>>,
    local_urls: &[String],
    jobs: &Arc<jobs::Jobs>,
    presence: Arc<comms::presence::Presence>,
) -> anyhow::Result<worlds::Stage5> {
    let max_concurrent_uploads = worlds::core::partial::uploads_within_pool(
        config.worlds.max_concurrent_uploads,
        config.node.pg_pool_size,
    );
    if max_concurrent_uploads < config.worlds.max_concurrent_uploads {
        tracing::warn!(
            configured = config.worlds.max_concurrent_uploads,
            effective = max_concurrent_uploads,
            pg_pool_size = config.node.pg_pool_size,
            "MAX_CONCURRENT_UPLOADS lowered to three quarters of PG_POOL_SIZE"
        );
    }
    let non_empty = Config::non_empty;

    let core = Arc::new(
        worlds::core::CoreState::new(
            pool.clone(),
            storage.clone(),
            policy.clone(),
            worlds::core::Settings::resolve(
                non_empty(&config.worlds.catalyrst_worlds_admin_token),
                worlds::core::UploadLimits {
                    max_in_flight_upload_bytes: config.worlds.max_in_flight_upload_bytes,
                    max_concurrent_uploads,
                    max_in_flight_upload_files: config.worlds.max_in_flight_upload_files,
                    multipart_upload_timeout_ms: config.worlds.multipart_upload_timeout_ms,
                    deployment_processing_timeout_ms: config
                        .worlds
                        .deployment_processing_timeout_ms,
                    max_concurrent_uploads_per_source: u64::from(
                        config.content.max_concurrent_uploads_per_source,
                    ),
                    max_in_flight_upload_bytes_per_source: config
                        .content
                        .max_in_flight_upload_bytes_per_source
                        .unwrap_or(worlds::core::upload_limits::DEFAULT_MAX_IN_FLIGHT_UPLOAD_BYTES_PER_SOURCE),
                },
            )
            .and_then(|s| {
                s.with_global_scenes_urn(config.worlds_content.global_scenes_urn.as_deref())
                    .with_scene_eviction_ttl_ms(config.worlds.scene_eviction_ttl_ms)
                    .with_public_url(&worlds::Settings::from_config(config).public_url)
                    .with_max_file_count(config.worlds.max_file_count)
                    .with_access_caps(
                        config.worlds.access_max_wallets,
                        config.worlds.access_max_communities,
                    )
                    .with_owner_reconcile_interval_secs(
                        config.worlds.worlds_owner_reconcile_interval_secs,
                    )
                    // The variables content's partial uploads share; each domain keeps its own budget.
                    .with_partial_uploads(worlds::core::PartialUploadLimits {
                        pending_deployment_ttl_ms: config.content.pending_deployment_ttl_ms,
                        cleanup_interval_ms: config.worlds.partial_upload_cleanup_interval_ms,
                        max_pending_per_deployer: u64::from(
                            config.content.max_pending_deployments_per_deployer,
                        ),
                        max_pending_per_world: config.worlds.max_pending_deployments_per_world,
                        pending_bytes_per_world_factor: config
                            .worlds
                            .pending_bytes_per_world_factor,
                        max_pending_bytes_per_deployer: config
                            .content
                            .max_pending_bytes_per_deployer,
                        max_pending_bytes_per_source: config.worlds.max_pending_bytes_per_source,
                        max_pending_bytes: config.content.max_pending_bytes,
                        max_bytes_per_minute: config.content.max_partial_upload_bytes_per_minute,
                        completed_upload_ttl_ms: config.worlds.completed_upload_ttl,
                    })
            })
            .context(
                "MAX_IN_FLIGHT_UPLOAD_* / MAX_CONCURRENT_UPLOADS / *_TIMEOUT_MS configuration",
            )?,
            eip1654.clone(),
        )
        .with_presence(presence)
        .with_allowance(worlds::core::holdings::configured(
            http_client.clone(),
            config.content.eth_rpc_url()?.as_deref(),
            non_empty(&config.economy.rpc_endpoint_polygon).as_deref(),
            &config.node.eth_network,
        ))
        // World notices persist through social's directory, as comms' do.
        .with_notices(worlds::core::notices::Notices::new(
            Some(Arc::new(social::comms_directory::Directory::new(
                pool.clone(),
            ))),
            &config.worlds.builder_url,
        ))
        .with_forwarder(forwarder(
            "WORLDS_FORWARD_UPSTREAM_URL",
            config.worlds.worlds_forward_upstream_url.as_deref(),
            worlds::core::forwarding::LANE,
            config,
            pool,
            storage,
            &content::own_hosts(config),
            jobs,
        )?),
    );
    core.spawn_jobs(jobs);

    let world_storage = Arc::new(worlds::storage::StorageState::new(
        pool.clone(),
        policy.clone(),
        worlds::storage::Settings::resolve(worlds::storage::RawSettings {
            encryption_key: non_empty(&config.worlds.encryption_key),
            authoritative_server_address: non_empty(&config.social.authoritative_server_address),
            authorized_addresses: config.worlds.authorized_addresses.clone(),
            storage_cache_enabled: config.worlds.storage_cache_enabled,
            storage_cache_ttl_seconds: config.worlds.storage_cache_ttl_seconds,
            storage_cache_max: config.worlds.storage_cache_max,
            storage_cache_max_value_bytes: config.worlds.storage_cache_max_value_bytes,
            env_max_value_size_bytes: config.worlds.env_storage_max_value_size_bytes,
            env_max_total_size_bytes: config.worlds.env_storage_max_total_size_bytes,
            world_max_value_size_bytes: config.worlds.world_storage_max_value_size_bytes,
            world_max_total_size_bytes: config.worlds.world_storage_max_total_size_bytes,
            player_max_value_size_bytes: config.worlds.player_storage_max_value_size_bytes,
            player_max_total_size_bytes: config.worlds.player_storage_max_total_size_bytes,
            export_max_concurrent_per_scene: config.worlds.export_max_concurrent_per_scene,
            export_max_concurrent_per_pod: config.worlds.export_max_concurrent_per_pod,
            export_scene_cooldown_seconds: config.worlds.export_scene_cooldown_seconds,
            export_timeout_seconds: config.worlds.export_timeout_seconds,
            export_max_size_bytes: config.worlds.export_max_size_bytes,
        })
        .context("ENCRYPTION_KEY / AUTHORIZED_ADDRESSES / *_STORAGE_MAX_* configuration")?,
        eip1654.clone(),
    ));

    let scene_host = if config.worlds.scene_host_enabled {
        let settings = worlds::scene_state::Settings::resolve(
            worlds::scene_state::RawSettings {
                local_scene_path: non_empty(&config.worlds.scene_host_local_scene_path),
                debugging_secret: non_empty(&config.worlds.scene_host_debugging_secret),
                admin_token: non_empty(&config.worlds.scene_host_admin_token),
                auth_timeout_secs: config.worlds.scene_host_auth_timeout_secs,
                disable_js_runtime: config.worlds.scene_host_disable_js_runtime,
                realm_name: config.node.realm_name.clone(),
                commit_hash: config.node.commit_hash.clone(),
                js_heap_limit_mb: config.worlds.js_heap_limit_mb,
                js_tick_budget_ms: config.worlds.js_tick_budget_ms,
                js_shutdown_join_ms: config.worlds.js_shutdown_join_ms,
                js_update_failure_cap: config.worlds.js_update_failure_cap,
                client_outbound_max: config.worlds.client_outbound_max,
                client_inbound_max: config.worlds.client_inbound_max,
                crdt_max_components: config.worlds.crdt_max_components,
                ws_max_frame_bytes: config.worlds.ws_max_frame_bytes,
                storage_url: non_empty(&config.worlds.scene_host_storage_url),
                storage_allow_http: config.worlds.scene_host_storage_allow_http,
                authoritative_key: non_empty(&config.worlds.scene_host_authoritative_key),
                delegation_ttl_secs: config.worlds.scene_host_delegation_ttl_secs,
                delegation_minter_url: non_empty(&config.worlds.scene_host_delegation_minter_url),
                delegation_minter_token: non_empty(
                    &config.worlds.scene_host_delegation_minter_token,
                ),
                storage_delegation: non_empty(&config.worlds.scene_host_storage_delegation),
                signed_fetch_max_response_bytes: config.worlds.signed_fetch_max_response_bytes,
                signed_fetch_max_body_bytes: config.worlds.signed_fetch_max_body_bytes,
                signed_fetch_max_in_flight: config.worlds.signed_fetch_max_in_flight,
                signed_fetch_timeout_ms: config.worlds.signed_fetch_timeout_ms,
                max_scenes: config.worlds.scene_host_max_scenes,
                autoload: config.worlds.scene_host_autoload,
                sweep_interval_secs: config.worlds.scene_host_sweep_interval_secs,
            },
            http_client,
        )
        .context("SCENE_HOST_* configuration")?;
        let host = Arc::new(
            worlds::scene_state::SceneHostState::new(
                pool.clone(),
                storage.clone(),
                policy.clone(),
                settings,
                eip1654.clone(),
            )
            .context("building the scene host")?
            .with_secret_max_attempts(config.worlds.shared_secret_max_attempts_per_minute),
        );
        host.spawn_jobs(jobs);
        Some(host)
    } else {
        tracing::info!("SCENE_HOST_ENABLED=false: no authoritative scene host");
        None
    };

    let fed_cfg = worlds::fed::WorldsFedConfig::new(
        config.worlds_fed.worlds_fed_peers_file.clone(),
        config.worlds_fed.worlds_fed_poll_interval_secs,
        config.worlds_fed.worlds_fed_max_response_bytes,
        config.worlds_fed.worlds_fed_max_worlds_per_peer,
        config.worlds_fed.worlds_fed_allow_insecure_loopback_peers,
    )
    .context("WORLDS_FED_* configuration")?
    .with_own_urls(local_urls.to_vec());
    let fed = worlds::fed::FedState::boot(
        pool.clone(),
        fed_cfg,
        non_empty(&config.worlds.catalyrst_worlds_admin_token),
    )
    .await
    .context("worlds federation")?;
    fed.spawn_jobs(jobs);

    tracing::info!(
        scene_host = scene_host.is_some(),
        worlds_admin = config.worlds.catalyrst_worlds_admin_token.is_some(),
        encryption_key = config.worlds.encryption_key.is_some(),
        nats = config.worlds.nats_url.is_some(),
        federation = config.worlds_fed.worlds_fed_peers_file.is_some(),
        "worlds routes"
    );
    Ok(worlds::Stage5 {
        core,
        storage: world_storage,
        scene_host,
        fed,
        writes_enabled: config.worlds.worlds_writes_enabled,
    })
}

/// The comms states: the gatekeeper over the ONE pool and client,
/// the worlds permission core and room resolver
/// (`comms::interfaces::{AccessPolicy, SceneIdentity}`), the live presence its
/// webhook feeds (shared with worlds) and the worlds core as its room-event
/// sink (the access log), owning `serve`'s one NATS link (`NATS_URL`); and
/// archipelago's ws-connector, built from it, over the same pool and client,
/// the worlds permission core as its ban authority
/// (`comms::interfaces::AccessPolicy`) and the gatekeeper's link and LiveKit
/// (the connector holds no LiveKit credential); on the `primary`, the one
/// process that holds the connector's sockets, the gatekeeper's island
/// subscriber then delivers to them by call.
/// `serve` mounts archipelago's routes
/// right after `worlds::app_router`, where that router used to add them, and
/// the gatekeeper's right before the social router, where it held them.
struct CommsStage {
    archipelago: crate::comms::archipelago::AppState,
    gatekeeper: crate::comms::gatekeeper::SharedState,
}

#[allow(clippy::too_many_arguments)]
async fn comms_states(
    config: &Config,
    pool: &sqlx::PgPool,
    http_client: &http::Outbound,
    content: &Arc<content::ContentState>,
    market: &Arc<shop::market::MarketplaceState>,
    comms_settings: comms::gatekeeper::Settings,
    policy: &Arc<worlds::policy::Policy>,
    ban_status: Arc<dyn comms::gatekeeper::moderation::BanStatus>,
    room_events: Arc<dyn comms::gatekeeper::events::RoomEventSink>,
    presence: Arc<comms::presence::Presence>,
    eip1654: Option<Arc<dyn crate::auth::eip1654::Eip1654Validator>>,
    primary: bool,
    jobs: &Arc<jobs::Jobs>,
) -> anyhow::Result<CommsStage> {
    use crate::comms::archipelago;
    use crate::comms::interfaces::AccessPolicy;

    let non_empty = Config::non_empty;
    let archipelago_settings = archipelago::Settings::resolve(
        archipelago::config::RawSettings {
            max_sockets: config.worlds.ws_max_sockets,
            handshake_timeout_ms: config.worlds.handshake_timeout,
            deny_list_url: config.worlds.deny_list(),
            island_changed_dedup_ms: config.worlds.island_changed_dedup_ms,
            commit_hash: config.node.commit_hash.clone(),
        },
        &content.settings.content_public_url,
    );

    let social_directory = Arc::new(social::comms_directory::Directory::new(pool.clone()));
    let gatekeeper = comms::gatekeeper::CommsState::new(
        pool.clone(),
        http_client.clone(),
        comms_settings,
        Some(market.index.clone()),
        eip1654,
        policy.clone() as Arc<dyn AccessPolicy>,
        Arc::new(worlds::rooms::SceneResolver::new(pool.clone())),
        presence,
        social_directory,
    )
    .with_ban_status(ban_status.clone())
    .with_name_owner_reader(market.clone())
    .with_deny_list_url(config.worlds.deny_list())
    .with_cluster(comms::gatekeeper::admission::cluster::ClusterConfig {
        nats_url: non_empty(&config.worlds.nats_url),
        takeover_retry_delay_ms: config.worlds.cluster_takeover_retry_delay_ms,
        island_token_ttl_seconds: config.worlds.cluster_island_token_ttl_seconds,
        self_hosted_token_quarantine: config.worlds.cluster_self_hosted_token_quarantine,
        drain_timeout_ms: config.worlds.cluster_drain_timeout_ms,
        peer_state_max: config.worlds.cluster_peer_state_max,
        peer_state_ttl_ms: config.worlds.cluster_peer_state_ttl_ms,
        control_v4_audience: non_empty(&config.worlds.archipelago_control_v4_audience),
    })
    .context("ARCHIPELAGO_CONTROL_V4_AUDIENCE and NATS_URL configuration")?
    .with_max_users_per_world(config.worlds.max_users_per_world)
    .with_shared_secret_max_attempts(config.worlds.shared_secret_max_attempts_per_minute)
    .with_room_authority_key(non_empty(&config.room_authority.pulse_room_authority_key).as_deref())?
    .with_room_event_sink(room_events);
    let gatekeeper = Arc::new(gatekeeper);
    gatekeeper.spawn_presence_recovery(jobs);

    let archipelago = archipelago::build_state(archipelago_settings, gatekeeper.clone(), jobs)
        .await
        .context("wiring archipelago")?;
    if primary {
        gatekeeper.start_island_feed(jobs, archipelago.clone());
    }
    archipelago::spawn_jobs(&archipelago, jobs);
    Ok(CommsStage {
        archipelago,
        gatekeeper,
    })
}

/// Social's community exits, enforced by the worlds core on a spawned task
/// (the social write has committed; the re-decision never delays it).
struct WorldsCommunityAccess(worlds::core::SharedState);

impl communities::WorldAccessRevocation for WorldsCommunityAccess {
    fn member_removed(
        &self,
        community_id: crate::shared::identity::CommunityId,
        member: &crate::shared::identity::Address,
    ) {
        let (core, member) = (self.0.clone(), member.clone());
        tokio::spawn(async move {
            core.community_member_removed(community_id.as_uuid(), &member)
                .await
        });
    }

    fn community_closed(&self, community_id: crate::shared::identity::CommunityId) {
        let core = self.0.clone();
        tokio::spawn(async move { core.community_closed(community_id.as_uuid()).await });
    }
}

/// Social's boot-time provider setup: the bootstrap check, then the analytics
/// adapter, which is configured by its own settings only.
fn social_analytics(
    config: &Config,
    http_client: &http::Outbound,
) -> anyhow::Result<Option<social::analytics::Analytics>> {
    config
        .social
        .validate_provider_bootstrap()
        .map_err(anyhow::Error::msg)?;
    if config.social.community_compliance_api_key.is_none() {
        tracing::info!(
            "OPEN_AI_API_KEY unset: community AI compliance runs without its provider and skips classification"
        );
    }
    social::analytics::Analytics::new(
        http_client,
        social::analytics::Settings {
            context: config.social.social_analytics_context.clone(),
            api_url: config.social.social_analytics_api_url.clone(),
            api_token: config.social.social_analytics_api_token.clone(),
            environment: config.social.social_environment.clone(),
            request_timeout_ms: config.social.social_analytics_request_timeout_ms,
        },
    )
    .map_err(anyhow::Error::msg)
}

/// The referral settings `social_states` passes on, including the backend
/// proxies trusted to supply `X-Real-IP` for referral quotas.
fn referral_settings(config: &Config) -> communities::referral::Settings {
    communities::referral::Settings {
        trusted_proxy_cidrs: config.node.http_trusted_proxy_cidrs.clone(),
        max_ip_matches: config.social.referral_max_ip_matches,
        suspicious_window_ms: config.social.referral_suspicious_window_ms,
        slack: communities::referral::SlackSettings {
            token: config.social.referral_slack_token.clone(),
            dashboard: config.social.referral_metabase_dashboard.clone(),
            environment: config.social.social_environment.clone().unwrap_or_default(),
        },
        min_login_days: config.social.referral_min_login_days,
        profile_url: config.social.referral_profile_url.clone(),
        reward_server_url: config.social.referral_reward_server_url.clone(),
        reward_keys: [
            (
                5,
                &config.social.rewards_api_key_by_referral_invited_users_5,
            ),
            (
                10,
                &config.social.rewards_api_key_by_referral_invited_users_10,
            ),
            (
                20,
                &config.social.rewards_api_key_by_referral_invited_users_20,
            ),
            (
                25,
                &config.social.rewards_api_key_by_referral_invited_users_25,
            ),
            (
                30,
                &config.social.rewards_api_key_by_referral_invited_users_30,
            ),
            (
                50,
                &config.social.rewards_api_key_by_referral_invited_users_50,
            ),
            (
                60,
                &config.social.rewards_api_key_by_referral_invited_users_60,
            ),
            (
                75,
                &config.social.rewards_api_key_by_referral_invited_users_75,
            ),
        ]
        .into_iter()
        .map(|(tier, key)| (tier, key.clone().unwrap_or_default()))
        .collect(),
    }
}

#[cfg(test)]
mod referral_settings_tests {
    use super::{referral_settings, social_analytics};
    use crate::config::Config;

    /// The boot path's provider setup with `ENV` set and nothing else: the
    /// node starts, without analytics.
    #[test]
    fn env_alone_boots_without_analytics() {
        let http = crate::http::Outbound::for_tests();
        for env in ["prd", "production", ""] {
            let config = Config::parse_env_for_tests([
                ("DATABASE_URL", "postgres:///catalyst"),
                ("ENV", env),
            ])
            .unwrap();
            assert!(social_analytics(&config, &http).unwrap().is_none(), "{env}");
        }
        let config = Config::parse_env_for_tests([
            ("DATABASE_URL", "postgres:///catalyst"),
            ("ENV", "prd"),
            ("ANALYTICS_CONTEXT", "fixture"),
            ("ANALYTICS_API_URL", "https://analytics.example.org"),
            ("ANALYTICS_API_TOKEN", "fixture"),
        ])
        .unwrap();
        assert!(social_analytics(&config, &http).unwrap().is_some());
    }

    /// `HTTP_TRUSTED_PROXY_CIDRS` through `referral_settings` into the real
    /// referral route behind a real socket: a configured proxy's `X-Real-IP`
    /// is the stored client IP, any other peer is stored as itself.
    #[tokio::test]
    async fn configured_proxies_reach_the_referral_client_ip_path() {
        use crate::social::communities;
        let Some(pool) = crate::db::scratch::pool(4).await else {
            return;
        };
        for (cidrs, expected) in [
            ("127.0.0.1/32", "198.51.100.7"),
            ("192.0.2.0/24", "127.0.0.1"),
        ] {
            let config = Config::parse_env_for_tests([
                ("DATABASE_URL", "postgres:///catalyst"),
                ("HTTP_TRUSTED_PROXY_CIDRS", cidrs),
            ])
            .unwrap();
            let dir =
                std::env::temp_dir().join(format!("catalyst-referral-ip-{}", uuid::Uuid::new_v4()));
            let storage =
                std::sync::Arc::new(crate::storage::Storage::from_config(&dir).await.unwrap());
            let state = std::sync::Arc::new(communities::State::new(
                pool.clone(),
                storage,
                communities::Settings {
                    referral: communities::referral::Settings {
                        suspicious_window_ms: 0.0,
                        ..referral_settings(&config)
                    },
                    ..communities::Settings::for_tests()
                },
                None,
                &crate::http::Outbound::for_tests(),
                communities::State::explore_for_tests(&pool),
            ));
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            let endpoint = format!(
                "http://{}/v1/referral-progress",
                listener.local_addr().unwrap()
            );
            let app = communities::router(state);
            let server = tokio::spawn(async move {
                axum::serve(
                    listener,
                    app.into_make_service_with_connect_info::<std::net::SocketAddr>(),
                )
                .await
                .unwrap()
            });
            let (referrer, invited) = (
                alloy::signers::local::PrivateKeySigner::random(),
                alloy::signers::local::PrivateKeySigner::random(),
            );
            let mut request = crate::http::Outbound::for_tests()
                .post(&endpoint)
                .header("x-real-ip", "198.51.100.7")
                .json(&serde_json::json!({ "referrer": format!("{:#x}", referrer.address()) }));
            let headers = crate::shared::signed_fetch::test_support::signed_headers_by(
                &invited,
                "post",
                "/v1/referral-progress",
                "{}",
                true,
                &chrono::Utc::now().timestamp_millis().to_string(),
            )
            .await;
            for (name, value) in &headers {
                request = request.header(name.as_str(), value.to_str().unwrap());
            }
            let response = request.send().await.unwrap();
            assert_eq!(
                response.status(),
                axum::http::StatusCode::NO_CONTENT,
                "{cidrs}"
            );
            let stored: String = sqlx::query_scalar(
                "SELECT invited_user_ip FROM social.referral_progress WHERE invited_user = $1",
            )
            .bind(format!("{:#x}", invited.address()))
            .fetch_one(&pool)
            .await
            .unwrap();
            assert_eq!(stored, expected, "{cidrs}");
            server.abort();
            let _ = std::fs::remove_dir_all(&dir);
        }
    }

    #[test]
    fn referral_quotas_trust_exactly_the_configured_proxy_networks() {
        let parse = |vars: &[(&str, &str)]| {
            Config::parse_env_for_tests(
                [("DATABASE_URL", "postgres:///catalyst")]
                    .iter()
                    .chain(vars)
                    .copied(),
            )
            .unwrap()
        };
        assert!(
            referral_settings(&parse(&[]))
                .trusted_proxy_cidrs
                .is_empty(),
            "no proxy is trusted unless configured"
        );
        let config = parse(&[("HTTP_TRUSTED_PROXY_CIDRS", "10.0.0.0/8,2001:db8::/32")]);
        let trusted = referral_settings(&config).trusted_proxy_cidrs;
        assert_eq!(trusted, config.node.http_trusted_proxy_cidrs);
        assert_eq!(
            trusted.iter().map(ToString::to_string).collect::<Vec<_>>(),
            ["10.0.0.0/8", "2001:db8::/32"]
        );
    }
}

/// The social bundle's states (the old catalyrst-social members: communities,
/// notifications, badges, media, plus profile-images, the social RPC and the
/// MLS delivery service), every one over the same pool, client, blob store
/// and validator; `social::router` owns their routes, `serve` mounts the MLS
/// routes under `/comms-gatekeeper`. The gatekeeper's state (built in
/// `comms_states`) is also the social socket's voice runtime
/// (`comms::gatekeeper::voice::rooms`), the local call that replaced the old
/// `COMMS_GATEKEEPER_URL` hop.
#[allow(clippy::too_many_arguments)]
fn social_states(
    config: &Config,
    pool: &sqlx::PgPool,
    storage: &Arc<storage::Storage>,
    http_client: &http::Outbound,
    content: &Arc<content::ContentState>,
    market: &Arc<shop::market::MarketplaceState>,
    comms: comms::gatekeeper::SharedState,
    eip1654: Option<Arc<dyn crate::auth::eip1654::Eip1654Validator>>,
    explore: &explore::SharedState,
    onboarding: Arc<explore::api::onboarding::OnboardingState>,
    metrics_handle: comms::metrics::PrometheusHandle,
) -> anyhow::Result<social::States> {
    config
        .social
        .validate_provider_bootstrap()
        .map_err(anyhow::Error::msg)?;
    let non_empty = Config::non_empty;
    let public_url = config
        .node
        .public_url
        .clone()
        .unwrap_or_else(|| format!("http://{}", config.node.bind_addr()));
    let content_public_url = content.settings.content_public_url.clone();
    let profile_images_settings = profile_images::Settings::parse(profile_images::RawSettings {
        cache_dir: config.social.profile_images_cache_dir.clone(),
        cache_ttl_seconds: config.social.profile_images_cache_ttl_seconds,
        cache_max_bytes: config.social.profile_images_cache_max_bytes,
        origin_url: config.social.profile_images_origin_url.clone(),
        render_enabled: config.social.profile_images_render_enabled,
        renderer: config.social.profile_images_renderer.clone(),
        avatar_render_bin: config.social.profile_images_avatar_render_bin.clone(),
        godot_bin: config.social.profile_images_godot_bin.clone(),
        godot_project: config.social.profile_images_godot_project.clone(),
        rendering_method: config.social.profile_images_rendering_method.clone(),
        rendering_driver: config.social.profile_images_rendering_driver.clone(),
        dclenv: config.social.profile_images_dclenv.clone(),
        godot_headless: config.social.profile_images_godot_headless,
        godot_display: config.social.profile_images_godot_display.clone(),
        godot_extra_args: config.social.profile_images_godot_extra_args.clone(),
        render_timeout_seconds: config.social.profile_images_render_timeout_seconds,
        render_max_concurrent: config.social.profile_images_render_max_concurrent,
        render_workdir: config.social.profile_images_render_workdir.clone(),
        content_public_url: content_public_url.clone(),
    })
    .context("PROFILE_IMAGES_* configuration")?;
    let faces = communities::profiles::FaceUrls::new(
        market.settings.profile_cdn_base_url.as_deref(),
        &market.settings.profile_images_public_url,
        profile_images_settings.derives_faces(),
    );

    let communities = Arc::new(communities::State::new(
        pool.clone(),
        storage.clone(),
        communities::Settings {
            http_max_request_body_bytes: config.social.social_http_max_request_body_bytes,
            compliance: communities::compliance::Settings {
                environment: config.social.social_environment.clone().unwrap_or_default(),
                model: config.social.community_compliance_model.clone(),
                api_key: config.social.community_compliance_api_key.clone(),
                base_url: config.social.community_compliance_base_url.clone(),
                organization: config.social.community_compliance_organization.clone(),
                project: config.social.community_compliance_project.clone(),
            },
            admin_token: non_empty(&config.social.communities_admin_token),
            global_moderators: communities::Settings::global_moderators_from(
                &config.social.communities_global_moderators,
            ),
            features: communities::features::Settings {
                base_url: config.social.social_feature_flags_url.clone(),
                audience: config.social.social_service_base_url.clone(),
                interval: Duration::from_millis(config.social.social_feature_flags_refresh_ms),
                timeout: Duration::from_millis(config.social.social_feature_flags_timeout_ms),
                overrides: [
                    (
                        communities::features::AI_COMPLIANCE,
                        &config.social.social_ai_compliance_override,
                    ),
                    (
                        communities::features::DEV_AI_COMPLIANCE,
                        &config.social.social_dev_ai_compliance_override,
                    ),
                    (
                        communities::features::GLOBAL_MODERATORS,
                        &config.social.social_global_moderators_override,
                    ),
                    (
                        communities::features::REFERRAL_REGISTRATION_DISABLED,
                        &config.social.social_referral_registration_disabled_override,
                    ),
                ]
                .into_iter()
                .filter_map(|(key, value)| value.clone().map(|value| (key.into(), value)))
                .collect(),
            },
            restricted_names: communities::Settings::restricted_names_from(
                &config.social.restricted_names,
            ),
            cdn_url: config
                .social
                .communities_cdn_url
                .clone()
                .unwrap_or(public_url.clone())
                .trim_end_matches('/')
                .to_string(),
            faces: faces.clone(),
            archipelago_stats_url: non_empty(&config.social.archipelago_stats_url),
            places_ownership_check: config.social.communities_places_ownership_check,
            mirror_interval: Duration::from_secs(
                config.social.communities_mirror_interval_secs.max(1),
            ),
            referral: referral_settings(config),
        },
        eip1654.clone(),
        http_client,
        explore.clone(),
    ));

    let analytics = social_analytics(config, http_client)?;
    if let Some(analytics) = &analytics {
        communities.set_analytics(analytics.clone());
    }

    let notifications = Arc::new(notifications::State::new(
        pool.clone(),
        notifications::Settings {
            admin_token: non_empty(&config.social.catalyrst_notifications_admin_token),
            events_base_url: public_url.clone(),
            events_asset_base_url: explore.settings.events_asset_base_url.clone(),
            signing_key: non_empty(&config.social.signing_key),
            service_base_url: config.social.notifications_service_base_url.clone(),
            community_cdn_url: communities.settings.cdn_url.clone(),
            jump_in_site_url: explore.settings.jump_in_site_url.clone(),
            event_slack_notices: explore.settings.events_upcoming_summaries_enabled
                && explore.settings.operator_writes_enabled
                && explore
                    .settings
                    .events_slack_webhook
                    .as_deref()
                    .is_some_and(|endpoint| !endpoint.trim().is_empty()),
            email: notifications::EmailConfig {
                sendgrid_api_key: non_empty(&config.social.sendgrid_api_key),
                from_email: non_empty(&config.social.sendgrid_from_email),
                notification_template_id: non_empty(
                    &config.social.sendgrid_notification_template_id,
                ),
                validate_email_template_id: non_empty(
                    &config.social.sendgrid_validate_email_template_id,
                ),
                validate_credits_email_template_id: non_empty(
                    &config.social.sendgrid_validate_credits_email_template_id,
                ),
                account_base_url: config.social.account_base_url.clone(),
                turnstile_secret_key: non_empty(&config.social.turnstile_secret_key),
                domain_blacklist: notifications::EmailConfig::parse_domain_blacklist(
                    &config.social.email_domain_blacklist,
                ),
                sandbox_mode: config.social.sendgrid_sandbox_mode,
                environment: non_empty(&config.social.social_environment).unwrap_or_default(),
            },
            notification_service_token: non_empty(&config.social.notification_service_token),
            governance_token: non_empty(&config.social.notifications_governance_token),
            rewards_token: non_empty(&config.social.notifications_rewards_token),
            first_wear_enabled: config.social.first_wear_enabled,
            first_wear: notifications::FirstWearSettings {
                shop_item_base_url: config.social.shop_item_base_url.clone(),
                image_base: non_empty(&config.social.first_wear_image_base),
            },
            delivery_interval: Duration::from_secs(
                config.social.notification_delivery_interval_secs,
            ),
            broadcast_horizon: Duration::from_secs(
                config
                    .social
                    .notifications_broadcast_horizon_days
                    .max(1)
                    .saturating_mul(86_400),
            ),
            delivery_max_age: notifications::delivery::max_age_from_secs(
                config.social.notification_delivery_max_age_secs,
            ),
            current_version: config.social.social_rpc_current_version.clone(),
            commit_hash: config.node.commit_hash.clone(),
        },
        http_client.clone(),
        eip1654.clone(),
        Some(market.index.clone()),
    ));

    if let Some(analytics) = analytics {
        notifications.set_analytics(analytics);
    }
    communities.referral.set_mailer(notifications.email.clone());

    let badges = Arc::new(badges::State::new(
        pool.clone(),
        badges::Settings {
            admin_token: non_empty(&config.social.catalyrst_badges_admin_token),
            public_asset_base_url: config.social.badges_public_asset_base_url.clone(),
            assets_dir: non_empty(&config.social.badges_assets_dir).map(std::path::PathBuf::from),
        },
    ));

    let segment_webhooks = Arc::new(social::segment_webhooks::State::new(
        pool.clone(),
        social::segment_webhooks::Settings {
            signing_key: non_empty(&config.social.segment_signing_key),
            force_context_validation: config.social.force_explorer_context_validation,
            expected_realm: non_empty(&config.social.segment_expected_realm),
            expected_realm_url: non_empty(&config.social.segment_expected_realm_url),
            web_project_id: config.social.segment_web_project_id.clone(),
            retention: Duration::from_secs(
                u64::from(config.social.client_events_retention_days.max(1)) * 24 * 3600,
            ),
        },
        onboarding,
    ));

    let media = Arc::new(
        media::MediaState::new(
            pool.clone(),
            http_client.clone(),
            media::Settings::parse(media::RawSettings {
                backend: config.social.translate_backend.clone(),
                backend_url: config.social.translate_backend_url.clone(),
                backend_api_key: Config::non_empty(&config.social.translate_backend_api_key)
                    .unwrap_or_default(),
                llm_base_url: config.social.translate_llm_base_url.clone(),
                llm_api_key: Config::non_empty(&config.social.translate_llm_api_key)
                    .unwrap_or_default(),
                llm_model: config.social.translate_llm_model.clone(),
                char_limit: config.social.translate_char_limit,
                batch_limit: config.social.translate_batch_limit,
                request_timeout_secs: config.social.translate_request_timeout_secs,
                cache_ttl_days: config.social.translate_cache_ttl_days,
            })
            .context("TRANSLATE_* configuration")?,
        )
        .with_convert_sources(media::ConvertSources {
            remote: config.social.media_convert_remote_sources,
            own_url: Some(
                url::Url::parse(&config.node.own_url()).context("PUBLIC_URL is not a URL")?,
            ),
            storage: Some(storage.clone()),
            read_through: content.read_through().cloned(),
        }),
    );

    let profile_images = Arc::new(
        profile_images::ProfileImagesState::new(
            pool.clone(),
            storage.clone(),
            http_client.clone(),
            profile_images_settings,
        )
        .with_read_through(content.read_through().cloned()),
    );

    let messaging = Arc::new(messaging::MessagingState::new(
        pool.clone(),
        storage.clone(),
        eip1654.clone(),
        messaging::Settings {
            fed_peer_id: non_empty(&config.social.fed_peer_id),
            gatekeeper_auth_token: non_empty(&config.social.comms_gatekeeper_auth_token),
        },
    ));

    let ordinary_transport = shared::rpc::ws::OrdinaryQueueConfig {
        max_queue_size: config.social.ws_transport_max_queue_size,
        min_queue_size: config.social.ws_transport_min_queue_size,
        max_queue_size_limit: config.social.ws_transport_max_queue_size_limit,
        estimated_message_size: config.social.ws_transport_estimated_message_size,
        retry_delay_ms: config.social.ws_transport_retry_delay_ms,
        max_retry_attempts: config.social.ws_transport_max_retry_attempts,
        max_backoff_delay_ms: config.social.ws_transport_max_backoff_delay_ms,
    };
    ordinary_transport
        .capacity(config.social.ws_max_backpressure)
        .map_err(|error| anyhow::anyhow!(error))?;
    let rpc = Arc::new(social_rpc::State::new(
        pool.clone(),
        social_rpc::Settings {
            auth_window_secs: config.social.social_rpc_auth_window_secs,
            ws_auth_timeout_ms: config.social.ws_auth_timeout_in_ms,
            ws_auth_signature_expiration_ms: config.social.ws_auth_signature_expiration_in_ms,
            ws_idle_timeout_secs: config.social.ws_idle_timeout_in_seconds,
            private_voice_chat_expiration_ms: config.social.private_voice_chat_expiration_ms,
            private_voice_chat_job_interval_ms: config.social.private_voice_chat_job_interval_ms,
            private_voice_chat_expiration_batch_size: config
                .social
                .private_voice_chat_expiration_batch_size,
            community_voice_chat_polling_job_interval_ms: config
                .social
                .community_voice_chat_polling_job_interval_ms,
            ws_max_concurrent_connections: config.social.ws_max_concurrent_connections,
            ws_max_payload_bytes: config.social.ws_max_payload_length,
            ws_max_backpressure_bytes: config.social.ws_max_backpressure,
            ordinary_transport,
            friendship_limits: social_rpc::limits::FriendshipLimits {
                window_seconds: config.social.friendship_rate_limit_window_seconds,
                per_actor: config.social.friendship_rate_limit_per_actor,
                per_pair: config.social.friendship_rate_limit_per_pair,
            },
            admission: social_rpc::limits::AdmissionLimits {
                max_per_address: config.social.ws_max_connections_per_address as usize,
                subscriptions_per_second: config.social.ws_subscriptions_per_second,
                subscription_burst: config.social.ws_subscription_burst as usize,
                idle_budget_capacity: config.social.ws_address_budget_capacity as usize,
            },
            stream: social_rpc::stream::StreamLimits {
                max_payload_bytes: config.social.stream_ws_max_payload_length as usize,
                messages_per_second: config.social.stream_ws_messages_per_second,
                message_burst: config.social.stream_ws_message_burst as usize,
            },
            admin_token: non_empty(&config.social.catalyrst_social_rpc_admin_token),
            writes_enabled: config.writes.social_writes_enabled,
            commit_hash: config.node.commit_hash.clone(),
            current_version: config.social.social_rpc_current_version.clone(),
            metrics_path: config.social.wkc_metrics_public_path.clone(),
            metrics_bearer_token: config.social.wkc_metrics_bearer_token.clone(),
            metrics_reset_at_night: config.social.wkc_metrics_reset_at_night.as_deref()
                == Some("true"),
            metrics_handle: Some(metrics_handle),
            profile_source: Some(communities.profiles.registry()),
        },
        eip1654,
        comms.clone(),
    ));
    // Membership changes and deletions reach the open RPC streams; a voice
    // start reads the community's places and thumbnail.
    communities.set_updates(rpc.ctx.pubsub().clone());
    rpc.ctx.set_communities(communities.clone());

    tracing::info!(
        translate = media.backend_label(),
        profile_images = %profile_images.sources_label(),
        livekit = comms.livekit.configured,
        segment_webhooks = segment_webhooks.settings.signing_key.is_some(),
        "social routes"
    );
    Ok(social::States {
        communities,
        comms,
        messaging,
        notifications,
        badges,
        media,
        profile_images,
        rpc,
        segment_webhooks,
    })
}

/// Installs the shutdown signal handlers now and returns the wait for one of
/// them. Call it before logging that the process is ready, so a signal sent
/// in reply to that line finds its handler.
pub(crate) fn shutdown_signal() -> impl std::future::Future<Output = ()> {
    #[cfg(unix)]
    {
        use tokio::signal::unix::{SignalKind, signal};
        let mut interrupt = signal(SignalKind::interrupt()).expect("installing the SIGINT handler");
        let mut term = signal(SignalKind::terminate()).expect("installing the SIGTERM handler");
        async move {
            tokio::select! {
                _ = interrupt.recv() => tracing::info!("SIGINT received"),
                _ = term.recv() => tracing::info!("SIGTERM received"),
            }
        }
    }
    #[cfg(not(unix))]
    async {
        let _ = tokio::signal::ctrl_c().await;
        tracing::info!("shutdown signal received");
    }
}

//! A windowless explorer: the renderer and the scene, avatar and comms stacks without a
//! window, input or HUD. The batch tools (`src/bin/avatar_render.rs`) build on it; the
//! render-free scene server has its own assembly in `headless`.

use std::time::Duration;

use assets::EmbedAssetsPlugin;
use bevy::{
    app::ScheduleRunnerPlugin, log::LogPlugin, prelude::*, window::ExitCondition,
    winit::WinitPlugin,
};
use common::{
    inputs::InputMap,
    rpc::RpcCallEvent,
    sets::SetupSets,
    structs::{
        AppConfig, CursorLocks, EngineMovementControl, PermissionUsed, PreviewMode,
        PrimaryCameraRes, PrimaryPlayerRes, SceneGlobalLight, SceneLoadDistance, SystemAudio,
        TimeOfDay, ToolTips,
    },
    util::UtilsPlugin,
};
use comms::{
    global_crdt::DiscardPlayerUpdates, profile::ProfileCache, CommsPlugin, DisableRealmComms,
};
use console::ConsolePlugin;
use imposters::DclImposterPlugin;
use input_manager::{CumulativeAxisData, InputPriorities};
use ipfs::{map_realm_name, IpfsIoPlugin};
use nft::asset_source::{Nft, NftReaderPlugin};
use restricted_actions::RestrictedActionsPlugin;
use scene_material::SceneBoundPlugin;
use scene_runner::{permissions::PermissionManager, SceneRunnerPlugin};
use system_bridge::SystemBridgePlugin;
use ui_core::{scrollable::ScrollTargetEvent, UiCorePlugin};
use user_input::avatar_movement::AvatarMovementInfo;
use wallet::{delegation::StorageDelegations, Wallet};

/// The settings file of the explorer's config directory, defaults where there is none: a
/// batch tool runs under its own `XDG_CONFIG_HOME`, so this is normally the defaults.
pub fn load_base_app_config() -> AppConfig {
    let config_file = platform::project_directories()
        .expect("a config directory")
        .config_dir()
        .join("config.json");
    match std::fs::read(&config_file) {
        Ok(bytes) => AppConfig::from_json(&bytes),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => AppConfig::default(),
        Err(e) => {
            eprintln!(
                "could not read {}: {e}; starting from the defaults",
                config_file.display()
            );
            AppConfig::default()
        }
    }
}

pub struct OfflineAppConfig {
    pub config: AppConfig,
    /// The realm the engine boots into (`DCL_PEER_BASE`, a catalyst root).
    pub realm: String,
    /// Where content resolves, when not the realm's advertised content server.
    pub content_server_override: Option<String>,
    pub task_pool_options: TaskPoolOptions,
    pub log_plugin: LogPlugin,
    pub load_imposter: f32,
    pub imposter: DclImposterPlugin,
    /// Seconds since midnight.
    pub time_of_day: f32,
}

impl OfflineAppConfig {
    pub fn build(self) -> App {
        let Self {
            config,
            realm,
            content_server_override,
            task_pool_options,
            log_plugin,
            load_imposter,
            imposter,
            time_of_day,
        } = self;

        let mut app = App::new();
        app.add_plugins(
            DefaultPlugins
                .set(TaskPoolPlugin { task_pool_options })
                .set(WindowPlugin {
                    primary_window: None,
                    exit_condition: ExitCondition::DontExit,
                    ..Default::default()
                })
                .set(bevy::asset::AssetPlugin {
                    // asset loads go through the ipfs module
                    unapproved_path_mode: bevy::asset::UnapprovedPathMode::Allow,
                    ..Default::default()
                })
                .set(
                    bevy::gltf::GltfPlugin::default().with_uri_resolver(std::sync::Arc::new(
                        ipfs::ipfs_path::resolve_content_uri,
                    )),
                )
                .disable::<WinitPlugin>()
                .set(log_plugin)
                .build()
                .add_before::<bevy::asset::AssetPlugin>(IpfsIoPlugin {
                    preview: false,
                    starting_realm: Some(map_realm_name(&realm)),
                    content_server_override,
                    assets_root: Default::default(),
                    num_slots: config.max_concurrent_remotes,
                })
                .add_before::<IpfsIoPlugin>(NftReaderPlugin),
        );

        app.add_plugins(EmbedAssetsPlugin);
        app.add_plugins(ScheduleRunnerPlugin::run_loop(Duration::ZERO));

        app.insert_resource(SceneLoadDistance {
            load: config.scene_load_distance,
            unload: config.scene_unload_extra_distance,
            load_imposter,
        });
        app.insert_resource(config);

        app.configure_sets(Startup, SetupSets::Init.before(SetupSets::Main));

        // never a participant in the realm's comms: no archipelago, no world room
        app.insert_resource(DisableRealmComms(true));
        app.add_plugins(UtilsPlugin)
            .add_plugins(UiCorePlugin)
            .add_plugins(ConsolePlugin {
                add_bevy_console: false,
            })
            .add_plugins(SceneBoundPlugin)
            .add_plugins(SceneRunnerPlugin)
            .add_plugins(CommsPlugin)
            .add_plugins(RestrictedActionsPlugin)
            .add_plugins(imposter)
            .add_plugins(SystemBridgePlugin { bare: true });

        app.insert_resource(PrimaryPlayerRes(Entity::PLACEHOLDER))
            .insert_resource(PrimaryCameraRes(Entity::PLACEHOLDER));

        let mut wallet = Wallet::default();
        wallet.finalize_as_guest();
        app.init_resource::<ProfileCache>()
            .insert_resource(wallet)
            .init_resource::<StorageDelegations>()
            .add_event::<SystemAudio>()
            .init_resource::<PermissionManager>()
            .init_resource::<InputMap>()
            .init_resource::<InputPriorities>()
            .init_resource::<CumulativeAxisData>()
            .init_resource::<ToolTips>()
            .init_resource::<SceneGlobalLight>()
            .add_event::<RpcCallEvent>()
            .add_event::<ScrollTargetEvent>()
            .add_event::<PermissionUsed>()
            .init_resource::<PreviewMode>()
            .init_asset::<Nft>()
            .init_resource::<CursorLocks>()
            .init_resource::<EngineMovementControl>()
            .init_resource::<AvatarMovementInfo>()
            .insert_resource(TimeOfDay { time: time_of_day })
            .insert_resource(DiscardPlayerUpdates(true));

        app
    }
}

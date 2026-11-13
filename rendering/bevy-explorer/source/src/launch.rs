//! What each launch option DOES — once, for every binary. `LaunchOptions` / `ClientOptions`
//! (crates/system_api_types) declare the options; this is where they take effect, so a binary
//! that flattens a struct (native, headless) or deserialises it (web) gets each option's
//! behaviour by calling the two phases below rather than re-implementing them:
//!
//! 1. [`latch`] before anything else — process-wide globals that url composition reads
//! 2. [`apply`] (every binary) and [`apply_client`] (the rendering clients: native and web) at
//!    app assembly — resources and plugins
//!
//! An option never writes into the app config, which is persisted: where one overrides a
//! config setting, `apply` reads the option first and the config second. Each `apply`
//! destructures its struct without `..`, so a new field fails to compile until it is given a
//! meaning here (or, for the destination and scene set, explicitly left to the binary).

use std::str::FromStr;

use bevy::{
    diagnostic::{FrameTimeDiagnosticsPlugin, LogDiagnosticsPlugin},
    prelude::*,
    render::render_asset::RenderAssetBytesPerFrame,
};
use common::structs::{AppConfig, EditorMode, IVec2Arg, PreviewMode, WorldHold};
use system_api_types::launch_options::{ClientOptions, LaunchOptions};

/// The base domain every backend host composes from — so this runs before
/// `AppConfig::default()`, which composes the default realm. Also rejects an unparseable
/// position up front, rather than silently spawning at the home parcel.
pub fn latch(launch: &LaunchOptions) -> Result<(), String> {
    if let Some(position) = &launch.position {
        IVec2Arg::from_str(position).map_err(|e| format!("--position {position}: {e}"))?;
    }
    if let Some(domain) = &launch.base_domain {
        common::base_domain::set(domain)?;
    }
    common::base_domain::set_services(launch.services.iter())
}

/// `boot_server` is the realm the binary decided to boot into (mapped through
/// `map_realm_name`), `config` the app config the options override.
pub fn apply(app: &mut App, launch: &LaunchOptions, config: &AppConfig, boot_server: &str) {
    let LaunchOptions {
        // the destination is the binary's: boot server / location, the IpfsIoPlugin's realm
        // and content server
        realm: _,
        position: _,
        content_server: _,
        // latch
        base_domain: _,
        services: _,
        preview,
        log_fps,
    } = launch;

    app.insert_resource(PreviewMode {
        server: preview.then(|| boot_server.to_owned()),
        is_preview: *preview,
        preview_parcel: None,
    });

    if log_fps.unwrap_or(config.graphics.log_fps) {
        // the HUD adds the frame timing too
        if !app.is_plugin_added::<FrameTimeDiagnosticsPlugin>() {
            app.add_plugins(FrameTimeDiagnosticsPlugin::default());
        }
        app.add_plugins(LogDiagnosticsPlugin::default());
    }
}

/// `launch` for the shared options' client-only effects.
pub fn apply_client(
    app: &mut App,
    launch: &LaunchOptions,
    client: &ClientOptions,
    config: &AppConfig,
) {
    let ClientOptions {
        // the scene set is the binary's: the ui scene and the startup scenes
        system_scene: _,
        portables: _,
        // mapped onto the launch options before they are latched (main.rs)
        deep_link: _,
        editor,
        hold_world,
        // seed the analytics common fields (lib.rs SegmentConfig)
        session_id: _,
        campaign_anon_user_id: _,
        imposter_source,
        bevy_variants,
        gpu_bytes_per_frame,
        // sizes the task pool, which exists before plugins run (lib.rs desktop_default_plugins;
        // web.rs engine_start spawns the workers)
        compute_threads: _,
        // the app assembly (lib.rs) inserts `common::autopilot::Autopilot`: only it knows the
        // CSV destination, a native flag or a web page param
        autopilot: _,
    } = client;

    app.insert_resource(EditorMode(*editor));

    // preview mode refuses realm changes, so nothing could ever release the hold
    if *hold_world && !launch.preview {
        app.insert_resource(WorldHold);
    }

    // the preview stats and sysinfo panels (system_ui) read the frame rate
    if launch.preview && !app.is_plugin_added::<FrameTimeDiagnosticsPlugin>() {
        app.add_plugins(FrameTimeDiagnosticsPlugin::default());
    }

    if let Some(source) = imposter_source {
        imposters::imposter_spec::set_source(source);
    }

    // abgen's Bevy variants: the launch option, else the config, else the asset-bundle
    // generator's CDN for this base domain; `off` in either turns them off
    let bevy_variants_base = bevy_variants
        .as_deref()
        .or(config.bevy_variants_url.as_deref())
        .map(str::to_owned)
        .unwrap_or_else(|| common::base_domain::service(common::base_domain::Service::AbgenCdn));
    if !bevy_variants_base.trim().eq_ignore_ascii_case("off") {
        if let Some(ipfs) = app.world().get_resource::<ipfs::IpfsResource>() {
            ipfs.set_bevy_variants_base(&bevy_variants_base);
        }
    }

    let gpu_bytes_per_frame = gpu_bytes_per_frame.unwrap_or(config.graphics.gpu_bytes_per_frame);
    if gpu_bytes_per_frame > 0 {
        app.insert_resource(RenderAssetBytesPerFrame::new_with_priorities(
            gpu_bytes_per_frame,
        ));
    }
}

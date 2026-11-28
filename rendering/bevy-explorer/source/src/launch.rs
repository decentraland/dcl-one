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
    diagnostic::{EntityCountDiagnosticsPlugin, FrameTimeDiagnosticsPlugin, LogDiagnosticsPlugin},
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
    if let Some(gateway) = &launch.gateway {
        common::base_domain::set_gateway(gateway)?;
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
        // the startup scenes are the binary's
        portables: _,
        // latch
        base_domain: _,
        gateway: _,
        services: _,
        preview,
        log_fps,
    } = launch;

    app.insert_resource(PreviewMode {
        server: preview.then(|| boot_server.to_owned()),
        is_preview: *preview,
        preview_parcel: None,
    });

    // the frame rate and the entity count diagnostics, in every build
    if !app.is_plugin_added::<FrameTimeDiagnosticsPlugin>() {
        app.add_plugins(FrameTimeDiagnosticsPlugin::default());
    }
    app.add_plugins(EntityCountDiagnosticsPlugin);
    if log_fps.unwrap_or(config.graphics.log_fps) {
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
        ff,
    } = client;

    // the flags the engine reads from the feature-flags service, under this option's overrides
    app.add_plugins(common::feature_flags::FeatureFlagsPlugin {
        overrides: ff.clone(),
    });

    app.insert_resource(EditorMode(*editor));

    // preview mode refuses realm changes, so nothing could ever release the hold
    if *hold_world && !launch.preview {
        app.insert_resource(WorldHold);
    }

    if let Some(source) = imposter_source {
        imposters::imposter_spec::set_source(source);
    }

    let abgen_cdn = common::base_domain::Service::AbgenCdn;
    let routing = bevy_variants_routing(
        bevy_variants.as_deref(),
        config.bevy_variants_url.as_deref(),
        common::base_domain::service_override(abgen_cdn),
        common::base_domain::service(abgen_cdn),
    );
    if let Some(ipfs) = app.world().get_resource::<ipfs::IpfsResource>() {
        let base = (!routing.base.trim().eq_ignore_ascii_case("off")).then_some(&*routing.base);
        // the flags plugin is added above, so a base left to the flag settles
        if routing.flag_may_route {
            ipfs.route_bevy_variants_by_flag(base, routing.source);
        } else {
            ipfs.set_bevy_variants_base(base, routing.source);
        }
    }

    let gpu_bytes_per_frame = gpu_bytes_per_frame.unwrap_or(config.graphics.gpu_bytes_per_frame);
    if gpu_bytes_per_frame > 0 {
        app.insert_resource(RenderAssetBytesPerFrame::new_with_priorities(
            gpu_bytes_per_frame,
        ));
    }
}

/// Where the Bevy variants start in a session.
#[derive(Debug, PartialEq)]
struct BevyVariantsRouting {
    /// A base url, or `off`.
    base: String,
    /// What set it.
    source: &'static str,
    /// Whether it waits for the endpoint flag, as the flag's fallback.
    flag_may_route: bool,
}

/// Launch options first: `bevy_variants`, then an `abgen_cdn` service override; both settle the
/// base at launch. Otherwise the endpoint flag settles it once the flags are read, falling back
/// to the config field, else to the `abgen_cdn` service composed for the base domain.
fn bevy_variants_routing(
    launch_option: Option<&str>,
    config: Option<&str>,
    abgen_cdn_override: Option<&str>,
    abgen_cdn: String,
) -> BevyVariantsRouting {
    let pinned = |base: &str, source| BevyVariantsRouting {
        base: base.to_owned(),
        source,
        flag_may_route: false,
    };
    if let Some(base) = launch_option {
        pinned(base, "launch option")
    } else if let Some(base) = abgen_cdn_override {
        pinned(base, "`abgen_cdn` service override")
    } else if let Some(base) = config {
        BevyVariantsRouting {
            base: base.to_owned(),
            source: "config",
            flag_may_route: true,
        }
    } else {
        BevyVariantsRouting {
            base: abgen_cdn,
            source: "the `abgen_cdn` service for the base domain",
            flag_may_route: true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const CDN: &str = "https://abgen-cdn.decentraland.org";

    fn routing(
        launch_option: Option<&str>,
        config: Option<&str>,
        abgen_cdn_override: Option<&str>,
    ) -> BevyVariantsRouting {
        bevy_variants_routing(launch_option, config, abgen_cdn_override, CDN.to_owned())
    }

    #[test]
    fn launch_options_pin_the_bevy_variants_base_and_the_flag_outranks_the_config() {
        let option = routing(
            Some("https://a.example"),
            Some("off"),
            Some("https://c.example"),
        );
        assert_eq!(
            (option.base.as_str(), option.source, option.flag_may_route),
            ("https://a.example", "launch option", false)
        );
        let overridden = routing(None, Some("off"), Some("https://c.example"));
        assert_eq!(
            (overridden.base.as_str(), overridden.flag_may_route),
            ("https://c.example", false)
        );
        // the config field is only the flag's fallback
        let config = routing(None, Some("off"), None);
        assert_eq!((config.base.as_str(), config.flag_may_route), ("off", true));
        let unpinned = routing(None, None, None);
        assert_eq!(
            (unpinned.base.as_str(), unpinned.flag_may_route),
            (CDN, true)
        );
    }
}

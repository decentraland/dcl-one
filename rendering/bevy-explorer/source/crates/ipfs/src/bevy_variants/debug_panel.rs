use bevy::prelude::*;
use bevy_console::ConsoleCommand;
use common::sync::MutexExt;
use console::{AddDebugSection, DebugSection, DoAddConsoleCommand};
use serde_json::{json, Value};

use super::{
    Base, ContentFileVariantManifests, ContentFileVariantSession, ContentFileVariantsConfig,
    ManifestState,
};
use crate::IpfsResource;

pub(super) struct BevyVariantsDebugPanelPlugin;

impl Plugin for BevyVariantsDebugPanelPlugin {
    fn build(&self, app: &mut App) {
        app.add_console_command::<BevyVariantsCommand, _>(bevy_variants_state)
            .add_debug_section(DebugSection {
                title: "Optimized variants",
                query: "/bevy_variants",
                fields: &[
                    ("Variants base url", "base"),
                    ("Base set by", "baseSource"),
                    ("State", "state"),
                    ("Formats taken", "formats"),
                    (
                        "Scene manifests (loaded / none / failed / not eligible / pending)",
                        "manifestCounts",
                    ),
                    ("Scene manifests", "manifests"),
                    ("Variants served (downloaded / from cache)", "served"),
                    ("Variants served by format", "servedByFormat"),
                    ("Fallbacks to the original", "fallbacks"),
                    (
                        "Fallbacks from a chosen variant, by format",
                        "fallbacksByFormat",
                    ),
                    ("Recent fallbacks", "recentFallbacks"),
                ],
                actions: &[],
                inputs: &[],
            });
    }
}

/// The abgen Bevy variants client: base url, per-scene manifests, variants served, fallbacks.
#[derive(clap::Parser, ConsoleCommand)]
#[command(name = "/bevy_variants")]
struct BevyVariantsCommand;

fn bevy_variants_state(mut input: ConsoleCommand<BevyVariantsCommand>, ipfs: Res<IpfsResource>) {
    if let Some(Ok(_)) = input.take() {
        input.reply_ok(
            snapshot(
                &ipfs.content_file_variants_config,
                &ipfs.content_file_variant_manifests,
                &ipfs.content_file_variant_session,
            )
            .to_string(),
        );
    }
}

pub(super) fn snapshot(
    config: &ContentFileVariantsConfig,
    manifests: &ContentFileVariantManifests,
    session: &ContentFileVariantSession,
) -> Value {
    let mut counts = [0u64; 5];
    let route = config.route();
    let mut manifests: Vec<String> = manifests
        .0
        .lock_ignore_poison()
        .states
        .iter()
        .map(|(entity, state)| {
            let (slot, text) = match &*state.borrow() {
                ManifestState::Loaded(manifest) => (
                    0,
                    format!(
                        "loaded, {} files{}",
                        manifest.files.len(),
                        if manifest.lanes.is_empty() {
                            String::new()
                        } else {
                            let lanes: Vec<_> =
                                manifest.lanes.values().map(String::as_str).collect();
                            format!(" ({})", lanes.join(", "))
                        }
                    ),
                ),
                ManifestState::None => (1, "none".to_owned()),
                ManifestState::Failed(reason) => (2, format!("failed: {reason}")),
                ManifestState::Ineligible(reason) => (3, format!("not eligible: {reason}")),
                ManifestState::Pending => (4, "pending".to_owned()),
            };
            counts[slot] += 1;
            format!("{entity}: {text}")
        })
        .collect();
    manifests.sort();

    let session = session.lock();
    let state = match (&route.base, &session.disabled, config.reported_bc()) {
        (Base::Off, _, _) => "off".to_owned(),
        (Base::Pending(_), _, _) => "waiting for the endpoint flag".to_owned(),
        (Base::On(..), Some(reason), _) => format!("off for this session: {reason}"),
        (Base::On(..), None, None) => "on, waiting for the renderer's texture formats".to_owned(),
        (Base::On(..), None, Some(false)) => {
            "on, but the GPU has no BC textures: nothing is replaced".to_owned()
        }
        (Base::On(..), None, Some(true)) => "on".to_owned(),
    };
    let base = match &route.base {
        Base::On(base_url, _) => Some(base_url.view().as_str()),
        Base::Pending(_) | Base::Off => None,
    };
    let recent: Vec<String> = session
        .recent
        .iter()
        .rev()
        .map(|fallback| {
            format!(
                "{} ({}): {}",
                fallback.file, fallback.scene, fallback.reason
            )
        })
        .collect();
    json!({
        "base": base,
        "baseSource": route.source,
        "state": state,
        "formats": super::taken_formats(),
        "manifestCounts": counts,
        "manifests": (!manifests.is_empty()).then(|| manifests.join("\n")),
        "served": [session.downloaded, session.cached],
        "servedByFormat": (!session.served_by_format.is_empty()).then(|| {
            session
                .served_by_format
                .iter()
                .map(|(format, [downloaded, cached])| {
                    format!("{format}: {downloaded} downloaded / {cached} from cache")
                })
                .collect::<Vec<_>>()
                .join("\n")
        }),
        "fallbacks": session.fallbacks,
        "fallbacksByFormat": (!session.fallbacks_by_format.is_empty()).then(|| {
            session
                .fallbacks_by_format
                .iter()
                .map(|(format, count)| format!("{format}: {count}"))
                .collect::<Vec<_>>()
                .join("\n")
        }),
        "recentFallbacks": (!recent.is_empty()).then(|| recent.join("\n")),
        "unavailable": {
            "base": "Off, or waiting for the `bevy-variants-endpoint` flag: see State and Base set by.",
            "manifests": "No scene has asked for a manifest yet.",
            "servedByFormat": "No variant has been served.",
            "fallbacksByFormat": "No chosen variant has fallen back.",
            "recentFallbacks": "No file has fallen back to its original.",
        },
    })
}

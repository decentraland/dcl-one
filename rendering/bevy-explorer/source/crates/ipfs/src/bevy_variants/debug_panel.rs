use bevy::prelude::*;
use bevy_console::ConsoleCommand;
use common::sync::MutexExt;
use console::{AddDebugSection, DebugSection, DoAddConsoleCommand};
use serde_json::{json, Value};

use super::{BevyVariants, ManifestState};
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
                    ("Bases used this session", "usedBases"),
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
        input.reply_ok(snapshot(&ipfs.bevy_variants).to_string());
    }
}

pub(super) fn snapshot(variants: &BevyVariants) -> Value {
    let mut counts = [0u64; 5];
    let (current, base_source, used_bases) = {
        let bases = variants.base.lock_ignore_poison();
        (
            bases.current.clone(),
            bases.source.clone(),
            bases.used.clone(),
        )
    };
    let mut manifests: Vec<String> = variants
        .manifests
        .lock_ignore_poison()
        .iter()
        .map(|(entity, state)| {
            let (slot, text) = match &*state.borrow() {
                ManifestState::Loaded(manifest) => (
                    0,
                    format!(
                        "loaded, {} files{}{}",
                        manifest.files.len(),
                        if manifest.lanes.is_empty() {
                            String::new()
                        } else {
                            let lanes: Vec<_> =
                                manifest.lanes.values().map(String::as_str).collect();
                            format!(" ({})", lanes.join(", "))
                        },
                        if current.as_deref() == Some(manifest.base.as_str()) {
                            String::new()
                        } else {
                            format!(", from {}", manifest.base)
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

    let session = variants.session.lock_ignore_poison();
    // `current` was read above: `base` is never taken while `session` is held
    let state = match (current.as_deref(), &session.disabled, variants.bc.get()) {
        (None, _, _) => "off".to_owned(),
        (Some(_), Some(reason), _) => format!("off for this session: {reason}"),
        (Some(_), None, None) => "on, waiting for the renderer's texture formats".to_owned(),
        (Some(_), None, Some(false)) => {
            "on, but the GPU has no BC textures: nothing is replaced".to_owned()
        }
        (Some(_), None, Some(true)) => "on".to_owned(),
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
        "base": current,
        "baseSource": base_source,
        "usedBases": (!used_bases.is_empty()).then(|| used_bases.join("\n")),
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
            "base": "Off: --bevy-variants-url, ?bevyVariants= or bevy_variants_url in the config is `off`, or the `bevy-variants-endpoint` flag's variant is.",
            "manifests": "No scene has asked for a manifest yet.",
            "servedByFormat": "No variant has been served.",
            "fallbacksByFormat": "No chosen variant has fallen back.",
            "recentFallbacks": "No file has fallen back to its original.",
        },
    })
}

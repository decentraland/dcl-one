use bevy::prelude::*;
use bevy_console::ConsoleCommand;
use common::structs::AppConfig;
use console::{AddDebugSection, DebugInput, DebugInputKind, DebugSection, DoAddConsoleCommand};
use serde_json::json;
use std::{sync::atomic::Ordering, time::Duration};

use crate::{
    batch_sizes::BatchSizes, IpfsIo, IpfsResource, IPFS_CACHED, IPFS_FAILED, IPFS_IN_FLIGHT,
    IPFS_NON_IPFS, IPFS_SUCCESS,
};

#[derive(Default)]
pub(super) struct RequestStats {
    scene_batches: BatchSizes,
    attachment_batches: BatchSizes,
    bytes: u64,
    small: u64,
    large: u64,
    small_seconds: f64,
    large_seconds: f64,
    large_bytes: u64,
    seconds_per_mib: f64,
    first_bytes: u64,
    first_byte_seconds: f64,
    #[cfg(not(target_arch = "wasm32"))]
    pub(super) connect_failed: u64,
}

impl RequestStats {
    pub(super) fn download(
        &mut self,
        size: usize,
        duration: Duration,
        first_byte: Option<Duration>,
    ) {
        if let Some(time) = first_byte {
            self.first_bytes += 1;
            self.first_byte_seconds += time.as_secs_f64();
        }
        let bytes = size as u64;
        self.bytes += bytes;
        let seconds = duration.as_secs_f64();
        if bytes <= 10 * 1024 {
            self.small += 1;
            self.small_seconds += seconds;
        }
        if bytes >= 10 * 1024 {
            self.large += 1;
            self.large_seconds += seconds;
            self.large_bytes += bytes;
            self.seconds_per_mib += seconds * 1_048_576.0 / bytes as f64;
        }
    }
}

impl IpfsIo {
    pub(super) fn preview_request_delay(&self) -> Option<Duration> {
        if !self.is_preview {
            return None;
        }
        let ms = self.preview_delay_ms.load(Ordering::Relaxed);
        (ms > 0).then(|| Duration::from_millis(ms.into()))
    }
    /// Definitions returned in one completed scene lookup (pointers or a World's entity URNs).
    pub fn record_scene_batch(&self, size: usize) {
        self.debug.lock().unwrap().scene_batches.record(size);
    }

    /// Definitions returned in one wearable/emote pointer resolution batch.
    pub fn record_attachment_batch(&self, size: usize) {
        self.debug.lock().unwrap().attachment_batches.record(size);
    }
}

pub(super) fn register(app: &mut App, preview: bool) {
    app.add_console_command::<WebState, _>(web_state)
        .add_debug_section(DebugSection {
            title: "Web Requests",
            query: "/content_request_state",
            fields: &[
                ("IPFS downloads in flight (including queued)", "inFlight"),
                ("Successful downloads", "success"),
                ("Cached reads", "cached"),
                ("Non-IPFS reads", "local"),
                ("Failed reads", "failed"),
                #[cfg(not(target_arch = "wasm32"))]
                ("IPFS connection failures (native)", "connectFailed"),
                ("Available request slots", "available"),
                ("Configured download slots", "slots"),
                ("Downloaded content bytes", "bytes"),
                ("Mean time to first body byte (ms)", "firstByteMs"),
                ("Mean small download (ms, ≤10 KiB)", "smallMs"),
                ("Mean download time (ms/MiB, ≥10 KiB)", "msPerMiB"),
                ("Download fill rate (bytes/s, ≥10 KiB)", "fillRate"),
            ],
            actions: &[],
            inputs: &[],
        })
        .add_debug_section(DebugSection {
            title: "Entity Requests",
            query: "/content_request_state",
            fields: &[
                (
                    "Scene definitions per batch (min / mean / max)",
                    "sceneBatch",
                ),
                (
                    "Avatar attachments per batch (min / mean / max)",
                    "attachmentBatch",
                ),
            ],
            actions: &[],
            inputs: &[],
        });
    if preview {
        app.init_resource::<PreviewDelay>()
            .add_preview_console_command::<DelayCommand, _>(request_delay)
            .add_debug_section(DebugSection {
                title: "Web Requests Delay",
                query: "/content_request_delay",
                fields: &[],
                actions: &[],
                inputs: &[
                    DebugInput {
                        label: "Use artificial delay",
                        field: "enabled",
                        command: "/content_request_delay enabled {value}",
                        kind: DebugInputKind::Toggle,
                    },
                    DebugInput {
                        label: "Delay before each IPFS request (seconds)",
                        field: "seconds",
                        command: "/content_request_delay seconds {value}",
                        kind: DebugInputKind::Number {
                            min: 0.0,
                            max: MAX_DELAY_SECONDS,
                            step: 0.1,
                        },
                    },
                ],
            });
    }
}

const MAX_DELAY_SECONDS: f64 = 10.0;

#[derive(Resource)]
struct PreviewDelay {
    enabled: bool,
    millis: u32,
}
impl Default for PreviewDelay {
    fn default() -> Self {
        Self {
            enabled: false,
            millis: 1_000,
        }
    }
}

/// Inspect or set artificial delay on real preview content requests.
#[derive(clap::Parser, ConsoleCommand)]
#[command(name = "/content_request_delay")]
struct DelayCommand {
    #[command(subcommand)]
    setting: Option<DelaySetting>,
}
#[derive(clap::Subcommand)]
enum DelaySetting {
    Enabled {
        #[arg(action=clap::ArgAction::Set)]
        value: bool,
    },
    Seconds {
        value: f64,
    },
}

fn request_delay(
    mut input: ConsoleCommand<DelayCommand>,
    mut delay: ResMut<PreviewDelay>,
    ipfs: Res<IpfsResource>,
) {
    let Some(Ok(cmd)) = input.take() else { return };
    match cmd.setting {
        None => {
            input.reply_ok(
                json!({"enabled":delay.enabled,"seconds":f64::from(delay.millis)/1000.0})
                    .to_string(),
            );
            return;
        }
        Some(DelaySetting::Enabled { value }) => delay.enabled = value,
        Some(DelaySetting::Seconds { value }) => {
            if !(0.0..=MAX_DELAY_SECONDS).contains(&value) {
                input.reply_failed("Delay must be 0 to 10 seconds");
                return;
            }
            delay.millis = (value * 1000.0).round() as u32;
        }
    }
    let ms = if delay.enabled { delay.millis } else { 0 };
    ipfs.preview_delay_ms.store(ms, Ordering::Relaxed);
    input.reply_ok(format!("Preview content request delay is {ms} ms"));
}

#[cfg(test)]
mod delay_tests {
    use super::*;

    #[test]
    fn only_preview_installs_delay_controls_and_starts_disabled() {
        let mut retail = App::new();
        register(&mut retail, false);
        assert!(!retail.world().contains_resource::<PreviewDelay>());
        let mut preview = App::new();
        register(&mut preview, true);
        assert!(!preview.world().resource::<PreviewDelay>().enabled);
    }
}

/// Content download counters and completed active-entity batch sizes.
#[derive(clap::Parser, ConsoleCommand)]
#[command(name = "/content_request_state")]
struct WebState;

fn web_state(mut input: ConsoleCommand<WebState>, ipfs: Res<IpfsResource>, config: Res<AppConfig>) {
    if let Some(Ok(_)) = input.take() {
        let stats = ipfs.debug.lock().unwrap();
        #[cfg(not(target_arch = "wasm32"))]
        let connect_failed = Some(stats.connect_failed);
        #[cfg(target_arch = "wasm32")]
        let connect_failed: Option<u64> = None;
        input.reply_ok(json!({
            "inFlight": IPFS_IN_FLIGHT.load(Ordering::Relaxed),
            "success": IPFS_SUCCESS.load(Ordering::Relaxed),
            "cached": IPFS_CACHED.load(Ordering::Relaxed),
            "local": IPFS_NON_IPFS.load(Ordering::Relaxed),
            "failed": IPFS_FAILED.load(Ordering::Relaxed),
            "connectFailed": connect_failed,
            "available": ipfs.request_slots.available_permits(),
            "slots": config.max_concurrent_remotes,
            "bytes": stats.bytes,
            "firstByteMs": (stats.first_bytes > 0).then(|| (1000.0 * stats.first_byte_seconds / stats.first_bytes as f64).round()),
            "smallMs": (stats.small > 0).then(|| (1000.0 * stats.small_seconds / stats.small as f64).round()),
            "msPerMiB": (stats.large > 0).then(|| (1000.0 * stats.seconds_per_mib / stats.large as f64).round()),
            "fillRate": (stats.large_seconds > 0.0).then(|| (stats.large_bytes as f64 / stats.large_seconds).round()),
            "sceneBatch": stats.scene_batches.range(),
            "attachmentBatch": stats.attachment_batches.range(),
            "unavailable": {
                "firstByteMs": "No content download has returned a body byte yet.",
                "smallMs": "No content download of 10 KiB or less has completed.",
                "msPerMiB": "No content download of 10 KiB or more has completed.",
                "fillRate": "No transfer time recorded for downloads of 10 KiB or more.",
                "sceneBatch": "No scene definition lookup has completed.",
                "attachmentBatch": "No avatar attachment lookup has completed.",
            },
        }).to_string());
    }
}

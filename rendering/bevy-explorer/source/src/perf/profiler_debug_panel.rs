//! The Render and Profiler debug panel sections' queries and the profiler's switch, in a
//! `performance-profiler` build. Each query builds its JSON only when asked.

use std::collections::BTreeMap;

use bevy::{
    diagnostic::{Diagnostic, DiagnosticsStore},
    prelude::*,
    render::renderer::RenderAdapterInfo,
};
use bevy_console::ConsoleCommand;
use common::perf::{PERF_ALLOCATED_BYTES_PER_FRAME, PERF_ALLOCATION_COUNT_PER_FRAME};
use serde_json::json;

use super::profiled_scopes::{ProfiledScopeSortMetric, ProfiledScopesTable};

/// Report the render adapter, every render pass's CPU and GPU time and the pipeline statistics
/// Bevy's render diagnostics record, as JSON.
#[derive(clap::Parser, ConsoleCommand)]
#[command(name = "/render_state")]
pub(super) struct RenderStateCommand;

pub(super) fn render_state(
    mut input: ConsoleCommand<RenderStateCommand>,
    diagnostics: Res<DiagnosticsStore>,
    adapter: Option<Res<RenderAdapterInfo>>,
) {
    if input.take().is_none_or(|parsed| parsed.is_err()) {
        return;
    }
    let hundredths = |ms: f64| (ms * 100.0).round() / 100.0;
    // `render/<span>/elapsed_cpu|elapsed_gpu|<statistic>`; a pass is (cpu ms, gpu ms)
    let mut passes: BTreeMap<&str, (Option<f64>, Option<f64>)> = BTreeMap::new();
    let mut vertex_invocations = None;
    let mut fragment_invocations = None;
    for diagnostic in diagnostics.iter() {
        let Some((span, field)) = diagnostic
            .path()
            .as_str()
            .strip_prefix("render/")
            .and_then(|rest| rest.rsplit_once('/'))
        else {
            continue;
        };
        let value = diagnostic.smoothed().map(hundredths);
        match field {
            "elapsed_cpu" => passes.entry(span).or_default().0 = value,
            "elapsed_gpu" => passes.entry(span).or_default().1 = value,
            "vertex_shader_invocations" => vertex_invocations = value,
            "fragment_shader_invocations" => fragment_invocations = value,
            _ => {}
        }
    }
    let format_ms_or_dash = |ms: Option<f64>| ms.map_or("-".to_owned(), |ms| format!("{ms:.2}"));
    let pass_summary: Vec<String> = passes
        .iter()
        .map(|(name, (cpu, gpu))| {
            format!(
                "{name}: {} / {}",
                format_ms_or_dash(*cpu),
                format_ms_or_dash(*gpu)
            )
        })
        .collect();
    input.reply_ok(
        json!({
            "adapter": adapter.as_deref().map(|adapter| {
                format!("{} / {:?} / {}", adapter.name, adapter.backend, adapter.driver)
            }),
            "passes": passes.iter().map(|(name, (cpu, gpu))| json!({
                "name": name, "cpuMs": cpu, "gpuMs": gpu,
            })).collect::<Vec<_>>(),
            "passSummary": (!pass_summary.is_empty()).then(|| pass_summary.join("\n")),
            "pipeline": vertex_invocations.map(|vertex| [Some(vertex), fragment_invocations]),
            "unavailable": {
                "adapter": "No render adapter in this world.",
                "passSummary": if cfg!(target_arch = "wasm32") {
                    "Bevy's render diagnostics do not run on the web."
                } else if cfg!(feature = "tracy") {
                    "Bevy's render diagnostics do not run in a `tracy` build."
                } else {
                    "No render pass has reported a time yet."
                },
                "pipeline": "Pipeline statistics need a Vulkan adapter.",
            },
        })
        .to_string(),
    );
}

/// Report the scope profiler's top scopes as JSON: `count` rows (20 by default) sorted by
/// `--sort` (self time by default), plus the allocations per frame.
#[derive(clap::Parser, ConsoleCommand)]
#[command(name = "/profile_top")]
pub(super) struct ProfileTopCommand {
    count: Option<usize>,
    #[arg(long, value_enum)]
    sort: Option<ProfiledScopeSortMetric>,
}

pub(super) fn profile_top(
    mut input: ConsoleCommand<ProfileTopCommand>,
    table: Res<ProfiledScopesTable>,
    diagnostics: Res<DiagnosticsStore>,
) {
    let Some(Ok(command)) = input.take() else {
        return;
    };
    let sort = command.sort.unwrap_or(ProfiledScopeSortMetric::SelfTime);
    let rows = table.top_scope_summaries(command.count.unwrap_or(20), sort);
    let hundredths = |ms: f32| (f64::from(ms) * 100.0).round() / 100.0;
    let scope_summary: Vec<String> = rows
        .iter()
        .map(|row| {
            format!(
                "{}: {:?}, {}, {:.2}, {:.2}, {:.2}, {:.2}",
                row.scope_name,
                row.scope_kind,
                row.call_count,
                row.self_time_ms,
                row.inclusive_time_ms,
                row.max_single_call_ms,
                row.smoothed_self_time_per_frame_ms,
            )
        })
        .collect();
    let allocations = diagnostics
        .get(&PERF_ALLOCATION_COUNT_PER_FRAME)
        .and_then(Diagnostic::smoothed)
        .map(|count| {
            [
                Some(count.round()),
                diagnostics
                    .get(&PERF_ALLOCATED_BYTES_PER_FRAME)
                    .and_then(Diagnostic::smoothed)
                    .map(f64::round),
            ]
        });
    input.reply_ok(
        json!({
            "enabled": table.recording_enabled(),
            "sort": format!("{sort:?}"),
            "scopeSummaries": rows.iter().map(|row| json!({
                "scopeName": row.scope_name, "scopeKind": row.scope_kind, "callCount": row.call_count,
                "selfTimeMs": hundredths(row.self_time_ms), "inclusiveTimeMs": hundredths(row.inclusive_time_ms),
                "maxSingleCallMs": hundredths(row.max_single_call_ms),
                "smoothedSelfTimePerFrameMs": hundredths(row.smoothed_self_time_per_frame_ms),
            })).collect::<Vec<_>>(),
            "scopeSummary": (!scope_summary.is_empty()).then(|| scope_summary.join("\n")),
            "allocations": allocations,
            "unavailable": {
                "scopeSummary": if table.recording_enabled() {
                    "No scope has been recorded yet."
                } else {
                    "Profiler disabled: enable it to record scopes."
                },
                "allocations": "Waiting for the first frame's allocation counts.",
            },
        })
        .to_string(),
    );
}

/// Start (`on`) or stop (`off`) the scope profiler's recording, or `reset` its table.
#[derive(clap::Parser, ConsoleCommand)]
#[command(name = "/profile_enable")]
pub(super) struct ProfileEnableCommand {
    #[arg(value_enum)]
    action: ProfilerRecordingAction,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, clap::ValueEnum)]
enum ProfilerRecordingAction {
    On,
    Off,
    Reset,
}

pub(super) fn profile_enable(
    mut input: ConsoleCommand<ProfileEnableCommand>,
    mut table: ResMut<ProfiledScopesTable>,
) {
    let Some(Ok(command)) = input.take() else {
        return;
    };
    match command.action {
        ProfilerRecordingAction::On => table.set_recording_enabled(true),
        ProfilerRecordingAction::Off => table.set_recording_enabled(false),
        ProfilerRecordingAction::Reset => table.reset(),
    }
    input.reply_ok(json!({ "enabled": table.recording_enabled() }).to_string());
}

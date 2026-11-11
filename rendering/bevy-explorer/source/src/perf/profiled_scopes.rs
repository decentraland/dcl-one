//! The in-app scope profiler: a `tracing` layer that times Bevy's spans (`system`, `schedule`,
//! `node`) and the explorer's own, folded once a frame into [`ProfiledScopesTable`].
//!
//! The layer records nothing while the table is disabled: `on_enter` and `on_exit` return on one
//! atomic load.
//!
//! Self time excludes the child scopes entered on the same thread: each thread keeps a stack of
//! the scopes it has entered, and a scope's inclusive time is added to the one below it when it
//! exits. A schedule run on the main thread whose systems run on the task pool keeps their time
//! in its self time.

use std::{
    cell::RefCell,
    collections::HashMap,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, LazyLock, Mutex, MutexGuard,
    },
    time::Duration,
};

use bevy::{
    log::{
        tracing::{
            field::{Field, Visit},
            span::{Attributes, Id},
            Metadata, Subscriber,
        },
        tracing_subscriber::{layer::Context, registry::LookupSpan, Layer},
    },
    platform::time::Instant,
    prelude::*,
};
use common::perf::{ProfiledScopeKind, ProfiledScopeSummary};

/// Frames the smoothed per-frame self time averages over.
const SMOOTHING_FRAMES: f32 = 60.0;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct ProfiledScopeKey {
    kind: ProfiledScopeKind,
    name: Arc<str>,
}

/// One scope's calls within the frame being recorded.
#[derive(Default, Clone, Copy)]
struct ProfiledScopeFrameAccumulation {
    call_count: u32,
    self_time: Duration,
    inclusive_time: Duration,
    max_single_call: Duration,
}

/// What the layer and the table share.
struct ProfiledScopesShared {
    enabled: AtomicBool,
    frame: Mutex<HashMap<ProfiledScopeKey, ProfiledScopeFrameAccumulation>>,
}

static PROFILED_SCOPES_SHARED: LazyLock<ProfiledScopesShared> =
    LazyLock::new(|| ProfiledScopesShared {
        enabled: AtomicBool::new(false),
        frame: Mutex::new(HashMap::new()),
    });

fn profiler_recording_enabled() -> bool {
    PROFILED_SCOPES_SHARED.enabled.load(Ordering::Relaxed)
}

fn lock_current_frame_accumulations(
) -> MutexGuard<'static, HashMap<ProfiledScopeKey, ProfiledScopeFrameAccumulation>> {
    PROFILED_SCOPES_SHARED
        .frame
        .lock()
        .unwrap_or_else(|poison| poison.into_inner())
}

fn take_current_frame_accumulations() -> HashMap<ProfiledScopeKey, ProfiledScopeFrameAccumulation> {
    std::mem::take(&mut *lock_current_frame_accumulations())
}

/// An entered scope on this thread.
struct EnteredProfiledScope {
    id: Id,
    key: ProfiledScopeKey,
    started: Instant,
    child_inclusive_time: Duration,
}

thread_local! {
    static ENTERED_SCOPE_STACK: RefCell<Vec<EnteredProfiledScope>> = const { RefCell::new(Vec::new()) };
}

/// The layer to install through `LogPlugin::custom_layer`.
pub fn profiled_scopes_layer() -> ProfiledScopesLayer {
    ProfiledScopesLayer
}

pub struct ProfiledScopesLayer;

fn classify_profiled_scope_kind(metadata: &Metadata<'_>) -> ProfiledScopeKind {
    match metadata.name() {
        "system" => ProfiledScopeKind::EcsSystem,
        "schedule" => ProfiledScopeKind::Schedule,
        "node" => ProfiledScopeKind::RenderGraphNode,
        _ if metadata.target().starts_with("bevy_") => ProfiledScopeKind::BevyInternal,
        _ => ProfiledScopeKind::ExplorerSpan,
    }
}

/// The span's `name` field, as Bevy records it: a string for systems and nodes, a `Debug` of
/// the label for schedules.
#[derive(Default)]
struct SpanNameFieldVisitor(Option<String>);

impl Visit for SpanNameFieldVisitor {
    fn record_str(&mut self, field: &Field, value: &str) {
        if field.name() == "name" {
            self.0 = Some(value.to_owned());
        }
    }

    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        if field.name() == "name" {
            self.0 = Some(format!("{value:?}"));
        }
    }
}

impl<S> Layer<S> for ProfiledScopesLayer
where
    S: Subscriber + for<'a> LookupSpan<'a>,
{
    fn on_new_span(&self, attrs: &Attributes<'_>, id: &Id, ctx: Context<'_, S>) {
        let metadata = attrs.metadata();
        let kind = classify_profiled_scope_kind(metadata);
        // a `system` span's name is kept even while recording is off, so a span created before
        // an enable still has one
        if kind != ProfiledScopeKind::EcsSystem && !profiler_recording_enabled() {
            return;
        }
        let Some(span) = ctx.span(id) else {
            return;
        };
        let mut visitor = SpanNameFieldVisitor::default();
        attrs.record(&mut visitor);
        let name = visitor.0.unwrap_or_else(|| metadata.name().to_owned());
        span.extensions_mut().insert(ProfiledScopeKey {
            kind,
            name: name.into(),
        });
    }

    fn on_enter(&self, id: &Id, ctx: Context<'_, S>) {
        if !profiler_recording_enabled() {
            return;
        }
        let Some(span) = ctx.span(id) else {
            return;
        };
        let extensions = span.extensions();
        let Some(key) = extensions.get::<ProfiledScopeKey>() else {
            return;
        };
        let entered = EnteredProfiledScope {
            id: id.clone(),
            key: key.clone(),
            started: Instant::now(),
            child_inclusive_time: Duration::ZERO,
        };
        ENTERED_SCOPE_STACK.with(|stack| stack.borrow_mut().push(entered));
    }

    fn on_exit(&self, id: &Id, _: Context<'_, S>) {
        if !profiler_recording_enabled() {
            return;
        }
        let Some((entered, inclusive)) = ENTERED_SCOPE_STACK.with(|stack| {
            let mut stack = stack.borrow_mut();
            if !stack.last().is_some_and(|top| top.id == *id) {
                return None;
            }
            let entered = stack.pop()?;
            let inclusive = entered.started.elapsed();
            if let Some(parent) = stack.last_mut() {
                parent.child_inclusive_time += inclusive;
            }
            Some((entered, inclusive))
        }) else {
            return;
        };
        let self_time = inclusive.saturating_sub(entered.child_inclusive_time);
        let mut frame = lock_current_frame_accumulations();
        let accum = frame.entry(entered.key).or_default();
        accum.call_count += 1;
        accum.self_time += self_time;
        accum.inclusive_time += inclusive;
        accum.max_single_call = accum.max_single_call.max(inclusive);
    }
}

/// What a scope's summary rows sort by.
#[derive(Clone, Copy, Debug, PartialEq, Eq, clap::ValueEnum)]
pub enum ProfiledScopeSortMetric {
    SelfTime,
    InclusiveTime,
    MaxSingleCall,
    CallCount,
}

/// Every scope seen since the last reset, with its summary. Resource.
#[derive(Resource)]
pub struct ProfiledScopesTable {
    scope_summaries: HashMap<ProfiledScopeKey, ProfiledScopeSummary>,
}

impl ProfiledScopesTable {
    /// Disabled and empty: the table's initial state, not a default value of the type.
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        Self {
            scope_summaries: HashMap::new(),
        }
    }

    pub fn recording_enabled(&self) -> bool {
        profiler_recording_enabled()
    }

    /// Starts or stops the layer's recording; starting resets the table.
    pub fn set_recording_enabled(&mut self, enabled: bool) {
        if enabled && !self.recording_enabled() {
            self.reset();
        }
        PROFILED_SCOPES_SHARED
            .enabled
            .store(enabled, Ordering::Relaxed);
    }

    pub fn reset(&mut self) {
        self.scope_summaries.clear();
        lock_current_frame_accumulations().clear();
    }

    /// The `count` scopes with the most of `sort`, highest first.
    pub fn top_scope_summaries(
        &self,
        count: usize,
        sort: ProfiledScopeSortMetric,
    ) -> Vec<ProfiledScopeSummary> {
        let mut rows: Vec<ProfiledScopeSummary> = self.scope_summaries.values().cloned().collect();
        rows.sort_by(|a, b| {
            let metric = |row: &ProfiledScopeSummary| match sort {
                ProfiledScopeSortMetric::SelfTime => row.self_time_ms,
                ProfiledScopeSortMetric::InclusiveTime => row.inclusive_time_ms,
                ProfiledScopeSortMetric::MaxSingleCall => row.max_single_call_ms,
                ProfiledScopeSortMetric::CallCount => row.call_count as f32,
            };
            metric(b)
                .total_cmp(&metric(a))
                .then_with(|| a.scope_name.cmp(&b.scope_name))
        });
        rows.truncate(count);
        rows
    }

    /// Adds one frame's accumulation to every row and moves the smoothed per-frame self times.
    fn fold_frame_accumulations(
        &mut self,
        frame: HashMap<ProfiledScopeKey, ProfiledScopeFrameAccumulation>,
    ) {
        let alpha = 2.0 / (SMOOTHING_FRAMES + 1.0);
        for row in self.scope_summaries.values_mut() {
            row.smoothed_self_time_per_frame_ms *= 1.0 - alpha;
        }
        let ms = |duration: Duration| duration.as_secs_f32() * 1000.0;
        for (key, accum) in frame {
            let row =
                self.scope_summaries
                    .entry(key.clone())
                    .or_insert_with(|| ProfiledScopeSummary {
                        scope_name: key.name.to_string(),
                        scope_kind: key.kind,
                        call_count: 0,
                        self_time_ms: 0.0,
                        inclusive_time_ms: 0.0,
                        max_single_call_ms: 0.0,
                        smoothed_self_time_per_frame_ms: 0.0,
                    });
            row.call_count += accum.call_count;
            row.self_time_ms += ms(accum.self_time);
            row.inclusive_time_ms += ms(accum.inclusive_time);
            row.max_single_call_ms = row.max_single_call_ms.max(ms(accum.max_single_call));
            row.smoothed_self_time_per_frame_ms += alpha * ms(accum.self_time);
        }
    }
}

/// `Last`: folds the frame's recording into the table.
pub fn fold_profiled_scopes_system(mut table: ResMut<ProfiledScopesTable>) {
    if !profiler_recording_enabled() {
        return;
    }
    let frame = take_current_frame_accumulations();
    if !frame.is_empty() {
        table.fold_frame_accumulations(frame);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(kind: ProfiledScopeKind, name: &str) -> ProfiledScopeKey {
        ProfiledScopeKey {
            kind,
            name: name.into(),
        }
    }

    #[test]
    fn a_fold_sums_calls_and_keeps_the_longest_call() {
        let mut table = ProfiledScopesTable::new();
        let system = key(ProfiledScopeKind::EcsSystem, "a::system");
        let frame = |self_ms: u64, max_ms: u64| {
            HashMap::from([(
                system.clone(),
                ProfiledScopeFrameAccumulation {
                    call_count: 2,
                    self_time: Duration::from_millis(self_ms),
                    inclusive_time: Duration::from_millis(self_ms + 1),
                    max_single_call: Duration::from_millis(max_ms),
                },
            )])
        };
        table.fold_frame_accumulations(frame(4, 3));
        table.fold_frame_accumulations(frame(2, 1));
        let rows = table.top_scope_summaries(10, ProfiledScopeSortMetric::SelfTime);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].scope_name, "a::system");
        assert_eq!(rows[0].scope_kind, ProfiledScopeKind::EcsSystem);
        assert_eq!(rows[0].call_count, 4);
        assert_eq!(rows[0].self_time_ms, 6.0);
        assert_eq!(rows[0].inclusive_time_ms, 8.0);
        assert_eq!(rows[0].max_single_call_ms, 3.0);
        assert!(rows[0].smoothed_self_time_per_frame_ms > 0.0);
    }

    #[test]
    fn top_sorts_by_the_chosen_metric_and_truncates() {
        let mut table = ProfiledScopesTable::new();
        let accum = |call_count, self_ms, max_ms| ProfiledScopeFrameAccumulation {
            call_count,
            self_time: Duration::from_millis(self_ms),
            inclusive_time: Duration::from_millis(self_ms),
            max_single_call: Duration::from_millis(max_ms),
        };
        table.fold_frame_accumulations(HashMap::from([
            (key(ProfiledScopeKind::EcsSystem, "slow"), accum(2, 9, 9)),
            (key(ProfiledScopeKind::EcsSystem, "busy"), accum(50, 5, 2)),
            (key(ProfiledScopeKind::Schedule, "Update"), accum(1, 1, 1)),
        ]));
        let names = |sort| {
            table
                .top_scope_summaries(2, sort)
                .into_iter()
                .map(|row| row.scope_name)
                .collect::<Vec<_>>()
        };
        assert_eq!(names(ProfiledScopeSortMetric::SelfTime), ["slow", "busy"]);
        assert_eq!(names(ProfiledScopeSortMetric::CallCount), ["busy", "slow"]);
        assert_eq!(
            names(ProfiledScopeSortMetric::MaxSingleCall),
            ["slow", "busy"]
        );
    }

    /// The layer on a registry: nested spans entered on one thread give the parent its self time
    /// without the child's, and a disabled layer records nothing.
    #[test]
    fn self_time_excludes_the_children_and_a_disabled_layer_records_nothing() {
        use bevy::log::tracing::{info_span, subscriber::with_default};
        use bevy::log::tracing_subscriber::{layer::SubscriberExt, registry};

        let subscriber = registry().with(profiled_scopes_layer());
        let mut table = ProfiledScopesTable::new();
        with_default(subscriber, || {
            // disabled: nothing recorded
            {
                let _outer = info_span!("outer").entered();
                std::thread::sleep(Duration::from_millis(2));
            }
            assert!(lock_current_frame_accumulations().is_empty());

            table.set_recording_enabled(true);
            {
                let _outer = info_span!("outer").entered();
                {
                    let _inner = info_span!("system", name = "the_system").entered();
                    std::thread::sleep(Duration::from_millis(5));
                }
                std::thread::sleep(Duration::from_millis(1));
            }
            table.set_recording_enabled(false);
        });
        table.fold_frame_accumulations(take_current_frame_accumulations());
        let rows = table.top_scope_summaries(10, ProfiledScopeSortMetric::InclusiveTime);
        assert_eq!(rows.len(), 2, "{rows:?}");
        let outer = rows.iter().find(|row| row.scope_name == "outer").unwrap();
        let inner = rows
            .iter()
            .find(|row| row.scope_name == "the_system")
            .unwrap();
        assert_eq!(outer.scope_kind, ProfiledScopeKind::ExplorerSpan);
        assert_eq!(inner.scope_kind, ProfiledScopeKind::EcsSystem);
        assert!(inner.self_time_ms >= 5.0, "{inner:?}");
        assert!(
            outer.inclusive_time_ms >= inner.inclusive_time_ms,
            "{outer:?}"
        );
        assert!(
            outer.self_time_ms < inner.self_time_ms,
            "{outer:?} {inner:?}"
        );
    }
}

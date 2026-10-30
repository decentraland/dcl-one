//! The GPU time of each frame, measured with timestamp queries on every backend, published as
//! [`GPU_FRAME_TIME`].
//!
//! Two empty compute passes bracket the frame's work inside its submission: the first node of
//! the render graph records a pass that writes a timestamp as it begins, the last node one that
//! writes a timestamp as it ends, then the resolve of the two and their copy to a readback buffer.
//! Every other node of the render graph, camera sub-graphs included, records its passes between
//! the two, and all of them reach the GPU in one submission, so the time between the timestamps
//! is the GPU time of the frame's passes. Not counted: the render world's preparation of the
//! frame (the first timestamp is written only once the frame is submitted), the uploads queued
//! while it was prepared (they run just ahead of the submission's command buffers) and work
//! submitted outside the render graph. The brackets are empty passes, the cheapest that write a
//! timestamp; `tests/gpu_timestamps.rs` checks that an empty pass writes its timestamps on an
//! adapter.
//!
//! After the submission the readback buffer is mapped without waiting. A later frame reads it,
//! converts the ticks with the queue's timestamp period and hands the milliseconds to the main
//! world, which records them. A frame's measurement is recorded three frames after the frame
//! when the GPU finishes each frame before the next one is submitted, and later when it does not.
//! On WebGPU, Chrome quantises timestamps (to tens of microseconds): a frame whose two timestamps
//! quantise to the same value gives no sample, and the CSV repeats the previous value.
//!
//! Each frame in flight holds one of [`RING`] slots: two timestamp queries and a readback buffer.
//! When every slot is still waiting for its readback, the frame is not measured; nothing waits
//! and nothing grows. Only a frame whose two brackets were both recorded is resolved and read
//! back: a frame that got a slot but did not record both frees it at the next frame without
//! resolving, since resolving an unwritten timestamp stalls the queue on Vulkan. A readback that
//! fails to map leaves its buffer unable to map again, so its slot is retired for good, with one
//! warning under target `gpu_time`; once every slot is retired nothing more is measured.
//!
//! The brackets enclose the render-graph nodes that exist when the plugin finishes. On the first
//! render frame the plugin checks that every node of the graph runs between them and names, in
//! one warning under `gpu_time`, any that does not.
//!
//! An adapter without timestamp queries gets none of this: one info line under target
//! `gpu_time`, and [`GPU_FRAME_TIME`] is never registered.

use std::{
    collections::HashSet,
    sync::{
        atomic::{AtomicU64, AtomicU8, Ordering},
        Arc,
    },
};

use bevy::{
    diagnostic::{Diagnostic, Diagnostics, RegisterDiagnostic},
    prelude::*,
    render::{
        render_graph::{
            InternedRenderLabel, Node, NodeRunError, RenderGraph, RenderGraphContext, RenderLabel,
        },
        renderer::{render_system, RenderContext, RenderDevice, RenderQueue, WgpuWrapper},
        Render, RenderApp, RenderSet,
    },
};
use common::gpu_time::GPU_FRAME_TIME;
use wgpu::{
    Buffer, BufferDescriptor, BufferUsages, CommandEncoder, ComputePassDescriptor,
    ComputePassTimestampWrites, Features, Maintain, MapMode, QuerySet, QuerySetDescriptor,
    QueryType,
};

/// Frames that can be measured at once: a frame holds its slot from the start of its render
/// schedule until its readback has been read.
const RING: usize = 4;

/// The two timestamps of a slot, in bytes.
const SLOT_BYTES: u64 = 2 * 8;

/// Measures the GPU time of each frame where the adapter supports timestamp queries; see the
/// module doc. Rendering clients only: it needs the render app. Its brackets enclose the
/// render-graph nodes that exist when it finishes, so it goes after the plugins that add nodes;
/// the first render frame names any node it does not enclose.
pub struct GpuTimePlugin;

impl Plugin for GpuTimePlugin {
    fn build(&self, _app: &mut App) {}

    /// The render device exists once the render plugin has finished (on the web, `finish` runs
    /// on the render worker, which owns the device).
    fn finish(&self, app: &mut App) {
        let Some(render_app) = app.get_sub_app_mut(RenderApp) else {
            return;
        };
        let Some(device) = render_app.world().get_resource::<RenderDevice>().cloned() else {
            return;
        };
        if !device.features().contains(Features::TIMESTAMP_QUERY) {
            info!(
                target: "gpu_time",
                "GPU frame time unavailable: the adapter has no timestamp queries"
            );
            return;
        }
        let shared = GpuTimeShared::default();
        render_app
            .insert_resource(GpuTimer::new(&device))
            .insert_resource(shared.clone())
            .add_systems(
                Render,
                (
                    (check_brackets.run_if(run_once), begin_frame)
                        .in_set(RenderSet::ExtractCommands),
                    end_frame.in_set(RenderSet::Render).after(render_system),
                    collect.in_set(RenderSet::Cleanup),
                ),
            );
        add_brackets(&mut render_app.world_mut().resource_mut::<RenderGraph>());
        add_publisher(app, shared);
    }
}

/// What a slot's readback map has reported, as its callback stores it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
enum MapState {
    Waiting,
    Mapped,
    Failed,
}

impl MapState {
    fn load(state: &AtomicU8) -> Self {
        match state.load(Ordering::Acquire) {
            s if s == MapState::Mapped as u8 => MapState::Mapped,
            s if s == MapState::Failed as u8 => MapState::Failed,
            _ => MapState::Waiting,
        }
    }
}

/// How far the open slot's brackets were recorded this frame.
const RECORDED_NONE: u8 = 0;
const RECORDED_BEGIN: u8 = 1;
const RECORDED_BOTH: u8 = 2;

/// Render world: the queries, buffers and slots of the measurement.
#[derive(Resource)]
struct GpuTimer(WgpuWrapper<Measurement>);

struct Measurement {
    /// Two timestamps per slot: `2 * slot` written by the first bracket, `2 * slot + 1` by the
    /// second.
    queries: QuerySet,
    /// Where a slot's two timestamps are resolved before they are copied to its readback.
    resolve: Buffer,
    readbacks: [Readback; RING],
    ring: Ring,
    /// `RECORDED_NONE`, `RECORDED_BEGIN` or `RECORDED_BOTH`, set by the graph's nodes, which see
    /// the render world only through a shared reference.
    recorded: AtomicU8,
    /// Whether a retired slot was reported.
    warned_retired: bool,
}

struct Readback {
    buffer: Buffer,
    /// A [`MapState`], set by the map callback.
    map: Arc<AtomicU8>,
}

impl GpuTimer {
    fn new(device: &RenderDevice) -> Self {
        let device = device.wgpu_device();
        Self(WgpuWrapper::new(Measurement {
            queries: device.create_query_set(&QuerySetDescriptor {
                label: Some("gpu frame time"),
                ty: QueryType::Timestamp,
                count: 2 * RING as u32,
            }),
            resolve: device.create_buffer(&BufferDescriptor {
                label: Some("gpu frame time resolve"),
                size: SLOT_BYTES,
                usage: BufferUsages::QUERY_RESOLVE | BufferUsages::COPY_SRC,
                mapped_at_creation: false,
            }),
            readbacks: std::array::from_fn(|_| Readback {
                buffer: device.create_buffer(&BufferDescriptor {
                    label: Some("gpu frame time readback"),
                    size: SLOT_BYTES,
                    usage: BufferUsages::MAP_READ | BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                }),
                map: Arc::new(AtomicU8::new(MapState::Waiting as u8)),
            }),
            ring: Ring::default(),
            recorded: AtomicU8::new(RECORDED_NONE),
            warned_retired: false,
        }))
    }
}

impl Readback {
    fn map_state(&self) -> MapState {
        MapState::load(&self.map)
    }

    /// The two timestamps of a mapped readback, unmapping it.
    fn read(&self) -> Option<(u64, u64)> {
        let ticks = {
            let bytes = self.buffer.slice(..).get_mapped_range();
            match bytes.as_chunks::<8>().0 {
                [begin, end] => Some((u64::from_le_bytes(*begin), u64::from_le_bytes(*end))),
                _ => None,
            }
        };
        self.buffer.unmap();
        ticks
    }
}

/// The first of a slot's two timestamp queries.
fn first_query(slot: usize) -> u32 {
    2 * slot as u32
}

/// Records one empty compute pass that writes the given timestamps of `queries`.
fn bracket(
    encoder: &mut CommandEncoder,
    queries: &QuerySet,
    beginning_of_pass_write_index: Option<u32>,
    end_of_pass_write_index: Option<u32>,
) {
    drop(encoder.begin_compute_pass(&ComputePassDescriptor {
        label: Some("gpu frame time bracket"),
        timestamp_writes: Some(ComputePassTimestampWrites {
            query_set: queries,
            beginning_of_pass_write_index,
            end_of_pass_write_index,
        }),
    }));
}

#[derive(Debug, Hash, PartialEq, Eq, Clone, RenderLabel)]
struct BeginLabel;

#[derive(Debug, Hash, PartialEq, Eq, Clone, RenderLabel)]
struct EndLabel;

/// Runs before every other node of the render graph: the first bracket of the open slot.
struct BeginNode;

impl Node for BeginNode {
    fn run<'w>(
        &self,
        _: &mut RenderGraphContext,
        render_context: &mut RenderContext<'w>,
        world: &'w World,
    ) -> Result<(), NodeRunError> {
        let measurement = &*world.resource::<GpuTimer>().0;
        if let Some(slot) = measurement.ring.open() {
            let first = first_query(slot);
            bracket(
                render_context.command_encoder(),
                &measurement.queries,
                Some(first),
                None,
            );
            measurement
                .recorded
                .store(RECORDED_BEGIN, Ordering::Relaxed);
        }
        Ok(())
    }
}

/// Runs after every other node of the render graph: the second bracket of the open slot, the
/// resolve of its two timestamps and their copy to its readback, once the first was recorded.
struct EndNode;

impl Node for EndNode {
    fn run<'w>(
        &self,
        _: &mut RenderGraphContext,
        render_context: &mut RenderContext<'w>,
        world: &'w World,
    ) -> Result<(), NodeRunError> {
        let measurement = &*world.resource::<GpuTimer>().0;
        let Some(slot) = measurement.ring.open() else {
            return Ok(());
        };
        if measurement.recorded.load(Ordering::Relaxed) != RECORDED_BEGIN {
            return Ok(());
        }
        let first = first_query(slot);
        let encoder = render_context.command_encoder();
        bracket(encoder, &measurement.queries, None, Some(first + 1));
        encoder.resolve_query_set(
            &measurement.queries,
            first..first + 2,
            &measurement.resolve,
            0,
        );
        let readback = &measurement.readbacks[slot].buffer;
        encoder.copy_buffer_to_buffer(&measurement.resolve, 0, readback, 0, SLOT_BYTES);
        measurement.recorded.store(RECORDED_BOTH, Ordering::Relaxed);
        Ok(())
    }
}

/// Adds the two bracket nodes to `graph`, ordered before and after every node already in it.
fn add_brackets(graph: &mut RenderGraph) {
    let nodes: Vec<InternedRenderLabel> = graph.iter_nodes().map(|node| node.label).collect();
    graph.add_node(BeginLabel, BeginNode);
    graph.add_node(EndLabel, EndNode);
    graph.add_node_edge(BeginLabel, EndLabel);
    for node in nodes {
        graph.add_node_edge(BeginLabel, node);
        graph.add_node_edge(node, EndLabel);
    }
}

/// The nodes of `graph`, other than the brackets, that do not run both after the first bracket
/// and before the second.
fn unbracketed(graph: &RenderGraph) -> Vec<InternedRenderLabel> {
    let (begin, end) = (BeginLabel.intern(), EndLabel.intern());
    let after_begin = reachable(graph, begin, true);
    let before_end = reachable(graph, end, false);
    graph
        .iter_nodes()
        .map(|node| node.label)
        .filter(|label| *label != begin && *label != end)
        .filter(|label| !(after_begin.contains(label) && before_end.contains(label)))
        .collect()
}

/// The nodes that run after `from` (`after`) or before it, following the graph's edges.
fn reachable(
    graph: &RenderGraph,
    from: InternedRenderLabel,
    after: bool,
) -> HashSet<InternedRenderLabel> {
    let mut found = HashSet::new();
    let mut next = vec![from];
    while let Some(label) = next.pop() {
        let Ok(node) = graph.get_node_state(label) else {
            continue;
        };
        let edges = if after {
            node.edges.output_edges()
        } else {
            node.edges.input_edges()
        };
        for edge in edges {
            let other = if after {
                edge.get_input_node()
            } else {
                edge.get_output_node()
            };
            if found.insert(other) {
                next.push(other);
            }
        }
    }
    found
}

/// First render frame: names the render-graph nodes the brackets do not enclose, whose GPU work
/// the measurement leaves out (nodes added after the plugin finished).
fn check_brackets(graph: Res<RenderGraph>) {
    let outside = unbracketed(&graph);
    if !outside.is_empty() {
        warn!(
            target: "gpu_time",
            "GPU frame time leaves out the render-graph nodes {outside:?}: they run outside its \
             brackets"
        );
    }
}

/// Start of the render schedule: frees the slot of a frame that did not record both brackets,
/// then opens a slot for this frame, which the graph's first and last nodes bracket. With every
/// slot in use or retired the frame is not measured.
fn begin_frame(mut timer: ResMut<GpuTimer>) {
    let measurement = &mut *timer.0;
    measurement.ring.abandon_open();
    measurement.recorded.store(RECORDED_NONE, Ordering::Relaxed);
    measurement.ring.begin();
}

/// After the frame was submitted: maps the readback of a frame that recorded both brackets,
/// without waiting.
fn end_frame(mut timer: ResMut<GpuTimer>) {
    let measurement = &mut *timer.0;
    let Some(slot) = measurement.ring.open() else {
        return;
    };
    if measurement.recorded.swap(RECORDED_NONE, Ordering::Relaxed) != RECORDED_BOTH {
        return;
    }
    measurement.ring.end(slot);
    let readback = &measurement.readbacks[slot];
    readback
        .map
        .store(MapState::Waiting as u8, Ordering::Release);
    let map = readback.map.clone();
    readback
        .buffer
        .slice(..)
        .map_async(MapMode::Read, move |result| {
            let state = if result.is_ok() {
                MapState::Mapped
            } else {
                MapState::Failed
            };
            map.store(state as u8, Ordering::Release);
        });
}

/// End of the render schedule: publishes the samples whose readback has arrived and frees their
/// slots; retires the slots whose readback failed, warning the first time.
fn collect(
    mut timer: ResMut<GpuTimer>,
    device: Res<RenderDevice>,
    queue: Res<RenderQueue>,
    shared: Res<GpuTimeShared>,
) {
    // runs the map callbacks of the readbacks that finished (on the web the browser does)
    device.poll(Maintain::Poll);
    let Measurement {
        readbacks,
        ring,
        warned_retired,
        ..
    } = &mut *timer.0;
    let retired = collect_ready(
        ring,
        |slot| readbacks[slot].map_state(),
        |slot| readbacks[slot].read(),
        queue.get_timestamp_period(),
        &shared,
    );
    if retired > 0 && !*warned_retired {
        *warned_retired = true;
        warn!(
            target: "gpu_time",
            "a GPU frame time readback failed to map: its slot is retired, and measuring stops \
             once all {RING} are"
        );
    }
}

/// The time between two timestamps in milliseconds, or `None` when it is not a duration: an end
/// at or before its begin, or a period that is not positive.
fn sample_ms(begin: u64, end: u64, period_ns: f32) -> Option<f64> {
    if end <= begin || period_ns.is_nan() || period_ns <= 0.0 {
        return None;
    }
    Some((end - begin) as f64 * f64::from(period_ns) / 1e6)
}

/// Where a slot is in its frame's measurement.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SlotState {
    Free,
    /// This frame's slot, which the render graph brackets.
    Open,
    /// The frame was submitted with both brackets, the resolve and the copy; the readback is
    /// mapping.
    Pending,
    /// The readback has arrived and can be read.
    Ready,
    /// The readback failed to map; the slot is never used again.
    Retired,
}

/// The slots of the frames being measured.
#[derive(Debug)]
struct Ring {
    slots: [SlotState; RING],
    /// When each slot was begun, in frames begun, so samples that arrive together are published
    /// oldest first and the latest value is the newest frame's.
    begun: [u64; RING],
    frames: u64,
}

impl Default for Ring {
    fn default() -> Self {
        Self {
            slots: [SlotState::Free; RING],
            begun: [0; RING],
            frames: 0,
        }
    }
}

impl Ring {
    /// Opens a free slot for a frame, or `None` when every slot is in use or retired.
    fn begin(&mut self) -> Option<usize> {
        let slot = self.slots.iter().position(|s| *s == SlotState::Free)?;
        self.slots[slot] = SlotState::Open;
        self.begun[slot] = self.frames;
        self.frames += 1;
        Some(slot)
    }

    /// The open slot, if a frame was begun and not yet ended.
    fn open(&self) -> Option<usize> {
        self.slots.iter().position(|s| *s == SlotState::Open)
    }

    /// The frame in `slot` was submitted with both brackets; ignored unless the slot is open.
    fn end(&mut self, slot: usize) {
        if self.slots[slot] == SlotState::Open {
            self.slots[slot] = SlotState::Pending;
        }
    }

    /// Frees every open slot: a frame that was not submitted with both brackets.
    fn abandon_open(&mut self) {
        for slot in &mut self.slots {
            if *slot == SlotState::Open {
                *slot = SlotState::Free;
            }
        }
    }

    /// The readback of `slot` has arrived; ignored unless the slot is pending.
    fn mark_ready(&mut self, slot: usize) {
        if self.slots[slot] == SlotState::Pending {
            self.slots[slot] = SlotState::Ready;
        }
    }

    /// The readback of `slot` failed to map; ignored unless the slot is pending.
    fn retire(&mut self, slot: usize) {
        if self.slots[slot] == SlotState::Pending {
            self.slots[slot] = SlotState::Retired;
        }
    }

    /// Frees the ready slots and returns them, oldest frame first.
    fn take_ready(&mut self) -> impl Iterator<Item = usize> {
        let mut ready = [(0, 0); RING];
        let mut count = 0;
        for (slot, state) in self.slots.iter_mut().enumerate() {
            if *state == SlotState::Ready {
                *state = SlotState::Free;
                ready[count] = (self.begun[slot], slot);
                count += 1;
            }
        }
        ready[..count].sort_unstable();
        ready.into_iter().take(count).map(|(_, slot)| slot)
    }
}

/// The latest sample and its sequence number in one atomic, handed from the render world to the
/// main world, so a value is always read with its own sequence. Both worlds hold a clone.
#[derive(Resource, Clone, Default)]
struct GpuTimeShared(Arc<AtomicU64>);

/// The sequence in the high 32 bits and the milliseconds, as `f32` bits, in the low 32. The
/// sequence is 0 before the first sample and moves by one with each sample, wrapping: a reader
/// knows a new sample by the sequence moving from the one it last saw, never by its value.
fn pack(sequence: u32, ms: f32) -> u64 {
    (u64::from(sequence) << 32) | u64::from(ms.to_bits())
}

fn unpack(packed: u64) -> (u32, f32) {
    ((packed >> 32) as u32, f32::from_bits(packed as u32))
}

impl GpuTimeShared {
    /// Publishes a sample under the next sequence number. The render world is the only writer.
    fn publish(&self, ms: f64) {
        let (sequence, _) = unpack(self.0.load(Ordering::Relaxed));
        self.0
            .store(pack(sequence.wrapping_add(1), ms as f32), Ordering::Release);
    }

    /// The latest sequence number and sample.
    fn latest(&self) -> (u32, f32) {
        unpack(self.0.load(Ordering::Acquire))
    }

    /// The latest sample when the sequence moved since `seen`, which it updates.
    fn take_new(&self, seen: &mut u32) -> Option<f64> {
        let (sequence, ms) = self.latest();
        if sequence == *seen {
            return None;
        }
        *seen = sequence;
        Some(f64::from(ms))
    }
}

/// Moves each pending slot whose readback has finished mapping: a mapped one becomes ready, a
/// failed one is retired. Then publishes the ready slots' samples, oldest first, and frees them.
/// `read` returns a mapped slot's two timestamps and unmaps it; it is never called for a slot
/// whose map failed. Returns how many slots it retired.
fn collect_ready(
    ring: &mut Ring,
    map_state: impl Fn(usize) -> MapState,
    mut read: impl FnMut(usize) -> Option<(u64, u64)>,
    period_ns: f32,
    shared: &GpuTimeShared,
) -> usize {
    let mut retired = 0;
    for slot in 0..RING {
        if ring.slots[slot] != SlotState::Pending {
            continue;
        }
        match map_state(slot) {
            MapState::Waiting => {}
            MapState::Mapped => ring.mark_ready(slot),
            MapState::Failed => {
                ring.retire(slot);
                retired += 1;
            }
        }
    }
    for slot in ring.take_ready() {
        if let Some(ms) = read(slot).and_then(|(begin, end)| sample_ms(begin, end, period_ns)) {
            shared.publish(ms);
        }
    }
    retired
}

/// Main world: registers [`GPU_FRAME_TIME`] and records each new sample in `PreUpdate`.
fn add_publisher(app: &mut App, shared: GpuTimeShared) {
    app.insert_resource(shared)
        .register_diagnostic(Diagnostic::new(GPU_FRAME_TIME).with_suffix("ms"))
        .add_systems(PreUpdate, publish_gpu_frame_time);
}

/// Records the latest sample once per new sequence; samples that arrived since the last frame
/// leave only the newest. `seen` starts where the sequence does, before any sample.
fn publish_gpu_frame_time(
    shared: Res<GpuTimeShared>,
    mut seen: Local<u32>,
    mut diagnostics: Diagnostics,
) {
    if let Some(ms) = shared.take_new(&mut seen) {
        diagnostics.add_measurement(&GPU_FRAME_TIME, || ms);
    }
}

#[cfg(test)]
mod tests {
    use bevy::diagnostic::DiagnosticsStore;

    use super::*;

    fn measurements(app: &App) -> Vec<f64> {
        app.world()
            .resource::<DiagnosticsStore>()
            .get(&GPU_FRAME_TIME)
            .expect("the publisher registers the path")
            .values()
            .copied()
            .collect()
    }

    fn publisher(shared: &GpuTimeShared) -> App {
        let mut app = App::new();
        add_publisher(&mut app, shared.clone());
        app
    }

    /// The ticks a slot reads back: slot `n` measures `n + 1` milliseconds at 1 ns per tick.
    fn ticks(slot: usize) -> Option<(u64, u64)> {
        Some((1_000, 1_000 + (slot as u64 + 1) * 1_000_000))
    }

    /// The slots for which `mapped` holds have their readback mapped; the others wait.
    fn mapped_if(mapped: impl Fn(usize) -> bool) -> impl Fn(usize) -> MapState {
        move |slot| {
            if mapped(slot) {
                MapState::Mapped
            } else {
                MapState::Waiting
            }
        }
    }

    #[test]
    fn a_sample_is_the_time_between_the_ticks_in_milliseconds() {
        assert_eq!(sample_ms(100, 1_100, 1.0), Some(0.001));
    }

    #[test]
    fn equal_ticks_are_no_sample() {
        assert_eq!(sample_ms(5, 5, 1.0), None);
    }

    #[test]
    fn an_end_before_its_begin_is_no_sample() {
        assert_eq!(sample_ms(9, 3, 1.0), None);
    }

    #[test]
    fn a_period_that_is_not_positive_is_no_sample() {
        assert_eq!(sample_ms(1, 2, 0.0), None);
        assert_eq!(sample_ms(1, 2, -1.0), None);
        assert_eq!(sample_ms(1, 2, f32::NAN), None);
    }

    #[test]
    fn a_full_ring_leaves_the_next_frames_unmeasured() {
        let mut ring = Ring::default();
        let mut slots: Vec<usize> = (0..RING)
            .map(|_| {
                let slot = ring.begin().expect("a free slot");
                ring.end(slot);
                slot
            })
            .collect();
        slots.sort();
        assert_eq!(slots, [0, 1, 2, 3]);
        assert_eq!(ring.begin(), None);
        assert_eq!(ring.begin(), None);
        assert_eq!(ring.slots, [SlotState::Pending; RING]);
    }

    #[test]
    fn a_frame_without_an_end_frees_its_slot_unresolved() {
        let mut ring = Ring::default();
        let pending = ring.begin().unwrap();
        ring.end(pending);
        let abandoned = ring.begin().unwrap();
        assert_eq!(ring.open(), Some(abandoned));

        ring.abandon_open();
        assert_eq!(ring.slots[abandoned], SlotState::Free);
        assert_eq!(ring.slots[pending], SlotState::Pending);
        assert_eq!(ring.open(), None);
        // a late end does not pair the freed slot with anything
        ring.end(abandoned);
        assert_eq!(ring.slots[abandoned], SlotState::Free);
        assert_eq!(ring.take_ready().count(), 0);
        // nothing leaked: the three other slots are free to measure
        for _ in 1..RING {
            let slot = ring.begin().expect("a free slot");
            ring.end(slot);
        }
        assert_eq!(ring.begin(), None);
    }

    #[test]
    fn a_slot_is_reused_only_after_its_sample_is_taken() {
        let mut ring = Ring::default();
        for _ in 0..RING {
            let slot = ring.begin().unwrap();
            ring.end(slot);
        }
        ring.mark_ready(2);
        assert_eq!(ring.slots[2], SlotState::Ready);
        assert_eq!(ring.begin(), None);

        assert_eq!(ring.take_ready().collect::<Vec<_>>(), [2]);
        assert_eq!(ring.slots[2], SlotState::Free);
        assert_eq!(ring.begin(), Some(2));
    }

    #[test]
    fn only_a_pending_slot_becomes_ready() {
        let mut ring = Ring::default();
        let open = ring.begin().unwrap();
        ring.mark_ready(open);
        assert_eq!(ring.slots[open], SlotState::Open);
        ring.mark_ready(3);
        assert_eq!(ring.slots[3], SlotState::Free);
    }

    #[test]
    fn publishing_moves_the_sequence_with_the_value() {
        let shared = GpuTimeShared::default();
        assert_eq!(shared.latest(), (0, 0.0));
        shared.publish(2.5);
        assert_eq!(shared.latest(), (1, 2.5));
        shared.publish(4.0);
        assert_eq!(shared.latest(), (2, 4.0));
    }

    #[test]
    fn a_sequence_and_its_value_are_read_together() {
        let shared = GpuTimeShared::default();
        let mut seen = 0;
        assert_eq!(shared.take_new(&mut seen), None);
        shared.publish(2.5);
        assert_eq!(shared.take_new(&mut seen), Some(2.5));
        assert_eq!(seen, 1);
        assert_eq!(shared.take_new(&mut seen), None);
        // every publish is one store of both halves: a value is never paired with another's
        // sequence
        for n in 2..100u32 {
            shared.publish(f64::from(n));
            assert_eq!(shared.latest(), (n, n as f32));
        }
        // the sequence wraps without being mistaken for none
        shared.0.store(pack(u32::MAX, 1.0), Ordering::Release);
        let mut seen = u32::MAX - 1;
        assert_eq!(shared.take_new(&mut seen), Some(1.0));
        shared.publish(2.0);
        assert_eq!(shared.take_new(&mut seen), Some(2.0));
        assert_eq!(seen, 0);
    }

    #[test]
    fn the_publisher_adds_one_measurement_per_new_sequence() {
        let shared = GpuTimeShared::default();
        let mut app = publisher(&shared);
        let diagnostic = app
            .world()
            .resource::<DiagnosticsStore>()
            .get(&GPU_FRAME_TIME)
            .unwrap();
        assert_eq!(diagnostic.suffix, "ms");

        app.update();
        assert_eq!(measurements(&app), [] as [f64; 0]);

        shared.publish(2.5);
        app.update();
        assert_eq!(measurements(&app), [2.5]);
        app.update();
        assert_eq!(measurements(&app), [2.5]);

        // two samples between updates: the main world records the newer
        shared.publish(4.0);
        shared.publish(5.0);
        app.update();
        assert_eq!(measurements(&app), [2.5, 5.0]);
    }

    #[test]
    fn a_frame_is_published_only_once_its_readback_has_arrived() {
        let shared = GpuTimeShared::default();
        let mut app = publisher(&shared);
        let mut ring = Ring::default();

        // frame N is submitted; its readback has not arrived
        let frame_n = ring.begin().unwrap();
        ring.end(frame_n);
        collect_ready(&mut ring, |_| MapState::Waiting, ticks, 1.0, &shared);
        app.update();
        assert_eq!(measurements(&app), [] as [f64; 0]);
        assert_eq!(ring.slots[frame_n], SlotState::Pending);

        // frame N + 1 is submitted, then frame N's readback arrives
        let frame_n1 = ring.begin().unwrap();
        ring.end(frame_n1);
        collect_ready(
            &mut ring,
            mapped_if(|slot| slot == frame_n),
            ticks,
            1.0,
            &shared,
        );
        app.update();
        assert_eq!(measurements(&app), [frame_n as f64 + 1.0]);
        assert_eq!(ring.slots[frame_n], SlotState::Free);
        assert_eq!(ring.slots[frame_n1], SlotState::Pending);
    }

    #[test]
    fn samples_that_arrive_together_leave_the_newest_frame_last() {
        let shared = GpuTimeShared::default();
        let mut ring = Ring::default();
        // slot 0 is freed and begun again, so it holds a newer frame than slot 1
        let first = ring.begin().unwrap();
        ring.end(first);
        let older = ring.begin().unwrap();
        ring.end(older);
        collect_ready(
            &mut ring,
            mapped_if(|slot| slot == first),
            ticks,
            1.0,
            &shared,
        );
        let newer = ring.begin().unwrap();
        ring.end(newer);
        assert_eq!((first, older, newer), (0, 1, 0));

        let (before, _) = shared.latest();
        collect_ready(&mut ring, |_| MapState::Mapped, ticks, 1.0, &shared);
        assert_eq!(shared.latest(), (before + 2, newer as f32 + 1.0));
    }

    #[test]
    fn a_failed_readback_retires_its_slot_for_good() {
        let shared = GpuTimeShared::default();
        let mut ring = Ring::default();
        let failed = ring.begin().unwrap();
        ring.end(failed);
        let never_read = |_: usize| -> Option<(u64, u64)> { panic!("a failed readback is read") };
        let retired = collect_ready(&mut ring, |_| MapState::Failed, never_read, 1.0, &shared);
        assert_eq!(retired, 1);
        assert_eq!(ring.slots[failed], SlotState::Retired);
        assert_eq!(shared.latest(), (0, 0.0));

        // the other slots keep measuring, and the retired one is never handed out again
        for _ in 0..3 * RING {
            ring.abandon_open();
            let slot = ring.begin().expect("a free slot");
            assert_ne!(slot, failed);
            ring.end(slot);
            assert_eq!(
                collect_ready(&mut ring, |_| MapState::Mapped, ticks, 1.0, &shared),
                0
            );
        }
        assert_eq!(ring.slots[failed], SlotState::Retired);
        assert_eq!(shared.latest().0, 3 * RING as u32);
    }

    #[test]
    fn with_every_slot_retired_measuring_stops() {
        let shared = GpuTimeShared::default();
        let mut ring = Ring::default();
        let mut retired = 0;
        while let Some(slot) = ring.begin() {
            ring.end(slot);
            retired += collect_ready(&mut ring, |_| MapState::Failed, |_| None, 1.0, &shared);
        }
        assert_eq!(retired, RING);
        assert_eq!(ring.slots, [SlotState::Retired; RING]);
        for _ in 0..10 {
            ring.abandon_open();
            assert_eq!(ring.begin(), None);
            assert_eq!(ring.open(), None);
            let never_read = |_: usize| -> Option<(u64, u64)> { panic!("a retired slot is read") };
            assert_eq!(
                collect_ready(&mut ring, |_| MapState::Mapped, never_read, 1.0, &shared),
                0
            );
        }
        assert_eq!(shared.latest(), (0, 0.0));
    }

    #[test]
    fn a_dropped_sample_publishes_nothing() {
        let shared = GpuTimeShared::default();
        let mut ring = Ring::default();
        let slot = ring.begin().unwrap();
        ring.end(slot);
        collect_ready(
            &mut ring,
            |_| MapState::Mapped,
            |_| Some((9, 3)),
            1.0,
            &shared,
        );
        assert_eq!(ring.slots[slot], SlotState::Free);
        assert_eq!(shared.latest(), (0, 0.0));
    }

    #[derive(Debug, Hash, PartialEq, Eq, Clone, RenderLabel)]
    enum TestNode {
        Camera,
        Compute,
        Late,
    }

    struct Empty;

    impl Node for Empty {
        fn run<'w>(
            &self,
            _: &mut RenderGraphContext,
            _: &mut RenderContext<'w>,
            _: &'w World,
        ) -> Result<(), NodeRunError> {
            Ok(())
        }
    }

    #[test]
    fn the_brackets_enclose_every_node_present_when_they_are_added() {
        let mut graph = RenderGraph::default();
        graph.add_node(TestNode::Compute, Empty);
        graph.add_node(TestNode::Camera, Empty);
        graph.add_node_edge(TestNode::Compute, TestNode::Camera);
        add_brackets(&mut graph);
        assert_eq!(unbracketed(&graph), []);
    }

    #[test]
    fn a_node_added_after_the_brackets_is_named() {
        let mut graph = RenderGraph::default();
        graph.add_node(TestNode::Camera, Empty);
        add_brackets(&mut graph);
        graph.add_node(TestNode::Late, Empty);
        assert_eq!(unbracketed(&graph), [TestNode::Late.intern()]);
        // ordered before the camera only: still not after the first bracket
        graph.add_node_edge(TestNode::Late, TestNode::Camera);
        assert_eq!(unbracketed(&graph), [TestNode::Late.intern()]);
        // after the first bracket too: enclosed
        graph.add_node_edge(BeginLabel, TestNode::Late);
        assert_eq!(unbracketed(&graph), []);
    }
}

//! Performance output of the explorer.
//!
//! - The GPU frame time ([`gpu`]): measured in every build of the rendering clients, native and
//!   web, where the adapter supports timestamp queries, and recorded as
//!   `common::gpu_time::GPU_FRAME_TIME`, which the system info panel and the benchmark CSV read.
//! - The benchmark autopilot's output: the per-frame CSV and the log, in the formats the
//!   benchmark's report reads (`benchmark report`, `local-harness/benchmark`). See
//!   `common::autopilot` for the run's phases. Each file has one writer and two sinks: the
//!   desktop's file (`--csv`, `--log-file`) and the web's stream to the page, which sends it to
//!   the benchmark runner (`perfStream`, `perfCsv`; [`stream`]).

pub mod csv;
pub mod format;
pub mod gpu;
pub mod layer;
pub mod scrub;
pub mod sink;
pub mod stream;

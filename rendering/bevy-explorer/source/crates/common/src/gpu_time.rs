//! The GPU time of a frame, the one path every consumer reads: the benchmark CSV and the system
//! info panel look it up in the [`DiagnosticsStore`](bevy::diagnostic::DiagnosticsStore).
//!
//! The measurement registers the diagnostic and writes it only on an adapter that supports
//! timestamp queries. Where it is absent there is no GPU time: a consumer shows none (the panel a
//! dash, the CSV 0) and never an estimate.
//!
//! Where the measurement is known to misread, the embedder inserts [`GpuFrameTimeUnreliable`].
//! The readings are still recorded, for whoever looks at them knowing that; what is derived from
//! them without a person reading it leaves them out: the performance sample (and so the analytics
//! event built from it), the benchmark summary and the frame's bottleneck.

use bevy::{diagnostic::DiagnosticPath, prelude::Resource};

/// The GPU time of a frame in milliseconds, one measurement per measured frame, the same on every
/// platform. A measurement is recorded once the GPU has finished the frame it measures and its
/// readback has been read: three frames after that frame when the GPU finishes each frame before
/// the next one is submitted, later when it does not.
pub const GPU_FRAME_TIME: DiagnosticPath = DiagnosticPath::const_new("gpu/frame_time");

/// Present where [`GPU_FRAME_TIME`] is known to misread: in a browser on macOS (WebGPU on Metal)
/// a run can sit on one near-constant value or read a fraction of a millisecond for a frame that
/// takes several. The diagnostic, the system info panel, the benchmark CSV and, in
/// `/performance_state`, `gpuFrameMs` and the GPU element of `frameTimeBreakdown` keep the
/// readings; the performance sample, the benchmark summary and the frame's bottleneck take none.
#[derive(Resource, Clone, Copy, Debug, Default)]
pub struct GpuFrameTimeUnreliable;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_path_is_gpu_frame_time() {
        assert_eq!(GPU_FRAME_TIME.as_str(), "gpu/frame_time");
    }
}

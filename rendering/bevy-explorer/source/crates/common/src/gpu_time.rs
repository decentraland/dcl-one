//! The GPU time of a frame, the one path every consumer reads: the benchmark CSV and the system
//! info panel look it up in the [`DiagnosticsStore`](bevy::diagnostic::DiagnosticsStore).
//!
//! The measurement registers the diagnostic and writes it only on an adapter that supports
//! timestamp queries. Where it is absent there is no GPU time: a consumer shows none (the panel a
//! dash, the CSV 0) and never an estimate.

use bevy::diagnostic::DiagnosticPath;

/// The GPU time of a frame in milliseconds, one measurement per measured frame, the same on every
/// platform. A measurement is recorded once the GPU has finished the frame it measures and its
/// readback has been read: three frames after that frame when the GPU finishes each frame before
/// the next one is submitted, later when it does not.
pub const GPU_FRAME_TIME: DiagnosticPath = DiagnosticPath::const_new("gpu/frame_time");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_path_is_gpu_frame_time() {
        assert_eq!(GPU_FRAME_TIME.as_str(), "gpu/frame_time");
    }
}

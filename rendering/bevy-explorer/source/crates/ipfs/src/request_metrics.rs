use std::time::Duration;

use crate::{batch_sizes::BatchSizes, IpfsIo};

/// Cumulative measurements for this content client, including requests before the first read.
#[derive(Clone, Copy, Default)]
pub(super) struct RequestMetrics {
    pub(super) scene_batches: BatchSizes,
    pub(super) attachment_batches: BatchSizes,
    pub(super) bytes: u64,
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

impl RequestMetrics {
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

    pub(super) fn mean_first_byte_ms(&self) -> Option<f64> {
        (self.first_bytes > 0).then(|| 1000.0 * self.first_byte_seconds / self.first_bytes as f64)
    }

    pub(super) fn mean_small_ms(&self) -> Option<f64> {
        (self.small > 0).then(|| 1000.0 * self.small_seconds / self.small as f64)
    }

    pub(super) fn mean_ms_per_mib(&self) -> Option<f64> {
        (self.large > 0).then(|| 1000.0 * self.seconds_per_mib / self.large as f64)
    }

    pub(super) fn bytes_per_second(&self) -> Option<f64> {
        (self.large_seconds > 0.0).then(|| self.large_bytes as f64 / self.large_seconds)
    }
}

impl IpfsIo {
    pub(super) fn request_metrics(&self) -> RequestMetrics {
        *self.request_metrics.lock().unwrap()
    }

    /// Definitions returned in one completed scene lookup (pointers or a World's entity URNs).
    pub fn record_scene_batch(&self, size: usize) {
        self.request_metrics
            .lock()
            .unwrap()
            .scene_batches
            .record(size);
    }

    /// Definitions returned in one wearable/emote pointer resolution batch.
    pub fn record_attachment_batch(&self, size: usize) {
        self.request_metrics
            .lock()
            .unwrap()
            .attachment_batches
            .record(size);
    }
}

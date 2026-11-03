//! Data-channel payload counts for the last completed second. Each connection starts fresh.

use web_time::Instant;

#[derive(Clone, Copy, Default)]
pub(crate) struct PayloadCount {
    pub bytes: u64,
    pub packets: u64,
}

pub(crate) struct PayloadRate {
    started: Instant,
    buckets: [(u64, PayloadCount); 2],
}

impl Default for PayloadRate {
    fn default() -> Self {
        Self {
            started: Instant::now(),
            buckets: Default::default(),
        }
    }
}

impl PayloadRate {
    pub fn record(&mut self, bytes: usize) {
        let second = self.started.elapsed().as_secs();
        let bucket = &mut self.buckets[(second % 2) as usize];
        if bucket.0 != second {
            *bucket = (second, PayloadCount::default());
        }
        bucket.1.bytes += bytes as u64;
        bucket.1.packets += 1;
    }

    pub fn last_second(&self) -> PayloadCount {
        let Some(second) = self.started.elapsed().as_secs().checked_sub(1) else {
            return PayloadCount::default();
        };
        let (stamp, count) = self.buckets[(second % 2) as usize];
        if stamp == second {
            count
        } else {
            PayloadCount::default()
        }
    }
}

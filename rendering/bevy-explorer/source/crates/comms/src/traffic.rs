//! Data-channel payloads in the last completed second of this connection.

use web_time::Instant;

#[derive(Clone, Copy, Default)]
pub(crate) struct PayloadCount {
    pub bytes: u64,
    pub packets: u64,
}

impl PayloadCount {
    fn record(&mut self, bytes: usize) {
        self.bytes += bytes as u64;
        self.packets += 1;
    }
}

#[derive(Clone, Copy, Default)]
pub(crate) struct PayloadCounts {
    pub sent: PayloadCount,
    pub received: PayloadCount,
}

pub(crate) struct PayloadTraffic {
    started: Instant,
    buckets: [(u64, PayloadCounts); 2],
}

impl Default for PayloadTraffic {
    fn default() -> Self {
        Self {
            started: Instant::now(),
            buckets: Default::default(),
        }
    }
}

impl PayloadTraffic {
    pub fn record_sent(&mut self, bytes: usize) {
        self.bucket(self.started.elapsed().as_secs())
            .sent
            .record(bytes);
    }

    pub fn record_received(&mut self, bytes: usize) {
        self.bucket(self.started.elapsed().as_secs())
            .received
            .record(bytes);
    }

    pub fn last_second(&self) -> PayloadCounts {
        self.completed_at(self.started.elapsed().as_secs())
    }

    fn bucket(&mut self, second: u64) -> &mut PayloadCounts {
        let bucket = &mut self.buckets[(second % 2) as usize];
        if bucket.0 != second {
            *bucket = (second, PayloadCounts::default());
        }
        &mut bucket.1
    }

    fn completed_at(&self, now: u64) -> PayloadCounts {
        let Some(second) = now.checked_sub(1) else {
            return PayloadCounts::default();
        };
        let (stamp, counts) = self.buckets[(second % 2) as usize];
        if stamp == second {
            counts
        } else {
            PayloadCounts::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_read_counts_completed_payloads_then_expires_while_idle() {
        let mut traffic = PayloadTraffic::default();
        traffic.bucket(4).sent.record(100);
        traffic.bucket(4).sent.record(70);
        traffic.bucket(4).received.record(30);
        traffic.bucket(5).sent.record(1);

        let first = traffic.completed_at(5);
        assert_eq!((first.sent.bytes, first.sent.packets), (170, 2));
        assert_eq!((first.received.bytes, first.received.packets), (30, 1));
        assert_eq!(traffic.completed_at(6).sent.bytes, 1);
        let idle = traffic.completed_at(7);
        assert_eq!((idle.sent.packets, idle.received.packets), (0, 0));
    }

    #[test]
    fn a_new_connection_starts_with_empty_windows() {
        let mut old = PayloadTraffic::default();
        old.bucket(0).received.record(50);
        let new = PayloadTraffic::default();

        assert_eq!(old.completed_at(1).received.bytes, 50);
        assert_eq!(new.completed_at(0).received.packets, 0);
        assert_eq!(new.completed_at(1).received.packets, 0);
    }
}

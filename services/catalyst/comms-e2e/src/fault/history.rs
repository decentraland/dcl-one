//! What a link saw: every packet that reached it, when, how big, and what the fault did with it.
//! Each packet is noted as it arrives, before the fault acts, so the history of a direction is
//! what its sender sent, whatever the link then did to it.
//!
//! A history tells packets apart by size and time only, never by content. [`cadence`] reads the
//! packets of at least a given size as a sender's data and measures the gaps between them, and
//! from the start of each span to its first packet and from its last packet to the span's end: a
//! sender that sends on a regular beat and stops for a while, or for good, shows a gap longer than
//! its beat.

use std::time::Duration;

use serde::Serialize;

/// What the fault did with a packet when it arrived.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum Fate {
    /// Sent on at once.
    Passed,
    /// Sent on after a wait.
    Delayed,
    /// Never sent on.
    Lost,
    /// Kept by a queue, to be sent on when it ends.
    Queued,
}

/// One packet that reached the link.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Seen {
    /// When it arrived, from the moment the link started.
    pub at: Duration,
    pub bytes: usize,
    pub fate: Fate,
}

/// Packets a history keeps per direction; later ones are counted, not kept.
pub(crate) const HISTORY_LIMIT: usize = 500_000;

/// The packets of each direction, in time order whichever task noted them.
#[derive(Default)]
pub(crate) struct History {
    pub up: Vec<Seen>,
    pub down: Vec<Seen>,
    /// Packets that arrived after a direction's history was full.
    pub missed: u64,
}

impl History {
    pub fn push(&mut self, uplink: bool, seen: Seen) {
        let list = if uplink { &mut self.up } else { &mut self.down };
        if list.len() < HISTORY_LIMIT {
            // A task that read the clock before another may push after it: such a packet goes
            // in its place, which is near the end.
            let place = list.len()
                - list
                    .iter()
                    .rev()
                    .take_while(|kept| kept.at > seen.at)
                    .count();
            list.insert(place, seen);
        } else {
            self.missed += 1;
        }
    }
}

/// The packets that arrived in one window of time.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Window {
    pub start: Duration,
    pub packets: u32,
    pub bytes: u64,
    /// Packets of at least the size the caller named.
    pub large: u32,
}

/// The windows of `width` from `from` up to `to`, each with what arrived in it. The last window
/// may reach past `to`; it counts only what arrived before `to`.
pub fn windows(
    seen: &[Seen],
    from: Duration,
    to: Duration,
    width: Duration,
    large_from: usize,
) -> Vec<Window> {
    if width.is_zero() || to <= from {
        return Vec::new();
    }
    let count = (to - from).as_nanos().div_ceil(width.as_nanos()) as usize;
    let mut out: Vec<Window> = (0..count)
        .map(|index| Window {
            start: from + width * index as u32,
            packets: 0,
            bytes: 0,
            large: 0,
        })
        .collect();
    for packet in seen
        .iter()
        .filter(|packet| packet.at >= from && packet.at < to)
    {
        let index = ((packet.at - from).as_nanos() / width.as_nanos()) as usize;
        let window = &mut out[index];
        window.packets += 1;
        window.bytes += packet.bytes as u64;
        if packet.bytes >= large_from {
            window.large += 1;
        }
    }
    out
}

/// One gap inside a span: between two consecutive large packets, from the span's start to its
/// first large packet, or from its last large packet (or the start of a span with none) to the
/// span's end.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Gap {
    /// When the packet before the gap arrived, or the span's start.
    pub start: Duration,
    pub length: Duration,
    /// Which of the spans it lies in.
    pub span: usize,
}

/// The beat of the large packets inside some spans of time.
#[derive(Debug, Clone, Serialize)]
pub struct Cadence {
    /// Large packets inside the spans.
    pub large: usize,
    /// The spans' length together.
    pub span: Duration,
    /// The median gap between two consecutive large packets: the sender's regular beat. The gaps
    /// at the edges of a span are left out of it.
    pub median_gap: Duration,
    pub longest_gap: Duration,
    /// Every gap of each span, in order: from the span's start to its first large packet, between
    /// consecutive ones, and from the last one to the span's end; a span with none is one gap.
    /// Gaps at an edge that last nothing are left out. A gap across two spans is not one.
    pub gaps: Vec<Gap>,
}

/// The gaps of the packets of at least `large_from` bytes inside each of `spans` (start, end),
/// which must not overlap. `seen` need not be in time order.
pub fn cadence(seen: &[Seen], spans: &[(Duration, Duration)], large_from: usize) -> Cadence {
    let mut gaps = Vec::new();
    let mut lengths = Vec::new();
    let mut large = 0;
    let mut span = Duration::ZERO;
    for (index, &(from, to)) in spans.iter().enumerate() {
        if to <= from {
            continue;
        }
        span += to - from;
        let mut inside: Vec<Duration> = seen
            .iter()
            .filter(|packet| packet.bytes >= large_from && packet.at >= from && packet.at < to)
            .map(|packet| packet.at)
            .collect();
        inside.sort();
        large += inside.len();
        let gap = |start: Duration, end: Duration| Gap {
            start,
            length: end.saturating_sub(start),
            span: index,
        };
        let first = inside.first().copied().unwrap_or(to);
        if first > from {
            gaps.push(gap(from, first));
        }
        for pair in inside.windows(2) {
            gaps.push(gap(pair[0], pair[1]));
            lengths.push(pair[1].saturating_sub(pair[0]));
        }
        if let Some(&last) = inside.last()
            && to > last
        {
            gaps.push(gap(last, to));
        }
    }
    lengths.sort();
    Cadence {
        large,
        span,
        median_gap: lengths.get(lengths.len() / 2).copied().unwrap_or_default(),
        longest_gap: gaps.iter().map(|gap| gap.length).max().unwrap_or_default(),
        gaps,
    }
}

/// The gaps of a [`Cadence`] longer than a threshold: times the sender stopped sending.
#[derive(Debug, Clone, Serialize)]
pub struct Silences {
    /// The threshold they are longer than.
    pub longer_than: Duration,
    /// Each one, in order.
    pub gaps: Vec<Gap>,
    /// What they last beyond the regular beat, together.
    pub silent: Duration,
    /// `silent` as a share of the spans' length.
    pub share: f64,
    pub longest: Duration,
    /// The median time from the start of one to the start of the next in the same span.
    pub period: Option<Duration>,
}

impl Cadence {
    /// The threshold a gap must pass to count as a silence: one and a half regular beats, so a
    /// beat missed is one, and never less than `floor`. A sender that sends on a slower beat
    /// throughout has no silences.
    pub fn silence_threshold(&self, floor: Duration) -> Duration {
        (self.median_gap * 3 / 2).max(floor)
    }

    /// What all the gaps last beyond the regular beat, as a share of the spans' length: the
    /// time the sender was later than its beat, silences or not.
    pub fn beyond_beat_share(&self) -> f64 {
        if self.span.is_zero() {
            return 0.0;
        }
        let late: Duration = self
            .gaps
            .iter()
            .map(|gap| gap.length.saturating_sub(self.median_gap))
            .sum();
        late.as_secs_f64() / self.span.as_secs_f64()
    }

    /// The gaps longer than `longer_than`.
    pub fn silences(&self, longer_than: Duration) -> Silences {
        let gaps: Vec<Gap> = self
            .gaps
            .iter()
            .filter(|gap| gap.length > longer_than)
            .copied()
            .collect();
        let silent: Duration = gaps
            .iter()
            .map(|gap| gap.length.saturating_sub(self.median_gap))
            .sum();
        let mut periods: Vec<Duration> = gaps
            .windows(2)
            .filter(|pair| pair[0].span == pair[1].span)
            .map(|pair| pair[1].start.saturating_sub(pair[0].start))
            .collect();
        periods.sort();
        Silences {
            longer_than,
            share: if self.span.is_zero() {
                0.0
            } else {
                silent.as_secs_f64() / self.span.as_secs_f64()
            },
            longest: gaps.iter().map(|gap| gap.length).max().unwrap_or_default(),
            period: periods.get(periods.len() / 2).copied(),
            silent,
            gaps,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ms(value: u64) -> Duration {
        Duration::from_millis(value)
    }

    fn packet(at: u64, bytes: usize) -> Seen {
        Seen {
            at: ms(at),
            bytes,
            fate: Fate::Passed,
        }
    }

    /// Data of 80 bytes every `beat` ms from 0 up to `until`, except inside the `quiet` spans,
    /// and a small packet of 12 bytes every 50 ms throughout.
    fn sender(beat: u64, until: u64, quiet: &[(u64, u64)]) -> Vec<Seen> {
        let mut seen = Vec::new();
        let mut at = 0;
        while at < until {
            if at % 50 == 0 {
                seen.push(packet(at, 12));
            }
            if at % beat == 0 && !quiet.iter().any(|&(from, to)| at >= from && at < to) {
                seen.push(packet(at, 80));
            }
            at += 10;
        }
        seen
    }

    #[test]
    fn windows_count_every_packet_and_the_large_ones() {
        let seen = sender(100, 1000, &[]);
        let windows = windows(&seen, ms(0), ms(1000), ms(200), 40);
        assert_eq!(windows.len(), 5);
        for window in &windows {
            assert_eq!(window.large, 2, "{window:?}");
            assert_eq!(window.packets, 6, "{window:?}");
            assert_eq!(window.bytes, 2 * 80 + 4 * 12);
        }
        assert_eq!(windows[3].start, ms(600));
        // a span that does not divide by the width: the last window counts up to the end only
        let windows = super::windows(&seen, ms(100), ms(350), ms(200), 40);
        assert_eq!(windows.len(), 2);
        assert_eq!((windows[0].large, windows[1].large), (2, 1));
        assert!(super::windows(&seen, ms(10), ms(10), ms(200), 40).is_empty());
    }

    #[test]
    fn a_steady_sender_has_its_beat_and_no_silence() {
        let seen = sender(100, 10_000, &[]);
        let cadence = cadence(&seen, &[(ms(0), ms(10_000))], 40);
        assert_eq!(cadence.large, 100);
        assert_eq!(cadence.median_gap, ms(100));
        assert_eq!(cadence.longest_gap, ms(100));
        let silences = cadence.silences(cadence.silence_threshold(ms(150)));
        assert!(silences.gaps.is_empty());
        assert_eq!(silences.share, 0.0);
        assert_eq!(silences.period, None);
        assert_eq!(cadence.beyond_beat_share(), 0.0);
        // one beat missed is a silence
        let missed = sender(100, 2000, &[(1000, 1100)]);
        let cadence = super::cadence(&missed, &[(ms(0), ms(2000))], 40);
        let silences = cadence.silences(cadence.silence_threshold(ms(120)));
        assert_eq!(silences.gaps.len(), 1);
        assert_eq!(silences.longest, ms(200));
    }

    #[test]
    fn a_sender_that_stops_on_a_cycle_shows_each_stop_its_length_and_period() {
        // sends for 600 ms of every 800 and stops for the rest, as a link that stops it would
        let quiet: Vec<(u64, u64)> = (0..10)
            .map(|cycle| (cycle * 800 + 600, cycle * 800 + 800))
            .collect();
        let seen = sender(100, 8000, &quiet);
        let cadence = cadence(&seen, &[(ms(0), ms(8000))], 40);
        assert_eq!(cadence.median_gap, ms(100));
        let silences = cadence.silences(cadence.silence_threshold(ms(150)));
        // the last stop runs to the end of the span, from the last packet: it counts as well
        assert_eq!(silences.gaps.len(), 10);
        for gap in &silences.gaps {
            assert_eq!(gap.length, ms(300), "{gap:?}");
        }
        assert_eq!(silences.longest, ms(300));
        assert_eq!(silences.period, Some(ms(800)));
        // beyond the beat each stop lasts 200 ms: 2 s of 8 s
        assert_eq!(silences.silent, ms(2000));
        assert!((silences.share - 0.25).abs() < 1e-9, "{}", silences.share);
        assert!((cadence.beyond_beat_share() - 0.25).abs() < 1e-9);
    }

    #[test]
    fn a_sender_that_stops_for_the_rest_of_a_span_shows_the_stop() {
        // sends until 6 s of a 10 s span, then nothing
        let seen = sender(100, 6000, &[]);
        let cadence = cadence(&seen, &[(ms(0), ms(10_000))], 40);
        assert_eq!(cadence.median_gap, ms(100));
        let silences = cadence.silences(cadence.silence_threshold(ms(150)));
        assert_eq!(silences.gaps.len(), 1, "{silences:?}");
        assert_eq!(silences.gaps[0].start, ms(5900));
        assert_eq!(silences.longest, ms(4100));
    }

    #[test]
    fn a_sender_that_starts_late_in_a_span_shows_the_wait() {
        let seen = sender(100, 10_000, &[(0, 3000)]);
        let cadence = cadence(&seen, &[(ms(0), ms(10_000))], 40);
        let silences = cadence.silences(cadence.silence_threshold(ms(150)));
        assert_eq!(silences.gaps.len(), 1, "{silences:?}");
        assert_eq!(
            (silences.gaps[0].start, silences.longest),
            (ms(0), ms(3000))
        );
    }

    #[test]
    fn a_span_with_no_packet_is_one_gap_as_long_as_the_span() {
        let seen = sender(100, 10_000, &[(2000, 6000)]);
        let cadence = cadence(&seen, &[(ms(0), ms(1000)), (ms(3000), ms(5000))], 40);
        assert_eq!(cadence.large, 10);
        let silences = cadence.silences(cadence.silence_threshold(ms(150)));
        assert_eq!(silences.gaps.len(), 1, "{silences:?}");
        assert_eq!(
            (
                silences.gaps[0].start,
                silences.gaps[0].length,
                silences.gaps[0].span
            ),
            (ms(3000), ms(2000), 1)
        );
        // and a sender that sends nothing at all is silent throughout
        let quiet = sender(100, 10_000, &[(0, 10_000)]);
        let cadence = super::cadence(&quiet, &[(ms(0), ms(4000)), (ms(5000), ms(9000))], 40);
        let silences = cadence.silences(cadence.silence_threshold(ms(150)));
        assert_eq!(silences.gaps.len(), 2);
        assert_eq!(silences.silent, ms(8000));
    }

    #[test]
    fn the_history_is_in_time_order_whatever_order_it_was_pushed_in() {
        let mut history = History::default();
        for at in [10, 30, 20, 40, 5] {
            history.push(false, packet(at, 10));
        }
        let order: Vec<Duration> = history.down.iter().map(|seen| seen.at).collect();
        assert_eq!(order, [ms(5), ms(10), ms(20), ms(30), ms(40)]);
    }

    #[test]
    fn cadence_reads_packets_that_are_out_of_time_order() {
        let mut seen: Vec<Seen> = (0..20).map(|index| packet(index * 100, 80)).collect();
        seen.swap(3, 10);
        let cadence = cadence(&seen, &[(ms(0), ms(2000))], 40);
        assert_eq!(cadence.median_gap, ms(100));
        assert_eq!(cadence.longest_gap, ms(100));
    }

    #[test]
    fn a_slower_beat_throughout_is_not_a_silence() {
        let seen = sender(250, 10_000, &[]);
        let cadence = cadence(&seen, &[(ms(0), ms(10_000))], 40);
        assert_eq!(cadence.median_gap, ms(250));
        assert!(
            cadence
                .silences(cadence.silence_threshold(ms(180)))
                .gaps
                .is_empty()
        );
    }

    #[test]
    fn gaps_are_measured_inside_spans_only() {
        let seen = sender(100, 4000, &[(1000, 3000)]);
        // two spans that leave the long stop out
        let cadence = cadence(&seen, &[(ms(0), ms(1000)), (ms(3000), ms(4000))], 40);
        assert_eq!(cadence.large, 20);
        assert_eq!(cadence.span, ms(2000));
        assert_eq!(cadence.longest_gap, ms(100));
        assert!(cadence.gaps.iter().all(|gap| gap.length == ms(100)));
        // the same packets in one span: the stop is there
        let whole = super::cadence(&seen, &[(ms(0), ms(4000))], 40);
        assert_eq!(whole.longest_gap, ms(2100));
        assert_eq!(whole.median_gap, ms(100));
    }

    #[test]
    fn a_full_history_counts_what_it_does_not_keep() {
        let mut history = History::default();
        for index in 0..HISTORY_LIMIT + 3 {
            history.push(true, packet(index as u64, 10));
        }
        history.push(false, packet(0, 10));
        assert_eq!(history.up.len(), HISTORY_LIMIT);
        assert_eq!(history.down.len(), 1);
        assert_eq!(history.missed, 3);
    }
}

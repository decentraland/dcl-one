use super::*;
use crate::pulse::transport::{FrameKind, PulseReliability};

/// A frame of `kind` that the tests tell apart by its number.
fn frame(kind: FrameKind, number: u32) -> PulseFrame {
    PulseFrame {
        bytes: number.to_be_bytes().to_vec(),
        reliability: PulseReliability::Reliable,
        kind,
    }
}

fn teleport(number: u32) -> PulseFrame {
    frame(FrameKind::Teleport, number)
}

fn emote(number: u32) -> PulseFrame {
    frame(FrameKind::Emote, number)
}

/// The number a frame was made with.
fn number(frame: &PulseFrame) -> u32 {
    u32::from_be_bytes(frame.bytes.as_slice().try_into().unwrap())
}

/// The numbers of the released frames, in order.
fn numbers(frames: Vec<PulseFrame>) -> Vec<u32> {
    frames.iter().map(number).collect()
}

/// Releases at `now`, noting when each frame went out.
fn release_noting(pacer: &mut DiscreteEventPacer, now: f64, sent: &mut Vec<(f64, u32)>) {
    sent.extend(pacer.release(now).iter().map(|frame| (now, number(frame))));
}

/// Spends every token of a full bucket at `now`: each offer goes out at once.
fn empty_the_bucket(pacer: &mut DiscreteEventPacer, now: f64) {
    for spent in 0..PACED_EVENTS_BURST as u32 {
        pacer.offer(teleport(1000 + spent));
        assert_eq!(numbers(pacer.release(now)), [1000 + spent]);
    }
}

/// A peer's discrete-event bucket on the servers, set to 20 a second and a burst of 16
/// (`DiscreteEvent` in Pulse `src/DCLPulse/appsettings.json:44-47`). Both implement it alike: Pulse
/// `TokenBucketRateLimiter.cs`, dcl-one `GameplayRateLimiter::charge` in
/// `services/catalyst/comms/src/pulse/hardening.rs`. The first event finds it full, a whole token
/// returns every 50 ms, and an event that finds it empty is a terminal disconnect.
#[derive(Default)]
struct ServerBucket {
    tokens: u64,
    /// The server's millisecond clock when its refills last counted; `None` before the first event.
    refilled_at: Option<u64>,
}

impl ServerBucket {
    const BURST: u64 = 16;
    const REFILL_MS: u64 = 50;

    /// Whether the server takes an event arriving at `at` seconds, and takes its token if so.
    fn takes(&mut self, at: f64) -> bool {
        let now = (at * 1000.0) as u64;
        self.refilled_at = Some(match self.refilled_at {
            None => {
                self.tokens = Self::BURST;
                now
            }
            Some(last) => {
                // Whole tokens only: the part of an interval that has not passed yet is kept.
                let refills = now.saturating_sub(last) / Self::REFILL_MS;
                self.tokens = (self.tokens + refills).min(Self::BURST);
                last + refills * Self::REFILL_MS
            }
        });
        match self.tokens.checked_sub(1) {
            Some(left) => {
                self.tokens = left;
                true
            }
            None => false,
        }
    }
}

/// What a scene does with a durationless `movePlayerTo` every frame: a teleport 60 times a second,
/// which empties the servers' bucket in under a second.
#[test]
fn a_teleport_every_frame_stays_within_the_servers_budget() {
    const FRAMES: u32 = 600;
    let time = |frame: u32| f64::from(frame) / 60.0;
    let mut pacer = DiscreteEventPacer::default();
    let mut sent = Vec::new();

    for n in 0..FRAMES {
        pacer.offer(teleport(n));
        release_noting(&mut pacer, time(n), &mut sent);
    }
    // The offers stop, and what waits goes on going out.
    for n in FRAMES..FRAMES + 60 {
        release_noting(&mut pacer, time(n), &mut sent);
    }

    let mut server = ServerBucket::default();
    for &(at, number) in &sent {
        assert!(
            server.takes(at),
            "the servers' bucket ran dry at {at:.2} s, on teleport {number}"
        );
    }
    assert_eq!(sent.last().map(|&(_, number)| number), Some(FRAMES - 1));
}

#[test]
fn a_waiting_teleport_is_replaced_by_a_newer_one() {
    let mut pacer = DiscreteEventPacer::default();
    empty_the_bucket(&mut pacer, 0.0);

    pacer.offer(teleport(1));
    pacer.offer(teleport(2));

    // Nothing goes while the bucket is empty; once it has refilled, only the newer one is left.
    assert!(pacer.release(0.0).is_empty());
    assert_eq!(numbers(pacer.release(1.0)), [2]);
    assert!(pacer.release(2.0).is_empty());
}

#[test]
fn waiting_frames_go_out_in_the_order_of_their_latest_offer() {
    let mut pacer = DiscreteEventPacer::default();
    empty_the_bucket(&mut pacer, 0.0);

    // The second teleport takes the first one's place, behind the emote offered in between.
    pacer.offer(teleport(1));
    pacer.offer(emote(2));
    pacer.offer(teleport(3));

    // A token returns every tenth of a second, and each lets one frame go.
    assert_eq!(numbers(pacer.release(0.11)), [2]);
    assert_eq!(numbers(pacer.release(0.22)), [3]);
    assert!(pacer.release(1.0).is_empty());
}

/// Dropping the teleport that waits takes nothing else back: the emote waiting with it keeps its
/// place, and goes once the bucket allows.
#[test]
fn dropping_the_waiting_teleport_keeps_the_waiting_emote() {
    let mut pacer = DiscreteEventPacer::default();
    empty_the_bucket(&mut pacer, 0.0);

    pacer.offer(teleport(1));
    pacer.offer(emote(2));
    pacer.drop_waiting(FrameKind::Teleport);

    assert_eq!(numbers(pacer.release(1.0)), [2]);
    assert!(pacer.release(2.0).is_empty());
}

#[test]
fn a_full_bucket_lets_a_teleport_and_an_emote_through_at_once() {
    let mut pacer = DiscreteEventPacer::default();

    pacer.offer(teleport(1));
    pacer.offer(emote(2));

    assert_eq!(numbers(pacer.release(0.0)), [1, 2]);
}

/// The calls the design makes: half the servers' 20 a second and burst of 16.
#[test]
fn the_bucket_holds_eight_and_gives_back_ten_a_second() {
    let mut pacer = DiscreteEventPacer::default();
    for n in 0..8 {
        pacer.offer(teleport(n));
        assert_eq!(numbers(pacer.release(0.0)), [n]);
    }

    pacer.offer(teleport(8));
    assert!(pacer.release(0.0).is_empty());
    // A token takes a tenth of a second, and nine tenths of one is not enough.
    assert!(pacer.release(0.09).is_empty());
    assert_eq!(numbers(pacer.release(0.11)), [8]);
}

#[test]
fn an_idle_bucket_saves_no_more_than_the_burst() {
    let mut pacer = DiscreteEventPacer::default();
    empty_the_bucket(&mut pacer, 0.0);

    // A long quiet spell fills it again, and no further.
    empty_the_bucket(&mut pacer, 100.0);

    pacer.offer(teleport(1));
    assert!(pacer.release(100.0).is_empty());
}

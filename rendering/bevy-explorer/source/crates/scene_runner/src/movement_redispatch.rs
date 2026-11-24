//! Re-dispatches the movement controller scene on a late reply. A reply that lands after the
//! frame's scene loop deadline leaves the scene in flight, so the next frame would skip its
//! dispatch. On such a reply, the scene is dispatched again at once, ahead of the queue.
//!
//! The re-dispatch is sent the rotation that the late reply sets (`RedispatchRotation`), because
//! the engine applies that rotation only after the scene loop. With the player's current rotation,
//! every other frame would continue a different chain of replies, and the avatar would alternate
//! between two rotations.

use std::{collections::VecDeque, time::Duration};

use web_time::Instant;

use crate::{
    renderer_context::{RendererSceneContext, SceneTickTiming, SCENE_FIRST_UPDATE_TICK},
    SceneUpdates,
};

/// Ticks the percentile is taken over.
const TICK_SAMPLES: usize = 32;

/// The movement scene's recent tick times, from dispatch to reply, and counters for the debug
/// panel.
#[derive(Debug, Default)]
pub(crate) struct MovementTickTimes {
    samples: VecDeque<Duration>,
    /// Replies that landed in a later frame than their dispatch.
    pub(crate) late_replies: u64,
    /// Re-dispatches queued on a late reply.
    pub(crate) redispatches: u64,
    /// Re-dispatches sent the rotation staged from the late reply instead of the player's.
    pub(crate) staged_rotations: u64,
    /// The yaw (degrees) of the last staged rotation sent.
    pub(crate) last_staged_yaw: Option<f32>,
}

impl MovementTickTimes {
    fn record(&mut self, tick: Duration) {
        if self.samples.len() == TICK_SAMPLES {
            self.samples.pop_front();
        }
        self.samples.push_back(tick);
    }

    pub(crate) fn samples(&self) -> usize {
        self.samples.len()
    }

    pub(crate) fn record_staged_rotation(&mut self, rotation: bevy::math::Quat) {
        self.staged_rotations += 1;
        self.last_staged_yaw = Some(rotation.to_euler(bevy::math::EulerRot::YXZ).0.to_degrees());
    }

    /// Nearest rank. `None` before the first tick.
    pub(crate) fn p90(&self) -> Option<Duration> {
        if self.samples.is_empty() {
            return None;
        }
        let mut sorted: Vec<Duration> = self.samples.iter().copied().collect();
        sorted.sort_unstable();
        let rank = (sorted.len() * 9).div_ceil(10);
        Some(sorted[rank.max(1) - 1])
    }

    /// At the loop's first poll, a re-dispatch leaves where the frame's regular dispatch would, so
    /// any time left is enough. The percentile cannot judge that case: a missed tick counts as a
    /// whole loop, so once over a tenth of the ticks miss, the percentile exceeds what any loop has
    /// left after its first poll.
    fn redispatch_fits(&self, left: Duration, first_poll: bool) -> bool {
        !left.is_zero() && (first_poll || self.p90().is_none_or(|p90| left >= p90))
    }
}

impl SceneUpdates {
    /// Records the movement scene's tick that just replied. Returns whether to dispatch the scene
    /// again now.
    pub(crate) fn movement_redispatch_due(
        &mut self,
        context: &RendererSceneContext,
        tick_timing: &SceneTickTiming,
        first_poll: bool,
        now: f64,
    ) -> bool {
        let Some(reply) = tick_timing.reply else {
            return false;
        };
        let late = reply.late();
        let ticks = &mut self.movement_ticks;
        ticks.record(tick_time(
            reply.sent.at,
            reply.at,
            first_poll && late,
            self.previous_loop_end,
        ));
        ticks.late_replies += u64::from(late);
        if !late
            || context.sender().is_none()
            || context.last_sent >= now
            || !context.blocked.is_empty()
            || context.tick_number < SCENE_FIRST_UPDATE_TICK
        {
            return false;
        }
        let left = self.loop_end_time.saturating_duration_since(Instant::now());
        let due = ticks.redispatch_fits(left, first_poll);
        ticks.redispatches += u64::from(due);
        due
    }
}

/// The scene loop polls replies only while it runs. A late reply first seen at a loop's first poll
/// arrived after the previous loop ended, so it counts as arriving then, the earliest it could
/// have.
fn tick_time(
    sent: Instant,
    received: Instant,
    first_poll_of_later_frame: bool,
    previous_loop_end: Instant,
) -> Duration {
    let arrived = if first_poll_of_later_frame {
        previous_loop_end.min(received)
    } else {
        received
    };
    arrived.saturating_duration_since(sent)
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::renderer_context::{RedispatchRotation, ReplyTiming, SceneTickTiming, SentTiming};

    fn ms(v: u64) -> Duration {
        Duration::from_millis(v)
    }

    fn us(v: u64) -> Duration {
        Duration::from_micros(v)
    }

    #[test]
    fn p90_is_the_nearest_rank_over_the_last_samples() {
        let mut times = MovementTickTimes::default();
        assert_eq!(times.p90(), None);
        for v in 1..=10 {
            times.record(ms(v));
        }
        assert_eq!(times.p90(), Some(ms(9)));
        // older samples drop out
        for _ in 0..TICK_SAMPLES {
            times.record(ms(2));
        }
        assert_eq!(times.p90(), Some(ms(2)));
    }

    #[test]
    fn a_late_reply_seen_later_in_the_loop_needs_the_percentile_left() {
        let mut times = MovementTickTimes::default();
        assert!(times.redispatch_fits(ms(1), false));
        for v in 1..=10 {
            times.record(ms(v));
        }
        assert!(times.redispatch_fits(ms(9), false));
        assert!(!times.redispatch_fits(ms(8), false));
    }

    /// A web frame at 60 Hz: the scene loop has 3.1 ms before its deadline, the movement scene
    /// is dispatched 0.2 ms into it, and a quarter of its ticks miss the deadline.
    #[test]
    fn a_late_reply_seen_at_the_first_poll_is_answered_while_the_loop_has_time_left() {
        let mut times = MovementTickTimes::default();
        let loop_start = Instant::now();
        let sent = loop_start + us(200);
        let loop_end = loop_start + us(3100);
        for i in 0..TICK_SAMPLES {
            let tick = if i % 4 == 0 {
                // missed: seen at the next loop's first poll, 14 ms after the deadline
                tick_time(sent, loop_end + ms(14), true, loop_end)
            } else {
                tick_time(sent, sent + us(1500), false, loop_end)
            };
            times.record(tick);
        }
        // the missed ticks count as lasting until the loop ended: the percentile is the loop
        assert_eq!(times.p90(), Some(us(2900)));
        // the next frame's first poll, 0.3 ms into the same 3.1 ms loop
        let left = us(2800);
        assert!(times.redispatch_fits(left, true));
        assert!(!times.redispatch_fits(left, false));
        // nothing is dispatched once the loop is past its deadline
        assert!(!times.redispatch_fits(Duration::ZERO, true));
        assert!(!times.redispatch_fits(Duration::ZERO, false));
    }

    #[test]
    fn a_dispatch_in_the_frame_a_late_reply_landed_is_a_redispatch() {
        let at = Instant::now();
        let sent = |frame| SentTiming { at, frame };
        let replied = |sent_frame, frame| SceneTickTiming {
            sent: Some(sent(sent_frame)),
            reply: Some(ReplyTiming {
                sent: sent(sent_frame),
                at,
                frame,
            }),
        };
        // the reply of frame 6's tick landed in frame 7
        assert!(replied(6, 7).late_reply_in(7));
        // frame 8's dispatch is not a re-dispatch
        assert!(!replied(6, 7).late_reply_in(8));
        // a reply two frames late also makes a re-dispatch, and only that dispatch gets the staged
        // rotation
        assert!(replied(5, 7).late_reply_in(7));
        let staged = RedispatchRotation(bevy::math::Quat::from_rotation_y(1.0));
        let two_late = replied(5, 7);
        assert_eq!(two_late.staged_rotation(Some(&staged), 7), Some(staged.0));
        assert_eq!(two_late.staged_rotation(Some(&staged), 8), None);
        assert_eq!(two_late.staged_rotation(None, 7), None);
        // a reply in its own frame is not late
        assert!(!replied(7, 7).late_reply_in(7));
        assert!(!SceneTickTiming::default().late_reply_in(7));
    }

    #[test]
    fn a_reply_seen_at_a_later_loops_first_poll_counts_until_the_previous_loop_ended() {
        let sent = Instant::now();
        let loop_end = sent + ms(5);
        let received = sent + ms(16);
        assert_eq!(tick_time(sent, received, false, loop_end), ms(16));
        assert_eq!(tick_time(sent, received, true, loop_end), ms(5));
        // a loop that ended before the dispatch (the first frame) gives no negative time
        assert_eq!(tick_time(loop_end, received, true, sent), Duration::ZERO);
    }
}

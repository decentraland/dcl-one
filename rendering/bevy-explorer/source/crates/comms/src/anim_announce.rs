//! Decides what clip identity a scene-driven animation packet announces.
//!
//! The animation packet rides an unreliable transport, and the receiver keeps the clip it was last
//! told about. A clip change is announced by the hash pair (or, for the end of an animation, an
//! empty scene hash). If the one packet that carries it is lost, the receiver keeps showing the old
//! clip, and after a clear nothing more is sent to correct it. So a change is announced on the send
//! that sees it and on the next [`ANNOUNCE_REPEATS`] sends, which bounds a single loss to about
//! two send intervals. A clip that then stays active is re-announced every keepalive so late
//! joiners learn it. Repeating the pair is safe for the receiver: it rebuilds the same clip URN, and
//! seeks and sounds travel only on the send that latches them.
//!
//! A character at rest sends only once a second, so the repeats would be a second apart exactly
//! when a clip changes at rest (stopping, landing). [`ClipAnnouncer::repeat_pending`] lets the
//! caller's send gate treat an owed repeat as a reason to send, which puts the repeats on the next
//! regular sends.
//!
//! A pure object, like the Pulse pacer: its times are `f64` seconds on the caller's clock and it
//! knows nothing of Bevy or the connection, so a test drives it without an app.

/// Hashes of the active clip: `(scene_hash, content_hash)`.
pub type ClipHashes = (String, String);

/// How many sends after the one that sees a change repeat the announcement.
pub const ANNOUNCE_REPEATS: u32 = 2;

/// What one send says about the clip.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Announcement {
    /// Leave both hashes out; the receiver keeps the clip it has.
    Nothing,
    /// Attach the hash pair of the current clip.
    Clip,
    /// Attach an empty scene hash: no animation is active.
    Clear,
}

#[derive(Default)]
pub struct ClipAnnouncer {
    /// The clip as of the previous send (`None` = no animation).
    current: Option<ClipHashes>,
    /// Sends still to repeat the announcement of `current`.
    repeats_left: u32,
    /// When a hash pair or clear last went out.
    announced_at: f64,
}

impl ClipAnnouncer {
    /// Whether the next send still owes a repeat of the last change or clear.
    pub fn repeat_pending(&self) -> bool {
        self.repeats_left > 0
    }

    /// The announcement for a send at `now` while `current` is the active clip. Call it once per
    /// packet actually sent. `keepalive` is how often an unchanged active clip is re-announced.
    pub fn announce(
        &mut self,
        now: f64,
        current: Option<&ClipHashes>,
        keepalive: f64,
    ) -> Announcement {
        let changed = self.current.as_ref() != current;
        if changed {
            self.current = current.cloned();
            self.repeats_left = ANNOUNCE_REPEATS;
        } else if self.repeats_left > 0 {
            self.repeats_left -= 1;
        } else if current.is_none() || now - self.announced_at < keepalive {
            return Announcement::Nothing;
        }
        self.announced_at = now;
        if current.is_some() {
            Announcement::Clip
        } else {
            Announcement::Clear
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const KEEPALIVE: f64 = 1.0;

    fn clip(name: &str) -> ClipHashes {
        (format!("scene-{name}"), format!("content-{name}"))
    }

    /// Sends every 0.125 s (exact in binary, so the keepalive arithmetic has no rounding) from `start`, returning what each announced.
    fn sends(
        announcer: &mut ClipAnnouncer,
        start: f64,
        count: usize,
        current: Option<&ClipHashes>,
    ) -> Vec<Announcement> {
        (0..count)
            .map(|i| announcer.announce(start + i as f64 * 0.125, current, KEEPALIVE))
            .collect()
    }

    #[test]
    fn a_change_is_announced_on_its_send_and_the_next_repeats() {
        use Announcement::*;
        let mut announcer = ClipAnnouncer::default();
        let jump = clip("jump");
        assert_eq!(
            sends(&mut announcer, 0.0, 5, Some(&jump)),
            [Clip, Clip, Clip, Nothing, Nothing]
        );
    }

    #[test]
    fn an_unchanged_clip_keeps_the_keepalive_cadence() {
        use Announcement::*;
        let mut announcer = ClipAnnouncer::default();
        let walk = clip("walk");
        let all = sends(&mut announcer, 0.0, 25, Some(&walk));
        let announced: Vec<usize> = (0..all.len()).filter(|&i| all[i] == Clip).collect();
        // the change and its repeats, then once a second after the last of them (at 0.25 s)
        assert_eq!(announced, [0, 1, 2, 10, 18]);
        assert!(all.iter().all(|a| *a != Clear));
    }

    #[test]
    fn repeats_are_pending_from_a_change_until_they_are_spent() {
        let mut announcer = ClipAnnouncer::default();
        let walk = clip("walk");
        assert!(!announcer.repeat_pending());
        // the send that sees the change owes the next two
        for _ in 0..ANNOUNCE_REPEATS {
            announcer.announce(0.0, Some(&walk), KEEPALIVE);
            assert!(announcer.repeat_pending());
        }
        announcer.announce(0.25, Some(&walk), KEEPALIVE);
        assert!(!announcer.repeat_pending());
        // a keepalive owes nothing
        assert_eq!(
            announcer.announce(1.25, Some(&walk), KEEPALIVE),
            Announcement::Clip
        );
        assert!(!announcer.repeat_pending());
    }

    #[test]
    fn a_clear_followed_by_silence_still_yields_its_repeats() {
        use Announcement::*;
        let mut announcer = ClipAnnouncer::default();
        let walk = clip("walk");
        sends(&mut announcer, 0.0, 3, Some(&walk));
        // the caller sends only while a repeat is pending once nothing else changes
        let mut announced = Vec::new();
        let mut now = 0.375;
        while announcer.repeat_pending() || announced.is_empty() {
            announced.push(announcer.announce(now, None, KEEPALIVE));
            now += 0.125;
        }
        assert_eq!(announced, [Clear, Clear, Clear]);
        assert_eq!(announcer.announce(now, None, KEEPALIVE), Nothing);
    }

    #[test]
    fn nothing_is_announced_while_no_animation_has_been_active() {
        let mut announcer = ClipAnnouncer::default();
        assert!(sends(&mut announcer, 0.0, 30, None)
            .iter()
            .all(|a| *a == Announcement::Nothing));
    }

    #[test]
    fn a_clear_is_repeated_though_no_animation_is_active() {
        use Announcement::*;
        let mut announcer = ClipAnnouncer::default();
        let walk = clip("walk");
        sends(&mut announcer, 0.0, 3, Some(&walk));
        assert_eq!(
            sends(&mut announcer, 0.375, 5, None),
            [Clear, Clear, Clear, Nothing, Nothing]
        );
    }

    #[test]
    fn a_change_inside_the_repeat_window_restarts_it_with_the_new_pair() {
        use Announcement::*;
        let mut announcer = ClipAnnouncer::default();
        let (jump, land) = (clip("jump"), clip("land"));
        assert_eq!(sends(&mut announcer, 0.0, 2, Some(&jump)), [Clip, Clip]);
        // the clip changes on the third send, one repeat short of done
        assert_eq!(
            sends(&mut announcer, 0.25, 5, Some(&land)),
            [Clip, Clip, Clip, Nothing, Nothing]
        );
        // and a clear in the window replaces a pair's repeats
        let mut announcer = ClipAnnouncer::default();
        assert_eq!(sends(&mut announcer, 0.0, 2, Some(&jump)), [Clip, Clip]);
        assert_eq!(
            sends(&mut announcer, 0.25, 4, None),
            [Clear, Clear, Clear, Nothing]
        );
    }
}

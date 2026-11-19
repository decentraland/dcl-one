//! Decides what clip identity a scene-driven animation packet announces.
//!
//! The animation packet rides an unreliable transport, and the receiver keeps the clip it was last
//! told about. A clip change is announced by the hash pair (or, for the end of an animation, an
//! empty scene hash). If the one packet that carries it is lost, the receiver keeps showing the old
//! clip, and after a clear nothing more is sent to correct it. So a change is announced on the send
//! that sees it and on the next [`ANNOUNCE_REPEATS`] sends, which tolerates up to two consecutive
//! losses, so a single loss costs about one send interval. A clip that then stays active is
//! re-announced every keepalive so late joiners learn it. Repeating the pair is safe for the
//! receiver: it rebuilds the same clip URN, and seeks and sounds travel only on the send that
//! latches them. The repeats carry the clip identity only: a seek or a sound trigger on a lost
//! first packet is not repeated, so the clip then starts from its beginning and the sound does not
//! play.
//!
//! A character at rest sends only once a second, so the repeats would be a second apart exactly
//! when a clip changes at rest (stopping, landing). [`ClipAnnouncer::repeat_pending`] lets the
//! caller's send gate treat an owed repeat as a reason to send, which puts the repeats on the next
//! regular sends.
//!
//! Each movement send consumes a repeat even when no transport carries the animation packet.
//! An active clip is announced again at the next keepalive. A clear has only its initial send
//! and its repeats.
//!
//! A pure object, like the Pulse pacer: its times are `f64` seconds on the caller's clock and it
//! knows nothing of Bevy or the connection, so a test drives it without an app.

/// Hashes of the active clip: `(scene_hash, content_hash)`.
pub type ClipHashes = (String, String);

/// How many sends after the one that sees a change repeat the announcement.
pub const ANNOUNCE_REPEATS: u32 = 2;

/// What one send says about the clip.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Announcement<'a> {
    /// Leave both hashes out; the receiver keeps the clip it has.
    Nothing,
    /// Attach the hash pair of the current clip.
    Clip(&'a ClipHashes),
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

    /// Whether `current` differs from the clip as of the previous send.
    pub fn changed(&self, current: Option<&ClipHashes>) -> bool {
        self.current.as_ref() != current
    }

    /// Call once per movement send at `now`, even if no animation transport can carry the packet.
    /// `current` is the active clip. `keepalive` is the interval for re-announcing an unchanged clip.
    pub fn announce<'a>(
        &mut self,
        now: f64,
        current: Option<&'a ClipHashes>,
        keepalive: f64,
    ) -> Announcement<'a> {
        if self.changed(current) {
            self.current = current.cloned();
            self.repeats_left = ANNOUNCE_REPEATS;
        } else if self.repeats_left > 0 {
            self.repeats_left -= 1;
        } else if current.is_none() || now - self.announced_at < keepalive {
            return Announcement::Nothing;
        }
        self.announced_at = now;
        match current {
            Some(hashes) => Announcement::Clip(hashes),
            None => Announcement::Clear,
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

    /// Sends every 0.125 s from `start`, returning what each announced. The step is exact in
    /// binary, so the keepalive arithmetic has no rounding.
    fn sends<'a>(
        announcer: &mut ClipAnnouncer,
        start: f64,
        count: usize,
        current: Option<&'a ClipHashes>,
    ) -> Vec<Announcement<'a>> {
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
            [Clip(&jump), Clip(&jump), Clip(&jump), Nothing, Nothing]
        );
    }

    #[test]
    fn an_unchanged_clip_keeps_the_keepalive_cadence() {
        use Announcement::*;
        let mut announcer = ClipAnnouncer::default();
        let walk = clip("walk");
        let all = sends(&mut announcer, 0.0, 25, Some(&walk));
        let announced: Vec<usize> = (0..all.len()).filter(|&i| all[i] == Clip(&walk)).collect();
        // the change and its repeats, then once a second after the last of them (at 0.25 s)
        assert_eq!(announced, [0, 1, 2, 10, 18]);
        assert!(all.iter().all(|a| *a != Clear));
    }

    #[test]
    fn changed_compares_with_the_clip_of_the_previous_send() {
        let mut announcer = ClipAnnouncer::default();
        let walk = clip("walk");
        assert!(!announcer.changed(None));
        assert!(announcer.changed(Some(&walk)));
        // asking does not record anything: only a send does
        assert!(announcer.changed(Some(&walk)));
        announcer.announce(0.0, Some(&walk), KEEPALIVE);
        assert!(!announcer.changed(Some(&walk)));
        assert!(announcer.changed(Some(&clip("jump"))));
        assert!(announcer.changed(None));
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
            Announcement::Clip(&walk)
        );
        assert!(!announcer.repeat_pending());
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
        assert!(!announcer.repeat_pending());
        assert_eq!(announcer.announce(0.375, None, KEEPALIVE), Clear);
        for now in [0.5, 0.625] {
            assert!(announcer.repeat_pending());
            assert_eq!(announcer.announce(now, None, KEEPALIVE), Clear);
        }
        assert!(!announcer.repeat_pending());
        assert_eq!(sends(&mut announcer, 0.75, 2, None), [Nothing, Nothing]);
    }

    #[test]
    fn a_change_inside_the_repeat_window_restarts_it_with_the_new_pair() {
        use Announcement::*;
        let mut announcer = ClipAnnouncer::default();
        let (jump, land) = (clip("jump"), clip("land"));
        assert_eq!(
            sends(&mut announcer, 0.0, 2, Some(&jump)),
            [Clip(&jump), Clip(&jump)]
        );
        // the clip changes on the third send, one repeat short of done
        assert_eq!(
            sends(&mut announcer, 0.25, 5, Some(&land)),
            [Clip(&land), Clip(&land), Clip(&land), Nothing, Nothing]
        );
        // and a clear in the window replaces a pair's repeats
        let mut announcer = ClipAnnouncer::default();
        assert_eq!(
            sends(&mut announcer, 0.0, 2, Some(&jump)),
            [Clip(&jump), Clip(&jump)]
        );
        assert_eq!(
            sends(&mut announcer, 0.25, 4, None),
            [Clear, Clear, Clear, Nothing]
        );
    }
}

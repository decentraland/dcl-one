//! What crosses the wire for situational reactions, apart from how they are drawn: the local
//! user's picks gathered into messages, and what of a peer's messages is let through and when
//! each of its reactions plays.

use std::collections::VecDeque;

use bevy::prelude::*;
use common::token_bucket::TokenBucket;
use comms::chat_reaction::atlas_index_to_emoji;
use dcl_component::proto_components::kernel::comms::rfc4;

/// Picks are sent this long after the last one, in seconds.
const SEND_DEBOUNCE: f32 = 0.3;
/// Picks are sent at once when this many wait.
const SEND_THRESHOLD: u32 = 10;
/// The least by which a sent stamp follows the one before, in seconds.
const STAMP_STEP: f32 = 0.001;

/// The messages of a sender remembered to recognise a copy of one.
const SEEN_MESSAGES: usize = 64;
/// Messages a sender may have let through per second, and at once.
const MESSAGES_PER_SECOND: f64 = 20.0;
const MESSAGE_BURST: f64 = 20.0;
/// The most reactions one message plays.
const MAX_COUNT: i32 = 50;

/// The most reactions of a sender waiting to play; the oldest give way.
const MAX_QUEUED: usize = 64;
/// A sender's reactions play this far apart, in seconds.
const STAGGER: f32 = 0.2;

/// One message to send: an emoji's tile in the wire's atlas, how many times it was picked, and
/// the stamp receivers tell the message apart by.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Outgoing {
    pub tile: i32,
    pub count: i32,
    pub timestamp: f32,
}

impl From<Outgoing> for rfc4::Reaction {
    fn from(outgoing: Outgoing) -> Self {
        Self {
            emoji_index: outgoing.tile,
            timestamp: outgoing.timestamp,
            count: outgoing.count,
        }
    }
}

/// The local user's picks not yet sent: repeats of an emoji go out as one message with a count.
#[derive(Component, Default)]
pub struct ReactionOutbox {
    /// Each emoji's tile and how many times it was picked, in the order first picked.
    picks: Vec<(i32, i32)>,
    waiting: u32,
    /// Seconds until the picks are sent.
    timer: f32,
    last_stamp: f32,
}

impl ReactionOutbox {
    pub fn push(&mut self, tile: i32) {
        match self.picks.iter_mut().find(|(picked, _)| *picked == tile) {
            Some((_, count)) => *count += 1,
            None => self.picks.push((tile, 1)),
        }
        self.waiting += 1;
        self.timer = SEND_DEBOUNCE;
    }

    pub fn waiting(&self) -> u32 {
        self.waiting
    }

    /// Advance by `dt` seconds: the messages to send now, none while the picks still wait.
    /// `now` is how long the sender has been up, in seconds.
    pub fn tick(&mut self, dt: f32, now: f32) -> Vec<Outgoing> {
        if self.picks.is_empty() {
            return Vec::new();
        }
        self.timer -= dt;
        if self.timer > 0.0 && self.waiting < SEND_THRESHOLD {
            return Vec::new();
        }
        self.waiting = 0;
        std::mem::take(&mut self.picks)
            .into_iter()
            .map(|(tile, count)| Outgoing {
                tile,
                count,
                timestamp: self.next_stamp(now),
            })
            .collect()
    }

    /// A stamp later than every one sent before. Receivers drop a message whose stamp they
    /// have seen, and a stamp of zero reads as none; a long uptime leaves an f32 no room for
    /// the step, so the next value up stands in.
    fn next_stamp(&mut self, now: f32) -> f32 {
        let after_last = (self.last_stamp + STAMP_STEP).max(self.last_stamp.next_up());
        self.last_stamp = now.max(after_last);
        self.last_stamp
    }
}

/// Why a peer's message plays nothing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refused {
    /// A copy of a message already taken: the same message arrives by each room shared with
    /// its sender.
    Duplicate,
    /// No emoji has that index.
    UnknownEmoji,
    /// The sender is over their rate.
    RateLimited,
}

/// What a sender's earlier messages allow of their next. A peer chooses everything in a
/// message, so each field is bounded here before it is used.
#[derive(Component)]
pub struct ReactionGuard {
    /// The stamp bits and emoji index of the last messages seen.
    seen: VecDeque<(u32, i32)>,
    /// The messages the sender may still have let through.
    bucket: TokenBucket,
}

impl Default for ReactionGuard {
    fn default() -> Self {
        Self {
            seen: VecDeque::new(),
            bucket: TokenBucket::new(MESSAGES_PER_SECOND, MESSAGE_BURST),
        }
    }
}

impl ReactionGuard {
    /// The emoji a message plays and how many times, or why it plays nothing. `now` is in
    /// seconds, on a clock that only moves on.
    pub fn admit(
        &mut self,
        reaction: &rfc4::Reaction,
        now: f64,
    ) -> Result<(&'static str, u32), Refused> {
        // a stamp of zero is no stamp: such a message can't be told from another
        if reaction.timestamp != 0.0 {
            let key = (reaction.timestamp.to_bits(), reaction.emoji_index);
            if self.seen.contains(&key) {
                return Err(Refused::Duplicate);
            }
            if self.seen.len() == SEEN_MESSAGES {
                self.seen.pop_front();
            }
            self.seen.push_back(key);
        }

        let emoji = atlas_index_to_emoji(reaction.emoji_index).ok_or(Refused::UnknownEmoji)?;

        if !self.bucket.try_spend(now) {
            return Err(Refused::RateLimited);
        }

        Ok((emoji, reaction.count.clamp(1, MAX_COUNT) as u32))
    }
}

/// A sender's reactions waiting to play, one at a time: on the sender while any wait and until
/// the gap after the last has run out, so one that follows it waits the gap out too.
#[derive(Component, Default)]
pub struct ReactionQueue {
    waiting: VecDeque<&'static str>,
    /// Seconds until the next plays.
    timer: f32,
}

impl ReactionQueue {
    /// Queue `emoji` `count` times. Returns how many older reactions gave way.
    pub fn push(&mut self, emoji: &'static str, count: u32) -> u32 {
        let mut dropped = 0;
        for _ in 0..count {
            if self.waiting.len() == MAX_QUEUED {
                self.waiting.pop_front();
                dropped += 1;
            }
            self.waiting.push_back(emoji);
        }
        dropped
    }

    /// Advance by `dt` seconds: the reactions that play now, oldest first.
    pub fn tick(&mut self, dt: f32) -> Vec<&'static str> {
        let mut playing = Vec::new();
        self.timer -= dt;
        while self.timer <= 0.0 {
            let Some(emoji) = self.waiting.pop_front() else {
                break;
            };
            playing.push(emoji);
            self.timer += STAGGER;
        }
        playing
    }

    pub fn len(&self) -> usize {
        self.waiting.len()
    }

    /// Nothing waits and the gap after the last has run out.
    pub fn done(&self) -> bool {
        self.waiting.is_empty() && self.timer <= 0.0
    }
}

#[cfg(test)]
mod tests {
    use comms::chat_reaction::emoji_to_atlas_index;

    use super::*;

    fn message(tile: i32, timestamp: f32, count: i32) -> rfc4::Reaction {
        rfc4::Reaction {
            emoji_index: tile,
            timestamp,
            count,
        }
    }

    #[test]
    fn picks_are_sent_once_they_stop() {
        let mut outbox = ReactionOutbox::default();
        outbox.push(7);
        assert!(outbox.tick(0.2, 10.0).is_empty());
        // a pick in the meantime starts the wait again
        outbox.push(7);
        assert!(outbox.tick(0.2, 10.2).is_empty());
        assert_eq!(outbox.waiting(), 2);
        let sent = outbox.tick(0.15, 10.35);
        assert_eq!(sent.len(), 1);
        assert_eq!((sent[0].tile, sent[0].count), (7, 2));
        assert_eq!(outbox.waiting(), 0);
        // nothing is left to send
        assert!(outbox.tick(1.0, 11.3).is_empty());
    }

    #[test]
    fn each_emoji_is_one_message_in_the_order_first_picked() {
        let mut outbox = ReactionOutbox::default();
        for tile in [3, 9, 3, 5, 9, 3] {
            outbox.push(tile);
        }
        let sent = outbox.tick(SEND_DEBOUNCE, 4.0);
        assert_eq!(
            sent.iter()
                .map(|out| (out.tile, out.count))
                .collect::<Vec<_>>(),
            [(3, 3), (9, 2), (5, 1)]
        );
    }

    #[test]
    fn enough_picks_are_sent_without_waiting() {
        let mut outbox = ReactionOutbox::default();
        for _ in 0..SEND_THRESHOLD - 1 {
            outbox.push(1);
        }
        assert!(outbox.tick(0.0, 1.0).is_empty());
        outbox.push(2);
        let sent = outbox.tick(0.0, 1.0);
        assert_eq!(
            sent.iter()
                .map(|out| (out.tile, out.count))
                .collect::<Vec<_>>(),
            [(1, SEND_THRESHOLD as i32 - 1), (2, 1)]
        );
    }

    #[test]
    fn stamps_only_go_up_however_long_the_sender_has_been_up() {
        for now in [0.0, 12.5, 100_000.0, 4_000_000.0] {
            let mut outbox = ReactionOutbox::default();
            let mut last = 0.0;
            for round in 0..3 {
                for tile in 0..4 {
                    outbox.push(tile);
                }
                // the clock may not have moved between two sends
                for out in outbox.tick(SEND_DEBOUNCE, now + round as f32 * f32::EPSILON) {
                    assert!(
                        out.timestamp > last,
                        "{} after {last} at {now}",
                        out.timestamp
                    );
                    last = out.timestamp;
                }
            }
        }
    }

    #[test]
    fn a_copy_of_a_message_plays_once() {
        let thumbs = emoji_to_atlas_index("👍");
        let fire = emoji_to_atlas_index("🔥");
        let mut guard = ReactionGuard::default();
        assert_eq!(guard.admit(&message(thumbs, 5.0, 2), 1.0), Ok(("👍", 2)));
        assert_eq!(
            guard.admit(&message(thumbs, 5.0, 2), 1.0),
            Err(Refused::Duplicate)
        );
        // another emoji under the same stamp is another message
        assert_eq!(guard.admit(&message(fire, 5.0, 1), 1.0), Ok(("🔥", 1)));
        // a message with no stamp is never a copy
        assert!(guard.admit(&message(thumbs, 0.0, 1), 1.0).is_ok());
        assert!(guard.admit(&message(thumbs, 0.0, 1), 1.0).is_ok());
        // only the last messages are remembered
        for stamp in 0..SEEN_MESSAGES {
            let _ = guard.admit(&message(fire, 100.0 + stamp as f32, 1), 1.0);
        }
        assert_ne!(
            guard.admit(&message(thumbs, 5.0, 2), 1000.0),
            Err(Refused::Duplicate)
        );
    }

    #[test]
    fn a_copy_costs_its_sender_nothing() {
        let thumbs = emoji_to_atlas_index("👍");
        let mut guard = ReactionGuard::default();
        for _ in 0..3 {
            let _ = guard.admit(&message(thumbs, 1.0, 1), 0.0);
        }
        // the whole burst is still there but for the one let through
        for stamp in 1..MESSAGE_BURST as u32 {
            assert!(guard
                .admit(&message(thumbs, 1.0 + stamp as f32, 1), 0.0)
                .is_ok());
        }
        assert_eq!(
            guard.admit(&message(thumbs, 99.0, 1), 0.0),
            Err(Refused::RateLimited)
        );
    }

    #[test]
    fn a_sender_over_their_rate_waits_for_it() {
        let thumbs = emoji_to_atlas_index("👍");
        let mut guard = ReactionGuard::default();
        let mut stamp = 0.0;
        let mut send = |guard: &mut ReactionGuard, now: f64| {
            stamp += 1.0;
            guard.admit(&message(thumbs, stamp, 1), now)
        };
        for _ in 0..MESSAGE_BURST as u32 {
            assert!(send(&mut guard, 10.0).is_ok());
        }
        assert_eq!(send(&mut guard, 10.0), Err(Refused::RateLimited));
        // one message's worth of time lets one more through
        let one = 1.0 / MESSAGES_PER_SECOND;
        assert!(send(&mut guard, 10.0 + one * 1.5).is_ok());
        assert_eq!(
            send(&mut guard, 10.0 + one * 1.5),
            Err(Refused::RateLimited)
        );
        // a long quiet refills no more than the burst
        for _ in 0..MESSAGE_BURST as u32 {
            assert!(send(&mut guard, 1000.0).is_ok());
        }
        assert_eq!(send(&mut guard, 1000.0), Err(Refused::RateLimited));
    }

    #[test]
    fn what_a_peer_chose_is_bounded() {
        let thumbs = emoji_to_atlas_index("👍");
        let mut guard = ReactionGuard::default();
        for (stamp, index) in [-1, i32::MIN, 3576, 4095, i32::MAX].into_iter().enumerate() {
            assert_eq!(
                guard.admit(&message(index, 1.0 + stamp as f32, 1), 0.0),
                Err(Refused::UnknownEmoji),
                "{index}"
            );
        }
        for (stamp, (count, plays)) in [(0, 1), (-5, 1), (i32::MIN, 1), (7, 7), (i32::MAX, 50)]
            .into_iter()
            .enumerate()
        {
            assert_eq!(
                guard.admit(&message(thumbs, 50.0 + stamp as f32, count), 0.0),
                Ok(("👍", plays)),
                "{count}"
            );
        }
        for stamp in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY, -1.0] {
            assert!(guard.admit(&message(thumbs, stamp, 1), 0.0).is_ok());
        }
    }

    #[test]
    fn reactions_play_one_at_a_time() {
        let mut queue = ReactionQueue::default();
        assert_eq!(queue.push("👍", 3), 0);
        // the first plays at once
        assert_eq!(queue.tick(0.0), ["👍"]);
        assert!(queue.tick(STAGGER * 0.5).is_empty());
        assert_eq!(queue.tick(STAGGER * 0.5), ["👍"]);
        // a long frame plays what its time covers
        queue.push("🔥", 3);
        assert_eq!(queue.tick(STAGGER * 2.0), ["👍", "🔥"]);
        assert_eq!(queue.len(), 2);
        assert_eq!(queue.tick(60.0), ["🔥", "🔥"]);
        assert!(queue.done());
    }

    #[test]
    fn a_reaction_that_follows_the_last_waits_out_the_gap() {
        let mut queue = ReactionQueue::default();
        queue.push("🔥", 1);
        assert_eq!(queue.tick(0.0), ["🔥"]);
        // nothing waits, and the gap has only begun
        assert_eq!(queue.len(), 0);
        assert!(!queue.done());

        // the next message's reaction, a moment later
        queue.push("👍", 1);
        assert!(queue.tick(STAGGER * 0.5).is_empty());
        assert_eq!(queue.tick(STAGGER * 0.5), ["👍"]);
        assert!(!queue.done());
        assert!(queue.tick(STAGGER).is_empty());
        assert!(queue.done());
    }

    #[test]
    fn a_full_queue_keeps_the_newest() {
        let mut queue = ReactionQueue::default();
        assert_eq!(queue.push("👍", MAX_QUEUED as u32), 0);
        assert_eq!(queue.push("🔥", 2), 2);
        assert_eq!(queue.len(), MAX_QUEUED);
        let played = queue.tick(600.0);
        assert_eq!(played.len(), MAX_QUEUED);
        assert_eq!(played[0], "👍");
        assert_eq!(played[MAX_QUEUED - 2..], ["🔥", "🔥"]);
    }
}

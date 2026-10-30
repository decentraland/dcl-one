//! The abuse throttle of the friendship/block RPC mutations (upstream
//! social-service-ea #456): each step publishes an update into the other
//! account's subscription stream, so an unbounded REQUEST/CANCEL or
//! block/unblock loop is a push-spam primitive. Budgets: 30 actions per actor
//! and 10 per pair per minute, over the shared token bucket
//! (`crate::comms::limits::RateLimiter`). In-process only, like the rest of
//! this module's state (see `pubsub`).

use std::time::Duration;

pub use crate::comms::limits::{RateLimitDecision, RateLimiter};

const FRIENDSHIP_RATE_LIMIT_PER_ACTOR: u32 = 30;
const FRIENDSHIP_RATE_LIMIT_PER_PAIR: u32 = 10;
const FRIENDSHIP_RATE_LIMIT_WINDOW: Duration = Duration::from_secs(60);

/// A friendship is symmetric while a block is not: a canonical (sorted) key
/// for block/unblock would let the account being blocked spend the budget the
/// victim needs, deciding whether the block lands (upstream #456).
#[derive(Debug, Clone, Copy)]
pub enum PairScope {
    Symmetric,
    Directional,
}

/// The two pair schemes keep distinct key prefixes so they can never merge.
pub struct FriendshipMutationLimiter {
    per_actor: RateLimiter,
    per_pair: RateLimiter,
}

impl Default for FriendshipMutationLimiter {
    fn default() -> Self {
        Self::new()
    }
}

impl FriendshipMutationLimiter {
    pub fn new() -> Self {
        Self {
            per_actor: RateLimiter::new(
                FRIENDSHIP_RATE_LIMIT_PER_ACTOR,
                FRIENDSHIP_RATE_LIMIT_WINDOW,
            ),
            per_pair: RateLimiter::new(
                FRIENDSHIP_RATE_LIMIT_PER_PAIR,
                FRIENDSHIP_RATE_LIMIT_WINDOW,
            ),
        }
    }

    pub fn allow(&self, actor: &str, target: &str, scope: PairScope) -> bool {
        if self.per_actor.check(&format!("actor:{actor}")) == RateLimitDecision::Deny {
            return false;
        }
        let pair_key = match scope {
            PairScope::Symmetric => {
                let (a, b) = if actor <= target {
                    (actor, target)
                } else {
                    (target, actor)
                };
                format!("pair:sym:{a}:{b}")
            }
            PairScope::Directional => format!("pair:dir:{actor}:{target}"),
        };
        self.per_pair.check(&pair_key) == RateLimitDecision::Allow
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn actor_budget_spans_targets() {
        let l = FriendshipMutationLimiter::new();
        for i in 0..FRIENDSHIP_RATE_LIMIT_PER_ACTOR {
            assert!(l.allow("0xa", &format!("0xt{i}"), PairScope::Symmetric));
        }
        assert!(!l.allow("0xa", "0xfresh", PairScope::Symmetric));
    }

    #[test]
    fn pair_budget_is_symmetric_for_friendship_actions() {
        let l = FriendshipMutationLimiter::new();
        for i in 0..FRIENDSHIP_RATE_LIMIT_PER_PAIR / 2 {
            assert!(l.allow("0xa", "0xb", PairScope::Symmetric), "a->b {i}");
            assert!(l.allow("0xb", "0xa", PairScope::Symmetric), "b->a {i}");
        }
        assert!(!l.allow("0xa", "0xb", PairScope::Symmetric));
        assert!(!l.allow("0xb", "0xa", PairScope::Symmetric));
    }

    #[test]
    fn block_budget_is_directional_so_the_counterparty_cannot_spend_it() {
        let l = FriendshipMutationLimiter::new();
        for _ in 0..FRIENDSHIP_RATE_LIMIT_PER_PAIR {
            assert!(l.allow("0xspammer", "0xvictim", PairScope::Directional));
        }
        assert!(!l.allow("0xspammer", "0xvictim", PairScope::Directional));
        assert!(l.allow("0xvictim", "0xspammer", PairScope::Directional));
    }

    #[test]
    fn symmetric_and_directional_buckets_never_merge() {
        let l = FriendshipMutationLimiter::new();
        for _ in 0..FRIENDSHIP_RATE_LIMIT_PER_PAIR {
            assert!(l.allow("0xa", "0xb", PairScope::Symmetric));
        }
        assert!(!l.allow("0xa", "0xb", PairScope::Symmetric));
        assert!(l.allow("0xa", "0xb", PairScope::Directional));
    }
}

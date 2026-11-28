//! The community chat message router: a port of Decentraland's `comms-message-sfu`
//! (revision d38ccd0885ad23028604e9a9fcc788cdfcbb81fe). It runs as its own process
//! (`catalyst-message-router`), a LiveKit participant on the private messages room. An Explorer
//! sends a community message or reaction to it on the topic `community:<id>`; the router checks
//! that the sender is a member of the community, stamps the sender into `Chat.forwarded_from`
//! (a reaction into `ChatReaction.address`) and publishes the packet to the other members of the
//! active community, 100 destination identities per publish.
//!
//! - [`routing`]: what happens to one packet, from admission to the delivery outcome;
//! - [`membership`]: the one read of `social.community_members` and `social.communities`;
//! - [`connection`]: when a disconnect reconnects, and when it ends the process;
//! - [`in_flight`]: the packets routed at once, dropped `overloaded` beyond the limit;
//! - [`token`]: the join token, asked of the gatekeeper (`shared::message_router`), so this
//!   process holds no LiveKit secret;
//! - [`metrics`]: the delivery counter, latency histogram and connection gauge;
//! - [`config`]: the environment it reads, and the per-sender token bucket
//!   (`shared::limits::RateLimiter`);
//! - `participant` (feature `livekit`): the LiveKit room and its event loop.

// A silently truncated, wrapped or rounded cast is a bug.
#![deny(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_possible_wrap,
    clippy::cast_precision_loss
)]

// The crates this one builds on, under their module names (AGENTS.md).
pub(crate) use catalyst_shared as shared;

pub mod config;
pub mod connection;
pub mod in_flight;
pub mod membership;
pub mod metrics;
#[cfg(feature = "livekit")]
pub mod participant;
pub mod proto;
pub mod routing;
pub mod token;

#[cfg(test)]
mod tests {
    /// The workspace takes LiveKit from the commit the Explorer's patched client-sdk-rust names,
    /// so a repin of that source cannot leave this process on another SDK.
    #[test]
    fn livekit_is_the_explorer_s_patched_sdk() {
        let read = |path: &str| {
            std::fs::read_to_string(format!("{}/{path}", env!("DCL_ONE_ROOT")))
                .unwrap_or_else(|e| panic!("{path}: {e}"))
                .parse::<toml::Table>()
                .unwrap_or_else(|e| panic!("{path}: {e}"))
        };
        let source = read("rendering/bevy-explorer/patches/client-sdk-rust/source.toml");
        let workspace = read("services/catalyst/Cargo.toml");
        let explorer = read("rendering/bevy-explorer/source/Cargo.toml");
        let livekit = &workspace["workspace"]["dependencies"]["livekit"];
        let pinned = &explorer["patch"]["crates-io"]["livekit"];
        assert_eq!(livekit["rev"], source["rev"]);
        assert_eq!(livekit["git"], pinned["git"]);
        assert_eq!(livekit["rev"], pinned["rev"]);
    }
}

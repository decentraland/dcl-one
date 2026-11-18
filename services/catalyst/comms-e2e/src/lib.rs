//! End-to-end scenarios against a running comms stack.
//!
//! A scenario scripts a situation with three kinds of parts and states what must hold: thin
//! protocol bots (`pulse`, `archipelago`, `gatekeeper`, `livekit`), a fault link that relays and
//! disturbs traffic (`fault`), and the headless explorer as a child process (`explorer`). The
//! `engine` runs scenarios, repeats races with seeds, and reports.

pub mod engine;
pub mod identity;
pub mod pulse;
pub mod scenarios;
pub mod stack;

//! The scene runtime's per-tick limits that the engine relies on, checked again by the web relay
//! (`inner::relay`) because on web the runtime's memory is reachable by scene code. Kept outside
//! the wasm-only module so the host test suite covers it.

use common::rpc::RpcCall;
use dcl_js::comms::{MAX_COMMS_MESSAGE_BYTES, MAX_SEND_MESSAGES_PER_TICK};
use dcl_types::{MAX_RPC_CALLS_PER_TICK, MAX_TOPIC_MESSAGE_BYTES};

/// The first limit a tick's calls break, if any.
pub fn tick_limit_broken(calls: &[RpcCall]) -> Option<&'static str> {
    if calls.len() > MAX_RPC_CALLS_PER_TICK {
        return Some("too many rpc calls");
    }
    // message bus sends and topic publishes share the scene's per-tick send budget
    let mut sends = 0;
    for call in calls {
        match call {
            RpcCall::SendMessageBus { data, .. } => {
                sends += 1;
                // the message plus its one-byte type tag
                if data.len() > MAX_COMMS_MESSAGE_BYTES + 1 {
                    return Some("message bus send too large");
                }
            }
            RpcCall::PublishSceneTopic { data } => {
                sends += 1;
                if data.len() > MAX_TOPIC_MESSAGE_BYTES {
                    return Some("topic publish too large");
                }
            }
            _ => (),
        }
    }
    if sends > MAX_SEND_MESSAGES_PER_TICK {
        return Some("too many message bus sends");
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bus(len: usize) -> RpcCall {
        RpcCall::SendMessageBus {
            data: vec![0; len],
            recipient: None,
        }
    }

    fn topic(len: usize) -> RpcCall {
        RpcCall::PublishSceneTopic { data: vec![0; len] }
    }

    #[test]
    fn sends_at_every_limit_pass() {
        let mut calls: Vec<_> = (0..MAX_SEND_MESSAGES_PER_TICK / 2)
            .map(|_| bus(MAX_COMMS_MESSAGE_BYTES + 1))
            .collect();
        calls.extend(
            (calls.len()..MAX_SEND_MESSAGES_PER_TICK).map(|_| topic(MAX_TOPIC_MESSAGE_BYTES)),
        );
        assert_eq!(tick_limit_broken(&calls), None);
    }

    #[test]
    fn an_oversized_topic_publish_breaks_the_limit() {
        assert_eq!(
            tick_limit_broken(&[topic(MAX_TOPIC_MESSAGE_BYTES + 1)]),
            Some("topic publish too large")
        );
    }

    #[test]
    fn an_oversized_message_bus_send_breaks_the_limit() {
        assert_eq!(
            tick_limit_broken(&[bus(MAX_COMMS_MESSAGE_BYTES + 2)]),
            Some("message bus send too large")
        );
    }

    #[test]
    fn bus_sends_and_topic_publishes_share_the_send_budget() {
        let mut calls: Vec<_> = (0..MAX_SEND_MESSAGES_PER_TICK / 2)
            .map(|_| bus(1))
            .collect();
        calls.extend((calls.len()..=MAX_SEND_MESSAGES_PER_TICK).map(|_| topic(1)));
        assert_eq!(
            tick_limit_broken(&calls),
            Some("too many message bus sends")
        );
    }

    #[test]
    fn too_many_rpc_calls_break_the_limit() {
        let calls: Vec<_> = (0..=MAX_RPC_CALLS_PER_TICK).map(|_| topic(1)).collect();
        assert_eq!(tick_limit_broken(&calls), Some("too many rpc calls"));
    }
}

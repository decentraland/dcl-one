use super::*;
use common::rpc::{
    rmp_encode, IpcMessage, RequestContext, ResponseContext, ENGINE_IPC_CONTEXT, SCENE_IPC_CONTEXT,
};
use serde_json::json;
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver};

struct Bridge {
    runtime: tokio::runtime::Runtime,
    replies: UnboundedReceiver<(u64, IpcMessage)>,
    closes: UnboundedReceiver<u64>,
}
impl Bridge {
    fn new() -> Self {
        let (close_sender, closes) = unbounded_channel();
        let (ipc_router, replies) = unbounded_channel();
        SCENE_IPC_CONTEXT.set(Some(RequestContext {
            registry: Default::default(),
            close_sender,
            next_id: 1,
        }));
        ENGINE_IPC_CONTEXT.set(Some(ResponseContext {
            ipc_channel_registry: Default::default(),
            ipc_router,
            max_channels: Some(4096),
            parent_token: None,
        }));
        Self {
            runtime: tokio::runtime::Builder::new_current_thread()
                .build()
                .unwrap(),
            replies,
            closes,
        }
    }
    fn decode(&self, call: &RpcCall) -> (RpcCall, usize) {
        let _guard = self.runtime.enter();
        let bytes = rmp_encode(call).unwrap();
        (rmp_serde::from_slice(&bytes).unwrap(), bytes.len())
    }
    fn admit(&self, window: &mut ImmediateRpcWindow, call: RpcCall) {
        let (remote, bytes) = self.decode(&call);
        window
            .admit(remote, bytes, Self::open, Self::exists)
            .unwrap();
    }
    fn exists(id: u64) -> bool {
        ENGINE_IPC_CONTEXT.with_borrow(|context| {
            context
                .as_ref()
                .unwrap()
                .ipc_channel_registry
                .contains_key(&id)
        })
    }
    fn open(id: u64) -> bool {
        ENGINE_IPC_CONTEXT.with_borrow(|context| {
            context
                .as_ref()
                .unwrap()
                .ipc_channel_registry
                .get(&id)
                .is_some_and(|token| !token.is_cancelled())
        })
    }
    fn pump(&mut self) {
        for _ in 0..6 {
            self.runtime.block_on(async {
                for _ in 0..8 {
                    tokio::task::yield_now().await;
                }
            });
            while let Ok(id) = self.closes.try_recv() {
                SCENE_IPC_CONTEXT.with_borrow_mut(|context| {
                    context.as_mut().unwrap().registry.remove(&id);
                });
                ENGINE_IPC_CONTEXT.with_borrow(|context| {
                    if let Some(token) = context.as_ref().unwrap().ipc_channel_registry.get(&id) {
                        token.cancel();
                    }
                });
            }
            while let Ok((id, message)) = self.replies.try_recv() {
                if matches!(message, IpcMessage::Closed) {
                    ENGINE_IPC_CONTEXT.with_borrow_mut(|context| {
                        context.as_mut().unwrap().ipc_channel_registry.remove(&id);
                    });
                }
                SCENE_IPC_CONTEXT.with_borrow_mut(|context| {
                    let context = context.as_mut().unwrap();
                    match message {
                        IpcMessage::Data(bytes) => {
                            if let Some(endpoint) = context.registry.get_mut(&id) {
                                endpoint.send(bytes);
                            }
                        }
                        IpcMessage::Closed => {
                            context.registry.remove(&id);
                        }
                    }
                });
            }
        }
    }
}
impl Drop for Bridge {
    fn drop(&mut self) {
        ENGINE_IPC_CONTEXT.with_borrow_mut(|context| {
            if let Some(context) = context.take() {
                for token in context.ipc_channel_registry.values() {
                    token.cancel();
                }
            }
        });
        SCENE_IPC_CONTEXT.set(None);
    }
}

#[test]
fn serialized_subscribe_and_pulls_share_one_lifetime_while_slow_signing_allows_reads() {
    let mut bridge = Bridge::new();
    let mut window = ImmediateRpcWindow::default();
    let (sender, lifetime) = RpcStreamSender::channel_with_capacity(1);
    bridge.admit(&mut window, RpcCall::SubscribeLegacyAvatarEvents { sender });
    let RpcCall::SubscribeLegacyAvatarEvents {
        sender: subscription,
    } = window.next(Bridge::open, Bridge::exists).unwrap()
    else {
        panic!()
    };
    let (response, mut signed) = RpcResultSender::channel();
    bridge.admit(
        &mut window,
        RpcCall::SignLegacyMessage {
            message: "name: test".into(),
            response,
        },
    );
    let RpcCall::SignLegacyMessage { response: slow, .. } =
        window.next(Bridge::open, Bridge::exists).unwrap()
    else {
        panic!()
    };
    let (response, mut account) = RpcResultSender::channel();
    bridge.admit(&mut window, RpcCall::GetUserAccount { response });
    let RpcCall::GetUserAccount { response } = window.next(Bridge::open, Bridge::exists).unwrap()
    else {
        panic!()
    };
    response.send(Some("0xabc".into()));
    drop(response);
    for _ in 0..2 {
        let (response, mut pull) = RpcResultSender::channel();
        bridge.admit(&mut window, RpcCall::PullLegacyAvatarEvents { response });
        let RpcCall::PullLegacyAvatarEvents { response } =
            window.next(Bridge::open, Bridge::exists).unwrap()
        else {
            panic!()
        };
        response.send(Vec::new());
        drop(response);
        bridge.pump();
        assert!(pull.try_recv().unwrap().is_empty());
    }
    assert_eq!(account.try_recv().unwrap(), Some("0xabc".into()));
    assert!(matches!(
        signed.try_recv(),
        Err(tokio::sync::oneshot::error::TryRecvError::Empty)
    ));
    assert!(!subscription.is_closed());
    slow.send(Ok(json!("approved")));
    drop(slow);
    bridge.pump();
    assert_eq!(signed.try_recv().unwrap(), Ok(json!("approved")));
    drop(lifetime);
    bridge.pump();
    assert!(subscription.is_closed());
    drop(subscription);
    bridge.pump();
    assert_eq!(
        ENGINE_IPC_CONTEXT.with_borrow(|c| c.as_ref().unwrap().ipc_channel_registry.len()),
        0
    );
}

#[test]
fn full_window_processes_cancellation_but_waits_for_engine_owned_work_to_end() {
    let mut bridge = Bridge::new();
    let mut window = ImmediateRpcWindow::default();
    let mut receivers = Vec::new();
    let mut active = Vec::new();
    for _ in 0..MAX_IMMEDIATE_IN_FLIGHT {
        let (response, receiver) = RpcResultSender::channel();
        receivers.push(receiver);
        bridge.admit(&mut window, RpcCall::GetUserAccount { response });
        active.push(window.next(Bridge::open, Bridge::exists).unwrap());
    }
    let (response, _last) = RpcResultSender::channel();
    bridge.admit(&mut window, RpcCall::GetUserAccount { response });
    assert!(window.next(Bridge::open, Bridge::exists).is_none());
    drop(receivers.remove(0));
    bridge.pump();
    assert!(
        window.next(Bridge::open, Bridge::exists).is_none(),
        "cancelling a caller does not end engine-owned work"
    );
    assert_eq!(window.outstanding.len(), MAX_IMMEDIATE_IN_FLIGHT);
    drop(active.remove(0));
    bridge.pump();
    assert!(window.next(Bridge::open, Bridge::exists).is_some());
    assert_eq!(window.outstanding.len(), MAX_IMMEDIATE_IN_FLIGHT);
    assert!(window.next(Bridge::open, Bridge::exists).is_none());
    drop(active);
    drop(receivers);
    bridge.pump();
    window.next(Bridge::open, Bridge::exists);
    assert!(window.outstanding.is_empty());
}

#[test]
fn file_reads_keep_their_barrier_and_teardown_drops_pending_replies() {
    let mut bridge = Bridge::new();
    let mut window = ImmediateRpcWindow::default();
    let (response, mut file) = RpcResultSender::channel();
    bridge.admit(
        &mut window,
        RpcCall::ReadFile {
            filename: "main.js".into(),
            response,
        },
    );
    let RpcCall::ReadFile { response, .. } = window.next(Bridge::open, Bridge::exists).unwrap()
    else {
        panic!()
    };
    let (query, mut receiver) = RpcResultSender::channel();
    bridge.admit(&mut window, RpcCall::GetUserAccount { response: query });
    assert!(window.next(Bridge::open, Bridge::exists).is_none());
    response.send(Err("not found".into()));
    drop(response);
    bridge.pump();
    assert!(file.try_recv().unwrap().is_err());
    window.clear();
    bridge.pump();
    assert!(matches!(
        receiver.try_recv(),
        Err(tokio::sync::oneshot::error::TryRecvError::Closed)
    ));
}

#[test]
fn decoded_duplicate_unknown_and_unapproved_channels_are_rejected() {
    let bridge = Bridge::new();
    let mut window = ImmediateRpcWindow::default();
    let (response, _receiver) = RpcResultSender::channel();
    let local = RpcCall::GetUserAccount { response };
    let (remote, bytes) = bridge.decode(&local);
    let id = endpoint(&remote).unwrap().0;
    window
        .admit(remote, bytes, Bridge::open, Bridge::exists)
        .unwrap();
    let _guard = bridge.runtime.enter();
    let encoded = rmp_encode(&local).unwrap();
    assert!(
        rmp_serde::from_slice::<RpcCall>(&encoded).is_err(),
        "live sender IDs cannot be decoded twice"
    );
    let (response, _receiver) = RpcResultSender::channel();
    let (remote, bytes) = bridge.decode(&RpcCall::GetUserAccount { response });
    ENGINE_IPC_CONTEXT.with_borrow_mut(|context| {
        context
            .as_mut()
            .unwrap()
            .ipc_channel_registry
            .remove(&endpoint(&remote).unwrap().0);
    });
    assert_eq!(
        window.admit(remote, bytes, Bridge::open, Bridge::exists),
        Err("unknown immediate channel")
    );
    assert!(window.ids.contains(&id));
    assert!(window
        .admit(
            RpcCall::PublishSceneTopic { data: vec![] },
            1,
            Bridge::open,
            Bridge::exists
        )
        .is_err());
}

#[test]
fn admission_and_pending_bytes_remain_bounded_across_tick_resets() {
    let bridge = Bridge::new();
    let mut window = ImmediateRpcWindow::default();
    let mut receivers = Vec::new();
    for _ in 0..MAX_RPC_CALLS_PER_TICK {
        let (response, receiver) = RpcResultSender::channel();
        receivers.push(receiver);
        bridge.admit(&mut window, RpcCall::GetUserAccount { response });
    }
    assert_eq!(window.tick(1), Err("too many rpc calls"));
    window.tick(0).unwrap();
    let (response, _receiver) = RpcResultSender::channel();
    let (remote, bytes) = bridge.decode(&RpcCall::GetUserAccount { response });
    assert_eq!(
        window.admit(remote, bytes, Bridge::open, Bridge::exists),
        Err("immediate request queue full")
    );
    window.clear();
    let (response, _receiver) = RpcResultSender::channel();
    let (remote, _) = bridge.decode(&RpcCall::GetUserAccount { response });
    assert_eq!(
        window.admit(
            remote,
            MAX_RETAINED_IMMEDIATE_BYTES + 1,
            Bridge::open,
            Bridge::exists
        ),
        Err("too many immediate rpc bytes per tick")
    );
}

#[test]
fn forwarded_and_caller_cancelled_requests_keep_their_byte_charge_until_engine_release() {
    let mut bridge = Bridge::new();
    let mut window = ImmediateRpcWindow::default();
    let request = |response| RpcCall::SignLegacyMessage {
        message: "x".repeat(MAX_RETAINED_IMMEDIATE_BYTES / 2),
        response,
    };
    let (response, caller) = RpcResultSender::channel();
    bridge.admit(&mut window, request(response));
    let held = window.next(Bridge::open, Bridge::exists).unwrap();
    window.tick(0).unwrap();
    assert!(window.retained_bytes > MAX_RETAINED_IMMEDIATE_BYTES / 2);
    drop(caller);
    bridge.pump();
    window.next(Bridge::open, Bridge::exists);
    assert_eq!(
        window.outstanding.len(),
        1,
        "caller cancellation does not release engine-owned work"
    );
    let (response, _caller) = RpcResultSender::channel();
    let (remote, bytes) = bridge.decode(&request(response));
    assert_eq!(
        window.admit(remote, bytes, Bridge::open, Bridge::exists),
        Err("immediate request queue full")
    );
    assert!(
        window.retained_bytes > MAX_RETAINED_IMMEDIATE_BYTES / 2,
        "the engine still owns the original request"
    );
    drop(held);
    bridge.pump();
    let (response, _caller) = RpcResultSender::channel();
    let (remote, bytes) = bridge.decode(&request(response));
    window
        .admit(remote, bytes, Bridge::open, Bridge::exists)
        .unwrap();
}

#[test]
fn cancelling_a_file_caller_does_not_release_engine_owned_file_work() {
    let mut bridge = Bridge::new();
    let mut window = ImmediateRpcWindow::default();
    let (response, file_caller) = RpcResultSender::channel();
    bridge.admit(
        &mut window,
        RpcCall::ReadFile {
            filename: "main.js".into(),
            response,
        },
    );
    let held_file = window.next(Bridge::open, Bridge::exists).unwrap();
    let (response, _query_caller) = RpcResultSender::channel();
    bridge.admit(&mut window, RpcCall::GetUserAccount { response });
    drop(file_caller);
    bridge.pump();
    assert!(window.file_pending(Bridge::exists));
    assert!(window.next(Bridge::open, Bridge::exists).is_none());
    drop(held_file);
    bridge.pump();
    assert!(window.next(Bridge::open, Bridge::exists).is_some());
    assert!(!window.file_pending(Bridge::exists));
}

#[test]
fn renderer_backpressure_preserves_subscription_and_queries_without_readmission() {
    let mut bridge = Bridge::new();
    let mut window = ImmediateRpcWindow::default();
    let (tx, mut rx) = tokio::sync::mpsc::channel(1);
    let renderer = SceneResponseSender::new(dcl_types::SceneId::DUMMY, tx);
    renderer
        .try_send(SceneResponse::Error("occupied".into()))
        .unwrap();
    let (sender, lifetime) = RpcStreamSender::channel_with_capacity(1);
    bridge.admit(&mut window, RpcCall::SubscribeLegacyAvatarEvents { sender });
    let (response, mut account) = RpcResultSender::channel();
    bridge.admit(&mut window, RpcCall::GetUserAccount { response });
    let charged = window.retained_bytes;
    for _ in 0..3 {
        assert_eq!(
            window.forward_next(&renderer, Bridge::open, Bridge::exists),
            Ok(ImmediateDispatch::Blocked)
        );
        assert_eq!(window.pending.len(), 2);
        assert_eq!(window.admitted, 2);
        assert_eq!(window.retained_bytes, charged);
        assert!(window.outstanding.is_empty());
    }
    rx.try_recv().unwrap();
    assert_eq!(
        window.forward_next(&renderer, Bridge::open, Bridge::exists),
        Ok(ImmediateDispatch::Forwarded)
    );
    assert_eq!(
        window.forward_next(&renderer, Bridge::open, Bridge::exists),
        Ok(ImmediateDispatch::Blocked)
    );
    assert!(window.outstanding.is_empty());
    let (_, SceneResponse::ImmediateRpcCall(RpcCall::SubscribeLegacyAvatarEvents { sender })) =
        rx.try_recv().unwrap()
    else {
        panic!()
    };
    assert!(!sender.is_closed());
    assert_eq!(
        window.forward_next(&renderer, Bridge::open, Bridge::exists),
        Ok(ImmediateDispatch::Forwarded)
    );
    let (_, SceneResponse::ImmediateRpcCall(RpcCall::GetUserAccount { response })) =
        rx.try_recv().unwrap()
    else {
        panic!()
    };
    response.send(Some("0xabc".into()));
    drop(response);
    bridge.pump();
    assert_eq!(account.try_recv().unwrap(), Some("0xabc".into()));
    assert_eq!(
        window.forward_next(&renderer, Bridge::open, Bridge::exists),
        Ok(ImmediateDispatch::Idle)
    );
    drop(lifetime);
    bridge.pump();
    assert!(sender.is_closed());
    let (response, _receiver) = RpcResultSender::channel();
    bridge.admit(&mut window, RpcCall::GetUserAccount { response });
    drop(rx);
    assert_eq!(
        window.forward_next(&renderer, Bridge::open, Bridge::exists),
        Err("renderer response channel closed")
    );
}

#[test]
fn completed_engine_work_does_not_refund_the_per_flush_byte_budget() {
    let mut bridge = Bridge::new();
    let mut window = ImmediateRpcWindow::default();
    let (response, _receiver) = RpcResultSender::channel();
    let (remote, _) = bridge.decode(&RpcCall::GetUserAccount { response });
    // Admission receives the full encoded frame length from the trusted relay.
    window
        .admit(
            remote,
            MAX_RETAINED_IMMEDIATE_BYTES,
            Bridge::open,
            Bridge::exists,
        )
        .unwrap();
    drop(window.next(Bridge::open, Bridge::exists));
    bridge.pump();
    window.next(Bridge::open, Bridge::exists);
    assert_eq!(window.retained_bytes, 0);
    let (response, _receiver) = RpcResultSender::channel();
    let (remote, bytes) = bridge.decode(&RpcCall::GetUserAccount { response });
    assert_eq!(
        window.admit(remote, bytes, Bridge::open, Bridge::exists),
        Err("too many immediate rpc bytes per tick")
    );
    window.tick(0).unwrap();
    let (response, _receiver) = RpcResultSender::channel();
    bridge.admit(&mut window, RpcCall::GetUserAccount { response });
    let held = window.next(Bridge::open, Bridge::exists).unwrap();
    let retained = window.retained_bytes;
    window.tick(0).unwrap();
    assert_eq!(window.admitted_bytes, 0);
    assert_eq!(window.retained_bytes, retained);
    drop(held);
}

#[test]
fn file_barrier_is_armed_only_after_renderer_accepts_the_request() {
    let bridge = Bridge::new();
    let mut window = ImmediateRpcWindow::default();
    let (tx, mut rx) = tokio::sync::mpsc::channel(1);
    let renderer = SceneResponseSender::new(dcl_types::SceneId::DUMMY, tx);
    renderer
        .try_send(SceneResponse::Error("occupied".into()))
        .unwrap();
    let (response, _receiver) = RpcResultSender::channel();
    bridge.admit(
        &mut window,
        RpcCall::ReadFile {
            filename: "main.js".into(),
            response,
        },
    );
    assert_eq!(
        window.forward_next(&renderer, Bridge::open, Bridge::exists),
        Ok(ImmediateDispatch::Blocked)
    );
    assert!(!window.file_pending(Bridge::exists));
    assert_eq!(window.pending.len(), 1);
    rx.try_recv().unwrap();
    assert_eq!(
        window.forward_next(&renderer, Bridge::open, Bridge::exists),
        Ok(ImmediateDispatch::Forwarded)
    );
    assert!(window.file_pending(Bridge::exists));
    assert!(window.pending.is_empty());
    assert!(matches!(
        rx.try_recv().unwrap().1,
        SceneResponse::ImmediateRpcCall(RpcCall::ReadFile { .. })
    ));
}

#[test]
fn an_immediate_request_filling_the_renderer_queue_does_not_lose_the_following_tick() {
    let bridge = Bridge::new();
    let mut window = ImmediateRpcWindow::default();
    let (tx, mut rx) = tokio::sync::mpsc::channel(1);
    let renderer = SceneResponseSender::new(dcl_types::SceneId::DUMMY, tx);
    let (sender, _lifetime) = RpcStreamSender::channel_with_capacity(1);
    bridge.admit(&mut window, RpcCall::SubscribeLegacyAvatarEvents { sender });
    assert_eq!(
        window.forward_next(&renderer, Bridge::open, Bridge::exists),
        Ok(ImmediateDispatch::Forwarded)
    );
    assert_eq!(
        window.forward_next(&renderer, Bridge::open, Bridge::exists),
        Ok(ImmediateDispatch::Idle)
    );
    let tick = SceneResponse::Ok(
        dcl_types::SceneCensus {
            born: Default::default(),
            died: Default::default(),
        },
        Default::default(),
        dcl_types::SceneElapsedTime(42.0),
        Vec::new(),
        Default::default(),
    );
    let mut credits = 1;
    let mut held = None;
    assert_eq!(
        forward_renderer_response(&renderer, tick, &mut credits, &mut held),
        Ok(false)
    );
    assert_eq!(credits, 1);
    assert!(held.is_some());
    let subscription = rx.try_recv().unwrap();
    assert!(matches!(
        subscription.1,
        SceneResponse::ImmediateRpcCall(RpcCall::SubscribeLegacyAvatarEvents { .. })
    ));
    assert_eq!(
        forward_renderer_response(&renderer, held.take().unwrap(), &mut credits, &mut held),
        Ok(true)
    );
    assert_eq!(credits, 0);
    let (_, SceneResponse::Ok(_, _, elapsed, _, _)) = rx.try_recv().unwrap() else {
        panic!()
    };
    assert_eq!(elapsed.0, 42.0);
    assert!(held.is_none());
    drop(rx);
    credits = 1;
    assert_eq!(
        forward_renderer_response(
            &renderer,
            SceneResponse::Error("closed".into()),
            &mut credits,
            &mut held
        ),
        Err("renderer response channel closed")
    );
    assert_eq!(credits, 1);
}

#[test]
fn saturated_renderer_retains_snapshot_allocation_and_terminal_error_without_tick_credit() {
    let (tx, mut rx) = tokio::sync::mpsc::channel(1);
    let renderer = SceneResponseSender::new(dcl_types::SceneId::DUMMY, tx);
    let responses = [
        SceneResponse::CrdtSnapshot(Default::default()),
        SceneResponse::EntityAllocated(Vec::new()),
        SceneResponse::Error("scene ended".into()),
    ];
    let mut credits = 0;
    let mut held = None;
    for response in responses {
        let expected = std::mem::discriminant(&response);
        renderer
            .try_send(SceneResponse::Error("occupied".into()))
            .unwrap();
        assert_eq!(
            forward_renderer_response(&renderer, response, &mut credits, &mut held),
            Ok(false)
        );
        assert!(held.is_some());
        rx.try_recv().unwrap();
        assert_eq!(
            forward_renderer_response(&renderer, held.take().unwrap(), &mut credits, &mut held),
            Ok(true)
        );
        let received = rx.try_recv().unwrap().1;
        assert_eq!(std::mem::discriminant(&received), expected);
        if let SceneResponse::Error(message) = received {
            assert_eq!(message, "scene ended");
        }
        assert_eq!(credits, 0);
        assert!(held.is_none());
    }
}

#[test]
fn startup_reads_exceed_one_thousand_without_spending_action_calls() {
    let mut bridge = Bridge::new();
    let mut window = ImmediateRpcWindow::default();
    for index in 0..MAX_RPC_CALLS_PER_TICK + 1 {
        let (response, mut caller) = RpcResultSender::channel();
        bridge.admit(
            &mut window,
            RpcCall::ReadFile {
                filename: format!("asset-{index}"),
                response,
            },
        );
        let RpcCall::ReadFile { response, .. } = window.next(Bridge::open, Bridge::exists).unwrap()
        else {
            panic!()
        };
        response.send(Ok(common::rpc::ReadFileResponse {
            content: vec![1],
            hash: index.to_string(),
        }));
        drop(response);
        bridge.pump();
        assert_eq!(caller.try_recv().unwrap().unwrap().content, vec![1]);
    }
    assert_eq!(window.admitted, 0);
    assert!(window.admitted_bytes > MAX_RPC_CALLS_PER_TICK);
    let (response, _caller) = RpcResultSender::channel();
    let (remote, _) = bridge.decode(&RpcCall::ReadFile {
        filename: "over-budget".into(),
        response,
    });
    assert_eq!(
        window.admit(
            remote,
            MAX_RETAINED_IMMEDIATE_BYTES,
            Bridge::open,
            Bridge::exists
        ),
        Err("too many immediate rpc bytes per tick")
    );
    window.tick(MAX_RPC_CALLS_PER_TICK).unwrap();
}

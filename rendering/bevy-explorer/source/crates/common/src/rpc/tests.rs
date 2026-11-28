use super::*;
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver};

// both ends of the ipc bridge on one thread: the scene host serializes senders into
// SCENE_IPC_CONTEXT, the engine deserializes them against ENGINE_IPC_CONTEXT
struct Bridge {
    rt: tokio::runtime::Runtime,
    close_rx: UnboundedReceiver<u64>,
    router_rx: UnboundedReceiver<(u64, IpcMessage)>,
}

impl Bridge {
    fn new() -> Self {
        let (close_sender, close_rx) = unbounded_channel();
        SCENE_IPC_CONTEXT.set(Some(RequestContext {
            registry: Default::default(),
            close_sender,
            next_id: 1,
        }));
        let (ipc_router, router_rx) = unbounded_channel();
        ENGINE_IPC_CONTEXT.set(Some(ResponseContext {
            ipc_channel_registry: Default::default(),
            ipc_router,
            max_channels: None,
            parent_token: None,
        }));
        let rt = tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap();
        Self {
            rt,
            close_rx,
            router_rx,
        }
    }

    // let spawned watcher tasks run
    fn settle(&self) {
        self.rt.block_on(async {
            for _ in 0..10 {
                tokio::task::yield_now().await;
            }
        });
    }

    // deliver engine -> scene messages the way renderer_ipc_out and the scene host do
    fn route_to_scene(&mut self) {
        while let Ok((id, msg)) = self.router_rx.try_recv() {
            if matches!(msg, IpcMessage::Closed) {
                ENGINE_IPC_CONTEXT
                    .with_borrow_mut(|ctx| ctx.as_mut().unwrap().ipc_channel_registry.remove(&id));
            }
            SCENE_IPC_CONTEXT.with_borrow_mut(|ctx| {
                let ctx = ctx.as_mut().unwrap();
                match msg {
                    IpcMessage::Data(data) => {
                        if let Some(endpoint) = ctx.registry.get_mut(&id) {
                            endpoint.send(data);
                        }
                    }
                    IpcMessage::Closed => {
                        ctx.registry.remove(&id);
                    }
                }
            });
        }
    }

    // deliver scene -> engine closes the way scene_ipc_out and renderer_ipc_in do,
    // returning how many actually crossed the pipe
    fn route_to_engine(&mut self) -> usize {
        let mut sent = 0;
        while let Ok(id) = self.close_rx.try_recv() {
            let was_open = SCENE_IPC_CONTEXT
                .with_borrow_mut(|ctx| ctx.as_mut().unwrap().registry.remove(&id).is_some());
            if was_open {
                sent += 1;
                ENGINE_IPC_CONTEXT.with_borrow_mut(|ctx| {
                    if let Some(token) = ctx.as_mut().unwrap().ipc_channel_registry.remove(&id) {
                        token.cancel();
                    }
                });
            }
        }
        sent
    }

    fn scene_endpoints(&self) -> usize {
        SCENE_IPC_CONTEXT.with_borrow(|ctx| ctx.as_ref().unwrap().registry.len())
    }

    fn engine_tokens(&self) -> usize {
        ENGINE_IPC_CONTEXT.with_borrow(|ctx| ctx.as_ref().unwrap().ipc_channel_registry.len())
    }

    // each close watcher task holds a clone of the context's close sender
    fn close_watchers(&self) -> usize {
        SCENE_IPC_CONTEXT.with_borrow(|ctx| ctx.as_ref().unwrap().close_sender.strong_count() - 1)
    }
}

#[test]
fn result_channel_is_released_on_both_sides_after_a_reply() {
    let mut bridge = Bridge::new();
    let _guard = bridge.rt.enter();

    let (sender, mut receiver) = RpcResultSender::<u32>::channel();
    let bytes = rmp_encode(&sender).unwrap();
    let remote: RpcResultSender<u32> = rmp_serde::from_slice(&bytes).unwrap();
    assert_eq!(bridge.scene_endpoints(), 1);
    assert_eq!(bridge.engine_tokens(), 1);
    assert_eq!(bridge.close_watchers(), 1);

    remote.send(7);
    drop(remote);
    bridge.settle();
    bridge.route_to_scene();
    bridge.settle();

    assert_eq!(receiver.try_recv().unwrap(), 7);
    assert_eq!(bridge.scene_endpoints(), 0);
    assert_eq!(bridge.engine_tokens(), 0);
    assert_eq!(bridge.close_watchers(), 0);
    // the watcher's close stays local: the endpoint is already gone
    assert_eq!(bridge.route_to_engine(), 0);

    drop(receiver);
    bridge.settle();
    assert_eq!(bridge.route_to_engine(), 0);
}

#[test]
fn stream_channel_is_released_on_both_sides_when_the_engine_closes_it() {
    let mut bridge = Bridge::new();
    let _guard = bridge.rt.enter();

    let (sender, mut receiver) = RpcStreamSender::<u32>::channel();
    let bytes = rmp_encode(&sender).unwrap();
    let remote: RpcStreamSender<u32> = rmp_serde::from_slice(&bytes).unwrap();

    remote.send(1).unwrap();
    remote.send(2).unwrap();
    drop(remote);
    bridge.settle();
    bridge.route_to_scene();
    bridge.settle();

    assert_eq!(receiver.try_recv().unwrap(), 1);
    assert_eq!(receiver.try_recv().unwrap(), 2);
    assert_eq!(bridge.scene_endpoints(), 0);
    assert_eq!(bridge.engine_tokens(), 0);
    assert_eq!(bridge.close_watchers(), 0);
    assert_eq!(bridge.route_to_engine(), 0);
}

#[test]
fn dropping_a_pending_result_receiver_cancels_the_remote_reply() {
    let mut bridge = Bridge::new();
    let _guard = bridge.rt.enter();
    let (local, receiver) = RpcResultSender::<bool>::channel();
    let remote: RpcResultSender<bool> =
        rmp_serde::from_slice(&rmp_encode(&local).unwrap()).unwrap();
    assert!(!remote.is_cancelled());
    drop(receiver);
    assert!(local.is_cancelled());
    bridge.settle();
    assert_eq!(bridge.route_to_engine(), 1);
    assert!(remote.is_cancelled());
}

#[test]
fn dropping_the_receiver_early_still_notifies_the_engine() {
    let mut bridge = Bridge::new();
    let _guard = bridge.rt.enter();

    let (sender, receiver) = RpcStreamSender::<u32>::channel();
    let bytes = rmp_encode(&sender).unwrap();
    let remote: RpcStreamSender<u32> = rmp_serde::from_slice(&bytes).unwrap();

    drop(receiver);
    bridge.settle();

    assert_eq!(bridge.close_watchers(), 0);
    assert_eq!(bridge.route_to_engine(), 1);
    assert!(remote.is_closed());
    assert_eq!(bridge.scene_endpoints(), 0);
    assert_eq!(bridge.engine_tokens(), 0);

    // the engine's own close afterwards finds nothing left on either side
    drop(remote);
    bridge.settle();
    bridge.route_to_scene();
    bridge.settle();
    assert_eq!(bridge.route_to_engine(), 0);
}

#[derive(serde::Serialize, serde::Deserialize)]
enum LegacySignRequest {
    SignRequest {
        method: String,
        uri: String,
        response: RpcResultSender<Result<Vec<(String, String)>, String>>,
    },
}

#[test]
fn bodyless_sign_request_preserves_legacy_wire() {
    let bridge = Bridge::new();
    let _guard = bridge.rt.enter();
    let (response, _rx) = RpcResultSender::channel();
    let legacy = LegacySignRequest::SignRequest {
        method: "POST".into(),
        uri: "https://example.org/".into(),
        response: response.clone(),
    };
    let bytes = rmp_encode(&legacy).unwrap();
    let same_call = RpcCall::SignRequest(SignRequest {
        method: "POST".into(),
        uri: "https://example.org/".into(),
        response,
        body_hash: None,
    });
    assert_eq!(rmp_encode(&same_call).unwrap(), bytes);
    let current: RpcCall = rmp_serde::from_slice(&bytes).unwrap();
    let RpcCall::SignRequest(SignRequest { body_hash, .. }) = current else {
        panic!("wrong variant")
    };
    assert_eq!(body_hash, None);
    let (response, _rx) = RpcResultSender::channel();
    let current = RpcCall::SignRequest(SignRequest {
        method: "POST".into(),
        uri: "https://example.org/".into(),
        response,
        body_hash: None,
    });
    let bytes = rmp_encode(&current).unwrap();
    let decoded: LegacySignRequest = rmp_serde::from_slice(&bytes).unwrap();
    let LegacySignRequest::SignRequest { method, uri, .. } = decoded;
    assert_eq!(
        (method.as_str(), uri.as_str()),
        ("POST", "https://example.org/")
    );
}

#[test]
fn legacy_signer_can_ignore_the_named_body_hash() {
    let bridge = Bridge::new();
    let _guard = bridge.rt.enter();
    let (response, _rx) = RpcResultSender::channel();
    let bodyful = RpcCall::SignRequest(SignRequest {
        method: "POST".into(),
        uri: "https://example.org/".into(),
        response,
        body_hash: Some("a".repeat(64)),
    });
    let bytes = rmp_encode(&bodyful).unwrap();
    let wire: serde_json::Value = rmp_serde::from_slice(&bytes).unwrap();
    let fields = wire["SignRequest"].as_object().unwrap();
    assert_eq!(fields.len(), 4);
    assert_eq!(fields["body_hash"], "a".repeat(64));
    assert!(rmp_serde::from_slice::<LegacySignRequest>(&bytes).is_ok());
}

#[test]
fn sign_request_body_hash_crosses_messagepack_bridge() {
    let mut bridge = Bridge::new();
    let _guard = bridge.rt.enter();
    let digest = "a".repeat(64);
    for body_hash in [None, Some(digest.as_str())] {
        let (response, mut rx) = RpcResultSender::channel();
        let call = RpcCall::SignRequest(SignRequest {
            method: "POST".into(),
            uri: "https://example.org/Path".into(),
            body_hash: body_hash.map(str::to_owned),
            response,
        });
        let bytes = rmp_encode(&call).unwrap();
        let decoded: RpcCall = rmp_serde::from_slice(&bytes).unwrap();
        match decoded {
            RpcCall::SignRequest(SignRequest {
                method,
                uri,
                body_hash: decoded_hash,
                response,
            }) => {
                assert_eq!(method, "POST");
                assert_eq!(uri, "https://example.org/Path");
                assert_eq!(decoded_hash.as_deref(), body_hash);
                response.send(Ok(vec![("x-identity-metadata".into(), "MixedCase".into())]));
            }
            _ => panic!("wrong RPC variant"),
        }
        bridge.settle();
        bridge.route_to_scene();
        bridge.settle();
        assert_eq!(rx.try_recv().unwrap().unwrap()[0].1, "MixedCase");
    }
}

/// The parts of a `MovePlayer` the mapping sets.
fn move_parts(call: RpcCall) -> (Vec3, Option<Vec3>, Option<f32>, Option<Quat>) {
    match call {
        RpcCall::MovePlayer {
            to,
            looking_at,
            duration,
            camera_rotation,
            ..
        } => (to, looking_at, duration, camera_rotation),
        _ => panic!("a move"),
    }
}

/// Where a camera rotation looks.
fn forward(rotation: Option<Quat>) -> Vec3 {
    rotation.expect("an aim") * Vec3::NEG_Z
}

#[test]
fn move_player_faces_the_avatar_target_and_aims_the_camera_from_the_new_position() {
    // Decentraland's z is bevy's -z
    let (to, looking_at, duration, camera_rotation) = move_parts(RpcCall::move_player(
        [8.0, 0.0, 8.0],
        None,
        None,
        Some(2.5),
        None,
    ));
    assert_eq!(to, Vec3::new(8.0, 0.0, -8.0));
    assert_eq!(
        (looking_at, duration, camera_rotation),
        (None, Some(2.5), None)
    );

    // a camera target alone also turns the avatar; the camera turns along the line from the new
    // position to it, so a point level with the feet ahead is straight ahead
    let (_, looking_at, _, camera_rotation) = move_parts(RpcCall::move_player(
        [8.0, 0.0, 8.0],
        Some([8.0, 0.0, 18.0]),
        None,
        None,
        None,
    ));
    assert_eq!(looking_at, Some(Vec3::new(8.0, 0.0, -18.0)));
    let ahead = forward(camera_rotation);
    assert!(ahead.distance(Vec3::NEG_Z) < 1e-5, "{ahead}");

    // an avatar target wins for the avatar, and the camera still turns to its own target
    let (_, looking_at, _, camera_rotation) = move_parts(RpcCall::move_player(
        [8.0, 0.0, 8.0],
        Some([8.0, 10.0, 18.0]),
        Some([18.0, 0.0, 8.0]),
        None,
        None,
    ));
    assert_eq!(looking_at, Some(Vec3::new(18.0, 0.0, -8.0)));
    let up_and_ahead = forward(camera_rotation);
    let expected = Vec3::new(0.0, 1.0, -1.0).normalize();
    assert!(up_and_ahead.distance(expected) < 1e-5, "{up_and_ahead}");
}

#[test]
fn move_player_aims_at_a_target_on_the_position_or_straight_above_it_without_nan() {
    for (target, looks) in [
        // no direction: the camera keeps a default facing
        ([8.0, 0.0, 8.0], None),
        // straight up: the camera looks up
        ([8.0, 10.0, 8.0], Some(Vec3::Y)),
    ] {
        let (_, _, _, camera_rotation) = move_parts(RpcCall::move_player(
            [8.0, 0.0, 8.0],
            Some(target),
            None,
            None,
            None,
        ));
        let rotation = camera_rotation.expect("an aim");
        assert!(
            rotation.is_finite() && rotation.is_normalized(),
            "{rotation}"
        );
        if let Some(looks) = looks {
            let towards = forward(camera_rotation);
            assert!(towards.distance(looks) < 1e-5, "{towards}");
        }
    }
}

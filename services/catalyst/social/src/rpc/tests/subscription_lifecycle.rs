//! Subscription lifecycle from `decentraland/social-service-ea` at
//! `b6b2cefefdf727b2ed47ddb887e77d226a41eea6`, `src/logic/updates.ts`
//! and `src/adapters/rpc-server/subscribers-context.ts`. The wire tags come from
//! `@dcl/protocol@1.0.0-29103578374.commit-3c4b54e`.

use super::*;
use crate::rpc::drpc::server::RpcServer;
use crate::rpc::drpc::transport::memory::MemoryTransport;

fn context_without_database() -> Context {
    test_ctx(
        Db::new(
            sqlx::postgres::PgPoolOptions::new()
                .connect_lazy("postgres://localhost/unused")
                .unwrap(),
        ),
        Arc::new(crate::comms::gatekeeper::voice::rooms::NoVoiceRuntime),
    )
}

struct MemoryClient {
    module: RpcClientModule<MemoryTransport>,
}

impl ServiceClient<MemoryTransport> for MemoryClient {
    fn set_client_module(module: RpcClientModule<MemoryTransport>) -> Self {
        Self { module }
    }
}

/// Every registered stream must encode its own final notice and then cleanly end;
/// rejected duplicates must not allocate another pubsub receiver.
#[tokio::test]
async fn duplicate_rpc_streams_send_the_pinned_final_notice() {
    let ctx = context_without_database();
    let me = addr("dup");
    ctx.register_identity(1, me.clone());
    let procedures = [
        "SubscribeToFriendshipUpdates",
        "SubscribeToFriendConnectivityUpdates",
        "SubscribeToBlockUpdates",
        "SubscribeToCommunityMemberConnectivityUpdates",
        "SubscribeToPrivateVoiceChatUpdates",
        "SubscribeToCommunityVoiceChatUpdates",
    ];
    // Reserve each first subscription without depending on database snapshots.
    let mut active: Vec<_> = procedures
        .iter()
        .map(|name| ctx.try_subscription(1, name).unwrap())
        .collect();
    let (client_transport, server_transport) = MemoryTransport::create();
    let mut server = RpcServer::create(ctx.clone());
    server.set_module_registrator_handler(crate::rpc::service::register_service);
    server
        .attach_transport(Arc::new(server_transport))
        .await
        .unwrap();
    let server_task = tokio::spawn(async move { server.run().await });
    let mut client = RpcClient::new(client_transport).await.unwrap();
    let remote = client
        .create_port("lifecycle")
        .await
        .unwrap()
        .load_module::<MemoryClient>(MODULE_NAME)
        .await
        .unwrap();

    macro_rules! check {
        ($procedure:literal, $message:ident, $event:literal, $tag:literal) => {{
            let mut updates: Generator<$message> = remote
                .module
                .call_server_streams_procedure($procedure, ())
                .await
                .unwrap();
            let update = tokio::time::timeout(Duration::from_secs(1), updates.next())
                .await
                .unwrap()
                .unwrap();
            let notice = SubscriptionStreamClosed {
                reason: SubscriptionStreamClosedReason::StreamClosedDuplicateSubscription as i32,
                message: Some(
                    concat!(
                        "This connection already has an active ",
                        $event,
                        " subscription"
                    )
                    .to_string(),
                ),
            };
            assert_eq!(
                update,
                $message {
                    stream_closed: Some(notice.clone()),
                    ..Default::default()
                }
            );
            // Independently pin the wire field number, including empty update data.
            let payload = notice.encode_to_vec();
            let mut expected = vec![($tag << 3) | 2, payload.len() as u8];
            expected.extend(payload);
            assert_eq!(update.encode_to_vec(), expected);
            let source: serde_json::Value = serde_json::from_str(include_str!(concat!(
                env!("DCL_ONE_ROOT"),
                "/services/catalyst/tests/fixtures/conformance/social/voice-source-cases.json"
            )))
            .unwrap();
            let capture = source["cases"]
                .as_array()
                .unwrap()
                .iter()
                .find(|case| {
                    case["group"] == "stream-closed"
                        && case["response_type"] == stringify!($message)
                })
                .unwrap();
            assert_eq!(
                hex::encode(update.encode_to_vec()),
                capture["expected_hex"].as_str().unwrap()
            );

            assert!(
                tokio::time::timeout(Duration::from_secs(1), updates.next())
                    .await
                    .unwrap()
                    .is_none()
            );
            assert!(
                ctx.try_subscription(1, $procedure).is_none(),
                "duplicate must not release the original guard"
            );
        }};
    }
    check!(
        "SubscribeToFriendshipUpdates",
        FriendshipUpdate,
        "friendshipUpdate",
        7
    );
    check!(
        "SubscribeToFriendConnectivityUpdates",
        FriendConnectivityUpdate,
        "friendConnectivityUpdate",
        3
    );
    check!("SubscribeToBlockUpdates", BlockUpdate, "blockUpdate", 3);
    check!(
        "SubscribeToCommunityMemberConnectivityUpdates",
        CommunityMemberConnectivityUpdate,
        "communityMemberConnectivityUpdate",
        4
    );
    check!(
        "SubscribeToPrivateVoiceChatUpdates",
        PrivateVoiceChatUpdate,
        "privateVoiceChatUpdate",
        6
    );
    check!(
        "SubscribeToCommunityVoiceChatUpdates",
        CommunityVoiceChatUpdate,
        "communityVoiceChatUpdate",
        10
    );
    assert_eq!(ctx.pubsub().subscriber_count(&me), 0);
    drop(active.remove(2));
    let mut original: Generator<BlockUpdate> = remote
        .module
        .call_server_streams_procedure("SubscribeToBlockUpdates", ())
        .await
        .unwrap();
    receivers(&ctx, &me, 1).await;
    check!("SubscribeToBlockUpdates", BlockUpdate, "blockUpdate", 3);
    assert_eq!(
        ctx.pubsub().subscriber_count(&me),
        1,
        "the original stays subscribed"
    );
    let update = BlockUpdate {
        address: addr("wire"),
        is_blocked: true,
        ..Default::default()
    };
    ctx.pubsub()
        .publish(&me, SocialEvent::Block(update.clone()));
    assert_eq!(
        tokio::time::timeout(Duration::from_secs(1), original.next())
            .await
            .unwrap(),
        Some(update)
    );
    ctx.forget_identity(1);
    receivers(&ctx, &me, 0).await;
    assert!(
        tokio::time::timeout(Duration::from_secs(1), original.next())
            .await
            .unwrap()
            .is_none()
    );
    drop(client);
    server_task.abort();
}

async fn idle_block_stream(ctx: &Context, me: &str, transport: u32) -> Generator<Vec<u8>> {
    let guard = ctx
        .try_subscription(transport, "SubscribeToBlockUpdates")
        .unwrap();
    let source = SocialServiceImpl
        .subscribe_to_block_updates(procedure_ctx(ctx, me, transport), guard.cancellation())
        .await
        .unwrap();
    Generator::from_generator(source, move |update| {
        let _ = &guard;
        Some(update.encode_to_vec())
    })
}

async fn receivers(ctx: &Context, me: &str, expected: usize) {
    tokio::time::timeout(Duration::from_secs(1), async {
        while ctx.pubsub().subscriber_count(me) != expected {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
}

#[tokio::test]
async fn dropping_an_idle_subscription_releases_its_guard_and_receiver() {
    let ctx = context_without_database();
    let me = addr("drop");
    let stream = idle_block_stream(&ctx, &me, 1).await;
    assert_eq!(ctx.pubsub().subscriber_count(&me), 1);
    drop(stream);
    receivers(&ctx, &me, 0).await;
    let _new = ctx
        .try_subscription(1, "SubscribeToBlockUpdates")
        .expect("stream may be reopened");
}

#[tokio::test]
async fn detaching_a_connection_cancels_its_idle_stream_and_keeps_its_sibling() {
    let ctx = context_without_database();
    let me = addr("detach");
    let mut first = idle_block_stream(&ctx, &me, 1).await;
    let mut sibling = idle_block_stream(&ctx, &me, 2).await;
    assert_eq!(ctx.pubsub().subscriber_count(&me), 2);
    ctx.forget_identity(1);
    receivers(&ctx, &me, 1).await;
    assert!(
        tokio::time::timeout(Duration::from_secs(1), first.next())
            .await
            .unwrap()
            .is_none()
    );
    assert!(ctx.try_subscription(2, "SubscribeToBlockUpdates").is_none());
    let update = BlockUpdate {
        address: addr("blocked"),
        is_blocked: true,
        ..Default::default()
    };
    ctx.pubsub()
        .publish(&me, SocialEvent::Block(update.clone()));
    let bytes = tokio::time::timeout(Duration::from_secs(1), sibling.next())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(BlockUpdate::decode(bytes.as_slice()).unwrap(), update);
    drop(sibling);
    receivers(&ctx, &me, 0).await;
}

#[tokio::test]
async fn a_detached_guard_cannot_clear_a_reused_transport_subscription() {
    let ctx = context_without_database();
    let old = ctx.try_subscription(1, "SubscribeToBlockUpdates").unwrap();
    ctx.forget_identity(1);
    assert!(old.cancellation().is_cancelled());
    let current = ctx.try_subscription(1, "SubscribeToBlockUpdates").unwrap();
    drop(old);
    assert!(!current.cancellation().is_cancelled());
    assert!(ctx.try_subscription(1, "SubscribeToBlockUpdates").is_none());
}

/// The pinned RPC update handler retrieves the actor's profile for every action
/// (`updates.ts:540-545`), while the JSON stream consumes the raw address event.
#[tokio::test]
async fn friendship_rpc_resolves_profiles_for_every_action_without_hiding_raw_events() {
    let Some(pool) = scratch_pool().await else {
        return;
    };
    let ctx = test_ctx(
        Db::new(pool.clone()),
        Arc::new(crate::comms::gatekeeper::voice::rooms::NoVoiceRuntime),
    );
    let me = addr("profile-stream");
    let known = addr("known");
    let missing = addr("missing");
    let entity = seed_profile(&pool, &known, "Current name").await;
    let cancellation = tokio_util::sync::CancellationToken::new();
    let mut rpc = SocialServiceImpl
        .subscribe_to_friendship_updates(procedure_ctx(&ctx, &me, 6002), cancellation.clone())
        .await
        .unwrap();
    let mut raw = ctx.pubsub().subscribe(&me);
    fn update(actor: &str, action: usize) -> FriendshipUpdate {
        use friendship_update::*;
        let user = Some(User {
            address: actor.into(),
        });
        let update = match action {
            0 => Update::Request(RequestResponse {
                friend: Some(FriendProfile {
                    address: actor.into(),
                    name: "stale name".into(),
                    ..Default::default()
                }),
                id: Uuid::new_v4().to_string(),
                created_at: 123,
                message: Some("hello".into()),
            }),
            1 => Update::Accept(AcceptResponse { user }),
            2 => Update::Reject(RejectResponse { user }),
            3 => Update::Delete(DeleteResponse { user }),
            _ => Update::Cancel(CancelResponse { user }),
        };
        FriendshipUpdate {
            update: Some(update),
            stream_closed: None,
        }
    }
    for action in 0..5 {
        ctx.pubsub()
            .publish(&me, SocialEvent::Friendship(update(&missing, action)));
        let expected = update(&known, action);
        ctx.pubsub()
            .publish(&me, SocialEvent::Friendship(expected.clone()));
        // Both events remain available to the JSON transport's raw subscriber.
        assert!(matches!(
            &*raw.recv().await.unwrap(),
            SocialEvent::Friendship(_)
        ));
        assert!(matches!(
            &*raw.recv().await.unwrap(),
            SocialEvent::Friendship(_)
        ));
        let actual = tokio::time::timeout(Duration::from_secs(3), rpc.next())
            .await
            .unwrap()
            .unwrap();
        if let Some(friendship_update::Update::Request(request)) = actual.update.as_ref() {
            assert_eq!(request.friend.as_ref().unwrap().address, known);
            assert_eq!(request.friend.as_ref().unwrap().name, "Current name");
            assert_eq!(request.created_at, 123);
            assert_eq!(request.message.as_deref(), Some("hello"));
            assert_eq!(
                request.id,
                match expected.update.unwrap() {
                    friendship_update::Update::Request(request) => request.id,
                    _ => unreachable!(),
                }
            );
        } else {
            assert_eq!(actual, expected);
        }
    }
    cancellation.cancel();
    assert!(
        tokio::time::timeout(Duration::from_secs(1), rpc.next())
            .await
            .unwrap()
            .is_none()
    );
    drop_profile(&pool, &known, &entity).await;
}

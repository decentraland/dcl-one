//! Friendships and blocks through the RPC handlers, the cases of upstream's
//! `rpc-server/services/*` and `logic/friends-logic.spec.ts`
//! (`decentraland/social-service-ea` at `fb3dcb8`) the socket test does not
//! reach: the invalid-input answers, mutual friends, every transition and
//! what the other party receives.

use tokio::sync::broadcast::Receiver;

use super::*;

struct Rig {
    db: Db,
    ctx: Context,
}

impl Rig {
    async fn new() -> Option<Self> {
        let pool = scratch_pool().await?;
        let db = Db::new(pool.clone());
        let ctx = test_ctx(db.clone(), Arc::new(FakeVoiceRooms::new(pool)));
        Some(Self { db, ctx })
    }

    fn as_(&self, me: &str) -> ProcedureContext<Context> {
        procedure_ctx(&self.ctx, me, rand_transport())
    }

    fn events(&self, address: &str) -> Receiver<Arc<SocialEvent>> {
        self.ctx.pubsub().subscribe(address)
    }

    async fn upsert(
        &self,
        me: &str,
        action: upsert_friendship_payload::Action,
    ) -> upsert_friendship_response::Response {
        let target = match &action {
            upsert_friendship_payload::Action::Request(p) => p.user.as_ref(),
            upsert_friendship_payload::Action::Accept(p) => p.user.as_ref(),
            upsert_friendship_payload::Action::Cancel(p) => p.user.as_ref(),
            upsert_friendship_payload::Action::Reject(p) => p.user.as_ref(),
            upsert_friendship_payload::Action::Delete(p) => p.user.as_ref(),
        }
        .map(|u| u.address.as_str())
        .unwrap_or_default();
        seed_profiles(&self.ctx, &[me, target]).await;
        SocialServiceImpl
            .upsert_friendship(
                UpsertFriendshipPayload {
                    action: Some(action),
                },
                self.as_(me),
            )
            .await
            .unwrap()
            .response
            .unwrap()
    }

    async fn status(
        &self,
        me: &str,
        other: Option<&str>,
    ) -> get_friendship_status_response::Response {
        SocialServiceImpl
            .get_friendship_status(
                GetFriendshipStatusPayload {
                    user: other.map(|a| User {
                        address: a.to_string(),
                    }),
                },
                self.as_(me),
            )
            .await
            .unwrap()
            .response
            .unwrap()
    }

    async fn block(&self, me: &str, other: &str) -> block_user_response::Response {
        SocialServiceImpl
            .block_user(
                BlockUserPayload {
                    user: Some(User {
                        address: other.to_string(),
                    }),
                },
                self.as_(me),
            )
            .await
            .unwrap()
            .response
            .unwrap()
    }

    async fn unblock(&self, me: &str, other: &str) -> unblock_user_response::Response {
        SocialServiceImpl
            .unblock_user(
                UnblockUserPayload {
                    user: Some(User {
                        address: other.to_string(),
                    }),
                },
                self.as_(me),
            )
            .await
            .unwrap()
            .response
            .unwrap()
    }

    async fn friends_of(&self, me: &str) -> Vec<String> {
        SocialServiceImpl
            .get_friends(GetFriendsPayload { pagination: None }, self.as_(me))
            .await
            .unwrap()
            .friends
            .into_iter()
            .map(|f| f.address)
            .collect()
    }

    async fn befriend(&self, a: &str, b: &str) {
        seed_profiles(&self.ctx, &[a, b]).await;
        self.db
            .apply_friendship_action(a, b, "accept", true, None, None)
            .await
            .unwrap();
    }
}

/// A fresh transport id per call, so each acts as its own caller.
fn rand_transport() -> u32 {
    static NEXT: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(10_000);
    NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
}

fn user(a: &str) -> Option<User> {
    Some(User {
        address: a.to_string(),
    })
}

fn request(to: &str) -> upsert_friendship_payload::Action {
    upsert_friendship_payload::Action::Request(upsert_friendship_payload::RequestPayload {
        user: user(to),
        message: None,
    })
}

fn accept(to: &str) -> upsert_friendship_payload::Action {
    upsert_friendship_payload::Action::Accept(upsert_friendship_payload::AcceptPayload {
        user: user(to),
    })
}

fn reject(to: &str) -> upsert_friendship_payload::Action {
    upsert_friendship_payload::Action::Reject(upsert_friendship_payload::RejectPayload {
        user: user(to),
    })
}

fn cancel(to: &str) -> upsert_friendship_payload::Action {
    upsert_friendship_payload::Action::Cancel(upsert_friendship_payload::CancelPayload {
        user: user(to),
    })
}

fn delete(to: &str) -> upsert_friendship_payload::Action {
    upsert_friendship_payload::Action::Delete(upsert_friendship_payload::DeletePayload {
        user: user(to),
    })
}

fn status_of(r: &get_friendship_status_response::Response) -> FriendshipStatus {
    match r {
        get_friendship_status_response::Response::Accepted(ok) => {
            FriendshipStatus::try_from(ok.status).unwrap()
        }
        other => panic!("expected a status, got {other:?}"),
    }
}

/// Every event `rx` holds, in order.
fn drained(rx: &mut Receiver<Arc<SocialEvent>>) -> Vec<Arc<SocialEvent>> {
    std::iter::from_fn(|| rx.try_recv().ok()).collect()
}

fn friendship_updates(events: &[Arc<SocialEvent>]) -> Vec<friendship_update::Update> {
    events
        .iter()
        .filter_map(|e| match e.as_ref() {
            SocialEvent::Friendship(u) => u.update.clone(),
            _ => None,
        })
        .collect()
}

fn block_updates(events: &[Arc<SocialEvent>]) -> Vec<BlockUpdate> {
    events
        .iter()
        .filter_map(|e| match e.as_ref() {
            SocialEvent::Block(b) => Some(b.clone()),
            _ => None,
        })
        .collect()
}

fn updated_by(update: &friendship_update::Update) -> Option<&str> {
    use friendship_update::Update as U;
    match update {
        U::Accept(r) => r.user.as_ref(),
        U::Reject(r) => r.user.as_ref(),
        U::Delete(r) => r.user.as_ref(),
        U::Cancel(r) => r.user.as_ref(),
        U::Block(r) => r.user.as_ref(),
        U::Request(_) => None,
    }
    .map(|u| u.address.as_str())
}

#[tokio::test]
async fn mutual_friends_answer_the_shared_friends_and_an_empty_page_for_a_bad_user() {
    let Some(rig) = Rig::new().await else {
        return;
    };
    let (a, b, c, d) = (addr("mf1"), addr("mf2"), addr("mf3"), addr("mf4"));
    for (x, y) in [(&a, &c), (&b, &c), (&a, &b), (&b, &d)] {
        rig.befriend(x, y).await;
    }
    let mutual = |other: Option<&str>| {
        let payload = GetMutualFriendsPayload {
            user: other.map(|o| User {
                address: o.to_string(),
            }),
            pagination: None,
        };
        let ctx = rig.as_(&a);
        async move {
            SocialServiceImpl
                .get_mutual_friends(payload, ctx)
                .await
                .unwrap()
        }
    };
    let page = mutual(Some(&b.to_uppercase().replacen("0X", "0x", 1))).await;
    assert_eq!(
        page.friends
            .iter()
            .map(|f| f.address.as_str())
            .collect::<Vec<_>>(),
        [c.as_str()]
    );
    assert_eq!(
        page.pagination_data,
        Some(PaginatedResponse { total: 1, page: 1 })
    );
    for bad in [None, Some("invalid-address")] {
        let page = mutual(bad).await;
        assert!(page.friends.is_empty(), "{bad:?}");
        assert_eq!(
            page.pagination_data,
            Some(PaginatedResponse { total: 0, page: 1 }),
            "{bad:?}"
        );
    }
    for (x, y) in [(&a, &c), (&b, &c), (&a, &b), (&b, &d)] {
        cleanup_pair(&rig.db, x, y).await;
    }
}

#[tokio::test]
async fn friendship_status_names_a_missing_or_invalid_user_and_reads_a_lone_block() {
    let Some(rig) = Rig::new().await else {
        return;
    };
    let (a, b) = (addr("fs1"), addr("fs2"));
    for (other, message) in [
        (None, "User address is missing in the request payload"),
        (Some(""), "User address is missing in the request payload"),
        (
            Some("invalid-address"),
            "Invalid user address in the request payload",
        ),
    ] {
        match rig.status(&a, other).await {
            get_friendship_status_response::Response::InvalidRequest(e) => {
                assert_eq!(e.message.as_deref(), Some(message), "{other:?}")
            }
            r => panic!("{other:?}: {r:?}"),
        }
    }
    assert_eq!(
        status_of(&rig.status(&a, Some(&b)).await),
        FriendshipStatus::None
    );
    assert!(matches!(
        rig.block(&a, &b).await,
        block_user_response::Response::Ok(_)
    ));
    assert_eq!(
        status_of(&rig.status(&a, Some(&b)).await),
        FriendshipStatus::Blocked
    );
    assert_eq!(
        status_of(&rig.status(&b, Some(&a)).await),
        FriendshipStatus::BlockedBy
    );
    cleanup_pair(&rig.db, &a, &b).await;
}

/// Either direction of a block refuses every friendship action, and nothing is written.
#[tokio::test]
async fn a_blocked_pair_cannot_change_its_friendship() {
    let Some(rig) = Rig::new().await else {
        return;
    };
    let (a, b) = (addr("bp1"), addr("bp2"));
    assert!(matches!(
        rig.block(&b, &a).await,
        block_user_response::Response::Ok(_)
    ));
    for me in [&a, &b] {
        let other = if me == &a { &b } else { &a };
        match rig.upsert(me, request(other)).await {
            upsert_friendship_response::Response::InvalidFriendshipAction(e) => assert_eq!(
                e.message.as_deref(),
                Some(
                    "This action is not allowed because either you blocked this user or this user blocked you"
                )
            ),
            r => panic!("{r:?}"),
        }
    }
    assert!(
        rig.db
            .friendship_probe(&a, &b)
            .await
            .unwrap()
            .last
            .is_none()
    );
    cleanup_pair(&rig.db, &a, &b).await;
}

/// The 11th symmetric mutation of a pair inside the window is refused before any write.
#[tokio::test]
async fn friendship_mutations_past_the_pair_budget_are_refused_without_a_write() {
    let Some(rig) = Rig::new().await else {
        return;
    };
    let (a, b) = (addr("rl1"), addr("rl2"));
    let mut last = None;
    for i in 0..10 {
        let action = if i % 2 == 0 { request(&b) } else { cancel(&b) };
        match rig.upsert(&a, action).await {
            upsert_friendship_response::Response::Accepted(ok) => last = Some(ok.id),
            r => panic!("mutation {i}: {r:?}"),
        }
    }
    let before = rig
        .db
        .friendship_probe(&a, &b)
        .await
        .unwrap()
        .last
        .map(|l| l.action);
    match rig.upsert(&b, request(&a)).await {
        upsert_friendship_response::Response::InvalidRequest(e) => assert_eq!(
            e.message.as_deref(),
            Some("Too many friendship or block actions. Please try again later")
        ),
        r => panic!("{r:?}"),
    }
    assert!(last.is_some());
    assert_eq!(
        rig.db
            .friendship_probe(&a, &b)
            .await
            .unwrap()
            .last
            .map(|l| l.action),
        before
    );
    cleanup_pair(&rig.db, &a, &b).await;
}

/// Cancel, reject and delete: each lands its status, keeps the friendship
/// inactive and tells only the other party who acted.
#[tokio::test]
async fn cancel_reject_and_delete_reach_the_other_party() {
    let Some(rig) = Rig::new().await else {
        return;
    };
    let (a, b) = (addr("tr1"), addr("tr2"));

    // a requests, then cancels: b hears both
    let mut to_b = rig.events(&b);
    assert!(matches!(
        rig.upsert(&a, request(&b)).await,
        upsert_friendship_response::Response::Accepted(_)
    ));
    assert!(matches!(
        rig.upsert(&a, cancel(&b)).await,
        upsert_friendship_response::Response::Accepted(_)
    ));
    assert_eq!(
        status_of(&rig.status(&a, Some(&b)).await),
        FriendshipStatus::Canceled
    );
    let updates = friendship_updates(&drained(&mut to_b));
    assert!(matches!(updates[0], friendship_update::Update::Request(_)));
    assert!(matches!(updates[1], friendship_update::Update::Cancel(_)));
    assert_eq!(updated_by(&updates[1]), Some(a.as_str()));

    // a requests again, b rejects: a hears it
    let mut to_a = rig.events(&a);
    assert!(matches!(
        rig.upsert(&a, request(&b)).await,
        upsert_friendship_response::Response::Accepted(_)
    ));
    assert!(matches!(
        rig.upsert(&b, reject(&a)).await,
        upsert_friendship_response::Response::Accepted(_)
    ));
    assert_eq!(
        status_of(&rig.status(&b, Some(&a)).await),
        FriendshipStatus::Rejected
    );
    let updates = friendship_updates(&drained(&mut to_a));
    assert_eq!(updates.len(), 1, "{updates:?}");
    assert!(matches!(updates[0], friendship_update::Update::Reject(_)));
    assert_eq!(updated_by(&updates[0]), Some(b.as_str()));
    assert!(rig.friends_of(&a).await.is_empty());

    // a requests, b accepts, then a deletes: b hears the delete, neither lists the other
    assert!(matches!(
        rig.upsert(&a, request(&b)).await,
        upsert_friendship_response::Response::Accepted(_)
    ));
    assert!(matches!(
        rig.upsert(&b, accept(&a)).await,
        upsert_friendship_response::Response::Accepted(_)
    ));
    assert_eq!(rig.friends_of(&a).await, std::slice::from_ref(&b));
    drained(&mut to_b);
    assert!(matches!(
        rig.upsert(&a, delete(&b)).await,
        upsert_friendship_response::Response::Accepted(_)
    ));
    assert_eq!(
        status_of(&rig.status(&a, Some(&b)).await),
        FriendshipStatus::Deleted
    );
    let updates = friendship_updates(&drained(&mut to_b));
    assert!(
        matches!(updates[..], [friendship_update::Update::Delete(_)]),
        "{updates:?}"
    );
    assert_eq!(updated_by(&updates[0]), Some(a.as_str()));
    assert!(rig.friends_of(&a).await.is_empty() && rig.friends_of(&b).await.is_empty());
    cleanup_pair(&rig.db, &a, &b).await;
}

/// An illegal transition answers invalidFriendshipAction and leaves the last action as it was.
#[tokio::test]
async fn an_illegal_transition_writes_nothing() {
    let Some(rig) = Rig::new().await else {
        return;
    };
    let (a, b) = (addr("il1"), addr("il2"));
    assert!(matches!(
        rig.upsert(&a, accept(&b)).await,
        upsert_friendship_response::Response::InvalidFriendshipAction(_)
    ));
    assert!(
        rig.db
            .friendship_probe(&a, &b)
            .await
            .unwrap()
            .last
            .is_none()
    );
    assert!(matches!(
        rig.upsert(&a, request(&b)).await,
        upsert_friendship_response::Response::Accepted(_)
    ));
    // the requested party cannot cancel a request it received
    assert!(matches!(
        rig.upsert(&b, cancel(&a)).await,
        upsert_friendship_response::Response::InvalidFriendshipAction(_)
    ));
    assert_eq!(
        status_of(&rig.status(&a, Some(&b)).await),
        FriendshipStatus::RequestSent
    );
    cleanup_pair(&rig.db, &a, &b).await;
}

/// Blocking a friend: the response carries the blocked profile, the pair
/// leaves both friend lists, and the other party hears the block twice
/// (its friendship stream and its block stream).
#[tokio::test]
async fn blocking_a_friend_ends_the_friendship_and_tells_them() {
    let Some(rig) = Rig::new().await else {
        return;
    };
    let (a, b) = (addr("bf1"), addr("bf2"));
    rig.befriend(&a, &b).await;
    let mut to_b = rig.events(&b);
    match rig
        .block(&a, &b.to_uppercase().replacen("0X", "0x", 1))
        .await
    {
        block_user_response::Response::Ok(ok) => {
            let profile = ok.profile.unwrap();
            assert_eq!(profile.address, b);
            assert!(profile.blocked_at.is_some());
        }
        r => panic!("{r:?}"),
    }
    assert_eq!(
        status_of(&rig.status(&a, Some(&b)).await),
        FriendshipStatus::Blocked
    );
    assert_eq!(
        status_of(&rig.status(&b, Some(&a)).await),
        FriendshipStatus::BlockedBy
    );
    assert!(rig.friends_of(&a).await.is_empty() && rig.friends_of(&b).await.is_empty());
    let events = drained(&mut to_b);
    let updates = friendship_updates(&events);
    assert!(
        matches!(updates[..], [friendship_update::Update::Block(_)]),
        "{updates:?}"
    );
    assert_eq!(updated_by(&updates[0]), Some(a.as_str()));
    assert_eq!(
        block_updates(&events),
        [BlockUpdate {
            stream_closed: None,
            address: a.clone(),
            is_blocked: true
        }]
    );

    // unblocking records a delete and tells b in that order
    match rig.unblock(&a, &b).await {
        unblock_user_response::Response::Ok(ok) => {
            let profile = ok.profile.unwrap();
            assert_eq!(profile.address, b);
            assert_eq!(profile.blocked_at, None);
        }
        r => panic!("{r:?}"),
    }
    assert_eq!(
        status_of(&rig.status(&a, Some(&b)).await),
        FriendshipStatus::Deleted
    );
    let events = drained(&mut to_b);
    assert!(matches!(
        events.iter().map(|e| e.as_ref()).collect::<Vec<_>>()[..],
        [SocialEvent::Friendship(_), SocialEvent::Block(_)]
    ));
    assert!(matches!(
        friendship_updates(&events)[..],
        [friendship_update::Update::Delete(_)]
    ));
    assert_eq!(
        block_updates(&events),
        [BlockUpdate {
            stream_closed: None,
            address: a.clone(),
            is_blocked: false
        }]
    );
    cleanup_pair(&rig.db, &a, &b).await;
}

/// Unblocking a stranger only lifts the block.
#[tokio::test]
async fn unblocking_a_stranger_only_lifts_the_block() {
    let Some(rig) = Rig::new().await else {
        return;
    };
    let (a, b) = (addr("ub1"), addr("ub2"));
    assert!(matches!(
        rig.block(&a, &b).await,
        block_user_response::Response::Ok(_)
    ));
    let mut to_b = rig.events(&b);
    assert!(matches!(
        rig.unblock(&a, &b).await,
        unblock_user_response::Response::Ok(_)
    ));
    let events = drained(&mut to_b);
    assert!(
        friendship_updates(&events).is_empty(),
        "{} events",
        events.len()
    );
    assert_eq!(
        block_updates(&events),
        [BlockUpdate {
            stream_closed: None,
            address: a.clone(),
            is_blocked: false
        }]
    );
    assert_eq!(
        status_of(&rig.status(&a, Some(&b)).await),
        FriendshipStatus::None
    );
    cleanup_pair(&rig.db, &a, &b).await;
}

/// Unblocking someone who isn't blocked records nothing and tells no one, so a
/// friendship survives it (`decentraland/social-service-ea` `bdb14ce`).
#[tokio::test]
async fn unblocking_an_unblocked_friend_changes_nothing() {
    let Some(rig) = Rig::new().await else {
        return;
    };
    let (a, b) = (addr("ub5"), addr("ub6"));
    rig.befriend(&a, &b).await;
    let mut to_b = rig.events(&b);
    assert!(matches!(
        rig.unblock(&a, &b).await,
        unblock_user_response::Response::Ok(_)
    ));
    let events = drained(&mut to_b);
    assert!(events.is_empty(), "{} events", events.len());
    assert_eq!(
        status_of(&rig.status(&a, Some(&b)).await),
        FriendshipStatus::Accepted
    );
    assert_eq!(rig.friends_of(&a).await, std::slice::from_ref(&b));
    cleanup_pair(&rig.db, &a, &b).await;
}

/// A request message is capped at 256 characters (code points, not bytes);
/// a longer one is refused before anything is written.
#[tokio::test]
async fn a_request_message_over_256_characters_is_refused() {
    let Some(rig) = Rig::new().await else {
        return;
    };
    let (a, b) = (addr("rm1"), addr("rm2"));
    let with = |message: String| {
        upsert_friendship_payload::Action::Request(upsert_friendship_payload::RequestPayload {
            user: user(&b),
            message: Some(message),
        })
    };
    match rig.upsert(&a, with("ñ".repeat(257))).await {
        upsert_friendship_response::Response::InvalidRequest(e) => assert_eq!(
            e.message.as_deref(),
            Some("The friendship request message cannot be longer than 256 characters")
        ),
        other => panic!("expected invalidRequest, got {other:?}"),
    }
    assert_eq!(
        status_of(&rig.status(&a, Some(&b)).await),
        FriendshipStatus::None
    );
    assert!(matches!(
        rig.upsert(&a, with("ñ".repeat(256))).await,
        upsert_friendship_response::Response::Accepted(_)
    ));
    cleanup_pair(&rig.db, &a, &b).await;
}

#[tokio::test]
async fn blocking_yourself_is_an_invalid_request() {
    let Some(rig) = Rig::new().await else {
        return;
    };
    let a = addr("bs1");
    for target in [a.clone(), a.to_uppercase().replacen("0X", "0x", 1)] {
        assert!(
            matches!(
                rig.block(&a, &target).await,
                block_user_response::Response::InvalidRequest(_)
            ),
            "{target}"
        );
    }
}

/// Upstream's `get-blocking-status` cases: each direction alone, both, and
/// none, as the blocker and as the blocked read them.
#[tokio::test]
async fn blocking_status_reads_each_direction_alone() {
    let Some(rig) = Rig::new().await else {
        return;
    };
    let (a, b) = (addr("bs1"), addr("bs2"));
    let status = |me: String| {
        let pctx = rig.as_(&me);
        async move {
            let s = SocialServiceImpl.get_blocking_status(pctx).await.unwrap();
            (s.blocked_users, s.blocked_by_users)
        }
    };
    assert_eq!(status(a.clone()).await, (vec![], vec![]));
    rig.block(&a, &b).await;
    assert_eq!(status(a.clone()).await, (vec![b.clone()], vec![]));
    assert_eq!(status(b.clone()).await, (vec![], vec![a.clone()]));
    rig.block(&b, &a).await;
    assert_eq!(status(a.clone()).await, (vec![b.clone()], vec![b.clone()]));
    rig.unblock(&a, &b).await;
    assert_eq!(status(a.clone()).await, (vec![], vec![b.clone()]));
    cleanup_pair(&rig.db, &a, &b).await;
}

fn request_with(to: Option<&str>, message: Option<&str>) -> upsert_friendship_payload::Action {
    upsert_friendship_payload::Action::Request(upsert_friendship_payload::RequestPayload {
        user: to.and_then(user),
        message: message.map(str::to_string),
    })
}

/// Upstream `upsert-friendship.ts` (`decentraland/social-service-ea` at
/// `b6b2cef`, `:22-33`): an unknown action is `invalidRequest` "Unknown
/// message", a request to yourself `invalidFriendshipAction`, an address that
/// is not one `invalidRequest`; any other action to yourself meets the state
/// machine. An explicit `""` message is no message
/// (`parseUpsertFriendshipRequest`), and the answer's message reads `''`
/// (`parseFriendshipRequestToFriendshipRequestResponse`). Nothing refused is
/// written.
#[tokio::test]
async fn upsert_friendship_validates_and_reads_messages_as_upstream() {
    let Some(rig) = Rig::new().await else {
        return;
    };
    let (a, b) = (addr("uv1"), addr("uv2"));
    let refusal = |r: upsert_friendship_response::Response| match r {
        upsert_friendship_response::Response::InvalidRequest(e) => {
            ("invalidRequest", e.message.unwrap_or_default())
        }
        upsert_friendship_response::Response::InvalidFriendshipAction(e) => {
            ("invalidFriendshipAction", e.message.unwrap_or_default())
        }
        other => panic!("not a refusal: {other:?}"),
    };
    let unknown = SocialServiceImpl
        .upsert_friendship(UpsertFriendshipPayload { action: None }, rig.as_(&a))
        .await
        .unwrap()
        .response
        .unwrap();
    assert_eq!(
        refusal(unknown),
        ("invalidRequest", "Unknown message".into())
    );
    let checksummed_self = a.to_uppercase().replacen("0X", "0x", 1);
    assert_eq!(
        refusal(rig.upsert(&a, request(&checksummed_self)).await),
        (
            "invalidFriendshipAction",
            "You cannot send a friendship request to yourself".into()
        )
    );
    for target in [Some("0x12"), Some(""), Some("not an address")] {
        assert_eq!(
            refusal(rig.upsert(&a, request_with(target, None)).await),
            (
                "invalidRequest",
                "Invalid user address in the request payload".into()
            ),
            "{target:?}"
        );
    }
    let missing_user = rig.upsert(&a, request_with(None, None)).await;
    assert!(
        matches!(
            missing_user,
            upsert_friendship_response::Response::InternalServerError(ref error)
                if error.message.as_deref() == Some("Cannot read properties of undefined (reading 'address')")
        ),
        "{missing_user:?}"
    );
    assert_eq!(
        refusal(rig.upsert(&a, accept(&a)).await),
        (
            "invalidFriendshipAction",
            "The friendship action is not valid for the current friendship status".into()
        ),
        "an accept to yourself meets the state machine"
    );
    assert!(
        rig.db
            .last_friendship_action(&a, &a)
            .await
            .unwrap()
            .is_none()
    );

    let mut to_b = rig.events(&b);
    let accepted = match rig.upsert(&a, request_with(Some(&b), Some(""))).await {
        upsert_friendship_response::Response::Accepted(ok) => ok,
        other => panic!("{other:?}"),
    };
    assert_eq!(accepted.message.as_deref(), Some(""));
    let stored: Option<serde_json::Value> = sqlx::query_scalar(
        "SELECT fa.metadata::jsonb FROM social.friendship_actions fa \
         WHERE fa.friendship_id = $1",
    )
    .bind(Uuid::parse_str(&accepted.id).unwrap())
    .fetch_one(rig.db.pool())
    .await
    .unwrap();
    assert_eq!(stored, None, "an explicit empty message stores none");
    assert!(matches!(
        &*to_b.try_recv().unwrap(),
        SocialEvent::Friendship(FriendshipUpdate {
            update: Some(friendship_update::Update::Request(r)),
            ..
        }) if r.message.is_none()
    ));
    cleanup_pair(&rig.db, &a, &b).await;
}

/// Upstream `block-user.ts` / `unblock-user.ts` (`:20-26`): the raw address
/// must be one, then not the caller's own, in any letter case.
#[tokio::test]
async fn block_targets_are_checked_as_upstream() {
    let Some(rig) = Rig::new().await else {
        return;
    };
    let a = addr("bt1");
    let checksummed = a.to_uppercase().replacen("0X", "0x", 1);
    let message = |r: block_user_response::Response| match r {
        block_user_response::Response::InvalidRequest(e) => e.message.unwrap_or_default(),
        other => panic!("{other:?}"),
    };
    assert_eq!(
        message(rig.block(&a, &checksummed).await),
        "Cannot block yourself"
    );
    assert_eq!(
        message(rig.block(&a, "0x12").await),
        "Invalid user address in the request payload"
    );
    let missing = SocialServiceImpl
        .block_user(BlockUserPayload { user: None }, rig.as_(&a))
        .await
        .unwrap()
        .response
        .unwrap();
    assert_eq!(
        message(missing),
        "Invalid user address in the request payload"
    );
    let unblock_message = |r: unblock_user_response::Response| match r {
        unblock_user_response::Response::InvalidRequest(e) => e.message.unwrap_or_default(),
        other => panic!("{other:?}"),
    };
    assert_eq!(
        unblock_message(rig.unblock(&a, &checksummed).await),
        "Cannot unblock yourself"
    );
    assert_eq!(
        unblock_message(rig.unblock(&a, "nope").await),
        "Invalid user address in the request payload"
    );
    assert!(
        rig.db
            .get_blocked_users(&a, 10, 0)
            .await
            .unwrap()
            .0
            .is_empty()
    );
}

/// Upstream `applyBlock` (`src/logic/friends/component.ts`): a stranger has
/// no friendship to end, so only the block update is published.
#[tokio::test]
async fn blocking_a_stranger_publishes_only_the_block_update() {
    let Some(rig) = Rig::new().await else {
        return;
    };
    let (a, b) = (addr("bs3"), addr("bs4"));
    let mut to_b = rig.events(&b);
    assert!(matches!(
        rig.block(&a, &b).await,
        block_user_response::Response::Ok(_)
    ));
    let events = drained(&mut to_b);
    assert_eq!(events.len(), 1);
    assert!(matches!(
        &*events[0],
        SocialEvent::Block(BlockUpdate { address, is_blocked: true, .. }) if *address == a
    ));
    cleanup_pair(&rig.db, &a, &b).await;
}

/// Upstream `applyBlock` writes the block and the friendship's `block`
/// action in one transaction: when the action's write fails, the block is
/// rolled back with it and no one is told.
#[tokio::test]
async fn a_block_whose_friendship_write_fails_is_rolled_back() {
    let Some(rig) = Rig::new().await else {
        return;
    };
    let (a, b) = (addr("br1"), addr("br2"));
    rig.befriend(&a, &b).await;
    // A trigger refusing this blocker's friendship actions, and only theirs.
    let tag = Uuid::new_v4().simple().to_string();
    let function = format!("social.refuse_{tag}");
    sqlx::query(sqlx::AssertSqlSafe(format!(
        "CREATE FUNCTION {function}() RETURNS trigger LANGUAGE plpgsql AS $$ \
         BEGIN IF NEW.acting_user = '{a}' THEN RAISE EXCEPTION 'refused by test'; END IF; RETURN NEW; END $$"
    )))
    .execute(rig.db.pool())
    .await
    .unwrap();
    sqlx::query(sqlx::AssertSqlSafe(format!(
        "CREATE TRIGGER refuse_{tag} BEFORE INSERT ON social.friendship_actions \
         FOR EACH ROW EXECUTE FUNCTION {function}()"
    )))
    .execute(rig.db.pool())
    .await
    .unwrap();
    let mut to_b = rig.events(&b);
    let answer = rig.block(&a, &b).await;
    sqlx::query(sqlx::AssertSqlSafe(format!(
        "DROP TRIGGER refuse_{tag} ON social.friendship_actions"
    )))
    .execute(rig.db.pool())
    .await
    .unwrap();
    sqlx::query(sqlx::AssertSqlSafe(format!("DROP FUNCTION {function}()")))
        .execute(rig.db.pool())
        .await
        .unwrap();
    assert!(
        matches!(
            answer,
            block_user_response::Response::InternalServerError(_)
        ),
        "{answer:?}"
    );
    assert!(
        !rig.db.is_blocked(&a, &b).await.unwrap(),
        "the block rolled back"
    );
    assert_eq!(rig.friends_of(&a).await, vec![b.clone()], "still friends");
    assert!(drained(&mut to_b).is_empty());
    cleanup_pair(&rig.db, &a, &b).await;
}

/// With a profile source, a wallet without a profile is left out where
/// upstream's registry drops it: a listed friend or request (the total
/// stays the row count). RPC stream delivery filters its friendship updates;
/// the shared publisher retains address-only updates for the JSON stream. An
/// upsert naming it answers `internalServerError` after the write
/// (`src/logic/friends/component.ts` `upsertFriendship`,
/// `src/logic/friends/friendships.ts:254-266`). A listed request's absent
/// message reads `''` (`:242`).
#[tokio::test]
async fn a_wallet_without_a_profile_is_left_out_with_a_profile_source() {
    let Some(pool) = scratch_pool().await else {
        return;
    };
    let db = Db::new(pool.clone());
    let ctx = profiled_ctx(db.clone(), Arc::new(FakeVoiceRooms::new(pool.clone())));
    let (me, known, unknown) = (addr("np1"), addr("np2"), addr("np3"));
    let (pm, pk) = (
        deploy_profile(&pool, &me, "Me").await,
        deploy_profile(&pool, &known, "Known").await,
    );
    let mut to_me = ctx.pubsub().subscribe(&me);
    for from in [&known, &unknown] {
        db.apply_friendship_action(from, &me, "request", false, None, None)
            .await
            .unwrap();
    }
    let pending = SocialServiceImpl
        .get_pending_friendship_requests(
            GetFriendshipRequestsPayload { pagination: None },
            procedure_ctx(&ctx, &me, rand_transport()),
        )
        .await
        .unwrap();
    let Some(paginated_friendship_requests_response::Response::Requests(list)) = pending.response
    else {
        panic!("{pending:?}");
    };
    assert_eq!(list.requests.len(), 1);
    let only = &list.requests[0];
    assert_eq!(only.friend.as_ref().unwrap().address, known);
    assert_eq!(only.friend.as_ref().unwrap().name, "Known");
    assert_eq!(
        only.message.as_deref(),
        Some(""),
        "an absent message reads ''"
    );
    assert_eq!(pending.pagination_data.unwrap().total, 2);

    // an upsert naming the wallet without a profile is written, then fails
    let answer = SocialServiceImpl
        .upsert_friendship(
            UpsertFriendshipPayload {
                action: Some(accept(&unknown)),
            },
            procedure_ctx(&ctx, &me, rand_transport()),
        )
        .await
        .unwrap()
        .response
        .unwrap();
    assert!(
        matches!(
            answer,
            upsert_friendship_response::Response::InternalServerError(_)
        ),
        "{answer:?}"
    );
    assert_eq!(
        db.last_friendship_action(&me, &unknown)
            .await
            .unwrap()
            .unwrap()
            .action,
        "accept"
    );
    // Raw events retain the address for JSON stream delivery; RPC subscribers
    // resolve profiles and filter their own output at the delivery boundary.
    let answer = SocialServiceImpl
        .upsert_friendship(
            UpsertFriendshipPayload {
                action: Some(delete(&me)),
            },
            procedure_ctx(&ctx, &unknown, rand_transport()),
        )
        .await
        .unwrap()
        .response
        .unwrap();
    assert!(matches!(
        answer,
        upsert_friendship_response::Response::InternalServerError(_)
    ));
    let raw = friendship_updates(&drained(&mut to_me));
    assert_eq!(raw.len(), 1);
    assert!(matches!(&raw[0], friendship_update::Update::Delete(_)));
    assert_eq!(updated_by(&raw[0]), Some(unknown.as_str()));
    // the known wallet's cancel does reach me
    assert!(matches!(
        upsert_with(&ctx, &known, cancel(&me)).await,
        upsert_friendship_response::Response::Accepted(_)
    ));
    assert!(matches!(
        &*to_me.try_recv().unwrap(),
        SocialEvent::Friendship(FriendshipUpdate {
            update: Some(friendship_update::Update::Cancel(_)),
            ..
        })
    ));

    // friends: the profile-less friend is left out, the total keeps it
    db.apply_friendship_action(&me, &known, "accept", true, None, None)
        .await
        .unwrap();
    db.apply_friendship_action(&me, &unknown, "accept", true, None, None)
        .await
        .unwrap();
    let friends = SocialServiceImpl
        .get_friends(
            GetFriendsPayload { pagination: None },
            procedure_ctx(&ctx, &me, rand_transport()),
        )
        .await
        .unwrap();
    let addresses: Vec<String> = friends.friends.iter().map(|f| f.address.clone()).collect();
    assert_eq!(addresses, vec![known.clone()]);
    assert_eq!(friends.pagination_data.unwrap().total, 2);

    drop_profile(&pool, &me, &pm).await;
    drop_profile(&pool, &known, &pk).await;
    let _ = sqlx::query("DELETE FROM social.notifications WHERE address = ANY($1)")
        .bind(vec![me.clone(), known.clone(), unknown.clone()])
        .execute(&pool)
        .await;
    cleanup_pair(&db, &me, &known).await;
    cleanup_pair(&db, &me, &unknown).await;
}

async fn upsert_with(
    ctx: &Context,
    me: &str,
    action: upsert_friendship_payload::Action,
) -> upsert_friendship_response::Response {
    SocialServiceImpl
        .upsert_friendship(
            UpsertFriendshipPayload {
                action: Some(action),
            },
            procedure_ctx(ctx, me, rand_transport()),
        )
        .await
        .unwrap()
        .response
        .unwrap()
}

/// Accepting changes the connectivity audience without another Pulse frame.
/// Each recipient learns only about peers actually online in that snapshot.
#[tokio::test]
async fn accept_announces_only_current_online_friends_without_topology_change() {
    let Some(rig) = Rig::new().await else {
        return;
    };
    for (a_online, b_online) in [(true, true), (true, false), (false, false)] {
        let (a, b) = (addr("ac1"), addr("ac2"));
        let wallets = [(&a, a_online), (&b, b_online)]
            .into_iter()
            .filter(|(_, online)| *online)
            .map(|(wallet, _)| wallet.clone())
            .collect();
        rig.ctx.topology().receive(&pulse_frame(wallets)).unwrap();
        let generation = rig.ctx.topology().snapshot().generation;
        let mut to_a = rig.events(&a);
        let mut to_b = rig.events(&b);
        assert!(matches!(
            rig.upsert(&a, request(&b)).await,
            upsert_friendship_response::Response::Accepted(_)
        ));
        drained(&mut to_a);
        drained(&mut to_b);
        assert!(matches!(
            rig.upsert(&b, accept(&a)).await,
            upsert_friendship_response::Response::Accepted(_)
        ));
        assert_eq!(rig.ctx.topology().snapshot().generation, generation);
        for (recipient, peer, online, receiver) in
            [(&a, &b, b_online, &mut to_a), (&b, &a, a_online, &mut to_b)]
        {
            let connectivity: Vec<_> = drained(receiver)
                .into_iter()
                .filter_map(|event| match event.as_ref() {
                    SocialEvent::TopologyFriendConnectivity(update, generation) => {
                        Some((update.clone(), *generation))
                    }
                    _ => None,
                })
                .collect();
            if online {
                let [(update, event_generation)] = connectivity.as_slice() else {
                    panic!("expected one connectivity update, got {connectivity:?}");
                };
                assert_eq!(update.friend.as_ref().unwrap().address, *peer);
                assert_eq!(update.status, ConnectivityStatus::Online as i32);
                assert_eq!(*event_generation, generation);
                let guard = rig.ctx.friend_delivery_guard(
                    recipient.clone(),
                    peer.clone(),
                    true,
                    *event_generation,
                );
                assert!(guard().await.unwrap());
                rig.db.block_user(recipient, peer).await.unwrap();
                assert!(
                    !guard().await.unwrap(),
                    "queued accept cannot bypass a later block"
                );
                rig.db.unblock_user(recipient, peer).await.unwrap();
            } else {
                assert!(connectivity.is_empty(), "offline peer was announced online");
            }
        }
        cleanup_pair(&rig.db, &a, &b).await;
    }
}

/// Repeated request cycles have distinct committed action IDs. Only requests
/// and acceptance create inbox/outbox rows, with the processor's metadata.
#[tokio::test]
async fn friendship_notifications_use_action_ids_and_preserve_profile_metadata() {
    let Some(pool) = scratch_pool().await else {
        return;
    };
    let (a, b) = (addr("fn1"), addr("fn2"));
    let a_entity = seed_profile(&pool, &a, "Alice").await;
    let b_entity = seed_profile(&pool, &b, "Bob").await;
    let db = Db::new(pool.clone());
    let rig = Rig {
        db: db.clone(),
        ctx: Context::new(
            VoiceSettings::default(),
            db,
            Profiles::new(Some(pool.clone())),
            Arc::new(FakeVoiceRooms::new(pool.clone())),
        ),
    };
    let with_message =
        upsert_friendship_payload::Action::Request(upsert_friendship_payload::RequestPayload {
            user: user(&b),
            message: Some("hello".into()),
        });
    for (actor, action) in [
        (&a, with_message),
        (&a, cancel(&b)),
        (&a, request(&b)),
        (&b, reject(&a)),
        (&a, request(&b)),
        (&b, accept(&a)),
        (&a, delete(&b)),
    ] {
        assert!(matches!(
            rig.upsert(actor, action).await,
            upsert_friendship_response::Response::Accepted(_)
        ));
    }
    let rows: Vec<(Uuid, String, String, serde_json::Value, String)> = sqlx::query_as(
        "SELECT n.id, n.type, n.address, n.metadata, a.action \
         FROM social.notifications n JOIN social.friendship_actions a ON a.id = n.id \
         WHERE n.address IN ($1, $2) ORDER BY a.\"timestamp\", a.id",
    )
    .bind(&a)
    .bind(&b)
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(rows.len(), 4);
    assert_eq!(rows.iter().filter(|row| row.4 == "request").count(), 3);
    assert_eq!(rows.iter().filter(|row| row.4 == "accept").count(), 1);
    let mut messages = Vec::new();
    for (id, kind, address, metadata, action) in &rows {
        assert_eq!(metadata["requestId"], id.to_string());
        let (sender, receiver, name, image) = if action == "request" {
            assert_eq!(kind, "social_service_friendship_request");
            (&a, &b, "Alice", &a_entity)
        } else {
            assert_eq!(kind, "social_service_friendship_accepted");
            assert!(metadata.get("message").is_none());
            (&b, &a, "Bob", &b_entity)
        };
        assert_eq!(address, receiver);
        assert_eq!(metadata["sender"]["address"], *sender);
        assert_eq!(metadata["receiver"]["address"], *receiver);
        assert_eq!(metadata["sender"]["name"], name);
        assert_eq!(metadata["sender"]["hasClaimedName"], true);
        assert_eq!(
            metadata["sender"]["profileImageUrl"],
            format!("https://example.test/{image}.png")
        );
        if let Some(message) = metadata.get("message") {
            messages.push(message.clone());
        }
    }
    assert_eq!(messages, [serde_json::json!("hello")]);
    let deliveries: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM social.notification_deliveries d \
         JOIN social.notifications n ON n.id=d.notification_id WHERE n.address IN ($1,$2)",
    )
    .bind(&a)
    .bind(&b)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        deliveries, 4,
        "each inbox row uses the existing delivery outbox"
    );
    sqlx::query("DELETE FROM social.notifications WHERE address IN ($1,$2)")
        .bind(&a)
        .bind(&b)
        .execute(&pool)
        .await
        .unwrap();
    for (address, entity) in [(&a, a_entity), (&b, b_entity)] {
        sqlx::query("DELETE FROM content.active_pointers WHERE pointer=$1")
            .bind(address)
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("DELETE FROM content.deployments WHERE entity_id=$1")
            .bind(entity)
            .execute(&pool)
            .await
            .unwrap();
    }
    cleanup_pair(&rig.db, &a, &b).await;
}

#[tokio::test]
async fn missing_profiles_commit_friendship_but_fail_rpc_enrichment_and_skip_notifications() {
    let Some(rig) = Rig::new().await else {
        return;
    };
    let (a, b) = (addr("np1"), addr("np2"));
    for (actor, action) in [(&a, request(&b)), (&b, accept(&a))] {
        assert!(matches!(
            SocialServiceImpl
                .upsert_friendship(
                    UpsertFriendshipPayload {
                        action: Some(action)
                    },
                    rig.as_(actor)
                )
                .await
                .unwrap()
                .response
                .unwrap(),
            upsert_friendship_response::Response::InternalServerError(_)
        ));
    }
    let count: i64 =
        sqlx::query_scalar("SELECT count(*) FROM social.notifications WHERE address IN ($1,$2)")
            .bind(&a)
            .bind(&b)
            .fetch_one(rig.db.pool())
            .await
            .unwrap();
    assert_eq!(count, 0);
    cleanup_pair(&rig.db, &a, &b).await;
}

#[tokio::test]
async fn blocking_without_friendship_only_emits_a_block_update() {
    let Some(rig) = Rig::new().await else { return };
    let (a, b) = (addr("solo1"), addr("solo2"));
    let mut events = rig.events(&b);
    assert!(matches!(
        rig.block(&a, &b).await,
        block_user_response::Response::Ok(_)
    ));
    let updates = drained(&mut events);
    assert!(friendship_updates(&updates).is_empty());
    assert_eq!(block_updates(&updates).len(), 1);
    assert_eq!(
        status_of(&rig.status(&a, Some(&b)).await),
        FriendshipStatus::Blocked
    );
    cleanup_pair(&rig.db, &a, &b).await;
}

#[tokio::test]
async fn friendship_responses_keep_the_original_creation_time_after_later_actions() {
    let Some(rig) = Rig::new().await else { return };
    let (a, b) = (addr("time1"), addr("time2"));
    let upsert_friendship_response::Response::Accepted(first) = rig.upsert(&a, request(&b)).await
    else {
        panic!("request failed")
    };
    let original = chrono::Utc::now() - chrono::Duration::hours(1);
    sqlx::query("UPDATE social.friendships SET created_at=$1 WHERE id=$2")
        .bind(original.naive_utc())
        .bind(Uuid::parse_str(&first.id).unwrap())
        .execute(rig.db.pool())
        .await
        .unwrap();
    let upsert_friendship_response::Response::Accepted(cancelled) =
        rig.upsert(&a, cancel(&b)).await
    else {
        panic!("cancel failed")
    };
    assert_eq!(cancelled.created_at, original.timestamp_millis());
    assert_eq!(cancelled.message.as_deref(), Some(""));
    cleanup_pair(&rig.db, &a, &b).await;
}

#[tokio::test]
async fn block_and_unblock_roll_back_when_the_friendship_action_cannot_be_recorded() {
    let Some(rig) = Rig::new().await else { return };
    let (a, b) = (addr("atomic1"), addr("atomic2"));
    rig.befriend(&a, &b).await;
    let friendship = rig
        .db
        .last_friendship_action(&a, &b)
        .await
        .unwrap()
        .unwrap()
        .friendship_id;
    let name = format!("refuse_action_{}", Uuid::new_v4().simple());
    let function = format!(
        "CREATE FUNCTION social.{name}() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN IF NEW.friendship_id = '{friendship}'::uuid AND NEW.action IN ('block','delete') THEN RAISE EXCEPTION 'test action failure'; END IF; RETURN NEW; END $$"
    );
    sqlx::query(sqlx::AssertSqlSafe(function))
        .execute(rig.db.pool())
        .await
        .unwrap();
    sqlx::query(sqlx::AssertSqlSafe(format!("CREATE TRIGGER {name} BEFORE INSERT ON social.friendship_actions FOR EACH ROW EXECUTE FUNCTION social.{name}()")))
        .execute(rig.db.pool()).await.unwrap();
    let mut events = rig.events(&b);
    assert!(matches!(
        rig.block(&a, &b).await,
        block_user_response::Response::InternalServerError(_)
    ));
    assert!(!rig.db.is_blocked(&a, &b).await.unwrap());
    assert_eq!(
        rig.db
            .last_friendship_action(&a, &b)
            .await
            .unwrap()
            .unwrap()
            .action,
        "accept"
    );
    assert!(drained(&mut events).is_empty());
    rig.db.block_user(&a, &b).await.unwrap();
    assert!(matches!(
        rig.unblock(&a, &b).await,
        unblock_user_response::Response::InternalServerError(_)
    ));
    assert!(rig.db.is_blocked(&a, &b).await.unwrap());
    assert_eq!(
        rig.db
            .last_friendship_action(&a, &b)
            .await
            .unwrap()
            .unwrap()
            .action,
        "accept"
    );
    assert!(drained(&mut events).is_empty());
    sqlx::query(sqlx::AssertSqlSafe(format!(
        "DROP TRIGGER {name} ON social.friendship_actions"
    )))
    .execute(rig.db.pool())
    .await
    .unwrap();
    sqlx::query(sqlx::AssertSqlSafe(format!(
        "DROP FUNCTION social.{name}()"
    )))
    .execute(rig.db.pool())
    .await
    .unwrap();
    cleanup_pair(&rig.db, &a, &b).await;
}

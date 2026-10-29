use super::*;

/// The worlds hook, recording each call.
#[derive(Default)]
pub(crate) struct WorldExits(pub std::sync::Mutex<Vec<String>>);

impl super::super::WorldAccessRevocation for WorldExits {
    fn member_removed(&self, community_id: CommunityId, member: &Address) {
        self.0
            .lock()
            .unwrap()
            .push(format!("member {community_id} {member}"));
    }

    fn community_closed(&self, community_id: CommunityId) {
        self.0
            .lock()
            .unwrap()
            .push(format!("closed {community_id}"));
    }
}

#[tokio::test]
async fn leaving_kicking_banning_suspending_and_deleting_reach_the_worlds() {
    let Some(rig) = Rig::new(Settings {
        admin_token: Some(ADMIN_TOKEN.into()),
        ..Settings::for_tests()
    })
    .await
    else {
        return;
    };
    let recorded = Arc::new(WorldExits::default());
    rig.state.set_world_access_revocation(recorded.clone());
    let (owner, leaver, kicked, banned, stranger) =
        (wallet(), wallet(), wallet(), wallet(), wallet());
    let community = rig.community(&owner, false).await;
    for w in [&leaver, &kicked, &banned] {
        rig.member(community, w, "member").await;
    }
    let member_path = |w: &Wallet| format!("/v1/communities/{community}/members/{}", w.address);
    for (method, path, signer) in [
        ("delete", member_path(&leaver), &leaver),
        ("delete", member_path(&kicked), &owner),
        ("post", format!("{}/bans", member_path(&banned)), &owner),
        // Not a member: the ban removes nothing, the worlds are not told.
        ("post", format!("{}/bans", member_path(&stranger)), &owner),
    ] {
        let (status, body) = hit(&rig.app, Case::new(method, &path).signed(signer).await).await;
        assert_eq!(status, StatusCode::NO_CONTENT, "{method} {path}: {body}");
    }
    let admin = |action: &str| format!("/v1/admin/communities/{community}/{action}");
    for action in ["suspend", "unsuspend"] {
        let (status, body) = hit(
            &rig.app,
            Case::new("post", admin(action)).bearer(ADMIN_TOKEN),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{action}: {body}");
    }
    let path = format!("/v1/communities/{community}");
    let (status, body) = hit(&rig.app, Case::new("delete", &path).signed(&owner).await).await;
    assert_eq!(status, StatusCode::NO_CONTENT, "{body}");
    assert_eq!(
        *recorded.0.lock().unwrap(),
        vec![
            format!("member {community} {}", leaver.address),
            format!("member {community} {}", kicked.address),
            format!("member {community} {}", banned.address),
            format!("closed {community}"),
            format!("closed {community}"),
        ],
        "the suspension closes, the unsuspension does not"
    );
}

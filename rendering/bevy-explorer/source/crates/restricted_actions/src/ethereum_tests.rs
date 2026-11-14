use super::*;
use common::structs::{AppConfig, PermissionUsed, PrimaryPlayerRes};
use scene_runner::{initialize_scene::ScenePointers, permissions::PermissionManager};
use std::time::Duration;

pub(super) fn app() -> (App, Entity) {
    let mut app = App::new();
    app.add_event::<RpcCallEvent>()
        .add_event::<PermissionUsed>()
        .init_resource::<Wallet>()
        .init_resource::<Time>()
        .init_resource::<AppConfig>()
        .init_resource::<CurrentRealm>()
        .init_resource::<ScenePointers>()
        .init_resource::<LiveScenes>()
        .init_resource::<PortableScenes>()
        .init_resource::<PermissionManager>()
        .insert_resource(PrimaryPlayerRes(Entity::PLACEHOLDER))
        .add_systems(Update, (handle_eth_async, get_user_account));
    app.world_mut()
        .resource_mut::<Time>()
        .advance_by(Duration::from_secs(10));
    let mut context = RendererSceneContext::new(
        dcl_types::SceneId::DUMMY,
        "signing-test-scene".to_owned(),
        "storage_root".to_owned(),
        false,
        0,
        "title".to_owned(),
        IVec2::ZERO,
        HashSet::from_iter([IVec2::ZERO]),
        vec![],
        Default::default(),
        vec![],
        Entity::PLACEHOLDER,
        0.,
        false,
        "sdk_version",
        false,
        false,
    );
    context.last_action_event = Some(10.0);
    let scene = app.world_mut().spawn(context).id();
    (app, scene)
}

fn request(
    app: &mut App,
    scene: Entity,
    method: &str,
) -> common::rpc::RpcResultReceiver<Result<Value, String>> {
    let (response, receiver) = RpcResultSender::channel();
    app.world_mut().send_event(RpcCallEvent {
        origin: RpcOrigin::Scene {
            entity: scene,
            hash: "signing-test-scene".into(),
        },
        call: if method == "legacy_sign" {
            RpcCall::SignLegacyMessage {
                message: "title: piñata 🦊\nnonce: 42".into(),
                response,
            }
        } else {
            RpcCall::SendAsync {
                body: RPCSendableMessage {
                    method: method.into(),
                    params: vec![json!("0x617262697472617279"), json!("0xaccount")],
                },
                response,
            }
        },
    });
    receiver
}

#[test]
fn legacy_signing_requires_web3_permission_and_propagates_denial() {
    let (mut app, scene) = app();
    let mut receiver = request(&mut app, scene, "legacy_sign");
    app.update();
    assert!(receiver.try_recv().is_err());
    let prompt = app
        .world_mut()
        .resource_mut::<PermissionManager>()
        .pending
        .pop_front()
        .unwrap();
    assert_eq!(prompt.ty, PermissionType::Web3);
    assert_eq!(prompt.scene, scene);
    assert_eq!(
        prompt.additional.as_deref(),
        Some("# DCL Signed message\ntitle: piñata 🦊\nnonce: 42")
    );
    prompt.sender.send(false);
    app.update();
    assert_eq!(
        receiver.try_recv().unwrap(),
        Err("permission denied".into())
    );
}

#[test]
fn legacy_signing_rejects_guests_before_requesting_permission() {
    let (mut app, scene) = app();
    app.world_mut().resource_mut::<Wallet>().finalize_remote(
        Address::ZERO,
        Address::ZERO,
        vec![],
        true,
    );
    let mut receiver = request(&mut app, scene, "legacy_sign");
    app.update();
    assert_eq!(
        receiver.try_recv().unwrap(),
        Err("guest accounts cannot sign transactions".into())
    );
    assert!(app
        .world()
        .resource::<PermissionManager>()
        .pending
        .is_empty());
}

#[test]
fn session_guest_signing_denies_without_prompt_even_after_recent_activity() {
    let (mut app, scene) = app();
    app.world_mut().resource_mut::<Wallet>().finalize_as_guest();
    let wallet = app.world().resource::<Wallet>();
    assert!(wallet.address().is_some());
    assert!(wallet.is_guest());
    assert!(!wallet.is_guest_account());
    // app() supplies a recent action and the default Web3 Ask policy.
    let mut receiver = request(&mut app, scene, "legacy_sign");
    app.update();
    assert!(
        app.world()
            .resource::<PermissionManager>()
            .pending
            .is_empty(),
        "a session guest cannot approve signing, so it must not wait for Web3 permission"
    );
    assert_eq!(
        receiver.try_recv().unwrap(),
        Err("guest accounts cannot sign transactions".into())
    );
}

#[test]
fn legacy_signing_requires_recent_user_activity() {
    let (mut app, scene) = app();
    app.world_mut()
        .get_mut::<RendererSceneContext>(scene)
        .unwrap()
        .last_action_event = Some(0.0);
    let mut receiver = request(&mut app, scene, "legacy_sign");
    app.update();
    assert!(receiver
        .try_recv()
        .unwrap()
        .unwrap_err()
        .starts_with("no recent user activity"));
    assert!(app
        .world()
        .resource::<PermissionManager>()
        .pending
        .is_empty());
}

#[test]
fn unsupported_signing_methods_do_not_prompt() {
    let (mut app, scene) = app();
    let mut receiver = request(&mut app, scene, "eth_sign");
    app.update();
    assert_eq!(
        receiver.try_recv().unwrap(),
        Err("method eth_sign is not available".into())
    );
    assert!(app
        .world()
        .resource::<PermissionManager>()
        .pending
        .is_empty());
}

#[test]
fn legacy_signing_is_unavailable_on_authoritative_servers() {
    // Server mode is process-wide and irreversible: isolate it from other engine tests.
    const CHILD: &str = "DCL_ETHEREUM_SERVER_TEST_CHILD";
    if std::env::var_os(CHILD).is_none() {
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "ethereum_tests::legacy_signing_is_unavailable_on_authoritative_servers",
            ])
            .env(CHILD, "1")
            .output()
            .unwrap();
        assert!(output.status.success(), "{:?}", output);
        assert!(
            String::from_utf8_lossy(&output.stdout).contains("1 passed; 0 failed"),
            "{:?}",
            output
        );
        return;
    }
    common::structs::set_server_mode();
    let (mut app, scene) = app();
    app.world_mut().resource_mut::<Wallet>().finalize_remote(
        Address::ZERO,
        Address::ZERO,
        vec![],
        false,
    );
    let (response, mut account) = RpcResultSender::channel();
    app.world_mut()
        .send_event(RpcCallEvent::engine(RpcCall::GetUserAccount { response }));

    let mut receiver = request(&mut app, scene, "legacy_sign");
    app.update();
    assert_eq!(
        receiver.try_recv().unwrap(),
        Err("eth signing is not available on the authoritative server".into())
    );
    assert_eq!(account.try_recv().unwrap(), None);
    assert!(app
        .world()
        .resource::<PermissionManager>()
        .pending
        .is_empty());
}

#[test]
fn user_account_does_not_wait_for_a_profile() {
    let mut app = App::new();
    app.add_event::<RpcCallEvent>()
        .init_resource::<Wallet>()
        .add_systems(Update, get_user_account);
    for connected in [false, true] {
        if connected {
            app.world_mut().resource_mut::<Wallet>().finalize_remote(
                Address::ZERO,
                Address::ZERO,
                vec![],
                false,
            );
        }
        let (response, mut receiver) = RpcResultSender::channel();
        app.world_mut()
            .send_event(RpcCallEvent::engine(RpcCall::GetUserAccount { response }));
        app.update();
        assert_eq!(
            receiver.try_recv().unwrap(),
            connected.then(|| format!("{:#x}", Address::ZERO))
        );
    }
}

#[test]
fn granting_permission_does_not_allow_an_unconnected_wallet_to_sign() {
    let (mut app, scene) = app();
    let mut receiver = request(&mut app, scene, "legacy_sign");
    app.update();
    let prompt = app
        .world_mut()
        .resource_mut::<PermissionManager>()
        .pending
        .pop_front()
        .unwrap();
    prompt.sender.send(true);
    app.update();
    assert_eq!(
        receiver.try_recv().unwrap(),
        Err("wallet not connected".into())
    );
}

#[test]
fn raw_personal_sign_cannot_bypass_legacy_prefix_or_reach_permission() {
    let (mut app, scene) = app();
    let mut receiver = request(&mut app, scene, "personal_sign");
    app.update();
    assert_eq!(
        receiver.try_recv().unwrap(),
        Err("method personal_sign is not available".into())
    );
    assert!(app
        .world()
        .resource::<PermissionManager>()
        .pending
        .is_empty());
}

#[test]
fn legacy_signing_wallet_payload_matches_displayed_utf8_text_and_engine_account() {
    for fields in [
        "",
        "title: piñata 🦊\nnonce: 42",
        "__proto__: text\nurl: https://example.org",
        "0x617262697472617279",
    ] {
        let signing = EthereumSigningRequest::SignLegacyMessage(fields.into());
        let displayed = signing.display_message().unwrap();
        assert_eq!(displayed, format!("# DCL Signed message\n{fields}"));
        let address = Address::repeat_byte(0x12);
        let payload = signing.into_wallet_request(address);
        assert_eq!(payload.method, "personal_sign");
        assert_eq!(payload.params.len(), 2);
        let signed =
            alloy_core::primitives::hex::decode(payload.params[0].as_str().unwrap()).unwrap();
        assert_eq!(String::from_utf8(signed).unwrap(), displayed);
        assert_eq!(payload.params[1], json!(format!("{address:#x}")));
    }
}

#[test]
fn granting_permission_rechecks_wallet_when_user_becomes_guest() {
    let (mut app, scene) = app();
    app.world_mut().resource_mut::<Wallet>().finalize_remote(
        Address::ZERO,
        Address::ZERO,
        vec![],
        false,
    );
    let mut receiver = request(&mut app, scene, "legacy_sign");
    app.update();
    let prompt = app
        .world_mut()
        .resource_mut::<PermissionManager>()
        .pending
        .pop_front()
        .unwrap();
    app.world_mut().resource_mut::<Wallet>().finalize_as_guest();
    prompt.sender.send(true);
    app.update();
    assert_eq!(
        receiver.try_recv().unwrap(),
        Err("wallet not connected".into())
    );
}

#[test]
fn transaction_and_typed_data_requests_keep_the_existing_web3_permission_path() {
    for method in ["eth_sendTransaction", "eth_signTypedData_v4"] {
        let (mut app, scene) = app();
        let mut receiver = request(&mut app, scene, method);
        app.update();
        let prompt = app
            .world_mut()
            .resource_mut::<PermissionManager>()
            .pending
            .pop_front()
            .unwrap();
        assert_eq!(prompt.ty, PermissionType::Web3);
        assert_eq!(prompt.additional, None);
        prompt.sender.send(false);
        app.update();
        assert_eq!(
            receiver.try_recv().unwrap(),
            Err("permission denied".into())
        );
    }
}

#[test]
fn direct_legacy_signing_events_enforce_utf8_cap_before_prompt_without_truncating() {
    for fields in [
        "a".repeat(MAX_LEGACY_SIGN_MESSAGE_BYTES),
        "🦊".repeat(MAX_LEGACY_SIGN_MESSAGE_BYTES / 4),
    ] {
        for extra in ["", "a"] {
            let fields = fields.clone() + extra;
            let (mut app, scene) = app();
            let (response, mut receiver) = RpcResultSender::channel();
            app.world_mut().send_event(RpcCallEvent {
                origin: RpcOrigin::Scene {
                    entity: scene,
                    hash: "signing-test-scene".into(),
                },
                call: RpcCall::SignLegacyMessage {
                    message: fields.clone(),
                    response,
                },
            });
            app.update();
            if extra.is_empty() {
                assert_eq!(fields.len(), MAX_LEGACY_SIGN_MESSAGE_BYTES);
                assert!(receiver.try_recv().is_err());
                let prompt = app
                    .world_mut()
                    .resource_mut::<PermissionManager>()
                    .pending
                    .pop_front()
                    .unwrap();
                assert_eq!(
                    prompt.additional,
                    Some(format!("# DCL Signed message\n{fields}"))
                );
                prompt.sender.send(false);
                app.update();
                assert_eq!(
                    receiver.try_recv().unwrap(),
                    Err("permission denied".into())
                );
            } else {
                assert_eq!(fields.len(), MAX_LEGACY_SIGN_MESSAGE_BYTES + 1);
                assert_eq!(
                    receiver.try_recv().unwrap(),
                    Err(format!(
                    "legacy signing message exceeds {MAX_LEGACY_SIGN_MESSAGE_BYTES} UTF-8 bytes"
                ))
                );
                assert!(app
                    .world()
                    .resource::<PermissionManager>()
                    .pending
                    .is_empty());
            }
        }
    }
}

use bevy::prelude::*;
use common::{
    rpc::{RpcCall, RpcCallEvent},
    structs::PermissionType,
};
use scene_runner::permissions::Permission;

// PermissionItem values from the kernel API contract. Media hostname grants have no
// native grant store; reporting that permission must not create a new network exemption.
fn permission_type(value: i32) -> Option<PermissionType> {
    match value {
        0 => Some(PermissionType::MovePlayer),
        1 => Some(PermissionType::PlayEmote),
        2 => Some(PermissionType::Web3),
        3 => Some(PermissionType::Websocket),
        4 => Some(PermissionType::Fetch),
        6 => Some(PermissionType::OpenUrl),
        _ => None,
    }
}

fn permission_available(
    value: i32,
    web: bool,
    scene_exists: bool,
    policy: impl FnOnce(PermissionType, bool) -> bool,
) -> bool {
    if !scene_exists {
        return false;
    }
    let Some(ty) = permission_type(value) else {
        return false;
    };
    let networking = matches!(ty, PermissionType::Fetch | PermissionType::Websocket);
    // Web scenes already reach browser fetch and WebSocket without the native prompt.
    // This reports that boundary; it does not waive URL checks in either runtime.
    (web && networking) || policy(ty, networking)
}

pub(super) fn get_permissions(mut events: EventReader<RpcCallEvent>, perms: Permission<()>) {
    for event in events.read() {
        if let RpcCall::GetPermissions {
            permissions,
            response,
        } = &event.call
        {
            let values = permissions
                .iter()
                .map(|value| {
                    let Some(entity) = event.origin.scene() else {
                        return false;
                    };
                    permission_available(
                        *value,
                        cfg!(target_arch = "wasm32"),
                        perms.scene_exists(entity),
                        |ty, allow_out_of_scene| perms.is_allowed(ty, entity, allow_out_of_scene),
                    )
                })
                .collect();
            response.send(values);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn web_network_permission_reflects_the_ungated_browser_path() {
        for value in 0..=6 {
            assert_eq!(
                permission_available(value, true, true, |_, _| false),
                matches!(value, 3 | 4)
            );
            assert!(!permission_available(value, false, true, |_, _| false));
            assert!(!permission_available(value, true, false, |_, _| true));
        }
    }

    #[test]
    fn grants_are_read_from_policy_and_unknown_permissions_are_never_granted() {
        for value in [0, 1, 2, 3, 4, 6] {
            assert!(permission_available(value, false, true, |ty, allow_out| {
                assert_eq!(
                    allow_out,
                    matches!(ty, PermissionType::Fetch | PermissionType::Websocket)
                );
                true
            }));
        }
        for value in [-1, 5, 7, i32::MAX] {
            assert!(!permission_available(value, false, true, |_, _| panic!(
                "no policy for unknown permission"
            )));
        }
    }
    #[test]
    fn native_legacy_probe_uses_scene_realm_and_portable_policy_without_prompting() {
        use common::{
            rpc::{RpcOrigin, RpcResultSender},
            structs::{AppConfig, CurrentRealm, PermissionValue},
        };
        use scene_runner::{
            permissions::PermissionManager, renderer_context::RendererSceneContext,
        };
        let (mut app, scene) = crate::ethereum_tests::app();
        app.add_systems(Update, get_permissions);
        app.world_mut().resource_mut::<CurrentRealm>().about_url = "realm".into();
        app.world_mut()
            .resource_mut::<AppConfig>()
            .default_permissions
            .insert(PermissionType::Fetch, PermissionValue::Allow);
        fn probe(app: &mut App, scene: Entity) -> Vec<bool> {
            let (response, mut receiver) = RpcResultSender::channel();
            app.world_mut().send_event(RpcCallEvent {
                origin: RpcOrigin::Scene {
                    entity: scene,
                    hash: "untrusted-hash-is-not-policy".into(),
                },
                call: RpcCall::GetPermissions {
                    permissions: vec![4, 2, 0, 5, -1],
                    response,
                },
            });
            app.update();
            assert!(app
                .world()
                .resource::<PermissionManager>()
                .pending
                .is_empty());
            receiver.try_recv().unwrap()
        }
        // Fetch may be used out of the scene; actions requiring presence still ask and report false.
        assert_eq!(
            probe(&mut app, scene),
            vec![true, false, false, false, false]
        );
        app.world_mut()
            .resource_mut::<AppConfig>()
            .realm_permissions
            .entry("realm".into())
            .or_default()
            .insert(PermissionType::Fetch, PermissionValue::Deny);
        assert!(!probe(&mut app, scene)[0]);
        app.world_mut()
            .resource_mut::<AppConfig>()
            .scene_permissions
            .entry("signing-test-scene".into())
            .or_default()
            .insert(PermissionType::Fetch, PermissionValue::Allow);
        assert!(probe(&mut app, scene)[0]);
        app.world_mut()
            .resource_mut::<AppConfig>()
            .scene_permissions
            .clear();
        app.world_mut()
            .get_mut::<RendererSceneContext>(scene)
            .unwrap()
            .is_portable = true;
        assert!(
            probe(&mut app, scene)[0],
            "portables do not inherit realm grants or denials"
        );
        app.world_mut()
            .resource_mut::<AppConfig>()
            .default_permissions
            .insert(PermissionType::Fetch, PermissionValue::Ask);
        assert!(
            !probe(&mut app, scene)[0],
            "a probe does not grant an Ask policy"
        );
        assert_eq!(probe(&mut app, Entity::PLACEHOLDER), vec![false; 5]);
    }
}

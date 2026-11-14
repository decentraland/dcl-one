use bevy::prelude::*;
use common::{
    rpc::{RpcCall, RpcCallEvent},
    structs::PermissionType,
};
use scene_runner::permissions::Permission;

// Match the scene runtime cap; the engine enforces it independently of the caller.
const MAX_PERMISSION_QUERIES: usize = 16;

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

fn permission_values<P: Fn(PermissionType, bool) -> bool>(
    permissions: &[i32],
    web: bool,
    resolve_scene: impl FnOnce() -> Option<P>,
) -> Vec<bool> {
    // An empty result rejects oversized queries without scene work or proportional allocation.
    // The runtime requires one result per input and rejects a mismatched response.
    if permissions.len() > MAX_PERMISSION_QUERIES || permissions.is_empty() {
        return Vec::new();
    }
    let Some(policy) = resolve_scene() else {
        return vec![false; permissions.len()];
    };
    permissions
        .iter()
        .map(|value| permission_available(*value, web, true, &policy))
        .collect()
}

pub(super) fn get_permissions(mut events: EventReader<RpcCallEvent>, perms: Permission<()>) {
    for event in events.read() {
        if let RpcCall::GetPermissions {
            permissions,
            response,
        } = &event.call
        {
            response.send(permission_values(
                permissions,
                cfg!(target_arch = "wasm32"),
                || {
                    let policy = perms.for_scene(event.origin.scene()?)?;
                    Some(move |ty, allow_out_of_scene| policy.is_allowed(ty, allow_out_of_scene))
                },
            ));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn oversized_queries_do_no_scene_or_policy_work_and_allocate_no_results() {
        for len in [17, 1_000_000] {
            for web in [false, true] {
                let values = permission_values::<fn(PermissionType, bool) -> bool>(
                    &vec![0; len],
                    web,
                    || panic!("oversized queries must not resolve a scene or policy"),
                );
                assert!(values.is_empty());
                assert_eq!(values.capacity(), 0);
            }
        }
    }

    #[test]
    fn boundary_query_resolves_once_and_preserves_order_duplicates_and_unknown_denials() {
        use std::cell::Cell;
        let resolutions = Cell::new(0);
        let policy_calls = Cell::new(0);
        let permissions = [4, 0, 5, 4, 2, 6, -1, 3, 1, 0, 4, 2, i32::MAX, 6, 3, 1];
        let result = permission_values(&permissions, false, || {
            resolutions.set(resolutions.get() + 1);
            Some(|ty, allow_out| {
                policy_calls.set(policy_calls.get() + 1);
                assert_eq!(
                    allow_out,
                    matches!(ty, PermissionType::Fetch | PermissionType::Websocket)
                );
                matches!(ty, PermissionType::Fetch | PermissionType::PlayEmote)
            })
        });
        assert_eq!(resolutions.get(), 1);
        assert_eq!(policy_calls.get(), 13);
        assert_eq!(
            result,
            vec![
                true, false, false, true, false, false, false, false, true, false, true, false,
                false, false, false, true
            ]
        );
        assert!(
            permission_values::<fn(PermissionType, bool) -> bool>(&[], false, || panic!(
                "empty query"
            ))
            .is_empty()
        );
    }

    #[test]
    fn engine_rejects_oversized_permission_events_and_accepts_the_boundary() {
        use common::rpc::{RpcOrigin, RpcResultSender};
        use common::structs::{AppConfig, PermissionValue};
        let (mut app, scene) = crate::ethereum_tests::app();
        app.add_systems(Update, get_permissions);
        app.world_mut()
            .resource_mut::<AppConfig>()
            .default_permissions
            .insert(PermissionType::Fetch, PermissionValue::Allow);
        for len in [17, 1_000_000, 16] {
            let (response, mut receiver) = RpcResultSender::channel();
            app.world_mut().send_event(RpcCallEvent {
                origin: RpcOrigin::Scene {
                    entity: scene,
                    hash: "untrusted".into(),
                },
                call: RpcCall::GetPermissions {
                    permissions: vec![4; len],
                    response,
                },
            });
            app.update();
            let values = receiver.try_recv().unwrap();
            if len > 16 {
                assert!(values.is_empty());
                assert_eq!(values.capacity(), 0);
            } else {
                assert_eq!(values, vec![true; 16]);
            }
            assert!(app
                .world()
                .resource::<scene_runner::permissions::PermissionManager>()
                .pending
                .is_empty());
        }
    }

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
        app.world_mut()
            .insert_resource(common::autopilot::Autopilot { measure: false });
        assert_eq!(
            probe(&mut app, scene),
            vec![true, false, false, false, false]
        );
        app.world_mut()
            .remove_resource::<common::autopilot::Autopilot>();
        app.world_mut()
            .insert_resource(common::structs::StartupScenes {
                scenes: vec![common::structs::StartupScene {
                    source: "trusted-startup".into(),
                    super_user: true,
                    preview: false,
                    hot_reload: None,
                    hash: Some("signing-test-scene".into()),
                    failed: false,
                }],
            });
        assert_eq!(probe(&mut app, scene), vec![true, true, true, false, false]);
        assert_eq!(probe(&mut app, Entity::PLACEHOLDER), vec![false; 5]);
    }
}

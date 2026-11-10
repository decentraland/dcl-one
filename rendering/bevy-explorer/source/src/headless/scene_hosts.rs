//! The orchestrator's view of the engine's scene hosts.

use bevy::{ecs::entity::Entity, platform::collections::HashMap};
use dcl_deno_ipc::{HostExit, SceneHostInfo};

/// The `scene-host-exited` control event for a lost host whose scenes have `hashes`.
pub fn scene_host_exited_event(exit: &HostExit, hashes: &[&str]) -> serde_json::Value {
    serde_json::json!({
        "type": "scene-host-exited",
        "scenes": hashes,
        "code": exit.code,
        "signal": exit.signal,
        "reason": exit.reason.as_str(),
    })
}

/// The sampled resident memory of the host each scene runs in, keyed by scene entity.
pub fn host_rss_by_scene(hosts: &[SceneHostInfo]) -> HashMap<Entity, u64> {
    hosts
        .iter()
        .filter_map(|host| Some((host.scenes.iter(), host.rss_bytes?)))
        .flat_map(|(scenes, rss)| scenes.map(move |scene| (scene.0, rss)))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use dcl_deno_ipc::{HostExitReason, HostState};
    use dcl_types::SceneId;

    #[derive(serde::Deserialize, Debug, PartialEq)]
    struct Event {
        #[serde(rename = "type")]
        kind: String,
        scenes: Vec<String>,
        code: Option<i32>,
        signal: Option<i32>,
        reason: String,
    }

    fn parsed(exit: &HostExit, hashes: &[&str]) -> Event {
        serde_json::from_value(scene_host_exited_event(exit, hashes)).unwrap()
    }

    #[test]
    fn a_killed_host_reports_its_signal_and_no_code() {
        let exit = HostExit {
            scenes: Vec::new(),
            code: None,
            signal: Some(9),
            reason: HostExitReason::Exit,
        };

        let event = scene_host_exited_event(&exit, &["bafy1", "bafy2"]);

        assert!(event["code"].is_null());
        assert_eq!(
            parsed(&exit, &["bafy1", "bafy2"]),
            Event {
                kind: "scene-host-exited".to_owned(),
                scenes: vec!["bafy1".to_owned(), "bafy2".to_owned()],
                code: None,
                signal: Some(9),
                reason: "exit".to_owned(),
            }
        );
    }

    #[test]
    fn each_reason_has_its_wire_name() {
        for (reason, name) in [
            (HostExitReason::Exit, "exit"),
            (HostExitReason::Protocol, "protocol"),
            (HostExitReason::Memory, "memory"),
            (HostExitReason::Spawn, "spawn"),
        ] {
            let exit = HostExit {
                scenes: Vec::new(),
                code: Some(1),
                signal: None,
                reason,
            };
            let event = parsed(&exit, &["bafy1"]);
            assert_eq!(event.reason, name);
            assert_eq!(event.code, Some(1));
            assert!(scene_host_exited_event(&exit, &["bafy1"])["signal"].is_null());
        }
    }

    #[test]
    fn each_scene_gets_the_memory_of_its_own_sampled_host() {
        let host = |entity: u32, rss_bytes| SceneHostInfo {
            pid: Some(entity),
            state: HostState::Ready,
            scenes: vec![SceneId(Entity::from_raw(entity))],
            rss_bytes,
        };

        let rss = host_rss_by_scene(&[host(1, Some(10)), host(2, Some(20)), host(3, None)]);

        assert_eq!(rss.get(&Entity::from_raw(1)), Some(&10));
        assert_eq!(rss.get(&Entity::from_raw(2)), Some(&20));
        assert_eq!(rss.get(&Entity::from_raw(3)), None);
    }
}

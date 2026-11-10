//! The scene hosts section of the runtime debug panel: the processes that run scene code.

use bevy::prelude::*;
use bevy_console::ConsoleCommand;
use console::{AddDebugSection, DebugInput, DebugInputKind, DebugSection, DoAddConsoleCommand};
use dcl_deno_ipc::{HostState, Placement, PoolInfo};
use serde_json::{json, Value};

use crate::renderer_context::RendererSceneContext;

const MIB: f64 = 1024.0 * 1024.0;

pub(super) fn register(app: &mut App) {
    app.add_console_command::<SceneHostsCommand, _>(scene_hosts)
        .add_preview_console_command::<EndSceneHostCommand, _>(end_scene_host)
        .add_preview_debug_inputs(DebugSection {
            title: "Scene hosts",
            query: "/scene_hosts",
            fields: &[
                ("Placement", "placement"),
                ("Running hosts", "running"),
                ("Hosts", "hosts"),
                ("Lost hosts since start", "lost"),
                ("Messages rejected from hosts since start", "rejected"),
                ("Memory cap per host (MiB)", "memoryCapMiB"),
            ],
            actions: &[],
            inputs: &[DebugInput {
                label: "End the host with process id",
                field: "",
                command: "/scene_host_end {value}",
                kind: DebugInputKind::Number {
                    min: 1.0,
                    max: u32::MAX as f64,
                    step: 1.0,
                },
            }],
        });
}

/// List the scene host processes: placement, each host's process, state, scenes and resident
/// memory, and the hosts lost and messages rejected since start.
#[derive(clap::Parser, ConsoleCommand)]
#[command(name = "/scene_hosts")]
struct SceneHostsCommand;

/// End a scene host as if its process had exited; its scenes are lost.
#[derive(clap::Parser, ConsoleCommand)]
#[command(name = "/scene_host_end")]
struct EndSceneHostCommand {
    pid: u32,
}

fn scene_hosts(mut cmd: ConsoleCommand<SceneHostsCommand>, scenes: Query<&RendererSceneContext>) {
    let Some(Ok(_)) = cmd.take() else { return };
    let Some(pool) = dcl_deno_ipc::scene_host_pool() else {
        cmd.reply_failed("the scene runtime is not initialized");
        return;
    };
    let rejected = dcl_deno_ipc::rejected_host_messages();
    cmd.reply_ok(
        state(&pool, rejected, |scene| {
            let ctx = scenes.get(scene).ok()?;
            Some(format!("{} ({})", ctx.title, ctx.hash))
        })
        .to_string(),
    );
}

fn end_scene_host(mut cmd: ConsoleCommand<EndSceneHostCommand>) {
    let Some(Ok(args)) = cmd.take() else { return };
    if dcl_deno_ipc::end_scene_host(args.pid) {
        cmd.reply_ok(format!("ending scene host {}", args.pid));
    } else {
        cmd.reply_failed(format!(
            "no running scene host with process id {}",
            args.pid
        ));
    }
}

fn state(pool: &PoolInfo, rejected: u64, scene_name: impl Fn(Entity) -> Option<String>) -> Value {
    let hosts = pool
        .hosts
        .iter()
        .map(|host| {
            let scenes = host
                .scenes
                .iter()
                .map(|scene| scene_name(scene.0).unwrap_or_else(|| format!("{:?}", scene.0)))
                .collect::<Vec<_>>();
            json!({
                "pid": host.pid,
                "state": match host.state {
                    HostState::Starting => "starting",
                    HostState::Ready => "ready",
                    HostState::Lost => "lost",
                },
                "scenes": scenes,
                "residentMiB": host.rss_bytes.map(|rss| (rss as f64 / MIB * 10.0).round() / 10.0),
            })
        })
        .collect::<Vec<_>>();
    let mut unavailable = serde_json::Map::new();
    if pool.placement == Placement::Shared {
        unavailable.insert(
            "hosts".into(),
            "Resident memory is sampled only with per-scene hosts.".into(),
        );
    }
    if pool.memory_cap.is_none() {
        unavailable.insert("memoryCapMiB".into(), "No memory cap is set.".into());
    }
    json!({
        "placement": match pool.placement {
            Placement::Shared => "shared",
            Placement::PerScene => "per-scene",
        },
        "running": hosts.len(),
        "hosts": hosts,
        "lost": pool.lost_hosts,
        "rejected": rejected,
        "memoryCapMiB": pool.memory_cap.map(|cap| cap as f64 / MIB),
        "unavailable": unavailable,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use dcl_deno_ipc::SceneHostInfo;
    use dcl_types::SceneId;

    fn pool(placement: Placement, memory_cap: Option<u64>) -> PoolInfo {
        PoolInfo {
            placement,
            memory_cap,
            hosts: vec![SceneHostInfo {
                pid: Some(42),
                state: HostState::Ready,
                scenes: vec![SceneId(Entity::from_raw(7))],
                rss_bytes: memory_cap.map(|_| 3 * 1024 * 1024 / 2),
            }],
            lost_hosts: 2,
        }
    }

    #[test]
    fn per_scene_hosts_show_their_memory_and_cap() {
        let state = state(
            &pool(Placement::PerScene, Some(512 * 1024 * 1024)),
            5,
            |_| Some("Plaza (bafy)".to_owned()),
        );

        assert_eq!(state["placement"], "per-scene");
        assert_eq!(state["running"], 1);
        assert_eq!(state["hosts"][0]["pid"], 42);
        assert_eq!(state["hosts"][0]["state"], "ready");
        assert_eq!(state["hosts"][0]["scenes"][0], "Plaza (bafy)");
        assert_eq!(state["hosts"][0]["residentMiB"], 1.5);
        assert_eq!(state["memoryCapMiB"], 512.0);
        assert_eq!(state["lost"], 2);
        assert_eq!(state["rejected"], 5);
        assert!(state["unavailable"].as_object().unwrap().is_empty());
    }

    #[test]
    fn shared_hosts_explain_the_missing_memory_readings() {
        let state = state(&pool(Placement::Shared, None), 0, |_| None);

        assert_eq!(state["placement"], "shared");
        assert!(state["hosts"][0]["residentMiB"].is_null());
        assert!(state["memoryCapMiB"].is_null());
        assert!(state["unavailable"]["hosts"].is_string());
        assert!(state["unavailable"]["memoryCapMiB"].is_string());
        assert!(state["hosts"][0]["scenes"][0].is_string());
    }
}

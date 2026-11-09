//! The portables the player can see and stop: every portable but startup ones and the HUD's own
//! scene, plus the smart wearables stopped while still held. The HUD follows them through
//! `GetPortablesStream`; the console lists them (`/portables`) and stops one (`/stop_portable`).

use bevy::{ecs::system::SystemParam, prelude::*};
use bevy_console::ConsoleCommand;
use common::rpc::RpcStreamSender;
use console::DoAddConsoleCommand;
use scene_runner::{
    initialize_scene::{LiveScenes, PortableKind, PortableScenes, SceneLoading, SuperUserScene},
    portables::{is_listed, PORTABLE_EXPERIENCES_BLOCK},
    renderer_context::RendererSceneContext,
};
use system_bridge::{PortableInfo, PortableInfoKind, PortableState, SystemApi};

use crate::smart_wearables::SmartWearables;

pub struct PortablesPlugin;

impl Plugin for PortablesPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<PortableStreams>();
        app.add_systems(Update, stream_portables);
        app.add_console_command::<PortablesCommand, _>(portables_command);
        app.add_console_command::<StopPortableCommand, _>(stop_portable_command);
    }
}

#[derive(Resource, Default)]
struct PortableStreams {
    senders: Vec<RpcStreamSender<Vec<PortableInfo>>>,
    last: Option<Vec<PortableInfo>>,
}

/// What the live scenes say about portables.
#[derive(SystemParam)]
pub struct PortableScenesView<'w, 's> {
    pub portables: Res<'w, PortableScenes>,
    live_scenes: Res<'w, LiveScenes>,
    scenes: Query<
        'w,
        's,
        (
            Option<&'static RendererSceneContext>,
            Option<&'static SceneLoading>,
            Has<SuperUserScene>,
        ),
    >,
}

impl PortableScenesView<'_, '_> {
    /// The state of the scene `hash`, which should be running.
    pub fn scene_state(&self, hash: &str) -> (PortableState, Option<String>) {
        let Some((ctx, loading, _)) = self
            .live_scenes
            .scenes
            .get(hash)
            .and_then(|scene| self.scenes.get(*scene).ok())
        else {
            return (PortableState::Loading, None);
        };
        match ctx {
            _ if matches!(loading, Some(SceneLoading::Failed)) => (
                PortableState::Failed,
                Some("the scene failed to load".to_owned()),
            ),
            None => (PortableState::Loading, None),
            Some(ctx) if ctx.broken() => {
                (PortableState::Failed, Some("the scene broke".to_owned()))
            }
            Some(ctx) if ctx.blocked.contains(PORTABLE_EXPERIENCES_BLOCK) => {
                (PortableState::Blocked, None)
            }
            Some(_) => (PortableState::Running, None),
        }
    }

    fn title(&self, hash: &str) -> Option<&str> {
        let scene = self.live_scenes.scenes.get(hash)?;
        let (ctx, ..) = self.scenes.get(*scene).ok()?;
        ctx.map(|ctx| ctx.title.as_str()).filter(|t| !t.is_empty())
    }

    /// Whether `hash` is a portable the player sees and may stop.
    pub fn is_listed(&self, hash: &str) -> bool {
        let Some(source) = self.portables.get(hash) else {
            return false;
        };
        let is_super = self
            .live_scenes
            .scenes
            .get(hash)
            .and_then(|scene| self.scenes.get(*scene).ok())
            .is_some_and(|(.., is_super)| is_super);
        is_listed(source, is_super)
    }
}

/// The listed portables, by name: the live ones, then the smart wearables not (yet, or no
/// longer) in `PortableScenes`.
pub fn portable_list(view: &PortableScenesView, sw: &SmartWearables) -> Vec<PortableInfo> {
    let mut list: Vec<PortableInfo> = view
        .portables
        .iter()
        .filter(|(hash, _)| view.is_listed(hash))
        .map(|(hash, source)| {
            let (state, reason) = view.scene_state(hash);
            let (name, kind, smart_source) = match &source.kind {
                PortableKind::SmartWearable { name, .. } => (
                    name.clone(),
                    PortableInfoKind::SmartWearable,
                    sw.model.entries.get(&source.pid).map(|e| e.source),
                ),
                _ => (
                    view.title(hash)
                        .map(ToOwned::to_owned)
                        .or_else(|| source.ens.clone())
                        .unwrap_or_else(|| source.pid.clone()),
                    PortableInfoKind::Scene,
                    None,
                ),
            };
            PortableInfo {
                hash: Some(hash.clone()),
                pid: source.pid.clone(),
                name,
                kind,
                state,
                reason,
                source: smart_source,
            }
        })
        .collect();

    for (urn, entry) in sw.model.entries.iter() {
        let scene_id = entry.scene_id();
        if scene_id
            .as_ref()
            .is_some_and(|id| view.portables.contains_key(id))
        {
            continue;
        }
        // not running: loading, stopped, failed to resolve; a scene that should run but is not
        // inserted yet is loading
        let Some((state, reason)) = sw.model.state_of(urn, |_| (PortableState::Loading, None))
        else {
            continue;
        };
        list.push(PortableInfo {
            hash: None,
            pid: urn.clone(),
            name: entry.name.clone(),
            kind: PortableInfoKind::SmartWearable,
            state,
            reason,
            source: Some(entry.source),
        });
    }

    list.sort_by(|a, b| (&a.name, &a.pid).cmp(&(&b.name, &b.pid)));
    list
}

fn stream_portables(
    mut streams: ResMut<PortableStreams>,
    mut events: EventReader<SystemApi>,
    view: PortableScenesView,
    sw: Res<SmartWearables>,
) {
    let new: Vec<_> = events
        .read()
        .filter_map(|ev| match ev {
            SystemApi::GetPortablesStream(sender) => Some(sender.clone()),
            _ => None,
        })
        .collect();
    streams.senders.extend(new.iter().cloned());
    streams.senders.retain(|s| !s.is_closed());
    if streams.senders.is_empty() {
        streams.last = None;
        return;
    }

    let list = portable_list(&view, &sw);
    if streams.last.as_ref() != Some(&list) {
        for sender in streams.senders.iter() {
            let _ = sender.send(list.clone());
        }
        streams.last = Some(list);
    } else {
        for sender in new {
            let _ = sender.send(list.clone());
        }
    }
}

fn state_name<T: serde::Serialize>(value: T) -> String {
    serde_json::to_value(value)
        .ok()
        .and_then(|v| v.as_str().map(ToOwned::to_owned))
        .unwrap_or_default()
}

/// One line per listed portable, `<name> [<hash or ->, <kind>, <state>]`.
pub fn format_portables(list: &[PortableInfo]) -> String {
    if list.is_empty() {
        return "no portables".to_owned();
    }
    list.iter()
        .map(|p| {
            format!(
                "{} [{}, {}, {}]",
                p.name,
                p.hash.as_deref().unwrap_or("-"),
                state_name(p.kind),
                state_name(p.state),
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// List the portables the player can see and stop.
#[derive(clap::Parser, ConsoleCommand)]
#[command(name = "/portables")]
struct PortablesCommand;

fn portables_command(
    mut input: ConsoleCommand<PortablesCommand>,
    view: PortableScenesView,
    sw: Res<SmartWearables>,
) {
    if let Some(Ok(_)) = input.take() {
        input.reply_ok(format_portables(&portable_list(&view, &sw)));
    }
}

/// Stop a listed portable by its scene hash. A smart wearable stays equipped and can be
/// restarted.
#[derive(clap::Parser, ConsoleCommand)]
#[command(name = "/stop_portable")]
struct StopPortableCommand {
    hash: String,
}

fn stop_portable_command(
    mut input: ConsoleCommand<StopPortableCommand>,
    mut commands: Commands,
    view: PortableScenesView,
    mut sw: ResMut<SmartWearables>,
) {
    let Some(Ok(command)) = input.take() else {
        return;
    };
    let hash = command.hash.trim().to_owned();
    if !view.is_listed(&hash) {
        input.reply_failed(format!("not a listed portable: {hash}"));
        return;
    }
    if let Some(source) = view.portables.get(&hash) {
        if source.is_smart_wearable() {
            // the plugin removes it from PortableScenes and keeps it as stopped
            sw.model.stop(&source.pid);
        } else {
            let hash = hash.clone();
            // deferred: PortableScenesView reads PortableScenes
            commands.queue(move |world: &mut World| {
                world.resource_mut::<PortableScenes>().remove(&hash);
            });
        }
    }
    input.reply_ok(format!("stopped {hash}"));
}

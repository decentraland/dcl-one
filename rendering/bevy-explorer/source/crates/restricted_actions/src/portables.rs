//! The portables the player can see and stop: every portable but startup ones and the HUD's own
//! scene, plus the smart wearables stopped while still held. The HUD follows them through
//! `GetPortablesStream`; the console lists them (`/portables`) and stops one (`/stop_portable`).
//! A stop from anywhere (a scene's `kill` or `exit`, the console) is a [`StopPortable`] event,
//! applied by one system.

use std::hash::{DefaultHasher, Hash, Hasher};

use bevy::{ecs::system::SystemParam, prelude::*};
use bevy_console::ConsoleCommand;
use common::{rpc::RpcStreamSender, sets::SceneSets};
use console::DoAddConsoleCommand;
use scene_runner::{
    initialize_scene::{LiveScenes, PortableKind, PortableScenes, SceneLoading, SuperUserScene},
    portables::{is_listed, PORTABLE_EXPERIENCES_BLOCK},
    renderer_context::RendererSceneContext,
};
use system_bridge::{PortableInfo, PortableInfoKind, PortableState, SystemApi};

use crate::smart_wearables::{Model, Phase, SmartWearables};

pub struct PortablesPlugin;

impl Plugin for PortablesPlugin {
    fn build(&self, app: &mut App) {
        app.add_event::<StopPortable>();
        app.init_resource::<PortableStreams>();
        app.add_systems(
            Update,
            (
                stop_portables
                    .after(crate::kill_portable)
                    .after(crate::exit_portable),
                stream_portables.after(stop_portables),
            )
                .in_set(SceneSets::RestrictedActions),
        );
        app.add_console_command::<PortablesCommand, _>(portables_command);
        app.add_console_command::<StopPortableCommand, _>(stop_portable_command);
    }
}

/// Stop the portable with this scene hash: it leaves `PortableScenes`; a smart wearable is also
/// recorded as stopped, so it is not started again while still held until restarted.
#[derive(Event, Debug)]
pub(crate) struct StopPortable {
    pub hash: String,
}

fn stop_portables(
    mut stops: EventReader<StopPortable>,
    mut portables: ResMut<PortableScenes>,
    mut sw: ResMut<SmartWearables>,
) {
    for StopPortable { hash } in stops.read() {
        let Some(source) = portables.remove(hash) else {
            continue;
        };
        info!("stopping portable {hash} ({})", source.pid);
        if source.is_smart_wearable() {
            sw.model.stop(&source.pid);
        }
    }
}

#[derive(Resource, Default)]
struct PortableStreams {
    senders: Vec<RpcStreamSender<Vec<PortableInfo>>>,
    /// a digest of what the last list was built from; the list is rebuilt when it changes
    fingerprint: Option<u64>,
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

    /// The listed portables of `PortableScenes`, unsorted.
    fn live_rows<'a>(&'a self, sw: &'a SmartWearables) -> impl Iterator<Item = PortableInfo> + 'a {
        self.portables
            .iter()
            .filter(|(hash, _)| self.is_listed(hash))
            .map(move |(hash, source)| {
                let (state, reason) = self.scene_state(hash);
                let (name, kind, smart_source) = match &source.kind {
                    PortableKind::SmartWearable { name, .. } => (
                        name.clone(),
                        PortableInfoKind::SmartWearable,
                        sw.model.entries.get(&source.pid).map(|e| e.source),
                    ),
                    _ => (
                        self.title(hash)
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
    }
}

/// The smart wearables known to be smart (their scene resolved) whose scene is not in
/// `PortableScenes`: stopped, or about to be inserted. A wearable still waiting for its data, one
/// that failed to resolve, or one with no scene is not a portable and is not listed: every
/// equipped wearable passes through those phases.
pub fn held_rows(model: &Model, running: impl Fn(&str) -> bool) -> Vec<PortableInfo> {
    model
        .entries
        .iter()
        .filter(|(_, entry)| matches!(entry.phase, Phase::Ready { .. }))
        .filter(|(_, entry)| !entry.scene_id().is_some_and(|id| running(&id)))
        .map(|(urn, entry)| PortableInfo {
            hash: None,
            pid: urn.clone(),
            name: entry.name.clone(),
            kind: PortableInfoKind::SmartWearable,
            state: if model.stopped.contains(urn) {
                PortableState::Stopped
            } else {
                PortableState::Loading
            },
            reason: None,
            source: Some(entry.source),
        })
        .collect()
}

/// The listed portables, by name: the live ones, then the smart wearables not (yet, or no
/// longer) in `PortableScenes`.
pub fn portable_list(view: &PortableScenesView, sw: &SmartWearables) -> Vec<PortableInfo> {
    let mut list: Vec<PortableInfo> = view.live_rows(sw).collect();
    list.extend(held_rows(&sw.model, |id| view.portables.contains_key(id)));
    list.sort_by(|a, b| (&a.name, &a.pid).cmp(&(&b.name, &b.pid)));
    list
}

/// A digest of everything `portable_list` reads, without building it: the list is rebuilt only
/// when this changes. Order-independent, since `PortableScenes` is a hash map.
fn fingerprint(view: &PortableScenesView, sw: &SmartWearables) -> u64 {
    fn digest(hash: impl Hash) -> u64 {
        let mut hasher = DefaultHasher::new();
        hash.hash(&mut hasher);
        hasher.finish()
    }
    let mut total = 0u64;
    for (hash, source) in view.portables.iter() {
        if !view.is_listed(hash) {
            continue;
        }
        let (state, reason) = view.scene_state(hash);
        total = total.wrapping_add(digest((
            hash,
            &source.pid,
            view.title(hash),
            state as u8,
            reason,
        )));
    }
    for (urn, entry) in sw.model.entries.iter() {
        total = total.wrapping_add(digest((
            urn,
            &entry.name,
            matches!(entry.phase, Phase::Ready { .. }),
            entry.scene_id(),
            entry.source as u8,
            sw.model.stopped.contains(urn),
        )));
    }
    total
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
    if new.is_empty() && streams.senders.is_empty() {
        return;
    }
    streams.senders.extend(new.iter().cloned());
    streams.senders.retain(|s| !s.is_closed());
    if streams.senders.is_empty() {
        streams.fingerprint = None;
        streams.last = None;
        return;
    }

    let fingerprint = fingerprint(&view, &sw);
    if streams.fingerprint != Some(fingerprint) {
        let list = portable_list(&view, &sw);
        streams.fingerprint = Some(fingerprint);
        if streams.last.as_ref() != Some(&list) {
            for sender in streams.senders.iter() {
                let _ = sender.send(list.clone());
            }
            streams.last = Some(list);
            return;
        }
    }
    if let Some(list) = streams.last.as_ref() {
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
    view: PortableScenesView,
    mut stops: EventWriter<StopPortable>,
) {
    let Some(Ok(command)) = input.take() else {
        return;
    };
    let hash = command.hash.trim().to_owned();
    if !view.is_listed(&hash) {
        input.reply_failed(format!("not a listed portable: {hash}"));
        return;
    }
    input.reply_ok(format!("stopped {hash}"));
    stops.write(StopPortable { hash });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::smart_wearables::{normalize_urn, EquippedSet};

    const HAT: &str = "urn:decentraland:matic:collections-v2:0xabc:1";
    const CAPE: &str = "urn:decentraland:matic:collections-v2:0xabc:2";
    const SCARF: &str = "urn:decentraland:matic:collections-v2:0xabc:3";

    fn model(urns: &[&str]) -> Model {
        let mut model = Model::default();
        model.observe_profile(Some(EquippedSet {
            urns: urns.iter().map(|u| normalize_urn(u)).collect(),
            shape: "male",
        }));
        model
    }

    #[test]
    fn only_a_resolved_smart_wearable_is_listed_while_not_running() {
        let mut model = model(&[HAT, CAPE, SCARF]);
        // HAT is still waiting for its data, CAPE failed to resolve: neither is known to be smart
        model.set_phase(
            &normalize_urn(CAPE),
            Phase::Failed("wearable Missing".to_owned()),
        );
        model.set_phase(
            &normalize_urn(SCARF),
            Phase::Ready {
                collection: "bafyscarf".to_owned(),
            },
        );
        let rows = held_rows(&model, |_| false);
        assert_eq!(
            rows.iter().map(|r| r.pid.as_str()).collect::<Vec<_>>(),
            vec![normalize_urn(SCARF)]
        );
        assert_eq!(rows[0].state, PortableState::Loading);
        assert_eq!(rows[0].hash, None);

        // once its scene is in PortableScenes the live row stands for it
        assert!(held_rows(&model, |id| id == "bafyscarf-sw-male").is_empty());

        assert!(model.stop(&normalize_urn(SCARF)));
        let rows = held_rows(&model, |_| false);
        assert_eq!(rows[0].state, PortableState::Stopped);
    }

    #[test]
    fn the_listing_names_each_portable_with_its_hash_kind_and_state() {
        let list = vec![
            PortableInfo {
                hash: Some("bafyscene".to_owned()),
                pid: "urn:decentraland:entity:bafyscene".to_owned(),
                name: "A scene".to_owned(),
                kind: PortableInfoKind::Scene,
                state: PortableState::Running,
                reason: None,
                source: None,
            },
            PortableInfo {
                hash: None,
                pid: normalize_urn(HAT),
                name: "Hat".to_owned(),
                kind: PortableInfoKind::SmartWearable,
                state: PortableState::Stopped,
                reason: None,
                source: None,
            },
        ];
        assert_eq!(
            format_portables(&list),
            "A scene [bafyscene, scene, running]\nHat [-, smartWearable, stopped]"
        );
        assert_eq!(format_portables(&[]), "no portables");
    }
}

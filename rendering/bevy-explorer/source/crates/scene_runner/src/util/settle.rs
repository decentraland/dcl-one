//! Waiting for a parcel scene to settle, for the commands that replace one (`/teleport --wait`,
//! `/reload --wait`), and `/scene_state`, the readiness they wait for.

use bevy::{diagnostic::FrameCount, prelude::*};
use bevy_console::ConsoleCommand;
use common::{
    rpc::RpcResultReceiver,
    structs::{OutOfWorld, PrimaryUser, ReturnedToWorld},
};
use serde_json::{json, Value};

use crate::{
    debug_panel::{scene_on_parcel, stage_name, NO_SCENE},
    initialize_scene::{LiveScenes, SceneHash, SceneLoading, ScenePointers, SuperUserScene},
    load_timing::{scene_is_halted, READY_TICK},
    renderer_context::RendererSceneContext,
    vec3_to_parcel,
};

/// Frames waited once the scene is ready, so the frame it first drew in has been presented: the
/// render world draws a frame while the main world runs the next one.
const PRESENT_FRAMES: u32 = 2;
pub const DEFAULT_TIMEOUT_SECONDS: f32 = 60.0;
const MAX_TIMEOUT_SECONDS: f32 = 300.0;

/// `--timeout-seconds` of a settling command, checked.
pub fn timeout_seconds(timeout: Option<f32>) -> Result<f32, String> {
    let timeout = timeout.unwrap_or(DEFAULT_TIMEOUT_SECONDS);
    if !timeout.is_finite() || timeout <= 0.0 || timeout > MAX_TIMEOUT_SECONDS {
        return Err(format!(
            "--timeout-seconds takes more than 0 and at most {MAX_TIMEOUT_SECONDS}; got {timeout}"
        ));
    }
    Ok(timeout)
}

/// A scene's lifecycle state as the settling replies and `/scene_state` name it: `loading`
/// (definition, main.crdt or JavaScript still being fetched), `failed`, `starting` (running, not
/// yet [`READY_TICK`]), `blocked`, `broken` or `running`.
fn state_name(
    loading: Option<&SceneLoading>,
    context: Option<&RendererSceneContext>,
) -> &'static str {
    match (loading, context) {
        (Some(SceneLoading::Failed), _) => "failed",
        (Some(_), _) | (None, None) => "loading",
        (None, Some(context)) if context.broken() => "broken",
        (None, Some(context)) if !context.blocked.is_empty() => "blocked",
        (None, Some(context)) if context.tick_number < READY_TICK => "starting",
        _ => "running",
    }
}

/// Whether the scene runs and has ticked to [`READY_TICK`].
fn is_ready(loading: Option<&SceneLoading>, context: Option<&RendererSceneContext>) -> bool {
    state_name(loading, context) == "running"
}

/// Why the scene will never be ready, for one that failed to load or is halted
/// ([`scene_is_halted`]: broken or frozen).
fn never_ready(
    loading: Option<&SceneLoading>,
    context: Option<&RendererSceneContext>,
) -> Option<&'static str> {
    match (loading, context) {
        (Some(SceneLoading::Failed), _) => Some("failed"),
        (Some(_), _) | (None, None) => None,
        (None, Some(context)) if context.broken() => Some("broken"),
        (None, Some(context)) if scene_is_halted(context) => Some("frozen"),
        _ => None,
    }
}

enum Target {
    /// The parcel scene the player lands in after a teleport. `arrived` is the
    /// [`ReturnedToWorld`] seen since the teleport.
    Arrival {
        parcel: IVec2,
        arrived: Option<ReturnedToWorld>,
    },
    /// A new instance of a reloaded scene: any entity for the hash but the one replaced.
    Reload { hash: String, replaced: Entity },
}

struct SceneWait {
    target: Target,
    /// The teleport request a realm change waits on before the wait starts.
    gate: Option<RpcResultReceiver<Result<(), String>>>,
    /// The reply's fields known when the wait started.
    reply: Value,
    timeout: f32,
    elapsed: f32,
    /// The scene entity watched and its tick when first seen (0 for a scene that started since).
    watched: Option<(Entity, u32)>,
    /// The frame the scene was first seen ready in.
    ready: Option<u32>,
    sender: Option<tokio::sync::oneshot::Sender<Result<String, String>>>,
}

/// The settling waits in flight, finished by [`settle_scenes`].
#[derive(Resource, Default)]
pub struct SceneWaits(Vec<SceneWait>);

impl SceneWaits {
    /// How many commands wait for a scene to settle.
    pub(crate) fn pending(&self) -> usize {
        self.0.len()
    }

    /// Wait for the player to land on `parcel` and its scene to be ready, after `gate` (a realm
    /// change's teleport request) resolves. The reply is `reply` with the outcome added.
    pub fn arrival(
        &mut self,
        parcel: IVec2,
        timeout: f32,
        gate: Option<RpcResultReceiver<Result<(), String>>>,
        reply: Value,
    ) -> tokio::sync::oneshot::Receiver<Result<String, String>> {
        self.push(
            Target::Arrival {
                parcel,
                arrived: None,
            },
            gate,
            reply,
            timeout,
        )
    }

    /// Wait for a new instance of the scene `hash` to be ready; `replaced` is the scene entity the
    /// reload removed from [`LiveScenes`].
    pub(super) fn reload(
        &mut self,
        hash: String,
        replaced: Entity,
        timeout: f32,
    ) -> tokio::sync::oneshot::Receiver<Result<String, String>> {
        let reply = json!({ "reloaded": true, "sceneHash": hash });
        self.push(Target::Reload { hash, replaced }, None, reply, timeout)
    }

    fn push(
        &mut self,
        target: Target,
        gate: Option<RpcResultReceiver<Result<(), String>>>,
        reply: Value,
        timeout: f32,
    ) -> tokio::sync::oneshot::Receiver<Result<String, String>> {
        let (sender, receiver) = tokio::sync::oneshot::channel();
        self.0.push(SceneWait {
            target,
            gate,
            reply,
            timeout,
            elapsed: 0.0,
            watched: None,
            ready: None,
            sender: Some(sender),
        });
        receiver
    }
}

#[allow(clippy::type_complexity)]
pub(super) fn settle_scenes(
    mut waits: ResMut<SceneWaits>,
    mut returned: EventReader<ReturnedToWorld>,
    time: Res<Time<Real>>,
    frame: Res<FrameCount>,
    live: Res<LiveScenes>,
    pointers: Option<Res<ScenePointers>>,
    player: Query<Has<OutOfWorld>, With<PrimaryUser>>,
    scenes: Query<(Option<&SceneLoading>, Option<&RendererSceneContext>), Without<SuperUserScene>>,
) {
    let landed = returned.read().last().copied();
    if waits.0.is_empty() {
        return;
    }
    let out_of_world = player.single().unwrap_or(true);
    let dt = time.delta_secs();

    waits.0.retain_mut(|wait| {
        if wait.sender.as_ref().is_none_or(|s| s.is_closed()) {
            return false;
        }
        if let Some(gate) = wait.gate.as_mut() {
            match gate.poll_once() {
                Ok(None) => return true,
                Ok(Some(Ok(()))) => wait.gate = None,
                Ok(Some(Err(e))) => return wait.finish(Err(e)),
                Err(()) => return wait.finish(Err("the teleport was cancelled".to_owned())),
            }
        }
        wait.elapsed += dt;

        // the scene entity to watch, or why there is none yet
        let entity = match &mut wait.target {
            Target::Arrival { parcel, arrived } => {
                if let Some(landed) = landed {
                    arrived.get_or_insert(landed);
                }
                if arrived.is_none() || out_of_world {
                    Err("Waiting for the player to land.")
                } else {
                    match scene_on_parcel(pointers.as_deref(), &live, *parcel) {
                        Err(NO_SCENE) => {
                            wait.reply["arrived"] = json!(true);
                            wait.reply["sceneReady"] = json!(false);
                            wait.reply["sceneHash"] = Value::Null;
                            wait.reply["sceneState"] = json!("none");
                            wait.reply["renderedFrames"] = json!(0);
                            wait.reply["settled"] = json!(true);
                            wait.reply["settleMs"] = json!((wait.elapsed * 1000.0).round());
                            let warning =
                                format!("there is no scene on parcel {},{}", parcel.x, parcel.y);
                            return wait.finish_with_warning(Some(warning));
                        }
                        scene => scene,
                    }
                }
            }
            Target::Reload { hash, replaced } => match live.scenes.get(hash.as_str()) {
                Some(entity) if entity != replaced => Ok(*entity),
                _ => Err("Waiting for the scene to spawn again."),
            },
        };

        let (loading, context) = entity
            .ok()
            .and_then(|e| scenes.get(e).ok())
            .unwrap_or((None, None));
        if let Ok(entity) = entity {
            if wait.watched.is_none_or(|(watched, _)| watched != entity) {
                let tick = match wait.target {
                    // a reloaded scene is a new one: every tick it has, it ran since
                    Target::Reload { .. } => 0,
                    Target::Arrival { .. } => context.map_or(0, |c| c.tick_number),
                };
                wait.watched = Some((entity, tick));
                wait.ready = None;
            }
        }
        let state = state_name(loading, context);
        let ready = entity.is_ok() && is_ready(loading, context);
        if ready && wait.ready.is_none() {
            wait.ready = Some(frame.0);
        } else if !ready {
            wait.ready = None;
        }

        let presented = wait
            .ready
            .is_some_and(|at| frame.0.wrapping_sub(at) >= PRESENT_FRAMES);
        let halted = never_ready(loading, context).filter(|_| entity.is_ok());
        let expired = wait.elapsed >= wait.timeout;
        if !(presented || halted.is_some() || expired) {
            return true;
        }

        if let Some(context) = context {
            wait.reply["sceneHash"] = json!(context.hash.as_ref());
            wait.reply["title"] = json!(context.title);
            let since = wait.watched.map_or(0, |(_, tick)| tick);
            wait.reply["ticksWaited"] = json!(context.tick_number.saturating_sub(since));
            if !context.blocked.is_empty() {
                let mut blocked: Vec<_> = context.blocked.iter().collect();
                blocked.sort();
                wait.reply["blocked"] = json!(blocked);
            }
        }
        if let Target::Arrival { arrived, .. } = &wait.target {
            wait.reply["arrived"] = json!(arrived.is_some() && !out_of_world);
            wait.reply["sceneReady"] = json!(ready);
        }
        wait.reply["sceneState"] = json!(state);
        wait.reply["renderedFrames"] = json!(u32::from(presented));
        wait.reply["settleMs"] = json!((wait.elapsed * 1000.0).round());
        wait.reply["settled"] = json!(presented);
        let warning = if presented {
            None
        } else if let Some(halted) = halted {
            Some(format!("the scene is {halted}; it will not become ready"))
        } else {
            let waiting = match entity {
                Err(why) => why.to_owned(),
                Ok(_) if ready => "Waiting for a frame to be drawn.".to_owned(),
                Ok(_) => format!("Waiting for the scene to run (it is {state})."),
            };
            Some(format!(
                "timed out after {} s. {waiting} Is the simulation paused?",
                wait.timeout
            ))
        };
        wait.finish_with_warning(warning)
    });
}

impl SceneWait {
    /// Send the reply; `false`, so `retain_mut` drops the wait.
    fn finish(&mut self, result: Result<String, String>) -> bool {
        if let Some(sender) = self.sender.take() {
            let _ = sender.send(result);
        }
        false
    }

    fn finish_with_warning(&mut self, warning: Option<String>) -> bool {
        let mut reply = std::mem::take(&mut self.reply);
        if matches!(self.target, Target::Reload { .. }) && reply["settled"] == json!(true) {
            reply["hint"] = json!(
                "the scene started again from scratch: its state, counters and scene-UI ids are \
                 new; its startup logs are in /scene_logs again"
            );
        }
        if let Some(warning) = warning {
            reply["warning"] = json!(warning);
        }
        self.finish(Ok(reply.to_string()))
    }
}

/// The parcel scene under the player: its id (the hash), title, base parcel, lifecycle state,
/// whether it is ready, its loading stage and SDK version
#[derive(clap::Parser, ConsoleCommand)]
#[command(name = "/scene_state")]
pub(super) struct SceneStateCommand;

pub(super) fn scene_state_command(
    mut input: ConsoleCommand<SceneStateCommand>,
    live: Res<LiveScenes>,
    pointers: Option<Res<ScenePointers>>,
    player: Query<(&GlobalTransform, Has<OutOfWorld>), With<PrimaryUser>>,
    scenes: Query<
        (
            Option<&SceneHash>,
            Option<&SceneLoading>,
            Option<&RendererSceneContext>,
        ),
        Without<SuperUserScene>,
    >,
) {
    if input.take().is_none() {
        return;
    }
    let Ok((transform, out_of_world)) = player.single() else {
        input.reply_failed("the player has no position yet");
        return;
    };
    let parcel = vec3_to_parcel(transform.translation());
    let mut reply = json!({
        "playerParcel": [parcel.x, parcel.y],
        "outOfWorld": out_of_world,
    });
    let root = match scene_on_parcel(pointers.as_deref(), &live, parcel) {
        Ok(root) => root,
        Err(why) => {
            reply["state"] = json!(if why == NO_SCENE { "none" } else { "loading" });
            reply["isReady"] = json!(false);
            reply["loadingStage"] = json!(why);
            input.reply_ok(reply.to_string());
            return;
        }
    };
    let (hash, loading, context) = scenes.get(root).unwrap_or((None, None, None));
    let state = state_name(loading, context);
    if let Some(hash) = hash {
        reply["sceneId"] = json!(hash.0);
    }
    reply["state"] = json!(state);
    reply["isReady"] = json!(is_ready(loading, context));
    reply["loadingStage"] = json!(match state {
        "starting" => "Starting",
        _ => stage_name(loading, context),
    });
    if let Some(context) = context {
        reply["title"] = json!(context.title);
        reply["parcel"] = json!([context.base.x, context.base.y]);
        reply["sdkVersion"] = json!(context.sdk_version);
        reply["tick"] = json!(context.tick_number);
        if !context.blocked.is_empty() {
            let mut blocked: Vec<_> = context.blocked.iter().collect();
            blocked.sort();
            reply["blocked"] = json!(blocked);
        }
    }
    input.reply_ok(reply.to_string());
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use dcl_types::SceneId;

    use crate::{initialize_scene::PointerResult, renderer_context::FROZEN_BLOCK};
    use scene_material::SceneBounds;

    use super::*;

    fn app() -> App {
        let mut app = App::new();
        app.init_resource::<SceneWaits>()
            .init_resource::<LiveScenes>()
            .init_resource::<FrameCount>()
            .init_resource::<Time<Real>>()
            .add_event::<ReturnedToWorld>()
            .add_systems(Update, settle_scenes);
        app.world_mut().spawn((
            PrimaryUser::default(),
            GlobalTransform::from_translation(Vec3::new(8.0, 0.0, -8.0)),
        ));
        app
    }

    fn scene(app: &mut App, hash: &str, tick: u32) -> Entity {
        let root = app.world_mut().spawn_empty().id();
        let mut context = RendererSceneContext::new(
            SceneId(root),
            hash.to_owned(),
            String::new(),
            false,
            0,
            "title".to_owned(),
            IVec2::ZERO,
            [IVec2::ZERO].into_iter().collect(),
            Vec::new(),
            SceneBounds::default(),
            Vec::new(),
            root,
            0.0,
            false,
            "7",
            false,
            false,
        );
        context.tick_number = tick;
        app.world_mut().entity_mut(root).insert(context);
        app.world_mut()
            .resource_mut::<LiveScenes>()
            .scenes
            .insert(hash.to_owned(), root);
        root
    }

    /// One frame of `seconds`; the reply, once there is one.
    fn step(
        app: &mut App,
        rx: &mut tokio::sync::oneshot::Receiver<Result<String, String>>,
        seconds: f32,
    ) -> Option<Value> {
        app.world_mut()
            .resource_mut::<Time<Real>>()
            .advance_by(Duration::from_secs_f32(seconds));
        app.world_mut().resource_mut::<FrameCount>().0 += 1;
        app.update();
        rx.try_recv()
            .ok()
            .map(|r| serde_json::from_str(&r.unwrap()).unwrap())
    }

    #[test]
    fn a_scene_is_ready_after_its_second_update() {
        let mut app = app();
        for (tick, state) in [(0, "starting"), (3, "starting"), (READY_TICK, "running")] {
            let root = scene(&mut app, "h", tick);
            let context = app.world().get::<RendererSceneContext>(root);
            assert_eq!(state_name(None, context), state);
        }
        let root = scene(&mut app, "h", 10);
        let mut context = app
            .world_mut()
            .get_mut::<RendererSceneContext>(root)
            .unwrap();
        context.blocked.insert("frozen");
        assert_eq!(state_name(None, Some(&*context)), "blocked");
        assert_eq!(state_name(Some(&SceneLoading::Failed), None), "failed");
        assert_eq!(
            state_name(Some(&SceneLoading::SceneEntity), None),
            "loading"
        );
    }

    #[test]
    fn a_reload_settles_on_the_new_scene_after_a_presented_frame() {
        let mut app = app();
        let old = scene(&mut app, "h", 40);
        let removed = app
            .world_mut()
            .resource_mut::<LiveScenes>()
            .scenes
            .remove("h");
        assert_eq!(removed, Some(old));
        let mut rx = app
            .world_mut()
            .resource_mut::<SceneWaits>()
            .reload("h".to_owned(), old, 5.0);
        assert!(step(&mut app, &mut rx, 0.1).is_none());

        let new = scene(&mut app, "h", 2);
        assert!(step(&mut app, &mut rx, 0.1).is_none());
        app.world_mut()
            .get_mut::<RendererSceneContext>(new)
            .unwrap()
            .tick_number = READY_TICK;
        // ready, then the frame drawn with it is presented two frames later
        assert!(step(&mut app, &mut rx, 0.1).is_none());
        assert!(step(&mut app, &mut rx, 0.1).is_none());
        let reply = step(&mut app, &mut rx, 0.1).unwrap();
        assert_eq!(reply["reloaded"], true);
        assert_eq!(reply["settled"], true);
        assert_eq!(reply["sceneState"], "running");
        assert_eq!(reply["ticksWaited"], READY_TICK);
        assert_eq!(reply["renderedFrames"], 1);
        assert!(reply["hint"].is_string());
        assert!(reply.get("warning").is_none());
    }

    #[test]
    fn a_frozen_scene_ends_the_wait_at_once() {
        let mut app = app();
        let old = scene(&mut app, "h", 40);
        app.world_mut()
            .resource_mut::<LiveScenes>()
            .scenes
            .remove("h");
        let mut rx = app
            .world_mut()
            .resource_mut::<SceneWaits>()
            .reload("h".to_owned(), old, 60.0);
        let new = scene(&mut app, "h", 3);
        app.world_mut()
            .get_mut::<RendererSceneContext>(new)
            .unwrap()
            .blocked
            .insert(FROZEN_BLOCK);
        let reply = step(&mut app, &mut rx, 0.1).unwrap();
        assert_eq!(reply["settled"], false);
        assert_eq!(reply["sceneState"], "blocked");
        assert_eq!(reply["blocked"], json!(["frozen"]));
        assert!(reply["warning"].as_str().unwrap().contains("frozen"));
    }

    #[test]
    fn a_reload_that_never_runs_times_out_with_a_warning() {
        let mut app = app();
        let old = scene(&mut app, "h", 40);
        app.world_mut()
            .resource_mut::<LiveScenes>()
            .scenes
            .remove("h");
        let mut rx = app
            .world_mut()
            .resource_mut::<SceneWaits>()
            .reload("h".to_owned(), old, 1.0);
        assert!(step(&mut app, &mut rx, 0.6).is_none());
        let reply = step(&mut app, &mut rx, 0.6).unwrap();
        assert_eq!(reply["settled"], false);
        assert_eq!(reply["renderedFrames"], 0);
        assert!(reply["warning"]
            .as_str()
            .unwrap()
            .contains("Waiting for the scene to spawn again."));
    }

    #[test]
    fn an_arrival_waits_for_the_player_to_land() {
        let mut app = app();
        let mut pointers = ScenePointers::default();
        pointers.set_realm(IVec2::ZERO, IVec2::ONE);
        pointers.insert(IVec2::ZERO, PointerResult::Nothing);
        app.insert_resource(pointers);
        let player = app
            .world_mut()
            .query_filtered::<Entity, With<PrimaryUser>>()
            .single(app.world())
            .unwrap();
        app.world_mut().entity_mut(player).insert(OutOfWorld);
        let mut rx =
            app.world_mut()
                .resource_mut::<SceneWaits>()
                .arrival(IVec2::ZERO, 5.0, None, json!({}));
        assert!(step(&mut app, &mut rx, 0.1).is_none());

        app.world_mut().entity_mut(player).remove::<OutOfWorld>();
        app.world_mut().send_event(ReturnedToWorld::SceneReady);
        let reply = step(&mut app, &mut rx, 0.1).unwrap();
        assert_eq!(reply["sceneReady"], false);
        assert_eq!(reply["sceneState"], "none");
        assert!(reply["warning"].as_str().unwrap().contains("no scene"));
    }
}

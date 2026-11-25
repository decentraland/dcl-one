//! Waiting for a parcel scene to settle, for the commands that replace one (`/teleport --wait`,
//! `/reload --wait`). The readiness they wait for is the scene's [`Stage`], which
//! `/scene_loading_state` reports.

use bevy::{diagnostic::FrameCount, prelude::*};
use common::{
    rpc::RpcResultReceiver,
    structs::{OutOfWorld, PrimaryUser, ReturnedToWorld},
};
use serde_json::{json, Value};

use crate::{
    initialize_scene::{NoParcelScene, SceneLoading, SuperUserScene},
    renderer_context::RendererSceneContext,
    stage::{state_of, Stage},
    ContainingScene,
};

/// Frames waited once the scene is ready, so the frame it first drew in has been presented: the
/// render world draws a frame while the main world runs the next one.
const PRESENT_FRAMES: u32 = 2;
const DEFAULT_TIMEOUT_SECONDS: f32 = 60.0;
const MAX_TIMEOUT_SECONDS: f32 = 300.0;

/// The `--wait [--timeout-seconds S]` pair of a settling command, checked: how long to wait, or
/// `None` for a command that does not wait.
pub fn wait_timeout(wait: bool, timeout_seconds: Option<f32>) -> Result<Option<f32>, String> {
    if !wait {
        return Ok(None);
    }
    let timeout = timeout_seconds.unwrap_or(DEFAULT_TIMEOUT_SECONDS);
    if !timeout.is_finite() || timeout <= 0.0 || timeout > MAX_TIMEOUT_SECONDS {
        return Err(format!(
            "--timeout-seconds takes more than 0 and at most {MAX_TIMEOUT_SECONDS}; got {timeout}"
        ));
    }
    Ok(Some(timeout))
}

enum Target {
    /// The parcel scene the player lands in after a teleport. `landed` once a
    /// [`ReturnedToWorld`] was seen since the teleport.
    Arrival { parcel: IVec2, landed: bool },
    /// A new instance of a reloaded scene: any entity for the hash but the one replaced.
    Reload { hash: String, replaced: Entity },
}

/// The scene a wait watches, or why it has none yet.
enum Watched {
    Scene(Entity),
    /// The teleport landed on a parcel that will hold no scene for the player: an empty one, or
    /// one whose scene refuses them.
    NoScene(IVec2, NoParcelScene),
    Waiting(&'static str),
}

/// What a wait observed, for its reply.
struct Outcome {
    /// The player landed (a teleport's reply only).
    arrived: bool,
    /// The scene is ready (a teleport's reply only).
    ready: bool,
    state: &'static str,
    /// Frames drawn since the scene was first seen ready.
    rendered_frames: u32,
    settled: bool,
}

/// What one frame gives every wait.
struct Frame {
    count: u32,
    dt: f32,
    /// A [`ReturnedToWorld`] arrived this frame.
    landed: bool,
    out_of_world: bool,
}

/// The parcel lookup of the frame: the scenes placed, and the ones that refuse the player.
struct Parcels<'a, 'w, 's> {
    containing: &'a ContainingScene<'w, 's>,
    bans: &'a comms::SceneBanGate<'w>,
}

type Scenes<'w, 's> = Query<
    'w,
    's,
    (
        Option<&'static SceneLoading>,
        Option<&'static RendererSceneContext>,
    ),
    Without<SuperUserScene>,
>;

struct SceneWait {
    target: Target,
    /// The teleport request an arrival waits on first, until it is answered.
    request: Option<RpcResultReceiver<Result<(), String>>>,
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

    /// Wait for the teleport `request` to be answered, the player to land on `parcel` and its
    /// scene to be ready. The reply is `reply` with the outcome added.
    pub fn arrival(
        &mut self,
        parcel: IVec2,
        timeout: f32,
        request: RpcResultReceiver<Result<(), String>>,
        reply: Value,
    ) -> tokio::sync::oneshot::Receiver<Result<String, String>> {
        self.push(
            Target::Arrival {
                parcel,
                landed: false,
            },
            Some(request),
            reply,
            timeout,
        )
    }

    /// Wait for a new instance of the scene `hash` to be ready; `replaced` is the scene entity the
    /// reload removed from `LiveScenes`.
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
        request: Option<RpcResultReceiver<Result<(), String>>>,
        reply: Value,
        timeout: f32,
    ) -> tokio::sync::oneshot::Receiver<Result<String, String>> {
        let (sender, receiver) = tokio::sync::oneshot::channel();
        self.0.push(SceneWait {
            target,
            request,
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

pub(super) fn settle_scenes(
    mut waits: ResMut<SceneWaits>,
    mut returned: EventReader<ReturnedToWorld>,
    time: Res<Time<Real>>,
    frame: Res<FrameCount>,
    containing: ContainingScene,
    bans: comms::SceneBanGate,
    player: Query<Has<OutOfWorld>, With<PrimaryUser>>,
    scenes: Scenes,
) {
    let landed = returned.read().last().is_some();
    if waits.0.is_empty() {
        return;
    }
    let frame = Frame {
        count: frame.0,
        dt: time.delta_secs(),
        landed,
        out_of_world: player.single().unwrap_or(true),
    };
    let parcels = Parcels {
        containing: &containing,
        bans: &bans,
    };
    waits
        .0
        .retain_mut(|wait| wait.advance(&frame, &parcels, &scenes));
}

impl SceneWait {
    /// One frame of the wait; `false` once it has replied, so `retain_mut` drops it.
    fn advance(&mut self, frame: &Frame, parcels: &Parcels, scenes: &Scenes) -> bool {
        if self.sender.as_ref().is_none_or(|s| s.is_closed()) {
            return false;
        }
        self.elapsed += frame.dt;
        if let Some(keep) = self.poll_request() {
            return keep;
        }

        let entity = match self.watch(frame, parcels) {
            Watched::Scene(entity) => Ok(entity),
            Watched::Waiting(why) => Err(why),
            Watched::NoScene(parcel, none) => {
                self.reply["sceneHash"] = Value::Null;
                let outcome = Outcome {
                    arrived: true,
                    ready: false,
                    state: state_of(Err(none)),
                    rendered_frames: 0,
                    settled: true,
                };
                let (x, y) = (parcel.x, parcel.y);
                let warning = match none {
                    NoParcelScene::Refused => {
                        format!("the scene on parcel {x},{y} refuses this player; it is not loaded")
                    }
                    _ => format!("there is no scene on parcel {x},{y}"),
                };
                return self.finish(outcome, Some(warning));
            }
        };

        let (loading, context) = entity
            .ok()
            .and_then(|e| scenes.get(e).ok())
            .unwrap_or((None, None));
        if let Ok(entity) = entity {
            if self.watched.is_none_or(|(watched, _)| watched != entity) {
                let tick = match self.target {
                    // a reloaded scene is a new one: every tick it has, it ran since
                    Target::Reload { .. } => 0,
                    Target::Arrival { .. } => context.map_or(0, |c| c.tick_number),
                };
                self.watched = Some((entity, tick));
                self.ready = None;
            }
        }
        // no scene to watch reads as one that has not started
        let stage = Stage::of(loading, context);
        let ready = stage.is_ready();
        if ready && self.ready.is_none() {
            self.ready = Some(frame.count);
        } else if !ready {
            self.ready = None;
        }

        let rendered_frames = self.ready.map_or(0, |at| frame.count.wrapping_sub(at));
        let presented = rendered_frames >= PRESENT_FRAMES;
        let halted = stage.halted(context);
        let expired = self.elapsed >= self.timeout;
        if !(presented || halted.is_some() || expired) {
            return true;
        }

        if let Some(context) = context {
            self.reply["sceneHash"] = json!(context.hash.as_ref());
            self.reply["title"] = json!(context.title);
            let since = self.watched.map_or(0, |(_, tick)| tick);
            self.reply["ticksWaited"] = json!(context.tick_number.saturating_sub(since));
            if !context.blocked.is_empty() {
                let mut blocked: Vec<_> = context.blocked.iter().collect();
                blocked.sort();
                self.reply["blocked"] = json!(blocked);
            }
        }
        let warning = if presented {
            None
        } else if let Some(halted) = halted {
            Some(format!("the scene is {halted}; it will not become ready"))
        } else {
            let waiting = match entity {
                Err(why) => why.to_owned(),
                Ok(_) if ready => "Waiting for a frame to be drawn.".to_owned(),
                Ok(_) => format!("Waiting for the scene to run (it is {}).", stage.state()),
            };
            Some(format!(
                "timed out after {} s. {waiting} Is the simulation paused?",
                self.timeout
            ))
        };
        let outcome = Outcome {
            arrived: matches!(self.target, Target::Arrival { landed: true, .. })
                && !frame.out_of_world,
            ready,
            state: stage.state(),
            rendered_frames,
            settled: presented,
        };
        self.finish(outcome, warning)
    }

    /// Poll the teleport request an arrival waits on first: `Some` while the wait stops there
    /// this frame, with what [`Self::advance`] returns.
    fn poll_request(&mut self) -> Option<bool> {
        match self.request.as_mut()?.poll_once() {
            Ok(None) if self.elapsed < self.timeout => Some(true),
            // the teleport request (a realm change, when it asks for one) counts against the
            // timeout too
            Ok(None) => {
                let outcome = Outcome {
                    arrived: false,
                    ready: false,
                    state: Stage::Unstarted.state(),
                    rendered_frames: 0,
                    settled: false,
                };
                let warning = format!(
                    "timed out after {} s. Waiting for the teleport request to be answered.",
                    self.timeout
                );
                Some(self.finish(outcome, Some(warning)))
            }
            Ok(Some(Ok(()))) => {
                self.request = None;
                None
            }
            Ok(Some(Err(e))) => Some(self.send(Err(e))),
            Err(()) => Some(self.send(Err("the teleport was cancelled".to_owned()))),
        }
    }

    /// The scene entity to watch this frame, or why there is none yet.
    fn watch(&mut self, frame: &Frame, parcels: &Parcels) -> Watched {
        match &mut self.target {
            Target::Arrival { parcel, landed } => {
                *landed |= frame.landed;
                if !*landed || frame.out_of_world {
                    return Watched::Waiting("Waiting for the player to land.");
                }
                // the lookup the teleport released the player on: neither waits for a scene on an
                // empty parcel or for one that refuses the player
                match parcels.containing.admitted_scene(*parcel, parcels.bans) {
                    Ok(entity) => Watched::Scene(entity),
                    Err(none @ (NoParcelScene::Empty | NoParcelScene::Refused)) => {
                        Watched::NoScene(*parcel, none)
                    }
                    Err(none) => Watched::Waiting(none.reason()),
                }
            }
            Target::Reload { hash, replaced } => {
                match parcels.containing.live_scenes.scenes.get(hash.as_str()) {
                    Some(entity) if entity != replaced => Watched::Scene(*entity),
                    _ => Watched::Waiting("Waiting for the scene to spawn again."),
                }
            }
        }
    }

    /// Fill the outcome into the reply and send it; `false`, as [`Self::send`].
    fn finish(&mut self, outcome: Outcome, warning: Option<String>) -> bool {
        let mut reply = std::mem::take(&mut self.reply);
        match self.target {
            Target::Arrival { .. } => {
                reply["arrived"] = json!(outcome.arrived);
                reply["sceneReady"] = json!(outcome.ready);
            }
            Target::Reload { .. } if outcome.settled => {
                reply["hint"] = json!(
                    "the scene started again from scratch: its state, counters and scene-UI ids \
                     are new; its startup logs are in /scene_logs again"
                );
            }
            Target::Reload { .. } => {}
        }
        reply["sceneState"] = json!(outcome.state);
        reply["renderedFrames"] = json!(outcome.rendered_frames);
        reply["settleMs"] = json!((self.elapsed * 1000.0).round());
        reply["settled"] = json!(outcome.settled);
        if let Some(warning) = warning {
            reply["warning"] = json!(warning);
        }
        self.send(Ok(reply.to_string()))
    }

    /// Send the reply; `false`, so `retain_mut` drops the wait.
    fn send(&mut self, result: Result<String, String>) -> bool {
        if let Some(sender) = self.sender.take() {
            let _ = sender.send(result);
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use common::rpc::RpcResultSender;
    use dcl_types::SceneId;

    use crate::{
        initialize_scene::{LiveScenes, PointerResult, PortableScenes, ScenePointers},
        renderer_context::{FROZEN_BLOCK, STARTUP_TICKS},
        update_world::gltf_container::GLTF_LOADING,
    };
    use scene_material::SceneBounds;

    use super::*;

    fn app() -> App {
        let mut app = App::new();
        app.init_resource::<SceneWaits>()
            .init_resource::<LiveScenes>()
            .init_resource::<PortableScenes>()
            .init_resource::<ScenePointers>()
            // what the ban gate reads
            .init_resource::<comms::SceneAdmission>()
            .init_resource::<common::structs::CurrentRealm>()
            .init_resource::<common::structs::PreviewMode>()
            .init_resource::<wallet::Wallet>()
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
    fn a_scene_is_ready_once_it_has_started() {
        let mut app = app();
        for (tick, state) in [
            (0, "starting"),
            (STARTUP_TICKS, "starting"),
            (STARTUP_TICKS + 1, "running"),
        ] {
            let root = scene(&mut app, "h", tick);
            let context = app.world().get::<RendererSceneContext>(root).unwrap();
            let stage = Stage::of(None, Some(context));
            assert_eq!(stage.state(), state);
            // the rule the scene's stand-ins go by, and the one the teleport releases on
            assert_eq!(stage.is_ready(), context.content_shown());
            assert_eq!(
                stage.is_ready(),
                crate::load_timing::scene_has_loaded(context)
            );
            assert_eq!(stage.is_ready(), state == "running");
        }
        let root = scene(&mut app, "h", 10);
        let mut context = app
            .world_mut()
            .get_mut::<RendererSceneContext>(root)
            .unwrap();
        context.blocked.insert(FROZEN_BLOCK);
        let stage = Stage::of(None, Some(&*context));
        assert_eq!(stage.state(), "blocked");
        // a frozen scene shows what it has, and is not ready
        assert!(context.content_shown());
        assert!(!stage.is_ready());
        // it will not be, once the glTFs it still loads have
        assert_eq!(stage.halted(Some(&*context)), Some("frozen"));
        context.blocked.insert(GLTF_LOADING);
        assert_eq!(
            Stage::of(None, Some(&*context)).halted(Some(&*context)),
            None
        );
        assert_eq!(Stage::Failed.halted(None), Some("failed"));
        assert_eq!(Stage::Broken.halted(None), Some("broken"));
        assert_eq!(Stage::Starting.halted(None), None);
        assert_eq!(
            Stage::of(Some(&SceneLoading::Failed), None).state(),
            "failed"
        );
        assert_eq!(
            Stage::of(Some(&SceneLoading::SceneEntity), None).state(),
            "loading"
        );
        assert_eq!(state_of(Err(NoParcelScene::Empty)), "none");
        assert_eq!(state_of(Err(NoParcelScene::Refused)), "refused");
        assert_eq!(state_of(Err(NoParcelScene::NotSpawned)), "loading");
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
            .tick_number = STARTUP_TICKS + 1;
        // ready, then the frame drawn with it is presented two frames later
        assert!(step(&mut app, &mut rx, 0.1).is_none());
        assert!(step(&mut app, &mut rx, 0.1).is_none());
        let reply = step(&mut app, &mut rx, 0.1).unwrap();
        assert_eq!(reply["reloaded"], true);
        assert_eq!(reply["settled"], true);
        assert_eq!(reply["sceneState"], "running");
        assert_eq!(reply["ticksWaited"], STARTUP_TICKS + 1);
        // the frame the scene was first ready in, presented by now, and the one after it
        assert_eq!(reply["renderedFrames"], PRESENT_FRAMES);
        assert!(reply["hint"].is_string());
        assert!(reply.get("warning").is_none());
    }

    #[test]
    fn a_frozen_scene_ends_the_wait_once_its_gltfs_have_loaded() {
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
        // the editor freezes a scene at tick 3, while its models still load
        let new = scene(&mut app, "h", 3);
        let mut context = app
            .world_mut()
            .get_mut::<RendererSceneContext>(new)
            .unwrap();
        context.blocked.insert(FROZEN_BLOCK);
        context.blocked.insert(GLTF_LOADING);
        // the teleport's release holds the player for those too
        assert!(!crate::load_timing::scene_has_loaded(&context));
        assert!(step(&mut app, &mut rx, 0.1).is_none());
        assert!(step(&mut app, &mut rx, 0.1).is_none());

        let mut context = app
            .world_mut()
            .get_mut::<RendererSceneContext>(new)
            .unwrap();
        context.blocked.remove(GLTF_LOADING);
        assert!(crate::load_timing::scene_has_loaded(&context));
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
    fn a_teleport_request_that_is_never_answered_times_out() {
        let mut app = app();
        let (_response, request) = RpcResultSender::<Result<(), String>>::channel();
        let mut rx = app.world_mut().resource_mut::<SceneWaits>().arrival(
            IVec2::ZERO,
            1.0,
            request,
            json!({ "realm": "a.dcl.eth" }),
        );
        assert!(step(&mut app, &mut rx, 0.6).is_none());
        let reply = step(&mut app, &mut rx, 0.6).unwrap();
        assert_eq!(reply["realm"], "a.dcl.eth");
        assert_eq!(reply["arrived"], false);
        assert_eq!(reply["sceneReady"], false);
        assert_eq!(reply["sceneState"], "loading");
        assert_eq!(reply["renderedFrames"], 0);
        assert_eq!(reply["settled"], false);
        assert!(reply["warning"]
            .as_str()
            .unwrap()
            .contains("Waiting for the teleport request to be answered."));
        assert_eq!(app.world().resource::<SceneWaits>().pending(), 0);
    }

    #[test]
    fn a_refused_teleport_request_fails_the_wait() {
        let mut app = app();
        let (response, request) = RpcResultSender::<Result<(), String>>::channel();
        let mut rx = app.world_mut().resource_mut::<SceneWaits>().arrival(
            IVec2::ZERO,
            5.0,
            request,
            json!({}),
        );
        response.send(Err("no such spawn point".to_owned()));
        app.update();
        assert_eq!(
            rx.try_recv().unwrap(),
            Err("no such spawn point".to_owned())
        );
        assert_eq!(app.world().resource::<SceneWaits>().pending(), 0);
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
        app.world_mut()
            .entity_mut(player)
            .insert(OutOfWorld::default());
        let (response, request) = RpcResultSender::<Result<(), String>>::channel();
        response.send(Ok(()));
        let mut rx = app.world_mut().resource_mut::<SceneWaits>().arrival(
            IVec2::ZERO,
            5.0,
            request,
            json!({}),
        );
        assert!(step(&mut app, &mut rx, 0.1).is_none());

        app.world_mut().entity_mut(player).remove::<OutOfWorld>();
        app.world_mut().send_event(ReturnedToWorld::SceneReady);
        let reply = step(&mut app, &mut rx, 0.1).unwrap();
        assert_eq!(reply["arrived"], true);
        assert_eq!(reply["sceneReady"], false);
        assert_eq!(reply["sceneState"], "none");
        assert_eq!(reply["settled"], true);
        assert!(reply["sceneHash"].is_null());
        assert!(reply["warning"].as_str().unwrap().contains("no scene"));
    }

    /// Parcel (0, 0) holds the scene "h".
    fn place_scene(app: &mut App) {
        let mut pointers = ScenePointers::default();
        pointers.set_realm(IVec2::ZERO, IVec2::ONE);
        pointers.insert(
            IVec2::ZERO,
            PointerResult::Exists {
                hash: "h".to_owned(),
                urn: None,
            },
        );
        app.insert_resource(pointers);
    }

    #[test]
    fn an_arrival_on_a_parcel_whose_scene_refuses_the_player_settles_on_landing() {
        let mut app = app();
        place_scene(&mut app);
        // a refused scene is not loaded for the player: it has no live root to wait for
        crate::stage::refuse_player(app.world_mut(), "h");
        let (response, request) = RpcResultSender::<Result<(), String>>::channel();
        response.send(Ok(()));
        let mut rx = app.world_mut().resource_mut::<SceneWaits>().arrival(
            IVec2::ZERO,
            5.0,
            request,
            json!({}),
        );
        assert!(step(&mut app, &mut rx, 0.1).is_none());

        // the teleport's release lands the player there without a scene
        app.world_mut().send_event(ReturnedToWorld::SceneReady);
        let reply = step(&mut app, &mut rx, 0.1).unwrap();
        assert_eq!(reply["arrived"], true);
        assert_eq!(reply["sceneReady"], false);
        assert_eq!(reply["sceneState"], "refused");
        assert_eq!(reply["renderedFrames"], 0);
        assert_eq!(reply["settled"], true);
        assert!(reply["sceneHash"].is_null());
        assert!(reply["warning"]
            .as_str()
            .unwrap()
            .contains("the scene on parcel 0,0 refuses this player"));
        assert_eq!(app.world().resource::<SceneWaits>().pending(), 0);
    }

    #[test]
    fn a_scene_that_refuses_the_player_while_it_loads_ends_the_wait() {
        let mut app = app();
        place_scene(&mut app);
        let (response, request) = RpcResultSender::<Result<(), String>>::channel();
        response.send(Ok(()));
        let mut rx = app.world_mut().resource_mut::<SceneWaits>().arrival(
            IVec2::ZERO,
            60.0,
            request,
            json!({}),
        );
        app.world_mut().send_event(ReturnedToWorld::SceneReady);
        scene(&mut app, "h", 2);
        assert!(step(&mut app, &mut rx, 0.1).is_none());

        // the room refuses the player after the landing, and the scene is unloaded
        crate::stage::refuse_player(app.world_mut(), "h");
        app.world_mut()
            .resource_mut::<LiveScenes>()
            .scenes
            .remove("h");
        let reply = step(&mut app, &mut rx, 0.1).unwrap();
        assert_eq!(reply["arrived"], true);
        assert_eq!(reply["sceneState"], "refused");
        assert_eq!(reply["settled"], true);
        assert!(reply["warning"].as_str().unwrap().contains("refuses"));
    }

    #[test]
    fn an_arrival_settles_on_the_scene_the_player_landed_in() {
        let mut app = app();
        let mut pointers = ScenePointers::default();
        pointers.set_realm(IVec2::ZERO, IVec2::ONE);
        pointers.insert(
            IVec2::ZERO,
            PointerResult::Exists {
                hash: "h".to_owned(),
                urn: None,
            },
        );
        app.insert_resource(pointers);
        let (response, request) = RpcResultSender::<Result<(), String>>::channel();
        response.send(Ok(()));
        let mut rx = app.world_mut().resource_mut::<SceneWaits>().arrival(
            IVec2::ZERO,
            5.0,
            request,
            json!({}),
        );
        app.world_mut().send_event(ReturnedToWorld::SceneReady);
        // landed, and the parcel's scene has not spawned
        assert!(step(&mut app, &mut rx, 0.1).is_none());

        let root = scene(&mut app, "h", 2);
        assert!(step(&mut app, &mut rx, 0.1).is_none());
        app.world_mut()
            .get_mut::<RendererSceneContext>(root)
            .unwrap()
            .tick_number = STARTUP_TICKS + 3;
        assert!(step(&mut app, &mut rx, 0.1).is_none());
        assert!(step(&mut app, &mut rx, 0.1).is_none());
        let reply = step(&mut app, &mut rx, 0.1).unwrap();
        assert_eq!(reply["arrived"], true);
        assert_eq!(reply["sceneReady"], true);
        assert_eq!(reply["sceneHash"], "h");
        assert_eq!(reply["sceneState"], "running");
        // the ticks the scene ran while the wait watched it
        assert_eq!(reply["ticksWaited"], STARTUP_TICKS + 1);
        assert_eq!(reply["settled"], true);
        assert!(reply.get("warning").is_none());
    }
}

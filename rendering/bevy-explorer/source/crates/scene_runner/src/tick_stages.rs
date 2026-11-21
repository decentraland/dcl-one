//! MEASUREMENT ONLY (local measurement build, not for merge): the scene loop's stages each frame
//! and the movement scene's tick from dispatch to reply, on the clock the web workers share
//! (`performance.timeOrigin + performance.now()`, ms; on native, ms since the Unix epoch, with
//! no relay marks). The movement trace (`/movement_trace`, user_input) writes them into each
//! frame's `extra.stage`. Nothing here changes scheduling: it only reads the clock and counts.

use bevy::prelude::*;

/// The shared clock, ms.
pub fn now_ms() -> f64 {
    #[cfg(target_arch = "wasm32")]
    {
        dcl_wasm::tick_marks::engine_now_ms()
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(f64::NAN, |d| d.as_secs_f64() * 1000.0)
    }
}

/// `now_ms` at an `Instant` already taken (the scene loop's own times).
pub fn at_ms(instant: web_time::Instant) -> f64 {
    let now = web_time::Instant::now();
    if now >= instant {
        now_ms() - (now - instant).as_secs_f64() * 1000.0
    } else {
        now_ms() + (instant - now).as_secs_f64() * 1000.0
    }
}

/// A movement scene dispatch.
#[derive(Clone, Copy, Debug)]
pub struct SentMark {
    /// The dispatch's number (the movement scene's dispatches since startup)
    pub seq: u64,
    pub frame: u32,
    pub at: f64,
    /// The dispatch frame's scene loop start (S) and deadline (D)
    pub loop_start: f64,
    pub deadline: f64,
}

/// A movement scene reply, as the engine took it off the scene channel.
#[derive(Clone, Copy, Debug)]
pub struct ReplyMark {
    pub sent: SentMark,
    pub frame: u32,
    pub at: f64,
    /// Taken by the frame's first `receive_scene_updates` call: it was waiting when the loop began
    pub first_poll: bool,
    /// The relay's and the scene worker's marks (`dcl_wasm::tick_marks::RELAY_NAMES`), when the
    /// relay published them for this dispatch
    pub relay: Option<[f64; RELAY_FIELDS]>,
}

#[cfg(target_arch = "wasm32")]
pub use dcl_wasm::tick_marks::{RELAY_FIELDS, RELAY_NAMES};
#[cfg(not(target_arch = "wasm32"))]
pub const RELAY_FIELDS: usize = 12;
#[cfg(not(target_arch = "wasm32"))]
pub const RELAY_NAMES: [&str; RELAY_FIELDS] = [
    "pick_pump",
    "pick_slept",
    "pick",
    "b_prev_end",
    "b_start",
    "b_put",
    "b_n",
    "fwd_pump",
    "fwd_slept",
    "fwd",
    "pumps",
    "held",
];

/// Why the scene loop ended.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum LoopEnd {
    #[default]
    None,
    /// Its deadline passed
    Deadline,
    /// No scene was eligible and none in flight
    Idle,
}

impl LoopEnd {
    pub fn name(&self) -> &'static str {
        match self {
            LoopEnd::None => "none",
            LoopEnd::Deadline => "deadline",
            LoopEnd::Idle => "idle",
        }
    }
}

/// This frame's stages; reset at the frame's start.
#[derive(Clone, Copy, Debug, Default)]
pub struct FrameStages {
    /// The frame's start (`First`)
    pub start: f64,
    /// Scene loop start (S), deadline (D) and end
    pub loop_start: f64,
    pub deadline: f64,
    pub loop_end: f64,
    /// S since the previous frame's S (I)
    pub interval: f64,
    pub end: LoopEnd,
    pub eligible_left: u32,
    pub in_flight_at_end: u32,
    /// Scenes dispatched in the loop
    pub dispatched: u32,
    pub schedule_runs: u32,
    /// `receive_scene_updates` calls in the loop
    pub polls: u32,
    /// The previous frame's end (`Last`), for its Q
    pub prev_end: f64,
    pub heap_js_mb: Option<f64>,
    pub wasm_mb: Option<f64>,
    /// The scene loop mode (`dcl_wasm::scene_loop_mode`; `native` natively)
    pub mode: &'static str,
    /// What the deadline mode leaves for the frame's tail (K̂ = p90 + 1 ms), ms; null until
    /// measured, and natively
    pub tail_reserve: Option<f64>,
    /// Every relay's steps since startup that waited for the engine's send and that slept
    /// (web; the relays of the scenes the engine wakes wait, the others sleep), at the frame's start
    pub relay_waits: Option<u64>,
    pub relay_sleeps: Option<u64>,
    /// The previous frame's pipelined-rendering timings (web), ms: its `pipelined/wait_render`,
    /// `pipelined/wait_handoff` and `pipelined/main_update`, and when its handoff ended (the end of
    /// its app update) on the shared clock
    pub prev_wait_render: Option<f64>,
    pub prev_wait_handoff: Option<f64>,
    pub prev_main_update: Option<f64>,
    pub prev_handoff_end: Option<f64>,
    /// The movement scene's dispatch and reply this frame
    pub sent: Option<SentMark>,
    pub reply: Option<ReplyMark>,
}

#[derive(Resource, Default)]
pub struct TickStages {
    pub frame: FrameStages,
    /// The movement scene's last dispatch
    pub last_sent: Option<SentMark>,
    seq: u64,
    prev_loop_start: f64,
    last_end: f64,
}

impl TickStages {
    /// The scene loop starts: S and D, and the movement scene the relay should mark.
    pub fn loop_started(
        &mut self,
        start: web_time::Instant,
        deadline: web_time::Instant,
        movement: Option<Entity>,
        mode: &'static str,
        tail_reserve: Option<std::time::Duration>,
    ) {
        let s = at_ms(start);
        let f = &mut self.frame;
        f.mode = mode;
        f.tail_reserve = tail_reserve.map(|d| d.as_secs_f64() * 1000.0);
        f.loop_start = s;
        f.deadline = at_ms(deadline);
        f.interval = s - self.prev_loop_start;
        self.prev_loop_start = s;
        #[cfg(target_arch = "wasm32")]
        dcl_wasm::tick_marks::set_movement_scene(movement.map(Entity::to_bits));
        #[cfg(not(target_arch = "wasm32"))]
        let _ = movement;
    }

    pub fn loop_ended(
        &mut self,
        end: web_time::Instant,
        deadline_passed: bool,
        eligible_left: u32,
        in_flight: u32,
        schedule_runs: u32,
    ) {
        let f = &mut self.frame;
        f.loop_end = at_ms(end);
        f.end = if deadline_passed {
            LoopEnd::Deadline
        } else {
            LoopEnd::Idle
        };
        f.eligible_left = eligible_left;
        f.in_flight_at_end = in_flight;
        f.schedule_runs = schedule_runs;
    }

    /// A scene is about to be dispatched; for the movement scene, numbers the dispatch for the
    /// relay first.
    pub fn dispatching(&mut self, movement: bool, frame: u32) {
        self.frame.dispatched += 1;
        if !movement {
            return;
        }
        self.seq += 1;
        #[cfg(target_arch = "wasm32")]
        dcl_wasm::tick_marks::set_dispatch_seq(self.seq);
        let sent = SentMark {
            seq: self.seq,
            frame,
            at: now_ms(),
            loop_start: self.frame.loop_start,
            deadline: self.frame.deadline,
        };
        self.frame.sent = Some(sent);
        self.last_sent = Some(sent);
    }

    /// A `receive_scene_updates` call; whether it is the loop's first.
    pub fn poll(&mut self) -> bool {
        self.frame.polls += 1;
        self.frame.polls == 1
    }

    /// The movement scene's reply came off the channel.
    pub fn movement_reply(&mut self, frame: u32, first_poll: bool) {
        let at = now_ms();
        let Some(sent) = self.last_sent.take() else {
            return;
        };
        #[cfg(target_arch = "wasm32")]
        let relay = dcl_wasm::tick_marks::read(sent.seq);
        #[cfg(not(target_arch = "wasm32"))]
        let relay = None;
        self.frame.reply = Some(ReplyMark {
            sent,
            frame,
            at,
            first_poll,
            relay,
        });
    }
}

pub struct TickStagesPlugin;

impl Plugin for TickStagesPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<TickStages>()
            .add_systems(First, frame_start.before(bevy::time::TimeSystem))
            .add_systems(Last, frame_end);
        #[cfg(target_arch = "wasm32")]
        app.add_systems(Update, log_wake_scenes.after(crate::update_scene_priority));
    }
}

/// The scenes whose relays wake on the engine's send (the set `update_scene_priority` published
/// this frame), logged as `relay wake scenes: <id> <title>, ...` when it changes and every 1800
/// frames.
#[cfg(target_arch = "wasm32")]
fn log_wake_scenes(
    player: Query<Entity, With<common::structs::PrimaryUser>>,
    containing_scene: crate::ContainingScene,
    updates: Res<crate::SceneUpdates>,
    contexts: Query<&crate::RendererSceneContext>,
    mut last: Local<(Vec<(Entity, String)>, u32)>,
) {
    let mut scenes: Vec<(Entity, String)> = crate::wake_scenes(
        player.single().ok(),
        &containing_scene,
        &updates.priority_scenes,
    )
    .into_iter()
    .map(|scene| {
        let title = contexts
            .get(scene)
            .map_or_else(|_| "(loading)".to_owned(), |ctx| ctx.title.clone());
        (scene, title)
    })
    .collect();
    scenes.sort();
    last.1 += 1;
    if scenes != last.0 || last.1 >= 1800 {
        let list = scenes
            .iter()
            .map(|(scene, title)| format!("{} {title}", scene.to_bits()))
            .collect::<Vec<_>>()
            .join(", ");
        info!("relay wake scenes: {list}");
        *last = (scenes, 0);
    }
}

fn frame_start(
    mut stages: ResMut<TickStages>,
    store: Option<Res<bevy::diagnostic::DiagnosticsStore>>,
) {
    let prev_end = stages.last_end;
    let start = now_ms();
    stages.frame = FrameStages {
        start,
        prev_end,
        ..Default::default()
    };
    // published at the previous frame's handoff, the end of its app update (web only)
    {
        use bevy::render::pipelined_rendering::diagnostics::{
            MAIN_UPDATE, WAIT_HANDOFF, WAIT_RENDER,
        };
        let [render, handoff, main] = [WAIT_RENDER, WAIT_HANDOFF, MAIN_UPDATE].map(|path| {
            store
                .as_ref()
                .and_then(|s| s.get(&path))
                .and_then(|d| d.measurement())
                .map(|m| (m.value, start - m.time.elapsed().as_secs_f64() * 1000.0))
        });
        let f = &mut stages.frame;
        f.prev_wait_render = render.map(|(v, _)| v);
        f.prev_wait_handoff = handoff.map(|(v, _)| v);
        f.prev_main_update = main.map(|(v, _)| v);
        f.prev_handoff_end = handoff.map(|(_, at)| at);
    }
    #[cfg(target_arch = "wasm32")]
    {
        let (js, wasm) = dcl_wasm::tick_marks::heap_mb();
        stages.frame.heap_js_mb = js;
        stages.frame.wasm_mb = wasm;
        let (waits, sleeps) = dcl_wasm::tick_marks::relay_steps();
        stages.frame.relay_waits = Some(waits);
        stages.frame.relay_sleeps = Some(sleeps);
    }
}

fn frame_end(mut stages: ResMut<TickStages>) {
    stages.last_end = now_ms();
}

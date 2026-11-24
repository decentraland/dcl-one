//! Auto Graphics: while the frame is longer than its target and the CPU does not account for
//! it, the session's graphics are lowered one rung of a ladder at a time (`ladder`), and a rung is
//! kept only when the frame got shorter and stayed shorter for a second reading. Rungs are tried
//! again upwards from time to time and kept only when the frame did not get longer. The rungs,
//! in the order measured to buy the most frame time on the web: the sun's shadows off, the render
//! scale down to the player's floor (50%) in Render Resolution steps, bloom off. The player's
//! saved settings never change: the lowered ones live in `SessionGraphics::auto`.
//!
//! It decides on the frame's wall time and the CPU timers of [`FrameTimeBreakdown`], never on a
//! GPU time: a GPU time is not available, or not reliable, on every platform. A trial that did
//! not help is undone and not repeated for a hold that doubles each time, so a frame that is long
//! for another reason costs a bounded share of the session at lower graphics.

use bevy::prelude::*;
use bevy_console::ConsoleCommand;
use common::{
    perf::FrameTimeBreakdown,
    structs::{
        AppConfig, AutoGraphics, AutoGraphicsNotices, BloomSetting, GraphicsSettings,
        SessionGraphics, ShadowSetting,
    },
};
use console::DoAddConsoleCommand;
use scene_runner::SceneLoopTiming;

/// After a change of rung, the time left out of the next reading: the scene target is resized,
/// pipelines may compile (the compositor's, the first time away from 100%).
const SETTLE_SECONDS: f64 = 1.0;
/// The length of one reading. Its frame time is the median, so the long frames of a scene load
/// do not move it.
const WINDOW_SECONDS: f64 = 2.0;
/// A reading needs this many frames, however long it takes.
const MIN_WINDOW_FRAMES: usize = 20;
/// A frame this long is a stall (a hidden tab, a blocked thread), not a rendering cost: it
/// discards the reading in progress.
const STALL_MS: f32 = 250.0;
/// The frame is over its target, and worth a lower rung, past this multiple of the target.
const OVER_TARGET: f32 = 1.1;
/// The frame is at its target up to this multiple of it. Between the two nothing changes.
const AT_TARGET: f32 = 1.05;
/// With the CPU side at this share of the frame or more, a lower rung is not tried.
const CPU_BOUND_SHARE: f32 = 0.85;
/// A lower rung is kept when the frame got shorter by the larger of these, in two readings.
const MIN_GAIN_MS: f32 = 1.5;
const MIN_GAIN_SHARE: f32 = 0.06;
/// A higher rung is kept when the frame got longer by no more than the larger of these.
const MAX_LOSS_MS: f32 = 0.75;
const MAX_LOSS_SHARE: f32 = 0.03;
/// The first hold after a trial that was undone; each further one doubles, up to the longest.
const FIRST_HOLD_SECONDS: f64 = 30.0;
const LONGEST_HOLD_SECONDS: f64 = 600.0;
/// How long the controller stays on a lower rung before the player is told: a descent stops on
/// several rungs on its way down, and the message names where it ended.
const ANNOUNCE_AFTER_SECONDS: f64 = 10.0;
/// The wait between higher rungs while each one is kept.
const CLIMB_SECONDS: f64 = 5.0;

/// What the controller is doing.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Phase {
    /// Reading the frame at the rung it has settled on.
    Steady,
    /// Trying lower rungs, from this rung and frame time, until one shortens the frame.
    TryingLower { from_steps: i32, from_ms: f32 },
    /// A lower rung shortened the frame once; one more reading decides whether it is kept, so a
    /// single quiet stretch does not settle the session on lower graphics.
    Confirming { from_steps: i32, from_ms: f32 },
    /// Trying one rung higher than the one that gave this frame time.
    TryingHigher { from_ms: f32 },
}

impl Phase {
    pub fn name(&self) -> &'static str {
        match self {
            Phase::Steady => "steady",
            Phase::TryingLower { .. } => "trying lower",
            Phase::Confirming { .. } => "confirming",
            Phase::TryingHigher { .. } => "trying higher",
        }
    }
}

/// The range of rungs and the frame time to reach.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Limits {
    pub floor_steps: i32,
    pub ceiling_steps: i32,
    pub target_ms: f32,
}

/// The decisions, apart from the engine so a test can drive them with a clock and frame times.
#[derive(Debug)]
pub struct Controller {
    steps: i32,
    ceiling_steps: i32,
    phase: Phase,
    frame_ms: Vec<f32>,
    cpu_ms: Vec<f32>,
    window_started: Option<f64>,
    settle_until: f64,
    lower_hold_until: f64,
    lower_hold_seconds: f64,
    higher_hold_until: f64,
    higher_hold_seconds: f64,
}

impl Controller {
    /// Starts at the ceiling.
    pub fn new(ceiling_steps: i32) -> Self {
        Self {
            steps: ceiling_steps,
            ceiling_steps,
            phase: Phase::Steady,
            frame_ms: Vec::new(),
            cpu_ms: Vec::new(),
            window_started: None,
            settle_until: 0.0,
            lower_hold_until: 0.0,
            lower_hold_seconds: FIRST_HOLD_SECONDS,
            higher_hold_until: 0.0,
            higher_hold_seconds: FIRST_HOLD_SECONDS,
        }
    }

    pub fn steps(&self) -> i32 {
        self.steps
    }

    pub fn phase(&self) -> Phase {
        self.phase
    }

    /// Takes one frame's wall time and CPU time at `now` (seconds) and returns the rung to use. `cpu_ms` is the longer of the main world's and the render world's
    /// busy time.
    pub fn frame(&mut self, now: f64, frame_ms: f32, cpu_ms: f32, limits: Limits) -> i32 {
        if limits.ceiling_steps != self.ceiling_steps {
            // the ladder got shorter or longer: start again from its top, keeping a lower rung
            // already in use
            *self = Self {
                steps: self.steps.min(limits.ceiling_steps),
                ..Self::new(limits.ceiling_steps)
            };
            self.settle_until = now + SETTLE_SECONDS;
        }
        if self.steps < limits.floor_steps {
            self.steps = limits.floor_steps.min(limits.ceiling_steps);
        }
        if frame_ms > STALL_MS {
            self.clear_window();
            return self.steps;
        }
        if now < self.settle_until {
            return self.steps;
        }
        let started = *self.window_started.get_or_insert(now);
        self.frame_ms.push(frame_ms);
        self.cpu_ms.push(cpu_ms);
        if now - started < WINDOW_SECONDS || self.frame_ms.len() < MIN_WINDOW_FRAMES {
            return self.steps;
        }
        let frame = median(&mut self.frame_ms);
        let cpu = median(&mut self.cpu_ms);
        self.clear_window();
        self.decide(now, frame, cpu, limits);
        self.steps
    }

    fn decide(&mut self, now: f64, frame_ms: f32, cpu_ms: f32, limits: Limits) {
        let at_target = frame_ms <= limits.target_ms * AT_TARGET;
        match self.phase {
            Phase::Steady => {
                let over_target = frame_ms > limits.target_ms * OVER_TARGET;
                let cpu_bound = cpu_ms >= frame_ms * CPU_BOUND_SHARE;
                if over_target
                    && !cpu_bound
                    && self.steps > limits.floor_steps
                    && now >= self.lower_hold_until
                {
                    self.phase = Phase::TryingLower {
                        from_steps: self.steps,
                        from_ms: frame_ms,
                    };
                    self.set_steps(self.steps - 1, now);
                } else if self.steps < limits.ceiling_steps && now >= self.higher_hold_until {
                    self.phase = Phase::TryingHigher { from_ms: frame_ms };
                    self.set_steps(self.steps + 1, now);
                }
            }
            Phase::TryingLower {
                from_steps,
                from_ms,
            } => {
                if helped(from_ms, frame_ms) || at_target {
                    self.phase = Phase::Confirming {
                        from_steps,
                        from_ms,
                    };
                } else {
                    self.lower_further(now, from_steps, from_ms, limits);
                }
            }
            Phase::Confirming {
                from_steps,
                from_ms,
            } => {
                if helped(from_ms, frame_ms) || at_target {
                    self.phase = Phase::Steady;
                    self.lower_hold_seconds = FIRST_HOLD_SECONDS;
                    self.higher_hold_until = now + self.higher_hold_seconds;
                } else {
                    self.lower_further(now, from_steps, from_ms, limits);
                }
            }
            Phase::TryingHigher { from_ms } => {
                self.phase = Phase::Steady;
                let kept =
                    at_target || frame_ms - from_ms <= MAX_LOSS_MS.max(from_ms * MAX_LOSS_SHARE);
                if kept {
                    self.higher_hold_seconds = FIRST_HOLD_SECONDS;
                    self.higher_hold_until = now + CLIMB_SECONDS;
                } else {
                    self.set_steps(self.steps - 1, now);
                    self.higher_hold_until = now + self.higher_hold_seconds;
                    self.higher_hold_seconds =
                        (self.higher_hold_seconds * 2.0).min(LONGEST_HOLD_SECONDS);
                }
            }
        }
    }

    /// The rung on trial did not help: the next lower one, or back to where the trial started
    /// at the bottom of the ladder, with a hold before the next trial.
    fn lower_further(&mut self, now: f64, from_steps: i32, from_ms: f32, limits: Limits) {
        if self.steps > limits.floor_steps {
            self.phase = Phase::TryingLower {
                from_steps,
                from_ms,
            };
            self.set_steps(self.steps - 1, now);
        } else {
            self.phase = Phase::Steady;
            self.set_steps(from_steps.min(limits.ceiling_steps), now);
            self.lower_hold_until = now + self.lower_hold_seconds;
            self.lower_hold_seconds = (self.lower_hold_seconds * 2.0).min(LONGEST_HOLD_SECONDS);
        }
    }

    fn set_steps(&mut self, steps: i32, now: f64) {
        self.steps = steps;
        self.settle_until = now + SETTLE_SECONDS;
    }

    fn clear_window(&mut self) {
        self.frame_ms.clear();
        self.cpu_ms.clear();
        self.window_started = None;
    }
}

/// Measured from the rung the trial started at: with frames that snap to the display's refresh,
/// one rung alone may change nothing.
fn helped(from_ms: f32, frame_ms: f32) -> bool {
    from_ms - frame_ms >= MIN_GAIN_MS.max(from_ms * MIN_GAIN_SHARE)
}

fn median(values: &mut [f32]) -> f32 {
    values.sort_unstable_by(f32::total_cmp);
    values[values.len() / 2]
}

pub struct AutoGraphicsPlugin;

impl Plugin for AutoGraphicsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<AutoGraphicsState>()
            .init_resource::<AutoGraphicsNotices>()
            .add_systems(Update, auto_adjust_graphics)
            .add_console_command::<AutoGraphicsStateCommand, _>(auto_graphics_state)
            .add_console_command::<AutoGraphicsNoticeCommand, _>(auto_graphics_notice);
    }
}

/// The rungs from the lowest graphics (index 0) to the player's own settings (the last), for the
/// player's settings: only what the player has on is turned off, and the render scale goes down
/// from the player's Render Resolution.
pub fn ladder(graphics: &GraphicsSettings) -> Vec<AutoGraphics> {
    let mut rungs = vec![AutoGraphics::default()];
    let mut rung = AutoGraphics::default();
    if graphics.shadow_settings != ShadowSetting::Off {
        rung.shadows = Some(ShadowSetting::Off);
        rungs.push(rung);
    }
    let floor = GraphicsSettings::MIN_PLAYER_RENDER_SCALE_STEPS;
    for steps in (floor..graphics.render_scale_steps()).rev() {
        rung.render_scale = Some(steps as f32 / GraphicsSettings::RENDER_SCALE_STEPS as f32);
        rungs.push(rung);
    }
    if graphics.bloom != BloomSetting::Off {
        rung.bloom = Some(BloomSetting::Off);
        rungs.push(rung);
    }
    rungs.reverse();
    rungs
}

/// Auto Graphics while it runs: the controller over the ladder built from the player's settings.
#[derive(Resource, Default)]
pub struct AutoGraphicsState {
    running: Option<Running>,
}

struct Running {
    ladder: Vec<AutoGraphics>,
    controller: Controller,
    /// The lowest rung announced since the player's own settings were last in effect.
    announced: i32,
    /// When the controller last settled, and on which rung.
    settled: Option<(f64, i32)>,
}

impl AutoGraphicsState {
    pub fn phase(&self) -> Option<Phase> {
        self.running
            .as_ref()
            .map(|running| running.controller.phase())
    }

    /// The rung in use and the number of rungs, while running.
    pub fn rung(&self) -> Option<(usize, usize)> {
        self.running
            .as_ref()
            .map(|running| (running.controller.steps() as usize, running.ladder.len()))
    }
}

/// The frame time the controller aims for: the paced frame rate's, and with no cap (Max, or 0)
/// the monitor's when it is known, else 60 fps.
fn target_frame_ms(
    session: &SessionGraphics,
    config: &AppConfig,
    refresh_hz: Option<usize>,
) -> f32 {
    let paced = SessionGraphics::paced_fps(Some(session), config, refresh_hz);
    let fps = if paced == 0 || paced >= 999 {
        refresh_hz.unwrap_or(60)
    } else {
        paced
    };
    1000.0 / fps.max(1) as f32
}

/// Feeds the controller the previous frame and writes its rung into `SessionGraphics::auto`. The
/// setting being off, or a host's session render scale, stops it and gives the player's settings
/// back; it starts again from the top of the ladder.
pub fn auto_adjust_graphics(
    time: Res<Time<Real>>,
    config: Res<AppConfig>,
    mut session: ResMut<SessionGraphics>,
    breakdown: Option<Res<FrameTimeBreakdown>>,
    scene_loop: Option<Res<SceneLoopTiming>>,
    mut state: ResMut<AutoGraphicsState>,
    mut notices: ResMut<AutoGraphicsNotices>,
) {
    if !config.graphics.auto_graphics || session.render_scale.is_some() {
        if state.running.is_some() {
            state.running = None;
        }
        if session.auto.is_adjusted() {
            session.auto = AutoGraphics::default();
        }
        return;
    }
    let ladder = ladder(&config.graphics);
    if state
        .running
        .as_ref()
        .is_none_or(|running| running.ladder != ladder)
    {
        // the player changed a setting the ladder is built from: start from their settings
        let top = ladder.len() as i32 - 1;
        state.running = Some(Running {
            ladder,
            controller: Controller::new(top),
            announced: top,
            settled: None,
        });
    }
    let target_ms = target_frame_ms(
        &session,
        &config,
        scene_loop.and_then(|timing| timing.refresh_hz),
    );
    // the limiter's sleep is inside the main world's schedules and is not work
    let cpu_ms = breakdown.map_or(0.0, |breakdown| {
        (breakdown.main_world_schedules_ms - breakdown.frame_limiter_sleep_ms)
            .max(breakdown.render_world_cpu_ms)
    });
    let frame_ms = time.delta_secs_f64() as f32 * 1000.0;
    let running = state.running.as_mut().unwrap();
    let limits = Limits {
        floor_steps: 0,
        ceiling_steps: running.ladder.len() as i32 - 1,
        target_ms,
    };
    let steps = running
        .controller
        .frame(time.elapsed_secs_f64(), frame_ms, cpu_ms, limits);
    let rung = running.ladder[steps as usize];
    let now = time.elapsed_secs_f64();
    if running.controller.phase() != Phase::Steady {
        running.settled = None;
    } else if running.settled.is_none_or(|(_, rung)| rung != steps) {
        running.settled = Some((now, steps));
    }
    if steps == limits.ceiling_steps {
        running.announced = steps;
    }
    let announce = running.settled.is_some_and(|(since, _)| {
        now - since >= ANNOUNCE_AFTER_SECONDS && steps < running.announced
    });
    if announce {
        running.announced = steps;
    }
    if announce {
        notices.raise(rung);
    }
    if session.auto != rung {
        session.auto = rung;
    }
}

/// Read Auto Graphics: `enabled`, `phase` (`off`, `steady`, `trying lower`, `confirming`,
/// `trying higher`), `rung` and `rungs` (0 is the lowest graphics, the last the player's own), what
/// is lowered (`shadows`, `renderScale`, `bloom`: `null` while the player's own setting is in
/// effect) and `notices`, the number of messages raised this session.
#[derive(clap::Parser, ConsoleCommand)]
#[command(name = "/auto_graphics_state")]
struct AutoGraphicsStateCommand;

fn auto_graphics_state(
    mut cmd: ConsoleCommand<AutoGraphicsStateCommand>,
    config: Res<AppConfig>,
    session: Res<SessionGraphics>,
    state: Res<AutoGraphicsState>,
    notices: Res<AutoGraphicsNotices>,
) {
    let Some(Ok(_)) = cmd.take() else { return };
    let (rung, rungs) = state.rung().unzip();
    cmd.reply_ok(
        serde_json::json!({
            "enabled": config.graphics.auto_graphics,
            "phase": state.phase().map_or("off", |phase| phase.name()),
            "rung": rung,
            "rungs": rungs,
            "shadows": session.auto.shadows.map(|s| format!("{s:?}")),
            "renderScale": session.auto.render_scale,
            "bloom": session.auto.bloom.map(|b| format!("{b:?}")),
            "notices": notices.count,
        })
        .to_string(),
    );
}

/// Raise the Auto Graphics message without changing any setting, as if graphics had been lowered
/// to shadows off and 70% render resolution, for checking the HUD. Replies with the number of
/// messages raised this session.
#[derive(clap::Parser, ConsoleCommand)]
#[command(name = "/auto_graphics_notice")]
struct AutoGraphicsNoticeCommand;

fn auto_graphics_notice(
    mut cmd: ConsoleCommand<AutoGraphicsNoticeCommand>,
    mut notices: ResMut<AutoGraphicsNotices>,
) {
    let Some(Ok(_)) = cmd.take() else { return };
    notices.raise(AutoGraphics {
        shadows: Some(ShadowSetting::Off),
        render_scale: Some(0.7),
        bloom: None,
    });
    cmd.reply_ok(notices.count.to_string());
}

#[cfg(test)]
mod tests {
    use super::*;

    const LIMITS: Limits = Limits {
        floor_steps: 5,
        ceiling_steps: 10,
        target_ms: 1000.0 / 60.0,
    };

    /// Runs the controller until `until` seconds with the frame and CPU time `cost` gives each
    /// scale, and returns the lowest scale it used.
    fn run(
        controller: &mut Controller,
        now: &mut f64,
        until: f64,
        cost: impl Fn(i32) -> (f32, f32),
    ) -> i32 {
        let mut lowest = controller.steps();
        while *now < until {
            let (frame_ms, cpu_ms) = cost(controller.steps());
            *now += f64::from(frame_ms) / 1000.0;
            lowest = lowest.min(controller.frame(*now, frame_ms, cpu_ms, LIMITS));
        }
        lowest
    }

    #[test]
    fn settles_on_the_highest_scale_that_reaches_the_target() {
        // 8 ms that do not depend on the scale, 24 ms of pixels at 100%: 60% is the first
        // scale at the target (16.6 ms), 70% is not (19.8 ms)
        let cost = |steps: i32| (8.0 + 24.0 * (steps as f32 / 10.0).powi(2), 8.0);
        let mut controller = Controller::new(10);
        let mut now = 0.0;
        run(&mut controller, &mut now, 40.0, cost);
        assert_eq!((controller.steps(), controller.phase()), (6, Phase::Steady));
        // the trial of 70% that follows is undone
        run(&mut controller, &mut now, 70.0, cost);
        assert_eq!((controller.steps(), controller.phase()), (6, Phase::Steady));
    }

    #[test]
    fn leaves_a_cpu_bound_frame_alone() {
        let mut controller = Controller::new(10);
        let lowest = run(&mut controller, &mut 0.0, 120.0, |_| (30.0, 28.0));
        assert_eq!(lowest, 10);
    }

    #[test]
    fn undoes_a_trial_that_did_not_shorten_the_frame() {
        // long for a reason the CPU timers do not show and the scale does not change
        let mut controller = Controller::new(10);
        let mut now = 0.0;
        let lowest = run(&mut controller, &mut now, 40.0, |_| (30.0, 10.0));
        assert_eq!(lowest, 5);
        assert_eq!(
            (controller.steps(), controller.phase()),
            (10, Phase::Steady)
        );
    }

    #[test]
    fn follows_frames_that_snap_to_the_refresh() {
        // four refreshes of a 120 Hz display above 50%, three at it: no single step from 100%
        // changes the frame
        let cost = |steps: i32| (if steps > 5 { 33.3 } else { 25.0 }, 10.0);
        let mut controller = Controller::new(10);
        let mut now = 0.0;
        run(&mut controller, &mut now, 40.0, cost);
        assert_eq!((controller.steps(), controller.phase()), (5, Phase::Steady));
        run(&mut controller, &mut now, 70.0, cost);
        assert_eq!((controller.steps(), controller.phase()), (5, Phase::Steady));
    }

    #[test]
    fn climbs_back_when_the_frame_gets_cheap() {
        let heavy = |steps: i32| (8.0 + 24.0 * (steps as f32 / 10.0).powi(2), 8.0);
        let mut controller = Controller::new(10);
        let mut now = 0.0;
        run(&mut controller, &mut now, 40.0, heavy);
        assert_eq!(controller.steps(), 6);
        run(&mut controller, &mut now, 200.0, |_| (16.6, 8.0));
        assert_eq!(
            (controller.steps(), controller.phase()),
            (10, Phase::Steady)
        );
    }

    #[test]
    fn ignores_stalls_and_follows_a_lower_ceiling() {
        let mut controller = Controller::new(10);
        let lowest = run(&mut controller, &mut 0.0, 60.0, |_| (300.0, 10.0));
        assert_eq!(lowest, 10);

        let lowered = Limits {
            ceiling_steps: 7,
            ..LIMITS
        };
        assert_eq!(controller.frame(61.0, 16.0, 8.0, lowered), 7);
    }

    #[test]
    fn the_ladder_turns_off_shadows_then_lowers_the_scale_then_turns_off_bloom() {
        let graphics = GraphicsSettings {
            shadow_settings: ShadowSetting::Low,
            render_scale: 1.0,
            bloom: BloomSetting::Low,
            ..Default::default()
        };
        let rungs = ladder(&graphics);
        assert_eq!(rungs.len(), 8);
        assert_eq!(rungs[7], AutoGraphics::default());
        let shadows_off = AutoGraphics {
            shadows: Some(ShadowSetting::Off),
            ..Default::default()
        };
        assert_eq!(rungs[6], shadows_off);
        assert_eq!(
            rungs[5],
            AutoGraphics {
                render_scale: Some(0.9),
                ..shadows_off
            }
        );
        assert_eq!(
            rungs[0],
            AutoGraphics {
                shadows: Some(ShadowSetting::Off),
                render_scale: Some(0.5),
                bloom: Some(BloomSetting::Off),
            }
        );
    }

    #[test]
    fn the_ladder_only_lowers_what_the_player_has_on() {
        let graphics = GraphicsSettings {
            shadow_settings: ShadowSetting::Off,
            render_scale: 0.6,
            bloom: BloomSetting::Off,
            ..Default::default()
        };
        assert_eq!(
            ladder(&graphics),
            vec![
                AutoGraphics {
                    render_scale: Some(0.5),
                    ..Default::default()
                },
                AutoGraphics::default(),
            ]
        );
    }
}

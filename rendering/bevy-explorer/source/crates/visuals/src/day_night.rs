use bevy::{
    ecs::{component::ComponentIdFor, entity::EntityHashSet, system::SystemParam},
    prelude::*,
};
use bevy_console::{AddConsoleCommand, ConsoleCommand};
use common::structs::{PrimaryPlayerRes, SceneTime, TimeOfDay};
use dcl_component::{
    proto_components::sdk::components::{PbSkyboxTime, TransitionMode},
    SceneComponentId,
};
use dcl_types::interface::ComponentPosition;
use scene_runner::{
    initialize_scene::TestingData, renderer_context::RendererSceneContext,
    update_world::AddCrdtInterfaceExt, ContainingScene,
};

/// One hour in seconds
const ONE_HOUR: f32 = 60. * 60.;
const DEFAULT_CLOCK_SPEED: f32 = 12.;
/// One hour in seconds using u32
const ONE_HOUR_U32: u32 = 60 * 60;
/// 24 hours in seconds
const TWENTY_FOUR_HOURS: f32 = 24. * ONE_HOUR;
/// How many hours per second are advanced during a [`TimeSkip`]
const HOURS_PER_SECOND: f32 = 12. * ONE_HOUR;

pub use common::sky_preset::day_fraction;

#[derive(Component)]
struct TimeKeeper;

#[derive(Component)]
struct RunningClock {
    /// secs since midnight
    pub time: f32,
    pub speed: f32,
}

impl RunningClock {
    /// A stopped clock is a time set explicitly (HUD / `/time`), which overrides scene times.
    fn overrides_scene(&self) -> bool {
        self.speed == 0.0
    }
}

/// Where a skip from `from` to `to` ends going the short way round the clock.
fn nearest_end(from: f32, to: f32) -> f32 {
    from + (to - from + TWENTY_FOUR_HOURS / 2.).rem_euclid(TWENTY_FOUR_HOURS)
        - TWENTY_FOUR_HOURS / 2.
}

#[derive(Resource)]
struct TimeSkip {
    start: f32,
    end: f32,
    progress: f32,
    easing: EaseFunction,
}

#[derive(Component)]
struct SceneTimeSource(Entity);

#[derive(Component, Deref)]
#[component(immutable)]
struct SkyboxTime(PbSkyboxTime);

impl From<PbSkyboxTime> for SkyboxTime {
    fn from(value: PbSkyboxTime) -> Self {
        Self(value)
    }
}

#[derive(Component)]
struct SkyboxTimeSource(Entity);

pub struct DayNightPlugin;

impl Plugin for DayNightPlugin {
    fn build(&self, app: &mut App) {
        app.add_crdt_lww_component::<PbSkyboxTime, SkyboxTime>(
            SceneComponentId::SKYBOX_TIME,
            ComponentPosition::RootOnly,
        );

        app.add_systems(Startup, start_clock);
        app.add_systems(
            First,
            (
                (
                    update_running_clock,
                    (fetch_time_from_skybox, fetch_time_from_scene).chain(),
                ),
                push_time_of_day_from_time_skip.run_if(resource_exists::<TimeSkip>),
                push_time_of_day_from_running_clock.run_if(not(resource_exists::<TimeSkip>)),
            )
                .chain()
                .after(bevy::time::TimeSystem),
        );

        app.add_console_command::<TimeOfDayConsoleCommand, _>(timeofday_console_command);
    }
}

fn start_clock(mut commands: Commands) {
    let time = 10.0 * 3600.0;
    commands.insert_resource(TimeOfDay { time });
    commands
        .spawn((
            TimeKeeper,
            RunningClock {
                time,
                speed: DEFAULT_CLOCK_SPEED,
            },
            SceneTime { time: 0. },
        ))
        .observe(check_new_scene_time)
        .observe(check_new_skybox_time);
}

/// Which scene a root-only component `C` comes from for the primary player: of the scenes
/// containing the player that have it, the parcel scene before portables, then the earliest
/// started.
#[derive(SystemParam)]
pub(crate) struct RootComponentSource<'w, 's, C: Component> {
    primary_player: Res<'w, PrimaryPlayerRes>,
    containing_scene: ContainingScene<'w, 's>,
    scenes: Query<'w, 's, (Entity, &'static RendererSceneContext), With<C>>,
    added: Query<'w, 's, (), Added<C>>,
    removed: RemovedComponents<'w, 's, C>,
    last_player_state: Local<'s, Option<Option<(IVec2, bool)>>>,
}

impl<C: Component> RootComponentSource<'_, '_, C> {
    /// The scene to read `C` from, or `None` while nothing it depends on has changed: the scene
    /// layout, the player's parcel, or a scene adding or removing `C`.
    pub(crate) fn changed_pick(&mut self) -> Option<Option<Entity>> {
        let player_state = self
            .containing_scene
            .transforms
            .get(self.primary_player.0)
            .ok()
            .map(|(gt, oow)| (scene_runner::vec3_to_parcel(gt.translation()), oow));
        let any_removed = self.removed.read().next().is_some();
        self.removed.clear();
        let dirty = self.containing_scene.scene_layout_changed()
            || !self.added.is_empty()
            || any_removed
            || self.last_player_state.as_ref() != Some(&player_state);
        *self.last_player_state = Some(player_state);
        if !dirty {
            return None;
        }

        let containing_primary_player = self.containing_scene.get(self.primary_player.0);
        Some(
            self.scenes
                .iter_many_unique(EntityHashSet::from_iter(containing_primary_player))
                .min_by_key(|(_, renderer_scene_context)| {
                    (
                        renderer_scene_context.is_portable,
                        renderer_scene_context.start_tick,
                    )
                })
                .map(|(entity, _)| entity),
        )
    }
}

#[expect(clippy::type_complexity, reason = "Queries are complex")]
fn fetch_time_from_scene(
    mut commands: Commands,
    mut source: RootComponentSource<SceneTime>,
    scenes: Query<&SceneTime>,
    time_keeper: Single<(Entity, Option<&SceneTime>, Option<&SceneTimeSource>), With<TimeKeeper>>,
) {
    // only recompute the time source when the scenes at the player position may have changed
    let Some(pick) = source.changed_pick() else {
        return;
    };
    let maybe_scene_time =
        pick.and_then(|entity| scenes.get(entity).ok().map(|time| (entity, time)));

    let (time_keeper, time_keeper_scene_time, scene_time_source) = time_keeper.into_inner();
    if let Some((entity, scene_time)) = maybe_scene_time {
        if scene_time_source
            .filter(|source| source.0 == entity)
            .is_none()
        {
            debug!("Now using time from scene {}.", entity);
            commands.entity(time_keeper).try_insert((
                SceneTime {
                    time: scene_time.time,
                },
                SceneTimeSource(entity),
            ));
        }
    } else if time_keeper_scene_time.is_some() {
        debug!("No longer using SceneTime.");
        commands
            .entity(time_keeper)
            .try_remove::<(SceneTime, SceneTimeSource)>();
    }
}

#[expect(clippy::type_complexity, reason = "Queries are complex")]
fn fetch_time_from_skybox(
    mut commands: Commands,
    mut source: RootComponentSource<SkyboxTime>,
    scenes: Query<Ref<SkyboxTime>>,
    time_keeper: Single<(Entity, Option<&SkyboxTime>, Option<&SkyboxTimeSource>), With<TimeKeeper>>,
    scene_time_component_id: ComponentIdFor<SceneTime>,
) {
    // only recompute the time source when the scenes at the player position may have changed.
    // SkyboxTime is an immutable component so value updates re-insert it (-> Added fires).
    let Some(pick) = source.changed_pick() else {
        return;
    };
    let maybe_skybox_time =
        pick.and_then(|entity| scenes.get(entity).ok().map(|time| (entity, time)));

    let (time_keeper, time_keeper_skybox_time, skybox_time_source) = time_keeper.into_inner();
    if let Some((entity, skybox_time)) = maybe_skybox_time {
        if skybox_time_source
            .filter(|source| source.0 == entity)
            .is_none()
        {
            debug!("Now using skybox time from scene {}.", entity);
            commands.entity(time_keeper).try_insert((
                SkyboxTime(PbSkyboxTime {
                    fixed_time: skybox_time.fixed_time,
                    transition_mode: skybox_time.transition_mode,
                }),
                SkyboxTimeSource(entity),
            ));
        } else if skybox_time.is_changed() {
            debug!("New skybox time from scene {}.", entity);
            commands.entity(time_keeper).try_insert((
                SkyboxTime(PbSkyboxTime {
                    fixed_time: skybox_time.fixed_time,
                    transition_mode: skybox_time.transition_mode,
                }),
                SkyboxTimeSource(entity),
            ));
        }
    } else if time_keeper_skybox_time.is_some() {
        debug!("No longer using SkyboxTime.");
        commands
            .entity(time_keeper)
            .try_remove::<(SkyboxTime, SkyboxTimeSource)>();
        commands.trigger_targets(OnInsert, (time_keeper, *scene_time_component_id));
    }
}

fn update_running_clock(
    mut time_keeper: Single<&mut RunningClock, With<TimeKeeper>>,
    time: Res<Time>,
    testing: Res<TestingData>,
) {
    // The test runner's speed must not change the sun between captures. Keep
    // scene-authored time overrides active; only the background clock is frozen.
    if testing.scene_test_run {
        return;
    }
    let speed = time_keeper.speed;
    time_keeper.time += time.delta_secs() * speed;
    time_keeper.time %= TWENTY_FOUR_HOURS;
    if time_keeper.time < 0.0 {
        time_keeper.time += TWENTY_FOUR_HOURS;
    }
}

fn push_time_of_day_from_time_skip(
    mut commands: Commands,
    mut time_skip: ResMut<TimeSkip>,
    mut time_of_day: ResMut<TimeOfDay>,
    time: Res<Time>,
) {
    trace!("Pushing time from TimeSkip");
    let delta = time.delta_secs();
    let amount_to_skip = (time_skip.end - time_skip.start).abs();
    let duration = amount_to_skip / HOURS_PER_SECOND;
    time_skip.progress += delta / duration;

    time_of_day.time = time_skip
        .start
        .lerp(
            time_skip.end,
            time_skip.easing.sample_clamped(time_skip.progress),
        )
        .rem_euclid(TWENTY_FOUR_HOURS);

    if time_skip.progress >= 1. {
        debug!("TimeSkip has ended.");
        commands.remove_resource::<TimeSkip>();
    }
}

#[expect(clippy::type_complexity, reason = "Queries are complex")]
fn push_time_of_day_from_running_clock(
    mut commands: Commands,
    time_keeper: Single<(&RunningClock, Has<SceneTime>, Has<SkyboxTime>), With<TimeKeeper>>,
    mut time_of_day: ResMut<TimeOfDay>,
) {
    let (running_clock, has_scene_time, has_skybox_time) = time_keeper.into_inner();
    if (has_scene_time || has_skybox_time) && !running_clock.overrides_scene() {
        return;
    }
    let end = nearest_end(time_of_day.time, running_clock.time);
    if (end - time_of_day.time).abs() > ONE_HOUR {
        debug!("Starting a TimeSkip from {} to {}.", time_of_day.time, end);
        commands.insert_resource(TimeSkip {
            start: time_of_day.time,
            end,
            progress: 0.,
            easing: EaseFunction::SmoothStep,
        });
    } else {
        trace!("Pushing time from RunningClock");
        time_of_day.time = running_clock.time;
    }
}

#[expect(clippy::type_complexity, reason = "Queries are complex")]
fn check_new_scene_time(
    trigger: Trigger<OnInsert, SceneTime>,
    mut commands: Commands,
    time_keeper: Single<(&SceneTime, &RunningClock), (With<TimeKeeper>, Without<SkyboxTime>)>,
    mut time_of_day: ResMut<TimeOfDay>,
    time_skip: Option<Res<TimeSkip>>,
) {
    let (scene_time, running_clock) = time_keeper.into_inner();
    if running_clock.overrides_scene() {
        return;
    }
    debug!(
        "Received SceneTime on {} of {}.",
        trigger.target(),
        scene_time.time
    );
    if (scene_time.time - time_of_day.time).abs() < ONE_HOUR {
        time_of_day.time = scene_time.time;
        if time_skip.is_some() {
            debug!("Stop TimeSkip since new SceneTime is already close.");
            commands.remove_resource::<TimeSkip>();
        }
    } else {
        let end = if scene_time.time > time_of_day.time {
            scene_time.time
        } else {
            scene_time.time + TWENTY_FOUR_HOURS
        };
        debug!("Starting a TimeSkip from {} to {}.", time_of_day.time, end);
        commands.insert_resource(TimeSkip {
            start: time_of_day.time,
            end,
            progress: 0.,
            easing: EaseFunction::SmoothStep,
        });
    }
}

fn check_new_skybox_time(
    _trigger: Trigger<OnInsert, SkyboxTime>,
    mut commands: Commands,
    time_keeper: Single<(&SkyboxTime, &RunningClock), With<TimeKeeper>>,
    mut time_of_day: ResMut<TimeOfDay>,
    time_skip: Option<Res<TimeSkip>>,
) {
    let (skybox_time, running_clock) = time_keeper.into_inner();
    if running_clock.overrides_scene() {
        return;
    }
    let new_time = skybox_time.fixed_time as f32;
    if (new_time - time_of_day.time).abs() < ONE_HOUR {
        time_of_day.time = new_time;
        if time_skip.is_some() {
            debug!("Stop TimeSkip since new SceneTime is already close.");
            commands.remove_resource::<TimeSkip>();
        }
    } else {
        let end = match (new_time > time_of_day.time, skybox_time.transition_mode()) {
            (true, TransitionMode::TmForward) => new_time,
            (false, TransitionMode::TmForward) => new_time + TWENTY_FOUR_HOURS,
            (true, TransitionMode::TmBackward) => new_time - TWENTY_FOUR_HOURS,
            (false, TransitionMode::TmBackward) => new_time,
        };
        debug!("Starting a TimeSkip from {} to {}.", time_of_day.time, end);
        commands.insert_resource(TimeSkip {
            start: time_of_day.time,
            end,
            progress: 0.,
            easing: EaseFunction::SmoothStep,
        });
    }
}

#[derive(clap::Parser, ConsoleCommand)]
#[command(name = "/time")]
pub struct TimeOfDayConsoleCommand {
    /// Resume the normal day cycle and scene-specified skies.
    #[arg(long, conflicts_with_all = ["time", "speed"])]
    pub resume: bool,
    pub time: Option<f32>,
    pub speed: Option<f32>,
}

#[expect(clippy::type_complexity, reason = "Queries are complex")]
fn timeofday_console_command(
    mut input: ConsoleCommand<TimeOfDayConsoleCommand>,
    mut commands: Commands,
    time_keeper: Single<
        (
            Entity,
            &mut RunningClock,
            Option<&SceneTime>,
            Option<&SkyboxTime>,
        ),
        With<TimeKeeper>,
    >,
    scene_time_component_id: ComponentIdFor<SceneTime>,
    skybox_time_component_id: ComponentIdFor<SkyboxTime>,
) {
    if let Some(Ok(command)) = input.take() {
        let (time_keeper, mut running_clock, scene_time, skybox_time) = time_keeper.into_inner();
        // the time the sky is set to: the clock when it overrides, else the scene's, else the clock
        let effective_time = |running_clock: &RunningClock| {
            if running_clock.overrides_scene() {
                return running_clock.time;
            }
            skybox_time
                .map(|skybox_time| skybox_time.fixed_time as f32)
                .or(scene_time.map(|scene_time| scene_time.time))
                .unwrap_or(running_clock.time)
        };
        let old_time = effective_time(&running_clock);
        let was_override = running_clock.overrides_scene();
        if let Some(hours) = command.time {
            running_clock.time = (hours * ONE_HOUR) % TWENTY_FOUR_HOURS;
        }
        if command.resume {
            running_clock.speed = DEFAULT_CLOCK_SPEED;
        }
        if let Some(speed) = command.speed {
            running_clock.speed = speed;
        }

        // the clock is running again: hand the sky back to the scene's time
        if was_override && !running_clock.overrides_scene() {
            if skybox_time.is_some() {
                commands.trigger_targets(OnInsert, (time_keeper, *skybox_time_component_id));
            } else if scene_time.is_some() {
                commands.trigger_targets(OnInsert, (time_keeper, *scene_time_component_id));
            }
        }

        let new_time = effective_time(&running_clock);
        input.reply_ok(format!(
            "time {}:{} -> {}:{}, speed {} (elapsed: {})",
            (old_time as u32 / ONE_HOUR_U32),
            old_time as u32 % ONE_HOUR_U32 / 60,
            (new_time as u32 / ONE_HOUR_U32),
            new_time as u32 % ONE_HOUR_U32 / 60,
            running_clock.speed,
            new_time
        ));
    }
}

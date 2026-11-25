use bevy::{
    ecs::{entity::EntityHashSet, system::SystemParam},
    prelude::*,
};
use bevy_console::{AddConsoleCommand, ConsoleCommand};
use common::structs::{AppConfig, PrimaryPlayerRes, SceneTime, SceneTimeLock, TimeOfDay};
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
    /// Set by `/time --force`: the clock sets the sky whatever a scene asks for, until the next
    /// `/time` that sets a time or a speed. The scene editor pins its own lighting this way.
    pub forced: bool,
    /// The player set a time or a speed with "Allow overriding scene controls" on: the clock
    /// beats the time of the scene the player stands in, until a scene's time arrives again.
    pub player_holds: bool,
}

/// What sets the sky.
#[derive(Resource, Clone, Copy, PartialEq, Eq, Debug, Default)]
pub(crate) enum SkySource {
    /// A scene's SDK `SkyboxTime`.
    Skybox,
    /// A scene's `scene.json` fixed time.
    Scene,
    #[default]
    Clock,
}

impl SkySource {
    pub(crate) fn name(self) -> &'static str {
        match self {
            SkySource::Skybox => "skybox",
            SkySource::Scene => "scene",
            SkySource::Clock => "clock",
        }
    }
}

/// The time of the scene the player stands in wins over the clock and over a portable's time,
/// its SDK `SkyboxTime` before its `scene.json` time. With "Allow overriding scene controls" on
/// (`allow_override`), a time the player set after the scene's arrived beats it. A portable's
/// time comes next and wins only over a running clock: a time the player set by hand beats it.
/// A forced clock beats them all.
fn sky_source(
    clock: &RunningClock,
    skybox: Option<&SkyboxTimeSource>,
    scene: Option<&SceneTimeSource>,
    allow_override: bool,
) -> SkySource {
    if clock.forced {
        return SkySource::Clock;
    }
    let skybox_portable = skybox.map(|source| source.portable);
    let scene_portable = scene.map(|source| source.portable);
    let scene_holds = !(allow_override && clock.player_holds);
    let portables_hold = clock.speed != 0.0;
    if skybox_portable == Some(false) && scene_holds {
        SkySource::Skybox
    } else if scene_portable == Some(false) && scene_holds {
        SkySource::Scene
    } else if skybox_portable == Some(true) && portables_hold {
        SkySource::Skybox
    } else if scene_portable == Some(true) && portables_hold {
        SkySource::Scene
    } else {
        SkySource::Clock
    }
}

/// Whether the scene the player stands in holds the sky and the player may not take it. A
/// portable's time is no lock: stopping the clock beats it.
fn scene_locks(
    source: SkySource,
    skybox: Option<&SkyboxTimeSource>,
    scene: Option<&SceneTimeSource>,
    allow_override: bool,
) -> bool {
    if allow_override {
        return false;
    }
    match source {
        SkySource::Skybox => skybox.is_some_and(|source| !source.portable),
        SkySource::Scene => scene.is_some_and(|source| !source.portable),
        SkySource::Clock => false,
    }
}

/// Where the sky is headed: the source that holds it and, for a scene's, its time.
#[derive(Clone, Copy, PartialEq, Debug)]
enum SkyTarget {
    Skybox { time: f32, mode: TransitionMode },
    Scene { time: f32 },
    Clock,
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
struct SceneTimeSource {
    portable: bool,
    scene: Entity,
}

#[derive(Component, Deref)]
#[component(immutable)]
struct SkyboxTime(PbSkyboxTime);

impl From<PbSkyboxTime> for SkyboxTime {
    fn from(value: PbSkyboxTime) -> Self {
        Self(value)
    }
}

#[derive(Component)]
struct SkyboxTimeSource {
    portable: bool,
    scene: Entity,
}

pub struct DayNightPlugin;

impl Plugin for DayNightPlugin {
    fn build(&self, app: &mut App) {
        app.add_crdt_lww_component::<PbSkyboxTime, SkyboxTime>(
            SceneComponentId::SKYBOX_TIME,
            ComponentPosition::RootOnly,
        );

        app.init_resource::<SceneTimeLock>();
        app.init_resource::<SkySource>();
        app.add_systems(Startup, start_clock);
        app.add_systems(
            First,
            (
                (
                    update_running_clock,
                    (fetch_time_from_skybox, fetch_time_from_scene).chain(),
                ),
                follow_sky_source,
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
    commands.spawn((
        TimeKeeper,
        RunningClock {
            time,
            speed: DEFAULT_CLOCK_SPEED,
            forced: false,
            player_holds: false,
        },
    ));
}

/// Which scene a root-only component `C` comes from for the primary player: of the scenes
/// containing the player that have it, the parcel scene before portables, then the earliest
/// started.
#[derive(SystemParam)]
pub(crate) struct RootComponentSource<'w, 's, C: Component> {
    primary_player: Res<'w, PrimaryPlayerRes>,
    containing_scene: ContainingScene<'w, 's>,
    scenes: Query<'w, 's, (Entity, &'static RendererSceneContext), With<C>>,
    changed: Query<'w, 's, (), Changed<C>>,
    removed: RemovedComponents<'w, 's, C>,
    last_player_state: Local<'s, Option<Option<(IVec2, bool)>>>,
}

impl<C: Component> RootComponentSource<'_, '_, C> {
    /// The scene to read `C` from, or `None` while nothing it depends on has changed: the scene
    /// layout, the player's parcel, or a scene adding, changing or removing `C`.
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
            || !self.changed.is_empty()
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

fn fetch_time_from_scene(
    mut commands: Commands,
    mut source: RootComponentSource<SceneTime>,
    scenes: Query<(&SceneTime, &RendererSceneContext)>,
    time_keeper: Single<(Entity, &mut RunningClock, Option<&SceneTimeSource>), With<TimeKeeper>>,
) {
    // only recompute the time source when the scenes at the player position may have changed
    let Some(pick) = source.changed_pick() else {
        return;
    };
    let maybe_scene_time =
        pick.and_then(|entity| scenes.get(entity).ok().map(|time| (entity, time)));

    let (time_keeper, mut clock, scene_time_source) = time_keeper.into_inner();
    if let Some((entity, (scene_time, context))) = maybe_scene_time {
        if scene_time_source
            .filter(|source| source.scene == entity)
            .is_none()
        {
            debug!("Now using time from scene {}.", entity);
            commands.entity(time_keeper).try_insert((
                SceneTimeSource {
                    scene: entity,
                    portable: context.is_portable,
                },
                SceneTime {
                    time: scene_time.time,
                },
            ));
            if !context.is_portable {
                // the scene's time has arrived: it takes the sky from the player again
                clock.player_holds = false;
            }
        }
    } else if scene_time_source.is_some() {
        debug!("No longer using SceneTime.");
        commands
            .entity(time_keeper)
            .try_remove::<(SceneTime, SceneTimeSource)>();
    }
}

fn fetch_time_from_skybox(
    mut commands: Commands,
    mut source: RootComponentSource<SkyboxTime>,
    scenes: Query<(Ref<SkyboxTime>, &RendererSceneContext)>,
    time_keeper: Single<
        (
            Entity,
            &mut RunningClock,
            Option<&SkyboxTime>,
            Option<&SkyboxTimeSource>,
        ),
        With<TimeKeeper>,
    >,
) {
    // only recompute the time source when the scenes at the player position may have changed.
    // A scene's new value re-inserts its SkyboxTime, which marks it changed.
    let Some(pick) = source.changed_pick() else {
        return;
    };
    let maybe_skybox_time =
        pick.and_then(|entity| scenes.get(entity).ok().map(|time| (entity, time)));

    let (time_keeper, mut clock, time_keeper_skybox_time, skybox_time_source) =
        time_keeper.into_inner();
    if let Some((entity, (skybox_time, context))) = maybe_skybox_time {
        let new_scene = skybox_time_source
            .filter(|source| source.scene == entity)
            .is_none();
        let new_value = time_keeper_skybox_time.map(|time| &time.0) != Some(&skybox_time.0);
        if new_scene || new_value {
            debug!("Now using skybox time from scene {}.", entity);
            commands.entity(time_keeper).try_insert((
                SkyboxTimeSource {
                    scene: entity,
                    portable: context.is_portable,
                },
                SkyboxTime(PbSkyboxTime {
                    fixed_time: skybox_time.fixed_time,
                    transition_mode: skybox_time.transition_mode,
                }),
            ));
            if !context.is_portable {
                // the scene's time has arrived: it takes the sky from the player again
                clock.player_holds = false;
            }
        }
    } else if time_keeper_skybox_time.is_some() {
        debug!("No longer using SkyboxTime.");
        commands
            .entity(time_keeper)
            .try_remove::<(SkyboxTime, SkyboxTimeSource)>();
    }
}

/// Moves the sky to whatever holds it now. It runs on every change of the holder or of its
/// time: a scene's time arriving, changing or going away, the player's `/time`, the setting.
#[allow(clippy::type_complexity)]
fn follow_sky_source(
    mut commands: Commands,
    time_keeper: Single<
        (
            &mut RunningClock,
            Option<&SkyboxTime>,
            Option<&SkyboxTimeSource>,
            Option<&SceneTime>,
            Option<&SceneTimeSource>,
        ),
        With<TimeKeeper>,
    >,
    config: Res<AppConfig>,
    mut last_target: Local<Option<SkyTarget>>,
    mut time_of_day: ResMut<TimeOfDay>,
    mut lock: ResMut<SceneTimeLock>,
    mut holder: ResMut<SkySource>,
) {
    let (mut clock, skybox_time, skybox, scene_time, scene) = time_keeper.into_inner();
    let allow_override = config.allow_scene_control_override;
    if !allow_override && clock.player_holds {
        // turning the setting back on does not bring an old override back
        clock.player_holds = false;
    }

    let source = sky_source(&clock, skybox, scene, allow_override);
    holder.set_if_neq(source);
    lock.set_if_neq(SceneTimeLock(scene_locks(
        source,
        skybox,
        scene,
        allow_override,
    )));

    let target = match (source, skybox_time, scene_time) {
        (SkySource::Skybox, Some(skybox_time), _) => SkyTarget::Skybox {
            time: skybox_time.fixed_time as f32,
            mode: skybox_time.transition_mode(),
        },
        (SkySource::Scene, _, Some(scene_time)) => SkyTarget::Scene {
            time: scene_time.time,
        },
        _ => SkyTarget::Clock,
    };
    if *last_target == Some(target) {
        return;
    }
    *last_target = Some(target);

    let from = time_of_day.time;
    let (time, end) = match target {
        // a skip toward a scene's time must not finish under the clock, which starts its own
        SkyTarget::Clock => {
            commands.remove_resource::<TimeSkip>();
            return;
        }
        SkyTarget::Scene { time } => (
            time,
            if time > from {
                time
            } else {
                time + TWENTY_FOUR_HOURS
            },
        ),
        SkyTarget::Skybox { time, mode } => (
            time,
            match (time > from, mode) {
                (true, TransitionMode::TmForward) => time,
                (false, TransitionMode::TmForward) => time + TWENTY_FOUR_HOURS,
                (true, TransitionMode::TmBackward) => time - TWENTY_FOUR_HOURS,
                (false, TransitionMode::TmBackward) => time,
            },
        ),
    };
    if (time - from).abs() < ONE_HOUR {
        debug!("The sky moves to {time}.");
        time_of_day.time = time;
        commands.remove_resource::<TimeSkip>();
    } else {
        debug!("Starting a TimeSkip from {from} to {end}.");
        commands.insert_resource(TimeSkip {
            start: from,
            end,
            progress: 0.,
            easing: EaseFunction::SmoothStep,
        });
    }
}

fn update_running_clock(
    mut time_keeper: Single<&mut RunningClock, With<TimeKeeper>>,
    time: Res<Time>,
    testing: Res<TestingData>,
) {
    // The test runner's speed must not change the sun between captures. Keep
    // scene-authored time overrides active; only the background clock is frozen.
    if testing.test_scenes.is_some() {
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

fn push_time_of_day_from_running_clock(
    mut commands: Commands,
    time_keeper: Single<&RunningClock, With<TimeKeeper>>,
    holder: Res<SkySource>,
    mut time_of_day: ResMut<TimeOfDay>,
) {
    if *holder != SkySource::Clock {
        return;
    }
    let running_clock = time_keeper.into_inner();
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

#[derive(clap::Parser, ConsoleCommand)]
#[command(name = "/time")]
pub struct TimeOfDayConsoleCommand {
    /// Resume the normal day cycle.
    #[arg(long, conflicts_with_all = ["time", "speed", "force"])]
    pub resume: bool,
    /// The time set here beats a scene's time, until the next `/time` that sets a time or a
    /// speed. The scene editor's lighting uses it.
    #[arg(long, requires = "time")]
    pub force: bool,
    pub time: Option<f32>,
    pub speed: Option<f32>,
}

#[allow(clippy::type_complexity)]
fn timeofday_console_command(
    mut input: ConsoleCommand<TimeOfDayConsoleCommand>,
    time_keeper: Single<
        (
            &mut RunningClock,
            Option<&SceneTime>,
            Option<&SceneTimeSource>,
            Option<&SkyboxTime>,
            Option<&SkyboxTimeSource>,
        ),
        With<TimeKeeper>,
    >,
    config: Res<AppConfig>,
) {
    if let Some(Ok(command)) = input.take() {
        let (mut running_clock, scene_time, scene_source, skybox_time, skybox_source) =
            time_keeper.into_inner();
        let allow_override = config.allow_scene_control_override;
        let source = |running_clock: &RunningClock| {
            sky_source(running_clock, skybox_source, scene_source, allow_override)
        };
        // the time the sky is set to
        let effective_time = |running_clock: &RunningClock| {
            match source(running_clock) {
                SkySource::Skybox => skybox_time.map(|skybox_time| skybox_time.fixed_time as f32),
                SkySource::Scene => scene_time.map(|scene_time| scene_time.time),
                SkySource::Clock => None,
            }
            .unwrap_or(running_clock.time)
        };
        let old_time = effective_time(&running_clock);
        if let Some(hours) = command.time {
            running_clock.time = (hours * ONE_HOUR) % TWENTY_FOUR_HOURS;
        }
        if command.resume {
            running_clock.speed = DEFAULT_CLOCK_SPEED;
        }
        if let Some(speed) = command.speed {
            running_clock.speed = speed;
        }
        // a bare `/time` only reads
        if command.time.is_some() || command.speed.is_some() || command.resume {
            running_clock.forced = command.force;
            // `--resume` hands the sky back to the scene
            running_clock.player_holds = allow_override && !command.resume;
        }

        // `follow_sky_source` moves the sky if this changed what holds it
        let new_time = effective_time(&running_clock);
        let locked = scene_locks(
            source(&running_clock),
            skybox_source,
            scene_source,
            allow_override,
        );
        input.reply_ok(format!(
            "time {}:{} -> {}:{}, speed {} (elapsed: {}), locked: {}",
            (old_time as u32 / ONE_HOUR_U32),
            old_time as u32 % ONE_HOUR_U32 / 60,
            (new_time as u32 / ONE_HOUR_U32),
            new_time as u32 % ONE_HOUR_U32 / 60,
            running_clock.speed,
            new_time,
            locked
        ));
    }
}

#[cfg(test)]
mod tests {
    use std::{
        sync::{mpsc, Arc},
        time::Duration,
    };

    use bevy::platform::collections::HashSet;
    use bevy_console::{ConsoleCommandEntered, PrintConsoleLine};
    use common::structs::PrimaryUser;
    use dcl_types::SceneId;
    use scene_runner::initialize_scene::{
        LiveScenes, PointerResult, PortableKind, PortableScenes, PortableSource, ScenePointers,
    };

    use super::*;

    const NOON: f32 = 12. * ONE_HOUR;
    const NIGHT: f32 = 22. * ONE_HOUR;
    const PARCEL_SCENE: &str = "parcel-scene";
    const PORTABLE: &str = "portable";

    /// The day's systems in their order, with the player on parcel 0,0 and no scene yet. The
    /// clock is stopped at noon by default so that only the tests' own changes move it.
    fn app() -> App {
        let mut app = App::new();
        app.init_resource::<ScenePointers>()
            .init_resource::<LiveScenes>()
            .init_resource::<PortableScenes>()
            .init_resource::<SceneTimeLock>()
            .init_resource::<SkySource>()
            .init_resource::<AppConfig>()
            .init_resource::<TestingData>()
            .init_resource::<Time>()
            .add_event::<ConsoleCommandEntered>()
            .add_event::<PrintConsoleLine>()
            .add_systems(Startup, start_clock)
            .add_systems(
                Update,
                (
                    timeofday_console_command,
                    update_running_clock,
                    (fetch_time_from_skybox, fetch_time_from_scene).chain(),
                    follow_sky_source,
                    push_time_of_day_from_time_skip.run_if(resource_exists::<TimeSkip>),
                    push_time_of_day_from_running_clock.run_if(not(resource_exists::<TimeSkip>)),
                )
                    .chain(),
            );
        let player = app
            .world_mut()
            .spawn((
                PrimaryUser::default(),
                GlobalTransform::from_xyz(8.0, 0.0, -8.0),
            ))
            .id();
        app.insert_resource(PrimaryPlayerRes(player));
        app.update();
        time(&mut app, "12 0");
        settle(&mut app);
        assert_eq!(sky(&app), NOON);
        app
    }

    fn scene(app: &mut App, hash: &str, portable: bool) -> Entity {
        let scene = app.world_mut().spawn_empty().id();
        let context = RendererSceneContext::new(
            SceneId::DUMMY,
            hash.to_owned(),
            "storage_root".to_owned(),
            portable,
            0,
            "title".to_owned(),
            IVec2::ZERO,
            HashSet::from_iter([IVec2::ZERO]),
            vec![],
            Default::default(),
            vec![],
            scene,
            0.0,
            false,
            "sdk_version",
            false,
            false,
        );
        app.world_mut().entity_mut(scene).insert(context);
        app.world_mut()
            .resource_mut::<LiveScenes>()
            .scenes
            .insert(hash.to_owned(), scene);
        scene
    }

    /// The scene under the player's parcel.
    fn parcel_scene(app: &mut App) -> Entity {
        let scene = scene(app, PARCEL_SCENE, false);
        app.world_mut().resource_mut::<ScenePointers>().insert(
            IVec2::ZERO,
            PointerResult::Exists {
                hash: PARCEL_SCENE.to_owned(),
                urn: None,
            },
        );
        scene
    }

    /// A scene that follows the player.
    fn portable(app: &mut App) -> Entity {
        let scene = scene(app, PORTABLE, true);
        app.world_mut().resource_mut::<PortableScenes>().insert(
            PORTABLE.to_owned(),
            PortableSource {
                pid: PORTABLE.to_owned(),
                parent_scene: None,
                ens: None,
                super_user: false,
                kind: PortableKind::Startup,
            },
        );
        scene
    }

    fn skybox_time(seconds: f32) -> SkyboxTime {
        SkyboxTime(PbSkyboxTime {
            fixed_time: seconds as u32,
            transition_mode: None,
        })
    }

    /// Runs `/time <args>` and returns its reply.
    fn time(app: &mut App, args: &str) -> String {
        let (tx, rx) = mpsc::channel();
        app.world_mut().send_event(ConsoleCommandEntered {
            command_name: "/time".into(),
            args: args.split_whitespace().map(str::to_owned).collect(),
            responder: Some(Arc::new(move |result| tx.send(result).unwrap())),
        });
        app.update();
        rx.try_recv().unwrap().unwrap()
    }

    /// Runs frames of half a second until no skip is under way.
    fn settle(app: &mut App) {
        for _ in 0..40 {
            app.world_mut()
                .resource_mut::<Time>()
                .advance_by(Duration::from_millis(500));
            app.update();
            if !app.world().contains_resource::<TimeSkip>() {
                app.update();
                return;
            }
        }
        panic!("the sky never settled");
    }

    /// Makes the next frames a millisecond long, so that a skip stays under way across them.
    fn slow_frames(app: &mut App) {
        app.world_mut()
            .resource_mut::<Time>()
            .advance_by(Duration::from_millis(1));
    }

    fn sky(app: &App) -> f32 {
        app.world().resource::<TimeOfDay>().time
    }

    fn locked(app: &App) -> bool {
        app.world().resource::<SceneTimeLock>().0
    }

    fn elapsed(reply: &str) -> f32 {
        reply
            .rsplit_once("(elapsed: ")
            .and_then(|(_, rest)| rest.split_once(')'))
            .unwrap()
            .0
            .parse()
            .unwrap()
    }

    /// Turns "Allow overriding scene controls" on or off.
    fn allow_override(app: &mut App, allow: bool) {
        app.world_mut()
            .resource_mut::<AppConfig>()
            .allow_scene_control_override = allow;
    }

    /// Puts the player on the parcel scene's parcel, or five parcels away from it.
    fn stand(app: &mut App, in_scene: bool) {
        let player = app.world().resource::<PrimaryPlayerRes>().0;
        let x = if in_scene { 8.0 } else { 8.0 + 16.0 * 5.0 };
        app.world_mut()
            .entity_mut(player)
            .insert(GlobalTransform::from_xyz(x, 0.0, -8.0));
    }

    #[test]
    fn with_no_scene_the_clock_sets_the_sky_and_nothing_is_locked() {
        let mut app = app();
        assert!(!locked(&app));
        let reply = time(&mut app, "");
        assert_eq!(elapsed(&reply), NOON);
        assert!(reply.ends_with(", locked: false"));
    }

    #[test]
    fn the_parcel_scenes_time_beats_a_running_and_a_stopped_clock() {
        let mut app = app();
        let scene = parcel_scene(&mut app);
        app.world_mut()
            .entity_mut(scene)
            .insert(SceneTime { time: NIGHT });
        settle(&mut app);
        assert_eq!(sky(&app), NIGHT);
        assert!(locked(&app));

        // stopped at another hour
        let reply = time(&mut app, "6 0");
        settle(&mut app);
        assert_eq!(sky(&app), NIGHT);
        assert_eq!(
            elapsed(&reply),
            NIGHT,
            "/time reports the time the sky shows"
        );
        assert!(reply.ends_with(", locked: true"));

        // running
        time(&mut app, "6 12");
        settle(&mut app);
        assert_eq!(sky(&app), NIGHT);
        assert!(locked(&app));
    }

    #[test]
    fn the_parcel_scenes_skybox_time_beats_the_clock() {
        let mut app = app();
        let scene = parcel_scene(&mut app);
        app.world_mut().entity_mut(scene).insert(skybox_time(NIGHT));
        settle(&mut app);
        assert_eq!(sky(&app), NIGHT);
        assert!(locked(&app));
        time(&mut app, "6 0");
        settle(&mut app);
        assert_eq!(sky(&app), NIGHT);
    }

    #[test]
    fn a_time_set_during_the_skip_to_the_scenes_time_does_not_stop_it() {
        let mut app = app();
        let scene = parcel_scene(&mut app);
        app.world_mut()
            .entity_mut(scene)
            .insert(SceneTime { time: NIGHT });
        slow_frames(&mut app);
        app.update();
        assert!(app.world().contains_resource::<TimeSkip>());
        time(&mut app, "3 0");
        assert!(app.world().contains_resource::<TimeSkip>());
        settle(&mut app);
        assert_eq!(sky(&app), NIGHT);
    }

    #[test]
    fn leaving_the_scene_with_a_stopped_clock_shows_the_clock_and_unlocks() {
        let mut app = app();
        let scene = parcel_scene(&mut app);
        app.world_mut()
            .entity_mut(scene)
            .insert(SceneTime { time: NIGHT });
        settle(&mut app);
        time(&mut app, "6 0");
        settle(&mut app);
        assert!(locked(&app));

        stand(&mut app, false);
        settle(&mut app);
        assert_eq!(sky(&app), 6. * ONE_HOUR);
        assert!(!locked(&app));

        // and back in
        stand(&mut app, true);
        settle(&mut app);
        assert_eq!(sky(&app), NIGHT);
        assert!(locked(&app));
    }

    #[test]
    fn a_forced_time_beats_the_scene_until_the_next_time_that_is_set() {
        let mut app = app();
        let scene = parcel_scene(&mut app);
        app.world_mut()
            .entity_mut(scene)
            .insert(SceneTime { time: NIGHT });
        settle(&mut app);

        let reply = time(&mut app, "12 0 --force");
        settle(&mut app);
        assert_eq!(sky(&app), NOON);
        assert_eq!(elapsed(&reply), NOON);
        assert!(!locked(&app));

        // reading the time leaves it forced
        assert_eq!(elapsed(&time(&mut app, "")), NOON);
        settle(&mut app);
        assert_eq!(sky(&app), NOON);

        // a time set without the flag hands the sky back
        time(&mut app, "6 0");
        settle(&mut app);
        assert_eq!(sky(&app), NIGHT);
        assert!(locked(&app));

        time(&mut app, "12 0 --force");
        settle(&mut app);
        assert_eq!(sky(&app), NOON);
        time(&mut app, "--resume");
        settle(&mut app);
        assert_eq!(sky(&app), NIGHT);
        assert!(locked(&app));
    }

    #[test]
    fn forcing_a_time_drops_the_skip_toward_the_scenes_time() {
        let mut app = app();
        let scene = parcel_scene(&mut app);
        app.world_mut()
            .entity_mut(scene)
            .insert(SceneTime { time: NIGHT });
        slow_frames(&mut app);
        app.update();
        assert!(app.world().contains_resource::<TimeSkip>());
        time(&mut app, "12 0 --force");
        settle(&mut app);
        assert_eq!(sky(&app), NOON);
    }

    #[test]
    fn force_and_resume_do_not_go_together() {
        let mut app = app();
        let (tx, rx) = mpsc::channel();
        app.world_mut().send_event(ConsoleCommandEntered {
            command_name: "/time".into(),
            args: vec!["--resume".into(), "--force".into()],
            responder: Some(Arc::new(move |result| tx.send(result).unwrap())),
        });
        app.update();
        assert!(rx.try_recv().unwrap().is_err());
    }

    #[test]
    fn a_portables_time_beats_a_running_clock_only_and_never_locks() {
        let mut app = app();
        time(&mut app, "12 12");
        let scene = portable(&mut app);
        app.world_mut().entity_mut(scene).insert(skybox_time(NIGHT));
        settle(&mut app);
        assert_eq!(sky(&app), NIGHT);
        assert!(!locked(&app));

        // a time the player sets by hand beats it
        time(&mut app, "6 0");
        settle(&mut app);
        assert_eq!(sky(&app), 6. * ONE_HOUR);
        assert!(!locked(&app));

        // and the running clock hands the sky back to it
        time(&mut app, "6 12");
        settle(&mut app);
        assert_eq!(sky(&app), NIGHT);
    }

    #[test]
    fn the_parcel_scenes_time_wins_over_a_portables_under_a_running_clock() {
        let mut app = app();
        time(&mut app, "12 12");
        let follower = portable(&mut app);
        app.world_mut()
            .entity_mut(follower)
            .insert(skybox_time(18. * ONE_HOUR));
        let scene = parcel_scene(&mut app);
        app.world_mut()
            .entity_mut(scene)
            .insert(SceneTime { time: NIGHT });
        settle(&mut app);
        assert_eq!(sky(&app), NIGHT);
        assert!(locked(&app));

        // stopping the clock changes nothing: the scene held the sky already
        time(&mut app, "6 0");
        settle(&mut app);
        assert_eq!(sky(&app), NIGHT);
        assert!(locked(&app));
    }

    #[test]
    fn force_needs_a_time() {
        let mut app = app();
        let (tx, rx) = mpsc::channel();
        app.world_mut().send_event(ConsoleCommandEntered {
            command_name: "/time".into(),
            args: vec!["--force".into()],
            responder: Some(Arc::new(move |result| tx.send(result).unwrap())),
        });
        app.update();
        assert!(rx.try_recv().unwrap().is_err());
    }

    #[test]
    fn the_parcel_scenes_time_wins_over_a_portables_under_a_stopped_clock() {
        let mut app = app();
        let follower = portable(&mut app);
        app.world_mut()
            .entity_mut(follower)
            .insert(skybox_time(18. * ONE_HOUR));
        let scene = parcel_scene(&mut app);
        app.world_mut()
            .entity_mut(scene)
            .insert(SceneTime { time: NIGHT });
        settle(&mut app);
        // the clock is stopped, so the portable's time is out and the scene's stays
        assert_eq!(sky(&app), NIGHT);
        assert!(locked(&app));
    }

    #[test]
    fn a_portable_takes_the_sky_when_the_parcel_scenes_time_goes() {
        let mut app = app();
        time(&mut app, "12 12");
        let follower = portable(&mut app);
        app.world_mut()
            .entity_mut(follower)
            .insert(skybox_time(18. * ONE_HOUR));
        let scene = parcel_scene(&mut app);
        app.world_mut()
            .entity_mut(scene)
            .insert(SceneTime { time: NIGHT });
        settle(&mut app);
        assert_eq!(sky(&app), NIGHT);

        app.world_mut().entity_mut(scene).remove::<SceneTime>();
        settle(&mut app);
        assert_eq!(sky(&app), 18. * ONE_HOUR);
        assert!(!locked(&app));
    }

    #[test]
    fn with_overriding_allowed_a_time_set_after_the_scenes_wins_until_the_scenes_arrives_again() {
        let mut app = app();
        allow_override(&mut app, true);
        // a time set before the scene's arrives does not hold
        time(&mut app, "6 0");
        let scene = parcel_scene(&mut app);
        app.world_mut()
            .entity_mut(scene)
            .insert(SceneTime { time: NIGHT });
        settle(&mut app);
        assert_eq!(sky(&app), NIGHT);
        assert!(!locked(&app), "the menu stays usable");

        let reply = time(&mut app, "3 0");
        settle(&mut app);
        assert_eq!(sky(&app), 3. * ONE_HOUR);
        assert_eq!(elapsed(&reply), 3. * ONE_HOUR);
        assert!(reply.ends_with(", locked: false"));

        // the override is not kept for the scene: its time is back on return
        stand(&mut app, false);
        settle(&mut app);
        assert_eq!(sky(&app), 3. * ONE_HOUR);
        stand(&mut app, true);
        settle(&mut app);
        assert_eq!(sky(&app), NIGHT);
    }

    #[test]
    fn with_overriding_allowed_a_new_skybox_time_takes_the_sky_back() {
        let mut app = app();
        allow_override(&mut app, true);
        let scene = parcel_scene(&mut app);
        app.world_mut().entity_mut(scene).insert(skybox_time(NIGHT));
        settle(&mut app);
        time(&mut app, "3 0");
        settle(&mut app);
        assert_eq!(sky(&app), 3. * ONE_HOUR);

        app.world_mut()
            .entity_mut(scene)
            .insert(skybox_time(18. * ONE_HOUR));
        settle(&mut app);
        assert_eq!(sky(&app), 18. * ONE_HOUR);
    }

    #[test]
    fn resume_hands_an_overridden_sky_back_to_the_scene() {
        let mut app = app();
        allow_override(&mut app, true);
        let scene = parcel_scene(&mut app);
        app.world_mut()
            .entity_mut(scene)
            .insert(SceneTime { time: NIGHT });
        settle(&mut app);
        time(&mut app, "3 0");
        settle(&mut app);
        assert_eq!(sky(&app), 3. * ONE_HOUR);
        time(&mut app, "--resume");
        settle(&mut app);
        assert_eq!(sky(&app), NIGHT);
    }

    #[test]
    fn turning_the_setting_off_ends_the_override_for_good() {
        let mut app = app();
        allow_override(&mut app, true);
        let scene = parcel_scene(&mut app);
        app.world_mut()
            .entity_mut(scene)
            .insert(SceneTime { time: NIGHT });
        settle(&mut app);
        time(&mut app, "3 0");
        settle(&mut app);
        assert_eq!(sky(&app), 3. * ONE_HOUR);

        allow_override(&mut app, false);
        settle(&mut app);
        assert_eq!(sky(&app), NIGHT);
        assert!(locked(&app));

        allow_override(&mut app, true);
        settle(&mut app);
        assert_eq!(sky(&app), NIGHT);
        assert!(!locked(&app));
    }
}

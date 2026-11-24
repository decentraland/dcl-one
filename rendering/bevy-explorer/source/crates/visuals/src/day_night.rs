use bevy::{
    ecs::{component::ComponentIdFor, entity::EntityHashSet, system::SystemParam},
    prelude::*,
};
use bevy_console::{AddConsoleCommand, ConsoleCommand};
use common::structs::{PrimaryPlayerRes, SceneTime, SceneTimeLock, TimeOfDay};
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
}

/// What sets the sky.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum SkySource {
    /// A scene's SDK `SkyboxTime`.
    Skybox,
    /// A scene's `scene.json` fixed time.
    Scene,
    Clock,
}

/// The time of the scene the player stands in always wins over the clock. A portable's time
/// wins only over a running clock: a time the player set by hand beats it. A forced clock beats
/// both.
fn sky_source(
    clock: &RunningClock,
    skybox: Option<&SkyboxTimeSource>,
    has_scene_time: bool,
    scene: Option<&SceneTimeSource>,
) -> SkySource {
    let holds = |portable: bool| !clock.forced && !(portable && clock.speed == 0.0);
    if skybox.is_some_and(|source| holds(source.portable)) {
        SkySource::Skybox
    } else if has_scene_time && holds(scene.is_some_and(|source| source.portable)) {
        SkySource::Scene
    } else {
        SkySource::Clock
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
struct SceneTimeSource {
    scene: Entity,
    portable: bool,
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
    scene: Entity,
    portable: bool,
}

pub struct DayNightPlugin;

impl Plugin for DayNightPlugin {
    fn build(&self, app: &mut App) {
        app.add_crdt_lww_component::<PbSkyboxTime, SkyboxTime>(
            SceneComponentId::SKYBOX_TIME,
            ComponentPosition::RootOnly,
        );

        app.init_resource::<SceneTimeLock>();
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
                publish_scene_time_lock,
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
                forced: false,
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

fn fetch_time_from_scene(
    mut commands: Commands,
    mut source: RootComponentSource<SceneTime>,
    scenes: Query<(&SceneTime, &RendererSceneContext)>,
    time_keeper: Single<(Entity, Option<&SceneTime>, Option<&SceneTimeSource>), With<TimeKeeper>>,
) {
    // only recompute the time source when the scenes at the player position may have changed
    let Some(pick) = source.changed_pick() else {
        return;
    };
    let maybe_scene_time =
        pick.and_then(|entity| scenes.get(entity).ok().map(|time| (entity, time)));

    let (time_keeper, time_keeper_scene_time, scene_time_source) = time_keeper.into_inner();
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
        }
    } else if time_keeper_scene_time.is_some() {
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
    if let Some((entity, (skybox_time, context))) = maybe_skybox_time {
        let source = || SkyboxTimeSource {
            scene: entity,
            portable: context.is_portable,
        };
        if skybox_time_source
            .filter(|source| source.scene == entity)
            .is_none()
        {
            debug!("Now using skybox time from scene {}.", entity);
            commands.entity(time_keeper).try_insert((
                source(),
                SkyboxTime(PbSkyboxTime {
                    fixed_time: skybox_time.fixed_time,
                    transition_mode: skybox_time.transition_mode,
                }),
            ));
        } else if skybox_time.is_changed() {
            debug!("New skybox time from scene {}.", entity);
            commands.entity(time_keeper).try_insert((
                source(),
                SkyboxTime(PbSkyboxTime {
                    fixed_time: skybox_time.fixed_time,
                    transition_mode: skybox_time.transition_mode,
                }),
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

fn publish_scene_time_lock(
    time_keeper: Single<
        (
            &RunningClock,
            Option<&SkyboxTimeSource>,
            Has<SceneTime>,
            Option<&SceneTimeSource>,
        ),
        With<TimeKeeper>,
    >,
    mut lock: ResMut<SceneTimeLock>,
) {
    let (clock, skybox, has_scene_time, scene) = *time_keeper;
    // a portable's time is no lock: stopping the clock beats it
    let locked = match sky_source(clock, skybox, has_scene_time, scene) {
        SkySource::Skybox => skybox.is_some_and(|source| !source.portable),
        SkySource::Scene => scene.is_some_and(|source| !source.portable),
        SkySource::Clock => false,
    };
    if lock.0 != locked {
        lock.0 = locked;
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
    time_keeper: Single<
        (
            &RunningClock,
            Option<&SkyboxTimeSource>,
            Has<SceneTime>,
            Option<&SceneTimeSource>,
        ),
        With<TimeKeeper>,
    >,
    mut time_of_day: ResMut<TimeOfDay>,
) {
    let (running_clock, skybox, has_scene_time, scene) = time_keeper.into_inner();
    if sky_source(running_clock, skybox, has_scene_time, scene) != SkySource::Clock {
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

fn check_new_scene_time(
    trigger: Trigger<OnInsert, SceneTime>,
    mut commands: Commands,
    time_keeper: Single<
        (
            &SceneTime,
            &RunningClock,
            Option<&SkyboxTimeSource>,
            Option<&SceneTimeSource>,
        ),
        With<TimeKeeper>,
    >,
    mut time_of_day: ResMut<TimeOfDay>,
    time_skip: Option<Res<TimeSkip>>,
) {
    let (scene_time, running_clock, skybox, scene) = time_keeper.into_inner();
    if sky_source(running_clock, skybox, true, scene) != SkySource::Scene {
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
    time_keeper: Single<
        (
            &SkyboxTime,
            &RunningClock,
            Option<&SkyboxTimeSource>,
            Has<SceneTime>,
            Option<&SceneTimeSource>,
        ),
        With<TimeKeeper>,
    >,
    mut time_of_day: ResMut<TimeOfDay>,
    time_skip: Option<Res<TimeSkip>>,
) {
    let (skybox_time, running_clock, skybox, has_scene_time, scene) = time_keeper.into_inner();
    if sky_source(running_clock, skybox, has_scene_time, scene) != SkySource::Skybox {
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
    /// Resume the normal day cycle.
    #[arg(long, conflicts_with_all = ["time", "speed", "force"])]
    pub resume: bool,
    /// The time set here beats a scene's time, until the next `/time` that sets a time or a
    /// speed. The scene editor's lighting uses it.
    #[arg(long)]
    pub force: bool,
    pub time: Option<f32>,
    pub speed: Option<f32>,
}

fn timeofday_console_command(
    mut input: ConsoleCommand<TimeOfDayConsoleCommand>,
    mut commands: Commands,
    time_keeper: Single<
        (
            Entity,
            &mut RunningClock,
            Option<&SceneTime>,
            Option<&SceneTimeSource>,
            Option<&SkyboxTime>,
            Option<&SkyboxTimeSource>,
        ),
        With<TimeKeeper>,
    >,
    scene_time_component_id: ComponentIdFor<SceneTime>,
    skybox_time_component_id: ComponentIdFor<SkyboxTime>,
) {
    if let Some(Ok(command)) = input.take() {
        let (time_keeper, mut running_clock, scene_time, scene_source, skybox_time, skybox_source) =
            time_keeper.into_inner();
        let source = |running_clock: &RunningClock| {
            sky_source(
                running_clock,
                skybox_source,
                scene_time.is_some(),
                scene_source,
            )
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
        let old_source = source(&running_clock);
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
        }

        let new_source = source(&running_clock);
        if new_source != old_source {
            match new_source {
                // the sky is the scene's again: move to its time
                SkySource::Skybox => {
                    commands.trigger_targets(OnInsert, (time_keeper, *skybox_time_component_id))
                }
                SkySource::Scene => {
                    commands.trigger_targets(OnInsert, (time_keeper, *scene_time_component_id))
                }
                // a skip toward the scene's time must not finish under the clock
                SkySource::Clock => commands.remove_resource::<TimeSkip>(),
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
                    push_time_of_day_from_time_skip.run_if(resource_exists::<TimeSkip>),
                    push_time_of_day_from_running_clock.run_if(not(resource_exists::<TimeSkip>)),
                    publish_scene_time_lock,
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
            .and_then(|(_, rest)| rest.strip_suffix(')'))
            .unwrap()
            .parse()
            .unwrap()
    }

    #[test]
    fn with_no_scene_the_clock_sets_the_sky_and_nothing_is_locked() {
        let mut app = app();
        // the keeper starts with a placeholder scene time that no scene gave
        assert!(!locked(&app));
        assert_eq!(elapsed(&time(&mut app, "")), NOON);
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

        let player = app.world().resource::<PrimaryPlayerRes>().0;
        app.world_mut()
            .entity_mut(player)
            .insert(GlobalTransform::from_xyz(8.0 + 16.0 * 5.0, 0.0, -8.0));
        settle(&mut app);
        assert_eq!(sky(&app), 6. * ONE_HOUR);
        assert!(!locked(&app));

        // and back in
        app.world_mut()
            .entity_mut(player)
            .insert(GlobalTransform::from_xyz(8.0, 0.0, -8.0));
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
}

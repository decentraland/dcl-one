use std::time::Duration;

use bevy::{
    animation::{animated_field, AnimationPlugin, AnimationTarget, AnimationTargetId},
    scene::{ScenePlugin, SceneSpawner},
    time::TimeUpdateStrategy,
};
use console::ConsolePlugin;
use dcl_component::{ComponentNameRegistry, DclReader, SceneEntityId};
use dcl_types::{interface::CrdtStore, SceneId};
use prost::Message;

use super::*;
use crate::{
    update_world::{
        gltf_container::GltfProcessed, CrdtExtractors, CrdtStateComponent, TrackComponents,
    },
    DeletedSceneEntities, RendererSceneContext, SceneLoopLabel, SceneLoopSchedule,
};

fn send_animator(app: &mut App, root: Entity, id: SceneEntityId, message: Option<&PbAnimator>) {
    let bytes = message.map(Message::encode_to_vec);
    let mut store = CrdtStore::default();
    store.force_update(
        SceneComponentId::ANIMATOR,
        CrdtType::LWW_ANY,
        id,
        bytes.as_deref().map(DclReader::new).as_mut(),
    );
    let updates = store.lww.remove(&SceneComponentId::ANIMATOR).unwrap();
    app.world_mut()
        .entity_mut(root)
        .insert(CrdtStateComponent::<_, PbAnimator>::new(updates));
    app.world_mut()
        .resource_scope(|world, mut scene_loop: Mut<SceneLoopSchedule>| {
            scene_loop.schedule.run(world);
        });
}

fn scene_fixture() -> (App, Entity, SceneEntityId, Entity) {
    let mut app = App::new();
    app.add_plugins((
        MinimalPlugins,
        AssetPlugin::default(),
        ScenePlugin,
        AnimationPlugin,
        ConsolePlugin {
            add_bevy_console: false,
        },
    ))
    .init_resource::<CrdtExtractors>()
    .init_resource::<ComponentNameRegistry>()
    .init_resource::<TrackComponents>()
    .insert_resource(SceneLoopSchedule {
        schedule: Schedule::new(SceneLoopLabel),
    })
    .add_plugins(AnimatorPlugin)
    .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_millis(
        100,
    )));

    let root = app.world_mut().spawn_empty().id();
    let id = SceneEntityId::new(512, 0);
    let container = app
        .world_mut()
        .spawn(SceneEntity {
            root,
            scene_id: SceneId(root),
            id,
        })
        .id();
    let mut context = RendererSceneContext::new(
        SceneId(root),
        "scene".into(),
        String::new(),
        false,
        0,
        String::new(),
        IVec2::ZERO,
        Default::default(),
        vec![],
        Default::default(),
        vec![],
        root,
        0.0,
        false,
        "",
        false,
        false,
    );
    context.associate_bevy_entity(id, container);
    app.world_mut()
        .entity_mut(root)
        .insert((context, DeletedSceneEntities::default()));
    (app, root, id, container)
}

fn load_model(app: &mut App, container: Entity) -> Entity {
    let mut world = World::new();
    world.spawn_empty();
    let model = app
        .world_mut()
        .resource_mut::<Assets<Scene>>()
        .add(Scene::new(world));
    let instance = app
        .world_mut()
        .resource_mut::<SceneSpawner>()
        .spawn_as_child(model, container);
    app.update();
    let bone = app
        .world()
        .resource::<SceneSpawner>()
        .iter_instance_entities(instance)
        .next()
        .unwrap();
    let target = AnimationTargetId::from_name(&Name::new("Bone"));
    app.world_mut().entity_mut(bone).insert((
        Transform::default(),
        AnimationTarget {
            id: target,
            player: container,
        },
    ));

    // The default and controlled clips drive different properties of the same bone.
    let curve = UnevenSampleAutoCurve::new([(0.0, Vec3::ONE), (1.0, Vec3::splat(2.0))]).unwrap();
    let mut first = AnimationClip::default();
    first.add_curve_to_target(
        target,
        AnimatableCurve::new(animated_field!(Transform::translation), curve.clone()),
    );
    let mut swim = AnimationClip::default();
    swim.add_curve_to_target(
        target,
        AnimatableCurve::new(animated_field!(Transform::scale), curve),
    );
    let mut assets = app.world_mut().resource_mut::<Assets<AnimationClip>>();
    let first = assets.add(first);
    let swim = assets.add(swim);
    let mut graph = AnimationGraph::new();
    let default = graph.add_clip(first.clone(), 1.0, graph.root);
    let bite = graph.add_clip(first, 1.0, graph.root);
    let swim = graph.add_clip(swim, 1.0, graph.root);
    let graph = app
        .world_mut()
        .resource_mut::<Assets<AnimationGraph>>()
        .add(graph);
    app.world_mut().entity_mut(container).insert((
        GltfReady(instance),
        GltfProcessed {
            instance_id: Some(instance),
            ..Default::default()
        },
        Clips {
            default: Some(default),
            named: HashMap::from_iter([
                ("swim".to_owned(), (swim, 1.0)),
                ("bite".to_owned(), (bite, 1.0)),
            ]),
        },
        AnimationGraphHandle(graph),
        AnimationPlayer::default(),
    ));
    bone
}

fn playing(clip: &str) -> PbAnimationState {
    PbAnimationState {
        clip: clip.into(),
        playing: Some(true),
        ..Default::default()
    }
}

fn swim_animator() -> PbAnimator {
    PbAnimator {
        states: vec![playing("swim")],
    }
}

fn swim_once() -> PbAnimator {
    PbAnimator {
        states: vec![PbAnimationState {
            r#loop: Some(false),
            ..playing("swim")
        }],
    }
}

/// The Animator the scene has been sent, with its timestamp.
fn reported(app: &App, root: Entity, id: SceneEntityId) -> Option<(u32, PbAnimator)> {
    let context = app.world().get::<RendererSceneContext>(root).unwrap();
    let entry = context
        .crdt_store
        .lww
        .get(&SceneComponentId::ANIMATOR)?
        .last_write
        .get(&id)?;
    Some((
        entry.timestamp.0,
        PbAnimator::decode(entry.data.as_slice()).unwrap(),
    ))
}

fn scale(app: &App, bone: Entity) -> f32 {
    app.world().get::<Transform>(bone).unwrap().scale.x
}

#[test]
fn sdk_animator_before_model_load_samples_the_default_pose_and_plays_swim() {
    let (mut app, root, id, container) = scene_fixture();
    send_animator(&mut app, root, id, Some(&swim_animator()));
    app.update();

    let bone = load_model(&mut app, container);
    assert_eq!(
        app.world().get::<Transform>(bone).unwrap().translation,
        Vec3::ZERO
    );
    app.update();
    let first = *app.world().get::<Transform>(bone).unwrap();
    assert_eq!(first.translation, Vec3::ONE);
    assert!(first.scale.x > 1.0);

    app.update();
    let next = app.world().get::<Transform>(bone).unwrap();
    assert_eq!(next.translation, Vec3::ONE);
    assert!(next.scale.x > first.scale.x);
}

#[test]
fn sdk_animator_controls_the_visible_model_during_a_source_swap() {
    let (mut app, root, id, container) = scene_fixture();
    let bone = load_model(&mut app, container);
    app.update();
    assert!(app.world().get::<Transform>(bone).unwrap().translation.x > 1.0);

    // The visible instance stays ready while the replacement source is loading.
    app.world_mut()
        .entity_mut(container)
        .remove::<GltfProcessed>();
    send_animator(&mut app, root, id, Some(&swim_animator()));
    app.update();
    let swimming = *app.world().get::<Transform>(bone).unwrap();
    assert_eq!(swimming.translation, Vec3::ONE);
    assert!(swimming.scale.x > 1.0);

    let mut both = swim_animator();
    both.states.push(playing("bite"));
    send_animator(&mut app, root, id, Some(&both));
    app.update();
    let moving = *app.world().get::<Transform>(bone).unwrap();
    assert!(moving.translation.x > 1.0);
    assert!(moving.scale.x > swimming.scale.x);

    send_animator(&mut app, root, id, None);
    app.update();
    assert!(app
        .world()
        .get::<AnimationPlayer>(container)
        .unwrap()
        .playing_animations()
        .next()
        .is_none());
    let stopped = app.world().get::<Transform>(bone).unwrap();
    assert_eq!(stopped.translation, Vec3::ONE);
    assert_eq!(stopped.scale, moving.scale);

    // Keep the SDK Animator attached as the new instance takes over.
    send_animator(&mut app, root, id, Some(&swim_animator()));
    app.update();
    let replacement_bone = load_model(&mut app, container);
    app.update();
    let first = *app.world().get::<Transform>(replacement_bone).unwrap();
    assert_eq!(first.translation, Vec3::ONE);
    assert!(first.scale.x > 1.0);
    app.update();
    assert!(
        app.world()
            .get::<Transform>(replacement_bone)
            .unwrap()
            .scale
            .x
            > first.scale.x
    );
}

#[test]
fn sdk_animator_transitions_sample_the_default_pose_and_release_playback() {
    let (mut app, root, id, container) = scene_fixture();
    let bone = load_model(&mut app, container);
    app.update();
    assert!(app.world().get::<Transform>(bone).unwrap().translation.x > 1.0);

    let control = swim_animator();
    send_animator(&mut app, root, id, Some(&control));
    app.update();
    let first_swim = *app.world().get::<Transform>(bone).unwrap();
    assert_eq!(
        first_swim.translation,
        Vec3::ONE,
        "attachment must reset the default clip's channels"
    );
    assert!(first_swim.scale.x > 1.0);

    send_animator(&mut app, root, id, Some(&control));
    app.update();
    let last_swim = *app.world().get::<Transform>(bone).unwrap();
    assert!(
        last_swim.scale.x > first_swim.scale.x,
        "an ordinary SDK update must preserve the clip clock"
    );

    // Make both properties move so deletion must rewind the bite and hold the swim.
    let mut both = control.clone();
    both.states.push(playing("bite"));
    send_animator(&mut app, root, id, Some(&both));
    app.update();
    let before_removal = *app.world().get::<Transform>(bone).unwrap();
    assert!(before_removal.translation.x > 1.0);
    send_animator(&mut app, root, id, None);
    for _ in 0..3 {
        app.update();
        assert_eq!(
            app.world()
                .get::<AnimationPlayer>(container)
                .unwrap()
                .playing_animations()
                .count(),
            0
        );
        let stopped = app.world().get::<Transform>(bone).unwrap();
        assert_eq!(stopped.translation, Vec3::ONE);
        assert_eq!(stopped.scale, before_removal.scale);
    }

    send_animator(&mut app, root, id, Some(&control));
    app.update();
    assert_eq!(*app.world().get::<Transform>(bone).unwrap(), first_swim);

    // A model-ready notification in the removal update must also leave playback stopped.
    app.world_mut()
        .get_mut::<GltfReady>(container)
        .unwrap()
        .set_changed();
    send_animator(&mut app, root, id, None);
    app.update();
    assert!(app
        .world()
        .get::<AnimationPlayer>(container)
        .unwrap()
        .playing_animations()
        .next()
        .is_none());
}

#[test]
fn a_state_plays_only_when_the_scene_sets_playing() {
    let (mut app, root, id, container) = scene_fixture();
    let bone = load_model(&mut app, container);
    app.update();

    let unset = PbAnimator {
        states: vec![PbAnimationState {
            clip: "swim".into(),
            ..Default::default()
        }],
    };
    send_animator(&mut app, root, id, Some(&unset));
    for _ in 0..3 {
        app.update();
        let held = app.world().get::<Transform>(bone).unwrap();
        assert_eq!(
            held.translation,
            Vec3::ONE,
            "the first clip no longer plays"
        );
        assert_eq!(held.scale, Vec3::ONE, "an unset `playing` must not play");
    }
}

#[test]
fn a_clip_played_once_tells_the_scene_when_it_ends() {
    let (mut app, root, id, container) = scene_fixture();
    let bone = load_model(&mut app, container);
    app.update();

    send_animator(&mut app, root, id, Some(&swim_once()));
    app.update();
    assert!(scale(&app, bone) > 1.0);
    assert!(reported(&app, root, id).is_none());

    // the clip lasts a second and each update advances 100 ms
    for _ in 0..12 {
        app.update();
    }
    let (timestamp, animator) = reported(&app, root, id).expect("the end was not reported");
    assert_eq!(animator.states.len(), 1);
    assert_eq!(animator.states[0].clip, "swim");
    assert_eq!(animator.states[0].playing, Some(false));
    assert_eq!(animator.states[0].r#loop, Some(false));
    assert_eq!(app.world().resource::<FinishReports>().0, 1);

    // told once, and the model holds the pose the clip ended on
    for _ in 0..3 {
        app.update();
        assert_eq!(reported(&app, root, id).unwrap().0, timestamp);
        assert!((scale(&app, bone) - 2.0).abs() < 1e-4);
    }

    // The scene now holds `playing: false` for the clip, so a write about another clip does
    // not play it again.
    let mut stopped = swim_once();
    stopped.states[0].playing = Some(false);
    stopped.states.push(playing("bite"));
    send_animator(&mut app, root, id, Some(&stopped));
    app.update();
    app.update();
    assert!((scale(&app, bone) - 2.0).abs() < 1e-4);

    // playing it again starts from the first frame, without shouldReset
    send_animator(&mut app, root, id, Some(&swim_once()));
    app.update();
    let replayed = scale(&app, bone);
    assert!(replayed > 1.0 && replayed < 1.5, "replayed at {replayed}");
    assert_eq!(reported(&app, root, id).unwrap().0, timestamp);
}

#[test]
fn only_a_clip_that_ends_by_itself_is_reported() {
    let (mut app, root, id, container) = scene_fixture();
    load_model(&mut app, container);
    app.update();

    // a looping clip never ends
    send_animator(&mut app, root, id, Some(&swim_animator()));
    for _ in 0..15 {
        app.update();
    }
    assert!(reported(&app, root, id).is_none());

    // a clip its scene stops has not ended
    send_animator(&mut app, root, id, Some(&swim_once()));
    app.update();
    app.update();
    let mut stopped = swim_once();
    stopped.states[0].playing = Some(false);
    send_animator(&mut app, root, id, Some(&stopped));
    for _ in 0..15 {
        app.update();
    }
    assert!(reported(&app, root, id).is_none());
    assert_eq!(app.world().resource::<FinishReports>().0, 0);
}

#[test]
fn should_reset_restarts_a_playing_clip_and_leaves_a_stopped_one() {
    let (mut app, root, id, container) = scene_fixture();
    let bone = load_model(&mut app, container);
    app.update();

    send_animator(&mut app, root, id, Some(&swim_animator()));
    app.update();
    app.update();
    let paused_at = scale(&app, bone);

    // stopped with shouldReset, as `Animator.stopAllAnimations` writes it
    let mut stopped = swim_animator();
    stopped.states[0].playing = Some(false);
    stopped.states[0].should_reset = Some(true);
    send_animator(&mut app, root, id, Some(&stopped));
    app.update();
    assert_eq!(scale(&app, bone), paused_at);

    send_animator(&mut app, root, id, Some(&swim_animator()));
    app.update();
    let resumed = scale(&app, bone);
    assert!(
        resumed > paused_at,
        "a stopped state must keep its clock: {resumed} after {paused_at}"
    );

    let mut restart = swim_animator();
    restart.states[0].should_reset = Some(true);
    send_animator(&mut app, root, id, Some(&restart));
    app.update();
    assert!(
        scale(&app, bone) < paused_at,
        "shouldReset must restart a playing clip"
    );
}

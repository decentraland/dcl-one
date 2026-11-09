use std::{collections::BTreeMap, fs::File, io::Write, path::PathBuf};

use bevy::{
    platform::collections::{HashMap, HashSet},
    prelude::*,
};
use common::structs::{PrimaryCamera, PrimaryUser};
use dcl_component::{
    transform_and_parent::DclTransformAndParent, DclReader, DclWriter, SceneComponentId,
    SceneCrdtTimestamp, SceneEntityId,
};
use dcl_types::{
    crdt::lww::CrdtLWWState,
    interface::{CrdtStore, CrdtType},
    SceneId,
};
use itertools::Itertools;

use crate::{
    process_scene_entity_lifecycle,
    renderer_context::RendererSceneContext,
    update_world::{
        transform_and_parent::process_transform_and_parent_updates, CrdtStateComponent,
    },
    DeletedSceneEntities, SceneEntity, SceneLoopLabel,
};

fn init_test_app() -> App {
    common::test_support::init_logging();
    let mut app = App::new();
    let root = app.world_mut().spawn(Visibility::default()).id();
    let scene_id = SceneId(root);
    let context = RendererSceneContext::new(
        scene_id,
        "hash".to_owned(),
        "storage_root".to_owned(),
        false,
        0,
        "title".to_owned(),
        IVec2::ZERO,
        HashSet::from_iter([IVec2::ZERO]),
        vec![],
        Default::default(),
        vec![],
        root,
        0.0,
        false,
        "sdk_version",
        false,
        false,
    );
    app.world_mut().entity_mut(root).insert((
        context,
        DeletedSceneEntities::default(),
        SceneEntity {
            root,
            scene_id,
            id: SceneEntityId::ROOT,
        },
    ));
    app.world_mut()
        .spawn((PrimaryUser::default(), PrimaryCamera::default()));
    app
}

// check output vs file
#[cfg(not(feature = "gen-tests"))]
macro_rules! assert_output_eq {
    ($result:ident, $path:expr) => {
        assert_eq!(
            $result.replace("\r", ""),
            include_str!($path).replace("\r", "")
        )
    };
}

// write output to file
#[allow(dead_code)]
fn write_expected(expected: String, filename: &str) {
    let mut path = PathBuf::from(file!());
    path.pop();
    path.push(filename);
    let mut f = File::create(path.clone()).unwrap();
    f.write_all(expected.as_bytes()).unwrap();
}

macro_rules! check_or_write {
    ($testdata:ident, $filename:expr) => {
        #[cfg(feature = "gen-tests")]
        write_expected($testdata, $filename);
        #[cfg(not(feature = "gen-tests"))]
        assert_output_eq!($testdata, $filename);
    };
}

fn make_graph(app: &mut App) -> String {
    let mut scene_query = app
        .world_mut()
        .query_filtered::<Entity, With<RendererSceneContext>>();
    assert_eq!(scene_query.iter(app.world()).len(), 1);
    let root = scene_query.iter(app.world()).next().unwrap();

    let mut scene_entity_query = app.world_mut().query::<(&SceneEntity, Option<&Children>)>();
    let mut graph_nodes = HashMap::new();
    let mut graph = petgraph::Graph::<_, ()>::new();
    let mut to_check = vec![root];

    while let Some(ent) = to_check.pop() {
        debug!("current: {ent:?}, to_check: {to_check:?}");
        let Ok((scene_entity, maybe_children)) = scene_entity_query.get(app.world(), ent) else {
            panic!()
        };
        assert_eq!(scene_entity.root, root);

        let graph_node = *graph_nodes
            .entry(ent)
            .or_insert_with(|| graph.add_node(scene_entity.id.to_string()));

        if let Some(children) = maybe_children {
            let sorted_children_with_scene_id: BTreeMap<_, _> = children
                .iter()
                .filter_map(|c| {
                    scene_entity_query
                        .get(app.world(), c)
                        .ok()
                        .map(|q| (q.0.id, c))
                })
                .collect();

            to_check.extend(sorted_children_with_scene_id.values().copied());
            for (child_id, child_ent) in sorted_children_with_scene_id.into_iter() {
                debug!(
                    "child of {:?}/{} -> {:?}/{}",
                    ent, scene_entity.id, child_ent, child_id
                );
                let child_graph_node = *graph_nodes
                    .entry(child_ent)
                    .or_insert_with(|| graph.add_node(child_id.to_string()));
                graph.add_edge(graph_node, child_graph_node, ());
            }
        }
    }

    let dot = petgraph::dot::Dot::with_config(&graph, &[petgraph::dot::Config::EdgeNoLabel]);
    format!("{:?}", dot)
}

fn make_reparent_buffer(parent: u16) -> Vec<u8> {
    let parent = SceneEntityId {
        id: parent,
        generation: 0,
    };
    let mut buf = Vec::new();
    DclWriter::new(&mut buf).write(&DclTransformAndParent {
        parent,
        ..Default::default()
    });
    buf
}

// fn run_single_update(app: &mut App) {
//     // run once
//     while app
//         .world
//         .resource_mut::<SceneUpdates>()
//         .jobs_in_flight
//         .is_empty()
//     {
//         // set last update time to zero so the scheduler doesn't freak out
//         app.world
//             .query::<&mut RendererSceneContext>()
//             .single_mut(&mut app.world)
//             .last_sent = 0.0;
//         Schedule::new(SceneLoopLabel)
//             .add_systems((update_scene_priority, send_scene_updates).chain())
//             .run(&mut app.world);
//     }
//     assert_eq!(
//         app.world
//             .resource_mut::<SceneUpdates>()
//             .jobs_in_flight
//             .len(),
//         1
//     );

//     while app
//         .world
//         .resource_mut::<SceneUpdates>()
//         .jobs_in_flight
//         .len()
//         == 1
//     {
//         // run the receiver and lifecycle part of the schedule
//         Schedule::new(SceneLoopLabel)
//             .add_systems(
//                 (
//                     receive_scene_updates,
//                     process_scene_entity_lifecycle,
//                     apply_deferred,
//                     process_transform_and_parent_updates,
//                 )
//                     .chain(),
//             )
//             .run(&mut app.world);
//     }

//     // make sure we got the one response
//     assert!(app
//         .world
//         .resource_mut::<SceneUpdates>()
//         .jobs_in_flight
//         .is_empty());
// }

// basic hierarchy test
// #[test]
// fn flat_hierarchy() {
//     // Setup app
//     let mut app = init_test_app("flat_hierarchy");

//     let graph = make_graph(&mut app);
//     check_or_write!(graph, "expected/flat_hierarchy_onStart.dot");

//     info!("running update");

//     // onUpdate
//     run_single_update(&mut app);

//     let graph = make_graph(&mut app);
//     check_or_write!(graph, "expected/flat_hierarchy_onUpdate.dot");
// }

// // test moving entities out of a hierarchy
// #[test]
// fn reparenting() {
//     // Setup app
//     let mut app = init_test_app("reparenting");

//     // onUpdate
//     run_single_update(&mut app);

//     let graph = make_graph(&mut app);
//     check_or_write!(graph, "expected/reparenting_1.dot");

//     // onUpdate
//     run_single_update(&mut app);
//     let graph = make_graph(&mut app);
//     check_or_write!(graph, "expected/reparenting_2.dot");
// }

// // test creating parents late
// #[test]
// fn late_entities() {
//     // Setup app
//     let mut app = init_test_app("late_entities");

//     // onUpdate
//     run_single_update(&mut app);

//     let graph = make_graph(&mut app);
//     check_or_write!(graph, "expected/late_entities_1.dot");

//     // onUpdate
//     run_single_update(&mut app);
//     let graph = make_graph(&mut app);
//     check_or_write!(graph, "expected/late_entities_2.dot");

//     // onUpdate
//     run_single_update(&mut app);
//     let graph = make_graph(&mut app);
//     check_or_write!(graph, "expected/late_entities_3.dot");
// }

#[test]
fn cyclic_recovery() {
    let states = [(603, 601), (601, 602), (602, 603), (603, 0)]
        .into_iter()
        .enumerate()
        .map(|(timestamp, (ent, par))| {
            (
                SceneEntityId {
                    id: ent,
                    generation: 0,
                },
                SceneCrdtTimestamp(timestamp as u32),
                make_reparent_buffer(par),
            )
        });

    for messages in states.permutations(4) {
        // create new app instance
        let mut app = init_test_app();
        // add lww state
        let scene_entity = app
            .world_mut()
            .query_filtered::<Entity, With<RendererSceneContext>>()
            .single(app.world_mut())
            .unwrap();
        app.world_mut()
            .entity_mut(scene_entity)
            .insert(CrdtStateComponent::<CrdtLWWState, DclTransformAndParent>::default());

        let mut crdt_store = CrdtStore::default();

        for (dcl_entity, timestamp, data) in messages.iter().take(4) {
            let (mut scene_context, mut crdt_state) = app
                .world_mut()
                .query::<(
                    &mut RendererSceneContext,
                    &mut CrdtStateComponent<CrdtLWWState, DclTransformAndParent>,
                )>()
                .single_mut(app.world_mut())
                .unwrap();

            // initialize the scene entity
            if scene_context.bevy_entity(*dcl_entity).is_none() {
                scene_context.nascent.insert(*dcl_entity);
            }

            // add next message
            let reader = &mut DclReader::new(data);
            crdt_store.try_update(
                SceneComponentId::TRANSFORM,
                CrdtType::LWW_ENT,
                *dcl_entity,
                *timestamp,
                Some(reader),
            );
            // pull updates
            *crdt_state = CrdtStateComponent::new(
                crdt_store
                    .take_updates()
                    .lww
                    .get(&SceneComponentId::TRANSFORM)
                    .cloned()
                    .unwrap_or_default(),
            );

            // run systems
            Schedule::new(SceneLoopLabel)
                .add_systems(
                    (
                        process_scene_entity_lifecycle,
                        process_transform_and_parent_updates,
                    )
                        .chain(),
                )
                .run(app.world_mut());
        }
        let graph = make_graph(&mut app);
        check_or_write!(graph, "expected/cyclic_recovery.dot");
    }
}

#[test]
fn lifecycle_cleans_dead_entities_from_crdt_store() {
    let mut world = World::new();

    let dead = SceneEntityId::new(600, 0);
    let live = SceneEntityId::new(601, 0);

    let mut context = RendererSceneContext::new(
        dcl_types::SceneId::DUMMY,
        "hash".to_owned(),
        "storage_root".to_owned(),
        false,
        0,
        "title".to_owned(),
        IVec2::ZERO,
        HashSet::from_iter([IVec2::ZERO]),
        vec![],
        Default::default(),
        vec![],
        Entity::PLACEHOLDER,
        0.0,
        false,
        "sdk_version",
        false,
        false,
    );
    for id in [dead, live] {
        context.crdt_store.force_update(
            SceneComponentId::TRANSFORM,
            CrdtType::LWW_ENT,
            id,
            Some(&mut DclReader::new(&make_reparent_buffer(0))),
        );
        // grow-only entries are kept per entity too
        for _ in 0..2 {
            context.crdt_store.force_update(
                SceneComponentId::POINTER_RESULT,
                CrdtType::GO_ENT,
                id,
                Some(&mut DclReader::new(&[1, 2, 3])),
            );
        }
    }
    context.death_row.insert(dead);
    world.spawn((context, DeletedSceneEntities::default()));

    Schedule::new(SceneLoopLabel)
        .add_systems(process_scene_entity_lifecycle)
        .run(&mut world);

    let context = world
        .query::<&RendererSceneContext>()
        .single(&world)
        .unwrap();
    let lww = context
        .crdt_store
        .lww
        .get(&SceneComponentId::TRANSFORM)
        .unwrap();
    assert!(!lww.last_write.contains_key(&dead));
    assert!(!lww.updates.contains(&dead));
    assert!(lww.last_write.contains_key(&live));

    let go = context
        .crdt_store
        .go
        .get(&SceneComponentId::POINTER_RESULT)
        .unwrap();
    assert!(!go.0.contains_key(&dead));
    assert_eq!(go.0.get(&live).map(|entries| entries.len()), Some(2));
}

#[test]
fn test_scene_ray() {
    fn ray_code(mut position: Vec3, mut ray: Vec3) -> Vec<(IVec2, f32)> {
        let mut results = Vec::default();

        let mut distance = 0.0;

        if ray.length() == 0.0 {
            return results;
        }

        if ray.length() > 1000.0 {
            ray = ray.normalize() * 1000.0;
        }

        const EPS: f32 = 0.01;
        let offset: Vec3 = Vec3::new(ray.x.signum() * EPS, ray.y.signum() * EPS, 0.0);

        loop {
            let adj_position = position + offset;

            results.push((
                (adj_position / 16.0).floor().truncate().as_ivec2(),
                distance,
            ));

            let x_dist = if ray.x < 0.0 {
                (((adj_position.x / 16.0).floor() * 16.0) - position.x) / ray.x
            } else if ray.x > 0.0 {
                (((adj_position.x / 16.0).ceil() * 16.0) - position.x) / ray.x
            } else {
                999.0
            };
            let y_dist = if ray.y < 0.0 {
                (((adj_position.y / 16.0).floor() * 16.0) - position.y) / ray.y
            } else if ray.y > 0.0 {
                (((adj_position.y / 16.0).ceil() * 16.0) - position.y) / ray.y
            } else {
                999.0
            };
            println!("pos: {position}, ray: {ray}, x:{x_dist} / y:{y_dist}");

            let step_fraction = x_dist.min(y_dist);
            if step_fraction > 1.0 {
                return results;
            }

            let step = ray * step_fraction;
            position += step;
            distance += step.length();
            ray -= step;
            println!("step: {step}, dist: {distance}");
        }
    }

    assert_eq!(ray_code(Vec3::ONE, Vec3::ONE), vec![(IVec2::ZERO, 0.0)]);
    assert_eq!(
        ray_code(Vec3::splat(17.0), Vec3::ONE),
        vec![(IVec2::ONE, 0.0)]
    );
    assert_eq!(
        ray_code(Vec3::splat(-17.0), -Vec3::ONE),
        vec![(IVec2::splat(-2), 0.0)]
    );

    let results = ray_code(Vec3::splat(15.0), Vec3::new(2.0, 2.0, 0.0));
    assert_eq!(results.len(), 2);
    assert_eq!(results[0].0, IVec2::splat(0));
    assert_eq!(results[1].0, IVec2::splat(1));
    assert_eq!(results[0].1, 0.0);
    assert!((results[1].1 - 2f32.sqrt()).abs() < 0.01);

    let results = ray_code(Vec3::splat(15.0), Vec3::new(2.0, 4.0, 0.0));
    assert_eq!(results.len(), 3);
    assert_eq!(results[0].0, IVec2::splat(0));
    assert_eq!(results[1].0, IVec2::new(0, 1));
    assert_eq!(results[2].0, IVec2::splat(1));
    assert_eq!(results[0].1, 0.0);
    assert!((results[1].1 - f32::sqrt(1.0 + 0.5 * 0.5)).abs() < 0.01);
    assert!((results[2].1 - 2.0 * f32::sqrt(1.0 + 0.5 * 0.5)).abs() < 0.01);

    let results = ray_code(Vec3::splat(-15.0), -Vec3::new(2.0, 2.0, 0.0));
    println!("results: {results:?}");
    assert_eq!(results.len(), 2);
    assert_eq!(results[0].0, IVec2::splat(-1));
    assert_eq!(results[1].0, IVec2::splat(-2));
    assert_eq!(results[0].1, 0.0);
    assert!((results[1].1 - 2f32.sqrt()).abs() < 0.01);

    let results = ray_code(Vec3::splat(-15.0), -Vec3::new(2.0, 4.0, 0.0));
    assert_eq!(results.len(), 3);
    assert_eq!(results[0].0, IVec2::splat(-1));
    assert_eq!(results[1].0, IVec2::new(-1, -2));
    assert_eq!(results[2].0, IVec2::splat(-2));
    assert_eq!(results[0].1, 0.0);
    assert!((results[1].1 - f32::sqrt(1.0 + 0.5 * 0.5)).abs() < 0.01);
    assert!((results[2].1 - 2.0 * f32::sqrt(1.0 + 0.5 * 0.5)).abs() < 0.01);

    let results = ray_code(Vec3::splat(-8.0), Vec3::new(8.0, -8.0, 0.0));
    assert_eq!(results.len(), 2);
    assert_eq!(results[0].0, IVec2::splat(-1));
    assert_eq!(results[1].0, IVec2::new(0, -2));
    assert_eq!(results[0].1, 0.0);
    assert!((results[1].1 - f32::sqrt(8.0 * 8.0 * 2.0)).abs() < 0.01);
}

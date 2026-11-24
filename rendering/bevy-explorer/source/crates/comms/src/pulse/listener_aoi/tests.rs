use bevy::platform::collections::HashSet;

use super::*;

/// A race track of about 22,000 parcels whose curved edges take about 1,000 rects.
pub(crate) fn track() -> Vec<IVec2> {
    let mut parcels = HashSet::new();
    for (radius, wobble) in [(140.0, 25.0), (90.0, 18.0), (40.0, 8.0)] {
        for step in 0..4000 {
            let t = std::f32::consts::TAU * step as f32 / 4000.0;
            let x = (radius * t.cos() + wobble * (6.0 * t).cos()).round() as i32;
            let z = (radius * t.sin() + wobble * (4.0 * t).sin()).round() as i32;
            for dx in -5..=5 {
                for dz in -5..=5 {
                    if dx * dx + dz * dz <= 30 {
                        parcels.insert(IVec2::new(x + dx, z + dz));
                    }
                }
            }
        }
    }
    let mut parcels: Vec<IVec2> = parcels.into_iter().collect();
    parcels.sort_unstable_by_key(|p| (p.x, p.y));
    parcels
}

fn scene(realm: &str, scene: &str, parcels: Vec<IVec2>) -> HostedScene {
    HostedScene {
        realm: realm.to_owned(),
        scene: scene.to_owned(),
        parcels,
    }
}

fn block(x: i32, z: i32, w: i32, h: i32) -> Vec<IVec2> {
    (x..x + w)
        .flat_map(|x| (z..z + h).map(move |z| IVec2::new(x, z)))
        .collect()
}

/// Every parcel a realm's rects cover.
fn covered(aoi: &[pulse::SceneListenerAoi], realm: &str) -> HashSet<IVec2> {
    aoi.iter()
        .filter(|entry| entry.realm == realm)
        .flat_map(|entry| &entry.parcel_rects)
        .flat_map(|r| {
            (r.min_x..=r.max_x)
                .flat_map(move |x| (r.min_z..=r.max_z).map(move |z| IVec2::new(x, z)))
        })
        .collect()
}

#[test]
fn a_small_aoi_is_announced_exactly() {
    let scenes = [
        scene("b", "s2", block(0, 0, 2, 2)),
        scene(
            "a",
            "s1",
            vec![IVec2::new(0, 0), IVec2::new(1, 0), IVec2::new(0, 1)],
        ),
        scene("b", "s3", block(2, 0, 1, 2)),
    ];

    let (aoi, compaction) = listener_aoi(&scenes, MAX_AOI_BYTES);

    assert!(compaction.is_empty());
    let realms: Vec<&str> = aoi.iter().map(|entry| entry.realm.as_str()).collect();
    assert_eq!(realms, ["a", "b"]);
    // adjacent scenes of one realm merge into one block
    assert_eq!(aoi[1].parcel_rects, [rect(IVec2::ZERO, IVec2::new(2, 1))]);
    assert_eq!(aoi[0].parcel_rects.len(), 2);
    assert_eq!(covered(&aoi, "a").len(), 3);
}

#[test]
fn a_track_world_is_announced_as_its_bounding_box_within_budget() {
    let track = track();
    assert!(track.len() > 20_000, "{}", track.len());
    let scenes = [
        scene("track.dcl.eth", "track", track.clone()),
        scene("other.dcl.eth", "small", block(0, 0, 3, 1)),
    ];
    let exact_bytes = encoded_len(&listener_aoi(&scenes, usize::MAX).0);
    assert!(exact_bytes > 8192, "{exact_bytes}");

    let (aoi, compaction) = listener_aoi(&scenes, MAX_AOI_BYTES);

    assert!(encoded_len(&aoi) <= MAX_AOI_BYTES);
    assert_eq!(compaction.exact_bytes, exact_bytes);
    let [collapsed] = compaction.collapsed.as_slice() else {
        panic!("{compaction:?}")
    };
    assert_eq!(
        (collapsed.realm.as_str(), collapsed.scene.as_str()),
        ("track.dcl.eth", "track")
    );
    assert!(collapsed.rects > 900, "{}", collapsed.rects);
    assert!(compaction.dropped.is_empty());
    let min = track.iter().fold(IVec2::MAX, |a, b| a.min(*b));
    let max = track.iter().fold(IVec2::MIN, |a, b| a.max(*b));
    let track_entry = aoi.iter().find(|e| e.realm == "track.dcl.eth").unwrap();
    assert_eq!(track_entry.parcel_rects, [rect(min, max)]);
    let other = aoi.iter().find(|e| e.realm == "other.dcl.eth").unwrap();
    assert_eq!(other.parcel_rects, [rect(IVec2::ZERO, IVec2::new(2, 0))]);
}

#[test]
fn the_scenes_with_the_most_rects_collapse_first_and_only_as_needed() {
    // a staircase of n steps takes n rects
    let stairs = |n: i32, x: i32| (0..n).map(|i| IVec2::new(x + i, i)).collect::<Vec<_>>();
    let scenes = [
        scene("r", "few", stairs(20, 1000)),
        scene("r", "many", stairs(200, 0)),
        scene("r", "some", stairs(100, 500)),
    ];
    let budget = encoded_len(&listener_aoi(&scenes, usize::MAX).0) - 1;

    let (aoi, compaction) = listener_aoi(&scenes, budget);

    let names: Vec<&str> = compaction
        .collapsed
        .iter()
        .map(|c| c.scene.as_str())
        .collect();
    assert_eq!(names, ["many"]);
    assert!(encoded_len(&aoi) <= budget);
    // every hosted parcel is still covered
    let covered = covered(&aoi, "r");
    assert!(scenes
        .iter()
        .flat_map(|s| &s.parcels)
        .all(|p| covered.contains(p)));
}

/// Only the track collapses: a block of one-parcel scenes in another realm stays one merged rect.
#[test]
fn compaction_keeps_other_scenes_merged() {
    let mut scenes: Vec<HostedScene> = block(0, 0, 20, 20)
        .into_iter()
        .enumerate()
        .map(|(i, parcel)| scene("genesis", &format!("s{i:03}"), vec![parcel]))
        .collect();
    scenes.push(scene("track.dcl.eth", "track", track()));

    let (aoi, compaction) = listener_aoi(&scenes, MAX_AOI_BYTES);

    assert_eq!(compaction.collapsed.len(), 1);
    assert!(compaction.dropped.is_empty());
    let genesis = aoi.iter().find(|e| e.realm == "genesis").unwrap();
    assert_eq!(
        genesis.parcel_rects,
        [rect(IVec2::ZERO, IVec2::new(19, 19))]
    );
}

#[test]
fn compaction_does_not_depend_on_input_order() {
    let track = track();
    let mut reversed = track.clone();
    reversed.reverse();
    let forward = [
        scene("w", "a", track.clone()),
        scene(
            "w",
            "b",
            track.iter().map(|p| *p + IVec2::new(400, 0)).collect(),
        ),
    ];
    let backward = [
        scene(
            "w",
            "b",
            reversed.iter().map(|p| *p + IVec2::new(400, 0)).collect(),
        ),
        scene("w", "a", reversed),
    ];

    assert_eq!(
        listener_aoi(&forward, MAX_AOI_BYTES),
        listener_aoi(&backward, MAX_AOI_BYTES)
    );
}

#[test]
fn realms_that_cannot_fit_are_dropped_largest_first() {
    let scenes: Vec<HostedScene> = (0..200)
        .map(|i| {
            let realm = format!(
                "{}{i:03}.dcl.eth",
                if i == 7 {
                    "a-much-longer-world-name-"
                } else {
                    "w"
                }
            );
            scene(&realm, "s", block(0, 0, 1, 1))
        })
        .collect();

    let (aoi, compaction) = listener_aoi(&scenes, MAX_AOI_BYTES);

    assert!(encoded_len(&aoi) <= MAX_AOI_BYTES);
    assert!(compaction.collapsed.is_empty());
    assert_eq!(
        compaction.dropped[0],
        "a-much-longer-world-name-007.dcl.eth"
    );
    assert_eq!(aoi.len() + compaction.dropped.len(), 200);
    // what is announced is announced exactly
    assert!(aoi.iter().all(|entry| entry.parcel_rects.len() == 1));
}

#[test]
fn the_largest_realm_by_area_is_left_out_after_a_rejection() {
    let aoi = [
        realm_aoi("big.dcl.eth", vec![rect(IVec2::ZERO, IVec2::new(99, 99))]),
        realm_aoi(
            "many-rects.dcl.eth",
            (0..50)
                .map(|i| rect(IVec2::new(i * 2, 0), IVec2::new(i * 2, 0)))
                .collect(),
        ),
        realm_aoi("small.dcl.eth", vec![rect(IVec2::ZERO, IVec2::new(1, 1))]),
    ];

    assert_eq!(realm_to_leave_out(&aoi).unwrap().realm, "big.dcl.eth");
}

#[test]
fn a_tie_leaves_out_the_first_realm_by_name() {
    let aoi = [
        realm_aoi("b", vec![rect(IVec2::ZERO, IVec2::ZERO)]),
        realm_aoi("a", vec![rect(IVec2::ZERO, IVec2::ZERO)]),
    ];

    assert_eq!(realm_to_leave_out(&aoi).unwrap().realm, "a");
}

#[test]
fn a_single_realm_is_never_left_out() {
    let aoi = [realm_aoi(
        "only",
        vec![rect(IVec2::ZERO, IVec2::new(999, 999))],
    )];

    assert!(realm_to_leave_out(&aoi).is_none());
}

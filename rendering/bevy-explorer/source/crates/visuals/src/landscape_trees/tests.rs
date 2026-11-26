use bevy::prelude::*;

use super::layout::{hidden_by_occupancy, TreeLayout};

const LAYOUT_FILE: &[u8] = include_bytes!(concat!(
    env!("DCL_ONE_ROOT"),
    "/rendering/bevy-explorer/source/crates/assets/src/assets/landscape/layout/landscape.trees"
));

/// Bytes before the instances: the magic, the bounds and one count per parcel.
const INSTANCES_AT: usize = 4 + 16 + 318 * 312;

#[test]
fn parses_both_layouts() {
    for worlds in [false, true] {
        let layout = TreeLayout::parse(LAYOUT_FILE, worlds).unwrap();
        assert_eq!(layout.min, IVec2::new(-152, -152));
        assert_eq!(layout.max_excl, IVec2::new(166, 160));
        assert_eq!(layout.len(), 303_020);
    }
}

#[test]
fn the_layouts_share_instances_and_differ_in_scale() {
    let genesis = TreeLayout::parse(LAYOUT_FILE, false).unwrap();
    let worlds = TreeLayout::parse(LAYOUT_FILE, true).unwrap();
    // parcel (0,0)'s second instance: Genesis scale 0, Worlds scale 205
    let (genesis, worlds) = (
        genesis.instances(IVec2::ZERO)[1],
        worlds.instances(IVec2::ZERO)[1],
    );
    assert_eq!(
        (genesis.prototype, genesis.pos, genesis.rot_y),
        (worlds.prototype, worlds.pos, worlds.rot_y)
    );
    assert_eq!((genesis.scale, worlds.scale), (0, 205));
}

#[test]
fn truncated_file_is_an_error() {
    assert!(TreeLayout::parse(&LAYOUT_FILE[..100], false).is_err());
    // every cut through the magic, the header and the counts
    for cut in [0, 3, 4, 19, 20, 21, INSTANCES_AT - 1, INSTANCES_AT] {
        assert!(
            TreeLayout::parse(&LAYOUT_FILE[..cut], false).is_err(),
            "cut at {cut}"
        );
    }
    // one byte short of the last instance
    assert!(TreeLayout::parse(&LAYOUT_FILE[..LAYOUT_FILE.len() - 1], true).is_err());
}

#[test]
fn malformed_files_are_an_error() {
    // a 2x1 layout: magic, header, the two parcels' counts, then 6-byte instances
    let file = |counts: [u8; 2], instances: usize, prototype: u8| {
        let mut bytes = b"TRL1".to_vec();
        for value in [0, 0, 2, 1] {
            bytes.extend_from_slice(&i32::to_le_bytes(value));
        }
        bytes.extend_from_slice(&counts);
        for _ in 0..instances {
            bytes.extend_from_slice(&[prototype, 0, 0, 0, 0, 0]);
        }
        bytes
    };
    let good = TreeLayout::parse(&file([1, 0], 1, 7), false).unwrap();
    assert_eq!(good.instances(IVec2::new(0, 0)).len(), 1);
    // the last parcel's slice ends at the instance count
    assert_eq!(good.instances(IVec2::new(1, 0)).len(), 0);
    // counts that name more instances than follow, or fewer
    assert!(TreeLayout::parse(&file([1, 1], 1, 0), false).is_err());
    assert!(TreeLayout::parse(&file([1, 0], 2, 0), false).is_err());
    // a prototype with no entry in the table
    assert!(TreeLayout::parse(&file([1, 0], 1, 8), false).is_err());
    // bounds whose maximum is below the minimum
    let mut inverted = file([1, 0], 1, 0);
    inverted[12..16].copy_from_slice(&(-1i32).to_le_bytes());
    assert!(TreeLayout::parse(&inverted, false).is_err());
    // another file
    let mut other = file([1, 0], 1, 0);
    other[0] = b'X';
    assert!(TreeLayout::parse(&other, false).is_err());
}

#[test]
fn parcels_outside_bounds_have_no_instances() {
    let layout = TreeLayout::parse(LAYOUT_FILE, false).unwrap();
    for parcel in [
        IVec2::new(-153, 0),
        IVec2::new(166, 0),
        IVec2::new(0, 160),
        IVec2::new(10_000, 10_000),
    ] {
        assert!(layout.instances(parcel).is_empty(), "{parcel}");
    }
    // the corners just inside the bounds index without panicking
    for parcel in [IVec2::new(-152, -152), IVec2::new(165, 159)] {
        let _ = layout.instances(parcel);
    }
}

#[test]
fn instance_transform_lands_in_bevy_axes() {
    // Bytes are {prototype, posX, posZ, rotY, genesisScale, worldsScale}; Genesis City's layout is
    // checked. Parcel (0,0) holds two instances (indices 148482 and 148483 at file offsets 990128
    // and 990134); the third is from parcel (-1,-1) (index 147525, offset 984386) so negative
    // parcels are covered too.
    //
    // The layout's formula, in its left-handed frame (x east, z north):
    //   x = (parcel.x + posX / 255) * 16          z = (parcel.y + posZ / 255) * 16
    //   yaw = rotY * 360 / 255 degrees about +y   (+x turns towards -z)
    //   scale = lerp(min, max, scale / 255) on every axis (its horizontal and vertical ranges)
    // Bevy is right-handed with z south, so bevy z = -z, and mirroring z reverses the sense of a
    // yaw: the bevy rotation is about +y by -yaw. Local +x then lands on (cos yaw, 0, sin yaw).
    // The height callback here is 0.5 * x + 0.25 * bevy z, so y checks which z it was handed.
    //
    // A, parcel (0,0), bytes [0, 107, 19, 212, 72, 72], prototype 0 (scale range 2 to 3):
    //   x = 107 / 255 * 16 = 6.713725        bevy z = -(19 / 255 * 16) = -1.192157
    //   y = 0.5 * 6.713725 + 0.25 * -1.192157 = 3.058824
    //   yaw = 212 * 360 / 255 = 299.294118; quat about y by -yaw: (0, -sin(yaw/2), 0, cos(yaw/2))
    //     = (0, -0.505325, 0, -0.862929); local +x -> (cos yaw, 0, sin yaw) = (0.489293, 0, -0.872120)
    //   scale = 2 + 72 / 255 * (3 - 2) = 2.282353 on every axis
    // B, parcel (0,0), bytes [0, 192, 62, 196, 0, 205], prototype 0 (2 to 3):
    //   x = 192 / 255 * 16 = 12.047059       bevy z = -(62 / 255 * 16) = -3.890196
    //   y = 0.5 * 12.047059 + 0.25 * -3.890196 = 5.050980
    //   yaw = 196 * 360 / 255 = 276.705882; quat (0, -0.664540, 0, -0.747253);
    //     local +x -> (0.116773, 0, -0.993159)
    //   scale = 2 + 0 = 2
    // C, parcel (-1,-1), bytes [1, 104, 195, 51, 227, 227], prototype 1 (2 to 3):
    //   x = (-1 + 104 / 255) * 16 = -9.474510   bevy z = -((-1 + 195 / 255) * 16) = 3.764706
    //   y = 0.5 * -9.474510 + 0.25 * 3.764706 = -3.796078
    //   yaw = 51 * 360 / 255 = 72; quat (0, -0.587785, 0, 0.809017);
    //     local +x -> (0.309017, 0, 0.951057)
    //   scale = 2 + 227 / 255 = 2.890196
    struct Expected {
        parcel: IVec2,
        index: usize,
        translation: Vec3,
        rotation: Quat,
        local_x: Vec3,
        scale: f32,
    }
    let expected = [
        Expected {
            parcel: IVec2::new(0, 0),
            index: 0,
            translation: Vec3::new(6.713725, 3.058824, -1.192157),
            rotation: Quat::from_xyzw(0.0, -0.505325, 0.0, -0.862929),
            local_x: Vec3::new(0.489293, 0.0, -0.872120),
            scale: 2.282353,
        },
        Expected {
            parcel: IVec2::new(0, 0),
            index: 1,
            translation: Vec3::new(12.047059, 5.05098, -3.890196),
            rotation: Quat::from_xyzw(0.0, -0.664540, 0.0, -0.747253),
            local_x: Vec3::new(0.116773, 0.0, -0.993159),
            scale: 2.0,
        },
        Expected {
            parcel: IVec2::new(-1, -1),
            index: 2,
            translation: Vec3::new(-9.47451, -3.796078, 3.764706),
            rotation: Quat::from_xyzw(0.0, -0.587785, 0.0, 0.809017),
            local_x: Vec3::new(0.309017, 0.0, 0.951057),
            scale: 2.890196,
        },
    ];

    let layout = TreeLayout::parse(LAYOUT_FILE, false).unwrap();
    assert_eq!(layout.instances(IVec2::new(0, 0)).len(), 2);
    assert_eq!(layout.instances(IVec2::new(-1, -1)).len(), 3);
    for case in expected {
        let instance = layout.instances(case.parcel)[case.index];
        let transform = instance.transform(case.parcel, |x, z| 0.5 * x + 0.25 * z);
        let label = format!("parcel {} instance {}", case.parcel, case.index);
        assert!(
            transform.translation.abs_diff_eq(case.translation, 1e-4),
            "{label}: {}",
            transform.translation
        );
        // q and -q are the same rotation
        assert!(
            transform.rotation.abs_diff_eq(case.rotation, 1e-4)
                || transform.rotation.abs_diff_eq(-case.rotation, 1e-4),
            "{label}: {}",
            transform.rotation
        );
        assert!(
            (transform.rotation * Vec3::X).abs_diff_eq(case.local_x, 1e-4),
            "{label}: {}",
            transform.rotation * Vec3::X
        );
        assert!(
            transform.scale.abs_diff_eq(Vec3::splat(case.scale), 1e-4),
            "{label}: {}",
            transform.scale
        );
    }
}

#[test]
fn hidden_when_radius_crosses_occupied() {
    // Parcel (0,0), 14 m east and 13 m north of its south-west corner: 2 m from the east
    // neighbour (1,0), 3 m from the north neighbour (0,1), 13 m or more from the west and south
    // ones. North is parcel y + 1, which is bevy -z.
    let tree = Vec2::new(14.0, -13.0);
    let only = |occupied: IVec2| move |parcel: IVec2| parcel == occupied;

    assert!(hidden_by_occupancy(tree, 10.0, only(IVec2::new(1, 0))));
    assert!(!hidden_by_occupancy(tree, 10.0, |_| false));

    assert!(hidden_by_occupancy(tree, 10.0, only(IVec2::new(0, 0))));
    assert!(hidden_by_occupancy(tree, 10.0, only(IVec2::new(0, 1))));
    assert!(hidden_by_occupancy(tree, 10.0, only(IVec2::new(1, 1))));
    assert!(!hidden_by_occupancy(tree, 10.0, only(IVec2::new(-1, 0))));
    assert!(!hidden_by_occupancy(tree, 10.0, only(IVec2::new(0, -1))));
    assert!(!hidden_by_occupancy(tree, 10.0, only(IVec2::new(1, -1))));
    // a radius that stays inside the parcel ignores the neighbours
    assert!(!hidden_by_occupancy(tree, 1.5, only(IVec2::new(1, 0))));
}

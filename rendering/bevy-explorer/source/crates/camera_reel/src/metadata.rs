//! What a photo records about itself: who took it, where, and who is in it. The wire shape is
//! the camera-reel service's `Metadata`, plus each person's `screenRect` in the saved image.

use bevy::{
    math::Affine3A,
    prelude::*,
    render::{
        camera::CameraProjection,
        primitives::{Aabb, Frustum},
    },
};
use common::dynamics::{PLAYER_COLLIDER_HEIGHT, PLAYER_COLLIDER_RADIUS};
use serde::{Deserialize, Serialize};

pub const UNKNOWN_USER: &str = "Unknown";
pub const UNKNOWN_USER_WALLET: &str = "0x000000000000000000000000000000000000000";
pub const UNKNOWN_PLACE: &str = "Unknown place";

/// The saved photo's size and the share of the screen it is cut from.
pub const PHOTO_WIDTH: u32 = 1920;
pub const PHOTO_HEIGHT: u32 = 1080;
pub const TARGET_ASPECT_RATIO: f32 = PHOTO_WIDTH as f32 / PHOTO_HEIGHT as f32;
pub const FRAME_SCALE: f32 = 0.87;

const BASE_WEARABLE_PREFIX: &str = "urn:decentraland:off-chain:base-avatars:";

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ScreenshotMetadata {
    pub user_name: String,
    pub user_address: String,
    /// Unix seconds, as the service stores it.
    pub date_time: String,
    pub realm: String,
    pub scene: Scene,
    pub visible_people: Vec<VisiblePerson>,
    pub place_id: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Scene {
    pub name: String,
    pub location: Location,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Location {
    pub x: String,
    pub y: String,
}

impl From<IVec2> for Location {
    fn from(parcel: IVec2) -> Self {
        Self {
            x: parcel.x.to_string(),
            y: parcel.y.to_string(),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct VisiblePerson {
    pub user_name: String,
    pub user_address: String,
    pub is_guest: bool,
    pub is_emoting: bool,
    /// Normalised to the saved image, origin top-left; zero when the person covers no area.
    pub screen_rect: ScreenRect,
    pub wearables: Vec<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Default)]
pub struct ScreenRect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

/// The share of the screen the photo covers, per axis, so the photo is 16:9 and `FRAME_SCALE`
/// of the screen's tighter dimension.
pub fn normalized_frame_size(screen_aspect_ratio: f32) -> Vec2 {
    if screen_aspect_ratio > TARGET_ASPECT_RATIO {
        Vec2::new(
            FRAME_SCALE * TARGET_ASPECT_RATIO / screen_aspect_ratio,
            FRAME_SCALE,
        )
    } else {
        Vec2::new(
            FRAME_SCALE,
            FRAME_SCALE * screen_aspect_ratio / TARGET_ASPECT_RATIO,
        )
    }
}

/// The vertical field of view of the cropped photo, from the screen camera's.
pub fn photo_fov(camera_fov: f32, frame: Vec2) -> f32 {
    ((camera_fov / 2.0).tan() * frame.y).atan() * 2.0
}

/// The frustum of the photo, for testing who is in it.
pub fn photo_frustum(camera: &GlobalTransform, camera_fov: f32, near: f32, frame: Vec2) -> Frustum {
    let projection = PerspectiveProjection {
        fov: photo_fov(camera_fov, frame),
        aspect_ratio: TARGET_ASPECT_RATIO,
        near,
        far: f32::INFINITY,
    };
    let clip_from_view = projection.get_clip_from_view();
    let view_from_world = camera.compute_matrix().inverse();
    Frustum::from_clip_from_world(&(clip_from_view * view_from_world))
}

/// An avatar's bounds: the player capsule standing at `feet`.
pub fn avatar_aabb(feet: Vec3) -> Aabb {
    Aabb::from_min_max(
        feet - Vec3::new(PLAYER_COLLIDER_RADIUS, 0.0, PLAYER_COLLIDER_RADIUS),
        feet + Vec3::new(
            PLAYER_COLLIDER_RADIUS,
            PLAYER_COLLIDER_HEIGHT,
            PLAYER_COLLIDER_RADIUS,
        ),
    )
}

pub fn in_frame(frustum: &Frustum, aabb: &Aabb) -> bool {
    frustum.intersects_obb(aabb, &Affine3A::IDENTITY, true, false)
}

/// Where `aabb` lands in the saved image: its corners in front of the near plane, and where its
/// edges cross it, projected through the screen camera and mapped into the photo frame.
pub fn screen_rect(
    aabb: &Aabb,
    camera: &GlobalTransform,
    clip_from_view: Mat4,
    near: f32,
    frame: Vec2,
) -> ScreenRect {
    let view_from_world = camera.affine().inverse();
    let min = Vec3::from(aabb.min());
    let max = Vec3::from(aabb.max());
    let corners: [Vec3; 8] = std::array::from_fn(|i| {
        Vec3::new(
            if i & 1 == 0 { min.x } else { max.x },
            if i & 2 == 0 { min.y } else { max.y },
            if i & 4 == 0 { min.z } else { max.z },
        )
    });
    // view space looks down -Z: depth is -z
    let view: Vec<Vec3> = corners
        .iter()
        .map(|c| view_from_world.transform_point3(*c))
        .collect();
    let in_front = |p: &Vec3| -p.z > near;

    let mut points: Vec<Vec3> = view.iter().copied().filter(in_front).collect();
    const EDGES: [(usize, usize); 12] = [
        (0, 1),
        (2, 3),
        (4, 5),
        (6, 7),
        (0, 2),
        (1, 3),
        (4, 6),
        (5, 7),
        (0, 4),
        (1, 5),
        (2, 6),
        (3, 7),
    ];
    for (a, b) in EDGES {
        let (pa, pb) = (view[a], view[b]);
        if in_front(&pa) != in_front(&pb) {
            let t = (-near - pa.z) / (pb.z - pa.z);
            let mut crossing = pa.lerp(pb, t);
            crossing.z = -near - f32::EPSILON;
            points.push(crossing);
        }
    }
    if points.is_empty() {
        return ScreenRect::default();
    }

    let mut lo = Vec2::splat(f32::MAX);
    let mut hi = Vec2::splat(f32::MIN);
    for point in points {
        let clip = clip_from_view * point.extend(1.0);
        if clip.w.abs() <= f32::EPSILON {
            continue;
        }
        let ndc = clip.xy() / clip.w;
        let viewport = Vec2::new((ndc.x + 1.0) * 0.5, (1.0 - ndc.y) * 0.5);
        let in_frame =
            ((viewport - (Vec2::splat(0.5) - frame * 0.5)) / frame).clamp(Vec2::ZERO, Vec2::ONE);
        lo = lo.min(in_frame);
        hi = hi.max(in_frame);
    }
    let size = hi - lo;
    if size.x <= 0.0 || size.y <= 0.0 {
        return ScreenRect::default();
    }
    ScreenRect {
        x: lo.x,
        y: lo.y,
        width: size.x,
        height: size.y,
    }
}

pub fn filter_non_base_wearables(wearables: &[String]) -> Vec<String> {
    wearables
        .iter()
        .filter(|urn| !urn.starts_with(BASE_WEARABLE_PREFIX))
        .cloned()
        .collect()
}

/// A `0x` address in the lowercase form the service compares against the signer.
pub fn lowercase_address(address: &str) -> String {
    address.to_ascii_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_wide_screen_keeps_the_frame_height_and_narrows_the_width() {
        let frame = normalized_frame_size(21.0 / 9.0);
        assert_eq!(frame.y, FRAME_SCALE);
        assert!((frame.x - FRAME_SCALE * TARGET_ASPECT_RATIO / (21.0 / 9.0)).abs() < 1e-6);
        let square = normalized_frame_size(1.0);
        assert_eq!(square.x, FRAME_SCALE);
        assert!((square.y - FRAME_SCALE / TARGET_ASPECT_RATIO).abs() < 1e-6);
        let exact = normalized_frame_size(TARGET_ASPECT_RATIO);
        assert!((exact - Vec2::splat(FRAME_SCALE)).length() < 1e-6);
    }

    #[test]
    fn the_photo_fov_is_narrower_than_the_screen_fov() {
        let fov = photo_fov(60f32.to_radians(), Vec2::splat(FRAME_SCALE));
        assert!(fov < 60f32.to_radians());
        assert!(fov > 50f32.to_radians());
    }

    #[test]
    fn people_in_front_are_in_frame_and_people_behind_are_not() {
        let camera = GlobalTransform::from(Transform::from_xyz(0.0, 1.8, 0.0));
        let frame = normalized_frame_size(16.0 / 9.0);
        let frustum = photo_frustum(&camera, 60f32.to_radians(), 0.1, frame);
        assert!(in_frame(&frustum, &avatar_aabb(Vec3::new(0.0, 0.0, -5.0))));
        assert!(!in_frame(&frustum, &avatar_aabb(Vec3::new(0.0, 0.0, 5.0))));
        assert!(!in_frame(
            &frustum,
            &avatar_aabb(Vec3::new(20.0, 0.0, -5.0))
        ));
    }

    #[test]
    fn a_centred_person_projects_to_the_middle_of_the_photo() {
        let camera = GlobalTransform::from(Transform::from_xyz(0.0, 0.9, 0.0));
        let projection = PerspectiveProjection {
            fov: 60f32.to_radians(),
            aspect_ratio: 16.0 / 9.0,
            near: 0.1,
            far: 1000.0,
        };
        let frame = normalized_frame_size(16.0 / 9.0);
        let rect = screen_rect(
            &avatar_aabb(Vec3::new(0.0, 0.0, -5.0)),
            &camera,
            projection.get_clip_from_view(),
            0.1,
            frame,
        );
        assert!(rect.width > 0.0 && rect.height > 0.0, "{rect:?}");
        let centre = Vec2::new(rect.x + rect.width / 2.0, rect.y + rect.height / 2.0);
        assert!((centre - Vec2::splat(0.5)).length() < 0.05, "{centre}");
        assert!(rect.height > rect.width);

        let behind = screen_rect(
            &avatar_aabb(Vec3::new(0.0, 0.0, 5.0)),
            &camera,
            projection.get_clip_from_view(),
            0.1,
            frame,
        );
        assert_eq!(behind, ScreenRect::default());
    }

    #[test]
    fn base_wearables_are_dropped() {
        let wearables = vec![
            "urn:decentraland:off-chain:base-avatars:f_sweater".to_owned(),
            "urn:decentraland:matic:collections-v2:0xabc:1".to_owned(),
        ];
        assert_eq!(
            filter_non_base_wearables(&wearables),
            vec!["urn:decentraland:matic:collections-v2:0xabc:1".to_owned()]
        );
    }

    #[test]
    fn the_metadata_serialises_with_the_service_field_names() {
        let metadata = ScreenshotMetadata {
            user_name: "me".into(),
            user_address: "0xabc".into(),
            date_time: "1700000000".into(),
            realm: "main".into(),
            scene: Scene {
                name: "Plaza".into(),
                location: IVec2::new(-1, 2).into(),
            },
            visible_people: vec![VisiblePerson {
                user_name: "you".into(),
                user_address: "0xdef".into(),
                is_guest: true,
                is_emoting: false,
                screen_rect: ScreenRect::default(),
                wearables: vec![],
            }],
            place_id: "place".into(),
        };
        let json = serde_json::to_value(&metadata).unwrap();
        assert_eq!(json["userAddress"], "0xabc");
        assert_eq!(json["scene"]["location"]["x"], "-1");
        assert_eq!(json["scene"]["location"]["y"], "2");
        assert_eq!(json["visiblePeople"][0]["isGuest"], true);
        assert_eq!(json["visiblePeople"][0]["screenRect"]["width"], 0.0);
        assert_eq!(json["placeId"], "place");
        assert_eq!(json["dateTime"], "1700000000");
    }
}

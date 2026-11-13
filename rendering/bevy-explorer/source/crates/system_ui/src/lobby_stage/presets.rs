//! The lobby stage's looks. Positions and rotations are in the stage's own frame: the origin one
//! metre above the avatar's feet, +x to the camera's right, +y up, +z towards the camera.

use bevy::{math::EulerRot, prelude::*};

/// A set piece on the stage. `position.y` is measured from the floor surface.
pub struct StageProp {
    pub model: &'static str,
    pub position: Vec3,
    /// Degrees, applied about z, then x, then y.
    pub rotation: Vec3,
    pub scale: f32,
}

pub struct StagePreset {
    pub id: &'static str,
    pub backdrop: &'static str,
    /// Screen height, from the bottom, where the floor has fully dissolved into the backdrop.
    pub blend_screen_height: f32,
    /// Metres of floor, towards the camera from the blend line, the dissolve takes.
    pub blend_depth: f32,
    /// 0 dissolves smoothly over the whole depth, 1 cuts hard at its middle.
    pub blend_hardness: f32,
    /// Share of the backdrop's height kept below the blend line.
    pub image_below_blend: f32,
    pub floor_color: Color,
    pub spot_color: Color,
    pub mist: Option<Color>,
    pub dust: Option<Color>,
    /// Degrees, applied about z, then x, then y, from a light shining along -z.
    pub light_rotation: Vec3,
    pub light_color: Color,
    pub light_intensity: f32,
    pub vignette: Vignette,
    pub props: &'static [StageProp],
}

#[derive(Clone, Copy)]
pub struct Vignette {
    /// How dark the frame's rim gets; 0 turns the vignette off.
    pub intensity: f32,
    /// How far in from the corners the darkening reaches, 1 being all the way to the centre.
    pub smoothness: f32,
    pub color: Color,
    /// The protected oval's centre, in screen units from the bottom left.
    pub mask_center: Vec2,
    /// Half width, reach above the centre and reach below it, in screen units.
    pub mask_radii: Vec3,
    pub mask_softness: f32,
}

const VIGNETTE: Vignette = Vignette {
    intensity: 1.0,
    smoothness: 1.0,
    color: Color::srgba(0.0, 0.0, 0.0, 1.0),
    mask_center: Vec2::new(0.5, 0.16),
    mask_radii: Vec3::new(0.27, 0.65, 0.3),
    mask_softness: 0.43,
};

// The source rig is left-handed with the camera on +z; seen through the camera, its +x is this
// frame's -x. Each position is given in the source's terms and mirrored by `prop`.
const fn prop(
    model: &'static str,
    x: f32,
    y: f32,
    z: f32,
    rotation: Vec3,
    scale: f32,
) -> StageProp {
    StageProp {
        model,
        position: Vec3::new(-x, y, z),
        rotation,
        scale,
    }
}

const fn euler(x: f32, y: f32, z: f32) -> Vec3 {
    Vec3::new(x, y, z)
}

pub const PRESETS: &[StagePreset] = &[
    StagePreset {
        id: "halloween-2026",
        backdrop: "embedded://lobby_stage/halloween_backdrop.jpg",
        blend_screen_height: 0.445,
        blend_depth: 4.85,
        blend_hardness: 0.342,
        image_below_blend: 0.598,
        floor_color: Color::srgba(0.23166609, 0.29725108, 0.3584906, 1.0),
        spot_color: Color::srgba(0.5943396, 0.35593525, 0.31679422, 1.0),
        mist: Some(Color::srgba(0.33664113, 0.42959112, 0.8018868, 0.7882353)),
        dust: Some(Color::srgba(0.8867924, 0.8867924, 0.8867924, 0.55)),
        light_rotation: euler(45.0, 120.0, 0.0),
        light_color: Color::srgba(0.8915094, 0.9544319, 1.0, 1.0),
        light_intensity: 2.0,
        vignette: VIGNETTE,
        props: &[
            prop("candle03", -1.63, 0.0, -1.1, euler(0.0, 1.11, 0.0), 0.6),
            prop("pumpkin4", -0.82, 0.0, -1.31, euler(0.0, 5.01, 0.0), 0.4),
            prop("pumpkin3", -1.19, 0.0, -0.69, euler(-5.6, 15.2, 0.0), 0.3),
            prop("candle03", 1.68, 0.0, -1.15, euler(0.0, 301.0, 0.0), 0.5),
            prop("candle04", 1.19, 0.0, -0.74, euler(0.0, 0.0, 0.0), 0.3),
            prop("pumpkin1", 0.93, 0.0, -0.96, euler(0.0, 0.16, 0.0), 0.3),
            prop("pumpkin2", 1.33, 0.0, -1.32, euler(0.0, -11.35, 0.0), 0.4),
        ],
    },
    StagePreset {
        id: "game-world",
        backdrop: "embedded://lobby_stage/game_world_backdrop.jpg",
        blend_screen_height: 0.445,
        blend_depth: 4.85,
        blend_hardness: 0.342,
        image_below_blend: 0.461,
        floor_color: Color::srgba(1.0, 0.2787464, 0.16509432, 1.0),
        spot_color: Color::srgba(0.48568767, 0.22329085, 0.18171112, 1.0),
        mist: Some(Color::srgba(0.6037736, 0.3223843, 0.27625492, 0.7882353)),
        dust: Some(Color::srgba(1.0, 0.96, 0.9, 0.55)),
        light_rotation: euler(45.0, 120.0, 0.0),
        light_color: Color::srgba(0.8915094, 0.9544319, 1.0, 1.0),
        light_intensity: 2.0,
        vignette: VIGNETTE,
        props: &[
            prop("rock01", 2.6, -0.02, -1.53, euler(0.0, 223.59, 0.0), 0.4),
            prop("rock02", -1.96, -0.1, -1.48, euler(-65.16, 566.5, 0.0), 0.6),
            prop("rock03", -1.46, 0.0, -0.84, euler(-65.16, 487.9, 0.0), 0.2),
            prop("rock03", 2.06, 0.0, -1.19, euler(116.2, 631.8, 0.0), 0.2),
            prop("rock03", 0.65, -0.03, -1.46, euler(81.8, 734.3, 0.0), 0.3),
        ],
    },
];

/// The preset with this id, or the first one.
pub fn preset(id: &str) -> &'static StagePreset {
    PRESETS.iter().find(|p| p.id == id).unwrap_or(&PRESETS[0])
}

/// A rotation given as the source's left-handed euler angles (z, then x, then y), seen in this
/// frame: the source's x axis points the other way, so the rotation is mirrored across x.
pub fn mirrored_rotation(degrees: Vec3) -> Quat {
    let q = Quat::from_euler(
        EulerRot::YXZ,
        degrees.y.to_radians(),
        degrees.x.to_radians(),
        degrees.z.to_radians(),
    );
    Quat::from_xyzw(q.x, -q.y, -q.z, q.w)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_preset_falls_back_to_the_first() {
        assert_eq!(preset("nope").id, "halloween-2026");
        assert_eq!(preset("game-world").id, "game-world");
    }

    #[test]
    fn mirrored_rotation_turns_yaw_the_other_way() {
        let q = mirrored_rotation(Vec3::new(0.0, 90.0, 0.0));
        // a source yaw of +90 turns +z towards the source's +x, which is this frame's -x
        let forward = q * Vec3::Z;
        assert!((forward - Vec3::NEG_X).length() < 1e-5, "{forward}");
    }

    #[test]
    fn mirrored_rotation_keeps_pitch() {
        let q = mirrored_rotation(Vec3::new(45.0, 0.0, 0.0));
        let forward = q * Vec3::Z;
        // a positive pitch about x tips +z downwards in a left-handed frame
        assert!(forward.y < 0.0 && forward.z > 0.0, "{forward}");
    }
}

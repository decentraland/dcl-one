import { Vector3 } from "@dcl/sdk/math";

// m/s^2. A jump's rise slows at JUMP_RISE_DECEL, not at this.
export const GRAVITY = Vector3.scale(Vector3.Down(), 10.0);

export const WALK_SPEED = 1.5; // max walk speed
export const JOG_SPEED = 8; // max run speed
export const SPRINT_SPEED = 10; // max sprint speed

export const MAX_STEPS = 16; // steps per tick at most. A longer hitch takes longer steps.
// Acceleration of each input axis (m/s^2). It rises from the low to the max value as the input-hold weight goes from
// ACCEL_CURVE_START to ACCEL_CURVE_END.
export const GROUND_ACCEL = 20;
export const GROUND_MAX_ACCEL = 25;
export const AIR_ACCEL = 15;
export const AIR_MAX_ACCEL = 20;
export const ACCEL_CURVE_START = 0.1;
export const ACCEL_CURVE_END = 0.9;
export const HORIZONTAL_ACCEL_TIME_GROUND = 0.5; // s for the input-hold weight to go 0 -> 1
// s for a released axis to settle. 0 stops the axis within one step.
export const HORIZONTAL_DAMP_TIME_GROUND = 0;
// 1/m, airborne: dv/dt = -AIR_DRAG * |v| * v.
export const AIR_DRAG = 0.2;
// 1/s, damping of the horizontal velocity of scene pushes.
export const EXTERNAL_ENV_DRAG = 1.5;
export const EXTERNAL_GROUND_FRICTION = 4;
export const MAX_EXTERNAL_SPEED = 50;
export const MAX_TICK_TIME = 0.25; // s, longest tick the model integrates
// While the vertical speed of pushes and gravity points up, it fades with VERTICAL_DAMP_TIME (s) and slows by
// VERTICAL_STOP_DECEL (m/s^2) on top of gravity. A fall is not damped.
export const VERTICAL_DAMP_TIME = 0.75;
export const VERTICAL_STOP_DECEL = 5;

// Hard cap on total velocity magnitude — applied after external velocity is
// added each frame, so runaway accumulation from repeated external impulses
// can't produce absurd speeds.
export const MAX_SPEED = 30;

// Apex (m) of a jump released at once, standing and at full sprint speed. AvatarLocomotionSettings overrides both.
export const JUMP_HEIGHT = 1;
export const JUMP_HEIGHT_SPRINT = 1.5;
export const JUMP_RISE_DECEL = 40; // m/s^2, the jump's own slowdown up to the apex
// For JUMP_HOLD_TIME (s) after the press, a held key scales the jump's slowdown, and then the fall, by JUMP_HOLD_SCALE.
export const JUMP_HOLD_TIME = 0.52; // s
export const JUMP_HOLD_SCALE = 0.5;
// A press up to JUMP_BUFFER_TIME before a landing jumps on the landing. A press up to JUMP_COYOTE_TIME after the
// avatar leaves the ground still jumps from the ground. Neither applies while the avatar moves up.
export const JUMP_BUFFER_TIME = 0.16; // s
export const JUMP_COYOTE_TIME = 0.14; // s
// Air jump: a fresh press in the air holds the height for AIR_JUMP_HOVER_TIME, then launches to DOUBLE_JUMP_HEIGHT.
export const DOUBLE_JUMP_HEIGHT = 2; // m, AvatarLocomotionSettings overrides it
export const AIR_JUMP_COUNT = 1; // air jumps per airtime
export const AIR_JUMP_COOLDOWN = 0.32; // s from the previous jump's press before an air jump can start
export const AIR_JUMP_HOVER_TIME = 0.2; // s the height is held from the end of the press step to the launch step
export const AIR_JUMP_HOVER_SPEED = 0; // m/s
export const AIR_JUMP_IMPULSE = 8; // m/s, the least horizontal speed of an air jump's launch
export const GLIDE_FALL_SPEED = 1; // max fall speed while gliding (m/s)
export const GLIDE_DAMP_TIME = 0.1; // s, time constant of the glider's drag on the vertical speed of scene pushes
export const GLIDE_HORIZONTAL_SPEED = 6; // horizontal speed limit while gliding (m/s)
export const GLIDE_MIN_GROUND_DISTANCE = 0.12; // m above the ground that the glider needs to open and to stay open
export const JUMP_TO_GLIDE_TIME = 0.52; // s from the latest jump's press before the glider can open
export const GLIDE_COOLDOWN = 0.22; // s from the glider's close before it can open again
// s from the press to the first gliding step: the length of the opening clip, plus one frame
export const GLIDE_OPEN_TIME = 0.36;
export const GLIDE_CLOSE_TIME = 0.28; // s, the length of the closing clip
export const GLIDE_TILT_DAMP_TIME = 0.3; // time constant for the glide tilt (animation playback time) to drift toward its target
export const GLIDE_TILT_FULL_ANGLE = 20; // orientation delta (degrees) at which the glide tilt reaches its full-left/full-right pose
// Landing stun: a landing more than LANDING_STUN_DROP below the top of the airtime stops horizontal movement for
// hardLandingCooldown (AvatarLocomotionSettings), or LANDING_STUN_TIME by default. Jumping is still allowed.
export const LANDING_STUN_DROP = 8; // m
export const LANDING_STUN_TIME = 0.76; // s from the landing step

export const TURN_MAX_DEGREES_SEC = 1040; // max degrees to turn (set to inf for TURN_FULL_TIME to apply)
export const TURN_FULL_TIME = 0.1; // time to turn fully towards target

export const GROUNDED_HEIGHT = 0.05; // distance from surface at which player is considered "grounded"
// Steepest walkable slope in degrees (can jump, will not slide). The half degree absorbs single-precision normals, so
// 46 degrees is walkable and 47 is not.
export const GROUNDED_ANGLE = 46.5;
// On the ground the gait speed scales by 1 - SLOPE_SPEED_FACTOR * angle / SLOPE_SPEED_MAX_ANGLE. The angle is the
// slope along the move: positive uphill, clamped to +-SLOPE_SPEED_MAX_ANGLE.
export const SLOPE_SPEED_FACTOR = 0.35;
export const SLOPE_SPEED_MAX_ANGLE = 55; // degrees
export const MAX_STEP_HEIGHT = 0.45; // highest step player can walk up (player may still walk up slightly higher steps if approached at an angle)
export const MIN_STEP_HEIGHT = 0.05; // rises up to this need no step (the capsule slides over them)
export const STEP_UP_PLAN_SPEED = 5.5; // m/s, the least speed that plans the climb ahead of the riser
export const STEP_UP_REACH_TIME = 0.1; // s ahead of the riser
export const STEP_UP_TIME = 0.1; // s, a slower climb from the riser's face
export const GROUND_SNAP_HEIGHT = 0.40 // drop ahead at which the engine walk stops (walk.ts)
// m. On the ground the avatar follows a drop under its centre up to this much beyond the slope. A larger drop is an edge.
export const GROUND_SNAP_STEP_DROP = 0.15;
// m below the capsule that the ground casts reach. The landing prediction, the glide's least height and the landing
// stun measure from that ground. Further down the world floor (y 0) stands in.
export const GROUND_CAST_RANGE = 3.6;


// don't edit

export const PLAYER_COLLIDER_RADIUS = 0.3;
export const VEC3_ZERO = Vector3.Zero();
export const VEC3_UP = Vector3.Up();
export const VEC3_DOWN = Vector3.Down();
export const VEC3_FORWARD = Vector3.Forward();
export const VEC3_RIGHT = Vector3.Right();
export const VEC3_HORIZONTAL_MASK = Vector3.create(1, 0, 1);

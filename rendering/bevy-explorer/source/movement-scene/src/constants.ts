import { Vector3 } from "@dcl/sdk/math";

// The one gravity of the world (m/s^2): falls, scene pushes and forces, and the avatar from a jump's apex on. A jump's
// rise is the player's own move (JUMP_RISE_DECEL), not a different gravity.
export const GRAVITY = Vector3.scale(Vector3.Down(), 10.0);

export const WALK_SPEED = 1.5; // max walk speed
export const JOG_SPEED = 8; // max run speed
export const SPRINT_SPEED = 10; // max sprint speed

// Horizontal input model. It is integrated in fixed steps inside each scene tick, so the result
// does not depend on the frame rate or on how often the scene gets to answer.
export const FIXED_STEP = 0.02; // s, length of one integration step (50 Hz)
export const MAX_FIXED_STEPS = 13; // integration steps per tick at most (bounds a hitch)
// Per input axis: a held key moves the axis speed toward its target at this acceleration (m/s^2).
// The acceleration blends from the low to the high value as the input-hold weight goes from
// ACCEL_CURVE_START to ACCEL_CURVE_END (smoothstep); the weight ramps over the accel time.
export const GROUND_ACCEL = 20;
export const GROUND_MAX_ACCEL = 25;
export const AIR_ACCEL = 15;
export const AIR_MAX_ACCEL = 20;
export const ACCEL_CURVE_START = 0.1;
export const ACCEL_CURVE_END = 0.9;
export const HORIZONTAL_ACCEL_TIME_GROUND = 0.5; // s for the input-hold weight to go 0 -> 1
// Smoothing time (s) of an axis whose key is released. 0 stops the axis within one step.
export const HORIZONTAL_DAMP_TIME_GROUND = 0;
// Quadratic horizontal drag while airborne (1/m): dv/dt = -AIR_DRAG * |v| * v.
export const AIR_DRAG = 0.2;
// Linear damping of external (impulse/force) horizontal velocity (1/s): always, plus on ground.
export const EXTERNAL_ENV_DRAG = 1.5;
export const EXTERNAL_GROUND_FRICTION = 4;
export const MAX_EXTERNAL_SPEED = 50;
// Steps up to the current frame use the input of the previous tick.
// Reply prediction: the engine applies a reply from the frame it lands on until the next one. The reply delay in frames
// is estimated from previousStepTime / stepTime, and the reply carries the state predicted for the last frame it is held.
export const FRAME_TIME_SMOOTHING = 0.1; // weight of the newest sample in the frame-time average
export const MAX_TICK_TIME = 0.25; // s, longest tick the model integrates
// Ramp guard, against the prediction switching between ticks: while the same input is held on the ground and the model
// accelerates, a reply never slows below the previous one; between two airborne replies (no launch, glide, landing or
// push), vy never rises.
// Vertical reply in the air: the velocity predicted 1.5 x the smoothed delay ahead (replyDelaySmooth), which only falls
// between replies. A launch from the ground uses the mean predicted velocity over the frames the reply is held, from
// the per-tick delay estimate.
// Height tracking during a jump: the engine integrates whatever velocity it is sent, so a reply that lands earlier
// or later than estimated leaves a height error that adds up over the jump. Each reply adds the error between the
// model's height and the avatar's, divided by VERTICAL_TRACK_TIME (s), at most VERTICAL_TRACK_MAX (m/s).
export const VERTICAL_TRACK_TIME: number = 0.15;
export const VERTICAL_TRACK_MAX: number = 2;
// Height tracking stops for the rest of the jump when the applied vertical velocity differs from the requested one by
// more than this (m/s): the avatar hit something.
export const VERTICAL_TRACK_BLOCK: number = 0.5;
// Scene pushes (impulses, forces) and gravity act on one vertical speed. While that speed points up it fades with
// this time constant and loses a constant deceleration on top of gravity; a fall keeps its speed until the ground
// stops it.
export const VERTICAL_DAMP_TIME = 0.75;
export const VERTICAL_STOP_DECEL = 5;

// Hard cap on total velocity magnitude — applied after external velocity is
// added each frame, so runaway accumulation from repeated external impulses
// can't produce absurd speeds.
export const MAX_SPEED = 30;

// Jump: the player's own move on top of GRAVITY. It launches at the speed that reaches its height while slowing at
// JUMP_RISE_DECEL, so a 1 m jump peaks after 0.22 s (1.5 m: 0.27 s, 2 m: 0.32 s); from the apex the avatar falls
// under GRAVITY. Heights are those of a jump released at once (AvatarLocomotionSettings jumpHeight, runJumpHeight,
// doubleJumpHeight); a key held into JUMP_HOLD_TIME adds height, and a running jump blends toward JUMP_HEIGHT_SPRINT with speed.
export const JUMP_HEIGHT = 1;
export const JUMP_HEIGHT_SPRINT = 1.5;
export const JUMP_RISE_DECEL = 40; // m/s^2, the jump's own slowdown from the launch to the apex
// Held jump: while the key is held, for JUMP_HOLD_TIME after the press, the player's own vertical acceleration (the
// jump's slowdown, then the fall) is scaled by JUMP_HOLD_SCALE. A held ground jump rises about twice as high in twice
// the time.
export const JUMP_HOLD_TIME = 0.52; // s (26 steps)
export const JUMP_HOLD_SCALE = 0.5;
// Steps a press is buffered before landing, and steps after leaving the ground that a jump
// is still allowed (one window for both; off while moving up).
export const JUMP_GRACE_STEPS = 8;
// Air jump: one per airtime, by a fresh press in the air after the ground jump (or once the grace window of a fall
// off an edge has passed). The avatar holds its height for AIR_JUMP_HOVER_TIME (a flat hover, no height tracking),
// then launches to DOUBLE_JUMP_HEIGHT (any gait). A press whose step reaches the ground is lost: the landing cancels
// the air jump. The held-key window (JUMP_HOLD_TIME) counts from the air press, so the hover uses part of it, and any
// jump key held in it counts. Scenes turn it off with InputModifier disableDoubleJump.
export const DOUBLE_JUMP_HEIGHT = 2; // in-air jump height (m), overridden by the locomotion settings
export const AIR_JUMP_COUNT = 1; // air jumps per airtime
export const AIR_JUMP_COOLDOWN = 0.32; // s from the previous jump's press before an air jump can start (16 steps)
export const AIR_JUMP_HOVER_TIME = 0.2; // s the height is held between the press and the launch (whole steps)
export const AIR_JUMP_HOVER_SPEED = 0; // vertical speed during the hover (m/s)
// Horizontal speed at the launch (m/s): max(AIR_JUMP_IMPULSE, current speed) along the keys held on the launch step;
// no movement key held stops the horizontal move. 0 keeps the current speed (turned toward the keys).
export const AIR_JUMP_IMPULSE = 8;
// Glide: a fresh press in the air once the jumps are spent (the air jump included) opens the glider's prop, once long
// enough after the latest jump's press, above the ground and with the prop shut. The fall goes on while the prop opens;
// once open, the glide holds the fall while the key is held. Released, or near the ground, the prop closes.
export const GLIDE_FALL_SPEED = 1; // max fall speed while gliding (m/s)
// While gliding, the glider's drag takes what scene pushes add to the vertical speed, up or down, with this time
// constant (s); the glide's own fall is held as above.
export const GLIDE_DAMP_TIME = 0.1;
export const GLIDE_HORIZONTAL_SPEED = 6; // horizontal speed limit while gliding (m/s)
export const GLIDE_MIN_GROUND_DISTANCE = 0.12; // model height above the ground (m)
export const JUMP_TO_GLIDE_TIME = 0.52; // s from the latest jump's press (26 steps)
export const GLIDE_COOLDOWN = 0.22; // s after the prop has closed (11 steps)
// s from the press that opens the prop to the first step that glides (18 steps: the 0.333 s opening clip, seen a frame
// later)
export const GLIDE_OPEN_TIME = 0.36;
export const GLIDE_CLOSE_TIME = 0.28; // s the prop takes to close (14 steps: the 0.283 s closing clip)
// A prop that starts opening opens fully: a tap opens it, then it closes without gliding; a landing while it opens lets
// it open, and it closes on the next step.
export const GLIDE_TILT_DAMP_TIME = 0.3; // time constant for the glide tilt (animation playback time) to drift toward its target
export const GLIDE_TILT_FULL_ANGLE = 20; // orientation delta (degrees) at which the glide tilt reaches its full-left/full-right pose
// Landing stun: a landing more than LANDING_STUN_DROP below the top of the airtime (the highest point since the ground;
// a glide restarts it from the glide's height) stops horizontal movement for the scene's hardLandingCooldown
// (AvatarLocomotionSettings), LANDING_STUN_TIME when no scene sets it. Jumping is still allowed. The keys held through
// the stun still accelerate the move, so the gait speed is back on the first step after it (in the air the move then
// builds up from 0 at the air acceleration). Placing the avatar (a teleport) keeps the top: a fall from where it was
// placed counts from the ground it stood on before or from a top it rises to later, so a teleport high up never stuns.
// The hard landing clips play only on a stunning landing out of the long fall loop (other stunning landings play the
// usual landing clip).
export const LANDING_STUN_DROP = 8; // m
export const LANDING_STUN_TIME = 0.76; // s from the landing step (38 steps)

export const TURN_MAX_DEGREES_SEC = 1040; // max degrees to turn (set to inf for TURN_FULL_TIME to apply)
export const TURN_FULL_TIME = 0.1; // time to turn fully towards target

export const GROUNDED_HEIGHT = 0.05; // distance from surface at which player is considered "grounded"
// Angle (0-90) from flat up to which ground is ground (can jump / won't slide): a 46 degree slope is walkable, 47 is
// steep (the half degree absorbs single-precision normals).
export const GROUNDED_ANGLE = 46.5;
// Speed on a slope: on the ground the gait speed is scaled by 1 - SLOPE_SPEED_FACTOR * angle / SLOPE_SPEED_MAX_ANGLE,
// with the angle of the ground along the move (positive uphill, clamped to +-SLOPE_SPEED_MAX_ANGLE): uphill slower,
// downhill faster, across unchanged. It scales the horizontal speed; on the ground the engine keeps the avatar on the
// slope. In the air the factor of the last ground holds.
export const SLOPE_SPEED_FACTOR = 0.35;
export const SLOPE_SPEED_MAX_ANGLE = 55; // degrees
export const MAX_STEP_HEIGHT = 0.45; // highest step player can walk up (player may still walk up slightly higher steps if approached at an angle)
export const MIN_STEP_HEIGHT = 0.05; // rises up to this need no step (the capsule slides over them)
export const GROUND_SNAP_HEIGHT = 0.40 // drop ahead at which the engine walk stops (walk.ts)
// The ground follows a walking avatar down at most GROUND_SNAP_STEP_DROP beyond its slope; past a larger drop the centre
// is over an edge.
export const GROUND_SNAP_STEP_DROP = 0.15; // m
// How far below the capsule's lower sphere the ground casts reach. Within it the scene knows the ground under an
// avatar in the air: the landing prediction, the glide's minimum height and the landing stun measure from it. Below it
// the casts find nothing and the world floor (y 0) stands in.
export const GROUND_CAST_RANGE = 3.6; // m


// don't edit

export const PLAYER_COLLIDER_RADIUS = 0.3;
export const VEC3_ZERO = Vector3.Zero();
export const VEC3_UP = Vector3.Up();
export const VEC3_DOWN = Vector3.Down();
export const VEC3_FORWARD = Vector3.Forward();
export const VEC3_RIGHT = Vector3.Right();
export const VEC3_HORIZONTAL_MASK = Vector3.create(1, 0, 1);

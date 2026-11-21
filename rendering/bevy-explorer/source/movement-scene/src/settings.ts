// Runtime values the movement and animation code reads as `settings.*`. The
// speeds and jump heights are seeded from constants.ts.
import {
  WALK_SPEED, JOG_SPEED, SPRINT_SPEED,
  HORIZONTAL_ACCEL_TIME_GROUND, HORIZONTAL_DAMP_TIME_GROUND,
  JUMP_HEIGHT, JUMP_HEIGHT_SPRINT,
} from './constants';

export const settings = {
  // --- Movement speeds ---
  walkSpeed: WALK_SPEED,
  jogSpeed: JOG_SPEED,
  sprintSpeed: SPRINT_SPEED,
  // Time (s) for the input-hold weight to ramp 0 -> 1; the acceleration blends from its low to
  // its high value with that weight.
  accelTimeGround: HORIZONTAL_ACCEL_TIME_GROUND,
  // Smoothing time (s) of a released input axis; 0 stops within one integration step.
  dampTimeGround: HORIZONTAL_DAMP_TIME_GROUND,

  // --- Animation phasing ---
  // Horizontal speed (m/s) above which the run clip is used instead of walk.
  walkRunThreshold: 2.6,
  // Horizontal speed (m/s) above which we leave idle and play walk/run.
  moveGate: 0.1,
  sprintThreshold: 9.5,          // h-speed boundary: jog clip below, run clip above
  // Cross-fade time (s) into each clip. Locomotion blends are ~0.2 (smooth gait
  // changes); air/jump phase transitions are short so the brief one-shot clips
  // (Start/Mid/End ~0.2-0.4s) aren't swallowed by the blend.
  transWalk: 0.2,
  transRun: 0.2,
  transIdle: 0.2,
  transAir: 0.1, // jump phases / glide / double-jump / landing
  // Minimum sustained jog+ time (s) before stopping plays the Jog_Stop clip.
  // Short taps / repositioning moves settle straight into idle instead of
  // foot-planting after two steps.
  jogStopMinRun: 0.7,
  // Jog_Stop playback speed — higher = quicker foot-plant, less visible step.
  jogStopSpeed: 1,
  // Playback time (s) the Jog_Stop clip starts at — skips the clip's opening
  // step, which otherwise reads as a shuffle (0.1 tuned in testing).
  jogStopStart: 0.1,
  // --- Glide pose selection ---
  // Turn rate (deg/s) of the glide's facing at which the bank reaches its full angle (GLIDE_TILT_FULL_ANGLE); a
  // gentle glide turn banks a fraction of it, a tight spiral fully.
  glideLeanRate: 300,
  // h-speed (m/s) above which glide uses the forward (pitched) pose; below it
  // uses the upright/idle pose ("lean back").
  glideForwardSpeed: 3,
  // Crossfade time (s) between the forward and idle glide poses (GLIDE_AVATAR in index.ts).
  // Higher = a lazier, smoother swap.
  gliderFade: 0.5,

  // --- Jump look & feel ---
  // Apex (m) of a tapped standing jump / full-speed sprint jump; holding the key reaches about
  // twice as high.
  jumpHeight: JUMP_HEIGHT,
  sprintJumpHeight: JUMP_HEIGHT_SPRINT,
};

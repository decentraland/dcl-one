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
  accelTimeGround: HORIZONTAL_ACCEL_TIME_GROUND,
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
  // --- Glide pose selection ---
  // Turn rate (deg/s) at which the glide bank reaches its full angle (GLIDE_TILT_FULL_ANGLE).
  glideLeanRate: 300,
  // h-speed (m/s) above which glide uses the forward (pitched) pose; below it
  // uses the upright/idle pose ("lean back").
  glideForwardSpeed: 3,
  // Crossfade time (s) between the forward and idle glide poses.
  gliderFade: 0.5,

  // --- Jump look & feel ---
  jumpHeight: JUMP_HEIGHT,
  sprintJumpHeight: JUMP_HEIGHT_SPRINT,
};

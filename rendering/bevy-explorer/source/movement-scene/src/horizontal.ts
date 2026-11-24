import { engine, InputAction, inputSystem, Transform } from '@dcl/sdk/ecs'
import { Quaternion, Vector3 } from '@dcl/sdk/math';
import {
  AIR_JUMP_IMPULSE, ACCEL_CURVE_END, ACCEL_CURVE_START, AIR_ACCEL, AIR_DRAG, AIR_MAX_ACCEL, AIR_SPEED_LOSS, EXTERNAL_ENV_DRAG, EXTERNAL_GROUND_FRICTION,
  GROUND_ACCEL, GROUND_MAX_ACCEL, MAX_EXTERNAL_SPEED,
  SLOPE_SPEED_FACTOR, SLOPE_SPEED_MAX_ANGLE, TURN_FULL_TIME, TURN_MAX_DEGREES_SEC, VEC3_FORWARD, VEC3_HORIZONTAL_MASK, VEC3_RIGHT,
  VEC3_UP, VEC3_ZERO,
} from './constants';
import { horizontalResync, playerRotation, prevActualVelocity, prevExternalVelocity, tickTime, velocity } from '.';
import { grounded, GROUNDED_ANGLE_Y_LEN, groundNormal, groundRayNormal } from './ground';
import { disableOrientation, glidingSpeed, jogSpeed, sprintSpeed, walkSpeed } from './parameters';
import { airLaunchTime, isGliding, landingStuns, launchAhead, leftEdge, predictedLandFrame, stunEndTime, stunned, wallContact } from './vertical';
import { getWalkAxis } from './walk';
import { frameTime, oneStep, replyDelay, stepsFor, tickAdvance } from './clock';
import { settings } from './settings';

export var orientation = 0;
// World-space unit direction of the movement input (zero when there is none).
export var movementAxis = Vector3.Zero();
var horizontalVelocity = Vector3.Zero();

// ---------------------------------------------------------------- input

// Camera-relative basis (horizontal, unit) and input of one tick.
var fwd = Vector3.Zero();
var right = Vector3.Zero();
// stun: a landing stun holds from the first step (the steps after a predicted touchdown)
type StepInput = { x: number; z: number; limit: number; grounded: boolean; stun: boolean };
const input: StepInput = { x: 0, z: 0, limit: 0, grounded: true, stun: false };
// Input of the previous tick: the steps that cover the time up to this tick's frame use it.
const prevInput: StepInput = { x: 0, z: 0, limit: 0, grounded: true, stun: false };
const landInput: StepInput = { x: 0, z: 0, limit: 0, grounded: true, stun: false };

function readInput() {
  const camera = Transform.get(engine.CameraEntity);
  Vector3.rotateToRef(VEC3_FORWARD, camera.rotation, fwd);
  Vector3.multiplyToRef(fwd, VEC3_HORIZONTAL_MASK, fwd);
  Vector3.normalizeToRef(fwd, fwd);
  Vector3.rotateToRef(VEC3_RIGHT, camera.rotation, right);
  Vector3.multiplyToRef(right, VEC3_HORIZONTAL_MASK, right);
  Vector3.normalizeToRef(right, right);

  // Auto-walk takes priority; getWalkAxis() returns null (and cancels the walk) on manual input.
  const walkAxis = getWalkAxis();
  if (walkAxis !== null) {
    Vector3.copyFrom(walkAxis, movementAxis);
  } else {
    Vector3.copyFrom(VEC3_ZERO, movementAxis);
    if (inputSystem.isPressed(InputAction.IA_LEFT)) Vector3.subtractToRef(movementAxis, right, movementAxis);
    if (inputSystem.isPressed(InputAction.IA_RIGHT)) Vector3.addToRef(movementAxis, right, movementAxis);
    if (inputSystem.isPressed(InputAction.IA_FORWARD)) Vector3.addToRef(movementAxis, fwd, movementAxis);
    if (inputSystem.isPressed(InputAction.IA_BACKWARD)) Vector3.subtractToRef(movementAxis, fwd, movementAxis);
    Vector3.normalizeToRef(movementAxis, movementAxis);
  }

  input.x = snap(Vector3.dot(movementAxis, right));
  input.z = snap(Vector3.dot(movementAxis, fwd));
  // Each held key drives its own axis at full speed: a diagonal is not shortened here, the step cuts the move to the
  // gait's speed. Adding or releasing a side key while running then only turns the move; shortened axes would have one
  // slow down while the other speeds up, a dip of about a fifth of the speed.
  if (walkAxis === null && input.x !== 0 && input.z !== 0) {
    input.x = Math.sign(input.x);
    input.z = Math.sign(input.z);
  }
  input.limit = isGliding ? glidingSpeed
    : inputSystem.isPressed(InputAction.IA_MODIFIER) ? sprintSpeed
    : inputSystem.isPressed(InputAction.IA_WALK) ? walkSpeed
    : jogSpeed;
  input.grounded = grounded;
  if (!isGliding) input.limit *= slopeSpeedFactor();
}

// Gait speed factor of the slope along the move (SLOPE_SPEED_FACTOR), from the walkable ground the capsule rests on (on
// a step edge, the edge's slope), else the ground under the centre; the factor of the last ground holds in the air.
var slopeFactor = 1;
function slopeSpeedFactor(): number {
  const n = groundNormal.y >= GROUNDED_ANGLE_Y_LEN ? groundNormal : groundRayNormal;
  if (grounded && n.y >= GROUNDED_ANGLE_Y_LEN && Vector3.lengthSquared(movementAxis) > 0) {
    // the move direction projected onto the ground plane: its angle above the horizontal
    const un = movementAxis.x * n.x + movementAxis.z * n.z;
    const angle = Math.atan2(-un * n.y, Math.hypot(movementAxis.x - un * n.x, movementAxis.z - un * n.z)) * 180 / Math.PI;
    const clamped = Math.max(-SLOPE_SPEED_MAX_ANGLE, Math.min(SLOPE_SPEED_MAX_ANGLE, angle));
    slopeFactor = 1 - SLOPE_SPEED_FACTOR * clamped / SLOPE_SPEED_MAX_ANGLE;
  }
  return slopeFactor;
}

// Removes float noise from an axis so an unpressed axis is exactly 0.
function snap(a: number): number {
  return Math.abs(a) < 1e-4 ? 0 : a;
}

function copyInput(from: StepInput, to: StepInput) {
  to.x = from.x; to.z = from.z; to.limit = from.limit; to.grounded = from.grounded;
}

// ---------------------------------------------------------------- per-frame model

// Integration state. ax/az are the camera-relative axis speeds (right, forward), adx/adz their
// smoothing rates, weight the input-hold weight (0..1), vx/vz the world-space move velocity,
// ex/ez the external (impulse) velocity, t the model time at the end of the steps run so far and
// t0 the start of the last step (the vertical model's clock).
type State = { ax: number; az: number; adx: number; adz: number; weight: number; vx: number; vz: number; ex: number; ez: number; t: number; t0: number };
const state: State = { ax: 0, az: 0, adx: 0, adz: 0, weight: 0, vx: 0, vz: 0, ex: 0, ez: 0, t: 0, t0: 0 };
const probe: State = { ax: 0, az: 0, adx: 0, adz: 0, weight: 0, vx: 0, vz: 0, ex: 0, ez: 0, t: 0, t0: 0 };
// The air jump launches on the first step starting at or after this time (vertical model), read once per tick.
var launchTime = -1e9;
// Horizontal velocity of the last reply (ramp guard).
var lastReplyX = 0, lastReplyZ = 0;
// Steps starting before this time are stunned (vertical model's committed landing stun), read once per tick.
var stunEnd = -1e9;
// Start of the step the last reply was predicted for, and whether that step was stunned.
var lastReplyT0 = -1e9;
var lastReplyStun = false;
const EPS = 1e-6;

function copyState(from: State, to: State) {
  to.ax = from.ax; to.az = from.az; to.adx = from.adx; to.adz = from.adz; to.weight = from.weight;
  to.vx = from.vx; to.vz = from.vz; to.ex = from.ex; to.ez = from.ez; to.t = from.t; to.t0 = from.t0;
}

function moveTowards(current: number, target: number, maxDelta: number): number {
  return Math.abs(target - current) <= maxDelta ? target : current + Math.sign(target - current) * maxDelta;
}

function accelCurve(w: number): number {
  if (w <= ACCEL_CURVE_START) return 0;
  if (w >= ACCEL_CURVE_END) return 1;
  const s = (w - ACCEL_CURVE_START) / (ACCEL_CURVE_END - ACCEL_CURVE_START);
  return s * s * (3 - 2 * s);
}

// Critically damped spring toward target (returns the new value, writes the new rate to rate[0]).
const rateOut = [0];
function smoothDamp(current: number, target: number, rate: number, smoothTime: number, dt: number): number {
  smoothTime = Math.max(0.0001, smoothTime);
  const omega = 2 / smoothTime;
  const x = omega * dt;
  const e = 1 / (1 + x + 0.48 * x * x + 0.235 * x * x * x);
  const change = current - target;
  const temp = (rate + omega * change) * dt;
  let out = target + (change + temp) * e;
  rate = (rate - omega * temp) * e;
  if ((target - current > 0) === (out > target)) {
    out = target;
    rate = 0;
  }
  rateOut[0] = rate;
  return out;
}

function step(s: State, c: StepInput, h: number) {
  const prevT0 = s.t0;
  const t0 = s.t;
  s.t0 = t0;
  s.t = t0 + h;
  const stun = c.stun || t0 < stunEnd - EPS;
  const launch = t0 >= launchTime - EPS && prevT0 < launchTime - EPS;
  const xTarget = c.limit * c.x;
  const zTarget = c.limit * c.z;
  const w0 = s.weight;
  s.weight = moveTowards(s.weight, xTarget !== 0 || zTarget !== 0 ? 1 : 0, h / Math.max(1e-3, settings.accelTimeGround));
  // the acceleration of the step's mean input-hold weight (the weight ramps linearly within the step)
  const k = accelCurve(0.5 * (w0 + s.weight));
  const accel = c.grounded ? GROUND_ACCEL + (GROUND_MAX_ACCEL - GROUND_ACCEL) * k : AIR_ACCEL + (AIR_MAX_ACCEL - AIR_ACCEL) * k;

  // A held axis keeps its speed when its key flips sign (strafe A/D, W/S reversal), then moves
  // toward the target; a released axis settles to 0 over the stop time.
  if (c.x !== 0) {
    s.ax = moveTowards(Math.sign(xTarget) * Math.abs(s.ax), xTarget, accel * h);
  } else {
    s.ax = smoothDamp(s.ax, 0, s.adx, settings.dampTimeGround, h);
    s.adx = rateOut[0];
  }
  if (c.z !== 0) {
    s.az = moveTowards(Math.sign(zTarget) * Math.abs(s.az), zTarget, accel * h);
  } else {
    s.az = smoothDamp(s.az, 0, s.adz, settings.dampTimeGround, h);
    s.adz = rateOut[0];
  }

  let tx = right.x * s.ax + fwd.x * s.az;
  let tz = right.z * s.ax + fwd.z * s.az;
  const tl = Math.sqrt(tx * tx + tz * tz);
  // in the air the keys drive the move toward the gait speed less AIR_SPEED_LOSS of its square
  const limit = c.grounded ? c.limit : c.limit * Math.max(0, 1 - AIR_SPEED_LOSS * c.limit);
  if (tl > limit) {
    const f = limit / tl;
    tx *= f; tz *= f;
  }

  if (c.grounded) {
    // on the ground the move velocity follows the axes directly
    s.vx = tx; s.vz = tz;
    if (launch) airLaunch(s, c);
  } else {
    // in the air it chases them at the air acceleration, against quadratic drag. The drag acts first and the chase
    // makes up for it, so a speed the chase holds ends every step at its target whatever the step's length.
    airDrag(s, h);
    const dx = tx - s.vx, dz = tz - s.vz;
    const dl = Math.sqrt(dx * dx + dz * dz);
    const maxD = accel * h;
    if (dl <= maxD) { s.vx = tx; s.vz = tz; } else { s.vx += dx / dl * maxD; s.vz += dz / dl * maxD; }
    if (launch) airLaunch(s, c);
  }
  // a landing stun stops the move velocity (external pushes still apply); the keys held through it keep moving the
  // axes, so the gait speed is back on the first step after it
  if (stun) { s.vx = 0; s.vz = 0; }

  const damping = c.grounded ? EXTERNAL_ENV_DRAG + EXTERNAL_GROUND_FRICTION : EXTERNAL_ENV_DRAG;
  const ef = Math.exp(-damping * h);
  s.ex *= ef; s.ez *= ef;
  const es = s.ex * s.ex + s.ez * s.ez;
  if (es < 1e-4) { s.ex = 0; s.ez = 0; }
  else if (es > MAX_EXTERNAL_SPEED * MAX_EXTERNAL_SPEED) { const f = MAX_EXTERNAL_SPEED / Math.sqrt(es); s.ex *= f; s.ez *= f; }
}

// Quadratic air drag over h in closed form: dv/dt = -AIR_DRAG v^2 gives v / (1 + AIR_DRAG v h).
function airDrag(s: State, h: number) {
  const sp = Math.sqrt(s.vx * s.vx + s.vz * s.vz);
  if (sp > 0) {
    const f = 1 / (1 + AIR_DRAG * sp * h);
    s.vx *= f; s.vz *= f;
  }
}

// Air jump launch: the move velocity turns to the keys held on this step, each held axis at least AIR_JUMP_IMPULSE
// fast. A diagonal launches sqrt(2) faster; above the gait's speed, air drag and acceleration take the excess back
// within about 0.1 s. With no movement key held the launch stops the move.
function airLaunch(s: State, c: StepInput) {
  const v = Math.max(AIR_JUMP_IMPULSE, Math.sqrt(s.vx * s.vx + s.vz * s.vz));
  s.ax = v * c.x; s.az = v * c.z; s.adx = 0; s.adz = 0;
  s.vx = right.x * s.ax + fwd.x * s.az;
  s.vz = right.z * s.ax + fwd.z * s.az;
}

// Runs the steps that cover `time` (one per frame, see clock.ts).
function advance(s: State, c: StepInput, time: number) {
  const n = stepsFor(time);
  for (let k = 0; k < n; k++) step(s, c, time / n);
}

export function updateHorizontalVelocity() {
  readInput();
  launchTime = airLaunchTime();
  stunEnd = stunEndTime();

  if (horizontalResync) {
    // the engine moved us at another velocity: continue from what it applied
    state.vx = prevActualVelocity.x; state.vz = prevActualVelocity.z;
    state.ax = state.vx * right.x + state.vz * right.z;
    state.az = state.vx * fwd.x + state.vz * fwd.z;
    state.adx = 0; state.adz = 0; state.ex = 0; state.ez = 0;
  }
  state.ex += prevExternalVelocity.x;
  state.ez += prevExternalVelocity.z;

  // Cover the time up to the end of this frame with the previous tick's input. Off an edge this tick, those steps are in
  // the air: the keys reach the model a tick late, leaving the edge does not (a release a frame before an edge is a
  // release in the air past it).
  if (leftEdge) prevInput.grounded = false;
  advance(state, prevInput, tickAdvance);

  // The engine holds the reply from the frame it lands on until the next reply lands; reply
  // with the state predicted (on this tick's input) for the last of those frames.
  let ox = state.vx + state.ex, oz = state.vz + state.ez;
  // the move velocity of that state alone, without pushes and before the cuts below: the avatar faces its heading
  let mx = state.vx, mz = state.vz;
  let replyT0 = state.t0, replyStun = false;
  if (replyDelay > 0 && frameTime > 0) {
    copyState(state, probe);
    const frames = 2 * replyDelay;
    if (!input.grounded && predictedLandFrame >= 0 && predictedLandFrame < replyDelay) {
      // the avatar touches the ground before the reply lands, so every frame it is held for
      // comes after the touchdown: the steps after that frame are ground steps (the first
      // grounded step sets the move velocity to its target). A touchdown inside the held
      // frames keeps the air velocity, so the touchdown frame still moves at air speed.
      copyInput(input, landInput);
      landInput.grounded = true;
      landInput.stun = landingStuns();
      advance(probe, input, predictedLandFrame * frameTime);
      advance(probe, landInput, (frames - predictedLandFrame) * frameTime);
      replyStun = landInput.stun;
    } else {
      advance(probe, input, frames * frameTime);
    }
    ox = probe.vx + probe.ex; oz = probe.vz + probe.ez;
    mx = probe.vx; mz = probe.vz;
    replyT0 = probe.t0;
  } else if (launchAhead) {
    // the vertical reply already carries a launch the committed steps reach after this frame: so does this one,
    // with this tick's input for the launch step (a ground move), so a jump with the keys released goes straight up
    copyState(state, probe);
    advance(probe, input, oneStep());
    ox = probe.vx + probe.ex; oz = probe.vz + probe.ez;
    mx = probe.vx; mz = probe.vz;
    replyT0 = probe.t0;
  }
  // A reply from a stunned step: the guard below does not hold the speed from before the stun, only the end of the
  // stun once a reply has already passed it.
  replyStun = replyStun || replyT0 < stunEnd - EPS;
  // a stun a reply has published holds: until the committed landing, a later reply that does not reach the touchdown
  // keeps it
  if (!replyStun && lastReplyStun && landingStuns()) {
    ox = state.ex; oz = state.ez;
    mx = 0; mz = 0;
    replyStun = true;
  }

  // Ramp guard. The delay estimate follows the previous reply's delay, which does not predict
  // this one, so consecutive replies alternate between the predicted and the committed state.
  // During a ground acceleration that reads as a speed dip of up to one prediction horizon
  // (0.5-0.9 m/s), which the rigidly attached camera shows as a jerk. While the same input is
  // held on the ground and the model is still accelerating, a reply never slows below the last
  // one; the committed state catches up within a tick or two.
  if (!horizontalResync && !(replyStun && lastReplyT0 < stunEnd - EPS) && (input.x !== 0 || input.z !== 0) && input.grounded && prevInput.grounded
    && input.x === prevInput.x && input.z === prevInput.z && input.limit === prevInput.limit
    && state.ex === 0 && state.ez === 0) {
    const committed = Math.sqrt(state.vx * state.vx + state.vz * state.vz);
    const out = Math.sqrt(ox * ox + oz * oz);
    const last = Math.sqrt(lastReplyX * lastReplyX + lastReplyZ * lastReplyZ);
    if (committed < input.limit - 1e-3 && out < last) {
      if (out < 1e-6) {
        // the committed steps have not started yet: keep the last reply
        const f = Math.min(1, input.limit / last);
        ox = lastReplyX * f; oz = lastReplyZ * f;
      } else if (ox * lastReplyX + oz * lastReplyZ > 0) {
        const f = Math.min(last, input.limit) / out;
        ox *= f; oz *= f;
      }
    }
  }
  // Against a wall on the ground the move slows with how squarely it meets the wall: by 1 - |cos| of the angle between
  // the move and the wall's normal, so head-on it stops and at 45 degrees it slides along the wall at 0.29 x 0.71 of
  // the speed. In the air a wall only blocks the move.
  const wall = input.grounded ? wallContact() : null;
  if (wall !== null) {
    const speed = Math.hypot(ox, oz);
    const into = speed > 0 ? -(ox * wall.x + oz * wall.z) / speed : 0;
    if (into > 0) { ox *= 1 - into; oz *= 1 - into; }
  }

  lastReplyX = ox; lastReplyZ = oz; lastReplyT0 = replyT0; lastReplyStun = replyStun;
  copyInput(input, prevInput);

  velocity.x = ox;
  velocity.z = oz;
  Vector3.multiplyToRef(velocity, VEC3_HORIZONTAL_MASK, horizontalVelocity);
  setOrientation(mx, mz, replyStun || stunned());
}

// ---------------------------------------------------------------- orientation

var targetOrientation = 0;
var scratch: Vector3.MutableVector3 = Vector3.Zero();
var targetFacing = Quaternion.Identity();
// The heading the avatar turns toward, kept when the keys are released so a turn under way finishes; null until the
// keys move the avatar, and again after the engine sets the facing itself.
var heading: number | null = null;
// Facings of the last replies, newest first. The engine applies a reply from the frame it lands on, possibly a frame
// late, so the facing a tick reads is one of these unless the engine set it itself (a movePlayerTo target, a teleport,
// a rotating platform).
const publishedOrientation: number[] = [NaN, NaN, NaN];

// returns equivalent angle within 180 degrees of center
export function relativeDegrees(center: number, angle: number): number {
  return center + ((angle - center + 180) % 360 + 360) % 360 - 180;
}

// Turns the avatar toward the heading of its own move velocity (moveX, moveZ: without scene pushes and before the reply's
// cuts at slopes and walls), in every state: on the ground, in the air, while the glider opens, glides and closes. The
// heading follows the move while the keys are held; on release the avatar keeps turning to the last one. A landing stun
// holds the facing.
function setOrientation(moveX: number, moveZ: number, stun: boolean) {
  // Always adopt the current avatar transform as the orientation baseline, even when
  // locked, so an engine-imposed facing (e.g. a movePlayerTo cameraTarget) takes effect
  // instead of freezing at the pre-teleport orientation.
  orientation = Quaternion.toEulerAngles(playerRotation).y;
  // a facing the engine set itself drops the kept heading, so finishing a turn does not undo it
  let published = false;
  for (const o of publishedOrientation) if (Math.abs(relativeDegrees(0, orientation - o)) < 0.1) published = true;
  if (!published) heading = null;
  if (disableOrientation) {
    heading = null;
    targetOrientation = orientation;
    publishOrientation();
    return;
  }

  if (Vector3.lengthSquared(movementAxis) > 0 && moveX * moveX + moveZ * moveZ > 1e-4) {
    scratch.x = moveX; scratch.y = 0; scratch.z = moveZ;
    Quaternion.fromLookAtToRef(VEC3_ZERO, scratch, VEC3_UP, targetFacing);
    heading = Quaternion.toEulerAngles(targetFacing).y;
  }
  targetOrientation = heading === null || stun ? orientation : heading;

  // The facing eases toward the heading over TURN_FULL_TIME, at most TURN_MAX_DEGREES_SEC; a turn under way trails a
  // turning heading by about TURN_FULL_TIME of that turn. A glide slowed to 1-3 m/s lets a key spiral turn its heading
  // at 350-700 deg/s: a slower limit would fall further behind on every lap until the avatar flies backwards.
  if (targetOrientation != orientation) {
    if (Math.abs(targetOrientation - orientation) < 1) {
      orientation = targetOrientation;
    } else {
      orientation = relativeDegrees(targetOrientation, orientation);
      // an about-turn (W to S) turns left, whichever way rounding puts the heading
      if (targetOrientation - orientation > 179.9) orientation += 360;
      let perc = Math.min(tickTime / TURN_FULL_TIME, 1);
      orientation = Math.max(
        orientation - TURN_MAX_DEGREES_SEC * tickTime,
        Math.min(
          orientation + TURN_MAX_DEGREES_SEC * tickTime,
          targetOrientation * perc + orientation * (1 - perc)
        ));
      orientation = relativeDegrees(180, orientation);
    }
  }
  publishOrientation();
}

function publishOrientation() {
  for (let i = publishedOrientation.length - 1; i > 0; i--) publishedOrientation[i] = publishedOrientation[i - 1];
  publishedOrientation[0] = orientation;
}

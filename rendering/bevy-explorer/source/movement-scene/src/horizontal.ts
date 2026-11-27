import { engine, InputAction, inputSystem, Transform } from '@dcl/sdk/ecs'
import { Quaternion, Vector3 } from '@dcl/sdk/math';
import {
  AIR_JUMP_IMPULSE, ACCEL_CURVE_END, ACCEL_CURVE_START, AIR_ACCEL, AIR_DRAG, AIR_MAX_ACCEL, EXTERNAL_ENV_DRAG, EXTERNAL_GROUND_FRICTION,
  GROUND_ACCEL, GROUND_MAX_ACCEL, MAX_EXTERNAL_SPEED,
  SLOPE_SPEED_FACTOR, SLOPE_SPEED_MAX_ANGLE, TURN_RATE, VEC3_FORWARD, VEC3_HORIZONTAL_MASK, VEC3_RIGHT,
  VEC3_UP, VEC3_ZERO,
} from './constants';
import { horizontalResync, playerRotation, prevActualVelocity, prevExternalVelocity, tickTime, velocity } from '.';
import { grounded, GROUNDED_ANGLE_Y_LEN, groundNormal, groundRayNormal } from './ground';
import { disableOrientation, glidingSpeed, jogSpeed, sprintSpeed, walkSpeed } from './parameters';
import { airLaunchTime, isGliding, leftEdge, steepAhead, stunEndTime, stunned, wallContact } from './vertical';
import { getWalkAxis } from './walk';
import { oneStep, stepsFor, tickAdvance } from './clock';
import { settings } from './settings';

export var orientation = 0;
// Gait the keys ask for (the animation's movement blend).
export var gait: 'idle' | 'walk' | 'jog' | 'run' = 'idle';
// World-space unit direction of the movement input (zero when there is none).
export var movementAxis = Vector3.Zero();
var horizontalVelocity = Vector3.Zero();

// ---------------------------------------------------------------- input

// camera-relative horizontal unit vectors
var fwd = Vector3.Zero();
var right = Vector3.Zero();
type StepInput = { x: number; z: number; limit: number; grounded: boolean };
const input: StepInput = { x: 0, z: 0, limit: 0, grounded: true };
// The steps that cover the time up to this tick's frame use the previous tick's input.
const prevInput: StepInput = { x: 0, z: 0, limit: 0, grounded: true };

function readInput() {
  const camera = Transform.get(engine.CameraEntity);
  Vector3.rotateToRef(VEC3_FORWARD, camera.rotation, fwd);
  Vector3.multiplyToRef(fwd, VEC3_HORIZONTAL_MASK, fwd);
  Vector3.normalizeToRef(fwd, fwd);
  Vector3.rotateToRef(VEC3_RIGHT, camera.rotation, right);
  Vector3.multiplyToRef(right, VEC3_HORIZONTAL_MASK, right);
  Vector3.normalizeToRef(right, right);

  // Auto-walk takes priority. getWalkAxis() returns null, and cancels the walk, on manual input.
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
  // Each held key drives its axis at full speed, and the step cuts the move to the gait's speed. With shortened axes,
  // adding or releasing a side key while running would dip the speed instead of only turning the move.
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
  gait = input.x === 0 && input.z === 0 ? 'idle'
    : inputSystem.isPressed(InputAction.IA_MODIFIER) ? 'run'
    : inputSystem.isPressed(InputAction.IA_WALK) ? 'walk'
    : 'jog';
}

// The slope comes from the capsule's walkable contact (on a step edge, the edge), else from the ground under the centre.
// In the air the factor of the last ground holds.
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

// ax/az: camera-relative axis speeds (right, forward). adx/adz: their smoothing rates. weight: input-hold weight (0..1).
// vx/vz: world-space move velocity. ex/ez: velocity of scene pushes. t: model time at the end of the steps run so far.
// t0: start of the last step.
type State = { ax: number; az: number; adx: number; adz: number; weight: number; vx: number; vz: number; ex: number; ez: number; t: number; t0: number };
const state: State = { ax: 0, az: 0, adx: 0, adz: 0, weight: 0, vx: 0, vz: 0, ex: 0, ez: 0, t: 0, t0: 0 };
// The air jump's launch time from the vertical model, read once per tick.
var launchTime = -1e9;
// Steps starting before this time are stunned, read once per tick.
var stunEnd = -1e9;
const EPS = 1e-6;

function moveTowards(current: number, target: number, maxDelta: number): number {
  return Math.abs(target - current) <= maxDelta ? target : current + Math.sign(target - current) * maxDelta;
}

function accelCurve(w: number): number {
  if (w <= ACCEL_CURVE_START) return 0;
  if (w >= ACCEL_CURVE_END) return 1;
  const s = (w - ACCEL_CURVE_START) / (ACCEL_CURVE_END - ACCEL_CURVE_START);
  return s * s * (3 - 2 * s);
}

// Critically damped spring toward target (returns the new value, writes the new rate to rateOut[0]).
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
  const stun = t0 < stunEnd - EPS;
  const launch = t0 >= launchTime - EPS && prevT0 < launchTime - EPS;
  const xTarget = c.limit * c.x;
  const zTarget = c.limit * c.z;
  const w0 = s.weight;
  s.weight = moveTowards(s.weight, xTarget !== 0 || zTarget !== 0 ? 1 : 0, h / Math.max(1e-3, settings.accelTimeGround));
  // the acceleration of the step's mean input-hold weight (the weight ramps linearly within the step)
  const k = accelCurve(0.5 * (w0 + s.weight));
  const accel = c.grounded ? GROUND_ACCEL + (GROUND_MAX_ACCEL - GROUND_ACCEL) * k : AIR_ACCEL + (AIR_MAX_ACCEL - AIR_ACCEL) * k;

  // A held axis keeps its speed when its key flips sign, then moves toward the target. A released axis settles to 0.
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
  if (tl > c.limit) {
    const f = c.limit / tl;
    tx *= f; tz *= f;
  }

  if (c.grounded) {
    s.vx = tx; s.vz = tz;
    if (launch) airLaunch(s, c);
  } else {
    // the drag acts before the chase, so a speed the chase holds ends every step at its target whatever the step length
    airDrag(s, h);
    const dx = tx - s.vx, dz = tz - s.vz;
    const dl = Math.sqrt(dx * dx + dz * dz);
    const maxD = accel * h;
    if (dl <= maxD) { s.vx = tx; s.vz = tz; } else { s.vx += dx / dl * maxD; s.vz += dz / dl * maxD; }
    if (launch) airLaunch(s, c);
  }
  // A landing stun stops the move velocity, not the pushes. The axes keep moving, so the gait speed is back on the first
  // step after the stun.
  if (stun) { s.vx = 0; s.vz = 0; }

  const damping = c.grounded ? EXTERNAL_ENV_DRAG + EXTERNAL_GROUND_FRICTION : EXTERNAL_ENV_DRAG;
  const ef = Math.exp(-damping * h);
  s.ex *= ef; s.ez *= ef;
  const es = s.ex * s.ex + s.ez * s.ez;
  if (es < 1e-4) { s.ex = 0; s.ez = 0; }
  else if (es > MAX_EXTERNAL_SPEED * MAX_EXTERNAL_SPEED) { const f = MAX_EXTERNAL_SPEED / Math.sqrt(es); s.ex *= f; s.ez *= f; }
}

// closed form of dv/dt = -AIR_DRAG v^2 over h
function airDrag(s: State, h: number) {
  const sp = Math.sqrt(s.vx * s.vx + s.vz * s.vz);
  if (sp > 0) {
    const f = 1 / (1 + AIR_DRAG * sp * h);
    s.vx *= f; s.vz *= f;
  }
}

// The launch goes along the input at the impulse or the current speed, whichever is higher. An input longer than 1 (two
// held keys) counts as 1, so a diagonal launches as fast as a straight move; a shorter analog input keeps its length.
// Air drag and acceleration take back any excess over the gait's speed. With no movement key held the launch stops the
// move.
function airLaunch(s: State, c: StepInput) {
  const len = Math.sqrt(c.x * c.x + c.z * c.z);
  const v = Math.max(AIR_JUMP_IMPULSE, Math.sqrt(s.vx * s.vx + s.vz * s.vz)) / Math.max(1, len);
  s.ax = v * c.x; s.az = v * c.z; s.adx = 0; s.adz = 0;
  s.vx = right.x * s.ax + fwd.x * s.az;
  s.vz = right.z * s.ax + fwd.z * s.az;
}

function advance(s: State, c: StepInput, time: number) {
  const n = stepsFor(time);
  for (let k = 0; k < n; k++) step(s, c, time / n);
}

export function updateHorizontalVelocity() {
  readInput();
  launchTime = airLaunchTime();
  stunEnd = stunEndTime();

  if (horizontalResync) {
    state.vx = prevActualVelocity.x; state.vz = prevActualVelocity.z;
    state.ax = state.vx * right.x + state.vz * right.z;
    state.az = state.vx * fwd.x + state.vz * fwd.z;
    state.adx = 0; state.adz = 0; state.ex = 0; state.ez = 0;
  }
  state.ex += prevExternalVelocity.x;
  state.ez += prevExternalVelocity.z;

  // The steps up to this frame use the previous tick's keys, but leaving an edge applies to them at once, so a release a
  // frame before an edge is a release in the air past it.
  if (leftEdge) prevInput.grounded = false;
  advance(state, prevInput, tickAdvance);

  let ox = state.vx + state.ex, oz = state.vz + state.ez;
  // Slope limit: on the ground, cut the speed into a steep slope ahead so that the avatar reaches it no sooner than the
  // next reply lands.
  const steep = input.grounded ? steepAhead() : null;
  if (steep !== null) {
    const into = -(ox * steep.x + oz * steep.z);
    const allowed = steep.distance / oneStep();
    if (into > allowed) {
      ox += (into - allowed) * steep.x;
      oz += (into - allowed) * steep.z;
    }
  }
  // Against a wall on the ground the move scales by 1 - cos of its angle to the wall's normal, so head-on it stops. In
  // the air the wall only blocks the move.
  const wall = input.grounded ? wallContact() : null;
  if (wall !== null) {
    const speed = Math.hypot(ox, oz);
    const into = speed > 0 ? -(ox * wall.x + oz * wall.z) / speed : 0;
    if (into > 0) { ox *= 1 - into; oz *= 1 - into; }
  }

  copyInput(input, prevInput);

  velocity.x = ox;
  velocity.z = oz;
  Vector3.multiplyToRef(velocity, VEC3_HORIZONTAL_MASK, horizontalVelocity);
  // the move velocity alone, without pushes and before the cuts above: the avatar faces its heading
  setOrientation(state.vx, state.vz, stunned());
}

// ---------------------------------------------------------------- orientation

var targetOrientation = 0;
var scratch: Vector3.MutableVector3 = Vector3.Zero();
var targetFacing = Quaternion.Identity();
// The heading the avatar turns toward. It is kept after the keys are released, so a turn under way finishes. null until
// the keys move the avatar, and after the engine sets the facing itself.
var heading: number | null = null;
// Facings of the last replies, newest first. A reply can land a frame late, so a facing that is none of these was set
// by the engine (a movePlayerTo target, a teleport, a rotating platform).
const publishedOrientation: number[] = [NaN, NaN, NaN];

// returns equivalent angle within 180 degrees of center
export function relativeDegrees(center: number, angle: number): number {
  return center + ((angle - center + 180) % 360 + 360) % 360 - 180;
}

// Turns the avatar toward its own move velocity (without pushes and before the slope and wall cuts) in every state. A
// landing stun holds the facing.
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

  // Each tick the facing moves TURN_RATE * tickTime of the way to the heading along the rotation between them (a
  // normalized quaternion lerp): a gap of g degrees closes by 2 * atan2(f sin(g/2), 1 - f + f cos(g/2)), so a small gap
  // eases out exponentially and an about-turn opens at 22.6 degrees a frame at 60 fps.
  if (targetOrientation != orientation) {
    if (Math.abs(targetOrientation - orientation) < 1) {
      orientation = targetOrientation;
    } else {
      orientation = relativeDegrees(targetOrientation, orientation);
      // an about-turn (W to S) turns left, whichever way rounding puts the heading
      if (targetOrientation - orientation > 179.9) orientation += 360;
      const gap = (targetOrientation - orientation) * Math.PI / 180;
      const f = Math.min(TURN_RATE * tickTime, 1);
      orientation += 2 * Math.atan2(f * Math.sin(gap / 2), 1 - f + f * Math.cos(gap / 2)) * 180 / Math.PI;
      orientation = relativeDegrees(180, orientation);
    }
  }
  publishOrientation();
}

function publishOrientation() {
  for (let i = publishedOrientation.length - 1; i > 0; i--) publishedOrientation[i] = publishedOrientation[i - 1];
  publishedOrientation[0] = orientation;
}

import { ColliderLayer, engine, Entity, InputAction, inputSystem, RaycastQueryType, RaycastShape, raycastSystem, RaycastSystemCallback, Transform } from '@dcl/sdk/ecs'
import { Vector3 } from '@dcl/sdk/math';
import { centreOnGround, centreSupported, groundDistance, grounded, GROUNDED_ANGLE_Y_LEN, groundNormal, groundRayDistance, groundRayNormal, prevGrounded, setGrounded } from './ground';
import {
  AIR_JUMP_COOLDOWN, AIR_JUMP_HOVER_SPEED, AIR_JUMP_HOVER_TIME,
  GLIDE_CLOSE_TIME, GLIDE_COOLDOWN, GLIDE_DAMP_TIME, GLIDE_MIN_GROUND_DISTANCE, GLIDE_OPEN_TIME,
  GRAVITY, GROUNDED_HEIGHT,
  JUMP_BUFFER_TIME, JUMP_COYOTE_TIME, JUMP_HOLD_SCALE, JUMP_HOLD_TIME, JUMP_RISE_DECEL,
  JUMP_TO_GLIDE_TIME, LANDING_STUN_DROP, MAX_STEP_HEIGHT, MIN_STEP_HEIGHT, STEP_UP_PLAN_SPEED, STEP_UP_REACH_TIME, STEP_UP_TIME,
  GROUNDED_ANGLE, PLAYER_COLLIDER_RADIUS, VEC3_FORWARD, VEC3_ZERO,
  VERTICAL_DAMP_TIME, VERTICAL_STOP_DECEL, VERTICAL_TRACK_BLOCK, VERTICAL_TRACK_MAX, VERTICAL_TRACK_TIME,
} from './constants';
import { engineDriven, horizontalResync, playerPosition, playerRotation, prevActualVelocity, prevExternalVelocity, prevRequestedVelocity, tickTime, velocity } from '.';
import { movementAxis } from './horizontal';
import { doubleJumpHeight, glideEnabled, glidingFallingSpeed, jumpHeight, landingStunTime, maxAirJumps, maxGroundJumps, sprintJumpHeight, sprintSpeed } from './parameters';
import { frameTime, oneStep, replyDelay, replyDelaySmooth, stepsFor, tickAdvance } from './clock';

const G = -GRAVITY.y;

export function updateVerticalVelocity() {
  leftEdge = false;
  followsGround = false;
  snapSpeed = 0;
  if (engineDriven) {
    stepping = false;
    climbSent = 0;
    // Hold the grounded state while the engine drives us (see engineDriven): a lerp lifting
    // us off the ground must not read as a fall, nor arrive as a landing. A jump press still
    // goes through so the player can cancel the move.
    // ... unless an upward push launches us off it
    setGrounded(isLaunched() ? false : prevGrounded);
    const top = vstate.topY;
    applyJumpAndGravity(true);
    // a fall from where the engine put us counts from the top before, raised only by heights above the placement, so a
    // placement high up never stuns
    vstate.topY = top; placedY = playerPosition.y;
    return;
  }
  stepUp();
  leaveEdge();
  // a snap keeps the model on the ground this tick
  const snap = snapDistance();
  if (snap > 0) setGrounded(true);
  applyJumpAndGravity(false);
  // pushed upward: off the ground for this tick's horizontal move and the next tick
  if (isLaunched()) setGrounded(false);
  // in the air, or a jump leaves the ground: no climb, snap or ground following
  if (!grounded || jumpStartHeight !== undefined) {
    stepping = false;
    climbSent = 0;
    return;
  }
  if (stepping) {
    // not past the tread: the climb replies still in flight rise further before this one applies
    const left = stepTargetY - playerPosition.y - replyDelay * oneStep() * climbSent;
    velocity.y = Math.min(stepSpeed, left / oneStep());
    climbSent = velocity.y;
    // the model stands where the climb has the feet
    vstate.y = playerPosition.y;
    return;
  }
  climbSent = 0;
  snapSpeed = snap / oneStep();
  followsGround = centreOnGround();
}

// The avatar stands on an edge: grounded with no ground under the centre (centreSupported), only the rim of the capsule
// resting on the corner, its centre R * sin(tilt) past it, where the tilt of the contact normal reaches GROUNDED_ANGLE at
// the most. A move off the edge that carries the centre past that before the next reply lands leaves the ground at
// once: it goes on at air speed and falls (a jog released at a ledge carries on past it), where a slower one stays on
// the edge until then. Standing on an edge, the capsule rests on the corner: the ground snap leaves it there.
var onEdge = false;
// The avatar leaves the edge this tick.
export var leftEdge = false;
const EDGE_REACH = PLAYER_COLLIDER_RADIUS * Math.sin(GROUNDED_ANGLE * Math.PI / 180);
function leaveEdge() {
  onEdge = grounded && !stepping && jumpStartHeight === undefined && !centreSupported();
  const h = Math.hypot(groundNormal.x, groundNormal.z);
  if (!onEdge || h < 1e-3) return;
  const out = (velocity.x * groundNormal.x + velocity.z * groundNormal.z) / h;
  const ahead = tickTime + replyDelay * frameTime;
  if (out > 0 && PLAYER_COLLIDER_RADIUS * h + out * ahead > EDGE_REACH) {
    setGrounded(false);
    leftEdge = true;
  }
}

// ---------------------------------------------------------------- jump and gravity
// Stepped once per frame on the same clock as the horizontal model (see clock.ts). Timers are in seconds of model time:
// an event happens on the first step that starts at or after its time.

// Height the current (or next) jump was launched for (a tapped jump's apex).
var currentJumpHeight = 0;
// Feet height at launch while the jump is rising; undefined otherwise.
export var jumpStartHeight: number | undefined = undefined;
// An air jump is under way (from its press until the landing or a glide), for the animation selector.
export var isDoubleJump = false;
// The glider: its prop opens over GLIDE_OPEN_TIME before the glide holds the fall, and closes over GLIDE_CLOSE_TIME
// before the glide cooldown starts. Only GLIDING moves the avatar differently (isGliding); the prop's state is for the
// animation.
export enum GlideProp { CLOSED, OPENING, GLIDING, CLOSING }
export var glideProp = GlideProp.CLOSED;
export var isGliding = false;

// The avatar moves at vy + ve: one vertical speed that gravity and scene pushes act on (the ballistic speed, vg + ve),
// with the player's own moves added on top of it (vy - vg). A jump's rise and its held window, an air jump's held
// height and the glide's fall shape vy only, so they never cancel a push, and a push never gets the jump's slowdown.
// The glider's drag takes a push's share while gliding (GLIDE_DAMP_TIME).
type VState = {
  vy: number; // the player's own vertical velocity: gravity, jumps, an air jump's held height, the glide
  // the ballistic part of vy: vy itself, except while a jump rises or an air jump holds the height, when it goes on
  // under plain gravity from the step before (at the apex the jump's added lift joins it)
  vg: number;
  ve: number; // what scene pushes add to vy
  t: number; // model time at the end of the steps run so far (s)
  t0: number; // start of the last step
  y0: number; // model feet height at the start of the last step
  yo: number; // the player's own vertical travel so far (pushes excluded), for the replies' mean velocity
  yo0: number; // the same at the start of the last step
  groundedLast: boolean; // the last step was on the ground
  lastGroundedEnd: number; // end of the last step on the ground
  triggerT: number; // start of the step the latest press is due on
  pressBack: number; // s the latest press may predate its due step (it was made in an earlier frame of its tick)
  consumedT: number; // start of the step the latest jump launched on
  jumpCount: number; // jumps since the ground (falling past the coyote window counts as one)
  glideWanted: boolean;
  prop: GlideProp; // the glider prop's state
  propT: number; // start of the step the prop entered that state
  glideOpenT: number; // start of the step the prop last started opening
  glideClosedT: number; // start of the step the prop last closed: the glide cooldown counts from it
  launched: boolean; // a jump launched during the last advance
  y: number; // model feet height: the ground height while on the ground, integrated in the air
  airPressT: number; // start of this airtime's air-jump press step
  airLaunchT: number; // the air jump launches on the first step starting at or after it (held height before); NEVER: none
  airLaunchEnd: number; // end of the step the air jump launched on (NEVER: not launched)
  airLaunched: boolean; // an air jump launched during the last advance
  topY: number; // height a landing stun measures the drop from: the ground, then the avatar's highest since
  stunEndT: number; // steps starting before it are stunned
};
const NEVER = -1e9;
// tolerance of time comparisons (s)
const EPS = 1e-6;
const vstate: VState = {
  vy: 0, vg: 0, ve: 0, t: 0, t0: 0, y0: 0, yo: 0, yo0: 0, groundedLast: false, lastGroundedEnd: NEVER, triggerT: NEVER, consumedT: NEVER,
  jumpCount: 0, glideWanted: false, prop: GlideProp.CLOSED, propT: NEVER, glideOpenT: NEVER, glideClosedT: NEVER,
  launched: false, y: 0, airPressT: NEVER, airLaunchT: NEVER, airLaunchEnd: NEVER, airLaunched: false,
  topY: -Infinity, stunEndT: NEVER, pressBack: 0,
};
const vprobe: VState = { ...vstate };
var jumpWasPressed = false;
var prevJumpPressed = false;
var prevRunning = false;
// The published reply includes a launch that the committed steps have not reached yet.
var launchPending = false;
// A launch is published ahead of the committed steps this tick (read by the horizontal reply).
export var launchAhead = false;
// Frames from this tick's dispatch frame to the frame the avatar is predicted to touch the
// ground (0 = during this frame), within the frames this reply is held; -1 otherwise.
export var predictedLandFrame = -1;

function copyVState(from: VState, to: VState) {
  to.vy = from.vy; to.vg = from.vg; to.ve = from.ve; to.t = from.t; to.t0 = from.t0; to.y0 = from.y0;
  to.yo = from.yo; to.yo0 = from.yo0;
  to.groundedLast = from.groundedLast; to.lastGroundedEnd = from.lastGroundedEnd; to.triggerT = from.triggerT;
  to.consumedT = from.consumedT; to.jumpCount = from.jumpCount; to.glideWanted = from.glideWanted;
  to.prop = from.prop; to.propT = from.propT; to.glideOpenT = from.glideOpenT;
  to.glideClosedT = from.glideClosedT; to.launched = from.launched; to.y = from.y;
  to.airPressT = from.airPressT; to.airLaunchT = from.airLaunchT; to.airLaunchEnd = from.airLaunchEnd;
  to.airLaunched = from.airLaunched; to.topY = from.topY; to.stunEndT = from.stunEndT;
  to.pressBack = from.pressBack;
}

// Ground height under the avatar this tick (-Infinity: none in range). An air jump needs the model above it.
var groundY = -Infinity;
function airJumpAllowed(s: VState): boolean {
  return maxAirJumps > 0 && s.jumpCount >= 1 && Math.max(s.jumpCount, 1) <= maxAirJumps
    && s.t0 - s.consumedT >= AIR_JUMP_COOLDOWN - EPS && s.y > groundY;
}

// The launch speed of a jump of `height`: the speed that rises height while slowing at JUMP_RISE_DECEL.
function launchFor(height: number): number {
  return Math.sqrt(2 * Math.max(0, height) * JUMP_RISE_DECEL);
}

function launchSpeed(running: boolean): number {
  // Use the more conservative of prev requested vs prev actual so external forces (impulses,
  // moving platforms) don't inflate the running jump.
  const req = Math.sqrt(prevRequestedVelocity.x * prevRequestedVelocity.x + prevRequestedVelocity.z * prevRequestedVelocity.z);
  const act = Math.sqrt(prevActualVelocity.x * prevActualVelocity.x + prevActualVelocity.z * prevActualVelocity.z);
  const maxHeight = running ? sprintJumpHeight : jumpHeight;
  const k = sprintSpeed > 0 ? Math.min(1, Math.max(0, Math.min(req, act) / sprintSpeed)) : 0;
  currentJumpHeight = jumpHeight + (maxHeight - jumpHeight) * k;
  return launchFor(currentJumpHeight);
}

function vstep(s: VState, pressed: boolean, running: boolean, onGroundTick: boolean, noGravity: boolean, h: number) {
  const prevT0 = s.t0;
  const t0 = s.t;
  s.t0 = t0;
  s.t = t0 + h;
  s.y0 = s.y;
  s.yo0 = s.yo;
  const vy0 = s.vy;
  let vb0 = s.vg + s.ve; // the ballistic speed at the start of the step
  let justJumped = false;
  let glideDrag = false;
  const onGround = onGroundTick && s.vy <= 0;
  if (onGround) {
    // a landing more than LANDING_STUN_DROP below the top of the airtime stuns for landingStunTime
    if (!s.groundedLast && s.topY - s.y > LANDING_STUN_DROP) {
      s.stunEndT = t0 + landingStunTime;
    }
    s.topY = s.y;
    s.lastGroundedEnd = s.t;
    s.jumpCount = 0;
    s.airLaunchT = NEVER; // the landing cancels an air jump that has not launched
  }
  s.groundedLast = onGround;
  // an air jump holds the height until its launch step; no other jump starts meanwhile
  const hover = s.airLaunchT !== NEVER && t0 < s.airLaunchT - EPS;
  if (hover) {
    s.vy = AIR_JUMP_HOVER_SPEED;
  } else if (s.airLaunchT !== NEVER && prevT0 < s.airLaunchT - EPS) {
    currentJumpHeight = doubleJumpHeight;
    s.vy = launchFor(doubleJumpHeight);
    s.airLaunched = true;
    s.airLaunchEnd = s.t;
  }

  let airPress = false;
  if (!hover) {
    // the press is due on this step; off while moving up, a press is buffered before a landing and a late one off an
    // edge still jumps from the ground
    const due = Math.abs(t0 - s.triggerT) < EPS;
    const windows = !(s.vy > 0) && t0 >= JUMP_COYOTE_TIME;
    // a late jump off an edge counts from when the press may have been made, not from when the tick saw it
    const coyote = windows && t0 - (due ? s.pressBack : 0) - s.lastGroundedEnd < JUMP_COYOTE_TIME - EPS;
    if (!onGround && !coyote) s.jumpCount = Math.max(s.jumpCount, 1);
    const canJump = s.jumpCount === 0 && (onGround || coyote) && maxGroundJumps > 0 && jumpHeight > 0;
    const wants = due || (windows && t0 - s.triggerT <= JUMP_BUFFER_TIME + EPS);
    if (canJump && wants) {
      s.vy = launchSpeed(running);
      vb0 = keepPush(s);
      justJumped = true;
      s.launched = true;
      s.jumpCount++;
      s.triggerT = NEVER;
      s.consumedT = t0;
    } else if (wants && !onGround && airJumpAllowed(s)) {
      // air jump: gravity still acts on this step, the height is held from the next one for AIR_JUMP_HOVER_TIME
      airPress = true;
      vb0 = keepPush(s);
      s.jumpCount++;
      s.triggerT = NEVER;
      s.consumedT = t0;
      s.airPressT = t0;
      s.airLaunchT = s.t + AIR_JUMP_HOVER_TIME;
      s.airLaunchEnd = NEVER;
    }
  }

  // glide: a fresh press in the air once the jumps are spent opens the glider's prop, held to keep gliding
  // the model's own height above the ground (the avatar's lags it by the reply delay)
  const canGlide = glideEnabled && s.jumpCount > maxAirJumps && !onGround && s.y - groundY > GLIDE_MIN_GROUND_DISTANCE;
  // the prop's clips run on their own time: an open prop glides, a shut one starts the cooldown. Once it starts opening
  // it opens fully, through a release (a tap) or a landing, and closes from there on the next step.
  if (s.prop === GlideProp.OPENING && t0 - s.propT >= GLIDE_OPEN_TIME - EPS) {
    s.prop = GlideProp.GLIDING;
    s.propT = t0;
  } else if (s.prop === GlideProp.CLOSING && t0 - s.propT >= GLIDE_CLOSE_TIME - EPS) {
    s.prop = GlideProp.CLOSED;
    s.propT = t0;
    s.glideClosedT = t0;
  }
  s.glideWanted = (s.glideWanted || (canGlide && Math.abs(t0 - s.triggerT) < EPS)) && pressed;
  const ready = t0 - s.consumedT >= JUMP_TO_GLIDE_TIME - EPS && t0 - s.glideClosedT >= GLIDE_COOLDOWN - EPS
    && s.prop === GlideProp.CLOSED;
  if (s.glideWanted && canGlide && ready) {
    // the fall goes on while the prop opens: a landing meanwhile still takes a buffered jump
    s.prop = GlideProp.OPENING;
    s.propT = t0;
    s.glideOpenT = t0;
  } else if (s.prop === GlideProp.GLIDING) {
    if (!pressed || !canGlide) {
      // the prop that has just opened stays open for this step and closes on the next one
      if (s.propT < t0) {
        s.prop = GlideProp.CLOSING;
        s.propT = t0;
      }
    } else {
      // the glide holds the player's own fall at glidingFallingSpeed; the glider's drag takes a push, up or down
      glideDrag = true;
    }
  }

  // the player's own move over the step
  let own: number;
  if (hover) {
    // no gravity while the height is held
    own = s.vy * h;
  } else if (!onGround || justJumped) {
    own = ownMove(s, pressed, noGravity, t0, h, glideDrag ? -glidingFallingSpeed : -Infinity);
  } else {
    s.vy = 0;
    own = 0.5 * vy0 * h;
  }
  // scene pushes: while the ballistic speed rises it fades and is braked, which takes from their share; on the ground
  // the ballistic speed is all theirs, gravity takes it and the ground stops it; a glide's drag takes their share instead
  const vb = vb0 > 0 ? Math.max(0, vb0 * Math.exp(-h / VERTICAL_DAMP_TIME) - VERTICAL_STOP_DECEL * h) : vb0;
  if (onGround && !justJumped) {
    s.ve = Math.max(0, noGravity ? vb : vb - G * h);
    s.vg = 0;
  } else {
    if (glideDrag) s.ve *= Math.exp(-h / GLIDE_DAMP_TIME);
    else s.ve -= vb0 - vb;
    if (!hover && s.vy <= 0) s.vg = s.vy;
    else if (!noGravity) s.vg -= G * h;
  }
  // height: the player's own move and the pushes' share
  s.y += own + s.ve * h;
  s.yo += own;

  if (airPress && s.y <= groundY) {
    // the press step reaches the ground: the landing cancels the air jump, and the press is spent
    s.airLaunchT = NEVER;
    s.airPressT = NEVER;
  }
}

// The player's own vertical move over a step from t0 to t0 + h: a rise slows at JUMP_RISE_DECEL to the apex, then the
// fall gains G down to minVy (a glide's held fall); inside the held window (heldUntil) both are scaled by
// JUMP_HOLD_SCALE. Each piece has a constant acceleration and moves in closed form, the step split at the apex, at the
// end of the held window and where the fall reaches minVy, so the height does not depend on the step length. Updates
// s.vy and returns the height moved.
function ownMove(s: VState, pressed: boolean, noGravity: boolean, t0: number, h: number, minVy: number): number {
  const holdEnd = heldUntil(s, pressed);
  if (s.vy < minVy) s.vy = minVy;
  let dy = 0, t = 0;
  for (let k = 0; k < 6 && t < h - 1e-12; k++) {
    const held = t0 + t < holdEnd - 1e-12;
    let seg = held ? Math.min(h - t, holdEnd - t0 - t) : h - t;
    const scale = held ? JUMP_HOLD_SCALE : 1;
    if (s.vy > 0) {
      // the jump's own rise slows until the apex
      const a = JUMP_RISE_DECEL * scale;
      const apex = s.vy / a;
      if (apex <= seg) {
        dy += 0.5 * s.vy * apex;
        s.vy = 0;
        seg = apex;
      } else {
        dy += (s.vy - 0.5 * a * seg) * seg;
        s.vy -= a * seg;
      }
    } else if (!noGravity && s.vy > minVy) {
      const a = G * scale;
      const floor = (s.vy - minVy) / a;
      if (floor <= seg) {
        dy += 0.5 * (s.vy + minVy) * floor;
        s.vy = minVy;
        seg = floor;
      } else {
        dy += (s.vy - 0.5 * a * seg) * seg;
        s.vy -= a * seg;
      }
    } else {
      dy += s.vy * seg;
    }
    t += seg;
  }
  return dy;
}

// A jump replaces the player's own vertical velocity, so the ballistic speed starts again from what the pushes still
// add: an upward push only as far as it still carries the avatar up (gravity has taken the rest), a downward one in
// full. Returns the ballistic speed.
function keepPush(s: VState): number {
  if (s.ve > 0) s.ve = Math.max(0, s.vg + s.ve);
  s.vg = 0;
  return s.ve;
}

// A held key scales the player's own vertical acceleration for JUMP_HOLD_TIME after a jump's press (an air jump's from
// the air press, so the hover uses part of it). Any jump key held in it counts (a later, ignored press too). Returns
// the end of the window while the key is held (-Infinity: not held).
function heldUntil(s: VState, pressed: boolean): number {
  return pressed ? s.consumedT + JUMP_HOLD_TIME : -Infinity;
}

// Runs the steps that cover `time` (one per frame, see clock.ts).
function vadvance(s: VState, pressed: boolean, running: boolean, onGround: boolean, noGravity: boolean, time: number) {
  const n = stepsFor(time);
  for (let k = 0; k < n; k++) {
    vstep(s, pressed, running, onGround, noGravity, time / n);
    // after a launch the ground is left behind for the rest of the tick
    if (s.launched) onGround = false;
  }
}

function applyJumpAndGravity(noGravity: boolean) {
  const pressed = inputSystem.isPressed(InputAction.IA_JUMP);
  // a running jump: sprint held with a movement key down
  const running = inputSystem.isPressed(InputAction.IA_MODIFIER) && (
    inputSystem.isPressed(InputAction.IA_FORWARD) || inputSystem.isPressed(InputAction.IA_BACKWARD)
    || inputSystem.isPressed(InputAction.IA_LEFT) || inputSystem.isPressed(InputAction.IA_RIGHT)
    || Vector3.lengthSquared(movementAxis) > 0);

  if (horizontalResync) {
    // the engine moved us at another velocity: continue from what it applied
    vstate.vy = prevActualVelocity.y;
    vstate.vg = Math.min(0, vstate.vy);
    vstate.ve = 0;
  }

  groundY = groundDistance < Infinity ? playerPosition.y - Math.max(0, groundDistance) : -Infinity;
  // While the model holds an air jump's height above the ground, the avatar reaching the ground on a late reply is not a
  // landing: the air jump goes on.
  if (grounded && vstate.airLaunchT !== NEVER && vstate.t < vstate.airLaunchT - EPS && vstate.y > groundY && !isLaunched()) {
    setGrounded(false);
  }
  // a published launch the committed steps have not reached yet: the model is still on the ground, even when the
  // published launch has already lifted the avatar past the grounded height. A push that lifts the avatar arrived
  // after these steps (it is added below): they still run on the ground.
  const onGround = grounded || launchPending;
  // on the ground the model stands where the avatar stands (a published late jump off an edge launches from where the
  // model is, not from the ground below the avatar)
  if (onGround && vstate.vy <= 0 && (grounded || vstate.groundedLast)) vstate.y = playerPosition.y - Math.max(0, groundDistance);
  // in the air the stun's top is the avatar's highest point since the ground (above where the engine placed it), or its
  // height while gliding
  if (onGround && !noGravity) placedY = -Infinity;
  else if (!onGround) vstate.topY = vstate.prop === GlideProp.GLIDING ? playerPosition.y : playerPosition.y > placedY ? Math.max(vstate.topY, playerPosition.y) : vstate.topY;
  vstate.launched = false;
  vstate.airLaunched = false;
  const pressBefore = vstate.airPressT;
  // the committed steps up to the end of this frame run on the previous tick's keys
  vadvance(vstate, prevJumpPressed, prevRunning, onGround, noGravity, tickAdvance);
  // this tick's scene pushes (externalVelocity sums impulses and force * time since the last tick); on the ground one
  // too weak to lift the avatar (isLaunched) is held by the ground
  if (!onGround || vstate.launched || isLaunched()) {
    vstate.ve += prevExternalVelocity.y;
  }
  if (vstate.launched) {
    jumpStartHeight = playerPosition.y;
    setGrounded(false);
  }
  if (vstate.airLaunched) {
    jumpStartHeight = playerPosition.y;
  }
  if (vstate.airLaunched || (vstate.airLaunchT !== NEVER && vstate.airPressT !== pressBefore)) jumpTracked = true;
  // a published air jump (press or launch) is kept until the committed steps reach it (within half a step: the
  // prediction's steps need not end where the committed ones do)
  if (onGround || vstate.t >= airPublishedT - 0.5 * oneStep() || (vstate.airLaunchT === NEVER && vstate.triggerT === NEVER)) airPublishedT = NEVER;

  // a press is due on the first step after this frame
  if (pressed && !jumpWasPressed) {
    vstate.triggerT = vstate.t;
    // the frames of this tick before the current one (none when the scene answers every frame)
    vstate.pressBack = Math.max(0, tickAdvance - frameTime);
  }
  jumpWasPressed = pressed;
  prevJumpPressed = pressed;
  prevRunning = running;
  if (vstate.launched || vstate.triggerT === NEVER) launchPending = false;

  // The reply is held from the frame it lands on (replyDelay frames after this one) until the
  // next reply lands (2 * replyDelay frames after this one). A launch from the ground answers with
  // the mean of the predicted per-frame velocities over those frames, so the height the engine
  // integrates over them is the height the model reaches (a rise is not cut short); in the air the
  // reply carries the model's mean velocity over the frame it is predicted for (below).
  // A launch the prediction has already published but the committed steps have not reached
  // yet (launchPending) is kept: the prediction then covers at least the next step.
  let vy = vstate.vy;
  const frames = frameTime > 0 ? 2 * replyDelay : 0;
  // the smoothed horizon once the committed model is airborne; a launch from the ground keeps the per-tick
  // delay (a smoothed horizon would publish a launch before the press frame's reply is due)
  const smooth = frameTime > 0 && (vstate.vy !== 0 || vstate.launched || vstate.airLaunchT !== NEVER);
  const ahead = smooth ? 1.5 * replyDelaySmooth * frameTime : 0;
  if (vstate.launched) jumpTracked = true;
  const airAhead = airPublishedT > vstate.t ? airPublishedT - vstate.t : 0;
  let probeRan = false;
  // the model's own mean vertical velocity over the frame that ends at the predicted time (null: not taken)
  let meanVy: number | null = null;
  if ((smooth ? ahead > 1e-4 : frames > 0) || launchPending || airAhead > 0) {
    probeRan = true;
    copyVState(vstate, vprobe);
    vprobe.launched = false;
    vprobe.airLaunched = false;
    let ground = onGround && !vstate.launched;
    if (smooth || airAhead > 0) {
      // the frame that ends at the predicted time
      const end = Math.max(ahead, launchPending ? oneStep() : 0, airAhead);
      const frame = frameTime > 0 ? frameTime : oneStep();
      let from = ownTravelAt(end - frame);
      if (end > frame) {
        vadvance(vprobe, pressed, running, ground, noGravity, end - frame);
        if (vprobe.launched) ground = false;
        from = vprobe.yo;
        vadvance(vprobe, pressed, running, ground, noGravity, frame);
      } else {
        vadvance(vprobe, pressed, running, ground, noGravity, end);
      }
      vy = vprobe.vy;
      meanVy = (vprobe.yo - from) / frame;
    } else if (frames > 0) {
      let sum = 0, n = 0;
      for (let f = 1; f <= frames; f++) {
        vadvance(vprobe, pressed, running, ground, noGravity, frameTime);
        if (vprobe.launched) ground = false;
        if (f >= replyDelay) { sum += vprobe.vy; n++; }
      }
      vy = sum / n;
    } else {
      vadvance(vprobe, pressed, running, ground, noGravity, oneStep());
      vy = vprobe.vy;
    }
    if (vprobe.launched && !vstate.launched) launchPending = true;
    if (vprobe.launched) jumpTracked = true;
    if (vprobe.airLaunchT !== NEVER && (vprobe.airPressT >= vstate.t - EPS || vprobe.airLaunched)) {
      airPublishedT = Math.max(airPublishedT, vprobe.airLaunchEnd !== NEVER ? Math.min(vprobe.t, vprobe.airLaunchEnd) : vprobe.t);
      jumpTracked = true;
    }
  }
  // a reply applied over this frame: the model's mean over it
  if (!probeRan && smooth) meanVy = (vstate.yo - ownTravelAt(-frameTime)) / frameTime;
  // an air jump's hold or launch is in this reply (committed or published)
  const airJump = vstate.airLaunched || (vstate.airLaunchT !== NEVER && vstate.t < vstate.airLaunchT - EPS)
    || (probeRan && (vprobe.airLaunched || (vprobe.airLaunchT !== NEVER && vprobe.t < vprobe.airLaunchT - EPS)));
  // A reply carries the model's mean velocity over its frame, so the height the engine integrates is the model's
  // whatever the frame length; a launch or an air jump's held height is carried as it is, at once.
  if (meanVy !== null && !airJump && !vstate.launched && !(probeRan && vprobe.launched) && !launchPending) vy = meanVy;
  const hoverReply = (vstate.airLaunchT !== NEVER && vstate.t < vstate.airLaunchT - EPS && (!probeRan || vprobe.t < vprobe.airLaunchT - EPS))
    || (probeRan && vprobe.airLaunchT !== NEVER && vprobe.t < vprobe.airLaunchT - EPS && vprobe.t > vstate.t + EPS);
  if (launchPending && jumpStartHeight === undefined) {
    // published launch: the jump starts now. The grounded state stays, so the launch step's
    // horizontal move is still a ground move.
    jumpStartHeight = playerPosition.y;
  }
  // Until the committed steps launch, the model is on the ground even when the published launch has already
  // lifted the avatar past the grounded height: the launch step's horizontal move stays a ground move.
  launchAhead = launchPending && !vstate.launched;
  if (launchAhead) setGrounded(true);

  // Height tracking (VERTICAL_TRACK_TIME): from a launch (committed or published) until the model lands.
  // Tracking stops when the engine did not move the avatar as asked: a rise cut short (a ceiling) or a lift it did not
  // ask for (a platform, a slope). The gap to the model is then geometry, not reply timing. In free flight the applied
  // vertical velocity equals the requested one; the ground stopping a fall is the landing, handled below.
  const blocked = (prevRequestedVelocity.y > 0 && prevActualVelocity.y < prevRequestedVelocity.y - VERTICAL_TRACK_BLOCK)
    || prevActualVelocity.y > Math.max(0, prevRequestedVelocity.y) + VERTICAL_TRACK_BLOCK;
  if (jumpTracked && (vstate.prop === GlideProp.GLIDING || vstate.ve !== 0 || horizontalResync || noGravity || blocked
    || (onGround && vstate.vy <= 0 && !vstate.launched && !launchPending))) jumpTracked = false;
  if (jumpTracked && frameTime > 0) {
    // compared at the start of this frame, where the avatar's height is known whichever reply the engine applies
    // over this frame (the model state is at the end of this frame; its last step ends with this frame)
    const h = vstate.t - vstate.t0;
    const yStart = h > 0 ? vstate.y - (vstate.y - vstate.y0) * Math.min(1, frameTime / h) : vstate.y;
    const err = yStart - playerPosition.y;
    let c = Math.max(-VERTICAL_TRACK_MAX, Math.min(VERTICAL_TRACK_MAX, err / VERTICAL_TRACK_TIME));
    if (hoverReply) {
      // an air jump's held height: the reply stays flat (tracking resumes at the launch)
      c = 0;
    } else {
      // never against the model's direction: no hop while it falls, no dip while it rises
      if (vy + c > 0 && vy <= 0) c = -vy;
      if (vy + c < 0 && vy > 0) c = -vy;
    }
    // a correction upward never makes the reply rise above both the model's own reply and the last airborne reply
    // (the ramp guard below only covers ticks without a launch; this covers the launch ticks too)
    const model = vy;
    vy += c;
    if (c > 0 && lastReplyVy !== 0) vy = Math.min(vy, Math.max(model, lastReplyVy));
  }

  // Predicted landing: frames from this one until the engine puts the avatar on the ground,
  // with the previous reply held for the frames before this one's lands (-1: not in the window).
  // Not on the launch tick: the avatar still touches the ground then, and the contact's tiny
  // height (either sign) must not read as a landing.
  predictedLandFrame = -1;
  if (!grounded && !vstate.launched && frameTime > 0 && vstate.prop !== GlideProp.GLIDING && groundDistance < Infinity) {
    let h = groundDistance;
    for (let k = 0; k <= frames; k++) {
      h += (k < replyDelay ? prevActualVelocity.y : vy + vstate.ve) * frameTime;
      if (h <= 0) { predictedLandFrame = k; break; }
    }
  }

  glideProp = vstate.prop;
  isGliding = vstate.prop === GlideProp.GLIDING;
  const air = vstate.airLaunchT !== NEVER ? vstate : probeRan && vprobe.airLaunchT !== NEVER ? vprobe : null;
  isDoubleJump = air !== null && air.glideOpenT < air.airPressT;
  if ((vstate.vy <= 0 && !launchPending) || isGliding) jumpStartHeight = undefined;

  // Vertical ramp guard: in the air gravity only lowers vy, but the switching delay estimate
  // makes consecutive replies alternate between the committed and the predicted state. Between
  // two airborne replies without a launch, glide, landing or push, a reply never rises above
  // the last one. It runs after the height tracking: a correction can slow a fall but never makes vy rise in the air.
  const inAir = !grounded && !vstate.launched && !launchPending && !isGliding && vstate.ve === 0
    && !horizontalResync && predictedLandFrame < 0 && !airJump;
  if (inAir && lastReplyAir) vy = Math.min(vy, lastReplyVy);
  lastReplyVy = vy;
  lastReplyAir = inAir;

  velocity.y = vy + vstate.ve;
}
// A jump is in progress (launched or published): replies track the model's height (VERTICAL_TRACK_MAX).
var jumpTracked = false;
// The committed model's own vertical travel `dt` after the end of its steps (dt <= 0: within its last step, where the
// travel is taken as linear).
function ownTravelAt(dt: number): number {
  const h = vstate.t - vstate.t0;
  if (dt >= 0 || h <= 0) return vstate.yo;
  return vstate.yo + (vstate.yo - vstate.yo0) * Math.max(-1, dt / h);
}
// End of the furthest step of an air jump (press .. launch) a reply has published; later predictions reach at least
// that far.
var airPublishedT = NEVER;
// The time the committed air jump launches at (NEVER: none), for the horizontal launch push: the first step starting
// at or after it launches.
export function airLaunchTime(): number { return vstate.airLaunchT; }
// Landing stun: horizontal movement stops on the steps of the committed model that start before stunEndTime(). Before
// the committed model reaches the ground, landingStuns() tells whether that landing will stun (the drop from the top
// to the ground under the avatar is over LANDING_STUN_DROP and the scene's hardLandingCooldown is above 0); once landed
// the top is the ground and it is false.
export function stunEndTime(): number { return vstate.stunEndT; }
export function stunned(): boolean { return vstate.t0 < vstate.stunEndT - EPS; }
export function landingStuns(): boolean { return landingStunTime > 0 && vstate.topY - groundY > LANDING_STUN_DROP; }
// Height the engine last placed the avatar at, while it has not touched the ground since (a landing stun's top ignores
// heights up to it).
var placedY = -Infinity;
// Vertical velocity of the last reply and whether it was airborne (ramp guard).
var lastReplyVy = 0;
var lastReplyAir = false;

// An upward push stronger than gravity is meant to lift the avatar, so like a jump it leaves
// the ground rather than being snapped back down. A weaker one (externalVelocity is summed
// over the tick, so a force shows up as force * tickTime) stays on the ground.
function isLaunched(): boolean {
  return prevExternalVelocity.y > -GRAVITY.y * tickTime;
}

// ---------------------------------------------------------------- the ground: step up, ground snap, ground following
// The engine moves the avatar by collide-and-slide alone, so the reply keeps it on the ground. Four casts read the
// ground: the capsule and the ray down from the feet (ground.ts), and here the capsule along the facing and the ray
// down past the face it finds.

// The forward cast reads a face the capsule touches (WALL_CONTACT_DISTANCE) and the risers and steep slopes ahead. It
// reaches further: the engine casts against the scenes its centre line crosses, so a wall on the next scene's edge is
// found only once that line reaches the edge, which this length does for a capsule meeting the wall up to about 75
// degrees off its normal.
const FWD_CAST_LENGTH = 1.3; // m
// Landing probe: a ray down onto the tread past the face the forward cast found, from above MAX_STEP_HEIGHT so a tread
// at exactly max step height is still hit. Each tick moves it to the face found that tick, LANDING_CAST_INSET past it;
// it reads there on the next tick, when the avatar has moved on by up to a frame, so up to LANDING_CAST_SLACK further.
const LANDING_CAST_HEIGHT = MAX_STEP_HEIGHT + 0.1;
const LANDING_CAST_INSET = 0.05; // past the riser face, so the probe lands on the tread rather than the riser
const LANDING_CAST_SLACK = 0.4; // m, a run's frame at 30 fps and more
// It reaches below the feet, not only down to them: treadRise counts the rise from the ground under the avatar,
// landingHeight + max(0, groundDistance), so while the avatar is above that ground a tread below its feet can still be a
// step. Below the feet it reaches as far as a walkable slope drops over the forward cast's length, about 1.4 m.
const LANDING_CAST_RANGE = LANDING_CAST_HEIGHT + FWD_CAST_LENGTH * Math.tan(GROUNDED_ANGLE * Math.PI / 180);

// cast fwd (nearest hit)
var fwdDistance: number = Infinity; // capsule travel to the hit
var fwdWalkable: boolean = true;
var fwdNormal: Vector3 = Vector3.Zero();
// cast down onto the tread past the face: height above the feet (-Infinity: no hit) and walkability (false for a probe
// that starts inside a collider)
var landingHeight = -Infinity;
var landingWalkable = false;
var landingCaster: Entity;
// the forward cast's distance the probe read this tick was placed at, and the one it is placed at for the next tick
var landingFrom = 0;
var landingNext = 0;

export function initStepCasts() {
  function initCast(position: Vector3, local: boolean, opts: { direction: Vector3, maxDistance: number, shape: RaycastShape, queryType?: RaycastQueryType }, cb: RaycastSystemCallback): Entity {
    const e = engine.addEntity();
    Transform.create(e, { parent: engine.PlayerEntity, position });

    const raycastData = {
      entity: e,
      opts: {
        queryType: RaycastQueryType.RQT_HIT_FIRST,
        ...opts,
        continuous: true,
        collisionMask: ColliderLayer.CL_PHYSICS,
        includeWorld: true,
      }
    };
    // local directions follow avatar yaw (the cast entity is parented to the
    // player with identity rotation)
    if (local) {
      raycastSystem.registerLocalDirectionRaycast(raycastData, cb);
    } else {
      raycastSystem.registerGlobalDirectionRaycast(raycastData, cb);
    }
    return e;
  }

  initCast({ x: 0, y: 0, z: 0 }, true, { direction: Vector3.Forward(), maxDistance: FWD_CAST_LENGTH, shape: RaycastShape.RS_AVATAR }, (hits) => {
    fwdDistance = Infinity;
    fwdWalkable = true;

    for (const hit of hits.hits) {
      fwdDistance = Math.min(fwdDistance, hit.length);
      fwdWalkable = (hit.normalHit?.y ?? 0) > GROUNDED_ANGLE_Y_LEN;
      Vector3.copyFrom(hit.normalHit ?? VEC3_ZERO, fwdNormal);
    }
  })

  // local forward offset, along the avatar's facing (the parent's yaw turns the position)
  landingCaster = initCast({ x: 0, y: LANDING_CAST_HEIGHT, z: PLAYER_COLLIDER_RADIUS + LANDING_CAST_INSET }, false, { direction: Vector3.Down(), maxDistance: LANDING_CAST_RANGE, shape: RaycastShape.RS_RAY }, (hits) => {
    landingHeight = -Infinity;
    landingWalkable = false;
    for (const hit of hits.hits) {
      const ny = hit.normalHit?.y ?? 0;
      landingHeight = LANDING_CAST_HEIGHT - hit.length;
      landingWalkable = hit.length > 1e-4 && ny >= GROUNDED_ANGLE_Y_LEN;
    }
  })
}

// Moves the landing probe to the face the forward cast found this tick (none: just past the capsule).
function placeLandingProbe() {
  landingFrom = landingNext;
  landingNext = fwdDistance < Infinity ? fwdDistance : 0;
  Transform.getMutable(landingCaster).position.z = PLAYER_COLLIDER_RADIUS + LANDING_CAST_INSET + landingNext;
}

// The tread past the face along the facing: its height above the ground under the feet. NaN: the probe did not read
// past this face; -Infinity: no ground there that the avatar can stand on.
function treadRise(): number {
  if (fwdDistance === Infinity) return NaN;
  const past = landingFrom + LANDING_CAST_INSET - fwdDistance;
  if (past <= 0 || past > LANDING_CAST_INSET + LANDING_CAST_SLACK) return NaN;
  return landingWalkable ? landingHeight + Math.max(0, groundDistance) : -Infinity;
}

// The face along the facing is a riser the avatar steps onto: too steep to stand on, with walkable ground past it
// MIN_STEP_HEIGHT to MAX_STEP_HEIGHT above the ground under the feet.
function stepAhead(): boolean {
  if (fwdWalkable) return false;
  const rise = treadRise();
  return rise >= MIN_STEP_HEIGHT && rise <= MAX_STEP_HEIGHT + 1e-3;
}

// A face too steep to walk that the capsule touches along the facing (a wall, a steep slope, a riser higher than a
// step): the horizontal unit normal pointing away from it; null if none.
const wallFace = { x: 0, z: 0 };
export function wallContact(): { x: number, z: number } | null {
  if (fwdDistance > WALL_CONTACT_DISTANCE || fwdWalkable || stepping || stepAhead()) return null;
  const h = Math.hypot(fwdNormal.x, fwdNormal.z);
  if (h < 1e-6) return null;
  wallFace.x = fwdNormal.x / h;
  wallFace.z = fwdNormal.z / h;
  return wallFace;
}
const WALL_CONTACT_DISTANCE = 0.02; // m of capsule travel to the face

// Slope limit: a slope ahead along the facing too steep to stand on (not a wall, not a riser): the horizontal unit
// normal pointing away from it and the capsule's distance to it; null if none. The engine's collide-and-slide would
// slide a move into it up the slope; the horizontal reply stops it at the face instead.
const steepFace = { x: 0, z: 0, distance: 0 };
export function steepAhead(): { x: number, z: number, distance: number } | null {
  if (fwdDistance === Infinity || fwdWalkable || fwdNormal.y < 0.05 || stepping || stepAhead()) return null;
  const h = Math.hypot(fwdNormal.x, fwdNormal.z);
  if (h < 1e-6) return null;
  steepFace.x = fwdNormal.x / h;
  steepFace.z = fwdNormal.z / h;
  steepFace.distance = fwdDistance;
  return steepFace;
}

// Step up: on the ground, a move along the facing toward a riser (stepAhead) rises onto its tread. A move at
// STEP_UP_PLAN_SPEED or faster plans the climb once the capsule would reach the face within STEP_UP_REACH_TIME plus
// twice the reply delay, so the feet are on the tread as the capsule reaches it; a slower one climbs once it touches
// the face, over STEP_UP_TIME, so a walk pauses at each riser. A move that has just left the ground (a climb that rose
// past the tread) still plans the next riser. The climb keeps the fastest speed planned, and holds the grounded state
// until the ground cast finds the feet on the tread, the keys are released or it stops rising.
var stepping = false;
// vertical speed of the last climb reply (0 when the last reply did not climb)
var climbSent = 0;
// feet height the climb is ascending to: the highest tread planned so far
var stepTargetY = 0;
// climb speed: the fastest planned
var stepSpeed = 0;
var prevSteppingY = 0;
// consecutive ticks a climb has not risen (the first ones wait for the reply to land)
var stepStalls = 0;
// the tread the latest plan climbs to
var planTargetY = -Infinity;
const facing: Vector3.MutableVector3 = Vector3.create(0, 0, 0);

function stepUp() {
  placeLandingProbe();
  Vector3.rotateToRef(VEC3_FORWARD, playerRotation, facing);
  facing.y = 0;
  Vector3.normalizeToRef(facing, facing);
  const along = velocity.x * facing.x + velocity.z * facing.z;
  const need = along > 0.1 && Vector3.dot(facing, movementAxis) > 0.9 && (grounded || prevGrounded || stepping)
    && jumpStartHeight === undefined ? planStepUp(along) : -Infinity;
  if (stepping) stepStalls = playerPosition.y <= prevSteppingY + 1e-4 ? stepStalls + 1 : 0;
  if (need > -Infinity) {
    if (!stepping) {
      stepping = true;
      stepStalls = 0;
      prevSteppingY = -Infinity;
      stepTargetY = planTargetY;
      stepSpeed = 0;
    }
    stepTargetY = Math.max(stepTargetY, planTargetY);
    stepSpeed = Math.max(stepSpeed, need);
  }
  if (!stepping) return;
  // the climb ends once the ground cast finds the feet on the tread: until then it reads the ground the climb left
  const onTread = playerPosition.y >= stepTargetY - 1e-3 && groundDistance <= GROUNDED_HEIGHT;
  if (Vector3.lengthSquared(movementAxis) === 0 || onTread || stepStalls > replyDelay + 1) {
    stepping = false;
    return;
  }
  prevSteppingY = playerPosition.y;
  // a climb is ground locomotion: no airborne flicker or landing on each riser
  setGrounded(true);
}

// The vertical speed that puts the feet on the tread past the riser ahead in time (-Infinity: nothing to climb yet), at
// `speed` toward it; sets planTargetY.
function planStepUp(speed: number): number {
  if (!stepAhead()) return -Infinity;
  const frame = oneStep();
  const delay = replyDelay * frame;
  const reach = fwdDistance / speed;
  let climb: number;
  if (speed >= STEP_UP_PLAN_SPEED) {
    // reached later: a later tick plans it
    if (reach > STEP_UP_REACH_TIME + 2 * delay) return -Infinity;
    climb = Math.max(reach - delay, frame);
  } else {
    if (fwdDistance > WALL_CONTACT_DISTANCE) return -Infinity;
    climb = STEP_UP_TIME;
  }
  planTargetY = playerPosition.y + landingHeight;
  return Math.max(0, landingHeight) / climb;
}

// Ground snap: on the ground last tick and not jumping, the avatar follows walkable ground under its centre down by up
// to GROUND_SNAP_STEP_DROP beyond the ground's slope over the distance moved (centreSupported), measured from the ground
// it stood on, or from the edge it rested on. A larger drop is an edge it walks off. The distance to move down: to the
// capsule's contact, or to the ground under the centre when the capsule only touches a face too steep to stand on.
var snapSpeed = 0;
// the reply follows the ground under the centre (groundSpeed)
var followsGround = false;
const SNAP_MIN = 1e-3; // m

function snapDistance(): number {
  if (stepping || !prevGrounded || jumpStartHeight !== undefined || isLaunched() || !centreSupported()) return 0;
  const d = groundNormal.y >= GROUNDED_ANGLE_Y_LEN ? groundDistance : groundRayDistance;
  return d > SNAP_MIN ? d : 0;
}

// The vertical reply on the ground on top of the model's: ground following, the vertical speed that moves the
// horizontal (vx, vz) along the walkable ground under the centre, so the engine's collide-and-slide neither slows a move
// up a slope nor leaves the slope going down; and the ground snap's speed down (0 when neither applies).
export function groundSpeed(vx: number, vz: number): { follow: number, snap: number } {
  groundReply.follow = followsGround ? -(groundRayNormal.x * vx + groundRayNormal.z * vz) / groundRayNormal.y : 0;
  groundReply.snap = snapSpeed;
  return groundReply;
}
const groundReply = { follow: 0, snap: 0 };

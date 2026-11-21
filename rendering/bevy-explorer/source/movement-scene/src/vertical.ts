import { ColliderLayer, engine, Entity, InputAction, inputSystem, RaycastQueryType, RaycastShape, raycastSystem, RaycastSystemCallback, Transform } from '@dcl/sdk/ecs'
import { Vector3 } from '@dcl/sdk/math';
import { centreSupported, groundDistance, grounded, GROUNDED_ANGLE_Y_LEN, groundNormal, prevGrounded, setGrounded } from './ground';
import {
  AIR_JUMP_COOLDOWN, AIR_JUMP_HOVER_SPEED, AIR_JUMP_HOVER_TIME,
  FIXED_STEP, GLIDE_CLOSE_TIME, GLIDE_COOLDOWN, GLIDE_DAMP_TIME, GLIDE_MIN_GROUND_DISTANCE, GLIDE_OPEN_TIME,
  GRAVITY,
  JUMP_GRACE_STEPS, JUMP_HOLD_SCALE, JUMP_HOLD_TIME, JUMP_RISE_DECEL,
  JUMP_TO_GLIDE_TIME, LANDING_STUN_DROP, MAX_FIXED_STEPS, MAX_STEP_HEIGHT, MIN_STEP_HEIGHT,
  GROUNDED_ANGLE, PLAYER_COLLIDER_RADIUS, VEC3_ZERO,
  VERTICAL_DAMP_TIME, VERTICAL_STOP_DECEL, VERTICAL_TRACK_BLOCK, VERTICAL_TRACK_MAX, VERTICAL_TRACK_TIME,
} from './constants';
import { engineDriven, horizontalResync, playerPosition, prevActualVelocity, prevExternalVelocity, prevRequestedVelocity, tickTime, velocity } from '.';
import { movementAxis } from './horizontal';
import { doubleJumpHeight, glideEnabled, glidingFallingSpeed, jumpHeight, landingStunTime, maxAirJumps, maxGroundJumps, sprintJumpHeight, sprintSpeed } from './parameters';
import { frameTime, replyDelay, replyDelaySmooth, tickAdvance } from './clock';

const G = -GRAVITY.y;

export function updateVerticalVelocity() {
  leftEdge = false;
  if (engineDriven) {
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
  leaveEdge();
  applyJumpAndGravity(false);
  // pushed upward: off the ground for this tick's horizontal move and the next tick
  if (isLaunched()) setGrounded(false);
}

// The avatar stands on an edge: grounded with no ground under the centre (centreSupported), only the rim of the capsule
// resting on the corner, its centre R * sin(tilt) past it, where the tilt of the contact normal reaches GROUNDED_ANGLE at
// the most. A move off the edge that carries the centre past that before the next reply lands leaves the ground at
// once: it goes on at air speed and falls (a jog released at a ledge carries on past it), where a slower one stays on
// the edge until then. Standing on an edge, the engine's ground snap settles the capsule onto the corner.
var onEdge = false;
// The avatar leaves the edge this tick.
export var leftEdge = false;
const EDGE_REACH = PLAYER_COLLIDER_RADIUS * Math.sin(GROUNDED_ANGLE * Math.PI / 180);
function leaveEdge() {
  onEdge = grounded && jumpStartHeight === undefined && !centreSupported();
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
// Integrated in fixed steps on the same clock as the horizontal model (see clock.ts).

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
  step: number; // steps run so far
  lastGroundedStep: number;
  triggerStep: number; // step the latest press is due on
  pressBack: number; // steps the latest press may predate its due step (it was made in an earlier frame of its tick)
  consumedStep: number; // step the latest jump launched on
  jumpCount: number; // jumps since the ground (falling past the grace window counts as one)
  glideWanted: boolean;
  prop: GlideProp; // the glider prop's state
  propStep: number; // step the prop entered that state
  glideOpenStep: number; // step the prop last started opening
  glideClosedStep: number; // step the prop last closed: the glide cooldown counts from it
  launched: boolean; // a jump launched during the last advance
  y: number; // model feet height: the ground height while on the ground, integrated in the air
  airPressStep: number; // step of this airtime's air-jump press
  airLaunchStep: number; // step the air jump launches on (held height before it); NEVER: none in this airtime
  airLaunched: boolean; // an air jump launched during the last advance
  topY: number; // height a landing stun measures the drop from: the ground, then the avatar's highest since
  stunEndStep: number; // first step after a landing stun
};
const NEVER = -1e9;
const vstate: VState = {
  vy: 0, vg: 0, ve: 0, step: 0, lastGroundedStep: NEVER, triggerStep: NEVER, consumedStep: NEVER, jumpCount: 0, glideWanted: false,
  prop: GlideProp.CLOSED, propStep: NEVER, glideOpenStep: NEVER, glideClosedStep: NEVER, launched: false, y: 0,
  airPressStep: NEVER, airLaunchStep: NEVER, airLaunched: false,
  topY: -Infinity, stunEndStep: NEVER, pressBack: 0,
};
const vprobe: VState = { ...vstate };
var vpending = 0;
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
  to.vy = from.vy; to.vg = from.vg; to.ve = from.ve; to.step = from.step; to.lastGroundedStep = from.lastGroundedStep; to.triggerStep = from.triggerStep;
  to.consumedStep = from.consumedStep; to.jumpCount = from.jumpCount; to.glideWanted = from.glideWanted;
  to.prop = from.prop; to.propStep = from.propStep; to.glideOpenStep = from.glideOpenStep;
  to.glideClosedStep = from.glideClosedStep; to.launched = from.launched; to.y = from.y;
  to.airPressStep = from.airPressStep; to.airLaunchStep = from.airLaunchStep; to.airLaunched = from.airLaunched;
  to.topY = from.topY; to.stunEndStep = from.stunEndStep;
  to.pressBack = from.pressBack;
}

// Ground height under the avatar this tick (-Infinity: none in range). An air jump needs the model above it.
var groundY = -Infinity;
// Steps between an air press and its launch: the height is held on every step in between.
const AIR_HOVER_STEPS = Math.round(AIR_JUMP_HOVER_TIME / FIXED_STEP);
const GLIDE_OPEN_STEPS = Math.round(GLIDE_OPEN_TIME / FIXED_STEP);
const GLIDE_CLOSE_STEPS = Math.round(GLIDE_CLOSE_TIME / FIXED_STEP);

function airJumpAllowed(s: VState, h: number): boolean {
  return maxAirJumps > 0 && s.jumpCount >= 1 && Math.max(s.jumpCount, 1) <= maxAirJumps
    && (s.step - s.consumedStep) * h >= AIR_JUMP_COOLDOWN - 1e-9 && s.y > groundY;
}

function launchSpeed(running: boolean): number {
  // Use the more conservative of prev requested vs prev actual so external forces (impulses,
  // moving platforms) don't inflate the running jump.
  const req = Math.sqrt(prevRequestedVelocity.x * prevRequestedVelocity.x + prevRequestedVelocity.z * prevRequestedVelocity.z);
  const act = Math.sqrt(prevActualVelocity.x * prevActualVelocity.x + prevActualVelocity.z * prevActualVelocity.z);
  const maxHeight = running ? sprintJumpHeight : jumpHeight;
  const k = sprintSpeed > 0 ? Math.min(1, Math.max(0, Math.min(req, act) / sprintSpeed)) : 0;
  currentJumpHeight = jumpHeight + (maxHeight - jumpHeight) * k;
  return Math.sqrt(2 * Math.max(0, currentJumpHeight) * JUMP_RISE_DECEL);
}

function vstep(s: VState, pressed: boolean, running: boolean, onGroundTick: boolean, noGravity: boolean, h: number) {
  s.step++;
  let vy0 = s.vy;
  let vb0 = s.vg + s.ve; // the ballistic speed at the start of the step
  let justJumped = false;
  let glideDrag = false;
  const onGround = onGroundTick && s.vy <= 0;
  if (onGround) {
    // a landing more than LANDING_STUN_DROP below the top of the airtime stuns for landingStunTime, in whole steps
    if (s.lastGroundedStep !== s.step - 1 && s.topY - s.y > LANDING_STUN_DROP) {
      s.stunEndStep = s.step + landingStunSteps();
    }
    s.topY = s.y;
    s.lastGroundedStep = s.step;
    s.jumpCount = 0;
    s.airLaunchStep = NEVER; // the landing cancels an air jump that has not launched
  }
  // an air jump holds the height until its launch step; no other jump starts meanwhile
  const hover = s.airLaunchStep !== NEVER && s.step < s.airLaunchStep;
  if (hover) {
    s.vy = AIR_JUMP_HOVER_SPEED;
    vy0 = s.vy;
  } else if (s.step === s.airLaunchStep) {
    currentJumpHeight = doubleJumpHeight;
    s.vy = Math.sqrt(2 * Math.max(0, doubleJumpHeight) * JUMP_RISE_DECEL);
    s.airLaunched = true;
  }

  let airPress = false;
  if (!hover) {
    // the grace window both buffers an early press and allows a late jump off an edge
    const grace = s.vy > 0 || s.step < JUMP_GRACE_STEPS ? 0 : JUMP_GRACE_STEPS;
    // a late jump off an edge counts from when the press may have been made, not from when the tick saw it
    const coyote = s.step - (s.step === s.triggerStep ? s.pressBack : 0) - s.lastGroundedStep < grace;
    if (!onGround && !coyote) s.jumpCount = Math.max(s.jumpCount, 1);
    const canJump = s.jumpCount === 0 && (onGround || coyote) && maxGroundJumps > 0 && jumpHeight > 0;
    const wants = s.step === s.triggerStep || s.step <= s.triggerStep + grace;
    if (canJump && wants) {
      s.vy = launchSpeed(running);
      vb0 = keepPush(s);
      justJumped = true;
      s.launched = true;
      s.jumpCount++;
      s.triggerStep = NEVER;
      s.consumedStep = s.step;
    } else if (wants && !onGround && airJumpAllowed(s, h)) {
      // air jump: gravity still acts on this step, the height is held from the next one
      airPress = true;
      vb0 = keepPush(s);
      s.jumpCount++;
      s.triggerStep = NEVER;
      s.consumedStep = s.step;
      s.airPressStep = s.step;
      s.airLaunchStep = s.step + AIR_HOVER_STEPS + 1;
    }
  }

  if (hover) {
    // no gravity while the height is held
  } else if (!onGround || justJumped) {
    const scale = heldWindow(s, pressed, h) ? JUMP_HOLD_SCALE : 1;
    if (s.vy > 0) {
      // the jump's own rise slows until the apex
      s.vy -= JUMP_RISE_DECEL * scale * h;
      if (noGravity) s.vy = Math.max(0, s.vy);
    } else if (!noGravity) {
      s.vy -= G * scale * h;
    }
  } else {
    s.vy = 0;
  }

  // glide: a fresh press in the air once the jumps are spent opens the glider's prop, held to keep gliding
  // the model's own height above the ground (the avatar's lags it by the reply delay)
  const canGlide = glideEnabled && s.jumpCount > maxAirJumps && !onGround && s.y - groundY > GLIDE_MIN_GROUND_DISTANCE;
  // the prop's clips run on their own time: an open prop glides, a shut one starts the cooldown. Once it starts opening
  // it opens fully, through a release (a tap) or a landing, and closes from there on the next step.
  if (s.prop === GlideProp.OPENING && s.step - s.propStep >= GLIDE_OPEN_STEPS) {
    s.prop = GlideProp.GLIDING;
    s.propStep = s.step;
  } else if (s.prop === GlideProp.CLOSING && s.step - s.propStep >= GLIDE_CLOSE_STEPS) {
    s.prop = GlideProp.CLOSED;
    s.propStep = s.step;
    s.glideClosedStep = s.step;
  }
  s.glideWanted = (s.glideWanted || (canGlide && s.step === s.triggerStep)) && pressed;
  const ready = (s.step - s.consumedStep) * h >= JUMP_TO_GLIDE_TIME && (s.step - s.glideClosedStep) * h >= GLIDE_COOLDOWN
    && s.prop === GlideProp.CLOSED;
  if (s.glideWanted && canGlide && ready) {
    // the fall goes on while the prop opens: a landing meanwhile still takes a buffered jump
    s.prop = GlideProp.OPENING;
    s.propStep = s.step;
    s.glideOpenStep = s.step;
  } else if (s.prop === GlideProp.GLIDING) {
    if (!pressed || !canGlide) {
      // the prop that has just opened stays open for this step and closes on the next one
      if (s.propStep < s.step) {
        s.prop = GlideProp.CLOSING;
        s.propStep = s.step;
      }
    } else {
      // the glide holds the player's own fall at glidingFallingSpeed; the glider's drag takes a push, up or down
      s.vy = Math.max(s.vy, -glidingFallingSpeed);
      glideDrag = true;
    }
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
  // height: the acceleration is constant within a step, so the step moves the mean of its start and end velocity, and
  // the pushes' share
  s.y += (0.5 * (vy0 + s.vy) + s.ve) * h;

  if (airPress && s.y <= groundY) {
    // the press step reaches the ground: the landing cancels the air jump, and the press is spent
    s.airLaunchStep = NEVER;
    s.airPressStep = NEVER;
  }
}

// A jump replaces the player's own vertical velocity, so the ballistic speed starts again from what the pushes still
// add: an upward push only as far as it still carries the avatar up (gravity has taken the rest), a downward one in
// full. Returns the ballistic speed.
function keepPush(s: VState): number {
  if (s.ve > 0) s.ve = Math.max(0, s.vg + s.ve);
  s.vg = 0;
  return s.ve;
}

// A held key scales the player's own vertical acceleration for JUMP_HOLD_TIME after a jump's press. For an air jump the
// window counts from the air press, so the hover uses part of it, and any jump key held in it counts (a later, ignored
// press too).
function heldWindow(s: VState, pressed: boolean, h: number): boolean {
  if (!pressed) return false;
  if (s.airLaunchStep === NEVER || s.consumedStep !== s.airPressStep) return (s.step - s.consumedStep) * h < JUMP_HOLD_TIME;
  return s.step >= s.airPressStep && (s.step - s.airPressStep) * h < JUMP_HOLD_TIME;
}

function vadvance(s: VState, pressed: boolean, running: boolean, onGround: boolean, noGravity: boolean, t: number): number {
  let n = 0;
  while (t >= FIXED_STEP - 1e-9 && n < MAX_FIXED_STEPS) {
    vstep(s, pressed, running, onGround, noGravity, FIXED_STEP);
    t -= FIXED_STEP;
    n++;
    // after a launch the ground is left behind for the rest of the tick
    if (s.launched) onGround = false;
  }
  return n === MAX_FIXED_STEPS ? 0 : t;
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
  if (grounded && vstate.airLaunchStep !== NEVER && vstate.step < vstate.airLaunchStep && vstate.y > groundY && !isLaunched()) {
    setGrounded(false);
  }
  // a published launch the committed steps have not reached yet: the model is still on the ground, even when the
  // published launch has already lifted the avatar past the grounded height. A push that lifts the avatar arrived
  // after these steps (it is added below): they still run on the ground.
  const onGround = grounded || launchPending;
  // on the ground the model stands where the avatar stands (a published late jump off an edge launches from where the
  // model is, not from the ground below the avatar)
  if (onGround && vstate.vy <= 0 && (grounded || vstate.lastGroundedStep === vstate.step)) vstate.y = playerPosition.y - Math.max(0, groundDistance);
  // in the air the stun's top is the avatar's highest point since the ground (above where the engine placed it), or its
  // height while gliding
  if (onGround && !noGravity) placedY = -Infinity;
  else if (!onGround) vstate.topY = vstate.prop === GlideProp.GLIDING ? playerPosition.y : playerPosition.y > placedY ? Math.max(vstate.topY, playerPosition.y) : vstate.topY;
  vstate.launched = false;
  vstate.airLaunched = false;
  const pressBefore = vstate.airPressStep;
  // the committed steps up to the end of this frame run on the previous tick's keys
  vpending = vadvance(vstate, prevJumpPressed, prevRunning, onGround, noGravity, vpending + tickAdvance);
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
  if (vstate.airLaunched || (vstate.airLaunchStep !== NEVER && vstate.airPressStep !== pressBefore)) jumpTracked = true;
  // a published air jump (press or launch) is kept until the committed steps reach it
  if (onGround || vstate.step >= airPublishedStep || (vstate.airLaunchStep === NEVER && vstate.triggerStep === NEVER)) airPublishedStep = NEVER;

  // a press is due on the first step after this frame
  if (pressed && !jumpWasPressed) {
    vstate.triggerStep = vstate.step + 1;
    // the frames of this tick before the current one (none when the scene answers every frame)
    vstate.pressBack = Math.max(0, Math.round((tickAdvance - frameTime) / FIXED_STEP));
  }
  jumpWasPressed = pressed;
  prevJumpPressed = pressed;
  prevRunning = running;
  if (vstate.launched || vstate.triggerStep === NEVER) launchPending = false;

  // The reply is held from the frame it lands on (replyDelay frames after this one) until the
  // next reply lands (2 * replyDelay frames after this one). Answer with the mean of the
  // predicted per-frame velocities over those frames, so the height the engine integrates
  // over them is the height the model reaches (a rise or fall is not cut short).
  // A launch the prediction has already published but the committed steps have not reached
  // yet (launchPending) is kept: the prediction then covers at least the next step.
  let vy = vstate.vy;
  const frames = frameTime > 0 ? 2 * replyDelay : 0;
  // the smoothed horizon once the committed model is airborne; a launch from the ground keeps the per-tick
  // delay (a smoothed horizon would publish a launch before the press frame's reply is due)
  const smooth = frameTime > 0 && (vstate.vy !== 0 || vstate.launched || vstate.airLaunchStep !== NEVER);
  const ahead = smooth ? 1.5 * replyDelaySmooth * frameTime : 0;
  if (vstate.launched) jumpTracked = true;
  const airAhead = airPublishedStep > vstate.step ? (airPublishedStep - vstate.step) * FIXED_STEP : 0;
  let probeRan = false;
  if ((smooth ? ahead > 1e-4 : frames > 0) || launchPending || airAhead > 0) {
    probeRan = true;
    copyVState(vstate, vprobe);
    vprobe.launched = false;
    vprobe.airLaunched = false;
    let ground = onGround && !vstate.launched;
    if (smooth || airAhead > 0) {
      vadvance(vprobe, pressed, running, ground, noGravity, Math.max(vpending + Math.max(ahead, launchPending ? FIXED_STEP : 0), airAhead));
      vy = vprobe.vy;
    } else if (frames > 0) {
      let acc = vpending, sum = 0, n = 0;
      for (let f = 1; f <= frames; f++) {
        acc = vadvance(vprobe, pressed, running, ground, noGravity, acc + frameTime);
        if (vprobe.launched) ground = false;
        if (f >= replyDelay) { sum += vprobe.vy; n++; }
      }
      vy = sum / n;
    } else {
      vadvance(vprobe, pressed, running, ground, noGravity, Math.max(vpending, FIXED_STEP));
      vy = vprobe.vy;
    }
    if (vprobe.launched && !vstate.launched) launchPending = true;
    if (vprobe.launched) jumpTracked = true;
    if (vprobe.airLaunchStep !== NEVER && (vprobe.airPressStep > vstate.step || vprobe.airLaunched)) {
      airPublishedStep = Math.max(airPublishedStep, Math.min(vprobe.step, vprobe.airLaunchStep));
      jumpTracked = true;
    }
  }
  // an air jump's hold or launch is in this reply (committed or published)
  const airJump = vstate.airLaunched || (vstate.airLaunchStep !== NEVER && vstate.step < vstate.airLaunchStep)
    || (probeRan && (vprobe.airLaunched || (vprobe.airLaunchStep !== NEVER && vprobe.step < vprobe.airLaunchStep)));
  const hoverReply = (vstate.airLaunchStep !== NEVER && vstate.step < vstate.airLaunchStep && (!probeRan || vprobe.step < vprobe.airLaunchStep))
    || (probeRan && vprobe.airLaunchStep !== NEVER && vprobe.step < vprobe.airLaunchStep && vprobe.step > vstate.step);
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
    // over this frame (the model state is at the end of this frame minus the part of a step not yet run)
    const err = vstate.y + vstate.vy * (vpending - frameTime) - playerPosition.y;
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
  const air = vstate.airLaunchStep !== NEVER ? vstate : probeRan && vprobe.airLaunchStep !== NEVER ? vprobe : null;
  isDoubleJump = air !== null && air.glideOpenStep < air.airPressStep;
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
// Furthest step of an air jump (press .. launch) a reply has published; later predictions reach at least that far.
var airPublishedStep = NEVER;
// The step the committed air jump launches on (NEVER: none), for the horizontal launch push.
export function airLaunchStep(): number { return vstate.airLaunchStep; }
// Landing stun: horizontal movement stops on steps up to stunEndStep() (exclusive) of the committed model. Before the
// committed model reaches the ground, landingStuns() tells whether that landing will stun (the drop from the top to
// the ground under the avatar is over LANDING_STUN_DROP and the stun lasts a step or more); once landed the top is the
// ground and it is false.
export function stunEndStep(): number { return vstate.stunEndStep; }
export function stunned(): boolean { return vstate.step < vstate.stunEndStep; }
export function landingStuns(): boolean { return landingStunSteps() > 0 && vstate.topY - groundY > LANDING_STUN_DROP; }
// Steps a stunning landing holds the avatar for: none when the scene's hardLandingCooldown rounds to 0 or less.
function landingStunSteps(): number { return Math.round(landingStunTime / FIXED_STEP); }
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

// The forward cast reads a face the capsule touches (WALL_CONTACT_DISTANCE). It reaches further: the engine casts
// against the scenes its centre line crosses, so a wall on the next scene's edge is found only once that line reaches
// the edge, which this length does for a capsule meeting the wall up to about 75 degrees off its normal.
const FWD_CAST_LENGTH = 1.3; // m
// Landing probe: a ray down onto the tread just past the face the capsule touches, from above MAX_STEP_HEIGHT so a tread
// at exactly max step height is still hit. It reads the height ahead in the same tick as the forward cast.
const LANDING_CAST_HEIGHT = MAX_STEP_HEIGHT + 0.1;
const LANDING_CAST_INSET = 0.05; // past the riser face, so the probe lands on the tread rather than the riser
// It reaches below the feet, not only down to them: engineStepAhead counts the rise from the ground under the avatar,
// landingHeight + max(0, groundDistance), so while the avatar is above that ground a tread below its feet can still be a
// step. Below the feet it reaches as far as a walkable slope drops over the forward cast's length, about 1.4 m.
const LANDING_CAST_RANGE = LANDING_CAST_HEIGHT + FWD_CAST_LENGTH * Math.tan(GROUNDED_ANGLE * Math.PI / 180);

// cast fwd (nearest hit)
var fwdDistance: number = Infinity; // capsule travel to the hit
var fwdWalkable: boolean = true;
var fwdNormal: Vector3 = Vector3.Zero();
// cast down onto the tread ahead: height above the feet (-Infinity: no hit) and walkability (false for a probe that
// starts inside a collider)
var landingHeight = -Infinity;
var landingWalkable = false;

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
  const forward = PLAYER_COLLIDER_RADIUS + LANDING_CAST_INSET;
  initCast({ x: 0, y: LANDING_CAST_HEIGHT, z: forward }, false, { direction: Vector3.Down(), maxDistance: LANDING_CAST_RANGE, shape: RaycastShape.RS_RAY }, (hits) => {
    landingHeight = -Infinity;
    landingWalkable = false;
    for (const hit of hits.hits) {
      const ny = hit.normalHit?.y ?? 0;
      landingHeight = LANDING_CAST_HEIGHT - hit.length;
      landingWalkable = hit.length > 1e-4 && ny >= GROUNDED_ANGLE_Y_LEN;
    }
  })
}

// A face too steep to walk that the capsule touches along the facing (a wall, a steep slope, a riser higher than a
// step): the horizontal unit normal pointing away from it; null if none.
const wallFace = { x: 0, z: 0 };
export function wallContact(): { x: number, z: number } | null {
  if (fwdDistance > WALL_CONTACT_DISTANCE || fwdWalkable || engineStepAhead()) return null;
  const h = Math.hypot(fwdNormal.x, fwdNormal.z);
  if (h < 1e-6) return null;
  wallFace.x = fwdNormal.x / h;
  wallFace.z = fwdNormal.z / h;
  return wallFace;
}
const WALL_CONTACT_DISTANCE = 0.02; // m of capsule travel to the face

// The face the capsule touches along the facing is a riser the engine steps onto: the landing probe just past the face
// finds walkable ground no higher than MAX_STEP_HEIGHT above the feet.
function engineStepAhead(): boolean {
  const rise = landingHeight + Math.max(0, groundDistance);
  return landingWalkable && rise >= MIN_STEP_HEIGHT && rise <= MAX_STEP_HEIGHT + 1e-3;
}

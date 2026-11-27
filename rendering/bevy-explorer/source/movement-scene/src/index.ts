import { AssetLoad, AvatarAnimationState, AvatarMovement, AvatarMovementInfo, engine, MovementAnimation, Transform } from '@dcl/sdk/ecs'
import { Quaternion, Vector3 } from '@dcl/sdk/math';
import { grounded, initGroundRaycast, recordGroundForNextTick } from './ground';
import { gait, movementAxis, orientation, relativeDegrees, updateHorizontalVelocity } from './horizontal';
import { GlideProp, glideProp, groundSpeed, initStepCasts, isDoubleJump, isGliding, jumpCount, jumpStartHeight, landingStuns, stunned, updateVerticalVelocity } from './vertical';
import { initParameters } from './parameters';
import { initWalkSystem, updateEngineWalk, consumeWalkResult } from './walk';
import { MAX_SPEED, MAX_TICK_TIME, GLIDE_TILT_DAMP_TIME, GLIDE_TILT_FULL_ANGLE, VEC3_DOWN, VEC3_ZERO } from './constants';
import { settings } from './settings';
import { beginTick } from './clock';
import { endEmoteOnInput } from './emote';
import { Animator } from './animator';
import { CHARACTER_ANIMATOR, GLIDE_POSE, LOCOMOTION } from './character-animator';

// Avatar-bus audio clip pools published via MovementAnimation.sounds. Engine
// plays each listed clip once per frame on the avatar's local audio bus — the
// `sounds` field is single-frame fire-and-forget, so we only include it on the
// tick the event actually fires. Timings and variant pools mirror the native
// built-ins in `crates/collectibles/src/emotes.rs` so this scene-driven path
// behaves the same as the velocity-based fallback.
const WALK_STEP_SOUNDS = [
  'assets/sounds/avatar/avatar_footstep_walk01.wav',
  'assets/sounds/avatar/avatar_footstep_walk02.wav',
  'assets/sounds/avatar/avatar_footstep_walk03.wav',
  'assets/sounds/avatar/avatar_footstep_walk04.wav',
  'assets/sounds/avatar/avatar_footstep_walk05.wav',
  'assets/sounds/avatar/avatar_footstep_walk06.wav',
  'assets/sounds/avatar/avatar_footstep_walk07.wav',
  'assets/sounds/avatar/avatar_footstep_walk08.wav',
];
const RUN_STEP_SOUNDS = [
  'assets/sounds/avatar/avatar_footstep_run01.wav',
  'assets/sounds/avatar/avatar_footstep_run02.wav',
  'assets/sounds/avatar/avatar_footstep_run03.wav',
  'assets/sounds/avatar/avatar_footstep_run04.wav',
  'assets/sounds/avatar/avatar_footstep_run05.wav',
  'assets/sounds/avatar/avatar_footstep_run06.wav',
  'assets/sounds/avatar/avatar_footstep_run07.wav',
  'assets/sounds/avatar/avatar_footstep_run08.wav',
];
const JUMP_SOUNDS = [
  'assets/sounds/avatar/avatar_footstep_jump01.wav',
  'assets/sounds/avatar/avatar_footstep_jump02.wav',
  'assets/sounds/avatar/avatar_footstep_jump03.wav',
];
const LAND_SOUNDS = [
  'assets/sounds/avatar/avatar_footstep_land01.wav',
  'assets/sounds/avatar/avatar_footstep_land02.wav',
];

function pickRandom(pool: string[]): string {
  return pool[Math.floor(Math.random() * pool.length)];
}

// Clip-relative playback times (seconds) at which the walk/run loops trigger
// a footstep. Native config: walk.(0.41, 0.91), run.(0.21, 0.54).
const WALK_STEP_TIMES = [0.41, 0.91];
const RUN_STEP_TIMES = [0.21, 0.54];

// Ground speed (m/s) at which each locomotion loop plays at 1.0x: the default gait speeds. Fixed per clip, so a scene
// that changes its gait speeds keeps the clips' stride.
const WALK_CLIP_SPEED = 1.5;
const JOG_CLIP_SPEED = 8;
const RUN_CLIP_SPEED = 10;

// Animation parameters, from the movement model (see updateAnimationParameters).
// The movement blend eases toward its target at MOVE_BLEND_RATE per second, faster when far from it.
const MOVE_BLEND_RATE = 5;
const FALLING_SPEED = -3; // m/s: below it an ungrounded avatar falls
const LONG_FALLING_SPEED = -8; // m/s: below it the fall is a long fall
const EMOTE_MOVE_BLEND = 0.1; // movement blend above which a requested emote waits

// Every GLB clip the scene can drive, pre-loaded by a single AssetLoad entity in main(): otherwise the first use of a
// clip hitches while its GLB streams in (most visible on the first jump/glide of a session).
const PRELOAD_CLIPS: string[] = Object.keys(CHARACTER_ANIMATOR.clips);

// export all the functions required to make the scene work
export * from '@dcl/sdk'

// setup
export function main() {
  // Pre-load every movement clip up front so playback never hitches on first use.
  AssetLoad.create(engine.addEntity(), { assets: PRELOAD_CLIPS });

  initGroundRaycast();
  initStepCasts();
  initWalkSystem();

  engine.addSystem(initFrame, 100000 + 1);
  engine.addSystem(applyMovement, 100000 - 3);
}

export var time = 0;
export var tick = 0;
export var stepTime = 0;
// Engine time this tick covers: the scene can miss engine frames (its reply lands after the
// frame's scene-loop deadline), so rate-based state integrates over everything applied since
// the previous tick (previousStepTime) rather than a single stepTime.
export var tickTime = 0;
var prevStepTime = 0;
export var playerPosition: Vector3 = Vector3.Zero();
export var prevPlayerPosition: Vector3 = Vector3.Zero();
export var playerRotation: Quaternion = Quaternion.Identity();

export var velocity = Vector3.Zero();
// True on frames where the engine moved the player itself (movePlayerTo lerp / teleport,
// or a platform carry) while consuming none of our velocity. Gravity is skipped and the
// grounded state is held, so the lerp neither asserts a fall velocity
// (which would cancel the engine move) nor arms a landing on arrival.
export var engineDriven = false;
export var prevRequestedVelocity = Vector3.Zero();
export var prevActualVelocity = Vector3.Zero();
export var prevExternalVelocity = Vector3.Zero();
// True when the engine applied a velocity other than the one we published last tick.
export var horizontalResync = false;

// What we published last frame. Compared against prevRequestedVelocity in
// initFrame to detect engine takeover (resync to prevActual when they differ).
var lastPublished = Vector3.Zero();

function initFrame() {
  tick += 1;
  Vector3.copyFrom(playerPosition, prevPlayerPosition);
  const playerTransform = Transform.get(engine.PlayerEntity)
  Vector3.copyFrom(playerTransform.position, playerPosition);
  playerRotation = playerTransform.rotation;

  const movementInfo = AvatarMovementInfo.getOrNull(engine.PlayerEntity);
  activeAnimationState = movementInfo?.activeAnimationState;
  if (movementInfo !== null) {
    Vector3.copyFrom(movementInfo.requestedVelocity ?? VEC3_ZERO, prevRequestedVelocity);
    Vector3.copyFrom(movementInfo.actualVelocity ?? VEC3_ZERO, prevActualVelocity);
    Vector3.copyFrom(movementInfo.externalVelocity ?? VEC3_ZERO, prevExternalVelocity);
    stepTime = movementInfo.stepTime;
    prevStepTime = movementInfo.previousStepTime;
  }
  tickTime = Math.min(prevStepTime > 0 ? prevStepTime : stepTime, MAX_TICK_TIME);
  beginTick(tickTime, stepTime);
  time += tickTime;

  initParameters(movementInfo?.activeAvatarLocomotionSettings, movementInfo?.activeInputModifier);
  updateEngineWalk(movementInfo?.walkTarget, movementInfo?.walkThreshold);

  // If we are not in control (engine consumed a different velocity than we
  // published), resync velocity to prevActual.
  // A missing requestedVelocity also means the engine applied zero. If the scene published zero, that is not a
  // takeover, so the horizontal model resyncs only on a real mismatch.
  horizontalResync = Vector3.distance(lastPublished, prevRequestedVelocity) > 0.1;
  const notInControl = horizontalResync || movementInfo?.requestedVelocity === undefined;
  if (notInControl) {
    Vector3.copyFrom(prevActualVelocity, velocity);
    Vector3.copyFrom(prevActualVelocity, prevRequestedVelocity);
  }
  engineDriven = movementInfo?.requestedVelocity === undefined
    && Vector3.distanceSquared(prevPlayerPosition, playerPosition) > 1e-8;
}

// Tracks the last jumpStartHeight we observed so we can detect a new jump
// (undefined -> defined): the launch the jump sound plays on.
var prevJumpStartHeight: number | undefined = undefined;
// Max Y reached while airborne; reset on landing. Used to gate the landing
// sound to drops of >0.5m so stepping off a small curb stays silent.
var maxUngroundedY = -Infinity;
var wasJumpingOrFalling = false;
// Mirror of the engine's currently-active scene animation state. Read in
// initFrame; consulted in selectAnimation to decide when to stop the landing.
var activeAnimationState: AvatarAnimationState | undefined = undefined;
// Last-observed playback phase of the currently-active scene animation, used to
// detect when the clip wrapped past a footstep trigger time since the previous
// frame. Keyed per src so switching clip resets tracking.
var prevSoundTrackedSrc: string | undefined = undefined;
var prevSoundTrackedTime: number = 0;

// --- Animation parameters (updateAnimationParameters) ---
// Movement blend: 0 idle, 1 walk, 2 jog, 3 run, each gait scaled by the horizontal speed over its clip's speed.
var moveBlend = 0;
// Jumps since the ground the animation has seen; a rise starts a jump state.
var animJumps = 0;
// Clip variation of the air jump, drawn when one starts: 0 the plain clip, 1 the Right one.
var jumpVariation = 0;
// The glider prop last tick: the glide starts when the prop starts opening.
var prevGlideProp = GlideProp.CLOSED;

// --- Animation state ---
// The reference client's base layer (character-animator.ts), advanced once per tick from the parameters above.
const animator = new Animator(CHARACTER_ANIMATOR);

// --- Glide turn rate ---
// Smoothed avatar turn rate (deg/s), used to pick the left/right glide lean.
// Computed each frame in applyMovement from the change in facing.
var glidePrevOrientation = 0;
var glideTurnRate = 0;
// Smoothed procedural glide bank (roll, degrees), eased toward a target derived from
// glideTurnRate while gliding and back to 0 otherwise. Published as AvatarMovement.tiltRoll
// so the engine banks the whole avatar.
var glideTilt = 0;

// Glide pose clip published last frame (forward/idle), to detect a swap mid-glide.
var prevGlideSrc: string | undefined = undefined;

function moveTowards(from: number, to: number, maxDelta: number): number {
  return Math.abs(to - from) <= maxDelta ? to : from + Math.sign(to - from) * maxDelta;
}

// The movement blend over this tick, on the ground and in the air alike. With no movement input its target is 0
// whatever the speed.
function updateAnimationParameters(horizontalSpeed: number) {
  const kind = gait === 'walk' ? 1 : gait === 'jog' ? 2 : gait === 'run' ? 3 : 0;
  const clipSpeed = [WALK_CLIP_SPEED, JOG_CLIP_SPEED, RUN_CLIP_SPEED][kind - 1];
  const target = kind === 0 ? 0 : Math.min(1, horizontalSpeed / clipSpeed) * kind;
  moveBlend = moveTowards(moveBlend, target, tickTime * MOVE_BLEND_RATE * Math.max(Math.abs(moveBlend - target), 1));
  if (moveBlend < 0.01) moveBlend = 0;
}

// The glide loop's pose. Left/right banking is procedural via tiltRoll (see applyMovement / writeMovement): the engine
// rolls the whole avatar, so the clip only distinguishes forward-lean (moving fast, pitched) from idle (slow /
// lean-back).
function glidePoseAnimation(horizontalSpeed: number, entered: boolean): MovementAnimation {
  const src = horizontalSpeed > settings.glideForwardSpeed ? GLIDE_POSE.forward : GLIDE_POSE.idle;
  // On a forward<->idle swap mid-glide, hand the engine the current playback time so the new clip, glider included,
  // continues in phase instead of snapping to 0. `activeAnimationState.playbackTime` is the engine's value from its
  // last report, so add this frame's `stepTime`. Sent only on the swap frame: the clip has no prop to load, so the
  // engine applies it in that frame.
  const poseSwapped = !entered && prevGlideSrc !== src;
  prevGlideSrc = src;
  return {
    src,
    speed: 1,
    loop: true,
    idle: true,
    transitionSeconds: settings.gliderFade,
    playbackTime: poseSwapped && activeAnimationState !== undefined ? activeAnimationState.playbackTime + stepTime : undefined,
    sounds: [],
  };
}

// Detects whether the clip's playback time crossed any of the given trigger
// timestamps (in seconds) while playing forward between `prev` and `cur`.
// Backward motion (a playback time that goes back without a loop) is
// ignored so a scrubbing clip can't fire a storm of sounds. A genuine loop
// wrap is distinguished from backward play by requiring the apparent reverse
// jump to span more than half the clip.
function stepTriggered(prev: number, cur: number, duration: number, triggers: number[]): boolean {
  if (cur === prev) return false;
  const isLoopWrap = cur < prev && duration > 0 && prev - cur > duration * 0.5;
  if (cur < prev && !isLoopWrap) return false;
  for (const t of triggers) {
    if (isLoopWrap) {
      if (t > prev || t <= cur) return true;
    } else if (t > prev && t <= cur) {
      return true;
    }
  }
  return false;
}

function selectAnimation(): MovementAnimation {
  const jumpingOrFalling = jumpStartHeight !== undefined || !grounded;
  const newJump = jumpStartHeight !== undefined && prevJumpStartHeight === undefined;
  prevJumpStartHeight = jumpStartHeight;

  if (jumpingOrFalling) {
    maxUngroundedY = Math.max(maxUngroundedY, playerPosition.y);
  }
  // Ungrounded -> grounded transition: the frame the landing sound may fire.
  const justLanded = wasJumpingOrFalling && !jumpingOrFalling;
  wasJumpingOrFalling = jumpingOrFalling;
  const drop = maxUngroundedY - playerPosition.y;
  if (justLanded) maxUngroundedY = -Infinity;

  const horizontalSpeed = Math.sqrt(velocity.x * velocity.x + velocity.z * velocity.z);

  updateAnimationParameters(horizontalSpeed);
  // A jump starts when the model counts one more since the ground (an air jump from its press; a fall past the grace
  // window counts as one), or launches one it has not counted yet.
  const jumps = Math.max(jumpCount(), isDoubleJump ? 2 : jumpStartHeight !== undefined ? 1 : 0);
  const jumpTrigger = jumps > animJumps || (newJump && !isDoubleJump);
  animJumps = jumps;
  // step(0.5, random) as the reference client draws it, here only for the air jump that reads it
  if (jumpTrigger && jumps > 1) jumpVariation = Math.random() < 0.5 ? 0 : 1;
  const startGliding = glideProp === GlideProp.OPENING && prevGlideProp !== GlideProp.OPENING;
  prevGlideProp = glideProp;

  const pose = animator.update({
    MovementBlend: moveBlend,
    Jump: jumpTrigger,
    JumpCount: jumps,
    JumpVariation: jumpVariation,
    IsGrounded: !jumpingOrFalling,
    IsFalling: jumpingOrFalling && velocity.y < FALLING_SPEED,
    IsLongFall: jumpingOrFalling && velocity.y < LONG_FALLING_SPEED,
    // the model may count the touchdown a frame after this one: landingStuns() tells it before
    IsStunned: stunned() || landingStuns(),
    StartGliding: startGliding,
    IsGliding: glideProp === GlideProp.OPENING || glideProp === GlideProp.GLIDING,
  }, tickTime, activeAnimationState);
  const animation: MovementAnimation = pose.scene === 'locomotion' ? locomotionAnimation(horizontalSpeed)
    : pose.scene === 'glidePose' ? glidePoseAnimation(horizontalSpeed, pose.entered)
    : { src: pose.clip!, speed: pose.speed, loop: pose.loop, idle: true, transitionSeconds: pose.blend, sounds: [] };
  if (pose.entered) {
    animation.transitionSeconds = pose.blend;
    if (pose.seek !== undefined) animation.playbackTime = pose.seek * CHARACTER_ANIMATOR.clips[animation.src].length;
  }
  // A requested emote waits, as in the reference client, until the avatar stands: grounded, out of the jump states
  // (the landing until its blend into locomotion is over, the hard landing's start until its loop has blended in), and
  // with the movement blend at most EMOTE_MOVE_BLEND. A published animation that is not idle holds the request back.
  // Scene: a jump state being blended into holds it too (the reference client reads only the state blended from).
  const jumpState = animator.tag === 'Jumping' || CHARACTER_ANIMATOR.states[animator.state].tag === 'Jumping';
  if (jumpState || jumpingOrFalling || moveBlend > EMOTE_MOVE_BLEND) animation.idle = false;
  const sounds = [
    ...(newJump ? [pickRandom(JUMP_SOUNDS)] : []),
    ...(justLanded && drop > 0.5 ? [pickRandom(LAND_SOUNDS)] : []),
  ];
  if (sounds.length > 0) animation.sounds = [...(animation.sounds ?? []), ...sounds];
  return animation;
}

function locomotionAnimation(horizontalSpeed: number): MovementAnimation {
  // Locomotion clips play at the speed's magnitude, not its projection on the facing, so a clip never plays backwards
  // while the avatar turns. No floor: below moveGate the avatar idles, and above it a slower clip keeps the feet planted.

  // Locomotion tiers only while there's actual movement input. With no keys
  // held, the residual decelerating velocity (jog -> 0 passes through walk
  // speeds for a few frames) would otherwise flash the walk clip — the
  // "residual walking" shuffle — instead of going straight to idle.
  const hasMoveInput = Vector3.lengthSquared(movementAxis) > 0;
  if (horizontalSpeed > settings.moveGate && hasMoveInput) {
    // Tier 1: walk
    if (horizontalSpeed <= settings.walkRunThreshold) {
      return {
        src: LOCOMOTION.walk,
        speed: horizontalSpeed / WALK_CLIP_SPEED,
        loop: true,
        idle: false,
        transitionSeconds: settings.transWalk,
        sounds: footstepsFor(LOCOMOTION.walk, WALK_STEP_TIMES, WALK_STEP_SOUNDS),
      };
    }
    // Tier 2: jog (the default movement speed)
    if (horizontalSpeed <= settings.sprintThreshold) {
      return {
        src: LOCOMOTION.jog,
        speed: horizontalSpeed / JOG_CLIP_SPEED,
        loop: true,
        idle: false,
        transitionSeconds: settings.transRun,
        sounds: footstepsFor(LOCOMOTION.jog, RUN_STEP_TIMES, RUN_STEP_SOUNDS),
      };
    }
    // Tier 3: run (sprint)
    return {
      src: LOCOMOTION.run,
      speed: horizontalSpeed / RUN_CLIP_SPEED,
      loop: true,
      idle: false,
      transitionSeconds: settings.transRun,
      sounds: footstepsFor(LOCOMOTION.run, RUN_STEP_TIMES, RUN_STEP_SOUNDS),
    };
  }

  // A stop settles straight into idle, as in the reference client, which never plays a stop clip.
  return {
    src: LOCOMOTION.idle,
    speed: 1.0,
    loop: true,
    idle: true,
    transitionSeconds: settings.transIdle,
    sounds: [],
  };
}

// Detects a footstep trigger crossing for the currently-playing clip. Returns a
// single-frame sounds list (or empty) so remote clients see each step as an
// independent transition.
function footstepsFor(src: string, triggers: number[], pool: string[]): string[] {
  const s = activeAnimationState;
  if (s === undefined || s.src !== src) {
    prevSoundTrackedSrc = src;
    prevSoundTrackedTime = 0;
    return [];
  }
  const cur = s.playbackTime;
  const prev = prevSoundTrackedSrc === src ? prevSoundTrackedTime : cur;
  prevSoundTrackedSrc = src;
  prevSoundTrackedTime = cur;
  return stepTriggered(prev, cur, s.duration, triggers) ? [pickRandom(pool)] : [];
}

function writeMovement() {
  const animation = selectAnimation();
  endEmoteOnInput(animation);
  AvatarMovement.createOrReplace(engine.PlayerEntity, {
    velocity,
    orientation: -orientation,
    // Render-only glide bank. Negated to match `orientation`'s sign convention; if the
    // avatar banks the wrong way relative to the turn, flip this sign (verify in-app).
    tiltRoll: -glideTilt,
    groundDirection: VEC3_DOWN,
    walkSuccess: consumeWalkResult(),
    animation,
  })

  Vector3.copyFrom(velocity, lastPublished);
}


function applyMovement() {
  updateVerticalVelocity();
  updateHorizontalVelocity();
  const ground = groundSpeed(velocity.x, velocity.z);
  velocity.y += ground.follow;

  // Track smoothed turn rate (deg/s) from the change in facing, for glide lean.
  const turnDelta = relativeDegrees(0, orientation - glidePrevOrientation);
  glidePrevOrientation = orientation;
  const turnAlpha = 1 - Math.exp(-tickTime / 0.2);
  glideTurnRate += (turnDelta / Math.max(tickTime, 1e-3) - glideTurnRate) * turnAlpha;

  // Procedural glide bank: roll proportional to how hard we're turning, clamped to
  // GLIDE_TILT_FULL_ANGLE and eased over GLIDE_TILT_DAMP_TIME. glideLeanRate (deg/s) is the
  // turn rate at which the bank reaches full angle. Target is 0 when not gliding so we ease
  // back upright.
  const tiltTarget = isGliding
    ? Math.max(
        -GLIDE_TILT_FULL_ANGLE,
        Math.min(GLIDE_TILT_FULL_ANGLE, (glideTurnRate / settings.glideLeanRate) * GLIDE_TILT_FULL_ANGLE),
      )
    : 0;
  const tiltAlpha = 1 - Math.exp(-tickTime / GLIDE_TILT_DAMP_TIME);
  glideTilt += (tiltTarget - glideTilt) * tiltAlpha;

  // Scene pushes are already in the velocity of the horizontal and vertical models.

  // The speed cap applies to movement, not to the ground snap's speed down.
  const speed = Vector3.length(velocity);
  if (speed > MAX_SPEED) {
    Vector3.scaleToRef(velocity, MAX_SPEED / speed, velocity);
  }
  velocity.y -= ground.snap;

  if (Vector3.length(velocity) < 0.01 || Number.isNaN(Vector3.length(velocity))) {
    Vector3.copyFrom(VEC3_ZERO, velocity);
  }
  recordGroundForNextTick();
  writeMovement();
}

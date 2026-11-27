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

// New phased / per-locomotion animation set lives in assets/animations/.
// Locomotion is a real 3-tier system: walk -> jog (default move) -> run (sprint).
// The engine reads jumps, the glide and the stun for its jump timing and network state from these
// exact file names (crates/common/src/movement_clip.rs), so a renamed clip is renamed there too.
const ANIM = 'assets/animations/';
const CLIP_IDLE = ANIM + 'Idle.glb';
const CLIP_WALK = ANIM + 'Walk.glb';
const CLIP_JOG = ANIM + 'Jog.glb';
const CLIP_RUN = ANIM + 'Run.glb';

// Jump clip sets, one per locomotion tier ('idle' is the standing set), picked by the movement blend when a state of
// the jump chain starts (see AnimState): Start, Mid, the Fall loop and the End of the landing.
type JumpVariation = 'idle' | 'jog' | 'run';
const JUMP_SETS: Record<JumpVariation, { start: string; mid: string; fall: string; end: string }> = {
  idle: { start: ANIM + 'Jump_Start.glb', mid: ANIM + 'Jump_Mid.glb', fall: ANIM + 'Jump_Fall_Loop.glb', end: ANIM + 'Idle_Jump_End.glb' },
  jog: { start: ANIM + 'Jog_Jump_Start.glb', mid: ANIM + 'Jog_Jump_Mid.glb', fall: ANIM + 'Jog_Jump_Fall_Loop.glb', end: ANIM + 'Jog_Jump_End.glb' },
  run: { start: ANIM + 'Run_Jump_Start.glb', mid: ANIM + 'Run_Jump_Mid.glb', fall: ANIM + 'Run_Jump_Fall_Loop.glb', end: ANIM + 'Run_Jump_End.glb' },
};
const CLIP_LONG_FALL = ANIM + 'Long_Fall_Loop.glb';
// The tier's End (as in JUMP_SETS) with the glider closing embedded, for a landing while the glider is open: the
// glider folds and shrinks away over the first 0.267 s, or vanishes with the clip when locomotion takes over earlier.
const GLIDE_LANDING: Record<JumpVariation, string> = {
  idle: ANIM + 'Gliding_Idle_Jump_EndRig.glb',
  jog: ANIM + 'Gliding_Jog_Jump_EndRig.glb',
  run: ANIM + 'Gliding_Run_Jump_EndRig.glb',
};
// Hard landing on a stunning touchdown out of the long fall loop. The engine's network state reads a stun from these
// clips played at speed 1 or slower: Enter and Loop, not the faster Exit.
const HARD_LANDING = {
  enter: ANIM + 'Hard_Landing_Enter.glb',
  loop: ANIM + 'Hard_Landing_Loop.glb',
  exit: ANIM + 'Hard_Landing_Exit.glb',
};
const HARD_LANDING_ENTER_BLEND = 0.0585; // s, from the long fall
const HARD_LANDING_LOOP_BLEND = 0.15; // s, from the end of Enter. The stun's end waits for it.
const HARD_LANDING_EXIT_BLEND = 0.05; // s, from Loop
const HARD_LANDING_EXIT_SPEED = 1.2;
const HARD_LANDING_EXIT_END = 0.2; // fraction of Exit
const HARD_LANDING_AFTER_BLEND = 0.14; // s, from Exit to the next clip

// Double-jump avatar clips, per tier of the movement blend at the air jump (tierFor).
const DOUBLE_JUMP: Record<JumpVariation, { plain: string; right: string }> = {
  idle: { plain: ANIM + 'DoubleJump_Base.glb', right: ANIM + 'DoubleJump_Base_Right.glb' },
  jog: { plain: ANIM + 'DoubleJump_Jog.glb', right: ANIM + 'DoubleJump_Jog_Right.glb' },
  run: { plain: ANIM + 'DoubleJump_Run.glb', right: ANIM + 'DoubleJump_Run_Right.glb' },
};
const DOUBLE_JUMP_SPEED = 0.7;
const DOUBLE_JUMP_BLEND = 0.05; // s
const DOUBLE_JUMP_EXIT = 0.778; // fraction of the clip at which the fall loop takes over
// Directional glide avatar poses.
// The glider model is embedded in these clips, so it renders and networks for free via the scene-driven movement animation — no
// separate prop rig. Left/right banking is procedural (tiltRoll), not per-clip.
const GLIDE_AVATAR = {
  forward: ANIM + 'Gliding_AvatarForwardRig.glb', // pitched forward (moving fast)
  idle: ANIM + 'Gliding_AvatarIdleRig.glb',       // upright / lean-back (slow)
  start: ANIM + 'Gliding_AvatarStartRig.glb',     // deploy: glider opens (Glider_Start, played whole)
  end: ANIM + 'Gliding_AvatarEndRig.glb',         // stow: glider closes (Glider_Close)
};

// Lengths (s) of the clips whose timing the animation states read on the frame they start, before the engine reports
// them (all 60 fps clips).
const CLIP_LENGTH: Record<string, number> = {
  [CLIP_IDLE]: 120 / 60, [CLIP_WALK]: 60 / 60, [CLIP_JOG]: 40 / 60, [CLIP_RUN]: 32 / 60,
  [JUMP_SETS.idle.start]: 12 / 60, [JUMP_SETS.jog.start]: 14 / 60, [JUMP_SETS.run.start]: 14 / 60,
  [JUMP_SETS.idle.mid]: 26 / 60, [JUMP_SETS.jog.mid]: 24 / 60, [JUMP_SETS.run.mid]: 24 / 60,
  [JUMP_SETS.idle.fall]: 40 / 60, [JUMP_SETS.jog.fall]: 40 / 60, [JUMP_SETS.run.fall]: 40 / 60,
  [JUMP_SETS.idle.end]: 16 / 60, [JUMP_SETS.jog.end]: 24 / 60, [JUMP_SETS.run.end]: 22 / 60,
  [CLIP_LONG_FALL]: 48 / 60,
  [GLIDE_AVATAR.start]: 68 / 60, [GLIDE_AVATAR.end]: 16 / 60,
};
// The locomotion clips at movement blend 0 (idle), 1 (walk), 2 (jog) and 3 (run).
const MOVE_CLIPS = [CLIP_IDLE, CLIP_WALK, CLIP_JOG, CLIP_RUN];

// Animation parameters, from the movement model (see updateAnimationParameters).
// The movement blend eases toward its target at MOVE_BLEND_RATE per second, faster when far from it.
const MOVE_BLEND_RATE = 5;
const FALLING_SPEED = -3; // m/s: below it an ungrounded avatar falls
const LONG_FALLING_SPEED = -8; // m/s: below it the fall is a long fall
// Movement blend from which a jump-chain state picks the jog clip (the run clip from 2.5), and the fall loop's.
const JOG_TIER_FROM = 1.5;
const JOG_FALL_FROM = 1.85;

// Animation state timing. Positions (*_FROM, *_EXIT) are fractions of the named clip; blends are seconds, or
// fractions (*_BLEND_OF) of the clip the state leaves.
const JUMP_START_FROM = 0.3291; // Start begins here
const JUMP_START_BLEND = 0.1311;
const JUMP_START_EXIT = 0.8145; // of Start: Up
const JUMP_MID_BLEND = 0.0645; // Up to Mid
const JUMP_MID_EXIT = 0.5; // of Mid: Down, rising or not
const JUMP_FALL_BLEND_OF = 0.2165; // of Mid
const FALL_BLEND_OF = 0.2904; // of the locomotion cycle (moveCycleLength), Movement to Down
const FALL_FROM = 0.0071; // of the fall loop, from Movement
const LONG_FALL_BLEND_OF = 0.3367; // of the fall loop
const LONG_FALL_END_BLEND = 0.25; // long fall back to the fall loop
const LAND_FROM_JUMP_BLEND_OF = 0.0218; // of Start or Mid
const LAND_FROM_JUMP_FROM = 0.0172; // of End
const LAND_FROM_FALL_BLEND_OF = 0.0624; // of the fall or long fall loop
const LAND_FROM_AIR_JUMP_BLEND = 0.1; // from the air jump or the glider
const LAND_MOVE_BLEND = 0.5; // movement blend above which a landing hands over to locomotion
const LAND_MOVE_BLEND_OF = 0.358; // of End
const LAND_MOVE_FROM = 0.0259; // of the locomotion clip
const LAND_EXIT = 0.4778; // of End: locomotion
const LAND_EXIT_BLEND_OF = 0.3982; // of End
const GLIDE_START_BLEND = 0.05; // into the glide's Start, from any state
const GLIDE_LOOP_BLEND = 0.05; // Start to the glide loop, at the end of Start
const GLIDE_END_BLEND = 0.1; // Start or the loop to the glide's End, when the glide ends in the air
const GLIDE_END_FROM = 0.05; // of the glide's End: it begins here
const GLIDE_FALL_BLEND = 0.1; // the glide's End to Down, at the end of End

// Every GLB clip the scene can drive, collected from the constants above so the
// list never drifts. Fed to a single AssetLoad entity in main() to pre-load all
// of them at startup — otherwise the first use of a clip hitches while its GLB
// streams in (most visible on the first jump/glide of a session).
const PRELOAD_CLIPS: string[] = [
  CLIP_IDLE, CLIP_WALK, CLIP_JOG, CLIP_RUN,
  ...Object.values(JUMP_SETS).flatMap((s) => [s.start, s.mid, s.fall, s.end]),
  CLIP_LONG_FALL, ...Object.values(GLIDE_LANDING),
  ...Object.values(HARD_LANDING),
  ...Object.values(DOUBLE_JUMP).flatMap((d) => [d.plain, d.right]),
  ...Object.values(GLIDE_AVATAR),
];

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

// --- Animation state ---
// One state per tick, advanced in stateAnimation from the movement model and the parameters above; Movement is the
// locomotion clips. A jump starts Jump, and the chain then runs on time: Start from JUMP_START_FROM, Up for one tick
// (it keeps Start), Mid, and Down from JUMP_MID_EXIT of Mid whether the avatar still rises or not. A fall enters Down
// only below FALLING_SPEED, so a short drop keeps the locomotion clips; a walk off a ledge that the movement model
// counts as a jump (past its grace window) starts Jump in the air. A state with a clip per tier picks it on starting
// and keeps it.
type AnimState = 'Movement' | 'Jump' | 'Up' | 'UpToDown' | 'Down' | 'LongFall' | 'Grounded' | 'HardLanding'
  | 'DoubleJump' | 'Gliding';
var animState: AnimState = 'Movement';
// Scene time and tick the state started.
var animFrom = 0;
var animTick = 0;
// Blend into the state's clip (undefined: the clip's own), and where in it the state starts (fraction of the clip;
// undefined: no seek).
var animBlend: number | undefined = undefined;
var animOffset: number | undefined = undefined;
var animTier: JumpVariation = 'idle';
// Grounded plays the tier's glide landing (GLIDE_LANDING) instead of its End: the landing of a glide, not of a stow.
var glideLanding = false;

// --- Double jump ---
// Clip of the current air jump, picked when DoubleJump starts.
var doubleJumpClip = '';

// --- Hard landing ---
type HardLandingPhase = 'none' | 'enter' | 'loop' | 'exit';
var hardLanding: HardLandingPhase = 'none';
var prevHardLanding: HardLandingPhase = 'none';
// Scene time Loop started.
var hardLandingLoopFrom = 0;

// --- Glide turn rate ---
// Smoothed avatar turn rate (deg/s), used to pick the left/right glide lean.
// Computed each frame in applyMovement from the change in facing.
var glidePrevOrientation = 0;
var glideTurnRate = 0;
// Smoothed procedural glide bank (roll, degrees), eased toward a target derived from
// glideTurnRate while gliding and back to 0 otherwise. Published as AvatarMovement.tiltRoll
// so the engine banks the whole avatar.
var glideTilt = 0;

// Glide animation phase, from the vertical model's glider prop (glideProp), on time: 'deploy' plays the
// Start clip (glider opens) whole from the tick the prop starts opening, 'glide' is the steady
// forward/idle loop after it while the prop stays open, 'stow' plays the End clip (glider closes)
// from GLIDE_END_FROM over the rest of its length when the glide ends mid-air, then the fall goes
// on. A glide that ends by landing skips 'stow' (the glide landing closes the glider).
type GlidePhase = 'none' | 'deploy' | 'glide' | 'stow';
var glidePhase: GlidePhase = 'none';
// Scene time and tick the current 'deploy' or 'stow' started.
var glideFrom = 0;
var glideTick = 0;
// Set when a glide ends by touching down, so its landing plays the glide landing (GLIDE_LANDING).
// Consumed in stateAnimation.
var landingFromGlide = false;
// Glide pose clip published last frame (forward/idle), to detect a swap mid-glide.
var prevGlideSrc: string | undefined = undefined;

// True once the engine reports a non-looped clip has played to its end.
function clipFinished(src: string): boolean {
  const s = activeAnimationState;
  return s !== undefined && s.src === src && !s.loop
    && (s.loopCount >= 1 || s.playbackTime >= s.duration - 1e-3);
}

// True once the engine reports the clip has played the given fraction of its length.
function clipReached(src: string, fraction: number): boolean {
  const s = activeAnimationState;
  return s !== undefined && s.src === src && (s.loopCount >= 1 || s.playbackTime >= fraction * s.duration);
}

// The jump-chain clip tier nearest the movement blend: the sets sit at blends 1 (idle), 2 (jog) and 3 (run), the fall
// loops' idle clip at 1.7 (jogFrom JOG_FALL_FROM).
function tierFor(blend: number, jogFrom = JOG_TIER_FROM): JumpVariation {
  return blend < jogFrom ? 'idle' : blend < 2.5 ? 'jog' : 'run';
}

// Length (s) of one locomotion cycle at the movement blend: the locomotion clips share their phase, so a cycle lasts
// the lengths of the two nearest clips weighted by the blend between them.
function moveCycleLength(blend: number): number {
  const i = Math.min(Math.floor(blend), MOVE_CLIPS.length - 2);
  const f = Math.min(1, blend - i);
  return (1 - f) * CLIP_LENGTH[MOVE_CLIPS[i]] + f * CLIP_LENGTH[MOVE_CLIPS[i + 1]];
}

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

// Starts a state on this tick: its clip blends in over `blend` (s) from `offset` of its length. Up keeps Jump's clip.
function enterState(state: AnimState, blend?: number, offset?: number) {
  if (state !== 'Up') animTier = tierFor(moveBlend, state === 'Down' ? JOG_FALL_FROM : JOG_TIER_FROM);
  animState = state;
  animFrom = time;
  animTick = tick;
  animBlend = blend;
  animOffset = offset;
  glideLanding = false;
  if (state !== 'HardLanding') hardLanding = 'none';
}

// Touchdown from an airborne state: a stunning landing from the long fall plays the hard landing, any other the tier's
// End.
function land(blend: number, offset = 0) {
  // the model may count the touchdown a frame after this one: landingStuns() tells it before
  const stun = stunned() || landingStuns();
  if (stun && animState === 'LongFall') {
    enterState('HardLanding', HARD_LANDING_ENTER_BLEND);
    hardLanding = 'enter';
    prevHardLanding = 'none';
  } else {
    enterState('Grounded', blend, offset);
  }
}

// Advances the animation state for this tick and returns its clip; null in Movement (the locomotion clips). At most
// one transition per tick: a state started this tick is not advanced again.
function stateAnimation(
  jumpTrigger: boolean, jumps: number, airborne: boolean, horizontalSpeed: number,
): MovementAnimation | null {
  const glideOpen = glideProp === GlideProp.OPENING || glideProp === GlideProp.GLIDING;
  // the End clip runs its length, then the fall takes over (below)
  if (glidePhase === 'stow' && time - glideFrom + 1e-6 >= CLIP_LENGTH[GLIDE_AVATAR.end]) glidePhase = 'none';
  if (airborne && (glideOpen || glidePhase === 'stow')) {
    if (animState !== 'Gliding') enterState('Gliding');
    if (glideOpen) {
      // Deploy: the Start clip (glider opens), whole, from the tick the prop starts opening, then the steady loop.
      if (glideProp === GlideProp.OPENING && glidePhase !== 'deploy') {
        glidePhase = 'deploy';
        glideFrom = time;
      }
      if (glidePhase === 'deploy' && time - glideFrom + 1e-6 < CLIP_LENGTH[GLIDE_AVATAR.start]) {
        return {
          src: GLIDE_AVATAR.start,
          speed: 1,
          loop: false,
          idle: false,
          transitionSeconds: GLIDE_START_BLEND,
          sounds: [],
        };
      }
      const loopStarts = glidePhase !== 'glide';
      if (loopStarts) {
        glidePhase = 'glide';
        prevGlideSrc = undefined; // first steady frame isn't a pose swap
      }
      // Steady glide. Left/right banking is procedural via tiltRoll (see applyMovement /
      // writeMovement) — the engine rolls the whole avatar — so the clip only
      // distinguishes forward-lean (moving fast, pitched) from idle (slow / lean-back).
      const src = horizontalSpeed > settings.glideForwardSpeed
        ? GLIDE_AVATAR.forward
        : GLIDE_AVATAR.idle;
      // On a forward<->idle swap mid-glide, hand the engine the current playback time so
      // the new clip, glider included, continues in phase instead of snapping to 0.
      // `activeAnimationState.playbackTime` is the engine's value from its last report, so
      // add this frame's `stepTime`. Sent only on the swap frame: the clip has no prop to
      // load, so the engine applies it in that frame.
      const poseSwapped = prevGlideSrc !== undefined && prevGlideSrc !== src;
      prevGlideSrc = src;
      return {
        src,
        speed: 1,
        loop: true,
        idle: false,
        transitionSeconds: loopStarts ? GLIDE_LOOP_BLEND : settings.gliderFade,
        playbackTime:
          poseSwapped && activeAnimationState !== undefined
            ? activeAnimationState.playbackTime + stepTime
            : undefined,
        sounds: [],
      };
    }

    // Stow: glide ended mid-air -> play the End clip (glider closes) from GLIDE_END_FROM, then the fall resumes.
    return {
      src: GLIDE_AVATAR.end,
      speed: 1,
      loop: false,
      idle: false,
      transitionSeconds: GLIDE_END_BLEND,
      playbackTime: glideTick === tick ? GLIDE_END_FROM * CLIP_LENGTH[GLIDE_AVATAR.end] : undefined,
      sounds: [],
    };
  }

  if (animState === 'Gliding') {
    // the glide is over: in the air the fall goes on; on the ground it lands in Grounded (a hard landing comes only
    // from the long fall), a glide (not a stow) on the glide landing, which closes the glider
    if (airborne) enterState('Down', GLIDE_FALL_BLEND, 0);
    else {
      land(LAND_FROM_AIR_JUMP_BLEND);
      glideLanding = landingFromGlide;
    }
    landingFromGlide = false;
  } else if (jumpTrigger && jumps > 1) {
    enterState('DoubleJump', DOUBLE_JUMP_BLEND, 0);
    const clips = DOUBLE_JUMP[animTier];
    doubleJumpClip = Math.random() < 0.5 ? clips.plain : clips.right;
  } else if (jumpTrigger && animState !== 'Jump') {
    enterState('Jump', JUMP_START_BLEND, JUMP_START_FROM);
  } else if (animTick !== tick) {
    // time in the state; the margin lets a tick that reaches an exit exactly (sums of frame times) take it
    const t = time - animFrom + 1e-6;
    const set = JUMP_SETS[animTier];
    switch (animState) {
      case 'Movement':
        if (airborne && velocity.y < FALLING_SPEED) {
          enterState('Down', FALL_BLEND_OF * moveCycleLength(moveBlend), FALL_FROM);
        }
        break;
      case 'Jump':
        if (JUMP_START_FROM + t / CLIP_LENGTH[set.start] >= JUMP_START_EXIT) enterState('Up', animBlend);
        else if (!airborne) land(LAND_FROM_JUMP_BLEND_OF * CLIP_LENGTH[set.start], LAND_FROM_JUMP_FROM);
        break;
      case 'Up':
        enterState('UpToDown', JUMP_MID_BLEND, 0);
        break;
      case 'UpToDown':
        if (!airborne) land(LAND_FROM_JUMP_BLEND_OF * CLIP_LENGTH[set.mid], LAND_FROM_JUMP_FROM);
        else if (t / CLIP_LENGTH[set.mid] >= JUMP_MID_EXIT) {
          enterState('Down', JUMP_FALL_BLEND_OF * CLIP_LENGTH[set.mid], 0);
        }
        break;
      case 'Down':
        if (!airborne) land(LAND_FROM_FALL_BLEND_OF * CLIP_LENGTH[set.fall]);
        else if (velocity.y < LONG_FALLING_SPEED) {
          enterState('LongFall', LONG_FALL_BLEND_OF * CLIP_LENGTH[set.fall], 0);
        }
        break;
      case 'LongFall':
        if (!airborne) land(LAND_FROM_FALL_BLEND_OF * CLIP_LENGTH[CLIP_LONG_FALL]);
        else if (velocity.y >= LONG_FALLING_SPEED) enterState('Down', LONG_FALL_END_BLEND, 0);
        break;
      case 'DoubleJump':
        if (!airborne) land(LAND_FROM_AIR_JUMP_BLEND);
        else if (clipReached(doubleJumpClip, DOUBLE_JUMP_EXIT)) enterState('Down', settings.transAir, 0);
        break;
      case 'Grounded': {
        // locomotion once the entry blend is over and the avatar moves, or from LAND_EXIT of End
        const end = CLIP_LENGTH[set.end];
        if (t >= (animBlend ?? 0) && moveBlend > LAND_MOVE_BLEND) {
          enterState('Movement', LAND_MOVE_BLEND_OF * end, LAND_MOVE_FROM);
        } else if ((animOffset ?? 0) + t / end >= LAND_EXIT) enterState('Movement', LAND_EXIT_BLEND_OF * end);
        break;
      }
    }
  }

  const set = JUMP_SETS[animTier];
  const clip = (src: string, loop: boolean, idle = false): MovementAnimation => ({
    src,
    speed: 1,
    loop,
    idle,
    transitionSeconds: animBlend ?? settings.transAir,
    playbackTime: animTick === tick && animOffset !== undefined ? animOffset * (CLIP_LENGTH[src] ?? 0) : undefined,
    sounds: [],
  });
  switch (animState) {
    case 'Jump': case 'Up': return clip(set.start, false);
    case 'UpToDown': return clip(set.mid, false);
    case 'Down': return clip(set.fall, true);
    case 'LongFall': return clip(CLIP_LONG_FALL, true);
    case 'Grounded': return clip(glideLanding ? GLIDE_LANDING[animTier] : set.end, false, true);
    case 'DoubleJump': return { ...clip(doubleJumpClip, false), speed: DOUBLE_JUMP_SPEED };
    case 'HardLanding': return hardLandingAnimation();
    default: return null;
  }
}

// Hard landing after the touchdown frame: Enter to its end, Loop until the stun is over (and at least its blend-in),
// Exit to HARD_LANDING_EXIT_END, then null to let what follows blend in over HARD_LANDING_AFTER_BLEND.
function hardLandingAnimation(): MovementAnimation | null {
  if (hardLanding === 'enter' && clipFinished(HARD_LANDING.enter)) {
    hardLanding = 'loop';
    hardLandingLoopFrom = time;
  } else if (hardLanding === 'loop' && !stunned() && time - hardLandingLoopFrom >= HARD_LANDING_LOOP_BLEND) {
    hardLanding = 'exit';
  } else if (hardLanding === 'exit' && clipReached(HARD_LANDING.exit, HARD_LANDING_EXIT_END)) {
    enterState('Movement', HARD_LANDING_AFTER_BLEND);
    return null;
  }
  const changed = hardLanding !== prevHardLanding;
  prevHardLanding = hardLanding;
  const exit = hardLanding === 'exit';
  return {
    src: HARD_LANDING[hardLanding as 'enter' | 'loop' | 'exit'],
    speed: exit ? HARD_LANDING_EXIT_SPEED : 1,
    loop: hardLanding === 'loop',
    idle: true,
    transitionSeconds: hardLanding === 'enter' ? HARD_LANDING_ENTER_BLEND : exit ? HARD_LANDING_EXIT_BLEND : HARD_LANDING_LOOP_BLEND,
    playbackTime: changed ? 0 : undefined,
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
  const glideOpen = glideProp === GlideProp.OPENING || glideProp === GlideProp.GLIDING;
  if (!glideOpen || grounded) {
    // Glide ended, or the avatar is on the ground with the prop still open(ing). Stow (close)
    // only if we ended mid-air (jump released while falling); a glide that ends by landing
    // lets the glide landing take over. A stow ends at the end of its clip (stateAnimation), on
    // touchdown, or when the prop has closed.
    if (glidePhase === 'deploy' || glidePhase === 'glide') {
      if (grounded) {
        // Landed while gliding: the glide landing closes the glider (close embedded).
        glidePhase = 'none';
        landingFromGlide = wasJumpingOrFalling;
      } else {
        glidePhase = 'stow';
        glideFrom = time;
        glideTick = tick;
      }
    } else if (glidePhase === 'stow' && (grounded || glideProp === GlideProp.CLOSED)) {
      glidePhase = 'none';
    }
    prevGlideSrc = undefined;
  }
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

  const animation = stateAnimation(jumpTrigger, jumps, jumpingOrFalling, horizontalSpeed)
    ?? locomotionAnimation(horizontalSpeed);
  if (animState === 'Movement' && animTick === tick) {
    if (animBlend !== undefined) animation.transitionSeconds = animBlend;
    if (animOffset !== undefined && MOVE_CLIPS.includes(animation.src)) {
      animation.playbackTime = animOffset * CLIP_LENGTH[animation.src];
    }
  }
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
        src: CLIP_WALK,
        speed: horizontalSpeed / WALK_CLIP_SPEED,
        loop: true,
        idle: false,
        transitionSeconds: settings.transWalk,
        sounds: footstepsFor(CLIP_WALK, WALK_STEP_TIMES, WALK_STEP_SOUNDS),
      };
    }
    // Tier 2: jog (the default movement speed)
    if (horizontalSpeed <= settings.sprintThreshold) {
      return {
        src: CLIP_JOG,
        speed: horizontalSpeed / JOG_CLIP_SPEED,
        loop: true,
        idle: false,
        transitionSeconds: settings.transRun,
        sounds: footstepsFor(CLIP_JOG, RUN_STEP_TIMES, RUN_STEP_SOUNDS),
      };
    }
    // Tier 3: run (sprint)
    return {
      src: CLIP_RUN,
      speed: horizontalSpeed / RUN_CLIP_SPEED,
      loop: true,
      idle: false,
      transitionSeconds: settings.transRun,
      sounds: footstepsFor(CLIP_RUN, RUN_STEP_TIMES, RUN_STEP_SOUNDS),
    };
  }

  // A stop settles straight into idle, as in the reference client, which never plays a stop clip.
  return {
    src: CLIP_IDLE,
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

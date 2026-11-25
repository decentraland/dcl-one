import { AssetLoad, AvatarAnimationState, AvatarMovement, AvatarMovementInfo, engine, MovementAnimation, Transform } from '@dcl/sdk/ecs'
import { Quaternion, Vector3 } from '@dcl/sdk/math';
import { grounded, initGroundRaycast, recordGroundForNextTick } from './ground';
import { movementAxis, orientation, relativeDegrees, updateHorizontalVelocity } from './horizontal';
import { GlideProp, glideProp, groundSpeed, initStepCasts, isDoubleJump, isGliding, jumpStartHeight, landingStuns, stunned, updateVerticalVelocity } from './vertical';
import { initParameters } from './parameters';
import { initWalkSystem, updateEngineWalk, consumeWalkResult } from './walk';
import { MAX_SPEED, MAX_TICK_TIME, GLIDE_TILT_DAMP_TIME, GLIDE_TILT_FULL_ANGLE, VEC3_DOWN, VEC3_ZERO } from './constants';
import { settings } from './settings';
import { beginTick } from './clock';

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

// Phased jump clip sets, picked by take-off locomotion. 'idle' is the plain
// "Jump_*" set (the standing jump). Each jump plays:
//   Start (once) -> Rise (loop, while ascending) -> Mid (once, at apex)
//   -> Fall (loop, while descending) -> End (once, on touchdown)
// Hard_Landing replaces End on big drops; Long_Fall_Loop replaces Fall on long ones.
type JumpVariation = 'idle' | 'jog' | 'run';
type JumpPhase = 'none' | 'start' | 'rise' | 'mid' | 'fall';
const JUMP_SETS: Record<JumpVariation, { start: string; rise: string; mid: string; fall: string; end: string }> = {
  idle: { start: ANIM + 'Jump_Start.glb', rise: ANIM + 'Jump_Rise_Loop.glb', mid: ANIM + 'Jump_Mid.glb', fall: ANIM + 'Jump_Fall_Loop.glb', end: ANIM + 'Jump_End.glb' },
  jog: { start: ANIM + 'Jog_Jump_Start.glb', rise: ANIM + 'Jog_Jump_Rise_Loop.glb', mid: ANIM + 'Jog_Jump_Mid.glb', fall: ANIM + 'Jog_Jump_Fall_Loop.glb', end: ANIM + 'Jog_Jump_End.glb' },
  run: { start: ANIM + 'Run_Jump_Start.glb', rise: ANIM + 'Run_Jump_Rise_Loop.glb', mid: ANIM + 'Run_Jump_Mid.glb', fall: ANIM + 'Run_Jump_Fall_Loop.glb', end: ANIM + 'Run_Jump_End.glb' },
};
const CLIP_LONG_FALL = ANIM + 'Long_Fall_Loop.glb';
const CLIP_HARD_LANDING = ANIM + 'Hard_Landing.glb';
// Landing clip with the glider close embedded, used when touching down while still
// gliding so the glider stows as part of the landing (plays from 0, not soft-sliced).
const CLIP_GLIDE_LANDING = ANIM + 'Hard_Landing_GliderRig.glb';
const CLIP_JOG_STOP = ANIM + 'Jog_Stop.glb';
// Hard landing on a stunning touchdown out of the long fall loop. The engine's network state reads a stun from these
// clips played at speed 1 or slower: Enter and Loop, not the faster Exit, nor the plain `Hard_Landing`.
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

// Double-jump avatar clips, per take-off locomotion variation.
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
  start: ANIM + 'Gliding_AvatarStartRig.glb',     // deploy: glider opens (Glider_Open)
  end: ANIM + 'Gliding_AvatarEndRig.glb',         // stow: glider closes (Glider_Close)
};

// Every GLB clip the scene can drive, collected from the constants above so the
// list never drifts. Fed to a single AssetLoad entity in main() to pre-load all
// of them at startup — otherwise the first use of a clip hitches while its GLB
// streams in (most visible on the first jump/glide of a session).
const PRELOAD_CLIPS: string[] = [
  CLIP_IDLE, CLIP_WALK, CLIP_JOG, CLIP_RUN,
  ...Object.values(JUMP_SETS).flatMap((s) => [s.start, s.rise, s.mid, s.fall, s.end]),
  CLIP_LONG_FALL, CLIP_HARD_LANDING, CLIP_GLIDE_LANDING, CLIP_JOG_STOP,
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
// The replies published before it, newest first. A late reply gets a tick on the frame it lands on, so the frame before
// a tick may have applied one of these. That is a late reply, not a takeover.
const PUBLISHED_HISTORY = 2;
const earlierPublished: Vector3.MutableVector3[] = [];
for (let i = 0; i < PUBLISHED_HISTORY; i++) earlierPublished.push(Vector3.Zero());
// Frames the last reply was still in flight after the frame before this tick (0: it was applied there).
var replyLateFrames = 0;

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
  replyLateFrames = -1;
  if (Vector3.distance(lastPublished, prevRequestedVelocity) <= 0.1) replyLateFrames = 0;
  else for (let i = 0; i < PUBLISHED_HISTORY && replyLateFrames < 0; i++) {
    if (Vector3.distance(earlierPublished[i], prevRequestedVelocity) <= 0.1) replyLateFrames = i + 1;
  }
  beginTick(tickTime, stepTime, prevStepTime, Math.max(0, replyLateFrames));
  time += tickTime;

  initParameters(movementInfo?.activeAvatarLocomotionSettings, movementInfo?.activeInputModifier);
  updateEngineWalk(movementInfo?.walkTarget, movementInfo?.walkThreshold);

  // If we are not in control (engine consumed a different velocity than we
  // published), resync velocity to prevActual.
  // A missing requestedVelocity also means the engine applied zero. If the scene published zero, that is not a
  // takeover, so the horizontal model resyncs only on a real mismatch.
  horizontalResync = replyLateFrames < 0;
  const notInControl = horizontalResync || movementInfo?.requestedVelocity === undefined;
  if (notInControl) {
    Vector3.copyFrom(prevActualVelocity, velocity);
    Vector3.copyFrom(prevActualVelocity, prevRequestedVelocity);
  }
  engineDriven = movementInfo?.requestedVelocity === undefined
    && Vector3.distanceSquared(prevPlayerPosition, playerPosition) > 1e-8;
}

// Tracks the last jumpStartHeight we observed so we can detect a new jump
// (undefined -> defined) and seek the jump clip back to 0 on its first frame.
var prevJumpStartHeight: number | undefined = undefined;
// True from touchdown until the engine reports the non-looped landing clip has
// played through. While true, keep re-requesting the landing so the engine
// holds it; once activeAnimationState shows it has completed, fall through.
var requestingLanding = false;
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

// --- Phased jump state machine ---
var jumpPhase: JumpPhase = 'none';
var prevJumpPhase: JumpPhase = 'none';
var jumpVariation: JumpVariation = 'idle';
// Chosen touchdown clip (a *_Jump_End or Hard_Landing), held across the landing.
var landingClip: string | undefined = undefined;
// True when the current landing stuns (vertical model): the clip plays out
// fully since the avatar is held in place. Other landings are
// skippable/abortable by movement input.
var landingLocked = false;
// Scene time the current landing clip started, for the moving-landing blend-out.
var landingStartTime = -Infinity;
// Playback speed of the current landing clip (softLandSpeed for small drops).
var landingSpeed = 1;
// True when the current landing is a small-drop bob (sliced, see softLandTime).
var landingSoft = false;

// --- Double jump ---
// Clip of the current air jump (undefined: none), picked on its first frame.
var doubleJumpClip: string | undefined = undefined;
// True once that clip reached DOUBLE_JUMP_EXIT: the fall loop plays for the rest of the airtime.
var doubleJumpFalling = false;

// --- Hard landing ---
type HardLandingPhase = 'none' | 'enter' | 'loop' | 'exit';
var hardLanding: HardLandingPhase = 'none';
var prevHardLanding: HardLandingPhase = 'none';
// Scene time Loop started.
var hardLandingLoopFrom = 0;
// True when the last airborne frame played the long fall loop.
var longFallShown = false;
// Blend of the next frame's clip, set when the hard landing hands over.
var nextBlend: number | undefined = undefined;

// --- Glide turn rate ---
// Smoothed avatar turn rate (deg/s), used to pick the left/right glide lean.
// Computed each frame in applyMovement from the change in facing.
var glidePrevOrientation = 0;
var glideTurnRate = 0;
// Smoothed procedural glide bank (roll, degrees), eased toward a target derived from
// glideTurnRate while gliding and back to 0 otherwise. Published as AvatarMovement.tiltRoll
// so the engine banks the whole avatar.
var glideTilt = 0;

// Glide animation phase, from the vertical model's glider prop (glideProp): 'deploy' plays the
// Start clip (glider opens) while the prop opens, 'glide' is the steady forward/idle loop while
// it glides, 'stow' plays the End clip (glider closes) while it closes mid-air. A glide that
// ends by landing skips 'stow' (the landing clip takes over).
type GlidePhase = 'none' | 'deploy' | 'glide' | 'stow';
var glidePhase: GlidePhase = 'none';
// Set when a glide ends by touching down, so the next landing uses the glider-close
// landing clip (the stow plays concurrently with the landing). Consumed in landingAnimation.
var landingFromGlide = false;
// Glide pose clip published last frame (forward/idle), to detect a swap mid-glide.
var prevGlideSrc: string | undefined = undefined;

// --- Jog/run stop transition ---
// True while the one-shot stop clip is playing out before settling to idle.
var stopping = false;
// Last time (s) horizontal speed was at jog+ pace, so we can detect a recent
// fast move when the avatar comes to a halt (deceleration passes through walk
// speeds, so we can't rely on the immediately-previous frame's speed).
var lastFastMoveTime = -Infinity;
// When the current uninterrupted stretch of jog+ movement started — the stop
// clip only plays if the run lasted at least settings.jogStopMinRun.
var fastMoveStartTime = Infinity;

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

// Pick the jump clip set from take-off horizontal speed, reusing the locomotion
// tier thresholds: walk -> idle jump, jog -> jog jump, run/sprint -> run jump.
function variationForSpeed(h: number): JumpVariation {
  if (h <= settings.walkRunThreshold) return 'idle';
  if (h <= settings.sprintThreshold) return 'jog';
  return 'run';
}

// Airborne (rising/falling) jump animation. Advances the phase machine each
// frame from vertical velocity + clip-completion, and returns the phase clip.
function jumpPhaseAnimation(newJump: boolean, horizontalSpeed: number): MovementAnimation {
  const falling = velocity.y <= 0;

  // Enter / re-enter the machine: a fresh ground jump starts at 'start';
  // otherwise (passive walk-off, or resuming after a glide/double-jump ended
  // mid-air) we drop straight into the fall loop.
  if (newJump && jumpStartHeight !== undefined) {
    jumpVariation = variationForSpeed(horizontalSpeed);
    jumpPhase = 'start';
  } else if (jumpPhase === 'none') {
    jumpVariation = variationForSpeed(horizontalSpeed);
    jumpPhase = 'fall';
  }

  const set = JUMP_SETS[jumpVariation];

  // Advance the phase.
  switch (jumpPhase) {
    case 'start':
      if (falling) jumpPhase = 'mid';
      else if (clipFinished(set.start)) jumpPhase = 'rise';
      break;
    case 'rise':
      if (falling) jumpPhase = 'mid';
      break;
    case 'mid':
      if (clipFinished(set.mid)) jumpPhase = 'fall';
      break;
    // 'fall' holds until we touch down (handled in landingAnimation).
  }

  const dropSoFar = maxUngroundedY - playerPosition.y;
  const useLongFall = jumpPhase === 'fall' && dropSoFar > settings.longFallDrop;

  let src: string;
  let loop: boolean;
  switch (jumpPhase) {
    case 'start': src = set.start; loop = false; break;
    case 'rise': src = set.rise; loop = true; break;
    case 'mid': src = set.mid; loop = false; break;
    default: src = useLongFall ? CLIP_LONG_FALL : set.fall; loop = true; break;
  }
  longFallShown = src === CLIP_LONG_FALL;

  // Seek to 0 on the first frame of a new phase so one-shots play in full.
  const phaseChanged = jumpPhase !== prevJumpPhase;
  prevJumpPhase = jumpPhase;

  // Fresh ground jump: snap into the start clip near-instantly (transJumpStart,
  // platformer-style responsiveness) and optionally skip the clip's anticipation
  // wind-up (jumpStartSkip) so the up-pose shows the moment the body launches.
  const freshStart = newJump && jumpPhase === 'start';

  return {
    src,
    speed: 1,
    loop,
    idle: false,
    transitionSeconds: freshStart ? settings.transJumpStart : settings.transAir,
    playbackTime: phaseChanged ? (freshStart ? settings.jumpStartSkip : 0) : undefined,
    sounds: newJump && jumpPhase === 'start' ? [pickRandom(JUMP_SOUNDS)] : [],
  };
}

// Touchdown animation: plays the chosen End / Hard_Landing clip once, then
// returns null to let locomotion resume.
function landingAnimation(justLanded: boolean): MovementAnimation | null {
  // Landing while moving (and not stunned) shows the touchdown briefly, then
  // blends out into locomotion after landRunBlend seconds instead of holding
  // the planted-feet clip to completion (which reads as a sliding crouch).
  // Only stunning landings play out compulsorily — there the stun
  // holds the player in place anyway.
  const moving = Vector3.lengthSquared(movementAxis) > 0;

  if (justLanded) {
    const drop = maxUngroundedY - playerPosition.y;
    // Every landing shows a crouch-absorb bob: small drops reuse the
    // Hard_Landing clip started partway in (softLandStart skips the deep
    // crouch) and sped up (softLandSpeed), so it reads as a quick head-bob
    // dip; big drops (> hardLandingDrop) play the clip in full from 0.
    const soft = drop <= settings.hardLandingDrop;
    // the model may count the touchdown a frame after this one: landingStuns() tells it before
    const stun = stunned() || landingStuns();
    hardLanding = 'none';
    if (landingFromGlide) {
      // Stow the glider as we land: the close-embedded clip, played from 0 (not
      // soft-sliced) so the glider close plays in full.
      landingClip = CLIP_GLIDE_LANDING;
      landingSoft = false;
      landingSpeed = 1;
      landingFromGlide = false;
    } else if (stun && longFallShown) {
      hardLanding = 'enter';
      landingClip = HARD_LANDING.enter;
      landingSoft = false;
      landingSpeed = 1;
    } else {
      landingClip = CLIP_HARD_LANDING;
      landingSoft = soft;
      landingSpeed = soft ? settings.softLandSpeed : 1;
    }
    landingLocked = stunned();
    landingStartTime = time;
    maxUngroundedY = -Infinity;
    jumpPhase = 'none';
    prevJumpPhase = 'none';
    prevHardLanding = hardLanding;
    return {
      src: landingClip,
      speed: landingSpeed,
      loop: false,
      idle: true,
      transitionSeconds: hardLanding === 'enter' ? HARD_LANDING_ENTER_BLEND : settings.transAir,
      playbackTime: landingSoft ? settings.softLandStart : 0,
      sounds: drop > 0.5 ? [pickRandom(LAND_SOUNDS)] : [],
    };
  }

  if (hardLanding !== 'none') return hardLandingAnimation();

  const clip = landingClip ?? JUMP_SETS[jumpVariation].end;
  // Moving + non-stunning: let the touchdown show for landRunBlend seconds,
  // then hand back to locomotion (which blends it out over transRun).
  if (moving && !landingLocked && time - landingStartTime >= settings.landRunBlend) {
    requestingLanding = false;
    landingClip = undefined;
    return null;
  }
  // Soft landings only show a slice of the clip (the absorb dip), then hand
  // back to idle/locomotion, which crossfades the rest out — the Hard_Landing
  // clip is too expressive to play out for a normal hop.
  if (landingSoft && !landingLocked && time - landingStartTime >= settings.softLandTime) {
    requestingLanding = false;
    landingClip = undefined;
    return null;
  }
  if (clipFinished(clip)) {
    requestingLanding = false;
    landingClip = undefined;
    return null;
  }
  return { src: clip, speed: landingSpeed, loop: false, idle: true, transitionSeconds: settings.transAir, sounds: [] };
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
    hardLanding = 'none';
    requestingLanding = false;
    landingClip = undefined;
    nextBlend = HARD_LANDING_AFTER_BLEND;
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
    // lets the landing clip take over. A stow ends when the prop has closed or on touchdown.
    if (glidePhase === 'deploy' || glidePhase === 'glide') {
      if (grounded) {
        // Landed while gliding: let the landing clip stow the glider (close embedded).
        glidePhase = 'none';
        landingFromGlide = wasJumpingOrFalling;
      } else {
        glidePhase = 'stow';
      }
    } else if (glidePhase === 'stow' && (grounded || glideProp === GlideProp.CLOSED)) {
      glidePhase = 'none';
    }
    prevGlideSrc = undefined;
  }
  if (!isDoubleJump) {
    doubleJumpClip = undefined;
    doubleJumpFalling = false;
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

  const horizontalSpeed = Math.sqrt(velocity.x * velocity.x + velocity.z * velocity.z);
  if (horizontalSpeed > settings.walkRunThreshold) {
    // Track when this stretch of jog+ movement began, so the stop clip can
    // require a sustained run — a quick reposition tap shouldn't foot-plant.
    if (time - lastFastMoveTime > 0.25) fastMoveStartTime = time;
    lastFastMoveTime = time;
  }

  if (jumpingOrFalling) {
    requestingLanding = true;
    stopping = false;
    hardLanding = 'none';
    longFallShown = false;

    if (glideOpen) {
      jumpPhase = 'none';
      // Deploy: the Start clip (glider opens) while the prop opens, then the steady loop.
      if (glideProp === GlideProp.OPENING) {
        glidePhase = 'deploy';
        return {
          src: GLIDE_AVATAR.start,
          speed: 1,
          loop: false,
          idle: false,
          transitionSeconds: settings.transAir,
          sounds: [],
        };
      }
      if (glidePhase !== 'glide') {
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
        transitionSeconds: settings.gliderFade,
        playbackTime:
          poseSwapped && activeAnimationState !== undefined
            ? activeAnimationState.playbackTime + stepTime
            : undefined,
        sounds: [],
      };
    }

    // Stow: glide ended mid-air -> play the End clip (glider closes) while the prop closes,
    // then the fall resumes.
    if (glidePhase === 'stow') {
      return {
        src: GLIDE_AVATAR.end,
        speed: 1,
        loop: false,
        idle: false,
        transitionSeconds: settings.transAir,
        sounds: [],
      };
    }

    if (isDoubleJump && !doubleJumpFalling) {
      // Use the variation captured by the first jump (persisted in jumpVariation).
      const fresh = doubleJumpClip === undefined;
      if (fresh) {
        const clips = DOUBLE_JUMP[jumpVariation];
        doubleJumpClip = Math.random() < 0.5 ? clips.plain : clips.right;
      }
      if (!clipReached(doubleJumpClip!, DOUBLE_JUMP_EXIT)) {
        jumpPhase = 'none';
        return {
          src: doubleJumpClip!,
          speed: DOUBLE_JUMP_SPEED,
          loop: false,
          idle: false,
          transitionSeconds: DOUBLE_JUMP_BLEND,
          playbackTime: fresh ? 0 : undefined,
          sounds: newJump ? [pickRandom(JUMP_SOUNDS)] : [],
        };
      }
      // the rest of the airtime falls: the fall loop of the same variation, from its start
      doubleJumpFalling = true;
      jumpPhase = 'fall';
      prevJumpPhase = 'none';
    }

    // Phased jump/fall: Start -> Rise -> Mid -> Fall (-> End on landing below). An air jump's launch is not a new jump
    // here.
    return jumpPhaseAnimation(newJump && !isDoubleJump, horizontalSpeed);
  }

  // Touchdown: play the End / Hard_Landing clip once, then fall through.
  if (requestingLanding) {
    const landing = landingAnimation(justLanded);
    if (landing !== null) return landing;
  }

  // Locomotion clips play at the speed's magnitude, not its projection on the facing, so a clip never plays backwards
  // while the avatar turns. No floor: below moveGate the avatar idles, and above it a slower clip keeps the feet planted.

  // Locomotion tiers only while there's actual movement input. With no keys
  // held, the residual decelerating velocity (jog -> 0 passes through walk
  // speeds for a few frames) would otherwise flash the walk clip — the
  // "residual walking" shuffle — instead of going straight to stop/idle.
  const hasMoveInput = Vector3.lengthSquared(movementAxis) > 0;
  if (horizontalSpeed > settings.moveGate && hasMoveInput) {
    stopping = false; // moving again — cancel any pending stop
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

  // Jog/run stop: if we halted within a moment of moving at jog+ pace, play the
  // one-shot stop clip before settling into idle.
  let newStop = false;
  // Only foot-plant after a sustained run (jogStopMinRun) — brief taps and
  // small repositions settle straight into idle instead.
  const ranLongEnough = lastFastMoveTime - fastMoveStartTime >= settings.jogStopMinRun;
  if (!stopping && time - lastFastMoveTime < 0.25 && ranLongEnough) {
    stopping = true;
    newStop = true;
  }
  if (stopping) {
    if (clipFinished(CLIP_JOG_STOP)) {
      stopping = false;
    } else {
      return {
        src: CLIP_JOG_STOP,
        speed: settings.jogStopSpeed,
        loop: false,
        idle: false,
        transitionSeconds: settings.transWalk,
        // Skip past the clip's opening step if tuned (jogStopStart > 0).
        ...(newStop ? { playbackTime: settings.jogStopStart } : {}),
        sounds: [],
      };
    }
  }

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
  if (nextBlend !== undefined) {
    animation.transitionSeconds = nextBlend;
    nextBlend = undefined;
  }
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

  for (let i = PUBLISHED_HISTORY - 1; i > 0; i--) Vector3.copyFrom(earlierPublished[i - 1], earlierPublished[i]);
  Vector3.copyFrom(lastPublished, earlierPublished[0]);
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

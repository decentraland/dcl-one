// The avatar animator's base layer as data for animator.ts: its states, with their clips, blend trees, speeds and tags,
// and its transitions, with their conditions, durations, exit times, offsets and interruption sources. The parameters
// come from the movement model (index.ts).
// Not here: emotes (the engine plays them) and states that no parameter of the scene starts.
import { AnimatorController, Condition, Transition, Tree } from './animator';

// The engine reads jumps, the glide and the stun for its jump timing and network state from these exact file names
// (crates/common/src/movement_clip.rs), so a renamed clip is renamed there too.
const ANIM = 'assets/animations/';
export const LOCOMOTION = { idle: ANIM + 'Idle.glb', walk: ANIM + 'Walk.glb', jog: ANIM + 'Jog.glb', run: ANIM + 'Run.glb' };
// The glider is embedded in the glide clips (Gliding_Start, Gliding_End and the glide loop's poses), so it renders and
// networks with the avatar.
export const GLIDE_POSE = { forward: ANIM + 'Gliding_AvatarForwardRig.glb', idle: ANIM + 'Gliding_AvatarIdleRig.glb' };
const GLIDE_START = ANIM + 'Gliding_AvatarStartRig.glb';
const GLIDE_END = ANIM + 'Gliding_AvatarEndRig.glb';
const LONG_FALL = ANIM + 'Long_Fall_Loop.glb';
const HARD_LANDING = { enter: ANIM + 'Hard_Landing_Enter.glb', loop: ANIM + 'Hard_Landing_Loop.glb', exit: ANIM + 'Hard_Landing_Exit.glb' };
const DOUBLE_JUMP = (t: string) => ANIM + `DoubleJump_${t}.glb`;
const TIERS = ['Jump', 'Jog_Jump', 'Run_Jump'];
const clip = (tier: string, part: string) => ANIM + (part === 'End' && tier === 'Jump' ? 'Idle_Jump_End' : `${tier}_${part}`) + '.glb';
// The jump states' 1D trees on MovementBlend: Jump_* at 1, Jog_Jump_* at 2, Run_Jump_* at 3.
const tierTree = (part: string, at = [1, 2, 3]): Tree =>
  ({ param: 'MovementBlend', children: TIERS.map((t, i) => ({ clip: clip(t, part), at: at[i] })) });
// A landing while the glider is open plays the tier's End with the glider closing embedded: the glider is part of the
// avatar's clips, so only the landing clip can close it.
const GLIDER_CLOSING: Record<string, string> = {
  [clip('Jump', 'End')]: ANIM + 'Gliding_Idle_Jump_EndRig.glb',
  [clip('Jog_Jump', 'End')]: ANIM + 'Gliding_Jog_Jump_EndRig.glb',
  [clip('Run_Jump', 'End')]: ANIM + 'Gliding_Run_Jump_EndRig.glb',
};

// A transition (destination, conditions, duration: s when fixed, else a fraction of the source state's length).
const tr = (to: string, when: Condition[], duration: number, fixed: boolean, o: Partial<Transition> = {}): Transition =>
  ({ to, when, duration, fixed, ...o });
const GROUNDED: Condition = ['IsGrounded', 'if'];
const AIRBORNE: Condition = ['IsGrounded', 'ifNot'];
const STUNNED: Condition = ['IsStunned', 'if'];
const NOT_GLIDING: Condition = ['IsGliding', 'ifNot'];
const DEST = 'destination', SRC = 'source';

const s = (frames: number) => frames / 60; // the clips are 60 fps
const once = (length: number) => ({ length, loop: false }), looped = (length: number) => ({ length, loop: true });

export const CHARACTER_ANIMATOR: AnimatorController = {
  entry: 'Movement',
  // The scene's clip lengths (s) and loop flags; also the clips the scene preloads.
  clips: {
    [LOCOMOTION.idle]: looped(s(120)), [LOCOMOTION.walk]: looped(s(60)),
    [LOCOMOTION.jog]: looped(s(40)), [LOCOMOTION.run]: looped(s(32)),
    [clip('Jump', 'Start')]: once(s(12)), [clip('Jump', 'Mid')]: once(s(26)),
    [clip('Jump', 'Fall_Loop')]: looped(s(40)), [clip('Jump', 'End')]: once(s(16)),
    [clip('Jog_Jump', 'Start')]: once(s(14)), [clip('Jog_Jump', 'Mid')]: once(s(24)),
    [clip('Jog_Jump', 'Fall_Loop')]: looped(s(40)), [clip('Jog_Jump', 'End')]: once(s(24)),
    [clip('Run_Jump', 'Start')]: once(s(14)), [clip('Run_Jump', 'Mid')]: once(s(24)),
    [clip('Run_Jump', 'Fall_Loop')]: looped(s(40)), [clip('Run_Jump', 'End')]: once(s(22)),
    [LONG_FALL]: looped(s(48)),
    [GLIDER_CLOSING[clip('Jump', 'End')]]: once(s(16)),
    [GLIDER_CLOSING[clip('Jog_Jump', 'End')]]: once(s(24)),
    [GLIDER_CLOSING[clip('Run_Jump', 'End')]]: once(s(22)),
    [HARD_LANDING.enter]: once(s(30)), [HARD_LANDING.loop]: looped(s(118)), [HARD_LANDING.exit]: once(s(25)),
    // The air jump leaves at 0.778 of the clip: the scene does not loop it.
    [DOUBLE_JUMP('Base')]: once(s(41)), [DOUBLE_JUMP('Base_Right')]: once(s(38)),
    [DOUBLE_JUMP('Jog')]: once(s(41)), [DOUBLE_JUMP('Jog_Right')]: once(s(38)),
    [DOUBLE_JUMP('Run')]: once(s(41)), [DOUBLE_JUMP('Run_Right')]: once(s(38)),
    [GLIDE_POSE.forward]: looped(s(238)), [GLIDE_POSE.idle]: looped(s(238)),
    [GLIDE_START]: once(s(68)), [GLIDE_END]: once(s(16)),
  },
  // No jump state while the glider is open (NOT_GLIDING): the glider is embedded in the glide clips, which a jump clip
  // would replace.
  any: [
    tr('Jump', [['Jump', 'if'], ['JumpCount', 'less', 2], NOT_GLIDING], 0.1310842, true,
      { offset: 0.3291167, interruption: DEST, noSelf: true }),
    tr('Double Jump', [['Jump', 'if'], ['JumpCount', 'greater', 1], NOT_GLIDING], 0.05, true),
    tr('Gliding_Start', [['StartGliding', 'if']], 0.05, true),
  ],
  states: {
    // The locomotion tree: the scene picks its clip by the horizontal speed (index.ts), and the tree times its
    // transitions.
    Movement: {
      tag: 'Movement',
      motion: {
        scene: 'locomotion',
        tree: { param: 'MovementBlend', children: Object.values(LOCOMOTION).map((c, i) => ({ clip: c, at: i })) },
      },
      transitions: [
        tr('Up', [['IsJumping', 'if']], 0.06466603, false),
        tr('Down', [['IsFalling', 'if']], 0.29039198, false, { offset: 0.007114472, interruption: DEST }),
      ],
    },
    Jump: {
      tag: 'Jumping', motion: tierTree('Start'),
      transitions: [
        tr('Up', [], 0.18402585, false, { exitTime: 0.8144884, interruption: DEST }),
        tr('Grounded', [GROUNDED], 0.021802306, false, { offset: 0.017173447 }),
      ],
    },
    // Up keeps Jump's clip: Up To Down interrupts the cross-fade into Up on the next update, so a clip of Up's own
    // would never lead.
    Up: {
      tag: 'Jumping', motion: 'previous',
      transitions: [tr('Up To Down', [['IsJumping', 'ifNot']], 0.06453482, true, { interruption: DEST })],
    },
    'Up To Down': {
      tag: 'Jumping', motion: tierTree('Mid'),
      transitions: [
        tr('Grounded', [GROUNDED], 0.021802306, false, { offset: 0.017173447 }),
        tr('Down', [], 0.21651052, false, { exitTime: 0.5, interruption: DEST }),
      ],
    },
    // The fall loops' tree, Jump_Fall_Loop at 1.7. The scene has no slide (SlideBlend 0).
    Down: {
      tag: 'Jumping', motion: tierTree('Fall_Loop', [1.7, 2, 3]),
      transitions: [
        tr('Grounded', [GROUNDED], 0.062423754, false),
        tr('Long Fall', [['IsLongFall', 'if'], ['SlideBlend', 'less', 0.5]], 0.33667788, false, { interruption: SRC }),
      ],
    },
    'Long Fall': {
      tag: 'Jumping', motion: { clip: LONG_FALL },
      transitions: [
        tr('Hard Landing', [GROUNDED, STUNNED], 0.058513165, true),
        tr('Down', [['IsLongFall', 'ifNot'], AIRBORNE], 0.25, true),
        tr('Grounded', [GROUNDED, ['IsStunned', 'ifNot']], 0.062423754, false),
      ],
    },
    // Its transitions end the jump states and return to Movement.
    Grounded: {
      tag: 'Jumping', motion: tierTree('End'), variant: { when: [['IsGliding', 'if']], clips: GLIDER_CLOSING },
      transitions: [
        tr('Movement', [['MovementBlend', 'greater', 0.5]], 0.35801375, false, { offset: 0.025870794 }),
        tr('Movement', [], 0.39824668, false, { exitTime: 0.4778491 }),
      ],
    },
    'Hard Landing': {
      tag: 'Jumping', motion: { clip: HARD_LANDING.enter },
      transitions: [tr('Hard Landing Loop', [], 0.15, true, { exitTime: 1 })],
    },
    'Hard Landing Loop': {
      motion: { clip: HARD_LANDING.loop },
      transitions: [tr('Hard Landing Exit', [['IsStunned', 'ifNot']], 0.05, true, { interruption: SRC })],
    },
    'Hard Landing Exit': {
      speed: 1.2, motion: { clip: HARD_LANDING.exit },
      transitions: [tr('Movement', [], 0.14, true, { exitTime: 0.2, interruption: SRC })],
    },
    // 2D cartesian tree: MovementBlend 1..3 by JumpVariation 0 (plain) or 1 (Right), each at 0.7.
    'Double Jump': {
      motion: {
        param: 'MovementBlend', param2: 'JumpVariation',
        children: ['Base', 'Jog', 'Run'].flatMap((t, i) =>
          [0, 1].map((v) => ({ clip: DOUBLE_JUMP(v ? `${t}_Right` : t), at: [i + 1, v] as [number, number], timeScale: 0.7 }))),
      },
      transitions: [
        tr('Down', [AIRBORNE], 0.1, true, { exitTime: 0.778, interruption: SRC }),
        tr('Grounded', [GROUNDED], 0.1, true),
      ],
    },
    Gliding_Start: {
      motion: { clip: GLIDE_START },
      transitions: [
        tr('Hard Landing', [GROUNDED, STUNNED], 0.1, true),
        tr('Grounded', [GROUNDED], 0.1, true),
        // The End before the loop when both are due on one update: the End would interrupt the loop's cross-fade on
        // the next update, so the loop's pose would only flash.
        tr('Gliding_End', [NOT_GLIDING], 0.1, true, { interruption: SRC }),
        tr('Gliding Loop', [], 0.05, true, { exitTime: 1, interruption: SRC }),
      ],
    },
    // The glide pose: the scene picks the forward or the idle pose by the horizontal speed (index.ts).
    'Gliding Loop': {
      motion: { scene: 'glidePose' },
      transitions: [
        tr('Grounded', [GROUNDED], 0.1, true),
        tr('Gliding_End', [NOT_GLIDING], 0.1, true, { interruption: SRC }),
      ],
    },
    Gliding_End: {
      cycleOffset: 0.05, motion: { clip: GLIDE_END },
      transitions: [
        tr('Down', [], 0.1, true, { exitTime: 1, interruption: SRC }),
        tr('Grounded', [GROUNDED], 0.1, true),
      ],
    },
  },
};

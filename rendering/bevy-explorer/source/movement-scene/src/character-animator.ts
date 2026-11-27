// The reference client's avatar animator, its base layer, as data for animator.ts: the states and transitions with the
// values its asset has, each with the line of its entry in that asset. The parameters come from the movement model
// (index.ts), set as the reference client sets them.
// Not here: the emote states and transitions (the engine plays emotes), Rotation State, Hit_Wall_Walk and AFK_Action
// (no parameter of the scene starts them). Where the scene departs from the reference, the entry says why.
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
// A landing while the glider is open plays the tier's End with the glider closing embedded: the reference client
// closes its glider prop apart from the animator.
const GLIDER_CLOSING: Record<string, string> = {
  [clip('Jump', 'End')]: ANIM + 'Gliding_Idle_Jump_EndRig.glb',
  [clip('Jog_Jump', 'End')]: ANIM + 'Gliding_Jog_Jump_EndRig.glb',
  [clip('Run_Jump', 'End')]: ANIM + 'Gliding_Run_Jump_EndRig.glb',
};

// A transition (line of its entry, destination, conditions, duration: s when fixed, else a fraction of the source
// state's length).
const tr = (line: number, to: string, when: Condition[], duration: number, fixed: boolean, o: Partial<Transition> = {}):
  Transition => ({ line, to, when, duration, fixed, ...o });
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
  // Scene: no jump state while the glider is open (NOT_GLIDING): the glider is embedded in the glide clips.
  any: [
    tr(2959, 'Jump', [['Jump', 'if'], ['JumpCount', 'less', 2], NOT_GLIDING], 0.1310842, true,
      { offset: 0.3291167, interruption: DEST, noSelf: true }),
    tr(2906, 'Double Jump', [['Jump', 'if'], ['JumpCount', 'greater', 1], NOT_GLIDING], 0.05, true),
    tr(2781, 'Gliding_Start', [['StartGliding', 'if']], 0.05, true),
  ],
  states: {
    // The locomotion tree (L2036): the scene picks its clip by the horizontal speed (index.ts).
    Movement: {
      line: 884, tag: 'Movement',
      motion: {
        scene: 'locomotion',
        tree: { param: 'MovementBlend', children: Object.values(LOCOMOTION).map((c, i) => ({ clip: c, at: i })) },
      },
      transitions: [
        // Jog_Stop (L3077) is never reached: this transition and its own back are muted.
        tr(2686, 'Jog_Stop', [['MovementBlend', 'less', 1], ['MovementBlend', 'greater', 0.9]], 0.24999994, true,
          { offset: 0.15320888, mute: true }),
        tr(3186, 'Up', [['IsJumping', 'if']], 0.06466603, false),
        tr(2303, 'Down', [['IsFalling', 'if']], 0.29039198, false, { offset: 0.007114472, interruption: DEST }),
      ],
    },
    Jump: {
      line: 3631, tag: 'Jumping', motion: tierTree('Start'),
      transitions: [
        tr(3287, 'Up', [], 0.18402585, false, { exitTime: 0.8144884, interruption: DEST }),
        tr(917, 'Grounded', [GROUNDED], 0.021802306, false, { offset: 0.017173447 }),
      ],
    },
    // The rise loops (tree L2463) never lead: Up To Down interrupts the cross-fade into Up on the next update, so the
    // scene keeps Jump's clip.
    Up: {
      line: 430, tag: 'Jumping', motion: 'previous',
      transitions: [tr(2934, 'Up To Down', [['IsJumping', 'ifNot']], 0.06453482, true, { interruption: DEST })],
    },
    'Up To Down': {
      line: 3371, tag: 'Jumping', motion: tierTree('Mid'),
      transitions: [
        tr(1404, 'Grounded', [GROUNDED], 0.021802306, false, { offset: 0.017173447 }),
        tr(81, 'Down', [], 0.21651052, false, { exitTime: 0.5, interruption: DEST }),
      ],
    },
    // The fall loops' tree (L316: Jump_Fall_Loop at 1.7) inside the slide tree (L3015); the scene has no slide
    // (SlideBlend 0).
    Down: {
      line: 3399, tag: 'Jumping', motion: tierTree('Fall_Loop', [1.7, 2, 3]),
      transitions: [
        tr(103, 'Grounded', [GROUNDED], 0.062423754, false),
        tr(1183, 'Long Fall', [['IsLongFall', 'if'], ['SlideBlend', 'less', 0.5]], 0.33667788, false, { interruption: SRC }),
      ],
    },
    // Long_Fall_Loop in the slide tree (L3046).
    'Long Fall': {
      line: 3236, tag: 'Jumping', motion: { clip: LONG_FALL },
      transitions: [
        tr(698, 'Hard Landing', [GROUNDED, STUNNED], 0.058513165, true),
        tr(1429, 'Down', [['IsLongFall', 'ifNot'], AIRBORNE], 0.25, true),
        tr(856, 'Grounded', [GROUNDED, ['IsStunned', 'ifNot']], 0.062423754, false),
      ],
    },
    // Its transitions go to the Jump State machine's exit, which returns to Movement.
    Grounded: {
      line: 377, tag: 'Jumping', motion: tierTree('End'), variant: { when: [['IsGliding', 'if']], clips: GLIDER_CLOSING },
      transitions: [
        tr(1379, 'Movement', [['MovementBlend', 'greater', 0.5]], 0.35801375, false, { offset: 0.025870794 }),
        tr(751, 'Movement', [], 0.39824668, false, { exitTime: 0.4778491 }),
      ],
    },
    'Hard Landing': {
      line: 590, tag: 'Jumping', motion: { clip: HARD_LANDING.enter },
      transitions: [tr(3527, 'Hard Landing Loop', [], 0.15, true, { exitTime: 1 })],
    },
    'Hard Landing Loop': {
      line: 4, motion: { clip: HARD_LANDING.loop },
      transitions: [tr(2174, 'Hard Landing Exit', [['IsStunned', 'ifNot']], 0.05, true, { interruption: SRC })],
    },
    'Hard Landing Exit': {
      line: 2147, speed: 1.2, motion: { clip: HARD_LANDING.exit },
      transitions: [tr(1009, 'Movement', [], 0.14, true, { exitTime: 0.2, interruption: SRC })],
    },
    // 2D cartesian tree (L203): MovementBlend 1..3 by JumpVariation 0 (plain) or 1 (Right), each at 0.7.
    'Double Jump': {
      line: 773,
      motion: {
        param: 'MovementBlend', param2: 'JumpVariation',
        children: ['Base', 'Jog', 'Run'].flatMap((t, i) =>
          [0, 1].map((v) => ({ clip: DOUBLE_JUMP(v ? `${t}_Right` : t), at: [i + 1, v] as [number, number], timeScale: 0.7 }))),
      },
      transitions: [
        tr(2856, 'Down', [AIRBORNE], 0.1, true, { exitTime: 0.778, interruption: SRC }),
        tr(1523, 'Grounded', [GROUNDED], 0.1, true),
      ],
    },
    Gliding_Start: {
      line: 507, motion: { clip: GLIDE_START },
      transitions: [
        tr(670, 'Hard Landing', [GROUNDED, STUNNED], 0.1, true),
        tr(457, 'Grounded', [GROUNDED], 0.1, true),
        // Scene: the End before the loop when both are due on one update (the reference lists the loop first, and its
        // End interrupts the loop's cross-fade on the next update, so the loop's pose would only flash).
        tr(1354, 'Gliding_End', [NOT_GLIDING], 0.1, true, { interruption: SRC }),
        tr(355, 'Gliding Loop', [], 0.05, true, { exitTime: 1, interruption: SRC }),
      ],
    },
    // The 2D directional tree (L1263) on GlideBlend and MovementBlend: the scene picks the forward or the idle pose by
    // the horizontal speed (index.ts).
    'Gliding Loop': {
      line: 956, motion: { scene: 'glidePose' },
      transitions: [
        tr(405, 'Grounded', [GROUNDED], 0.1, true),
        tr(1457, 'Gliding_End', [NOT_GLIDING], 0.1, true, { interruption: SRC }),
      ],
    },
    Gliding_End: {
      line: 3474, cycleOffset: 0.05, motion: { clip: GLIDE_END },
      transitions: [
        tr(1690, 'Down', [], 0.1, true, { exitTime: 1, interruption: SRC }),
        tr(726, 'Grounded', [GROUNDED], 0.1, true),
      ],
    },
  },
};

// A small state-machine animator for a player whose renderer plays one clip at a time: each update it returns the
// current state's clip and how to cross-fade into it, and the engine blends from the clip before. The state machine is
// data (AnimatorController: states, Any State transitions, conditions on parameters, durations, exit times, offsets,
// interruption sources); this file holds only the rules that run it, the reference avatar animator's:
// - Any State transitions come first and may start whenever their conditions hold; one that may not transition to
//   itself (noSelf) is skipped when its destination is the current state. Muted transitions are never taken.
// - The other candidates are the current state's transitions, in their order. While the cross-fade into the state
//   still runs at the start of an update, only the transition's interruption source may start one: the destination's
//   own transitions ('destination'), the source state's ('source', the running transition excepted), or none (absent).
//   An update that starts a transition starts no other.
// - An exit time is a normalized time of the source state (its length is the clip's over the state's speed, from the
//   transition offset). Below 1 it recurs every cycle; from 1 it is taken once. It is taken on the update that crosses
//   it, but not while a cross-fade runs: a crossing inside the cross-fade waits for the next cycle, one after its end
//   is taken even on the update that ends it (the reference animator updates every rendered frame, so a crossing just
//   after the end does not wait for a cycle there).
// - A duration is seconds, or a fraction of the source state's length when not fixed; the offset is where the
//   destination starts (normalized), the destination state's cycle offset added.
// - A trigger counts for the update it is set in.
// - A blend tree plays its dominant child: the nearest threshold (per axis on a 2D grid, a tie to the higher one),
//   picked when the state starts. A motion the caller picks ({ scene }) is played as the caller decides; its length is
//   the tree's at the current parameters (children share their normalized time).
// - During a cross-fade the current state, as the animator reports it (tag), is the source until the cross-fade ends.
// Self-contained: no scene state, so it can serve any avatar.

export type Params = Record<string, number | boolean | undefined>;
// [parameter, mode, threshold]: 'if' / 'ifNot' for a bool or trigger, 'greater' / 'less' for a number
export type Condition = [string, 'if' | 'ifNot' | 'greater' | 'less', number?];

export type Transition = {
  to: string;
  when?: Condition[]; // all of them
  duration: number; // s if fixed, else a fraction of the source state's length
  fixed: boolean;
  exitTime?: number;
  offset?: number;
  interruption?: 'source' | 'destination';
  noSelf?: boolean;
  mute?: boolean;
  // the exit time is read from the engine's report of the clip playing, not counted on the update clock
  engineExit?: boolean;
  line: number; // of the transition's entry in the reference animator asset
};

export type Child = { clip: string; at: number | [number, number]; timeScale?: number };
export type Tree = { param: string; param2?: string; children: Child[] };
// A clip; a blend tree; a motion the caller picks the clip of (scene), with the tree that times it; or the previous
// state's clip, kept.
export type Motion = { clip: string } | Tree | { scene: string; tree?: Tree } | 'previous';

export type State = {
  motion: Motion;
  speed?: number;
  cycleOffset?: number;
  tag?: string;
  // clips the state plays in place of its own when it starts with these conditions holding (a variant of the
  // scene's that the reference animator does not have)
  variant?: { when: Condition[]; clips: Record<string, string> };
  transitions: Transition[];
  line: number;
};

export type AnimatorController = {
  entry: string;
  any: Transition[];
  states: Record<string, State>;
  clips: Record<string, { length: number; loop: boolean }>; // s
};

export type Pose = {
  state: string;
  clip?: string; // absent: the caller picks it (scene)
  scene?: string;
  speed: number;
  loop: boolean;
  blend: number; // s, the cross-fade into the state
  entered: boolean; // the state started on this update
  seek?: number; // fraction of the clip to start from, on the update the state starts
};

// The engine's report of the clip it plays.
export type ClipReport = { src: string; playbackTime: number; duration: number; loopCount: number };

const EPS = 1e-6; // lets an update that reaches an exit exactly (a sum of frame times) take it

export class Animator {
  private current: string;
  private time = 0;
  private from = ''; // the state the current one was entered from
  private via: Transition | undefined; // and the transition
  private blend = 0; // s, its cross-fade
  private since = 0; // time the state started
  private offset = 0; // normalized time it started from
  private length = 1; // s per normalized unit, fixed when its clip is picked
  private fromLength = 1;
  private pose: Pose;

  constructor(private readonly c: AnimatorController) {
    this.current = c.entry;
    const { clip, scene } = this.pick(c.states[c.entry].motion as Exclude<Motion, 'previous'>, {});
    this.pose = { state: c.entry, clip, scene, speed: 1, loop: true, blend: 0, entered: false };
  }

  get state(): string {
    return this.current;
  }

  // The current state's tag as the animator reports it: the source state's while the cross-fade runs.
  get tag(): string | undefined {
    const blending = this.time - this.since < this.blend;
    return this.c.states[blending ? this.from : this.state]?.tag;
  }

  // Advances the layer by dt (s) with the parameters of this update; report: the engine's clip, for engineExit.
  update(p: Params, dt: number, report?: ClipReport): Pose {
    this.time += dt;
    const t = this.time - this.since + EPS; // in the state at the end of this update
    const before = t - dt;
    const blending = before < this.blend;
    // [transition, its owner, whether an exit time of it is the current state's]
    const candidates: [Transition, string, boolean][] = this.c.any.map((tr) => [tr, this.state, true]);
    const own = this.c.states[this.state].transitions;
    if (!blending || this.via?.interruption === 'destination') {
      for (const tr of own) candidates.push([tr, this.state, true]);
    } else {
      if (this.via?.interruption === 'source') {
        for (const tr of this.c.states[this.from].transitions) if (tr !== this.via) candidates.push([tr, this.from, false]);
      }
      // the update that ends the cross-fade: the state's exit times crossed after the end
      if (t >= this.blend + EPS) for (const tr of own) if (tr.exitTime !== undefined) candidates.push([tr, this.state, true]);
    }
    // exit times count from the end of the cross-fade
    const exitFrom = blending ? this.blend + EPS : before;
    for (const [tr, owner, timed] of candidates) {
      if (tr.mute || (tr.noSelf && tr.to === this.state)) continue;
      if (!(tr.when ?? []).every((c) => holds(c, p))) continue;
      if (tr.exitTime !== undefined && (!timed || !this.exitCrossed(tr, exitFrom, t, report))) continue;
      const ownerLength = owner === this.state ? this.stateLength(p) : this.fromLength;
      this.enter(tr, tr.fixed ? tr.duration : tr.duration * ownerLength, p);
      return this.pose;
    }
    this.pose = { ...this.pose, entered: false, seek: undefined };
    return this.pose;
  }

  private exitCrossed(tr: Transition, before: number, t: number, report?: ClipReport): boolean {
    const exit = tr.exitTime!;
    if (tr.engineExit) {
      return report !== undefined && report.src === this.pose.clip
        && (report.loopCount >= 1 || report.playbackTime >= exit * report.duration);
    }
    const a = this.offset + before / this.length - exit;
    const b = this.offset + t / this.length - exit;
    return exit < 1 ? b >= 0 && Math.floor(b) > Math.floor(a) : a < 0 && b >= 0;
  }

  private enter(tr: Transition, blend: number, p: Params) {
    const prev = this.pose;
    this.fromLength = this.stateLength(p);
    this.from = this.state;
    this.via = tr;
    this.current = tr.to;
    this.blend = blend;
    this.since = this.time;
    this.offset = tr.offset ?? 0;
    const st = this.c.states[tr.to];
    if (st.motion === 'previous') {
      this.pose = { ...prev, state: tr.to, entered: true, seek: undefined };
      return;
    }
    const picked = this.pick(st.motion, p);
    const v = st.variant && st.variant.when.every((c) => holds(c, p)) ? st.variant.clips : undefined;
    const clip = picked.clip !== undefined ? v?.[picked.clip] ?? picked.clip : undefined;
    this.pose = {
      state: tr.to, clip, scene: picked.scene, speed: (st.speed ?? 1) * picked.timeScale,
      loop: clip !== undefined && this.c.clips[clip].loop, blend, entered: true, seek: this.offset + (st.cycleOffset ?? 0),
    };
  }

  // Picks the state's clip (fixing the state's length) or names the caller's motion.
  private pick(m: Exclude<Motion, 'previous'>, p: Params): { clip?: string; scene?: string; timeScale: number } {
    if ('scene' in m) return { scene: m.scene, timeScale: 1 };
    const child = 'clip' in m ? { clip: m.clip, at: 0 } : dominant(m, p);
    const timeScale = child.timeScale ?? 1;
    this.length = this.c.clips[child.clip].length / timeScale / (this.c.states[this.state].speed ?? 1);
    return { clip: child.clip, timeScale };
  }

  // Length (s) of one normalized unit of the current state: its picked clip's, or a caller's tree's at the parameters
  // (a caller's motion without a tree has none: only fixed durations may leave it).
  private stateLength(p: Params): number {
    const m = this.c.states[this.state].motion;
    if (typeof m === 'object' && 'scene' in m && m.tree) return treeLength(m.tree, p, this.c) / (this.c.states[this.state].speed ?? 1);
    return this.length;
  }
}

function holds([param, mode, threshold]: Condition, p: Params): boolean {
  const v = p[param] ?? false;
  switch (mode) {
    case 'if': return v === true;
    case 'ifNot': return v !== true;
    case 'greater': return Number(v) > threshold!;
    case 'less': return Number(v) < threshold!;
  }
}

// Index of the nearest of the sorted values to x, a tie going to the higher one.
function nearest(values: number[], x: number): number {
  let i = 0;
  while (i < values.length - 1 && x >= (values[i] + values[i + 1]) / 2) i++;
  return i;
}

function dominant(t: Tree, p: Params): Child {
  const axis = (k: 0 | 1) => [...new Set(t.children.map((c) => (typeof c.at === 'number' ? [c.at, 0] : c.at)[k]))].sort((a, b) => a - b);
  const xs = axis(0), ys = axis(1);
  const x = xs[nearest(xs, Number(p[t.param] ?? 0))];
  const y = ys[nearest(ys, Number((t.param2 && p[t.param2]) ?? 0))];
  return t.children.find((c) => (typeof c.at === 'number' ? c.at === x && y === 0 : c.at[0] === x && c.at[1] === y))!;
}

// Length (s) of a 1D tree at the parameters: its two nearest children's lengths weighted by the blend between them.
function treeLength(t: Tree, p: Params, c: AnimatorController): number {
  const ch = t.children as (Child & { at: number })[];
  const x = Number(p[t.param] ?? 0);
  const len = (k: Child) => c.clips[k.clip].length / (k.timeScale ?? 1);
  if (x <= ch[0].at) return len(ch[0]);
  for (let i = 0; i < ch.length - 1; i++) {
    if (x <= ch[i + 1].at) {
      const f = (x - ch[i].at) / (ch[i + 1].at - ch[i].at);
      return (1 - f) * len(ch[i]) + f * len(ch[i + 1]);
    }
  }
  return len(ch[ch.length - 1]);
}

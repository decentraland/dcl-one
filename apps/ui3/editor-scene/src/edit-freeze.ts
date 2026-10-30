
import { cmd } from './cmd'
import { state, type Snapshot } from './state'

const ANIMATOR = 'Animator'
const VIDEO_PLAYER = 'VideoPlayer'

type Json = Record<string, unknown>

let inPlay = false

const authored = new Map<string, unknown>()

const applied = new Map<string, string>()

function clone<T>(v: T): T {
  return JSON.parse(JSON.stringify(v)) as T
}

function splitKey(key: string): [string, string] {
  const i = key.indexOf('/')
  return [key.slice(0, i), key.slice(i + 1)]
}

function authoredValue(key: string, snapshotValue: unknown): unknown {
  if (!authored.has(key)) authored.set(key, clone(snapshotValue))
  const edited = state.editorValues.get(key)
  if (edited !== undefined && JSON.stringify(edited) !== JSON.stringify(authored.get(key))) {
    authored.set(key, clone(edited))
  }
  return authored.get(key)
}

function currentAuthored(key: string): unknown {
  const edited = state.editorValues.get(key)
  return edited !== undefined ? edited : authored.get(key)
}

function animatorIsPlaying(v: unknown): boolean {
  const states = (v as { states?: unknown } | undefined)?.states
  if (!Array.isArray(states)) return false
  return states.some((s) => (s as { playing?: unknown })?.playing !== false)
}

function freezeAnimator(v: unknown): unknown {
  const c = clone(v) as { states?: Json[] }
  if (Array.isArray(c.states)) {
    c.states = c.states.map((s) => ({ ...s, speed: 0, shouldReset: true }))
  }
  return c
}

function videoIsPlaying(v: unknown): boolean {
  return (v as { playing?: unknown } | undefined)?.playing !== false
}

function freezeVideo(v: unknown): unknown {
  return { ...(clone(v) as Json), playing: false }
}

async function freezeOne(
  id: string,
  name: string,
  snapValue: unknown,
  isPlaying: (v: unknown) => boolean,
  makeFrozen: (v: unknown) => unknown
): Promise<void> {
  if (snapValue === undefined) return
  const key = `${id}/${name}`
  const auth = authoredValue(key, snapValue)
  const authJson = JSON.stringify(auth)
  if (applied.get(key) === authJson) return
  if (!isPlaying(auth)) {
    applied.delete(key)
    return
  }
  await cmd.setComponent(id, name, JSON.stringify(makeFrozen(auth)))
  applied.set(key, authJson)
}

export async function freezeAnimatables(): Promise<void> {
  if (inPlay) return
  await cmd.freezeView()
  const snap = state.snapshot
  for (const id of Object.keys(snap)) {
    const comps = snap[id]
    if (comps === undefined) continue
    await freezeOne(id, ANIMATOR, comps[ANIMATOR], animatorIsPlaying, freezeAnimator)
    await freezeOne(id, VIDEO_PLAYER, comps[VIDEO_PLAYER], videoIsPlaying, freezeVideo)
  }
}

export function reassertAuthoredIntoSnapshot(snapshot: Snapshot): void {
  if (inPlay) return
  for (const key of applied.keys()) {
    const [id, name] = splitKey(key)
    const comps = snapshot[id]
    if (comps === undefined || comps[name] === undefined) continue
    const auth = currentAuthored(key)
    if (auth !== undefined) comps[name] = clone(auth)
  }
}

async function restoreAll(): Promise<void> {
  const writes: Promise<unknown>[] = []
  for (const key of applied.keys()) {
    const [id, name] = splitKey(key)
    const auth = currentAuthored(key)
    if (auth === undefined) continue
    writes.push(cmd.setComponent(id, name, JSON.stringify(auth)))
  }
  await Promise.all(writes)
  applied.clear()
}

export async function setPlayMode(playing: boolean, paused = false): Promise<void> {
  if (playing && !paused) await cmd.unfreezeView()
  else await cmd.freezeView()
  if (playing && !inPlay) {
    await restoreAll()
    inPlay = true
  } else if (!playing && inPlay) {
    inPlay = false
    await freezeAnimatables()
  }
}

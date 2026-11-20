// Grammar, shared with the /goto chat command: "random" | "crowd" | "x,y" | "x,y/spawn" | "world" |
// "world/x,y" | "world/spawn" | "world/x,y/spawn"; a spawn point name holds neither ',' nor '/'.
// Anything else is taken verbatim as a world name. The lobby cannot travel to a spawn point, so
// isDestination refuses the forms that name one.

export interface GotoTarget {
  world: string | null
  parcel: { x: number; y: number } | null
  spawnPoint: string | null
  random: boolean
  crowd: boolean
}

const target = (t: Partial<GotoTarget>): GotoTarget => ({ world: null, parcel: null, spawnPoint: null, random: false, crowd: false, ...t })

// an optionally signed integer; surrounding whitespace is allowed
const INTEGER = /^\s*[+-]?\d+\s*$/

function position(text: string): { x: number; y: number } | null {
  const coords = text.split(',')
  if (coords.length !== 2 || !INTEGER.test(coords[0]) || !INTEGER.test(coords[1])) return null
  return { x: parseInt(coords[0], 10), y: parseInt(coords[1], 10) }
}

const isSpawnPointName = (segment: string): boolean => segment.length > 0 && !segment.includes(',') && !segment.includes('/')

export function parseGotoTarget(text: string): GotoTarget {
  if (text === 'random') return target({ random: true })
  if (text === 'crowd') return target({ crowd: true })
  const parcel = position(text)
  if (parcel != null) return target({ parcel })
  const segments = text.split('/')
  if (segments.length === 2 && segments[0].length > 0) {
    const [head, tail] = segments
    const headParcel = position(head)
    if (headParcel != null && isSpawnPointName(tail)) return target({ parcel: headParcel, spawnPoint: tail })
    const tailParcel = position(tail)
    if (tailParcel != null) return target({ world: head, parcel: tailParcel })
    if (isSpawnPointName(tail)) return target({ world: head, spawnPoint: tail })
  }
  if (segments.length === 3 && segments[0].length > 0) {
    const p = position(segments[1])
    if (p != null && isSpawnPointName(segments[2])) return target({ world: segments[0], parcel: p, spawnPoint: segments[2] })
  }
  return target({ world: text })
}

export const WORLD_SUFFIX = '.dcl.eth'
// name.dcl.eth (a NAME: letters and digits) or an ENS name.eth, the names worldName keeps
const WORLD = /^(?:[A-Za-z0-9]+\.dcl|[A-Za-z0-9](?:[A-Za-z0-9-]*[A-Za-z0-9])?)\.eth$/i
const WORLD_SHORT_NAME = /^[A-Za-z0-9]+$/

/** Genesis City's parcels: the square city and its two extensions, as `[minX, minY, maxX, maxY)`. */
const GENESIS_BOUNDS: ReadonlyArray<readonly [number, number, number, number]> = [
  [-150, -150, 151, 151],
  [62, 151, 163, 159],
  [151, 59, 164, 151]
]
/** The worlds server's bound on a deployment's parcels. */
export const WORLD_PARCEL_LIMIT = 150

/** A world's name as `worldName` completes it: `name`, `name.dcl.eth` or an ENS `name.eth`. */
export function isWorldName(name: string): boolean {
  return WORLD.test(name) || WORLD_SHORT_NAME.test(name)
}

/** The world a realm is, lowercased, from the realm's own name (`name.dcl.eth`) or its address on
 *  a worlds server (`…/world/name.dcl.eth`); null for a realm that is no world. A short name is
 *  not enough here: other realms have one too. */
export function worldOfRealm(realm: string): string | null {
  const name = /\/world\/([^/?#]+)\/?$/.exec(realm)?.[1] ?? realm
  return WORLD.test(name) ? name.toLowerCase() : null
}

export function isGenesisParcel(x: number, y: number): boolean {
  return GENESIS_BOUNDS.some(([minX, minY, maxX, maxY]) => x >= minX && x < maxX && y >= minY && y < maxY)
}

export function isDestination(text: string): boolean {
  const t = parseGotoTarget(text)
  if (t.random || t.crowd) return true
  if (t.spawnPoint != null) return false
  if (t.world != null && !isWorldName(t.world)) return false
  if (t.parcel != null) {
    const { x, y } = t.parcel
    const inside = t.world != null ? Math.abs(x) <= WORLD_PARCEL_LIMIT && Math.abs(y) <= WORLD_PARCEL_LIMIT : isGenesisParcel(x, y)
    if (!inside) return false
  }
  return t.world != null || t.parcel != null
}

/** Lowercased: world names, like ENS names, are case-insensitive. */
export function worldName(name: string): string {
  const lower = name.toLowerCase()
  return lower.endsWith('.eth') ? lower : lower + WORLD_SUFFIX
}

export function randomParcel(random: () => number = Math.random): { x: number; y: number } {
  const pick = (): number => -150 + Math.floor(random() * 300)
  return { x: pick(), y: pick() }
}

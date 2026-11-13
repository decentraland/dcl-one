// The lobby's Go to field: what a typed destination means, as the /goto chat command reads it, and
// whether it names a real place. Grammar: "random" | "crowd" | "x,y" | "x,y/spawn" | "world" |
// "world/x,y" | "world/spawn" | "world/x,y/spawn"; a spawn point name holds neither ',' nor '/'.
// Anything else is taken verbatim as a world name.

export interface GotoTarget {
  world: string | null
  parcel: { x: number; y: number } | null
  spawnPoint: string | null
  random: boolean
  crowd: boolean
}

const target = (t: Partial<GotoTarget>): GotoTarget => ({ world: null, parcel: null, spawnPoint: null, random: false, crowd: false, ...t })

// an integer with optional surrounding whitespace, as .NET's int.TryParse takes it
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
const WORLD = /^[A-Za-z0-9]+\.dcl\.eth$/
const WORLD_SHORT_NAME = /^[A-Za-z0-9]+$/

/** Genesis City's parcels: the square city and its two extensions, each one parcel wider on every side. */
const GENESIS_BOUNDS: ReadonlyArray<readonly [number, number, number, number]> = [
  [-151, -151, 151, 151],
  [61, 150, 163, 159],
  [150, 58, 164, 151]
]

export function isGenesisParcel(x: number, y: number): boolean {
  return GENESIS_BOUNDS.some(([minX, minY, maxX, maxY]) => x >= minX && x < maxX && y >= minY && y < maxY)
}

/** Whether the text names a place the field accepts: random, crowd, a Genesis parcel or a world name. */
export function isDestination(text: string): boolean {
  const t = parseGotoTarget(text)
  if (t.random || t.crowd) return true
  if (t.world != null && !WORLD.test(t.world) && !WORLD_SHORT_NAME.test(t.world)) return false
  if (t.parcel != null && !isGenesisParcel(t.parcel.x, t.parcel.y)) return false
  return t.world != null || t.parcel != null
}

/** A short name is a .dcl.eth world; a full ENS name stands as typed. */
export function worldName(name: string): string {
  return name.toLowerCase().endsWith('.eth') ? name : name + WORLD_SUFFIX
}

/** A parcel of the square city at random, where "random" goes. */
export function randomParcel(random: () => number = Math.random): { x: number; y: number } {
  const pick = (): number => -150 + Math.floor(random() * 300)
  return { x: pick(), y: pick() }
}

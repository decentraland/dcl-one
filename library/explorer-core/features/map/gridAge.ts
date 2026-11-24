// How old the parcel map is, for the label the Explorer's maps show over it ("Updated 12 min ago").
//
// The map API's `/v2/tiles/info` answers `checkedAt`: when the server last found its parcel grid
// current, in ms since the epoch. A server that cannot rebuild the grid (a source not ready) keeps
// serving the one it has, and `checkedAt` stops moving, so the label shows how long ago that was.
// An answer without `checkedAt` (atlas-server, or a server from before the field) shows no label:
// its `lastUpdatedAt` is when the land last changed, which for a current map is often hours ago.
//
// `checkedAt` is the server's clock, so the age is measured on it: the answer's `Date` header
// gives the server's time, and the difference from this device's clock (`skew`) corrects a device
// clock that runs ahead or behind.

/** A grid found current within this long shows no label. */
export const MAP_GRID_STALE_AFTER_MS = 5 * 60_000
/** How often a shown map reads the grid's age again. */
export const MAP_GRID_AGE_POLL_MS = 60_000

/** What the map API answered about its grid's age. */
export interface MapGridAge {
  /** When the server last found its grid current (ms since the epoch, its clock). */
  checkedAt: number
  /** The server's clock minus this device's when the answer arrived (ms); 0 without a `Date`. */
  skew: number
}

/** The age answered from fixed data instead of the network; `undefined` reads the network. */
let fixedAge: MapGridAge | null | undefined

/**
 * Answer [`fetchMapGridAge`] from `age` instead of the network (the HUD's mock mode, set with the
 * map's other unsigned reads by `fixMapPublicReads`); `undefined` reads the network again. This
 * module imports nothing, so the Shop's LAND pages take only it, not the map's reads.
 */
export function fixMapGridAge(age: MapGridAge | null | undefined): void {
  fixedAge = age
}

/** The `checkedAt` of a `/v2/tiles/info` answer, or null when it has none. */
export function gridCheckedAt(body: unknown): number | null {
  if (typeof body !== 'object' || body === null) return null
  const at = (body as { checkedAt?: unknown }).checkedAt
  return typeof at === 'number' && Number.isFinite(at) && at > 0 ? at : null
}

/**
 * The label for a grid last found current at `checkedAt`, at `now` on the same clock, or null
 * while that is recent or unknown.
 */
export function mapGridAgeLabel(checkedAt: number | null, now: number): string | null {
  if (checkedAt == null) return null
  const age = now - checkedAt
  if (!(age > MAP_GRID_STALE_AFTER_MS)) return null
  const minutes = Math.floor(age / 60_000)
  if (minutes < 60) return `Updated ${minutes} min ago`
  const hours = Math.floor(minutes / 60)
  if (hours < 24) return `Updated ${hours} h ago`
  const days = Math.floor(hours / 24)
  return `Updated ${days} ${days === 1 ? 'day' : 'days'} ago`
}

/**
 * The grid's age from the map API at `base`: null when the API answers without `checkedAt` or with
 * an error (a grid it cannot serve, whose map shows nothing to label). A failed request throws. In
 * the HUD's mock mode (`fixMapPublicReads`) it answers the fixed age and reads nothing.
 */
export async function fetchMapGridAge(base: string, signal?: AbortSignal): Promise<MapGridAge | null> {
  if (fixedAge !== undefined) return fixedAge
  const res = await fetch(`${base.replace(/\/$/, '')}/v2/tiles/info`, { signal })
  if (!res.ok) {
    await res.body?.cancel()
    return null
  }
  const checkedAt = gridCheckedAt(await res.json())
  if (checkedAt == null) return null
  const serverNow = Date.parse(res.headers?.get('date') ?? '')
  return { checkedAt, skew: Number.isFinite(serverNow) ? serverNow - Date.now() : 0 }
}

// How old the parcel map is, for the label the Explorer's maps show over it ("Updated 12 min ago").
//
// The map API's `/v2/tiles/info` answers `checkedAt`: when the server last found its parcel grid
// current, in ms since the epoch. A server that cannot rebuild the grid (a source not ready) keeps
// serving the one it has, and `checkedAt` stops moving, so the label shows how long ago that was.
// An answer without `checkedAt` (atlas-server, or a server from before the field) shows no label:
// its `lastUpdatedAt` is when the land last changed, which for a current map is often hours ago.

/** A grid found current within this long shows no label. */
export const MAP_GRID_STALE_AFTER_MS = 5 * 60_000
/** How often a shown map reads the grid's age again. */
export const MAP_GRID_AGE_POLL_MS = 60_000

/** The `checkedAt` of a `/v2/tiles/info` answer, or null when it has none. */
export function gridCheckedAt(body: unknown): number | null {
  if (typeof body !== 'object' || body === null) return null
  const at = (body as { checkedAt?: unknown }).checkedAt
  return typeof at === 'number' && Number.isFinite(at) && at > 0 ? at : null
}

/** The label for a grid last found current at `checkedAt`, or null while that is recent or unknown. */
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
 * The grid's `checkedAt` from the map API at `base`: null when the API answers without one or with
 * an error (a grid it cannot serve, whose map shows nothing to label). A failed request throws.
 */
export async function fetchMapGridCheckedAt(base: string, signal?: AbortSignal): Promise<number | null> {
  const res = await fetch(`${base.replace(/\/$/, '')}/v2/tiles/info`, { signal })
  if (!res.ok) {
    await res.body?.cancel()
    return null
  }
  return gridCheckedAt(await res.json())
}

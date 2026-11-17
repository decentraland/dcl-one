// An item's own entity on the catalyst (`/content/entities/active`): its description and, for a
// wearable, the categories it hides or replaces when worn. The marketplace rows carry neither.

import { queryOptions, useQuery } from '@tanstack/react-query'
import { serviceUrl } from '@hud-host'

export interface ItemEntity {
  description: string
  /** Categories the wearable hides or replaces, once each. */
  hides: string[]
}

type EntityRow = { metadata?: { description?: unknown; data?: { hides?: unknown; replaces?: unknown } } }

/** The strings of a metadata list, which the catalyst serves as the deployer wrote it. */
const strings = (list: unknown): string[] => (Array.isArray(list) ? list.filter((h): h is string => typeof h === 'string') : [])

export async function fetchItemEntity(urn: string, signal?: AbortSignal): Promise<ItemEntity> {
  const res = await fetch(`${serviceUrl('catalyst')}/content/entities/active`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ pointers: [urn] }),
    signal
  })
  if (!res.ok) {
    // An unread body holds the connection open.
    await res.body?.cancel()
    throw new Error(`The catalyst returned ${res.status}`)
  }
  const rows = (await res.json()) as EntityRow[]
  const meta = Array.isArray(rows) ? rows[0]?.metadata : undefined
  const data = meta?.data
  return { description: typeof meta?.description === 'string' ? meta.description.trim() : '', hides: [...new Set([...strings(data?.hides), ...strings(data?.replaces)])] }
}

export const itemEntityQuery = (urn: string) =>
  queryOptions({
    queryKey: ['shop', 'item-entity', urn] as const,
    queryFn: ({ signal }) => fetchItemEntity(urn, signal),
    staleTime: 5 * 60_000
  })

/** The selected item's entity; nothing is asked for while no URN is known. */
export function useItemEntity(urn: string | null) {
  return useQuery({ ...itemEntityQuery(urn ?? ''), enabled: urn != null })
}

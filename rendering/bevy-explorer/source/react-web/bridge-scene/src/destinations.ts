import { signed } from './http'

export type DestinationRaw = { id: string; title?: string | null; world_name?: string | null; base_position?: string | null; owner?: string | null }

// The places service refuses more ids than this in one request.
const PAGE = 100

/** The places' details by id, a page at a time. A place the service does not return (a disabled
 *  one, say) is left out, and so is a page answered with no body. */
export async function destinationsByIds(destinations: string, ids: string[]): Promise<DestinationRaw[]> {
  const rows: DestinationRaw[] = []
  for (let i = 0; i < ids.length; i += PAGE) rows.push(...((await signed<DestinationRaw[]>(destinations, 'POST', ids.slice(i, i + PAGE))) ?? []))
  return rows
}

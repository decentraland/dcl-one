import { z } from 'zod'
import { CatalystError, getJSON } from '../catalyst/client'

const slotCount = z.string().regex(/^(0|[1-9]\d*)$/).transform(Number).pipe(z.number().int().nonnegative())
const providerSlots = z.object({
  id: z.string(),
  maxItems: slotCount,
  availableSlots: slotCount
})

export type IndexedThirdPartySlots = Pick<z.infer<typeof providerSlots>, 'maxItems' | 'availableSlots'>

/** On-chain capacity for one Polygon provider. Builder tracks review reservations separately. */
export async function fetchIndexedThirdPartySlots(thirdPartyId: string): Promise<IndexedThirdPartySlots | undefined> {
  const path = `/lambdas/third-party-integrations/providers/${encodeURIComponent(thirdPartyId)}`
  const signal = AbortSignal.timeout(2_500)
  try {
    const provider = providerSlots.parse(await getJSON<unknown>(path, { signal, cache: 'no-store' }))
    if (provider.id !== thirdPartyId || provider.availableSlots > provider.maxItems) {
      throw new Error('Catalyst returned inconsistent third-party slot data')
    }
    return { maxItems: provider.maxItems, availableSlots: provider.availableSlots }
  } catch (error) {
    // A recently registered provider may be missing while the index catches up.
    if (signal.aborted || (error instanceof CatalystError && [0, 404, 502, 503, 504].includes(error.status))) {
      return undefined
    }
    throw error
  }
}

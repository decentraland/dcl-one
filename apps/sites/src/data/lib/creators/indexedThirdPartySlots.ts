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
    const result = providerSlots.safeParse(await getJSON<unknown>(path, { signal, cache: 'no-store' }))
    if (!result.success) return undefined

    const { id, maxItems, availableSlots } = result.data
    if (id !== thirdPartyId || availableSlots > maxItems) return undefined

    return { maxItems, availableSlots }
  } catch (error) {
    // Callers also read the live registry, so Catalyst availability does not gate publishing.
    if (signal.aborted || error instanceof CatalystError) return undefined
    throw error
  }
}

import { afterEach, describe, expect, it, vi } from 'vitest'
import { homeQueries, type OutfitSummary } from '../features/shop/homeQuery'
import { fetchItemEntity } from '../features/shop/itemEntity'

afterEach(() => {
  vi.unstubAllGlobals()
})

describe('the item entity', () => {
  it('cancels the body of a failed answer before giving up', async () => {
    const res = new Response('down', { status: 503 })
    const cancel = vi.spyOn(res.body!, 'cancel')
    vi.stubGlobal('fetch', vi.fn(async () => res))
    await expect(fetchItemEntity('urn:x')).rejects.toThrow('503')
    expect(cancel).toHaveBeenCalled()
  })

  it('keeps only the strings of what the deployer listed', async () => {
    vi.stubGlobal('fetch', vi.fn(async () => Response.json([{ metadata: { description: 7, data: { hides: ['hat', 3, null], replaces: ['mask', {}, 'hat'] } } }])))
    expect(await fetchItemEntity('urn:x')).toEqual({ description: '', hides: ['hat', 'mask'] })
  })
})

describe('an outfit', () => {
  it('is read again when it lists other items under the same id', () => {
    const outfit = (itemId: string): OutfitSummary =>
      ({ id: 'o1', name: 'Neon Night', items: [{ contractAddress: '0xAbC', itemId }] }) as unknown as OutfitSummary
    expect(homeQueries.outfitItems(outfit('1')).queryKey).not.toEqual(homeQueries.outfitItems(outfit('2')).queryKey)
    expect(homeQueries.outfitItems(outfit('1')).queryKey).toContainEqual(['0xabc-1'])
  })
})

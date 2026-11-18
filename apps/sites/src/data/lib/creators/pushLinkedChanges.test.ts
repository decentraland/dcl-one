import { describe, expect, it, vi } from 'vitest'
import { CurationStatus } from './collections'
import { ItemType, type Item } from './items'
import { type ItemCuration } from './linkedItemStatus'
import { pushLinkedChanges, type PushLinkedDeps } from './pushLinkedChanges'

function makeItem(id: string): Item {
  return {
    id,
    name: `Item ${id}`,
    description: '',
    thumbnail: 'thumbnail.png',
    owner: '0xowner',
    collectionId: 'c1',
    isPublished: true,
    isApproved: true,
    inCatalyst: true,
    currentContentHash: `hash-${id}`,
    type: ItemType.WEARABLE,
    data: { category: 'hat', representations: [] },
    contents: {},
    createdAt: 1,
    updatedAt: 1
  }
}

function curation(itemId: string, status: CurationStatus): ItemCuration {
  return { id: `ic-${itemId}`, itemId, status, contentHash: 'old', isMappingComplete: true, createdAt: 1, updatedAt: 1 }
}

function makeDeps(overrides: Partial<PushLinkedDeps> = {}): PushLinkedDeps {
  return {
    pushItemCuration: vi.fn(async (itemId: string) => curation(itemId, CurationStatus.PENDING)),
    updateItemCuration: vi.fn(async (itemId: string) => curation(itemId, CurationStatus.PENDING)),
    ...overrides
  }
}

describe('pushLinkedChanges', () => {
  it('refreshes pending requests and opens new ones for approved or rejected items', async () => {
    const items = [makeItem('a'), makeItem('b'), makeItem('c'), makeItem('d')]
    const curations = new Map([
      ['a', curation('a', CurationStatus.PENDING)],
      ['b', curation('b', CurationStatus.APPROVED)],
      ['c', curation('c', CurationStatus.REJECTED)]
    ])
    const deps = makeDeps()

    const result = await pushLinkedChanges(items, curations, deps)

    expect(result.pushed.map(item => item.id)).toEqual(['a', 'b', 'c', 'd'])
    expect(result.failed).toEqual([])
    expect(deps.updateItemCuration).toHaveBeenCalledTimes(1)
    expect(deps.updateItemCuration).toHaveBeenCalledWith('a', 'pending')
    expect(vi.mocked(deps.pushItemCuration).mock.calls.map(([id]) => id)).toEqual(['b', 'c', 'd'])
  })

  it('reports progress after every item, up to the total', async () => {
    const items = [makeItem('a'), makeItem('b'), makeItem('c')]
    const onProgress = vi.fn()

    await pushLinkedChanges(items, new Map(), makeDeps(), onProgress)

    expect(onProgress.mock.calls).toEqual([
      [1, 3],
      [2, 3],
      [3, 3]
    ])
  })

  it('keeps going when one item fails and reports it with its message', async () => {
    const items = [makeItem('a'), makeItem('b'), makeItem('c')]
    const deps = makeDeps({
      pushItemCuration: vi.fn(async (itemId: string) => {
        if (itemId === 'b') throw new Error('Mapping is not complete')
        return curation(itemId, CurationStatus.PENDING)
      })
    })

    const result = await pushLinkedChanges(items, new Map(), deps)

    expect(result.pushed.map(item => item.id)).toEqual(['a', 'c'])
    expect(result.failed).toEqual([{ item: items[1], message: 'Mapping is not complete' }])
  })

  it('never runs more requests at once than the concurrency limit', async () => {
    const items = Array.from({ length: 10 }, (_, index) => makeItem(String(index)))
    let inFlight = 0
    let maxInFlight = 0
    const deps = makeDeps({
      pushItemCuration: vi.fn(async (itemId: string) => {
        inFlight++
        maxInFlight = Math.max(maxInFlight, inFlight)
        await new Promise(resolve => setTimeout(resolve, 1))
        inFlight--
        return curation(itemId, CurationStatus.PENDING)
      })
    })

    const result = await pushLinkedChanges(items, new Map(), deps, undefined, 3)

    expect(maxInFlight).toBe(3)
    expect(result.pushed).toHaveLength(10)
    expect(result.pushed.map(item => item.id)).toEqual(items.map(item => item.id))
  })

  it('answers empty results for no items', async () => {
    const onProgress = vi.fn()
    const result = await pushLinkedChanges([], new Map(), makeDeps(), onProgress)
    expect(result).toEqual({ pushed: [], failed: [] })
    expect(onProgress).not.toHaveBeenCalled()
  })
})

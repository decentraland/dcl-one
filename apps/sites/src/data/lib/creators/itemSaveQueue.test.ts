import { describe, expect, it, vi } from 'vitest'
import { type Item } from './items'
import { createItemSaveQueue } from './itemSaveQueue'

const item = { id: 'i1', name: 'Cap', urn: 'urn:old', mappings: null, updatedAt: 1 } as unknown as Item

function server() {
  const saves: Item[] = []
  let version = 1
  const release: Array<() => void> = []
  const save = (sent: Item) =>
    new Promise<Item>(resolve => {
      release.push(() => {
        saves.push(sent)
        resolve({ ...sent, updatedAt: ++version })
      })
    })
  return { saves, save, release }
}

describe('createItemSaveQueue', () => {
  it('builds a later save of the same item on the earlier one, so neither undoes the other', async () => {
    const { saves, save, release } = server()
    const savePatch = createItemSaveQueue(save)

    const mapping = savePatch(item, { mappings: { amoy: {} } })
    // Built from the list's stale copy, before the mapping save came back.
    const urn = savePatch(item, { urn: 'urn:new' })
    await vi.waitFor(() => expect(release).toHaveLength(1))
    release.shift()!()
    await mapping
    await vi.waitFor(() => expect(release).toHaveLength(1))
    release.shift()!()
    await urn

    expect(saves.map(sent => [sent.urn, sent.mappings])).toEqual([
      ['urn:old', { amoy: {} }],
      ['urn:new', { amoy: {} }]
    ])
  })

  it('goes on after a failed save', async () => {
    const savePatch = createItemSaveQueue(async sent => {
      if (sent.name === 'boom') throw new Error('offline')
      return sent
    })
    await expect(savePatch(item, { name: 'boom' })).rejects.toThrow('offline')
    await expect(savePatch(item, { name: 'Cap 2' })).resolves.toMatchObject({ name: 'Cap 2' })
  })
})

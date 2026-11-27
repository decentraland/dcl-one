import { describe, it, expect, vi, beforeEach } from 'vitest'

const getJson = vi.fn<(url: string) => Promise<unknown>>()
vi.mock('../../bridge-scene/src/http', () => ({
  catalystBase: async () => 'https://catalyst.example',
  getJson: (url: string) => getJson(url)
}))
vi.mock('../../bridge-scene/src/domains/marketplace', () => ({ resolveShopUrls: async () => new Map() }))
vi.mock('../../bridge-scene/src/domains/avatarDraft', () => ({ currentLook: () => null, editLook: () => {} }))
vi.mock('../../bridge-scene/src/identity', () => ({ identity: { onChange: () => {} } }))

const def = (id: string, name: string) => ({ id, name, rarity: 'epic', data: { category: 'hat' } })
const answer = (...defs: unknown[]) => ({ wearables: defs })
const down = () => Promise.reject(new Error('lambdas down'))

// Each case gets a fresh module, so the session cache of definitions starts empty.
async function load() {
  vi.resetModules()
  return await import('../../bridge-scene/src/domains/wearables')
}

describe('resolveWearables when the lambdas request fails', () => {
  beforeEach(() => {
    getJson.mockReset()
    vi.spyOn(console, 'error').mockImplementation(() => {})
  })

  it('rejects a shown list when every lookup failed', async () => {
    const { resolveWearables } = await load()
    getJson.mockImplementationOnce(down)
    await expect(resolveWearables(['urn:a', 'urn:b'], { dropFailed: true })).rejects.toThrow()
  })

  it('leaves out only the items it could not ask about', async () => {
    const { resolveWearables } = await load()
    getJson.mockResolvedValueOnce(answer(def('urn:a', 'Hat')))
    await resolveWearables(['urn:a'])
    getJson.mockImplementationOnce(down)
    const list = await resolveWearables(['urn:a', 'urn:b'], { dropFailed: true })
    expect(list.map((w) => [w.urn, w.name])).toEqual([['urn:a', 'Hat']])
  })

  it('keeps an urn the lambdas answered without a definition', async () => {
    const { resolveWearables } = await load()
    getJson.mockResolvedValueOnce(answer(def('urn:a', 'Hat')))
    const list = await resolveWearables(['urn:a', 'urn:gone'], { dropFailed: true })
    expect(list.map((w) => [w.urn, w.category])).toEqual([['urn:a', 'hat'], ['urn:gone', 'unknown']])
  })

  it('keeps a successful lookup where every urn is unknown', async () => {
    const { resolveWearables } = await load()
    getJson.mockResolvedValueOnce(answer())
    const list = await resolveWearables(['urn:gone'], { dropFailed: true })
    expect(list.map((w) => w.category)).toEqual(['unknown'])
  })

  it('keeps every item of the equipped set, so the next equip still deploys it', async () => {
    const { resolveWearables, resolveEquippedSet } = await load()
    getJson.mockImplementation(down)
    expect((await resolveWearables(['urn:a'])).map((w) => w.urn)).toEqual(['urn:a'])
    expect((await resolveEquippedSet(['urn:a'])).map((w) => w.urn)).toEqual(['urn:a'])
  })
})

describe('fetchWearablesPage', () => {
  beforeEach(() => {
    getJson.mockReset()
  })

  const element = (urn: string, metadata: Record<string, unknown>) => ({
    urn,
    name: urn,
    rarity: 'epic',
    category: 'hat',
    entity: { metadata, content: [] }
  })

  it("carries each item's description from its entity", async () => {
    const { fetchWearablesPage } = await load()
    getJson.mockResolvedValueOnce({
      elements: [element('urn:a', { description: 'Keeps the sun off.' }), element('urn:b', {})],
      totalAmount: 2
    })
    const page = await fetchWearablesPage('0xabc', { page: 0, pageSize: 16 })
    expect(page.items.map((w) => [w.urn, w.description])).toEqual([['urn:a', 'Keeps the sun off.'], ['urn:b', undefined]])
  })
})

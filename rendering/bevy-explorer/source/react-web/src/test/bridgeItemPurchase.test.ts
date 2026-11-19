import { describe, it, expect, vi, beforeEach } from 'vitest'
import type { PageToScene, SceneToPage } from '@explorer-core/engine/protocol'
import type { ItemPurchaseRequest } from '@explorer-core/engine/generated'

const api = vi.hoisted(() => ({ getItemPurchaseRequestStream: vi.fn(), setItemPurchaseState: vi.fn() }))
const relay = vi.hoisted(() => vi.fn())
vi.mock('../../bridge-scene/src/bevy-api', () => ({ BevyApi: api }))
vi.mock('../../bridge-scene/src/system-helpers', () => ({ relay }))

const REQUEST: ItemPurchaseRequest = { id: 3, item: { contract: '0x' + 'ab'.repeat(20), itemId: '7' } }
const MESSAGE = { kind: 'itemPurchaseRequest', id: 3, item: REQUEST.item }

/** The domain, with the engine's stream (`engine`) and the page's messages (`page`) in the test's hands. */
async function domain(): Promise<{ sent: SceneToPage[]; engine: (request: ItemPurchaseRequest) => void; page: (msg: PageToScene) => void }> {
  const { registerItemPurchase } = await import('../../bridge-scene/src/domains/itemPurchase')
  const sent: SceneToPage[] = []
  const handlers = new Map<string, (msg: PageToScene) => void>()
  registerItemPurchase({ send: (msg) => sent.push(msg), on: (kind, handler) => handlers.set(kind, handler as never), push: vi.fn() })
  const engine = relay.mock.calls.at(-1)![2] as (request: ItemPurchaseRequest) => void
  return { sent, engine, page: (msg) => handlers.get(msg.kind)!(msg) }
}

describe('bridge scene: the item purchase domain', () => {
  beforeEach(() => {
    relay.mockClear()
    api.setItemPurchaseState.mockClear()
  })

  it("relays the engine's request to the page and the page's reports to the engine", async () => {
    const { sent, engine, page } = await domain()
    engine(REQUEST)
    expect(sent).toEqual([MESSAGE])
    page({ kind: 'itemPurchaseState', id: 3, state: 'accepted' })
    page({ kind: 'itemPurchaseState', id: 3, state: 'purchased' })
    expect(api.setItemPurchaseState.mock.calls).toEqual([[{ id: 3, state: 'accepted' }], [{ id: 3, state: 'purchased' }]])
  })

  it('sends a request the page never answered again when a page says hello', async () => {
    const { sent, engine, page } = await domain()
    engine(REQUEST)
    page({ kind: 'hello' } as PageToScene)
    expect(sent).toEqual([MESSAGE, MESSAGE])
    expect(api.setItemPurchaseState).not.toHaveBeenCalled()
  })

  it('reports an accepted request dismissed when a new page says hello, once', async () => {
    const { sent, engine, page } = await domain()
    engine(REQUEST)
    page({ kind: 'itemPurchaseState', id: 3, state: 'accepted' })
    page({ kind: 'hello' } as PageToScene)
    page({ kind: 'hello' } as PageToScene)
    expect(sent).toEqual([MESSAGE])
    expect(api.setItemPurchaseState.mock.calls).toEqual([[{ id: 3, state: 'accepted' }], [{ id: 3, state: 'dismissed' }]])
  })

  it.each(['unavailable', 'purchased', 'dismissed', 'failed'] as const)('keeps nothing once the page reported %s', async (state) => {
    const { sent, engine, page } = await domain()
    engine(REQUEST)
    page({ kind: 'itemPurchaseState', id: 3, state })
    page({ kind: 'hello' } as PageToScene)
    expect(sent).toEqual([MESSAGE])
    expect(api.setItemPurchaseState.mock.calls).toEqual([[{ id: 3, state }]])
  })
})

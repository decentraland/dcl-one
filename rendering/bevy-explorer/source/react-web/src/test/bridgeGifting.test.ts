import { describe, it, expect, vi, beforeEach } from 'vitest'
import type { Ctx } from '../../bridge-scene/src/bridge'
import type { PageToScene, SceneToPage } from '@explorer-core/engine/protocol'

const http = vi.hoisted(() => ({
  catalystBase: vi.fn(async () => 'https://peer.test'),
  getJson: vi.fn(async (_url: string): Promise<unknown> => undefined)
}))
const player = vi.hoisted(() => ({ current: { userId: '0xme' } as { userId: string } | null }))
const look = vi.hoisted(() => ({ current: { wearables: ['urn:decentraland:matic:collections-v2:0xabc:0:105'], emotes: ['', 'dance'] } as { wearables: string[]; emotes: string[] } | null }))
vi.mock('../../bridge-scene/src/http', () => http)
vi.mock('../../bridge-scene/src/domains/avatarDraft', () => ({ currentLook: () => look.current }))
vi.mock('../../bridge-scene/node_modules/@dcl/sdk/players/index.js', () => ({ getPlayer: () => player.current }))

const { registerGifting } = await import('../../bridge-scene/src/domains/gifting')

type Handler = (msg: PageToScene) => Promise<void>

function start(): { sent: SceneToPage[]; ask: (msg: PageToScene) => Promise<void> } {
  const sent: SceneToPage[] = []
  const handlers = new Map<string, Handler>()
  const ctx = { send: (m: SceneToPage) => sent.push(m), on: (kind: string, h: Handler) => handlers.set(kind, h), push: () => {} } as unknown as Ctx
  registerGifting(ctx)
  return { sent, ask: async (msg) => handlers.get(msg.kind)?.(msg) }
}

const ITEM = 'urn:decentraland:matic:collections-v2:0xabc:0'

beforeEach(() => {
  vi.clearAllMocks()
  player.current = { userId: '0xme' }
})

describe('bridge gifting', () => {
  it('reads one page of on-chain items, newest first, with each owned token', async () => {
    http.getJson.mockResolvedValueOnce({
      elements: [
        {
          urn: ITEM,
          name: 'Hat',
          rarity: 'epic',
          category: 'hat',
          amount: 2,
          individualData: [
            { id: `${ITEM}:105`, tokenId: '105', transferredAt: '1700000000' },
            { id: `${ITEM}:106`, tokenId: '106', transferredAt: 1700000500 },
            { id: 'broken', tokenId: 'x' }
          ],
          entity: { metadata: { thumbnail: 'thumb.png' }, content: [{ file: 'thumb.png', hash: 'Qm1' }] }
        }
      ],
      totalAmount: 21
    })
    const { sent, ask } = start()
    await ask({ kind: 'giftingQuery', catalog: 'wearables', page: 1, pageSize: 16, search: 'h a', requestId: 7 })
    const url = http.getJson.mock.calls[0]![0]
    expect(url).toBe(
      'https://peer.test/explorer/0xme/wearables?pageNum=2&pageSize=16&collectionType=on-chain&orderBy=date&direction=DESC&includeAmount=true&includeEntities=true&name=h%20a'
    )
    expect(sent).toEqual([
      {
        kind: 'giftingPage',
        catalog: 'wearables',
        items: [
          {
            urn: ITEM,
            name: 'Hat',
            rarity: 'epic',
            category: 'hat',
            thumbnail: 'https://peer.test/content/contents/Qm1',
            amount: 2,
            tokens: [
              { urn: `${ITEM}:105`, tokenId: '105', transferredAt: 1700000000 },
              { urn: `${ITEM}:106`, tokenId: '106', transferredAt: 1700000500 }
            ]
          }
        ],
        total: 21,
        equipped: [`${ITEM}:105`, 'dance'],
        failed: false,
        requestId: 7
      }
    ])
  })

  it('reads emotes from their own route', async () => {
    http.getJson.mockResolvedValueOnce({ elements: [], totalAmount: 0 })
    const { ask } = start()
    await ask({ kind: 'giftingQuery', catalog: 'emotes', page: 0, pageSize: 16, requestId: 1 })
    expect(http.getJson.mock.calls[0]![0]).toContain('/explorer/0xme/emotes?pageNum=1&pageSize=16&collectionType=on-chain')
  })

  it('answers a failed read as failed, with no items', async () => {
    http.getJson.mockRejectedValueOnce(new Error('offline'))
    const { sent, ask } = start()
    await ask({ kind: 'giftingQuery', catalog: 'wearables', page: 0, pageSize: 16, requestId: 2 })
    expect(sent[0]).toMatchObject({ kind: 'giftingPage', items: [], total: 0, failed: true, requestId: 2 })
  })
})

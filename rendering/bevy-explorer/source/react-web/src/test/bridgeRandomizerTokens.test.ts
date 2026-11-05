import { describe, it, expect, vi, afterEach } from 'vitest'
import type { AvatarLook } from '../engine/avatarEquip'
import type { PageToScene } from '../engine/protocol'
import type { Ctx } from '../../bridge-scene/src/bridge'

// The real wearables domain, so an owned pick goes through accumulateTokens and deployableUrn.
const MALE = 'urn:decentraland:off-chain:base-avatars:BaseMale'
const SHIRT = 'urn:decentraland:matic:collections-v2:0xshirts:0'
const MALE_ONLY = { metadata: { data: { representations: [{ bodyShapes: [MALE] }] } } }

vi.mock('../../bridge-scene/node_modules/@dcl/sdk/players/index.js', () => ({ getPlayer: () => ({ userId: '0xa' }) }))

let look: AvatarLook = { bodyShape: MALE, eyes: null, hair: null, skin: null, wearables: [], emotes: [], forceRender: [] }
vi.mock('../../bridge-scene/src/domains/avatarDraft', () => ({
  currentLook: () => look,
  editLook: (change: Partial<AvatarLook>) => {
    look = { ...look, ...change }
  }
}))

vi.mock('../../bridge-scene/src/http', () => ({
  catalystBase: async () => 'https://catalyst.test',
  getJson: async (url: string) => {
    if (url.includes('collectionType=base-wearable')) {
      return { elements: [{ urn: 'urn:decentraland:off-chain:base-avatars:tshirt', name: 'tshirt', rarity: 'base', category: 'upper_body', entity: MALE_ONLY }] }
    }
    if (url.includes('pageSize=1&')) return { elements: [], totalAmount: url.includes('category=upper_body') ? 1 : 0 }
    return {
      elements: [{ urn: SHIRT, name: 'shirt', rarity: 'rare', category: 'upper_body', individualData: [{ tokenId: '42' }], entity: MALE_ONLY }],
      totalAmount: 1
    }
  }
}))

vi.mock('../../bridge-scene/src/domains/collections', () => ({
  resolveDefsByUrn: async () => new Map(),
  thumbnailUrl: () => ''
}))

vi.mock('../../bridge-scene/src/domains/marketplace', () => ({ resolveShopUrls: async () => new Map() }))

afterEach(() => {
  vi.restoreAllMocks()
})

describe('bridge randomizer with the real wearables domain', () => {
  it('equips an owned pick by its token urn', async () => {
    vi.spyOn(Math, 'random').mockReturnValue(0)
    const { registerRandomizer } = await import('../../bridge-scene/src/domains/randomizer')
    const { identity } = await import('../../bridge-scene/src/identity')
    const handlers: Array<(m: PageToScene) => unknown> = []
    const ctx: Ctx = {
      send: () => {},
      on: (kind, h) => {
        if (kind === 'randomizeOutfit') handlers.push(h as (m: PageToScene) => unknown)
      },
      push: () => {}
    }
    registerRandomizer(ctx)
    identity.observe('0xa')

    await Promise.all(handlers.map((h) => h({ kind: 'randomizeOutfit' } as PageToScene)))

    expect(look.wearables).toContain(`${SHIRT}:42`)
    expect(look.wearables).not.toContain(SHIRT)
  })
})

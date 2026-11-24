// BUY CREDITS from the in-world Shop opens the site whose node holds the balance the checkout reads, even when the
// Explorer runs against another deployment (?baseDomain=): on a page Sites serves, the checkout reads this page's own
// node, so topping up anywhere else would fund a balance the checkout never sees.
import { afterEach, describe, expect, it, vi } from 'vitest'
import { batchAuthorization, createFakeChain, createFakeShop, createFakeWallet } from '../../shop/checkout/fakes'
import type { CatalogItem } from '@shop/discovery/lib/catalogItem'
import type { UsdCents } from '@shop/model/money'
import { BOOK, ITEM, storeMint } from '@shop/payment/fixtures'
import type { StoredLogin } from '@explorer-core/features/auth/sso'
import { SHOP_PROTOCOL } from '@dcl/one-ui/apps/shop/shopProtocol'
import type { ShellPurchaseHandlers, ShellPurchaseView } from '../shell/shop/purchase'

const BUYER = BOOK.buyer.toLowerCase()
const CONTRACT = BOOK.collection.toLowerCase()
const MANAGER = '0x' + 'de'.repeat(20)

const LOGIN: StoredLogin = {
  address: BUYER,
  identity: {
    ephemeralIdentity: { address: '0x' + '1'.repeat(40), publicKey: '0x04', privateKey: '0x' + 'ab'.repeat(32) },
    expiration: new Date(Date.now() + 3_600_000).toISOString(),
    authChain: [{ type: 'SIGNER', payload: BUYER, signature: '' }]
  }
}

const LISTING = {
  id: 'listing',
  name: 'Golden Armour',
  creator: BOOK.creator,
  contractAddress: CONTRACT,
  itemId: String(ITEM),
  chainId: 137,
  rarity: 'epic',
  category: 'wearable',
  wearableCategory: 'upper_body',
  available: 10
} as unknown as CatalogItem

function memoryStorage(): Storage {
  const map = new Map<string, string>()
  return {
    get length() {
      return map.size
    },
    clear: () => map.clear(),
    getItem: (k) => map.get(k) ?? null,
    key: (i) => [...map.keys()][i] ?? null,
    removeItem: (k) => void map.delete(k),
    setItem: (k, v) => void map.set(k, v)
  }
}

/** The shell's purchase with its live links, loaded on a page Sites serves at `search`, the balance short. */
async function shortOnSites(search: string) {
  vi.resetModules()
  window.__DCL_PUBLIC__ = {}
  window.history.replaceState(null, '', search)
  vi.stubEnv('VITE_SHOP_CHAIN_ID', '137')
  vi.stubEnv('VITE_SHOP_CREDITS_MANAGER', MANAGER)
  const { createShellShop } = await import('../shell/shop/purchase')
  const shop = createFakeShop({ authorizeCreditBatch: ({ body }) => batchAuthorization((body as { items: { usdPriceCents: number }[] }).items) })
  const shown: { view: ShellPurchaseView; handlers: ShellPurchaseHandlers }[] = []
  const openExternal = vi.fn()
  const shell = createShellShop({
    login: () => LOGIN,
    wallet: { kind: () => 'managed', provider: () => createFakeWallet({ account: BUYER }) },
    ui: { show: (view, handlers) => void shown.push({ view, handlers }), hide: () => {} },
    post: () => {},
    openExternal,
    signIn: vi.fn(),
    // Everything but the links, which stay the live ones under test.
    services: {
      client: () => shop.client,
      fetchItem: async () => LISTING,
      offers: () => async (queries, session) => Object.fromEntries(queries.map((q) => [q.key, storeMint(session)])),
      // One credit: short of the item's price.
      balance: async () => ({ balanceCents: 10 as UsdCents, credits: 1 }),
      resolvers: () => ({ trade: async () => null, store: async () => ({ priceWei: '2300000000000000000', available: 10 }) }),
      reader: () => createFakeChain(),
      primeFlags: async () => {},
      claim: async () => () => {},
      creatorName: async () => null,
      creatorFace: async () => null,
      manager: (chainId) => ({ chainId, address: MANAGER, name: 'Decentraland Credits', version: '1.0.0' }),
      storage: () => memoryStorage()
    }
  })
  await shell.request('shop.buy', { v: SHOP_PROTOCOL, items: [{ contract: CONTRACT, itemId: String(ITEM), quantity: 1 }], method: 'credits' })
  await vi.waitFor(() => expect(shown.at(-1)?.view.step).toBe('insufficient'), { timeout: 3000 })
  shown.at(-1)!.handlers.onBuyCredits?.()
  expect(openExternal).toHaveBeenCalledOnce()
  return new URL(openExternal.mock.calls[0][0] as string)
}

afterEach(() => {
  delete window.__DCL_PUBLIC__
  window.history.replaceState(null, '', '/')
  vi.unstubAllEnvs()
})

describe('BUY CREDITS from the in-world Shop', () => {
  it('opens this site’s Credits page while the Explorer runs against another deployment', async () => {
    const opened = await shortOnSites('/?baseDomain=decentraland.org')
    expect(opened.origin).toBe(window.location.origin)
    expect(opened.pathname).toBe('/shop/credits')
  })

  it('opens this site’s Credits page with no other deployment selected', async () => {
    const opened = await shortOnSites('/')
    expect(`${opened.origin}${opened.pathname}`).toBe(`${window.location.origin}/shop/credits`)
  })
})

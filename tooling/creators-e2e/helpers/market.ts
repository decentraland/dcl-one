import { StandardMerkleTree } from '@openzeppelin/merkle-tree'
import { type SentTransaction } from './rpc'

/** OffChainMarketplace V3 on Amoy: where every new trade is listed. */
export const MARKETPLACE_V3 = '0x36fd1434a6c4b8ade80c9847c1d15033ce34488c'
/** CouponManager for OffChainMarketplace V3 on Amoy: the manager new discounts are signed against. */
export const COUPON_MANAGER_V3 = '0x6c956587d9fe70032781edcdc626310648575382'
/** `received` asset type of a trade priced in USD-pegged MANA, which is what a credits price is. */
const USD_PEGGED_MANA = 2

/** A primary listing as `GET /v1/items` reports it. */
export type MarketItem = {
  itemId: string
  contractAddress: string
  price: string
  isOnSale: boolean
  tradeId: string | null
  tradeContractAddress: string | null
}

/** A primary listing as `GET /v3/catalog/unified` reports it. */
export type CatalogRow = {
  itemId: string
  contractAddress: string
  priceCredits: number | null
  source: 'native' | 'legacy'
  tradeId: string | null
}

export type MarketSeed = { items?: MarketItem[]; catalog?: CatalogRow[]; coupons?: unknown[] }

export type MarketResponse = { status: number; body: unknown }

const ok = (data: unknown): MarketResponse => ({ status: 200, body: { ok: true, data } })

/** An in-memory marketplace-server: listings, trades and coupons, so a listing created in a spec shows up. */
export function createMarket(seed: MarketSeed = {}) {
  const items = [...(seed.items ?? [])]
  const catalog = [...(seed.catalog ?? [])]
  const trades = new Map<string, Record<string, unknown>>()
  let tradeCount = 0
  const coupons = [...(seed.coupons ?? [])] as Array<Record<string, unknown>>

  const byContract = <T extends { contractAddress: string; itemId: string }>(rows: T[], query: URLSearchParams) =>
    rows.filter(
      row =>
        (!query.get('contractAddress') ||
          row.contractAddress.toLowerCase() === query.get('contractAddress')?.toLowerCase()) &&
        (!query.get('itemId') || row.itemId === query.get('itemId'))
    )

  function handle(method: string, path: string, query: URLSearchParams, body: unknown): MarketResponse | null {
    if (method === 'GET' && path === '/v1/items') return { status: 200, body: { data: byContract(items, query) } }
    if (method === 'GET' && path === '/v3/catalog/unified') {
      return { status: 200, body: { data: byContract(catalog, query) } }
    }
    if (method === 'GET' && path === '/v1/coupons') {
      const signer = query.get('signer')?.toLowerCase()
      return { status: 200, body: { data: coupons.filter(c => !signer || String(c.signer).toLowerCase() === signer) } }
    }
    if (method === 'POST' && path === '/v1/coupons') {
      const sent = body as { collections: string[]; checks: { effective: number } }
      const collections = [...new Set(sent.collections.map(c => c.toLowerCase()))]
      // What the server adds: the manager it was signed against, the root it rebuilds, and its status.
      const coupon = {
        id: `coupon-${coupons.length + 1}`,
        ...sent,
        collections,
        couponManager: COUPON_MANAGER_V3,
        root: StandardMerkleTree.of(
          collections.map(c => [c]),
          ['address']
        ).root,
        state: null,
        status: sent.checks.effective > Date.now() ? 'scheduled' : 'active',
        createdAt: Date.now()
      }
      coupons.push(coupon)
      return ok(coupon)
    }
    if (method === 'POST' && path === '/v1/trades') {
      const id = `trade-${++tradeCount}`
      const trade = { id, ...(body as Record<string, unknown>), contract: MARKETPLACE_V3, createdAt: Date.now() }
      trades.set(id, trade)
      list(trade)
      return ok(trade)
    }
    const trade = path.match(/^\/v1\/trades\/([^/]+)$/)
    if (method === 'GET' && trade) {
      const found = trades.get(trade[1])
      return found ? ok(found) : { status: 404, body: { ok: false, message: 'trade not found' } }
    }
    return null
  }

  /** A public item order is what the shop lists: mirror it in both listing feeds. */
  function list(trade: Record<string, unknown>) {
    const [sent] = trade.sent as Array<{ contractAddress: string; itemId: string }>
    const [received] = trade.received as Array<{ assetType: number; amount: string }>
    const key = (row: { contractAddress: string; itemId: string }) =>
      row.contractAddress.toLowerCase() === sent.contractAddress.toLowerCase() && row.itemId === sent.itemId
    unlist(key)
    const credits = received.assetType === USD_PEGGED_MANA
    items.push({
      itemId: sent.itemId,
      contractAddress: sent.contractAddress,
      price: received.amount,
      isOnSale: true,
      tradeId: String(trade.id),
      tradeContractAddress: MARKETPLACE_V3
    })
    catalog.push({
      itemId: sent.itemId,
      contractAddress: sent.contractAddress,
      priceCredits: credits ? Number(BigInt(received.amount) / 10n ** 17n) : null,
      source: credits ? 'native' : 'legacy',
      tradeId: String(trade.id)
    })
  }

  function unlist(match: (row: { contractAddress: string; itemId: string }) => boolean) {
    for (const rows of [items, catalog] as Array<Array<{ contractAddress: string; itemId: string }>>) {
      for (let i = rows.length - 1; i >= 0; i--) if (match(rows[i])) rows.splice(i, 1)
    }
  }

  // A cancelled signature takes its trade off the market, as the indexer would.
  function applyTransaction(tx: SentTransaction) {
    if (tx.fn !== 'cancelSignature') return
    for (const coupon of coupons) {
      if (tx.data.toLowerCase().includes(String(coupon.signature).slice(2).toLowerCase())) coupon.status = 'cancelled'
    }
    for (const [id, trade] of trades) {
      if (!tx.data.toLowerCase().includes(String(trade.signature).slice(2).toLowerCase())) continue
      trades.delete(id)
      const [sent] = trade.sent as Array<{ contractAddress: string; itemId: string }>
      unlist(
        row => row.contractAddress.toLowerCase() === sent.contractAddress.toLowerCase() && row.itemId === sent.itemId
      )
    }
  }

  return { handle, applyTransaction, items, catalog, trades, coupons }
}

export type Market = ReturnType<typeof createMarket>

// The gift picker's grid: the player's owned on-chain wearables or emotes, a page at a time, newest first, filtered
// by name on the server, each with its owned tokens (the token a gift sends) and how many copies there are.
//   from: catalyst GET /explorer/:address/{wearables,emotes}?collectionType=on-chain
// It also answers which token urns the player's look wears, so the picker keeps an equipped single copy.
import { getPlayer } from '@dcl/sdk/players'
import { catalystBase, getJson } from '../http'
import { thumbnailUrl } from './collections'
import { currentLook } from './avatarDraft'
import type { Ctx } from '../bridge'
import type { CatalogKind, GiftableItem, GiftableToken } from '@explorer-core/engine/protocol'

const MAX_PAGE_SIZE = 100

type Element = {
  urn: string
  name?: string
  rarity?: string
  category?: string
  amount?: number
  individualData?: Array<{ id?: string; tokenId?: string; transferredAt?: string | number }>
  entity?: { metadata?: { name?: string; thumbnail?: string; rarity?: string; data?: { category?: string }; emoteDataADR74?: { category?: string } }; content?: Array<{ file: string; hash: string }> }
}

const seconds = (v: unknown): number => {
  const n = typeof v === 'number' ? v : typeof v === 'string' ? Number(v) : NaN
  return Number.isFinite(n) && n > 0 ? n : 0
}

function tokensOf(el: Element): GiftableToken[] {
  const out: GiftableToken[] = []
  for (const d of el.individualData ?? []) {
    const tokenId = typeof d.tokenId === 'string' && /^\d{1,78}$/.test(d.tokenId) ? d.tokenId : null
    if (tokenId == null) continue
    const urn = typeof d.id === 'string' && d.id.startsWith(`${el.urn}:`) ? d.id : `${el.urn}:${tokenId}`
    out.push({ urn, tokenId, transferredAt: seconds(d.transferredAt) })
  }
  return out
}

/** One page of owned on-chain items as the picker lists them. */
function giftableItems(base: string, elements: readonly Element[]): GiftableItem[] {
  return elements.map((el) => {
    const file = el.entity?.metadata?.thumbnail
    const hash = el.entity?.content?.find((c) => c.file === file)?.hash
    const tokens = tokensOf(el)
    return {
      urn: el.urn,
      name: el.name ?? el.entity?.metadata?.name ?? '',
      rarity: el.rarity ?? el.entity?.metadata?.rarity ?? 'base',
      category: el.category ?? el.entity?.metadata?.data?.category ?? el.entity?.metadata?.emoteDataADR74?.category ?? '',
      thumbnail: hash != null ? `${base}/content/contents/${hash}` : thumbnailUrl(base, el.urn),
      amount: typeof el.amount === 'number' && el.amount > 0 ? el.amount : Math.max(tokens.length, 1),
      tokens
    }
  })
}

async function fetchGiftingPage(
  address: string,
  q: { catalog: CatalogKind; page: number; pageSize: number; search?: string }
): Promise<{ items: GiftableItem[]; total: number; failed: boolean }> {
  const base = await catalystBase()
  const size = Math.min(Math.max(1, Math.floor(q.pageSize)), MAX_PAGE_SIZE)
  let url = `${base}/explorer/${address}/${q.catalog}?pageNum=${Math.max(0, Math.floor(q.page)) + 1}&pageSize=${size}`
  url += '&collectionType=on-chain&orderBy=date&direction=DESC&includeAmount=true&includeEntities=true'
  if (q.search != null && q.search !== '') url += `&name=${encodeURIComponent(q.search)}`
  const data = await getJson<{ elements?: Element[]; totalAmount?: number }>(url).catch(() => null)
  if (data == null) return { items: [], total: 0, failed: true }
  const items = giftableItems(base, data.elements ?? [])
  return { items, total: data.totalAmount ?? items.length, failed: false }
}

export function registerGifting(ctx: Ctx): void {
  ctx.on('giftingQuery', async (msg) => {
    const me = getPlayer()
    const look = currentLook()
    const equipped = look != null ? [...look.wearables, ...look.emotes].filter((u) => u !== '') : []
    const page = me != null ? await fetchGiftingPage(me.userId, msg).catch(() => ({ items: [], total: 0, failed: true })) : { items: [], total: 0, failed: true }
    ctx.send({ kind: 'giftingPage', catalog: msg.catalog, items: page.items, total: page.total, equipped, failed: page.failed, requestId: msg.requestId })
  })
}

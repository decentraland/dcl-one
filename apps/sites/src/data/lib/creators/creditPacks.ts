// The shop-credit packs a creator can buy to cover a publication fee, and the rule that preselects
// one. The catalogue is credits-server's (public GET /credits/packs); the bundled copy is only a
// fallback so the picker renders before, or without, the fetch. Ids are the contract: checkout is
// always priced by the server from `packId`.
import { config } from '~/config'

export type CreditPack = {
  id: string
  /** The charge, in dollars. Priced above `credits × $0.10` on purpose: the premium covers the card fee. */
  usd: number
  credits: number
  /** Artwork published by the catalogue; absent on the bundled fallback. */
  artUrl?: string
}

export type PackSelection = {
  packId: string
  quantity: number
}

/**
 * FALLBACK ONLY, used when the catalogue fetch fails. The packs come from credits-server (GET /credits/packs,
 * the same map its checkout prices from); this copy exists so an outage still shows something to buy rather
 * than an empty picker. It is not a price list: checkout is priced by the server from `packId`, so a drift
 * here only affects what is displayed. Keep the ids in sync with the server catalogue.
 */
export const CREDIT_PACKS: CreditPack[] = [
  { id: 'pack_5', usd: 5.99, credits: 40 },
  { id: 'pack_10', usd: 11.99, credits: 100 },
  { id: 'pack_25', usd: 29.99, credits: 260 },
  { id: 'pack_50', usd: 59.99, credits: 540 }
]

/** One checkout buys several copies of one pack at most; the server refuses more. */
export const MAX_PACK_QUANTITY = 5

type ServerCreditPack = {
  id: string
  usd: number
  credits: number
  order?: number
  imageUrl?: string
  imageUrlWebp?: string
}

export async function fetchCreditPacks(): Promise<CreditPack[]> {
  const response = await fetch(`${config.get('CREDITS_SERVER_URL')}/credits/packs`)
  if (!response.ok) {
    await response.body?.cancel()
    throw new Error(`credit packs request failed (${response.status})`)
  }
  const { packs } = (await response.json()) as { packs: ServerCreditPack[] }
  return packs
    .slice()
    .sort((a, b) => (a.order ?? 0) - (b.order ?? 0))
    .map(pack => {
      // An empty string is a missing url, so `||` rather than `??`.
      const artUrl = pack.imageUrlWebp || pack.imageUrl || undefined
      return { id: pack.id, usd: pack.usd, credits: pack.credits, ...(artUrl ? { artUrl } : {}) }
    })
}

/**
 * The pack and quantity to preselect for a creator `shortfall` credits short: the cheapest single pack
 * that covers it, or, when none does, as many of the largest pack as it takes. Null with nothing to buy.
 */
export function recommendPack(packs: CreditPack[], shortfall: number): PackSelection | null {
  if (packs.length === 0) return null
  const needed = Math.max(0, shortfall)
  const covering = packs.filter(pack => pack.credits >= needed)
  if (covering.length > 0) {
    const cheapest = covering.reduce((best, pack) =>
      pack.usd < best.usd || (pack.usd === best.usd && pack.credits < best.credits) ? pack : best
    )
    return { packId: cheapest.id, quantity: 1 }
  }
  const largest = packs.reduce((best, pack) => (pack.credits > best.credits ? pack : best))
  return { packId: largest.id, quantity: Math.min(MAX_PACK_QUANTITY, Math.ceil(needed / largest.credits)) }
}

/** What a selection buys, in credits and dollars (dollars kept to cents, so 3 × 5.99 is 17.97). */
export type PackTotals = { credits: number; usd: number }

export function selectionTotals(packs: CreditPack[], selection: PackSelection | null): PackTotals {
  const pack = selection ? packs.find(candidate => candidate.id === selection.packId) : undefined
  if (!pack || !selection) return { credits: 0, usd: 0 }
  return {
    credits: pack.credits * selection.quantity,
    usd: Math.round(pack.usd * selection.quantity * 100) / 100
  }
}

export function formatUsd(usd: number): string {
  return usd.toLocaleString('en-US', { style: 'currency', currency: 'USD' })
}

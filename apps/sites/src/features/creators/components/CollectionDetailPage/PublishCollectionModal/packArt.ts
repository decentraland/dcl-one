import packChest from '~/assets/credits/pack-chest.webp'
import packCoins from '~/assets/credits/pack-coins.webp'
import packStacks from '~/assets/credits/pack-stacks.webp'
import { type CreditPack } from '~/lib/creditPacks'

// The bundled renders escalate with the pack size; the catalogue's own url wins when it publishes one.
const PACK_ART: Record<string, string> = {
  pack_5: packCoins,
  pack_10: packCoins,
  pack_25: packStacks,
  pack_50: packChest
}
const PACK_ART_ORDER = [packCoins, packCoins, packStacks, packChest]

export function artForPack(pack: CreditPack, index: number): string {
  return pack.artUrl ?? PACK_ART[pack.id] ?? PACK_ART_ORDER[index % PACK_ART_ORDER.length]
}

/** The bundled art of the largest pack that fits in `credits`: what a purchase of that many looks like. */
export function artForCredits(packs: CreditPack[], credits: number): string {
  const bought = packs
    .map((pack, index) => ({ pack, index }))
    .filter(({ pack }) => pack.credits <= credits)
    .reduce<{ pack: CreditPack; index: number } | null>(
      (best, entry) => (!best || entry.pack.credits > best.pack.credits ? entry : best),
      null
    )
  if (!bought) return packCoins
  return PACK_ART[bought.pack.id] ?? PACK_ART_ORDER[bought.index % PACK_ART_ORDER.length]
}

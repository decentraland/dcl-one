// Publication fee math. builder-server prices every rarity the same, so the fee per item is the
// first rarity's price; the wire prices are wei strings (USD wei from the graph, MANA wei from the
// oracle). Credits are the shop's USD credits: 1 credit = 10 US cents, always rounded up, matching
// credits-server's `authorize-publication` rounding.
import { formatEther } from 'viem'
import { type BlockchainRarity } from './rarities'

export const USD_CENTS_PER_CREDIT = 10

const WEI_PER_USD_CENT = 10n ** 16n

export type FeeAmount = {
  manaWei: bigint
  usdWei: bigint
  credits: number
}

export type PublicationFee = {
  itemCount: number
  perItem: FeeAmount
  total: FeeAmount
  /** MANA wei per credit at the current oracle rate (0 when the fee is free). */
  manaPerCredit: bigint
}

/** USD wei → whole cents, rounded up so a fraction of a cent never under-charges. */
export function weiToUsdCents(usdWei: bigint): number {
  return Number((usdWei + WEI_PER_USD_CENT - 1n) / WEI_PER_USD_CENT)
}

export function usdCentsToCredits(cents: number): number {
  return Math.ceil(cents / USD_CENTS_PER_CREDIT)
}

export function creditsForUsdWei(usdWei: bigint): number {
  return usdCentsToCredits(weiToUsdCents(usdWei))
}

function toFeeAmount(manaWei: bigint, usdWei: bigint): FeeAmount {
  return { manaWei, usdWei, credits: creditsForUsdWei(usdWei) }
}

/** The fee for publishing `itemCount` items, or null while the rarities carry no prices. */
export function getPublicationFee(rarities: BlockchainRarity[], itemCount: number): PublicationFee | null {
  const prices = rarities[0]?.prices
  if (!prices) return null
  const perItemMana = BigInt(prices.MANA)
  const perItemUsd = BigInt(prices.USD)
  const count = BigInt(Math.max(itemCount, 0))
  const total = toFeeAmount(perItemMana * count, perItemUsd * count)
  return {
    itemCount,
    perItem: toFeeAmount(perItemMana, perItemUsd),
    total,
    manaPerCredit: total.credits > 0 ? total.manaWei / BigInt(total.credits) : 0n
  }
}

/** MANA wei → a compact decimal string: whole numbers stay whole, fractions keep up to 2 decimals. */
export function formatMana(wei: bigint, maxDecimals = 2): string {
  const value = Number(formatEther(wei))
  return value.toLocaleString('en-US', { maximumFractionDigits: maxDecimals })
}

export function formatCredits(credits: number): string {
  return credits.toLocaleString('en-US', { maximumFractionDigits: 0 })
}

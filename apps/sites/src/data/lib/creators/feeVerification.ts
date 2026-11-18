// The publish fee shown to the creator comes from builder-server's /rarities; the CollectionManager
// charges whatever RaritiesWithOracle says at execution. Before any wallet prompt the quote is checked
// against that contract, so a wrong or poisoned server figure can neither over-approve MANA nor
// over-debit credits.
import { ContractName, getContract } from '~/lib/chain/contracts'
import { readContract } from '~/lib/auth'
import { type Item } from './items'
import { type PublicationFee } from './publishFee'

/** How far the quoted MANA total may sit from the on-chain total (the oracle moves between the two reads). */
export const FEE_TOLERANCE_BPS = 500n

export class PublicationFeeMismatchError extends Error {
  quotedWei: bigint
  onChainWei: bigint

  constructor(quotedWei: bigint, onChainWei: bigint) {
    super(`Publication fee mismatch: quoted ${quotedWei} wei, contract charges ${onChainWei} wei`)
    this.name = 'PublicationFeeMismatchError'
    this.quotedWei = quotedWei
    this.onChainWei = onChainWei
  }
}

export type RarityPriceReader = (rarityName: string, chainId: number) => Promise<bigint>

/** RaritiesWithOracle.getRarityByName(name).price: the MANA a single item of that rarity costs right now. */
export async function fetchOnChainRarityPrice(rarityName: string, chainId: number): Promise<bigint> {
  const contract = getContract(ContractName.RaritiesWithOracle, chainId)
  const rarity = await readContract<{ price: bigint }>(contract, 'getRarityByName', [rarityName])
  return rarity.price
}

/** `amountWei` plus the same tolerance, rounded up: what an approval must cover so the publish cannot revert on drift. */
export function withFeeTolerance(amountWei: bigint): bigint {
  return (amountWei * (10_000n + FEE_TOLERANCE_BPS) + 9_999n) / 10_000n
}

/** Whether `quoted` sits within the oracle tolerance of `onChain`. */
export function isWithinFeeTolerance(quoted: bigint, onChain: bigint): boolean {
  const difference = quoted > onChain ? quoted - onChain : onChain - quoted
  return difference * 10_000n <= onChain * FEE_TOLERANCE_BPS
}

/**
 * Throws PublicationFeeMismatchError when the quoted total is not what the contract would charge for
 * these items: one on-chain read per distinct rarity, summed by item count. Answers that on-chain total,
 * which is what the contract will actually pull.
 */
export async function verifyPublicationFee(
  fee: PublicationFee,
  items: Pick<Item, 'rarity'>[],
  chainId: number,
  readPrice: RarityPriceReader = fetchOnChainRarityPrice
): Promise<bigint> {
  const counts = new Map<string, bigint>()
  for (const item of items) {
    const rarity = item.rarity ?? ''
    counts.set(rarity, (counts.get(rarity) ?? 0n) + 1n)
  }
  const entries = [...counts.entries()]
  const prices = await Promise.all(entries.map(([rarity]) => readPrice(rarity, chainId)))
  const onChainTotal = entries.reduce((total, [, count], index) => total + prices[index] * count, 0n)
  if (!isWithinFeeTolerance(fee.total.manaWei, onChainTotal)) {
    throw new PublicationFeeMismatchError(fee.total.manaWei, onChainTotal)
  }
  return onChainTotal
}

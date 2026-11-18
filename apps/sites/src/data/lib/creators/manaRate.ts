// The MANA/USD rate the off-chain marketplace settles USD-pegged trades with, read from the same
// Chainlink aggregator the contract uses, so the USD hint next to a MANA price matches what a sale converts at.
import { parseAbi } from 'viem'
import { readContract, type ContractData } from '~/lib/auth'
import { getOffchainMarketplaceContract } from '~/lib/trades'

// Aggregator heartbeat (~24h) plus a buffer: an older round is treated as stale rather than shown.
const MAX_STALENESS_SECONDS = 90_000
// A round "from the future" means the local clock is off; a few minutes of skew is tolerated, more is not trusted.
const MAX_CLOCK_SKEW_SECONDS = 300

const AGGREGATOR_ABI = parseAbi([
  'function decimals() view returns (uint8)',
  'function latestRoundData() view returns (uint80 roundId, int256 answer, uint256 startedAt, uint256 updatedAt, uint80 answeredInRound)'
])

type RoundData = readonly [
  roundId: bigint,
  answer: bigint,
  startedAt: bigint,
  updatedAt: bigint,
  answeredInRound: bigint
]

export type ReadContract = <T>(contract: ContractData, method: string, args?: unknown[]) => Promise<T>

export class ManaRateError extends Error {
  constructor(message: string) {
    super(message)
    this.name = 'ManaRateError'
  }
}

/**
 * USD wei per whole MANA (1e18 = $1). Throws on an unreachable, incomplete or stale round so the
 * caller hides the hint instead of showing a bad rate.
 */
export async function fetchManaUsdRate(
  chainId: number,
  read: ReadContract = readContract,
  now = Date.now()
): Promise<bigint> {
  const marketplace = getOffchainMarketplaceContract(chainId)
  const address = await read<string>(marketplace, 'manaUsdAggregator')
  const aggregator: ContractData = { ...marketplace, name: 'ManaUsdAggregator', address, abi: AGGREGATOR_ABI }
  const [decimals, [roundId, answer, , updatedAt, answeredInRound]] = await Promise.all([
    read<number>(aggregator, 'decimals'),
    read<RoundData>(aggregator, 'latestRoundData')
  ])
  if (answer <= 0n) throw new ManaRateError('MANA rate unavailable')
  // An answer carried over from an earlier round is not fresh data for this one.
  if (answeredInRound < roundId) throw new ManaRateError('MANA rate incomplete')
  const age = Math.floor(now / 1000) - Number(updatedAt)
  if (updatedAt <= 0n || age < -MAX_CLOCK_SKEW_SECONDS || age > MAX_STALENESS_SECONDS) {
    throw new ManaRateError('MANA rate stale')
  }
  if (Number(decimals) > 18) throw new ManaRateError(`Unexpected oracle decimals: ${String(decimals)}`)
  return answer * 10n ** BigInt(18 - Number(decimals))
}

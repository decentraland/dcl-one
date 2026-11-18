import { describe, expect, it, vi } from 'vitest'
import { ContractName, getContract } from '~/lib/chain/contracts'
import { fetchManaUsdRate, type ReadContract } from './manaRate'

const CHAIN_ID = 80002
const AGGREGATOR = '0x00000000000000000000000000000000000000dd'
const NOW = 1_700_000_000_000

function makeRead(
  round: Partial<Record<'roundId' | 'answer' | 'updatedAt' | 'answeredInRound', number>>,
  decimals = 8
) {
  // roundId, answer, startedAt, updatedAt, answeredInRound
  const data = [
    BigInt(round.roundId ?? 10),
    BigInt(round.answer ?? 30_000_000),
    0n,
    BigInt(round.updatedAt ?? NOW / 1000 - 60),
    BigInt(round.answeredInRound ?? 10)
  ]
  return vi.fn(async (contract, method) => {
    if (method === 'manaUsdAggregator') return AGGREGATOR
    if (method === 'decimals') return decimals
    if (method === 'latestRoundData') return data
    throw new Error(`unexpected ${method} on ${contract.address}`)
  }) as unknown as ReadContract & { mock: { calls: [{ address: string }, string][] } }
}

describe('fetchManaUsdRate', () => {
  it('reads the aggregator the marketplace settles with and scales its answer to USD wei per MANA', async () => {
    const read = makeRead({ answer: 30_000_000 })
    await expect(fetchManaUsdRate(CHAIN_ID, read, NOW)).resolves.toBe(300_000_000_000_000_000n)
    expect(read.mock.calls[0][0].address).toBe(getContract(ContractName.OffChainMarketplaceV3, CHAIN_ID).address)
    expect(read.mock.calls[1][0].address).toBe(AGGREGATOR)
  })

  it('refuses an empty, incomplete or stale round rather than showing a bad rate', async () => {
    await expect(fetchManaUsdRate(CHAIN_ID, makeRead({ answer: 0 }), NOW)).rejects.toThrow(/unavailable/)
    await expect(fetchManaUsdRate(CHAIN_ID, makeRead({ roundId: 11, answeredInRound: 10 }), NOW)).rejects.toThrow(
      /incomplete/
    )
    await expect(fetchManaUsdRate(CHAIN_ID, makeRead({ updatedAt: NOW / 1000 - 100_000 }), NOW)).rejects.toThrow(
      /stale/
    )
    // A round far ahead of the local clock is not trusted either; a little skew is fine.
    await expect(fetchManaUsdRate(CHAIN_ID, makeRead({ updatedAt: NOW / 1000 + 3_600 }), NOW)).rejects.toThrow(/stale/)
    await expect(fetchManaUsdRate(CHAIN_ID, makeRead({ updatedAt: NOW / 1000 + 60 }), NOW)).resolves.toBeTypeOf(
      'bigint'
    )
  })

  it('refuses an aggregator with more than 18 decimals', async () => {
    await expect(fetchManaUsdRate(CHAIN_ID, makeRead({}, 19), NOW)).rejects.toThrow(/decimals/)
  })
})

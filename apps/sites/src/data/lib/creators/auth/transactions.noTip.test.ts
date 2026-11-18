import { describe, expect, it, vi } from 'vitest'
import { parseAbi } from 'viem'
import { ContractName, getCouponManager } from '~/lib/chain/contracts'

// The production networks set no tip floor: the wallet's own fee estimate goes through untouched.
vi.mock('~/config', async () => {
  const actual = await vi.importActual<typeof import('~/config')>('~/config')
  return {
    ...actual,
    config: { get: (key: string) => (key === 'MATIC_MIN_TIP_GWEI' ? '' : actual.config.get(key)) }
  }
})

import { type Session } from './auth'
import { sendWalletTransaction } from '.'

const ACCOUNT = '0x00000000000000000000000000000000000000aa'

describe('when the environment sets no tip floor', () => {
  it('should leave the fees to the wallet', async () => {
    const request = vi.fn(async ({ method }: { method: string }): Promise<unknown> => {
      if (method === 'eth_accounts') return [ACCOUNT]
      if (method === 'eth_chainId') return '0x13882'
      if (method === 'eth_estimateGas') return '0x5208'
      if (method === 'eth_getTransactionByHash') return { hash: `0x${'ab'.repeat(32)}`, from: ACCOUNT, nonce: '0x1' }
      return `0x${'ab'.repeat(32)}`
    })
    const session = { address: ACCOUNT, provider: { request } } as unknown as Session
    const manager = {
      ...getCouponManager(ContractName.OffChainMarketplaceV3, 80002),
      abi: parseAbi(['function increaseSignerSignatureIndex()'])
    }

    await sendWalletTransaction(session, { contract: manager, method: 'increaseSignerSignatureIndex', args: [] })

    const send = request.mock.calls.find(([{ method }]) => method === 'eth_sendTransaction')
    expect(send?.[0]).toMatchObject({
      params: [expect.not.objectContaining({ maxPriorityFeePerGas: expect.anything() })]
    })
  })
})

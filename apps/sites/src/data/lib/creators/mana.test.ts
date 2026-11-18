import { describe, expect, it } from 'vitest'
import { maxUint256 } from 'viem'
import { decodeCall, toTransaction } from '~/test/chain'
import { ContractName, getContract } from '~/lib/chain/contracts'
import { buildManaApproveCall, getManaContract } from './mana'

const CHAIN_ID = 80002
const SPENDER = getContract(ContractName.CollectionManager, CHAIN_ID).address

describe('buildManaApproveCall', () => {
  it('approves the spender for exactly the given amount, never an open-ended allowance', () => {
    const call = buildManaApproveCall(CHAIN_ID, SPENDER, 3000n * 10n ** 18n)
    expect(call.contract.address).toBe(getManaContract(CHAIN_ID).address)
    const decoded = decodeCall(call)
    expect((decoded[0] as string).toLowerCase()).toBe(SPENDER.toLowerCase())
    expect(decoded[1]).toBe(3000n * 10n ** 18n)
    expect(decoded[1]).not.toBe(maxUint256)
    expect(toTransaction(call)).toEqual({
      to: '0x7ad72b9f944ea9793cf4055d88f81138cc2c63a0',
      data: '0x095ea7b30000000000000000000000005309ae874fc4eb21adcd63f8b6c3f766cc3b18490000000000000000000000000000000000000000000000a2a15d09519be00000'
    })
  })

  it('refuses a zero or negative amount', () => {
    expect(() => buildManaApproveCall(CHAIN_ID, SPENDER, 0n)).toThrow()
  })
})

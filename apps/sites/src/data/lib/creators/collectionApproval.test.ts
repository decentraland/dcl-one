import { describe, expect, it } from 'vitest'
import { ContractName, getCollectionContract, getContract } from '~/lib/chain/contracts'
import { decodeFunctionData, type Hex } from 'viem'
import { toTransaction } from '~/test/chain'
import { type Collection } from './collections'
import { buildRescueItemsCalls, buildSetApprovedCall, RESCUE_CHUNK_SIZE } from './collectionApproval'

const CHAIN_ID = 80002
const collection = { id: 'c1', contractAddress: '0x' + '1'.repeat(40) } as Collection
const collectionAbi = getCollectionContract(CHAIN_ID, collection.contractAddress!).abi

function innerCall(call: ReturnType<typeof buildSetApprovedCall>) {
  const [, , target, data] = call.args as [string, string, string, string[]]
  return { target, decoded: decodeFunctionData({ abi: collectionAbi, data: data[0] as Hex }) as any }
}

describe('buildSetApprovedCall', () => {
  it('routes setApproved through the committee contract', () => {
    const call = buildSetApprovedCall(CHAIN_ID, collection, false)
    expect(call.contract.address).toBe(getContract(ContractName.Committee, CHAIN_ID).address)
    expect(call.method).toBe('manageCollection')
    const { target, decoded } = innerCall(call)
    expect(target).toBe(collection.contractAddress)
    expect(decoded.functionName).toBe('setApproved')
    expect(decoded.args[0]).toBe(false)
  })

  it('encodes the exact committee transaction', () => {
    expect(toTransaction(buildSetApprovedCall(CHAIN_ID, collection, true))).toEqual({
      to: '0x53c9c388000b6fc91b91fb466742d0e869785c3b',
      data:
        '0x81c9308e' +
        '0000000000000000000000005309ae874fc4eb21adcd63f8b6c3f766cc3b1849' +
        '0000000000000000000000007b1fe9de545b22cb553766817b84d655ce8121c9' +
        '0000000000000000000000001111111111111111111111111111111111111111' +
        '0000000000000000000000000000000000000000000000000000000000000080' +
        '0000000000000000000000000000000000000000000000000000000000000001' +
        '0000000000000000000000000000000000000000000000000000000000000020' +
        '0000000000000000000000000000000000000000000000000000000000000024' +
        '46d5a56800000000000000000000000000000000000000000000000000000000' +
        '0000000100000000000000000000000000000000000000000000000000000000'
    })
  })

  it('refuses a collection without a contract', () => {
    expect(() => buildSetApprovedCall(CHAIN_ID, { id: 'x' } as Collection, true)).toThrow()
  })
})

describe('buildRescueItemsCalls', () => {
  it('splits the items into chunks and keeps each item aligned with its hash and metadata', () => {
    const entries = Array.from({ length: RESCUE_CHUNK_SIZE + 2 }, (_, i) => ({
      tokenId: String(i),
      contentHash: `hash${i}`,
      metadata: `meta${i}`
    }))
    const calls = buildRescueItemsCalls(CHAIN_ID, collection, entries)
    expect(calls).toHaveLength(2)
    const { decoded } = innerCall(calls[1])
    expect(decoded.functionName).toBe('rescueItems')
    expect(decoded.args[0].map(String)).toEqual([String(RESCUE_CHUNK_SIZE), String(RESCUE_CHUNK_SIZE + 1)])
    expect(decoded.args[1]).toEqual([`hash${RESCUE_CHUNK_SIZE}`, `hash${RESCUE_CHUNK_SIZE + 1}`])
    expect(decoded.args[2]).toEqual([`meta${RESCUE_CHUNK_SIZE}`, `meta${RESCUE_CHUNK_SIZE + 1}`])
  })

  it('encodes the exact committee transaction', () => {
    const entry = { tokenId: '1', contentHash: 'bafkreifakecontenthash', metadata: '1:w:Hat::hat:BaseMale' }
    expect(toTransaction(buildRescueItemsCalls(CHAIN_ID, collection, [entry])[0])).toEqual({
      to: '0x53c9c388000b6fc91b91fb466742d0e869785c3b',
      data:
        '0x81c9308e' +
        '0000000000000000000000005309ae874fc4eb21adcd63f8b6c3f766cc3b1849' +
        '0000000000000000000000007b1fe9de545b22cb553766817b84d655ce8121c9' +
        '0000000000000000000000001111111111111111111111111111111111111111' +
        '0000000000000000000000000000000000000000000000000000000000000080' +
        '0000000000000000000000000000000000000000000000000000000000000001' +
        '0000000000000000000000000000000000000000000000000000000000000020' +
        '00000000000000000000000000000000000000000000000000000000000001a4' +
        '3c96365500000000000000000000000000000000000000000000000000000000' +
        '0000006000000000000000000000000000000000000000000000000000000000' +
        '000000a000000000000000000000000000000000000000000000000000000000' +
        '0000012000000000000000000000000000000000000000000000000000000000' +
        '0000000100000000000000000000000000000000000000000000000000000000' +
        '0000000100000000000000000000000000000000000000000000000000000000' +
        '0000000100000000000000000000000000000000000000000000000000000000' +
        '0000002000000000000000000000000000000000000000000000000000000000' +
        '000000166261666b72656966616b65636f6e74656e7468617368000000000000' +
        '0000000000000000000000000000000000000000000000000000000000000000' +
        '0000000100000000000000000000000000000000000000000000000000000000' +
        '0000002000000000000000000000000000000000000000000000000000000000' +
        '00000015313a773a4861743a3a6861743a426173654d616c6500000000000000' +
        '0000000000000000000000000000000000000000000000000000000000000000'
    })
  })
})

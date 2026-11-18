import { describe, it, expect, vi } from 'vitest'
import { type ContractNetwork } from '@dcl/schemas'
import {
  ERC1155_INTERFACE_ID,
  ERC721_INTERFACE_ID,
  checkLinkedContract,
  isAddressBeingTyped,
  isContractAddress
} from './linkedContract'

vi.mock('~/lib/auth', () => ({ readContract: vi.fn() }))

const ADDRESS = '0x1d9fb685c257e74f869ba302e260c0b68f5ebb37'

describe('address input', () => {
  it('accepts a full address and any prefix of one, nothing else', () => {
    expect(isContractAddress(ADDRESS)).toBe(true)
    expect(isContractAddress(ADDRESS.slice(0, 10))).toBe(false)
    expect(isAddressBeingTyped('')).toBe(true)
    expect(isAddressBeingTyped('0x1D9f')).toBe(true)
    expect(isAddressBeingTyped('0xzz')).toBe(false)
    expect(isAddressBeingTyped(ADDRESS + '0')).toBe(false)
  })
})

describe('checkLinkedContract', () => {
  const amoy = 'amoy' as ContractNetwork

  it('is ok when the contract supports ERC-721 or ERC-1155 on the picked network', async () => {
    const supports = vi.fn(async (_chainId: number, _address: string, id: string) => id === ERC721_INTERFACE_ID)
    await expect(checkLinkedContract(amoy, ADDRESS, supports)).resolves.toBe('ok')
    expect(supports).toHaveBeenCalledWith(80002, ADDRESS, ERC721_INTERFACE_ID)
    expect(supports).toHaveBeenCalledWith(80002, ADDRESS, ERC1155_INTERFACE_ID)
  })

  it('tells a non-NFT contract from a failed call', async () => {
    await expect(checkLinkedContract(amoy, ADDRESS, async () => false)).resolves.toBe('not_nft')
    await expect(
      checkLinkedContract(amoy, ADDRESS, async () => {
        throw new Error('call revert exception')
      })
    ).resolves.toBe('error')
  })

  it('fails without calling the chain for an invalid address or an unsupported network', async () => {
    const supports = vi.fn(async () => true)
    await expect(checkLinkedContract(amoy, '0x123', supports)).resolves.toBe('error')
    await expect(checkLinkedContract('base_mainnet' as ContractNetwork, ADDRESS, supports)).resolves.toBe('error')
    expect(supports).not.toHaveBeenCalled()
  })
})

import { describe, expect, it, vi } from 'vitest'
import { type ContractNetwork } from '@dcl/schemas'
import { ContractName, getContract } from '~/lib/chain/contracts'
import { zeroHash, parseSignature, serializeCompactSignature, signatureToCompactSignature } from 'viem'
import { readContract } from '~/lib/auth'
import { decodeCall, toTransaction } from '~/test/chain'
import { type ThirdParty } from './thirdParty'
import {
  buildAddThirdPartyCall,
  buildBuyItemSlotsCall,
  buildReviewThirdPartyCall,
  buildReviewThirdPartyWithRootCall,
  buildUpdateThirdPartyCall,
  fetchRegisteredContracts,
  fetchRegisteredThirdParty,
  fetchThirdPartyPrice,
  getThirdPartyRegistry,
  toChequeSlot,
  usdToMana
} from './thirdPartyRegistry'

vi.mock('~/lib/auth', () => ({ readContract: vi.fn() }))

const CHAIN_ID = 80002
const ETHER = 10n ** 18n
const MANAGER = '0x00000000000000000000000000000000000000aa'
const THIRD_PARTY_ID = 'urn:decentraland:amoy:collections-thirdparty:brand'
const REGISTRY = getThirdPartyRegistry(CHAIN_ID)
const CHEQUE_SALT = '0x' + '11'.repeat(32)
// The test key's ConsumeSlots signature over 7 slots for THIRD_PARTY_ID with CHEQUE_SALT.
const CHEQUE_SIGNATURE =
  '0xbf74a2f7e0d8605bf625e60290c386c17b46a686883679291f59cba180a29c3f02091e78bd3af41ca07b70402079d52fca7db57a373c3d8425f71f75d1be88d91c'

const thirdParty: ThirdParty = {
  id: THIRD_PARTY_ID,
  root: '',
  managers: [MANAGER],
  name: 'Brand',
  description: 'A brand',
  isApproved: false,
  isProgrammatic: false,
  maxItems: '0',
  contracts: [{ network: 'amoy' as ContractNetwork, address: '0x' + '1'.repeat(40) }],
  published: false
}

/** Encodes the call with the registry ABI and decodes it back: proves the args fit the contract's signature. */
function roundTrip(call: ReturnType<typeof buildBuyItemSlotsCall>): any[] {
  expect(call.contract.address).toBe(REGISTRY.address)
  return decodeCall(call) as any[]
}

describe('buildAddThirdPartyCall', () => {
  it('registers the third party with its raw metadata, enabled managers, slots, kind and price cap', () => {
    const call = buildAddThirdPartyCall(CHAIN_ID, { ...thirdParty, isProgrammatic: true }, 200, 603n * ETHER)

    expect(call.method).toBe('addThirdParties')
    const [parties, programmatic, maxPrices] = roundTrip(call)
    expect(parties).toHaveLength(1)
    const { id, metadata, resolver, managers, managerValues, slots } = parties[0]
    expect(id).toBe(THIRD_PARTY_ID)
    expect(metadata).toBe(`tp:1:Brand:A brand:amoy-${'0x' + '1'.repeat(40)}`)
    expect(resolver).toBe('Disabled')
    expect(managers.map((manager: string) => manager.toLowerCase())).toEqual([MANAGER])
    expect(managerValues).toEqual([true])
    expect(slots).toBe(200n)
    expect(programmatic).toEqual([true])
    expect(maxPrices).toEqual([603n * ETHER])
    expect(toTransaction(call)).toEqual({
      to: '0x7d7c0b9d97385bada5fec6861e97d0df414af3c3',
      data:
        '0xdb9aa95c' +
        '0000000000000000000000000000000000000000000000000000000000000060' +
        '0000000000000000000000000000000000000000000000000000000000000300' +
        '0000000000000000000000000000000000000000000000000000000000000340' +
        '0000000000000000000000000000000000000000000000000000000000000001' +
        '0000000000000000000000000000000000000000000000000000000000000020' +
        '00000000000000000000000000000000000000000000000000000000000000c0' +
        '0000000000000000000000000000000000000000000000000000000000000120' +
        '00000000000000000000000000000000000000000000000000000000000001a0' +
        '00000000000000000000000000000000000000000000000000000000000001e0' +
        '0000000000000000000000000000000000000000000000000000000000000220' +
        '00000000000000000000000000000000000000000000000000000000000000c8' +
        '0000000000000000000000000000000000000000000000000000000000000032' +
        '75726e3a646563656e7472616c616e643a616d6f793a636f6c6c656374696f6e' +
        '732d746869726470617274793a6272616e640000000000000000000000000000' +
        '0000000000000000000000000000000000000000000000000000000000000042' +
        '74703a313a4272616e643a41206272616e643a616d6f792d3078313131313131' +
        '3131313131313131313131313131313131313131313131313131313131313131' +
        '3131000000000000000000000000000000000000000000000000000000000000' +
        '0000000000000000000000000000000000000000000000000000000000000008' +
        '44697361626c6564000000000000000000000000000000000000000000000000' +
        '0000000000000000000000000000000000000000000000000000000000000001' +
        '00000000000000000000000000000000000000000000000000000000000000aa' +
        '0000000000000000000000000000000000000000000000000000000000000001' +
        '0000000000000000000000000000000000000000000000000000000000000001' +
        '0000000000000000000000000000000000000000000000000000000000000001' +
        '0000000000000000000000000000000000000000000000000000000000000001' +
        '0000000000000000000000000000000000000000000000000000000000000001' +
        '000000000000000000000000000000000000000000000020b04e592b488c0000'
    })
  })

  it('enables every manager', () => {
    const other = '0x00000000000000000000000000000000000000bb'
    const call = buildAddThirdPartyCall(CHAIN_ID, { ...thirdParty, managers: [MANAGER, other] }, 1, ETHER)
    const [parties] = roundTrip(call)
    expect(parties[0].managers).toHaveLength(2)
    expect(parties[0].managerValues).toEqual([true, true])
  })
})

describe('buildUpdateThirdPartyCall', () => {
  it('rewrites the metadata with the new contract and leaves resolver, managers and slots untouched', () => {
    const next = { network: 'mainnet' as ContractNetwork, address: '0x' + '2'.repeat(40) }
    const call = buildUpdateThirdPartyCall(CHAIN_ID, { ...thirdParty, contracts: [next] })

    expect(call.method).toBe('updateThirdParties')
    const [parties] = roundTrip(call)
    expect(parties).toHaveLength(1)
    const { id, metadata, resolver, managers, managerValues, slots } = parties[0]
    expect(id).toBe(THIRD_PARTY_ID)
    expect(metadata).toBe(`tp:1:Brand:A brand:mainnet-${next.address}`)
    expect(resolver).toBe('')
    expect(managers).toEqual([])
    expect(managerValues).toEqual([])
    expect(slots).toBe(0n)
    expect(toTransaction(call)).toEqual({
      to: '0x7d7c0b9d97385bada5fec6861e97d0df414af3c3',
      data:
        '0x7c08ba09' +
        '0000000000000000000000000000000000000000000000000000000000000020' +
        '0000000000000000000000000000000000000000000000000000000000000001' +
        '0000000000000000000000000000000000000000000000000000000000000020' +
        '00000000000000000000000000000000000000000000000000000000000000c0' +
        '0000000000000000000000000000000000000000000000000000000000000120' +
        '00000000000000000000000000000000000000000000000000000000000001a0' +
        '00000000000000000000000000000000000000000000000000000000000001c0' +
        '00000000000000000000000000000000000000000000000000000000000001e0' +
        '0000000000000000000000000000000000000000000000000000000000000000' +
        '0000000000000000000000000000000000000000000000000000000000000032' +
        '75726e3a646563656e7472616c616e643a616d6f793a636f6c6c656374696f6e' +
        '732d746869726470617274793a6272616e640000000000000000000000000000' +
        '0000000000000000000000000000000000000000000000000000000000000045' +
        '74703a313a4272616e643a41206272616e643a6d61696e6e65742d3078323232' +
        '3232323232323232323232323232323232323232323232323232323232323232' +
        '3232323232000000000000000000000000000000000000000000000000000000' +
        '0000000000000000000000000000000000000000000000000000000000000000' +
        '0000000000000000000000000000000000000000000000000000000000000000' +
        '0000000000000000000000000000000000000000000000000000000000000000'
    })
  })
})

describe('buildBuyItemSlotsCall', () => {
  it('buys the slots for the third party under the price cap', () => {
    const call = buildBuyItemSlotsCall(CHAIN_ID, THIRD_PARTY_ID, 3, 603n * ETHER)

    expect(call.method).toBe('buyItemSlots')
    const [id, qty, maxPrice] = roundTrip(call)
    expect(id).toBe(THIRD_PARTY_ID)
    expect(qty).toBe(3n)
    expect(maxPrice).toBe(603n * ETHER)
    expect(toTransaction(call)).toEqual({
      to: '0x7d7c0b9d97385bada5fec6861e97d0df414af3c3',
      data:
        '0xd8f0f3c1' +
        '0000000000000000000000000000000000000000000000000000000000000060' +
        '0000000000000000000000000000000000000000000000000000000000000003' +
        '000000000000000000000000000000000000000000000020b04e592b488c0000' +
        '0000000000000000000000000000000000000000000000000000000000000032' +
        '75726e3a646563656e7472616c616e643a616d6f793a636f6c6c656374696f6e' +
        '732d746869726470617274793a6272616e640000000000000000000000000000'
    })
  })
})

describe('toChequeSlot', () => {
  it('splits the cheque into the tuple the registry consumes', () => {
    expect(toChequeSlot({ signature: CHEQUE_SIGNATURE, qty: 7, salt: CHEQUE_SALT })).toEqual([
      '7',
      '0x1111111111111111111111111111111111111111111111111111111111111111',
      '0xbf74a2f7e0d8605bf625e60290c386c17b46a686883679291f59cba180a29c3f',
      '0x02091e78bd3af41ca07b70402079d52fca7db57a373c3d8425f71f75d1be88d9',
      28
    ])
  })

  it('splits a compact 64-byte signature into the same tuple', () => {
    const compact = serializeCompactSignature(signatureToCompactSignature(parseSignature(CHEQUE_SIGNATURE)))
    expect(toChequeSlot({ signature: compact, qty: 7, salt: CHEQUE_SALT })).toEqual(
      toChequeSlot({ signature: CHEQUE_SIGNATURE, qty: 7, salt: CHEQUE_SALT })
    )
  })
})

describe('buildReviewThirdPartyWithRootCall', () => {
  it('sets the root and consumes the cheques', () => {
    const salt = CHEQUE_SALT
    const root = '0x' + '22'.repeat(32)
    const slot = toChequeSlot({ signature: CHEQUE_SIGNATURE, qty: 7, salt })

    const call = buildReviewThirdPartyWithRootCall(CHAIN_ID, THIRD_PARTY_ID, root, [slot])

    expect(call.method).toBe('reviewThirdPartyWithRoot')
    const [id, decodedRoot, slots] = roundTrip(call)
    expect(id).toBe(THIRD_PARTY_ID)
    expect(decodedRoot).toBe(root)
    expect(slots).toHaveLength(1)
    expect(slots[0].qty).toBe(7n)
    expect(slots[0].salt).toBe(salt)
    expect(slots[0].sigV).toBe(slot[4])
    expect(toTransaction(call)).toEqual({
      to: '0x7d7c0b9d97385bada5fec6861e97d0df414af3c3',
      data:
        '0x3cb681b0' +
        '0000000000000000000000000000000000000000000000000000000000000060' +
        '2222222222222222222222222222222222222222222222222222222222222222' +
        '00000000000000000000000000000000000000000000000000000000000000c0' +
        '0000000000000000000000000000000000000000000000000000000000000032' +
        '75726e3a646563656e7472616c616e643a616d6f793a636f6c6c656374696f6e' +
        '732d746869726470617274793a6272616e640000000000000000000000000000' +
        '0000000000000000000000000000000000000000000000000000000000000001' +
        '0000000000000000000000000000000000000000000000000000000000000007' +
        '1111111111111111111111111111111111111111111111111111111111111111' +
        'bf74a2f7e0d8605bf625e60290c386c17b46a686883679291f59cba180a29c3f' +
        '02091e78bd3af41ca07b70402079d52fca7db57a373c3d8425f71f75d1be88d9' +
        '000000000000000000000000000000000000000000000000000000000000001c'
    })
  })

  it('accepts no cheques when they were already consumed', () => {
    const call = buildReviewThirdPartyWithRootCall(CHAIN_ID, THIRD_PARTY_ID, '0x' + '22'.repeat(32), [])
    const [, , slots] = roundTrip(call)
    expect(slots).toEqual([])
  })
})

describe('buildReviewThirdPartyCall', () => {
  it('enables or disables the whole third party', () => {
    const call = buildReviewThirdPartyCall(CHAIN_ID, THIRD_PARTY_ID, false)

    expect(call.method).toBe('reviewThirdParties')
    const [reviews] = roundTrip(call)
    expect(reviews).toHaveLength(1)
    expect(reviews[0].id).toBe(THIRD_PARTY_ID)
    expect(reviews[0].value).toBe(false)
    expect(toTransaction(call)).toEqual({
      to: '0x7d7c0b9d97385bada5fec6861e97d0df414af3c3',
      data:
        '0x39276e37' +
        '0000000000000000000000000000000000000000000000000000000000000020' +
        '0000000000000000000000000000000000000000000000000000000000000001' +
        '0000000000000000000000000000000000000000000000000000000000000020' +
        '0000000000000000000000000000000000000000000000000000000000000060' +
        '0000000000000000000000000000000000000000000000000000000000000000' +
        '00000000000000000000000000000000000000000000000000000000000000c0' +
        '0000000000000000000000000000000000000000000000000000000000000032' +
        '75726e3a646563656e7472616c616e643a616d6f793a636f6c6c656374696f6e' +
        '732d746869726470617274793a6272616e640000000000000000000000000000' +
        '0000000000000000000000000000000000000000000000000000000000000000'
    })
  })
})

describe('usdToMana', () => {
  it('converts a USD price to MANA at the oracle rate (USD per MANA)', () => {
    expect(usdToMana(100n * ETHER, ETHER / 2n)).toBe(200n * ETHER)
    expect(usdToMana(ETHER, (3n * ETHER) / 10n)).toBe(3333333333333333333n)
    expect(usdToMana(0n, ETHER)).toBe(0n)
  })

  it('answers zero rather than dividing by a zero rate', () => {
    expect(usdToMana(ETHER, 0n)).toBe(0n)
  })
})

describe('fetchThirdPartyPrice', () => {
  it('reads the slot price, the programmatic pack and the oracle rate, and prices a slot in MANA', async () => {
    const oracle = getContract(ContractName.ChainlinkOracle, CHAIN_ID)
    vi.mocked(readContract).mockImplementation(async (contract, method) => {
      if (contract.address === REGISTRY.address && method === 'itemSlotPrice') {
        return 100n * ETHER
      }
      if (contract.address === REGISTRY.address && method === 'programmaticBasePurchasedSlots') {
        return 200n
      }
      if (contract.address === oracle.address && method === 'getRate') {
        return ETHER / 2n
      }
      throw new Error(`Unexpected read ${method}`)
    })

    await expect(fetchThirdPartyPrice(CHAIN_ID)).resolves.toEqual({
      itemSlotUsdWei: 100n * ETHER,
      itemSlotManaWei: 200n * ETHER,
      programmaticMinSlots: 200,
      usdPerManaWei: ETHER / 2n
    })
  })
})

/** The registry's `thirdParties` outputs, as the chain returns them. */
function thirdPartyRecord({
  root = zeroHash,
  maxItems = 0n,
  registered = 0n,
  metadata = ''
}: {
  root?: string
  maxItems?: bigint
  registered?: bigint
  metadata?: string
}) {
  return [false, root, maxItems, 0n, registered, metadata, '']
}

describe('fetchRegisteredContracts', () => {
  it('lists the contracts of the registry record, lower-cased, and none for a bare record', async () => {
    vi.mocked(readContract).mockResolvedValueOnce(
      thirdPartyRecord({ metadata: `tp:1:Brand:A brand:mainnet-0x${'A'.repeat(40)};matic-0x${'b'.repeat(40)}` })
    )
    await expect(fetchRegisteredContracts(CHAIN_ID, THIRD_PARTY_ID)).resolves.toEqual([
      { network: 'mainnet', address: '0x' + 'a'.repeat(40) },
      { network: 'matic', address: '0x' + 'b'.repeat(40) }
    ])
    vi.mocked(readContract).mockResolvedValueOnce(thirdPartyRecord({ metadata: 'tp:1:Brand:A brand' }))
    await expect(fetchRegisteredContracts(CHAIN_ID, THIRD_PARTY_ID)).resolves.toEqual([])
  })

  it('does not mistake description text for a contract', async () => {
    vi.mocked(readContract).mockResolvedValueOnce(thirdPartyRecord({ metadata: 'tp:1:Brand:Shop: limited-edition' }))
    await expect(fetchRegisteredContracts(CHAIN_ID, THIRD_PARTY_ID)).resolves.toEqual([])
  })

  it('refuses a record whose contract segment is only partly readable', async () => {
    vi.mocked(readContract).mockResolvedValueOnce(
      thirdPartyRecord({ metadata: `tp:1:Brand:A brand:mainnet-0x${'a'.repeat(40)};moon-0x1` })
    )
    await expect(fetchRegisteredContracts(CHAIN_ID, THIRD_PARTY_ID)).rejects.toThrow('could not be parsed')
  })

  it('still reads the contracts when a legacy description contains a colon', async () => {
    vi.mocked(readContract).mockResolvedValueOnce(
      thirdPartyRecord({ metadata: `tp:1:Brand:Note: legacy:matic-0x${'b'.repeat(40)}` })
    )
    await expect(fetchRegisteredContracts(CHAIN_ID, THIRD_PARTY_ID)).resolves.toEqual([
      { network: 'matic', address: '0x' + 'b'.repeat(40) }
    ])
  })
})

describe('fetchRegisteredThirdParty', () => {
  it("reads the registry's own record of the third party", async () => {
    const root = '0x' + 'ab'.repeat(32)
    vi.mocked(readContract).mockImplementation(async (_contract, method, args) => {
      expect(method).toBe('thirdParties')
      expect(args).toEqual([THIRD_PARTY_ID])
      return thirdPartyRecord({ registered: 1n, maxItems: 12n, root })
    })
    await expect(fetchRegisteredThirdParty(CHAIN_ID, THIRD_PARTY_ID)).resolves.toEqual({
      registered: true,
      maxItems: 12,
      root
    })
  })

  it('reads an unknown third party as unregistered, with no slots and no root', async () => {
    vi.mocked(readContract).mockResolvedValue(thirdPartyRecord({}))
    await expect(fetchRegisteredThirdParty(CHAIN_ID, THIRD_PARTY_ID)).resolves.toEqual({
      registered: false,
      maxItems: 0,
      root: ''
    })
  })
})

import { describe, expect, it } from 'vitest'
import type { Hex } from 'viem'
import { BOOK, ITEM } from '@shop/payment/fixtures'
import { encodeExecuteMetaTransaction } from '../../shop/checkout/chain/metatx'
import { buildStoreBuyCalldata, buildStoreUseCreditsArgs, encodeUseCredits, type SpendableCredit, type StoreItemToBuy } from '../../shop/checkout/purchase/encoding'
import { carriesSale, isSplitSale, splitExecution, type SplitSale } from '../shell/shop/splitEvidence'

const BUYER = BOOK.buyer.toLowerCase()
const MANAGER = '0x8b3a40ca1b6f5cafc99d112a4d02e897d1fd8cc5'
const STORE = BOOK.store.contract.toLowerCase()
const C = 1_000_000_000_000_000_000n
const G = 1_300_000_000_000_000_000n
const credit: SpendableCredit = { id: '0x' + 'a1'.repeat(32), amount: C.toString(), availableAmount: C.toString(), expiresAt: Math.floor(Date.now() / 1000) + 900, signature: '0x' + 'cc'.repeat(65), contract: MANAGER }
const item: StoreItemToBuy = { collection: BOOK.collection, itemId: String(ITEM), priceWei: (C + G).toString() }
const sale: SplitSale = { buyer: BUYER, chainId: 137, manager: MANAGER, creditId: credit.id, creditWei: C.toString(), gapWei: G.toString(), callData: buildStoreBuyCalldata([item], BUYER).data }

/** The relayed meta-transaction a split's sale lands as, with `over` changing what it carries. */
function relayed(over: { credit?: SpendableCredit; items?: StoreItemToBuy[]; gap?: bigint; credited?: bigint; user?: string } = {}): Hex {
  const c = over.credit ?? credit
  const args = buildStoreUseCreditsArgs(STORE, over.items ?? [item], BUYER, [c], (over.credited ?? C).toString(), { maxUncreditedValue: over.gap ?? G })
  return encodeExecuteMetaTransaction('functionData', over.user ?? BUYER, encodeUseCredits(args), ('0x' + '11'.repeat(65)) as Hex)
}
const evidenceOf = (tx: { to: string | null; input: Hex } | null, status: 'success' | 'reverted' | null) => ({
  transaction: async () => tx,
  receipt: async () => (status ? { transactionHash: '0x' + 'e'.repeat(64), status, blockNumber: 1n } : null)
})

describe('which transaction is an in-world split purchase', () => {
  it('is bought only by a mined, successful execution of exactly its terms', async () => {
    expect(await splitExecution(evidenceOf({ to: MANAGER, input: relayed() }, 'success') as never, sale, '0xabc')).toBe('bought')
    expect(await splitExecution(evidenceOf({ to: MANAGER, input: relayed() }, 'reverted') as never, sale, '0xabc')).toBe('reverted')
  })

  it.each([
    ['another call (another price)', relayed({ items: [{ ...item, priceWei: '1' }] })],
    ['the same credit spent on another call (another item)', relayed({ items: [{ ...item, itemId: '8' }] })],
    ['another credit', relayed({ credit: { ...credit, id: '0x' + 'b2'.repeat(32) } })],
    ['another gap', relayed({ gap: G + 1n })],
    ['crediting another share', relayed({ credited: C - 1n })],
    ['another sender', relayed({ user: '0x' + 'cc'.repeat(20) })]
  ])('is not bought by %s, even successful', async (_, input) => {
    expect(await splitExecution(evidenceOf({ to: MANAGER, input }, 'success') as never, sale, '0xabc')).toBe('other')
  })

  it('is not bought by a transaction to another contract, or one it can\'t read', async () => {
    expect(await splitExecution(evidenceOf({ to: STORE, input: relayed() }, 'success') as never, sale, '0xabc')).toBe('other')
    expect(await splitExecution(evidenceOf({ to: MANAGER, input: '0xdeadbeef' }, 'success') as never, sale, '0xabc')).toBe('other')
  })

  it('says nothing yet without a transaction, without a receipt for its own, or when a read fails', async () => {
    expect(await splitExecution(evidenceOf(null, 'success') as never, sale, '0xabc')).toBe('unknown')
    expect(await splitExecution(evidenceOf({ to: MANAGER, input: relayed() }, null) as never, sale, '0xabc')).toBe('unknown')
    const failing = { transaction: async () => Promise.reject(new Error('rpc down')), receipt: async () => null }
    expect(await splitExecution(failing as never, sale, '0xabc')).toBe('unknown')
  })

  it('reads its kept terms back only when whole', () => {
    expect(isSplitSale(sale)).toBe(true)
    expect(isSplitSale({ ...sale, gapWei: '-1' })).toBe(false)
    expect(isSplitSale({ ...sale, creditId: '' })).toBe(false)
    expect(carriesSale({ to: MANAGER, input: relayed() }, sale)).toBe(true)
  })
})

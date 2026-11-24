import { describe, expect, it } from 'vitest'
import { decodeFunctionData, type Hex } from 'viem'
import { OFFCHAIN_META_TX_ABI } from '@shop/checkout/chain/abi'
import { BOOK, ITEM } from '@shop/payment/fixtures'
import { encodeExecuteMetaTransaction } from '../chain/metatx'
import { buildStoreBuyCalldata, buildStoreUseCreditsArgs, encodeUseCredits, type SpendableCredit, type StoreItemToBuy } from './encoding'
import { carriesSale, isSplitSale, splitEnvelope, splitExecution, SPLIT_TX_RETRY_MS, type SplitSale } from './splitEvidence'

const BUYER = BOOK.buyer.toLowerCase()
const MANAGER = '0x8b3a40ca1b6f5cafc99d112a4d02e897d1fd8cc5'
const STORE = BOOK.store.contract.toLowerCase()
const C = 1_000_000_000_000_000_000n
const G = 1_300_000_000_000_000_000n
const credit: SpendableCredit = { id: '0x' + 'a1'.repeat(32), amount: C.toString(), availableAmount: C.toString(), expiresAt: Math.floor(Date.now() / 1000) + 900, signature: '0x' + 'cc'.repeat(65), contract: MANAGER }
const item: StoreItemToBuy = { collection: BOOK.collection, itemId: String(ITEM), priceWei: (C + G).toString() }
const call = buildStoreBuyCalldata([item], BUYER)
const sale: SplitSale = { buyer: BUYER, chainId: 137, manager: MANAGER, creditId: credit.id, creditWei: C.toString(), gapWei: G.toString(), target: STORE, selector: call.selector, callData: call.data }

/** The relayed meta-transaction a split's sale lands as, with `over` changing what it carries. */
function relayed(over: { credit?: SpendableCredit; items?: StoreItemToBuy[]; gap?: bigint; credited?: bigint; user?: string; venue?: string; selector?: Hex; signature?: Hex } = {}): Hex {
  const c = over.credit ?? credit
  const built = buildStoreUseCreditsArgs(STORE, over.items ?? [item], BUYER, [c], (over.credited ?? C).toString(), { maxUncreditedValue: over.gap ?? G })
  const args = { ...built, externalCall: { ...built.externalCall, ...(over.venue ? { target: over.venue as Hex } : {}), ...(over.selector ? { selector: over.selector } : {}) } }
  return encodeExecuteMetaTransaction('functionData', over.user ?? BUYER, encodeUseCredits(args), over.signature ?? (('0x' + '11'.repeat(65)) as Hex))
}
/** The same relayed execution signed again: the same terms, the same call, another envelope. */
function resigned(input: Hex): Hex {
  const meta = decodeFunctionData({ abi: OFFCHAIN_META_TX_ABI, data: input })
  return encodeExecuteMetaTransaction('functionData', meta.args[0] as string, meta.args[1] as Hex, ('0x' + '22'.repeat(65)) as Hex)
}
const evidenceOf = (tx: { to: string | null; input: Hex } | null, status: 'success' | 'reverted' | null) => ({
  transaction: async () => tx,
  receipt: async () => (status ? { transactionHash: '0x' + 'e'.repeat(64), status, blockNumber: 1n } : null)
})

describe('which transaction is an in-world split purchase', () => {
  it('is bought only by a mined, successful execution of exactly its terms; reverted only in the very bytes it dispatched', async () => {
    const mine = relayed()
    const dispatched = { ...sale, envelope: splitEnvelope(mine) }
    expect(await splitExecution(evidenceOf({ to: MANAGER, input: mine }, 'success') as never, dispatched, '0xabc')).toBe('bought')
    expect(await splitExecution(evidenceOf({ to: MANAGER, input: mine }, 'reverted') as never, dispatched, '0xabc')).toBe('reverted')
  })

  it.each([
    ['the same call signed again (another outer signature)', resigned],
    ['the same terms built again (another external-call salt)', () => relayed()],
    ['the same terms with another signature and salt', () => relayed({ signature: ('0x' + '33'.repeat(65)) as Hex })]
  ])('a revert of %s is not its own: unknown, never reverted; a success of it still buys', async (_, other) => {
    const mine = relayed()
    const dispatched = { ...sale, envelope: splitEnvelope(mine) }
    const input = other(mine)
    expect(input).not.toBe(mine)
    expect(await splitExecution(evidenceOf({ to: MANAGER, input }, 'reverted') as never, dispatched, '0xabc')).toBe('unknown')
    expect(await splitExecution(evidenceOf({ to: MANAGER, input }, 'success') as never, dispatched, '0xabc')).toBe('bought')
  })

  it('a group paid by its credits: its credit at its own value under the signed cap, and nothing from the wallet', async () => {
    // The node's credit carries headroom over the cap it signed; the call pulls no MANA.
    const V = 1_200_000_000_000_000_000n
    const held: SpendableCredit = { ...credit, amount: V.toString(), availableAmount: V.toString() }
    const built = buildStoreUseCreditsArgs(STORE, [item], BUYER, [held], C.toString())
    expect(built.maxUncreditedValue).toBe(0n)
    const input = encodeExecuteMetaTransaction('functionData', BUYER, encodeUseCredits(built), ('0x' + '11'.repeat(65)) as Hex)
    const group: SplitSale = { ...sale, creditWei: C.toString(), creditValueWei: V.toString(), gapWei: '0' }
    expect(isSplitSale(group)).toBe(true)
    expect(await splitExecution(evidenceOf({ to: MANAGER, input }, 'success') as never, group, '0xabc')).toBe('bought')
    // Read with the cap as the credit's value, the same call carries other terms.
    const { creditValueWei: _, ...capped } = group
    expect(await splitExecution(evidenceOf({ to: MANAGER, input }, 'success') as never, capped, '0xabc')).toBe('other')
    expect(isSplitSale({ ...group, creditValueWei: '1.5' })).toBe(false)
  })

  it('with no envelope kept, no revert is its own', async () => {
    expect(await splitExecution(evidenceOf({ to: MANAGER, input: relayed() }, 'reverted') as never, sale, '0xabc')).toBe('unknown')
    expect(await splitExecution(evidenceOf({ to: MANAGER, input: relayed() }, 'success') as never, sale, '0xabc')).toBe('bought')
  })

  /** This buyer's relayed useCredits spending this credit together with `with` in one call (a batch). */
  function batched(withCredit: SpendableCredit): Hex {
    const built = buildStoreUseCreditsArgs(STORE, [item], BUYER, [credit, withCredit], C.toString(), { maxUncreditedValue: G })
    return encodeExecuteMetaTransaction('functionData', BUYER, encodeUseCredits(built), ('0x' + '11'.repeat(65)) as Hex)
  }

  it.each([
    ['another call (another price)', relayed({ items: [{ ...item, priceWei: '1' }] })],
    ['the same credit spent on another call (another item)', relayed({ items: [{ ...item, itemId: '8' }] })],
    ['another gap', relayed({ gap: G + 1n })],
    ['crediting another share', relayed({ credited: C - 1n })],
    ['the same arguments at another venue', relayed({ venue: BOOK.marketplace.contract })],
    ['the same arguments for another function', relayed({ selector: '0x12345678' })],
    ['this credit in a batch with another', batched({ ...credit, id: '0x' + 'b2'.repeat(32) })]
  ])('is not bought by the buyer spending this credit on %s: that is the credit gone elsewhere', async (_, input) => {
    expect(await splitExecution(evidenceOf({ to: MANAGER, input }, 'success') as never, sale, '0xabc')).toBe('other')
  })

  it.each([
    ['reverted', 'reverted' as const],
    ['not mined yet', null]
  ])('takes the buyer spending this credit on other terms as spent elsewhere only once it succeeded: %s, it is only not this sale', async (_, status) => {
    const input = relayed({ items: [{ ...item, itemId: '8' }] })
    expect(await splitExecution(evidenceOf({ to: MANAGER, input }, status) as never, sale, '0xabc', { sleep: async () => {} })).toBe('foreign')
    expect(await splitExecution(evidenceOf({ to: MANAGER, input }, 'success') as never, sale, '0xabc')).toBe('other')
  })

  it.each([
    ['another credit', relayed({ credit: { ...credit, id: '0x' + 'b2'.repeat(32) } })],
    ['another sender', relayed({ user: '0x' + 'cc'.repeat(20) })],
    ['another sender spending another credit', relayed({ user: '0x' + 'cc'.repeat(20), credit: { ...credit, id: '0x' + 'b2'.repeat(32) } })]
  ])('reads a useCredits that doesn\'t spend this credit for this buyer (%s) as foreign: not this sale, nor where its credit went', async (_, input) => {
    expect(await splitExecution(evidenceOf({ to: MANAGER, input }, 'success') as never, sale, '0xabc')).toBe('foreign')
  })

  it('says nothing of a transaction to another contract, or one it can\'t read: its destination alone isn\'t where the credit went', async () => {
    // Read, but not as a useCredits at the manager: unreadable, which is no answer either way (never bought, never other).
    expect(await splitExecution(evidenceOf({ to: STORE, input: relayed() }, 'success') as never, sale, '0xabc')).toBe('unreadable')
    expect(await splitExecution(evidenceOf({ to: MANAGER, input: '0xdeadbeef' }, 'success') as never, sale, '0xabc')).toBe('unreadable')
    // Nothing read at all is unknown.
    expect(await splitExecution(evidenceOf(null, 'success') as never, sale, '0xabc', { sleep: async () => {} })).toBe('unknown')
  })

  it('reads a useCredits the buyer\'s own wallet sent (the web Shop\'s fallback) by the transaction\'s sender', async () => {
    const built = buildStoreUseCreditsArgs(STORE, [item], BUYER, [credit], C.toString(), { maxUncreditedValue: G })
    const direct = encodeUseCredits(built)
    const evidence = (from: string | undefined, status: 'success' | 'reverted') => ({
      transaction: async () => ({ to: MANAGER, input: direct, ...(from !== undefined ? { from } : {}) }),
      receipt: async () => ({ transactionHash: '0x' + 'e'.repeat(64), status, blockNumber: 1n })
    })
    expect(await splitExecution(evidence(BUYER.toUpperCase().replace('0X', '0x'), 'success') as never, sale, '0xabc')).toBe('bought')
    // Its revert, in the call the wallet was handed: the envelope is that call's own.
    expect(await splitExecution(evidence(BUYER, 'reverted') as never, { ...sale, envelope: splitEnvelope(direct) }, '0xabc')).toBe('reverted')
    expect(await splitExecution(evidence(BUYER, 'reverted') as never, { ...sale, envelope: splitEnvelope(relayed()) }, '0xabc')).toBe('unknown')
    // Sent by another account: it doesn't spend this buyer's credit.
    expect(await splitExecution(evidence('0x' + 'cc'.repeat(20), 'success') as never, sale, '0xabc')).toBe('foreign')
    // A reader that doesn't give the sender can't tell whose it is: unreadable, no answer either way.
    expect(await splitExecution(evidence(undefined, 'success') as never, sale, '0xabc')).toBe('unreadable')
  })

  it('gives a transaction its receipt ran ahead of one more look, a moment later: a mined purchase reads as bought', async () => {
    const mine = relayed()
    let reads = 0
    const evidence = {
      transaction: async () => (reads++ === 0 ? null : { to: MANAGER, input: mine }),
      receipt: async () => ({ transactionHash: '0x' + 'e'.repeat(64), status: 'success' as const, blockNumber: 1n })
    }
    const waits: number[] = []
    const sleep = async (ms: number) => void waits.push(ms)
    expect(await splitExecution(evidence as never, sale, '0xabc', { sleep })).toBe('bought')
    expect(waits).toEqual([SPLIT_TX_RETRY_MS])
    // Still missing after that one look: nothing said. With no receipt either, no look at all.
    waits.length = 0
    expect(await splitExecution(evidenceOf(null, 'success') as never, sale, '0xabc', { sleep })).toBe('unknown')
    expect(waits).toHaveLength(1)
    waits.length = 0
    expect(await splitExecution(evidenceOf(null, null) as never, sale, '0xabc', { sleep })).toBe('unknown')
    expect(waits).toHaveLength(0)
  })

  it('says nothing yet without a transaction, without a receipt for its own, or when a read fails', async () => {
    expect(await splitExecution(evidenceOf(null, 'success') as never, sale, '0xabc', { sleep: async () => {} })).toBe('unknown')
    expect(await splitExecution(evidenceOf({ to: MANAGER, input: relayed() }, null) as never, sale, '0xabc')).toBe('unknown')
    const failing = { transaction: async () => Promise.reject(new Error('rpc down')), receipt: async () => null }
    expect(await splitExecution(failing as never, sale, '0xabc')).toBe('unknown')
  })

  it('reads its kept terms back only when whole', () => {
    expect(isSplitSale(sale)).toBe(true)
    expect(isSplitSale({ ...sale, gapWei: '-1' })).toBe(false)
    expect(isSplitSale({ ...sale, creditId: '' })).toBe(false)
    expect(isSplitSale({ ...sale, envelope: splitEnvelope(relayed()) })).toBe(true)
    // The issued credit's expiry, kept with the sale: whole unix seconds, or none (a sale kept before it was).
    expect(isSplitSale({ ...sale, creditExpiresAt: 1_700_000_000 })).toBe(true)
    expect(isSplitSale({ ...sale, creditExpiresAt: 1.5 })).toBe(false)
    expect(isSplitSale({ ...sale, creditExpiresAt: '1700000000' })).toBe(false)
    expect(isSplitSale({ ...sale, envelope: '0x1234' })).toBe(false)
    expect(isSplitSale({ ...sale, envelope: null })).toBe(false)
    expect(carriesSale({ to: MANAGER, input: relayed() }, sale)).toBe(true)
  })
})

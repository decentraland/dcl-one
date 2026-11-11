// The purchase guard's acceptance and rejection vectors, for every guard that has to agree with
// shell/shop/walletGuard: services/catalyst/tests/fixtures/shop/purchase-guard-vectors.json. A vector
// is a confirmed purchase (the intent, as data), what the guard is asked to sign or send, and whether it
// signs or refuses, at one fixed clock. This test builds them with the purchase rails' own encoders,
// runs each through walletGuard's checks, and holds the committed file to exactly what it built:
// changing a rule without the file, or the file without the rule, fails here.
// `UPDATE_PURCHASE_GUARD_VECTORS=1` rewrites the file.
//
// The vectors cover the checks on the payload only (checkMetaTransaction, checkTransaction), against an
// intent the host already trusts. The session's (the login still current, the wallet's account, the step
// kept durably) are the host's, tested where each host keeps them. So are the host's own gates (chain,
// manager, account kind, method): a vector a guard accepts (one on chain 137, a direct send) obliges no
// host to enable that case. Another guard has to agree on the outcome only, and only a refusal counts
// as refusing: any other error fails the run (in Rust, a panic is no refusal).

import { readFileSync, writeFileSync } from 'node:fs'
import { join } from 'node:path'
import { afterAll, beforeAll, describe, expect, it, vi } from 'vitest'
import type { Hex } from 'viem'
import { decodeAbiParameters, encodeAbiParameters, encodeFunctionData, getAbiItem, parseAbi } from 'viem'
import { COLLECTION_STORE_ABI } from '@shop/checkout/chain/abi'
import { BOOK, CREATOR_COUPON_ADDRESS, ITEM } from '@shop/payment/fixtures'
import { metaTxTypedData } from '../../shop/checkout/chain/metatx'
import { creatorTrade, resaleTrade } from '../../shop/checkout/fakes'
import { getCouponManagerForTrade } from '../../shop/checkout/lib/marketplace'
import { buildStoreUseCreditsArgs, buildUseCreditsArgs, encodeUseCredits, type EnvelopeOptions, type ListingCoupon, type SpendableCredit, type StoreItemToBuy, type UseCreditsArgs } from '../../shop/checkout/purchase/encoding'
import { checkMetaTransaction, checkTransaction, type PurchaseIntent, WalletRefusal } from '../shell/shop/walletGuard'

const FILE = join(__dirname, '../../../../../services/catalyst/tests/fixtures/shop/purchase-guard-vectors.json')
/** 2026-01-01T00:00:00Z: every vector is built and checked at this clock. */
const NOW_S = 1_767_225_600
const ENVELOPE: EnvelopeOptions = { now: () => NOW_S * 1000, salt: () => ('0x' + '5a'.repeat(32)) as Hex }

const BUYER = BOOK.buyer.toLowerCase()
const OTHER = '0x' + 'cc'.repeat(20)
const MANAGER = '0x' + 'ce'.repeat(20)
const STORE = BOOK.store.contract.toLowerCase()
const MARKETPLACE = BOOK.marketplace.contract.toLowerCase()
const CHAIN = 137
const STORE_BUY = getAbiItem({ abi: COLLECTION_STORE_ABI, name: 'buy' })

/** The intent as data: what a host confirmed, without its session callbacks. */
type IntentData = Omit<PurchaseIntent, 'current' | 'durable'>
type Vector =
  | { name: string; kind: 'metaTransaction'; intent: IntentData; from: string; data: unknown; expect: 'sign' | 'refuse'; refusal?: string }
  | { name: string; kind: 'transaction'; intent: IntentData; tx: Record<string, unknown>; expect: 'send' | 'refuse'; refusal?: string }

function buildVectors(): Vector[] {
  const credit: SpendableCredit = { id: '0x' + 'a1'.repeat(32), amount: '3000000000000000000', availableAmount: '3000000000000000000', expiresAt: NOW_S + 900, signature: '0x' + 'cc'.repeat(65), contract: MANAGER }
  const item: StoreItemToBuy = { collection: BOOK.collection, itemId: String(ITEM), priceWei: '2300000000000000000' }
  const other: StoreItemToBuy = { collection: OTHER, itemId: '8', priceWei: '1000000000000000000' }
  const storeArgs = (over: Partial<UseCreditsArgs> = {}, items = [item], buyer = BUYER): UseCreditsArgs => ({ ...buildStoreUseCreditsArgs(STORE, items, buyer, [credit], credit.amount, ENVELOPE), ...over })
  const marketArgs = (trades = [creatorTrade()], venue = MARKETPLACE, buyer = BUYER, coupons?: ListingCoupon[]): UseCreditsArgs =>
    buildUseCreditsArgs(venue, trades, buyer, [credit], credit.amount, coupons, ENVELOPE)
  const coupon = {
    id: 'sale-1',
    signer: BOOK.creator,
    couponManager: getCouponManagerForTrade(creatorTrade())!,
    couponAddress: CREATOR_COUPON_ADDRESS,
    checks: { uses: 10, expiration: NOW_S * 1000 + 86_400_000, effective: NOW_S * 1000 - 1_000, salt: '0x00', contractSignatureIndex: 0, signerSignatureIndex: 0, allowedRoot: '0x', externalChecks: [] },
    discountType: 1,
    discount: 100_000,
    root: '0x' + '00'.repeat(32),
    signature: '0x' + 'bb'.repeat(65),
    proof: []
  } as unknown as ListingCoupon
  const accept = { sale: { venue: MARKETPLACE, call: 'accept' as const } }
  const withCoupon = { sale: { venue: MARKETPLACE, call: 'acceptWithCoupon' as const } }
  const expiring = (expiresAt: number) => typed(encodeUseCredits({ ...storeArgs(), externalCall: { ...storeArgs().externalCall, expiresAt: BigInt(expiresAt) } }))
  const intent = (over: Partial<IntentData> = {}): IntentData => ({
    buyer: BUYER,
    chainId: CHAIN,
    manager: { chainId: CHAIN, address: MANAGER, name: 'Decentraland Credits', version: '1.0.0' },
    units: [{ contract: BOOK.collection.toLowerCase(), itemId: String(ITEM) }],
    sale: { venue: STORE, call: 'storeBuy' },
    ...over
  })
  const typed = (functionData: Hex = encodeUseCredits(storeArgs()), from = BUYER) =>
    JSON.parse(JSON.stringify(metaTxTypedData({ address: MANAGER as Hex, chainId: CHAIN, name: 'Decentraland Credits', version: '1.0.0', metaTx: 'functionData' } as never, from, 0n, functionData))) as Record<string, Record<string, unknown>>
  const signs = (name: string, data: unknown, over: Partial<IntentData> = {}): Vector => ({ name, kind: 'metaTransaction', intent: intent(over), from: BUYER, data, expect: 'sign' })
  const refuses = (name: string, data: unknown, over: Partial<IntentData> = {}, from = BUYER): Vector => ({ name, kind: 'metaTransaction', intent: intent(over), from, data, expect: 'refuse' })

  const units = [
    { contract: BOOK.collection.toLowerCase(), itemId: String(ITEM) },
    { contract: OTHER, itemId: '8' },
    { contract: OTHER, itemId: '8' }
  ]
  const unit = { contract: BOOK.collection.toLowerCase(), itemId: String(ITEM) }
  const decodeStoreItems = (args: UseCreditsArgs) => [...decodeAbiParameters(STORE_BUY.inputs, args.externalCall.data as Hex)[0]]
  const mine = buildStoreUseCreditsArgs(STORE, [item], BUYER, [credit], credit.amount, ENVELOPE)
  const theirs = buildStoreUseCreditsArgs(STORE, [other], OTHER, [credit], credit.amount, ENVELOPE)
  const mixed = { ...mine, externalCall: { ...mine.externalCall, data: encodeAbiParameters(STORE_BUY.inputs, [[decodeStoreItems(mine)[0], decodeStoreItems(theirs)[0]]]) } }
  const t = typed()
  const data = encodeUseCredits(storeArgs())
  const send = (name: string, tx: Record<string, unknown>, expect: 'send' | 'refuse'): Vector => ({ name, kind: 'transaction', intent: intent(), tx, expect })

  return [
    signs('the confirmed store mint: the manager’s meta-transaction, one credit, the item for the buyer', t),
    signs('the confirmed creator listing through its marketplace', typed(encodeUseCredits(marketArgs())), { sale: { venue: MARKETPLACE, call: 'accept' } }),
    signs('a cart group: one mint per confirmed unit, in any order', typed(encodeUseCredits(storeArgs({}, [other, item, other]))), { units }),
    signs('a cart group: one trade per confirmed unit through the marketplace', typed(encodeUseCredits(marketArgs([creatorTrade(), creatorTrade()]))), { sale: { venue: MARKETPLACE, call: 'accept' }, units: [unit, unit] }),
    signs('the confirmed creator listing with its coupon', typed(encodeUseCredits(marketArgs([creatorTrade()], MARKETPLACE, BUYER, [coupon]))), withCoupon),
    signs('an external call that expires exactly at the guard’s policy', expiring(NOW_S + 2 * 86400)),

    refuses('another domain name', { ...t, domain: { ...t.domain, name: 'Decentraland MANA' } }),
    refuses('another domain version', { ...t, domain: { ...t.domain, version: '2' } }),
    refuses('another manager', { ...t, domain: { ...t.domain, verifyingContract: OTHER } }),
    refuses('another chain', { ...t, domain: { ...t.domain, salt: '0x' + '0'.repeat(63) + '1' } }),
    refuses('a chainId beside the salt', { ...t, domain: { ...t.domain, chainId: 1 } }),
    refuses('no declared types', { ...t, types: {} }),
    refuses('a changed MetaTransaction type', { ...t, types: { ...t.types, MetaTransaction: [{ name: 'nonce', type: 'uint256' }, { name: 'from', type: 'address' }] } }),
    refuses('the MetaTransaction fields in another order', { ...t, types: { ...t.types, MetaTransaction: [{ name: 'from', type: 'address' }, { name: 'nonce', type: 'uint256' }, { name: 'functionData', type: 'bytes' }] } }),
    refuses('another primary type', { ...t, primaryType: 'Permit' }),
    refuses('an extra message field', { ...t, message: { ...t.message, value: '1' } }),
    refuses('a message from someone else', { ...t, message: { ...t.message, from: OTHER } }),
    refuses('a signature asked for another account', t, {}, OTHER),

    refuses('a call other than useCredits', typed(encodeFunctionData({ abi: parseAbi(['function getNonce(address _signer) view returns (uint256)']), functionName: 'getNonce', args: [BUYER as Hex] }))),
    refuses('two credits', typed(encodeUseCredits({ ...storeArgs(), credits: [...storeArgs().credits, ...storeArgs().credits], creditsSignatures: [credit.signature as Hex, credit.signature as Hex] }))),
    refuses('MANA from the wallet', typed(encodeUseCredits(storeArgs({ maxUncreditedValue: 1n })))),
    refuses('more than the credit holds', typed(encodeUseCredits(storeArgs({ maxCreditedValue: 4n * 10n ** 18n })))),
    refuses('a custom call', typed(encodeUseCredits(storeArgs({ customExternalCallSignature: '0x1234' })))),
    refuses('another venue', typed(encodeUseCredits({ ...storeArgs(), externalCall: { ...storeArgs().externalCall, target: OTHER as Hex } }))),
    refuses('another item', typed(encodeUseCredits(storeArgs({}, [{ ...item, itemId: '8' }])))),
    refuses('another collection', typed(encodeUseCredits(storeArgs({}, [{ ...item, collection: OTHER }])))),
    refuses('someone else as the beneficiary', typed(encodeUseCredits(storeArgs({}, [item], OTHER)))),
    refuses('two items for one confirmed', typed(encodeUseCredits(storeArgs({}, [item, item])))),
    refuses('externalCall.expiresAt exceeds the guard’s policy', expiring(NOW_S + 365 * 86400)),
    refuses('externalCall.expiresAt one second past the guard’s policy', expiring(NOW_S + 2 * 86400 + 1)),
    refuses('calldata it can’t decode', typed('0xdeadbeef')),
    refuses('a listing when the confirmed sale was the store’s mint', typed(encodeUseCredits(marketArgs([creatorTrade()], STORE)))),
    refuses('an empty confirmation', t, { units: [] }),

    refuses('a cart group one unit short', typed(encodeUseCredits(storeArgs({}, [item, other]))), { units }),
    refuses('a cart group one unit more', typed(encodeUseCredits(storeArgs({}, [item, other, other, other]))), { units }),
    refuses('a cart group with another unit in place of one', typed(encodeUseCredits(storeArgs({}, [item, other, { ...other, itemId: '9' }]))), { units }),
    refuses('a cart group with one of an item for two of another', typed(encodeUseCredits(storeArgs({}, [item, item, other]))), { units }),
    refuses('a cart group with a unit for someone else among the buyer’s', typed(encodeUseCredits(mixed)), { units: units.slice(0, 2) }),
    refuses('a cart group of one trade for two confirmed units', typed(encodeUseCredits(marketArgs([creatorTrade()]))), { sale: { venue: MARKETPLACE, call: 'accept' }, units: [unit, unit] }),

    refuses('a listing that sends the item to someone else', typed(encodeUseCredits(marketArgs([creatorTrade()], MARKETPLACE, OTHER))), accept),
    refuses('a listing that sends two assets', typed(encodeUseCredits(marketArgs([creatorTrade({ sent: [...creatorTrade().sent, ...creatorTrade().sent] })]))), accept),
    refuses('a resale of a minted token instead of the confirmed item', typed(encodeUseCredits(marketArgs([resaleTrade()]))), accept),
    refuses('a listing with a coupon when the confirmed sale had none', typed(encodeUseCredits(marketArgs([creatorTrade()], MARKETPLACE, BUYER, [coupon]))), accept),
    refuses('a coupon listing that sends the item to someone else', typed(encodeUseCredits(marketArgs([creatorTrade()], MARKETPLACE, OTHER, [coupon]))), withCoupon),

    send('the same call sent directly: to the manager, from the buyer, no value, fee fields allowed', { from: BUYER, to: MANAGER, data, maxFeePerGas: '0x1' }, 'send'),
    send('a direct send from someone else', { from: OTHER, to: MANAGER, data }, 'refuse'),
    send('a direct send to another contract', { from: BUYER, to: OTHER, data }, 'refuse'),
    send('a direct send with value', { from: BUYER, to: MANAGER, data, value: '0x1' }, 'refuse'),
    send('a direct send with a field it does not know', { from: BUYER, to: MANAGER, data, nonce: '0x1' }, 'refuse'),
    send('a direct send taking MANA from the wallet', { from: BUYER, to: MANAGER, data: encodeUseCredits(storeArgs({ maxUncreditedValue: 1n })) }, 'refuse')
  ]
}

/**
 * What walletGuard answers for a vector: its outcome, and the refusal's words when it refuses. Only a
 * WalletRefusal is a refusal; any other error is rethrown, so a broken check fails the run instead of
 * being written down as a vector it refuses.
 */
function judge(v: Vector): { expect: Vector['expect']; refusal?: string } {
  const full: PurchaseIntent = { ...v.intent, current: () => true }
  try {
    if (v.kind === 'metaTransaction') checkMetaTransaction(v.from, JSON.stringify(v.data), full)
    else checkTransaction(v.tx, full)
    return { expect: v.kind === 'metaTransaction' ? 'sign' : 'send' }
  } catch (e) {
    if (!(e instanceof WalletRefusal)) throw e
    return { expect: 'refuse', refusal: e.message }
  }
}

/** The vectors as walletGuard judges them, each held to the outcome it was built for. */
function judged(vectors: Vector[]): Vector[] {
  return vectors.map((v) => {
    const verdict = judge(v)
    expect(verdict.expect, v.name).toBe(v.expect)
    return verdict.refusal ? { ...v, refusal: verdict.refusal } : v
  })
}

describe('the purchase guard’s shared vectors', () => {
  beforeAll(() => {
    vi.useFakeTimers({ toFake: ['Date'] })
    vi.setSystemTime(NOW_S * 1000)
  })
  afterAll(() => vi.useRealTimers())

  it('agree with walletGuard on every vector, and the committed file holds exactly them', () => {
    const vectors = judged(buildVectors())
    expect(vectors.filter((v) => v.expect !== 'refuse').length).toBeGreaterThan(0)
    expect(vectors.filter((v) => v.expect === 'refuse').length).toBeGreaterThan(0)
    const file = JSON.stringify(
      {
        $comment:
          'Generated by apps/ui3/src/play/test/purchaseGuardVectors.test.ts (UPDATE_PURCHASE_GUARD_VECTORS=1). Every guard of an in-world purchase agrees with these at `now` on the outcome (sign, send or refuse) of checking a payload against an intent it already trusts; `refusal` is the TypeScript guard’s message, for reference. Only a refusal counts as refusing: any other error, or a panic, is a failure.',
        now: NOW_S,
        vectors
      },
      null,
      2
    ) + '\n'
    if (process.env.UPDATE_PURCHASE_GUARD_VECTORS === '1') writeFileSync(FILE, file)
    expect(readFileSync(FILE, 'utf8')).toBe(file)
  })

  it('fails on an error that is no refusal, instead of writing it down as a vector it refuses', () => {
    const refused = buildVectors().find((v) => v.name === 'another domain version')
    if (refused?.kind !== 'metaTransaction') throw new Error('the vector is gone')
    // With no manager in the intent, the domain check reads a field of undefined: a TypeError, not a refusal.
    const broken: Vector = { ...refused, intent: { ...refused.intent, manager: undefined as never } }
    expect(() => judged([broken])).toThrow(TypeError)
  })
})

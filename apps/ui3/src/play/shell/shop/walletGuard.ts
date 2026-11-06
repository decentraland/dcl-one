// The buyer's wallet as ONE confirmed in-world purchase may use it. Bound to that purchase's intent (the
// login's account, the settlement chain, the trusted CreditsManager, the item and where its primary sale
// settles), it forwards only what paying that item with one credit takes, and decodes it first:
//
//  - the account and the chain, and a switch to that chain;
//  - the CreditsManager's meta-transaction, in its exact EIP-712 schema (domain name, version, the
//    manager, the chain as the domain's salt; MetaTransaction{nonce, from, functionData}) and nothing
//    more, for the buyer;
//  - or the same call sent directly from the buyer, with no value;
//  - and in both, `useCredits` itself: one credit, no MANA from the wallet, no more spent than the credit
//    holds, no custom call, and an external call to the sale's own venue that mints or accepts exactly the
//    confirmed item for the buyer.
//
// Before each signature or transaction it reads the wallet's selected account and checks the login is
// still the one confirmed. Anything else is refused before it reaches the wallet.

import { decodeAbiParameters, decodeFunctionData, getAbiItem, toFunctionSelector, type Hex } from 'viem'
import type { Eip1193Provider } from '../../../data/auth/wallet'
import { COLLECTION_STORE_ABI, CREDITS_MANAGER_ABI, MARKETPLACE_ABI } from '../../../shop/checkout/chain/abi'
import { chainIdSalt, DOMAIN_TYPE, metaTxType } from '../../../shop/checkout/chain/metatx'
import { ASSET_TYPE } from '../../../shop/checkout/purchase/encoding'
import type { ChainId } from '../../../shop/model/chain'
import type { TrustedManager } from './config'

/** A refused request, in EIP-1193's terms: 4100 unauthorized, 4200 unsupported. */
export class WalletRefusal extends Error {
  override name = 'WalletRefusal'
  constructor(
    message: string,
    readonly code: 4100 | 4200
  ) {
    super(message)
  }
}

export interface PurchaseIntent {
  buyer: string
  chainId: number
  manager: TrustedManager
  item: { contract: string; itemId: string }
  /** The confirmed sale's venue and call: the CollectionStore's buy, or a marketplace's accept. */
  sale: { venue: string; call: 'storeBuy' | 'accept' | 'acceptWithCoupon' }
  /** False once the login or the wallet changed since the buyer confirmed. */
  current(): boolean
}

const STORE_BUY = getAbiItem({ abi: COLLECTION_STORE_ABI, name: 'buy' })
const ACCEPT = getAbiItem({ abi: MARKETPLACE_ABI, name: 'accept' })
const ACCEPT_WITH_COUPON = getAbiItem({ abi: MARKETPLACE_ABI, name: 'acceptWithCoupon' })
const SELECTOR = { storeBuy: toFunctionSelector(STORE_BUY), accept: toFunctionSelector(ACCEPT), acceptWithCoupon: toFunctionSelector(ACCEPT_WITH_COUPON) }
const META_TX_TYPE = metaTxType('functionData')
const TX_FIELDS = new Set(['from', 'to', 'data', 'value', 'gas', 'maxFeePerGas', 'maxPriorityFeePerGas'])
/** The external call's own expiry, as the rails set it (a day), with room for a slow clock. */
const MAX_CALL_LIFETIME_S = 2 * 24 * 60 * 60

const lower = (v: unknown): string => (typeof v === 'string' ? v.toLowerCase() : '')
const short = (a: string): string => `${a.slice(0, 6)}…${a.slice(-4)}`
const sameKeys = (o: unknown, keys: string[]): boolean =>
  o != null && typeof o === 'object' && !Array.isArray(o) && Object.keys(o).length === keys.length && keys.every((k) => k in o)
const sameFields = (fields: unknown, expected: readonly { name: string; type: string }[]): boolean =>
  Array.isArray(fields) &&
  fields.length === expected.length &&
  fields.every((f, i) => sameKeys(f, ['name', 'type']) && (f as { name: string }).name === expected[i].name && (f as { type: string }).type === expected[i].type)

function refuse(why: string): never {
  throw new WalletRefusal(why, 4100)
}

/** `useCredits` paying exactly the confirmed item, with one credit and nothing from the wallet. */
export function checkUseCredits(data: unknown, intent: PurchaseIntent, nowSeconds = Math.floor(Date.now() / 1000)): void {
  if (typeof data !== 'string' || !/^0x[0-9a-fA-F]*$/.test(data)) refuse('The Shop only signs its own purchases.')
  let decoded
  try {
    decoded = decodeFunctionData({ abi: CREDITS_MANAGER_ABI, data: data as Hex })
  } catch {
    refuse('The Shop only signs its own purchases.')
  }
  if (decoded.functionName !== 'useCredits') refuse('The Shop only signs its own purchases.')
  const args = (decoded.args as readonly unknown[])[0] as {
    credits: readonly { value: bigint }[]
    creditsSignatures: readonly Hex[]
    externalCall: { target: Hex; selector: Hex; data: Hex; expiresAt: bigint }
    customExternalCallSignature: Hex
    maxUncreditedValue: bigint
    maxCreditedValue: bigint
  }
  if (args.credits.length !== 1 || args.creditsSignatures.length !== 1) refuse('A purchase spends one credit.')
  if (args.maxUncreditedValue !== 0n) refuse('A Credits purchase takes no MANA from your wallet.')
  if (args.maxCreditedValue > args.credits[0].value) refuse('The purchase would spend more than its credit holds.')
  if (args.customExternalCallSignature !== '0x') refuse('The Shop makes no custom calls.')
  const call = args.externalCall
  if (lower(call.target) !== intent.sale.venue) refuse('The purchase goes somewhere other than the item’s sale.')
  if (call.expiresAt > BigInt(nowSeconds + MAX_CALL_LIFETIME_S)) refuse('The purchase would stay valid too long.')
  if (lower(call.selector) !== SELECTOR[intent.sale.call]) refuse('The purchase is not the item’s sale.')
  const { contract, itemId } = intent.item
  const buyer = intent.buyer
  try {
    if (intent.sale.call === 'storeBuy') {
      const [items] = decodeAbiParameters(STORE_BUY.inputs, call.data) as unknown as [{ collection: Hex; ids: readonly bigint[]; prices: readonly bigint[]; beneficiaries: readonly Hex[] }[]]
      const ok =
        items.length === 1 &&
        lower(items[0].collection) === contract &&
        items[0].ids.length === 1 &&
        items[0].ids[0] === BigInt(itemId) &&
        items[0].prices.length === 1 &&
        items[0].beneficiaries.length === 1 &&
        lower(items[0].beneficiaries[0]) === buyer
      if (!ok) refuse('The purchase is not the confirmed item.')
      return
    }
    const inputs = intent.sale.call === 'accept' ? ACCEPT.inputs : ACCEPT_WITH_COUPON.inputs
    const [trades] = decodeAbiParameters(inputs, call.data) as unknown as [{ sent: readonly { assetType: bigint; contractAddress: Hex; value: bigint; beneficiary: Hex }[] }[]]
    const sent = trades.length === 1 ? trades[0].sent : []
    const ok =
      sent.length === 1 &&
      sent[0].assetType === BigInt(ASSET_TYPE.COLLECTION_ITEM) &&
      lower(sent[0].contractAddress) === contract &&
      sent[0].value === BigInt(itemId) &&
      lower(sent[0].beneficiary) === buyer
    if (!ok) refuse('The purchase is not the confirmed item.')
  } catch (e) {
    if (e instanceof WalletRefusal) throw e
    refuse('The Shop only signs its own purchases.')
  }
}

/** The CreditsManager's meta-transaction for the buyer, exactly as the rails build it, and nothing more. */
export function checkMetaTransaction(from: unknown, data: unknown, intent: PurchaseIntent): void {
  if (lower(from) !== intent.buyer) refuse('The Shop signs only for the account you signed in with.')
  let typed: unknown
  try {
    typed = typeof data === 'string' ? JSON.parse(data) : data
  } catch {
    refuse('The Shop could not read what it was asked to sign.')
  }
  if (!sameKeys(typed, ['types', 'domain', 'primaryType', 'message'])) refuse('The Shop only signs its own purchases.')
  const { types, domain, primaryType, message } = typed as Record<string, Record<string, unknown>>
  const { manager } = intent
  const ok =
    sameKeys(types, ['EIP712Domain', 'MetaTransaction']) &&
    sameFields(types.EIP712Domain, DOMAIN_TYPE) &&
    sameFields(types.MetaTransaction, META_TX_TYPE) &&
    sameKeys(domain, ['name', 'version', 'verifyingContract', 'salt']) &&
    domain.name === manager.name &&
    domain.version === manager.version &&
    lower(domain.verifyingContract) === manager.address &&
    lower(domain.salt) === lower(chainIdSalt(intent.chainId as ChainId)) &&
    (primaryType as unknown) === 'MetaTransaction' &&
    sameKeys(message, ['nonce', 'from', 'functionData']) &&
    /^\d{1,78}$/.test(String(message.nonce)) &&
    lower(message.from) === intent.buyer
  if (!ok) refuse('The Shop only signs its own purchases.')
  checkUseCredits(message.functionData, intent)
}

/** The same call sent from the buyer's wallet: to the manager, with no value and no unknown fields. */
export function checkTransaction(tx: unknown, intent: PurchaseIntent): void {
  if (tx == null || typeof tx !== 'object' || Array.isArray(tx) || Object.keys(tx).some((k) => !TX_FIELDS.has(k))) refuse('The Shop only sends its own purchases.')
  const t = tx as { from?: unknown; to?: unknown; data?: unknown; value?: unknown }
  const value = typeof t.value === 'string' || typeof t.value === 'number' || typeof t.value === 'bigint' ? BigInt(t.value) : 0n
  if (lower(t.from) !== intent.buyer || lower(t.to) !== intent.manager.address || value !== 0n) refuse('The Shop only sends its own purchases.')
  checkUseCredits(t.data, intent)
}

/** The wallet's selected account, which must be the buyer's (a managed wallet names its own). */
export async function assertWalletAccount(provider: Eip1193Provider, buyer: string, prompt = false): Promise<void> {
  let accounts = (await provider.request({ method: 'eth_accounts' })) as unknown
  if (prompt && (!Array.isArray(accounts) || accounts.length === 0)) accounts = await provider.request({ method: 'eth_requestAccounts' })
  const first = Array.isArray(accounts) ? lower(accounts[0]) : ''
  if (first !== buyer) {
    throw new WalletRefusal(`Your wallet is using ${first ? short(first) : 'no account'}. Switch it to ${short(buyer)}, the account you signed in with, to buy.`, 4100)
  }
}

export function guardedProvider(provider: Eip1193Provider, intent: PurchaseIntent): Eip1193Provider {
  const chainHex = `0x${intent.chainId.toString(16)}`
  const ready = async (): Promise<void> => {
    if (!intent.current()) refuse('You signed in with another account meanwhile. Open the item again to buy it.')
    await assertWalletAccount(provider, intent.buyer)
  }
  return {
    async request({ method, params }) {
      const args = (Array.isArray(params) ? params : []) as unknown[]
      switch (method) {
        case 'eth_accounts':
        case 'eth_requestAccounts':
          await assertWalletAccount(provider, intent.buyer, method === 'eth_requestAccounts')
          return [intent.buyer]
        case 'eth_chainId':
          return provider.request({ method, params })
        case 'wallet_switchEthereumChain':
        case 'wallet_addEthereumChain': {
          if (lower((args[0] as { chainId?: unknown } | undefined)?.chainId) !== chainHex) refuse('The Shop only switches your wallet to its own network.')
          return provider.request({ method, params })
        }
        case 'eth_signTypedData_v4':
          checkMetaTransaction(args[0], args[1], intent)
          await ready()
          return provider.request({ method, params })
        case 'eth_sendTransaction':
          if (args.length !== 1) refuse('The Shop only sends its own purchases.')
          checkTransaction(args[0], intent)
          await ready()
          return provider.request({ method, params })
        default:
          throw new WalletRefusal(`The Shop doesn't use ${method}.`, 4200)
      }
    },
    // The wallet's own events, for the rails that follow a chain switch.
    ...(provider.on ? { on: provider.on.bind(provider) } : {}),
    ...(provider.removeListener ? { removeListener: provider.removeListener.bind(provider) } : {})
  }
}

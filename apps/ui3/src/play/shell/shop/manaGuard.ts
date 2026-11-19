// The buyer's wallet as ONE confirmed in-world purchase paid in MANA may use it: the MANA counterpart of ./walletGuard,
// which binds a Credits purchase. Bound to that purchase's intent (the login's account, the chain, and the sale's
// terms as the buyer confirmed them), it forwards only what paying them with the buyer's own MANA takes, and decodes
// it first:
//
//  - the account and the chain, and a switch to that chain;
//  - MANA's unlimited approve, only to the sale's own venue, as the web Shop grants it (once per venue). The venue
//    and its EIP-712 domain come from the chain's registry (a marketplace for listings, the CollectionStore for
//    mints), never from the request. The approve goes as MANA's meta-transaction or sent from the buyer;
//  - the sale:
//    - listings: exactly the calldata built from the confirmed terms before any request, coupons and their data
//      included. A seller's signature alone doesn't pin the price: a coupon is a separate argument, and part of it
//      is unsigned. Only MANA-priced listings: a USD-priced one has no MANA ceiling the contract enforces;
//    - mints: the CollectionStore's buy of exactly the confirmed units for the buyer, at most the confirmed total in
//      wei;
//    - as the venue's meta-transaction or sent from the buyer, with no value. A sale already allowed (an allowance
//      granted before) needs no approve at all; either way it is checked in full.
//
// Phase 1 buys with MANA on Polygon (and Amoy, its testnet) only.
//
// The intent is pinned when the guard is made: the buyer, the chain and a copy of the terms; only `current` and
// `handOff` stay live. Before each signature or transaction it checks the login is still the one confirmed and the
// wallet's selected account is the buyer's; a direct send reads the wallet's chain last, after every other await. A
// sale is forwarded only once the host has kept its attempt (`handOff`, told the rail and, for a meta-transaction, the
// nonce the checked signature names): the host holds one outstanding sale per confirmation, and an approve never
// counts as one. What reaches the wallet is what was checked: a typed-data text
// parsed once and its checked data serialized again for the pinned buyer, or a transaction snapshot built from the
// checked fields, its fees from the rails' own policy (amoyGasOverrides) and its chain bound. Anything else is
// refused before it reaches the wallet.

import { decodeAbiParameters, decodeFunctionData, getAbiItem, maxUint256, toFunctionSelector, type Hex } from 'viem'
import type { Eip1193Provider } from '../../../data/auth/wallet'
import { COLLECTION_STORE_ABI, ERC20_ABI, MARKETPLACE_ABI } from '@shop/checkout/chain/abi'
import { requireContracts, type ContractInfo } from '../../../shop/checkout/chain/contracts'
import { chainIdSalt, DOMAIN_TYPE, metaTxType } from '../../../shop/checkout/chain/metatx'
import { ASSET_TYPE } from '../../../shop/checkout/purchase/encoding'
import { amoyGasOverrides } from '../../../shop/checkout/chain/wallet'
import { CHAIN, type ChainId } from '@shop/model/chain'
import { AMOY_ADD_PARAMS } from '../../../shop/wallet/chain'
import { assertWalletAccount, sameFields, sameKeys, sameUnits, WalletRefusal } from './walletGuard'

/** What the buyer confirmed paying for in MANA: listings as their exact calldata, or mints as units under a total. */
export type ManaSaleTerms =
  | {
      kind: 'listings'
      /** The marketplace the listings name, lowercase. */
      venue: string
      /** accept / acceptWithCoupon, built from the confirmed trades and coupons (encodeMarketplaceCall). */
      calldata: Hex
      /** The confirmed units, one entry per unit, contracts lowercase. */
      units: { contract: string; itemId: string }[]
    }
  | {
      kind: 'mints'
      /** The CollectionStore, lowercase. */
      venue: string
      units: { contract: string; itemId: string }[]
      /** The confirmed total, in wei of MANA: the buy's prices add up to at most this. */
      maxWei: bigint
    }

export interface ManaPurchaseIntent {
  buyer: string
  chainId: number
  sale: ManaSaleTerms
  /** False once the login or the wallet changed since the buyer confirmed. */
  current(): boolean
  /**
   * Keeps the sale's attempt before its signature or transaction is forwarded, and holds it as this confirmation's one
   * outstanding sale. False refuses it: the attempt couldn't be kept, or another one is still outstanding. `attempt`
   * says which way it goes: as the venue's meta-transaction (`relay`, with the nonce the checked signature names) or
   * sent from the buyer's wallet (`direct`, whose nonce the wallet picks).
   */
  handOff(attempt: ManaHandOff): boolean
}

/** A sale's attempt, as the guard hands it off: what the host keeps before forwarding it. */
export type ManaHandOff = { rail: 'relay'; nonce: bigint } | { rail: 'direct' }

const APPROVE = getAbiItem({ abi: ERC20_ABI, name: 'approve' })
const STORE_BUY = getAbiItem({ abi: COLLECTION_STORE_ABI, name: 'buy' })
const SELECTOR = {
  accept: toFunctionSelector(getAbiItem({ abi: MARKETPLACE_ABI, name: 'accept' })),
  acceptWithCoupon: toFunctionSelector(getAbiItem({ abi: MARKETPLACE_ABI, name: 'acceptWithCoupon' })),
  storeBuy: toFunctionSelector(STORE_BUY),
  approve: toFunctionSelector(APPROVE)
}
const TX_FIELDS = new Set(['from', 'to', 'data', 'value', 'gas', 'maxFeePerGas', 'maxPriorityFeePerGas'])
/** The chains phase 1 buys with MANA on: Polygon, and Amoy, its testnet. */
const MANA_CHAINS: readonly number[] = [CHAIN.POLYGON, CHAIN.AMOY]

const lower = (v: unknown): string => (typeof v === 'string' ? v.toLowerCase() : '')
const isHex = (v: unknown): v is Hex => typeof v === 'string' && /^0x[0-9a-fA-F]*$/.test(v)

function refuse(why: string): never {
  throw new WalletRefusal(why, 4100)
}

/** The chain's MANA and the sale's venue, from the chain's registry: a venue it doesn't list for that sale is refused. */
export function registryFor(intent: Pick<ManaPurchaseIntent, 'chainId' | 'sale'>): { mana: ContractInfo; venue: ContractInfo } {
  if (!MANA_CHAINS.includes(intent.chainId)) refuse('The Shop doesn’t buy with MANA on this network.')
  let book
  try {
    book = requireContracts(intent.chainId as ChainId)
  } catch {
    refuse('The Shop doesn’t buy with MANA on this network.')
  }
  const venue =
    intent.sale.kind === 'listings'
      ? book.offChainMarketplaces.find((m) => lower(m.address) === lower(intent.sale.venue))
      : book.collectionStore && lower(book.collectionStore.address) === lower(intent.sale.venue)
        ? book.collectionStore
        : undefined
  if (!venue) refuse('The purchase goes somewhere other than the item’s sale.')
  return { mana: book.mana, venue }
}

/**
 * The confirmed terms themselves, before anything is asked of the wallet: listings whose calldata is a marketplace's
 * accept of exactly the confirmed units, each sending the buyer one collection item, each paid in MANA itself.
 */
export function checkManaTerms(intent: Pick<ManaPurchaseIntent, 'buyer' | 'chainId' | 'sale'>): void {
  const { mana } = registryFor(intent)
  if (intent.sale.kind === 'mints') {
    if (intent.sale.units.length === 0 || intent.sale.maxWei <= 0n) refuse('The purchase is not the confirmed item.')
    return
  }
  const data = intent.sale.calldata
  if (!isHex(data)) refuse('The purchase is not the confirmed item.')
  let decoded
  try {
    decoded = decodeFunctionData({ abi: MARKETPLACE_ABI, data })
  } catch {
    refuse('The purchase is not the confirmed item.')
  }
  if (decoded.functionName !== 'accept' && decoded.functionName !== 'acceptWithCoupon') refuse('The purchase is not the confirmed item.')
  const trades = decoded.args[0] as readonly {
    sent: readonly { assetType: bigint; contractAddress: string; value: bigint; beneficiary: string }[]
    received: readonly { assetType: bigint; contractAddress: string }[]
  }[]
  const buyer = lower(intent.buyer)
  const ok =
    trades.every((t) => t.sent.length === 1 && t.sent[0].assetType === BigInt(ASSET_TYPE.COLLECTION_ITEM) && lower(t.sent[0].beneficiary) === buyer) &&
    sameUnits(
      trades.map((t) => ({ contract: lower(t.sent[0].contractAddress), itemId: t.sent[0].value })),
      intent.sale.units
    )
  if (!ok) refuse('The purchase is not the confirmed item.')
  // MANA itself only: a USD-priced listing's MANA is set at mining, with no ceiling the contract enforces.
  if (!trades.every((t) => t.received.length > 0 && t.received.every((r) => r.assetType === BigInt(ASSET_TYPE.ERC20) && lower(r.contractAddress) === lower(mana.address)))) {
    refuse('This listing can’t be bought with MANA here.')
  }
}

/** MANA's unlimited approve, to the sale's venue only. */
export function checkApprove(data: unknown, intent: Pick<ManaPurchaseIntent, 'chainId' | 'sale'>): void {
  const { venue } = registryFor(intent)
  if (!isHex(data) || lower(data.slice(0, 10)) !== SELECTOR.approve) refuse('The Shop only approves MANA for its own purchases.')
  let spender: string
  let amount: bigint
  try {
    ;[spender, amount] = decodeAbiParameters(APPROVE.inputs, `0x${data.slice(10)}`)
  } catch {
    refuse('The Shop only approves MANA for its own purchases.')
  }
  if (lower(spender) !== lower(venue.address)) refuse('The Shop only lets the item’s sale spend your MANA.')
  if (amount !== maxUint256) refuse('The Shop only approves MANA for its own purchases.')
}

/** The sale: the confirmed listings' exact calldata, or the confirmed mints for the buyer under the confirmed total. */
export function checkSale(data: unknown, intent: Pick<ManaPurchaseIntent, 'buyer' | 'chainId' | 'sale'>): void {
  checkManaTerms(intent)
  if (!isHex(data)) refuse('The Shop only signs its own purchases.')
  const sale = intent.sale
  if (sale.kind === 'listings') {
    if (lower(data) !== lower(sale.calldata)) refuse('The purchase is not the one you confirmed.')
    return
  }
  if (lower(data.slice(0, 10)) !== SELECTOR.storeBuy) refuse('The purchase is not the item’s sale.')
  const buyer = lower(intent.buyer)
  let items
  try {
    ;[items] = decodeAbiParameters(STORE_BUY.inputs, `0x${data.slice(10)}`)
  } catch {
    refuse('The Shop only signs its own purchases.')
  }
  const ok =
    items.every((i) => i.ids.length === 1 && i.prices.length === 1 && i.beneficiaries.length === 1 && lower(i.beneficiaries[0]) === buyer) &&
    sameUnits(
      items.map((i) => ({ contract: lower(i.collection), itemId: i.ids[0] })),
      sale.units
    )
  if (!ok) refuse('The purchase is not the confirmed item.')
  const total = items.reduce((sum, i) => sum + i.prices[0], 0n)
  if (total > sale.maxWei) refuse('The purchase costs more MANA than you confirmed.')
}

type Checked = { sale: boolean }

/**
 * A meta-transaction for the buyer: MANA's approve under MANA's own domain, or the sale under its venue's, each in
 * its exact EIP-712 schema and nothing more. Returns the typed data as checked, for the wallet to sign that and not
 * the text it was read from, and whether it is the sale.
 */
export function checkMetaTransaction(from: unknown, data: unknown, intent: Pick<ManaPurchaseIntent, 'buyer' | 'chainId' | 'sale'>): Checked & { typed: unknown; nonce: bigint } {
  if (lower(from) !== lower(intent.buyer)) refuse('The Shop signs only for the account you signed in with.')
  const { mana, venue } = registryFor(intent)
  let typed: unknown
  try {
    typed = typeof data === 'string' ? JSON.parse(data) : data
  } catch {
    refuse('The Shop could not read what it was asked to sign.')
  }
  if (!sameKeys(typed, ['types', 'domain', 'primaryType', 'message'])) refuse('The Shop only signs its own purchases.')
  const { types, domain, primaryType, message } = typed as Record<string, Record<string, unknown>>
  const target = lower(domain?.verifyingContract) === lower(mana.address) ? mana : lower(domain?.verifyingContract) === lower(venue.address) ? venue : null
  if (!target || !target.metaTx) refuse('The Shop only signs its own purchases.')
  const field = target.metaTx
  const ok =
    sameKeys(types, ['EIP712Domain', 'MetaTransaction']) &&
    sameFields(types.EIP712Domain, DOMAIN_TYPE) &&
    sameFields(types.MetaTransaction, metaTxType(field)) &&
    sameKeys(domain, ['name', 'version', 'verifyingContract', 'salt']) &&
    domain.name === target.name &&
    domain.version === target.version &&
    lower(domain.salt) === lower(chainIdSalt(intent.chainId as ChainId)) &&
    (primaryType as unknown) === 'MetaTransaction' &&
    sameKeys(message, ['nonce', 'from', field]) &&
    /^\d{1,78}$/.test(String(message.nonce)) &&
    lower(message.from) === lower(intent.buyer)
  if (!ok) refuse('The Shop only signs its own purchases.')
  const nonce = BigInt(String(message.nonce))
  if (target === mana) {
    checkApprove(message[field], intent)
    return { typed, sale: false, nonce }
  }
  checkSale(message[field], intent)
  return { typed, sale: true, nonce }
}

/** A value field that is absent or exactly zero, in a shape a wallet reads one way. */
function zeroValue(value: unknown): boolean {
  if (value === undefined) return true
  if (typeof value === 'bigint') return value === 0n
  if (typeof value === 'number') return value === 0
  if (typeof value === 'string') return /^(0x0*|0+)$/i.test(value)
  return false
}

/** A fee field as a number, whatever well-formed shape a wallet reads it in; null for anything else. */
function feeOf(value: unknown): bigint | null {
  try {
    if (typeof value === 'bigint') return value
    if (typeof value === 'number' && Number.isSafeInteger(value) && value >= 0) return BigInt(value)
    if (typeof value === 'string' && /^(0x[0-9a-f]+|\d+)$/i.test(value)) return BigInt(value)
  } catch {
    // not a number
  }
  return null
}

/**
 * The same calls sent from the buyer's wallet: MANA's approve to MANA, or the sale to its venue, with no value and no
 * unknown fields. Returns the snapshot to send, built from the checked fields alone: its fees are the rails' own
 * policy (amoyGasOverrides: Amoy's priority fee, none elsewhere, the gas left to the wallet), and a request naming
 * other fees, or a gas limit, is refused.
 */
export function checkTransaction(tx: unknown, intent: Pick<ManaPurchaseIntent, 'buyer' | 'chainId' | 'sale'>): Checked & { snapshot: Record<string, unknown> } {
  if (tx == null || typeof tx !== 'object' || Array.isArray(tx) || Object.keys(tx).some((k) => !TX_FIELDS.has(k))) refuse('The Shop only sends its own purchases.')
  const t = tx as Record<string, unknown>
  if (lower(t.from) !== lower(intent.buyer) || !zeroValue(t.value)) refuse('The Shop only sends its own purchases.')
  const { mana, venue } = registryFor(intent)
  const to = lower(t.to)
  const data = t.data
  if (!isHex(data)) refuse('The Shop only sends its own purchases.')
  let sale: boolean
  if (to === lower(mana.address)) {
    checkApprove(data, intent)
    sale = false
  } else if (to === lower(venue.address)) {
    checkSale(data, intent)
    sale = true
  } else {
    refuse('The Shop only sends its own purchases.')
  }
  const policy = amoyGasOverrides(intent.chainId as ChainId) as Record<string, Hex | undefined>
  if (t.gas !== undefined) refuse('The Shop only sends its own purchases.')
  for (const k of ['maxFeePerGas', 'maxPriorityFeePerGas'] as const) {
    if (t[k] === undefined) continue
    const asked = feeOf(t[k])
    if (asked === null || policy[k] === undefined || asked !== BigInt(policy[k])) refuse('The Shop only sends its own purchases.')
  }
  const snapshot: Record<string, unknown> = { from: lower(intent.buyer), to, data, chainId: `0x${intent.chainId.toString(16)}`, ...policy }
  return { sale, snapshot }
}

export function manaGuardedProvider(provider: Eip1193Provider, source: ManaPurchaseIntent): Eip1193Provider {
  // Pinned here: the buyer, the chain and a copy of the terms. Only the login check and the hand-off stay live.
  const intent: ManaPurchaseIntent = {
    buyer: lower(source.buyer),
    chainId: source.chainId,
    sale: structuredClone(source.sale),
    current: () => source.current(),
    handOff: (attempt) => source.handOff(attempt)
  }
  // The confirmed terms are checked once, before the wallet is asked anything.
  checkManaTerms(intent)
  const chainHex = `0x${intent.chainId.toString(16)}`
  const stillCurrent = (): void => {
    if (!intent.current()) refuse('Your sign-in changed meanwhile. Open the item again to buy it.')
  }
  /** The login, then the wallet's account (awaited), then the login again. */
  const session = async (): Promise<void> => {
    stillCurrent()
    await assertWalletAccount(provider, intent.buyer)
    stillCurrent()
  }
  const handOff = (attempt: ManaHandOff): void => {
    if (!intent.handOff(attempt)) refuse("This purchase can't be sent right now: it couldn't be kept, or it is already on its way.")
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
        // Only to the purchase's own chain, with what this page knows of it: never the caller's endpoints.
        case 'wallet_switchEthereumChain': {
          if (args.length !== 1 || lower((args[0] as { chainId?: unknown } | undefined)?.chainId) !== chainHex) refuse('The Shop only switches your wallet to its own network.')
          return provider.request({ method, params: [{ chainId: chainHex }] })
        }
        case 'wallet_addEthereumChain': {
          if (args.length !== 1 || lower((args[0] as { chainId?: unknown } | undefined)?.chainId) !== chainHex || AMOY_ADD_PARAMS.chainId !== chainHex) {
            refuse('The Shop only adds its own network to your wallet.')
          }
          return provider.request({ method, params: [AMOY_ADD_PARAMS] })
        }
        case 'eth_signTypedData_v4': {
          if (args.length !== 2) refuse('The Shop only signs its own purchases.')
          // The rails hand over JSON text: it is parsed once, here, into data of the guard's own. An object is
          // serialized first, so what is checked is exactly what is forwarded.
          let text: string
          try {
            text = typeof args[1] === 'string' ? args[1] : JSON.stringify(args[1])
          } catch {
            refuse('The Shop could not read what it was asked to sign.')
          }
          const checked = checkMetaTransaction(args[0], text, intent)
          const signed = JSON.stringify(checked.typed)
          await session()
          if (checked.sale) handOff({ rail: 'relay', nonce: checked.nonce })
          return provider.request({ method, params: [intent.buyer, signed] })
        }
        case 'eth_sendTransaction': {
          if (args.length !== 1) refuse('The Shop only sends its own purchases.')
          const checked = checkTransaction(args[0], intent)
          await session()
          // The chain is the last thing awaited: a wallet that moved to another network meanwhile would spend gas
          // there and leave the purchase pending. The login is checked again after it.
          if (lower(await provider.request({ method: 'eth_chainId' })) !== chainHex) refuse('Your wallet is on another network. Switch it to the Shop’s and try again.')
          stillCurrent()
          if (checked.sale) handOff({ rail: 'direct' })
          return provider.request({ method, params: [checked.snapshot] })
        }
        default:
          throw new WalletRefusal(`The Shop doesn't use ${method}.`, 4200)
      }
    },
    ...(provider.on ? { on: provider.on.bind(provider) } : {}),
    ...(provider.removeListener ? { removeListener: provider.removeListener.bind(provider) } : {})
  }
}

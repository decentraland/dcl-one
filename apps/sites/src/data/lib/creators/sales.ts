// Putting a published item on sale in the Decentraland Shop: enabling sales on the collection (the
// off-chain marketplace becomes a minter of the collection contract) and signing the item's primary
// order, priced in credits (USD-pegged MANA) or plain MANA. Ported from the legacy builder's
// SellCollectionModal + PutForSaleOffchainModal.
import { Network, TradeAssetType, TradeType, type Trade, type TradeCreation } from '@dcl/schemas'
import { ContractName, getCollectionContract, getContract } from '~/lib/chain/contracts'
import { isAddress, maxUint256, parseEther, toHex, zeroAddress } from 'viem'
import { type ContractCall, type ContractData } from '~/lib/auth'
import { type Collection } from '~/lib/collections'
import { getItemSales, type Item } from '~/lib/items'
import { type ItemListing } from '~/lib/listings'
import { getManaContract } from '~/lib/mana'
import { USD_CENTS_PER_CREDIT } from '~/lib/publishFee'
import {
  TradeConflictError,
  TradeNotFoundError,
  getOffchainMarketplaceContract,
  getTradeContract,
  toOnChainTrade,
  type SignatureIndexes,
  type UnsignedTrade
} from '~/lib/trades'
import { isWalletRejection, isWalletStateError } from '~/lib/walletErrors'

// 1 credit = $0.10 = 1e17 USD wei (the shop's fixed peg; marketplace-server rounds up to whole credits).
const USD_WEI_PER_CREDIT = 10n ** 17n
// A listing without an expiration date is signed with one far enough away to never matter.
export const NO_EXPIRATION = Date.UTC(2100, 0, 1)
const ONE_DAY_MS = 24 * 60 * 60 * 1000

export type PricedSale = { kind: 'credits'; credits: number } | { kind: 'mana'; manaWei: bigint }
export type SalePrice = PricedSale | { kind: 'free' }
export type PriceCurrency = PricedSale['kind']

// The same ceiling in either currency. At 1e30 wei a MANA price also stays inside marketplace-server's
// catalog guard (dearer rows are stored but never shown in the Shop), and credits stay exact in a Number.
const MAX_SALE_PRICE = 1_000_000_000_000n
export const MAX_SALE_CREDITS = MAX_SALE_PRICE
export const MAX_SALE_MANA_WEI = MAX_SALE_PRICE * 10n ** 18n
// Below 1 MANA the buyer would have to cover the meta-transaction gas themselves, so the legacy builder
// warned about it; here it is the floor.
export const MIN_SALE_MANA_WEI = 10n ** 18n
// MANA prices are typed with at most this many decimals, like the legacy builder's input mask.
export const MANA_INPUT_DECIMALS = 2

/** A whole-credit price the Shop can list: from 1 up to the catalog's ceiling. */
export function isValidCredits(credits: number): boolean {
  return Number.isInteger(credits) && credits >= 1 && credits <= Number(MAX_SALE_CREDITS)
}

/** A MANA price the Shop can list: from the 1 MANA floor up to the catalog's ceiling. */
export function isValidManaWei(wei: bigint): boolean {
  return wei >= MIN_SALE_MANA_WEI && wei <= MAX_SALE_MANA_WEI
}

/** Keeps a typed MANA amount to digits and one decimal point with at most two decimals. */
export function sanitizeManaInput(value: string): string {
  const cleaned = value.replace(/[^\d.]/g, '')
  const dot = cleaned.indexOf('.')
  if (dot === -1) return cleaned
  const whole = cleaned.slice(0, dot)
  const decimals = cleaned
    .slice(dot + 1)
    .replace(/\./g, '')
    .slice(0, MANA_INPUT_DECIMALS)
  return `${whole}.${decimals}`
}

/** A typed MANA amount as wei, or null when it is empty or not a plain decimal number. */
export function parseManaAmount(value: string): bigint | null {
  // viem rounds digits past the 18th instead of refusing them.
  if (!/^(\d+(\.\d{0,18})?|\.\d{1,18})$/.test(value)) return null
  return parseEther(value)
}

/** The form's currency + typed amount as a price, or null while it is missing or out of bounds. */
export function toPricedSale(currency: PriceCurrency, amount: string): PricedSale | null {
  if (currency === 'credits') {
    const credits = Number(amount)
    return amount !== '' && isValidCredits(credits) ? { kind: 'credits', credits } : null
  }
  const manaWei = parseManaAmount(amount)
  return manaWei !== null && isValidManaWei(manaWei) ? { kind: 'mana', manaWei } : null
}

/** The price a listing was signed with, so a new one can be compared against it. */
export function listingToSalePrice(listing: ItemListing): SalePrice {
  if (listing.currency === 'credits') return { kind: 'credits', credits: listing.credits }
  return listing.manaWei === 0n ? { kind: 'free' } : { kind: 'mana', manaWei: listing.manaWei }
}

export function isSamePrice(a: SalePrice, b: SalePrice): boolean {
  if (a.kind !== b.kind) return false
  switch (a.kind) {
    case 'credits':
      return (b as typeof a).credits === a.credits
    case 'mana':
      return (b as typeof a).manaWei === a.manaWei
    case 'free':
      return true
  }
}

export function creditsToUsdWei(credits: number): string {
  if (!Number.isInteger(credits)) throw new SellItemError('generic', `Credits must be a whole number, got ${credits}`)
  return (BigInt(credits) * USD_WEI_PER_CREDIT).toString()
}

function formatUsd(dollars: number): string {
  return dollars.toLocaleString('en-US', {
    style: 'currency',
    currency: 'USD',
    minimumFractionDigits: 2,
    maximumFractionDigits: 2
  })
}

/** Whole US dollars with cents — "$5.00" — for a credits amount. */
export function formatCreditsAsUsd(credits: number): string {
  return formatUsd((credits * USD_CENTS_PER_CREDIT) / 100)
}

/** "$5.00" for a MANA amount at `rate` USD wei per whole MANA (see `lib/manaRate`). */
export function formatManaAsUsd(manaWei: bigint, rate: bigint): string {
  // MANA wei × USD wei per MANA is 1e36 per dollar; rounded to the nearest cent.
  const usdCents = (manaWei * rate + 5n * 10n ** 33n) / 10n ** 34n
  return formatUsd(Number(usdCents) / 100)
}

/** A 0x address in one case, or mixed case with a valid checksum; callers lowercase it before use. */
export function isValidAddress(value: string): boolean {
  return isAddress(value) || /^0x[0-9A-F]{40}$/.test(value)
}

/** Dates are picked in the creator's local time and expire at the end of the chosen day. */
export function parseExpirationDate(value: string): number | null {
  const match = /^(\d{4})-(\d{2})-(\d{2})$/.exec(value)
  if (!match) return null
  const date = new Date(Number(match[1]), Number(match[2]) - 1, Number(match[3]), 23, 59, 59, 999)
  return Number.isNaN(date.getTime()) ? null : date.getTime()
}

/** A local date as `YYYY-MM-DD`, the form's date value. */
export function formatDateValue(date: Date): string {
  const pad = (n: number) => String(n).padStart(2, '0')
  return `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(date.getDate())}`
}

/** The earliest date the picker allows: tomorrow. */
export function minExpirationDate(now = Date.now()): Date {
  return new Date(now + ONE_DAY_MS)
}

/**
 * Sales are enabled once the marketplace new orders are signed on may mint the collection's items. An
 * older marketplace as minter is not enough: its orders are no longer accepted, and a newer order it
 * cannot mint would never settle, so such a collection goes through Enable Sales again.
 */
export function isSalesEnabled(collection: Collection, chainId: number): boolean {
  const minter = getOffchainMarketplaceContract(chainId).address.toLowerCase()
  return collection.minters.some(address => address.toLowerCase() === minter)
}

/** An older off-chain marketplace (V1 or V2) may mint the collection: it was already on sale before the upgrade. */
export function isSellingOnOlderMarketplace(collection: Collection, chainId: number): boolean {
  const older = new Set(
    [ContractName.OffChainMarketplace, ContractName.OffChainMarketplaceV2].map(name =>
      getContract(name, chainId).address.toLowerCase()
    )
  )
  return collection.minters.some(address => older.has(address.toLowerCase()))
}

/** `setMinters([marketplace], [true])` on the collection contract itself. */
export function buildEnableSalesCall(collection: Collection, chainId: number): ContractCall {
  if (!collection.contractAddress) throw new SellItemError('not_published', 'The collection has no contract yet')
  const contract = getCollectionContract(chainId, collection.contractAddress)
  return { contract, method: 'setMinters', args: [[getOffchainMarketplaceContract(chainId).address], [true]] }
}

/** The collection as builder-server will report it once the subgraph catches up with the minters change. */
export function withSalesEnabled(collection: Collection, chainId: number): Collection {
  const minter = getOffchainMarketplaceContract(chainId).address.toLowerCase()
  if (isSalesEnabled(collection, chainId)) return collection
  return { ...collection, minters: [...collection.minters, minter] }
}

export type SellFailureReason = 'rejected' | 'wallet' | 'sold_out' | 'not_published' | 'not_listed' | 'generic'

export class SellItemError extends Error {
  reason: SellFailureReason
  /** The underlying failure, when this one wraps it. */
  cause?: unknown

  constructor(reason: SellFailureReason, message: string = reason, cause?: unknown) {
    super(message)
    this.name = 'SellItemError'
    this.reason = reason
    this.cause = cause
  }
}

export function toSellItemError(error: unknown): SellItemError {
  if (error instanceof SellItemError) return error
  const message = error instanceof Error ? error.message : String(error)
  if (isWalletRejection(error)) return new SellItemError('rejected', message)
  if (isWalletStateError(error)) return new SellItemError('wallet', message, error)
  return new SellItemError('generic', message)
}

export type SellItemParams = {
  /** The connected wallet: it signs the order and mints on the buyer's behalf. */
  address: string
  chainId: number
  collection: Collection
  item: Item
  price: SalePrice
  /** Who receives the payment; ignored for a giveaway, which pays nobody. */
  beneficiary: string
  /** Unix milliseconds. */
  expiresAt: number
}

/**
 * The primary order for one item: every remaining unit is sellable, paid in USD-pegged MANA (the
 * shop's credits), in plain MANA, or, for a giveaway, zero MANA — the legacy encoding the shop
 * understands as free.
 */
export function buildItemOrder(
  params: SellItemParams,
  indexes: SignatureIndexes,
  salt: string,
  now = Date.now()
): UnsignedTrade {
  const { address, chainId, collection, item, price, beneficiary, expiresAt } = params
  if (!collection.contractAddress || !item.tokenId || !item.isPublished) {
    throw new SellItemError('not_published', `Item "${item.id}" is not published`)
  }
  const sales = getItemSales(item)
  if (!sales) throw new SellItemError('generic', `Item "${item.id}" has no rarity`)
  const uses = sales.maxSupply - sales.minted
  if (uses <= 0) throw new SellItemError('sold_out', `Item "${item.id}" is sold out`)

  const mana = getManaContract(chainId).address
  return {
    signer: address,
    network: Network.MATIC,
    chainId,
    type: TradeType.PUBLIC_ITEM_ORDER,
    checks: {
      uses,
      expiration: expiresAt,
      effective: now,
      salt,
      contractSignatureIndex: indexes.contractSignatureIndex,
      signerSignatureIndex: indexes.signerSignatureIndex,
      allowedRoot: '0x',
      externalChecks: []
    },
    sent: [
      {
        assetType: TradeAssetType.COLLECTION_ITEM,
        contractAddress: collection.contractAddress,
        itemId: item.tokenId,
        extra: ''
      }
    ],
    received: [toReceivedAsset(price, mana, beneficiary)]
  }
}

function toReceivedAsset(price: SalePrice, mana: string, beneficiary: string): UnsignedTrade['received'][number] {
  switch (price.kind) {
    case 'free':
      return {
        assetType: TradeAssetType.ERC20,
        contractAddress: mana,
        amount: '0',
        extra: '',
        beneficiary: zeroAddress
      }
    case 'mana':
      return {
        assetType: TradeAssetType.ERC20,
        contractAddress: mana,
        amount: price.manaWei.toString(),
        extra: '',
        beneficiary
      }
    case 'credits':
      return {
        assetType: TradeAssetType.USD_PEGGED_MANA,
        contractAddress: mana,
        amount: creditsToUsdWei(price.credits),
        extra: '',
        beneficiary
      }
  }
}

/** The listing the collection page shows for a stored order. */
export function toListing(itemId: string, tradeId: string, price: SalePrice): ItemListing {
  switch (price.kind) {
    case 'free':
      return { itemId, tradeId, currency: 'mana', manaWei: 0n }
    case 'mana':
      return { itemId, tradeId, currency: 'mana', manaWei: price.manaWei }
    case 'credits':
      return { itemId, tradeId, currency: 'credits', credits: price.credits }
  }
}

export function randomSalt(): string {
  return toHex(crypto.getRandomValues(new Uint8Array(32)))
}

/** The chain reads, wallet signature and server call the sequence needs, so it can run against fakes in tests. */
export type SellItemDeps = {
  fetchSignatureIndexes: (signer: string, chainId: number) => Promise<SignatureIndexes>
  signTrade: (trade: UnsignedTrade) => Promise<string>
  createTrade: (trade: TradeCreation) => Promise<string>
  /** The wallet prompt is over; the order is being stored. */
  onSigned?: () => void
  randomSalt?: () => string
}

/** Reads the marketplace's signature indexes, signs the order, stores it. Answers the listing as the collection page shows it. */
export async function sellItem(params: SellItemParams, deps: SellItemDeps): Promise<ItemListing> {
  try {
    const indexes = await deps.fetchSignatureIndexes(params.address, params.chainId)
    const trade = buildItemOrder(params, indexes, (deps.randomSalt ?? randomSalt)())
    const signature = await deps.signTrade(trade)
    deps.onSigned?.()
    const tradeId = await deps.createTrade({ ...trade, signature })
    // The trade id is what lets the row's menu re-price or cancel the listing without a refetch.
    const tradeContract = getOffchainMarketplaceContract(params.chainId).address.toLowerCase()
    return { ...toListing(params.item.tokenId!, tradeId, params.price), tradeContract }
  } catch (error) {
    throw toSellItemError(error)
  }
}

/** `cancelSignature([trade])` on the marketplace the order was signed for: the listing is gone once mined. */
export function buildCancelListingCall(trade: Trade): ContractCall {
  return { contract: getTradeContract(trade), method: 'cancelSignature', args: [[toOnChainTrade(trade)]] }
}

/** Which order a listing is: the cached id plus what identifies the item, should the id have gone stale. */
export type ListingRef = { tradeId: string; contractAddress: string; itemId: string }

/** The chain calls a cancellation needs, so it can run against fakes in tests. */
export type CancelListingDeps = {
  fetchTrade: (tradeId: string) => Promise<Trade>
  /** The item's current order id, for when the cached one has been retired by the server. */
  fetchItemTradeId: (contractAddress: string, itemId: string) => Promise<string | null>
  sendTransaction: (call: ContractCall) => Promise<string>
  waitForTransaction: (txHash: string) => Promise<boolean>
  /** The wallet prompt is over; the cancellation is mining. */
  onSigned?: () => void
}

/** The listing's live order: the cached id first, then the server's current one when that id is gone. */
async function resolveOrder(
  ref: ListingRef,
  deps: Pick<CancelListingDeps, 'fetchTrade' | 'fetchItemTradeId'>
): Promise<Trade> {
  try {
    return await deps.fetchTrade(ref.tradeId)
  } catch (error) {
    if (!(error instanceof TradeNotFoundError)) throw error
    const current = await deps.fetchItemTradeId(ref.contractAddress, ref.itemId)
    if (!current || current === ref.tradeId) {
      throw new SellItemError('not_listed', `Item ${ref.itemId} is no longer on sale`)
    }
    return deps.fetchTrade(current)
  }
}

async function cancelOrder(trade: Trade, deps: CancelListingDeps): Promise<void> {
  const txHash = await deps.sendTransaction(buildCancelListingCall(trade))
  deps.onSigned?.()
  const mined = await deps.waitForTransaction(txHash)
  if (!mined) throw new SellItemError('generic', 'The cancel listing transaction reverted')
}

/** Removes a listing: reads the stored order, cancels its signature on chain and waits for the receipt. */
export async function removeListing(ref: ListingRef, deps: CancelListingDeps): Promise<void> {
  try {
    await cancelOrder(await resolveOrder(ref, deps), deps)
  } catch (error) {
    throw toSellItemError(error)
  }
}

/** What a re-listing keeps from the order it replaces. */
export type ListingTerms = { beneficiary: string; expiresAt: number }

/** The beneficiary and expiration of a stored order; a giveaway's zero-address payee falls back to the signer. */
export function getListingTerms(trade: Trade, fallbackBeneficiary: string): ListingTerms {
  const beneficiary = trade.received[0]?.beneficiary
  return {
    beneficiary: beneficiary && beneficiary !== zeroAddress ? beneficiary : fallbackBeneficiary,
    expiresAt: trade.checks.expiration
  }
}

export type UpdatePriceParams = Omit<SellItemParams, 'price' | 'beneficiary' | 'expiresAt'> & {
  tradeId: string
  price: PricedSale
}

// After the cancellation is mined the server may still index the old order as open for a few seconds
// and answer 409 to the new one. Only the POST is retried (never the signature), like the shop's import.
const CONFLICT_RETRY_DELAYS_MS = [3000, 3000, 4000, 5000, 6000, 8000, 8000]

export type UpdatePriceDeps = Omit<CancelListingDeps, 'onSigned'> &
  Omit<SellItemDeps, 'onSigned'> & {
    /** The wallet prompt for that step is over. */
    onSigned?: (step: 'cancel' | 'sign') => void
    /** The old listing is gone; a failure from here on leaves the item off sale. */
    onCancelled?: (terms: ListingTerms) => void
  }

const sleep = (ms: number) => new Promise<void>(resolve => setTimeout(resolve, ms))

/** `createTrade` that waits out the indexer on "already an open order", then gives up. */
export function withConflictRetry(
  createTrade: SellItemDeps['createTrade'],
  wait: (ms: number) => Promise<void> = sleep,
  delays = CONFLICT_RETRY_DELAYS_MS
): SellItemDeps['createTrade'] {
  return async trade => {
    for (let attempt = 0; ; attempt++) {
      try {
        return await createTrade(trade)
      } catch (error) {
        if (!(error instanceof TradeConflictError) || attempt >= delays.length) throw error
        await wait(delays[attempt])
      }
    }
  }
}

/**
 * An order is immutable, so a new price (or currency) means cancelling the current order (one
 * transaction) and signing a fresh one with the same beneficiary and expiration.
 */
export async function updatePrice(params: UpdatePriceParams, deps: UpdatePriceDeps): Promise<ItemListing> {
  const { tradeId, price, ...rest } = params
  // A sold-out item can't be re-listed: refuse before taking the current order down.
  const sales = getItemSales(params.item)
  if (sales && sales.minted >= sales.maxSupply)
    throw new SellItemError('sold_out', `Item "${params.item.id}" is sold out`)
  let terms: ListingTerms
  try {
    const trade = await resolveOrder(
      { tradeId, contractAddress: params.collection.contractAddress!, itemId: params.item.tokenId! },
      deps
    )
    terms = getListingTerms(trade, params.address)
    await cancelOrder(trade, { ...deps, onSigned: () => deps.onSigned?.('cancel') })
  } catch (error) {
    throw toSellItemError(error)
  }
  deps.onCancelled?.(terms)
  return sellItem(
    { ...rest, price, ...terms },
    {
      ...deps,
      createTrade: withConflictRetry(deps.createTrade),
      onSigned: () => deps.onSigned?.('sign')
    }
  )
}

/** What `editItemsData` must carry over untouched when only the price changes. */
export type StoreItemData = { beneficiary: string; metadata: string }

type StoreItemOutputs = readonly [
  rarity: string,
  maxSupply: bigint,
  totalSupply: bigint,
  price: bigint,
  beneficiary: string,
  metadata: string,
  contentHash: string
]

/** The item's current on-chain data, so delisting rewrites nothing but the price. */
export async function readStoreItem(
  collection: Collection,
  item: Item,
  chainId: number,
  read: <T>(contract: ContractData, method: string, args: unknown[]) => Promise<T>
): Promise<StoreItemData> {
  if (!collection.contractAddress || !item.tokenId)
    throw new SellItemError('not_published', `Item "${item.id}" is not published`)
  const contract = getCollectionContract(chainId, collection.contractAddress)
  const [, , , , beneficiary, metadata] = await read<StoreItemOutputs>(contract, 'items', [item.tokenId])
  return { beneficiary, metadata }
}

/**
 * A legacy CollectionStore listing lives on the collection contract as the item's price; "not for sale"
 * is the max uint256 sentinel, set with `editItemsData` (creator and collaborators only).
 */
export function buildStoreDelistCall(
  collection: Collection,
  item: Item,
  current: StoreItemData,
  chainId: number
): ContractCall {
  if (!collection.contractAddress || !item.tokenId)
    throw new SellItemError('not_published', `Item "${item.id}" is not published`)
  const contract = getCollectionContract(chainId, collection.contractAddress)
  return {
    contract,
    method: 'editItemsData',
    args: [[item.tokenId], [maxUint256.toString()], [current.beneficiary], [current.metadata]]
  }
}

export type RemoveStoreListingDeps = Omit<CancelListingDeps, 'fetchTrade' | 'fetchItemTradeId'> & {
  readContract: <T>(contract: ContractData, method: string, args: unknown[]) => Promise<T>
}

/** Removes a legacy store listing: reads the item on chain, clears its price and waits for the receipt. */
export async function removeStoreListing(
  collection: Collection,
  item: Item,
  chainId: number,
  deps: RemoveStoreListingDeps
): Promise<void> {
  try {
    const current = await readStoreItem(collection, item, chainId, deps.readContract)
    const txHash = await deps.sendTransaction(buildStoreDelistCall(collection, item, current, chainId))
    deps.onSigned?.()
    const mined = await deps.waitForTransaction(txHash)
    if (!mined) throw new SellItemError('generic', 'The delist transaction reverted')
  } catch (error) {
    throw toSellItemError(error)
  }
}

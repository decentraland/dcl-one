// Everything the publish flow needs besides React: the createCollection arguments, the
// CreditsManager external call, payment-method availability, and the publish sequence itself,
// ported from the legacy builder's publish saga (the forum post lives in hooks/useForumPost).
// Server and chain access is injected so the sequence is unit-testable end to end.
import { ContractName, getContract } from '~/lib/chain/contracts'
import { encodeAbiParameters, getAbiItem, toFunctionSelector, toHex, type AbiFunction } from 'viem'
import { config } from '~/config'
import { toBytes32 } from '~/lib/bytes32'
import { type ContractCall } from '~/lib/auth'
import { BuilderServerError, COLLECTION_LOCKED_STATUS, isRefusedRequest } from '~/lib/builder'
import { isCollectionLocked, type Collection } from '~/lib/collections'
import { type CollectionCuration } from '~/lib/curation'
import { CreditsServerError, type ExternalCall, type PublicationAuthorization } from '~/lib/credits'
import { PublicationFeeMismatchError, withFeeTolerance } from '~/lib/feeVerification'
import { hasOldHashedContents } from '~/lib/itemFactory'
import { isMissingSmartWearableVideo, type Item } from '~/lib/items'
import { fetchManaAllowance } from '~/lib/mana'
import { weiToUsdCents, type PublicationFee } from '~/lib/publishFee'
import { isWalletRejection, isWalletStateError } from '~/lib/walletErrors'
import { getCollectionSymbol, toInitializeItems, type InitializeItem } from '~/lib/saveCollection'

/** 'card' (buying credits with Stripe) is the planned third method; only these two ship today. */
export type PaymentMethod = 'credits' | 'mana'

// Legacy builder cap: the curation tooling processes collections in chunks of 50 items and refuses
// larger ones, so publishing more would leave the collection impossible to approve.
export const MAX_PUBLISH_ITEMS = 50

export type PublishBlocker =
  'not_draft' | 'no_items' | 'too_many_items' | 'missing_smart_wearable_video' | 'has_invalid_items'

/**
 * `items` are the collection's items; a smart wearable without its preview video blocks publishing, and so do
 * items with validation errors when the caller says they block (`hasInvalidItems`). A locked draft stays
 * publishable, as in legacy: its last transaction may have reverted, and a retry reuses the frozen salt, so at
 * most one createCollection can land.
 */
export function getPublishBlocker(
  collection: Collection,
  itemCount: number,
  items: Item[],
  { hasInvalidItems = false }: { hasInvalidItems?: boolean } = {}
): PublishBlocker | null {
  if (collection.isPublished) return 'not_draft'
  if (itemCount === 0) return 'no_items'
  if (itemCount > MAX_PUBLISH_ITEMS) return 'too_many_items'
  if (items.some(isMissingSmartWearableVideo)) return 'missing_smart_wearable_video'
  if (hasInvalidItems) return 'has_invalid_items'
  return null
}

export function getMaticChainId(): number {
  return Number(config.get('MATIC_CHAIN_ID'))
}

/**
 * Which payment methods to offer: credits always, MANA only when the wallet holds some. With credits
 * off (`shop-credits-for-collections-fee`), MANA is the only way to pay and shows at any balance —
 * an empty wallet then sees the card disabled with its Get MANA link rather than no card at all.
 */
export function getAvailablePaymentMethods(manaBalanceWei: bigint | undefined, creditsEnabled = true): PaymentMethod[] {
  if (!creditsEnabled) return ['mana']
  const methods: PaymentMethod[] = ['credits']
  if (manaBalanceWei !== undefined && manaBalanceWei > 0n) methods.push('mana')
  return methods
}

export function canPayWith(method: PaymentMethod, fee: PublicationFee, balances: CreditsAndMana): boolean {
  if (method === 'credits') return balances.credits >= fee.total.credits
  return balances.manaWei >= fee.total.manaWei
}

export type CreditsAndMana = { credits: number; manaWei: bigint }

/** How much MANA the CollectionManager may pull from `address` right now, read from the chain. */
export async function fetchCollectionManagerAllowance(address: string): Promise<bigint> {
  const chainId = getMaticChainId()
  return fetchManaAllowance(address, getContract(ContractName.CollectionManager, chainId).address, chainId)
}

/**
 * The MANA approval a publish paid in MANA still needs, or null when the allowance already covers it. The
 * CollectionManager pulls the on-chain total (which a verified quote may sit below), so that total plus the same
 * tolerance the verification grants the oracle is what must fit. A failed allowance read counts as no allowance: an
 * extra approval prompt beats a publish that reverts.
 */
export async function getManaApprovalNeeded(
  onChainTotalWei: bigint,
  fetchAllowance: () => Promise<bigint>
): Promise<bigint | null> {
  const required = withFeeTolerance(onChainTotalWei)
  if (required === 0n) return null
  const allowance = await fetchAllowance().catch(() => 0n)
  return allowance < required ? required : null
}

// CollectionManager.createCollection(forwarder, factory, salt, name, symbol, baseURI, creator, items).
export type CreateCollectionArgs = [string, string, string, string, string, string, string, InitializeItem[]]

export function buildCreateCollectionArgs(
  collection: Collection,
  items: Item[],
  creator: string,
  chainId: number
): CreateCollectionArgs {
  if (!collection.salt) throw new PublishCollectionError('missing_salt')
  return [
    getContract(ContractName.Forwarder, chainId).address,
    getContract(ContractName.CollectionFactoryV3, chainId).address,
    collection.salt,
    collection.name,
    getCollectionSymbol(collection.name),
    config.get('ERC721_COLLECTION_BASE_URI'),
    creator,
    toInitializeItems(items)
  ]
}

export function buildCreateCollectionCall(chainId: number, args: CreateCollectionArgs): ContractCall {
  return { contract: getContract(ContractName.CollectionManager, chainId), method: 'createCollection', args }
}

const EXTERNAL_CALL_TTL_MS = 24 * 60 * 60 * 1000

const randomSalt = () => toHex(crypto.getRandomValues(new Uint8Array(32)))

/**
 * The CreditsManager external call that runs createCollection on the creator's behalf. Byte-compatible
 * with decentraland-dapps' `buildCollectionManagerExternalCall`: selector + ABI-encoded params.
 */
export function buildCollectionManagerExternalCall(
  chainId: number,
  args: CreateCollectionArgs,
  now = Date.now(),
  salt: string = randomSalt()
): ExternalCall {
  const manager = getContract(ContractName.CollectionManager, chainId)
  const createCollection = getAbiItem({ abi: manager.abi, name: 'createCollection' }) as AbiFunction
  return {
    target: manager.address,
    selector: toFunctionSelector(createCollection),
    data: encodeAbiParameters(createCollection.inputs, args),
    expiresAt: Math.floor((now + EXTERNAL_CALL_TTL_MS) / 1000),
    salt
  }
}

/** CreditsManager.useCredits paying the whole fee with the server-issued credit: no MANA leaves the wallet. */
export function buildUseCreditsCall(
  chainId: number,
  authorization: PublicationAuthorization,
  externalCall: ExternalCall
): ContractCall {
  const { credit, externalCallSignature } = authorization
  return {
    contract: getContract(ContractName.CreditsManager, chainId),
    method: 'useCredits',
    args: [
      {
        credits: [{ value: credit.amount, expiresAt: credit.expiresAt, salt: toBytes32(credit.id) }],
        creditsSignatures: [credit.signature],
        externalCall,
        customExternalCallSignature: externalCallSignature,
        maxUncreditedValue: '0',
        maxCreditedValue: credit.amount
      }
    ]
  }
}

export type PublishFailureReason =
  | 'missing_salt'
  | 'unsynced'
  | 'locked'
  | 'insufficient_credits'
  | 'fee_mismatch'
  | 'tos_failed'
  | 'rejected'
  | 'wallet'
  | 'generic'

export class PublishCollectionError extends Error {
  reason: PublishFailureReason
  /** The underlying failure, when this one wraps it. */
  cause?: unknown

  constructor(reason: PublishFailureReason, message: string = reason, cause?: unknown) {
    super(message)
    this.name = 'PublishCollectionError'
    this.reason = reason
    this.cause = cause
  }
}

/** Maps any failure of the sequence to the reason the UI shows copy for. */
export function toPublishError(error: unknown): PublishCollectionError {
  if (error instanceof PublishCollectionError) return error
  if (error instanceof PublicationFeeMismatchError) return new PublishCollectionError('fee_mismatch', error.message)
  if (error instanceof CreditsServerError && error.code === 'insufficient_credits') {
    return new PublishCollectionError('insufficient_credits', error.message)
  }
  if (error instanceof BuilderServerError && error.status === COLLECTION_LOCKED_STATUS) {
    return new PublishCollectionError('locked', error.message)
  }
  const message = error instanceof Error ? error.message : String(error)
  if (isWalletRejection(error)) return new PublishCollectionError('rejected', message)
  if (isWalletStateError(error)) return new PublishCollectionError('wallet', message, error)
  return new PublishCollectionError('generic', message)
}

async function retry<T>(
  times: number,
  delayMs: number,
  fn: () => Promise<T>,
  shouldRetry: (error: unknown) => boolean = () => true
): Promise<T> {
  let lastError: unknown
  for (let attempt = 0; attempt < times; attempt++) {
    try {
      return await fn()
    } catch (error) {
      lastError = error
      if (!shouldRetry(error)) break
      if (attempt < times - 1) await new Promise(resolve => setTimeout(resolve, delayMs))
    }
  }
  throw lastError
}

const isTransientFailure = (error: unknown) => !isRefusedRequest(error)

export type PublishParams = {
  address: string
  collection: Collection
  items: Item[]
  paymentMethod: PaymentMethod
  fee: PublicationFee
  /** The contact email from step 1, recorded with the Terms of Service acceptance. */
  email: string
  chainId: number
}

/** The server and chain calls the sequence needs, so it can run against fakes in tests. */
export type PublishDeps = {
  saveCollection: (collection: Collection, items: Item[]) => Promise<Collection>
  fetchItems: (collectionId: string) => Promise<Item[]>
  /** Re-hashes and re-saves an item whose files still carry legacy hashes; answers the saved item. */
  rehashItem: (item: Item) => Promise<Item>
  saveTOS: (collection: Collection, email: string) => Promise<void>
  authorizePublication: (params: {
    usdPriceCents: number
    chainId: number
    creditsManagerAddress: string
    externalCall: ExternalCall
  }) => Promise<PublicationAuthorization>
  sendTransaction: (call: ContractCall) => Promise<string>
  lockCollection: (collectionId: string) => Promise<number>
  randomSalt?: () => string
}

export type PublishResult = {
  collection: Collection
  txHash: string
}

/**
 * The publish sequence: re-save the collection (regenerates salt + contract address), verify the
 * items match the server, re-save items still carrying legacy hashes, record the ToS, pay and send
 * createCollection, then lock the collection. Errors are normalized to PublishCollectionError.
 */
export async function publishCollection(params: PublishParams, deps: PublishDeps): Promise<PublishResult> {
  const { address, paymentMethod, fee, email, chainId } = params
  try {
    let collection = params.collection
    let items = params.items
    if (!isCollectionLocked(collection)) {
      collection = await deps.saveCollection(collection, items)
    }
    if (!collection.salt) throw new PublishCollectionError('missing_salt')

    const serverItems = await deps.fetchItems(collection.id)
    const localIds = new Set(items.map(item => item.id))
    if (serverItems.length !== items.length || serverItems.some(item => !localIds.has(item.id))) {
      throw new PublishCollectionError('unsynced')
    }

    // Legacy "Qm…" hashes can't be deployed to Catalyst: re-hash those files before they go on-chain.
    const rehashed: Item[] = []
    for (const item of items) rehashed.push(hasOldHashedContents(item) ? await deps.rehashItem(item) : item)
    items = rehashed

    // Recorded before the transaction so a ToS failure never leaves the collection locked.
    try {
      await retry(3, 500, () => deps.saveTOS(collection, email), isTransientFailure)
    } catch (error) {
      throw new PublishCollectionError('tos_failed', error instanceof Error ? error.message : String(error), error)
    }

    const args = buildCreateCollectionArgs(collection, items, address, chainId)
    let txHash: string
    if (paymentMethod === 'credits') {
      const externalCall = buildCollectionManagerExternalCall(
        chainId,
        args,
        Date.now(),
        (deps.randomSalt ?? randomSalt)()
      )
      const authorization = await deps.authorizePublication({
        usdPriceCents: weiToUsdCents(fee.total.usdWei),
        chainId,
        creditsManagerAddress: getContract(ContractName.CreditsManager, chainId).address,
        externalCall
      })
      txHash = await deps.sendTransaction(buildUseCreditsCall(chainId, authorization, externalCall))
    } else {
      txHash = await deps.sendTransaction(buildCreateCollectionCall(chainId, args))
    }

    // The fee is paid once the transaction is out: a lock failure must never surface as a retriable
    // publish error, or "try again" would send (and charge) createCollection a second time.
    let lock = collection.lock
    try {
      lock = await retry(10, 500, () => deps.lockCollection(collection.id))
    } catch (error) {
      console.error('Collection lock failed after publishing', error)
    }
    return { collection: { ...collection, lock }, txHash }
  } catch (error) {
    throw toPublishError(error)
  }
}

export type SyncDeps = {
  publishCollectionItems: (collectionId: string) => Promise<unknown>
}

export type ConsolidateDeps = SyncDeps & {
  waitForTransaction: (txHash: string) => Promise<boolean>
  /** Whether the collection's contract exists on chain: a retry can revert because an earlier attempt landed. */
  isCollectionDeployed?: () => Promise<boolean>
  /** Runs once the server holds the published items, e.g. to open the collection's forum topic. */
  onSynced?: () => void
}

// One hour of 5s polls: the subgraph can lag well past a couple of minutes on a busy Polygon day. Forum post
// recovery (`RECOVERY_GRACE_MS` in forumPost.ts) waits this out, so keep it shorter than that grace.
const CONSOLIDATE_RETRY_DELAY_MS = 5000
const CONSOLIDATE_RETRIES = (60 * 60 * 1000) / CONSOLIDATE_RETRY_DELAY_MS

/**
 * Asks builder-server to sync a published collection with the chain (item token ids). The server
 * answers 401 until the subgraph indexes the block, so that status is retried.
 */
export async function syncPublishedItems(
  collectionId: string,
  deps: SyncDeps,
  retries = CONSOLIDATE_RETRIES,
  retryDelayMs = CONSOLIDATE_RETRY_DELAY_MS
): Promise<void> {
  for (let attempt = 0; ; attempt++) {
    try {
      await deps.publishCollectionItems(collectionId)
      return
    } catch (error) {
      const notIndexedYet = error instanceof BuilderServerError && error.status === 401
      if (!notIndexedYet || attempt >= retries) throw error
      await new Promise(resolve => setTimeout(resolve, retryDelayMs))
    }
  }
}

// Matches the forum post's recovery window: older collections keep reading To review without a request.
const REVIEW_RECOVERY_MAX_AGE_MS = 30 * 24 * 60 * 60_000

/** Whether a publish finished on a later visit still gets its review request: only within a month of publishing. */
export function isRecentPublication(collection: Pick<Collection, 'lock' | 'updatedAt'>, now = Date.now()): boolean {
  return now - Math.max(collection.lock ?? 0, collection.updatedAt) < REVIEW_RECOVERY_MAX_AGE_MS
}

export type ReviewRequestDeps = {
  fetchCollectionCuration: (collectionId: string) => Promise<CollectionCuration | null>
  pushCollectionCuration: (collectionId: string) => Promise<CollectionCuration>
}

const ALREADY_REQUESTED_STATUS = 400

/**
 * Opens a published collection's first review request and returns the collection's latest one; a request that
 * already exists (a retry, or a curator who acted first) is returned as is, never reopened.
 */
export async function openFirstReviewRequest(
  collectionId: string,
  deps: ReviewRequestDeps
): Promise<CollectionCuration | null> {
  const existing = await deps.fetchCollectionCuration(collectionId)
  if (existing) return existing
  try {
    return await deps.pushCollectionCuration(collectionId)
  } catch (error) {
    const openedMeanwhile =
      error instanceof BuilderServerError &&
      error.status === ALREADY_REQUESTED_STATUS &&
      error.message.includes('ongoing review request')
    if (openedMeanwhile) return deps.fetchCollectionCuration(collectionId)
    throw error
  }
}

/** The publish transaction was mined but reverted: nothing was charged and nothing went on chain. */
export class PublishTransactionRevertedError extends Error {
  txHash: string

  constructor(txHash: string) {
    super(`Publish transaction ${txHash} reverted`)
    this.name = 'PublishTransactionRevertedError'
    this.txHash = txHash
  }
}

/** Waits for the publish transaction to be mined, then runs the server sync and `onSynced`. */
export async function consolidatePublishedCollection(
  collectionId: string,
  txHash: string,
  deps: ConsolidateDeps,
  retries = CONSOLIDATE_RETRIES,
  retryDelayMs = CONSOLIDATE_RETRY_DELAY_MS
): Promise<void> {
  const mined = await deps.waitForTransaction(txHash)
  // Code at the address can only be this exact collection: the factory derives it from the salt, its caller (the
  // CollectionManager's forwarder) and the full initialize calldata, creator included.
  if (!mined && !(await deps.isCollectionDeployed?.().catch(() => false))) {
    throw new PublishTransactionRevertedError(txHash)
  }
  await syncPublishedItems(collectionId, deps, retries, retryDelayMs)
  deps.onSynced?.()
}

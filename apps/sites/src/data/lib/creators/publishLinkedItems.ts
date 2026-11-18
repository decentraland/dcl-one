// The linked-collection publish sequence, ported from the legacy builder's thirdParty sagas
// (handlePublishAndPushChangesThirdPartyItemRequest + Success): record the ToS, buy the missing item slots on the
// ThirdPartyRegistry when needed (registering the third party on its first publish), then hand the signed slots
// cheque to builder-server, which opens one item curation per item, and finally push changes of already-reviewed
// items. Server and chain access is injected so the sequence runs against fakes in tests.
import { numberToHex, toHex } from 'viem'
import {
  type ContractCall,
  type TransactionSender,
  type TransactionSettlement,
  type TypedDataDomain,
  type TypedDataTypes
} from '~/lib/auth'
import { BuilderServerError, isRefusedRequest, type PublishLinkedItemsResult, type SlotsCheque } from '~/lib/builder'
import { CurationStatus, type Collection } from '~/lib/collections'
import { type Item } from '~/lib/items'
import { canPushLinkedItem, type ItemCuration } from '~/lib/linkedItemStatus'
import { buildManaApproveCall } from '~/lib/mana'
import { type PushLinkedResult } from '~/lib/pushLinkedChanges'
import { type ThirdParty } from '~/lib/thirdParty'
import {
  buildAddThirdPartyCall,
  buildBuyItemSlotsCall,
  getThirdPartyRegistry,
  type RegisteredThirdParty,
  type ThirdPartyPrice
} from '~/lib/thirdPartyRegistry'
import { isWalletRejection, isWalletStateError } from '~/lib/walletErrors'

export type LinkedPublishStep =
  'check' | 'tos' | 'kind' | 'allowance' | 'register' | 'buy_slots' | 'wait_tx' | 'delete_virtual' | 'publish' | 'push'

/** The few phases the creator sees while the run goes through its steps. */
export type LinkedPublishPhase = 'prepare' | 'pay' | 'submit'

const PHASE_OF_STEP: Record<LinkedPublishStep, LinkedPublishPhase> = {
  check: 'prepare',
  tos: 'prepare',
  kind: 'prepare',
  allowance: 'pay',
  register: 'pay',
  buy_slots: 'pay',
  wait_tx: 'pay',
  delete_virtual: 'submit',
  publish: 'submit',
  push: 'submit'
}

export function getLinkedPublishPhase(step: LinkedPublishStep): LinkedPublishPhase {
  return PHASE_OF_STEP[step]
}

/**
 * The phases to show for a run: paying only when something is bought or an earlier payment is resumed, so a publish
 * that charges nothing never shows a payment step. A run that turns out to pay anyway gains it as it gets there.
 */
export function getLinkedPublishPhases(pays: boolean, current?: LinkedPublishStep): LinkedPublishPhase[] {
  return pays || (current && PHASE_OF_STEP[current] === 'pay') ? ['prepare', 'pay', 'submit'] : ['prepare', 'submit']
}

export type LinkedPublishFailureReason =
  | 'rejected'
  | 'wallet'
  | 'reverted'
  | 'payment_pending'
  | 'fee_changed'
  | 'insufficient_slots'
  | 'pending_review'
  | 'tos_failed'
  | 'push_failed'
  | 'generic'

/** A registration or slot purchase this run sent. */
export type LinkedPayment = {
  txHash: string
  /** Who sent it and with which nonce, when known: tells a replaced payment apart once the node forgets it. */
  sender?: TransactionSender
  /** `addThirdParties`: the virtual third party must still be removed once the chain has it. */
  registration: boolean
}

export class LinkedPublishError extends Error {
  step: LinkedPublishStep
  reason: LinkedPublishFailureReason
  /** The underlying failure, when this one wraps it. */
  cause?: unknown
  /**
   * The registration or slot purchase this run sent: the one that reverted for 'reverted', otherwise a payment that
   * may already be on chain, which a retry must resume from instead of paying again.
   */
  txHash?: string
  /** The payment this run sent, when it got that far: a retry resumes from it instead of paying again. */
  payment?: LinkedPayment

  constructor(
    step: LinkedPublishStep,
    reason: LinkedPublishFailureReason,
    message: string = reason,
    cause?: unknown,
    txHash?: string
  ) {
    super(message)
    this.name = 'LinkedPublishError'
    this.step = step
    this.reason = reason
    this.cause = cause
    this.txHash = txHash
  }
}

// builder-server's publish endpoint answers both with 409: InsufficientSlotsError and, for a collection that
// already has a pending item curation, AlreadyPublishedCollectionError.
const CONFLICT_STATUS = 409
const INSUFFICIENT_SLOTS_PATTERN = /slots/i
const PENDING_REVIEW_PATTERN = /already has published items/i

function hasStatus(error: unknown, status: number): error is BuilderServerError {
  return error instanceof BuilderServerError && error.status === status
}

function isServerFailure(error: unknown): boolean {
  return error instanceof BuilderServerError && error.status >= 500
}

export function isInsufficientSlotsError(error: unknown): boolean {
  return hasStatus(error, CONFLICT_STATUS) && INSUFFICIENT_SLOTS_PATTERN.test(error.message)
}

export function isPendingReviewError(error: unknown): boolean {
  return hasStatus(error, CONFLICT_STATUS) && PENDING_REVIEW_PATTERN.test(error.message)
}

/** Maps any failure of the sequence to the reason the UI shows copy for, keeping the step it happened at. */
export function toLinkedPublishError(
  error: unknown,
  step: LinkedPublishStep,
  payment?: LinkedPayment
): LinkedPublishError {
  const wrapped = wrapLinkedPublishError(error, step, payment?.txHash)
  if (payment) {
    wrapped.payment ??= payment
    wrapped.txHash ??= payment.txHash
  }
  return wrapped
}

function wrapLinkedPublishError(error: unknown, step: LinkedPublishStep, txHash?: string): LinkedPublishError {
  if (error instanceof LinkedPublishError) return error
  const message = error instanceof Error ? error.message : String(error)
  if (step === 'tos') return new LinkedPublishError(step, 'tos_failed', message, error, txHash)
  if (isWalletRejection(error)) return new LinkedPublishError(step, 'rejected', message, error, txHash)
  if (isWalletStateError(error)) return new LinkedPublishError(step, 'wallet', message, error, txHash)
  if (isInsufficientSlotsError(error)) return new LinkedPublishError(step, 'insufficient_slots', message, error, txHash)
  if (isPendingReviewError(error)) return new LinkedPublishError(step, 'pending_review', message, error, txHash)
  return new LinkedPublishError(step, 'generic', message, error, txHash)
}

/**
 * The payment a retry must resume from: a registration or slot purchase that was sent and did not revert. Paying
 * again would charge the creator twice (or revert, for a registration).
 */
export function getResumablePayment(error: LinkedPublishError): LinkedPayment | undefined {
  return error.reason === 'reverted' ? undefined : error.payment
}

export type LinkedSlots = {
  /** Free slots, counting any the registry sold that builder-server hasn't indexed yet. */
  available: number
  /** Whether the registry has the third party, even if builder-server still reads it as virtual. */
  registered: boolean
}

/**
 * builder-server counts free slots against its indexed `maxItems`, which lags the registry after a purchase; adding
 * the difference tells a purchase still being indexed apart from one that never happened, so it is never bought twice.
 */
export function getLinkedSlots(
  serverAvailable: number,
  indexedMaxItems: number,
  registry: Pick<RegisteredThirdParty, 'registered' | 'maxItems'>
): LinkedSlots {
  return {
    available: serverAvailable + Math.max(0, registry.maxItems - indexedMaxItems),
    registered: registry.registered
  }
}

/**
 * builder-server refuses a publish while any item of the collection has a pending curation. When every item to publish
 * is among them, an earlier request already published them (it can answer 500 after writing them): there is nothing
 * left to buy or publish. Any other pending curation is a review in progress.
 */
export function getPublishState(
  itemsToPublish: Item[],
  curations: Map<string, ItemCuration>
): 'ready' | 'already_published' | 'pending_review' {
  const pending = new Set(
    [...curations.values()]
      .filter(curation => curation.status === CurationStatus.PENDING)
      .map(curation => curation.itemId)
  )
  if (pending.size === 0) return 'ready'
  return itemsToPublish.every(item => pending.has(item.id)) ? 'already_published' : 'pending_review'
}

export type ChequeTypedData = {
  domain: TypedDataDomain
  types: TypedDataTypes
  value: { thirdPartyId: string; qty: number; salt: string }
}

/**
 * The EIP-712 "ConsumeSlots" cheque exactly as builder-server verifies it (buildChequeSignatureData) and the
 * registry consumes it: the domain carries the chain id as its salt instead of a chainId field.
 */
export function buildChequeTypedData(
  chainId: number,
  thirdPartyId: string,
  qty: number,
  salt: string
): ChequeTypedData {
  const registry = getThirdPartyRegistry(chainId)
  return {
    domain: {
      name: registry.name,
      version: registry.version,
      verifyingContract: registry.address,
      salt: numberToHex(chainId, { size: 32 })
    },
    types: {
      ConsumeSlots: [
        { name: 'thirdPartyId', type: 'string' },
        { name: 'qty', type: 'uint256' },
        { name: 'salt', type: 'bytes32' }
      ]
    },
    value: { thirdPartyId, qty, salt }
  }
}

/** A random 32-byte hex salt; the registry rejects a cheque whose salt was consumed before. */
export function createChequeSalt(): string {
  return toHex(crypto.getRandomValues(new Uint8Array(32)))
}

export type SignChequeDeps = {
  signTypedData: (
    domain: TypedDataDomain,
    types: TypedDataTypes,
    primaryType: string,
    value: Record<string, unknown>
  ) => Promise<string>
  randomSalt?: () => string
}

/** Signs a cheque for `qty` slots; the wizard asks for it before the paid steps so the wallet prompt comes first. */
export async function signSlotsCheque(
  deps: SignChequeDeps,
  chainId: number,
  thirdPartyId: string,
  qty: number
): Promise<SlotsCheque> {
  const salt = (deps.randomSalt ?? createChequeSalt)()
  const { domain, types, value } = buildChequeTypedData(chainId, thirdPartyId, qty, salt)
  const signature = await deps.signTypedData(domain, types, 'ConsumeSlots', value)
  return { signature, qty, salt }
}

export type SlotsToBuyParams = {
  itemsToPublish: number
  availableSlots: number
  isProgrammatic: boolean
  /** Whether the third party is already on chain. */
  isPublished: boolean
  programmaticMinSlots: number
}

/**
 * Slots to buy for this publish: whatever the items exceed the free slots by. A programmatic third party registers
 * with at least the registry's base pack; after that it still buys its missing slots, which the registry gives it
 * for free (builder-server refuses any publish beyond the available slots).
 */
export function getSlotsToBuy(params: SlotsToBuyParams): number {
  const missing = Math.max(0, params.itemsToPublish - params.availableSlots)
  if (!params.isProgrammatic || params.isPublished) return missing
  return Math.max(missing, params.programmaticMinSlots)
}

export type LinkedPublishFee = {
  usdWei: bigint
  manaWei: bigint
  /** The cap handed to the registry: the MANA price plus the legacy 0.5% pad for oracle drift between quote and tx. */
  maxManaWei: bigint
}

const MAX_PRICE_PAD_NUMERATOR = 1005n
const MAX_PRICE_PAD_DENOMINATOR = 1000n

export type LinkedFeeKind = {
  isProgrammatic: boolean
  /** Whether the third party is already on chain. */
  isPublished: boolean
}

/**
 * What the registry charges for `slotsToBuy`: every slot for a standard third party; only the base pack when a
 * programmatic one registers, whatever extra slots it registers with; nothing once a programmatic one is on chain.
 */
export function getLinkedPublishFee(
  slotsToBuy: number,
  price: ThirdPartyPrice,
  kind: LinkedFeeKind = { isProgrammatic: false, isPublished: false }
): LinkedPublishFee {
  const charged =
    slotsToBuy <= 0 || !kind.isProgrammatic ? slotsToBuy : kind.isPublished ? 0 : price.programmaticMinSlots
  const slots = BigInt(Math.max(0, charged))
  const manaWei = price.itemSlotManaWei * slots
  const padded = manaWei * MAX_PRICE_PAD_NUMERATOR
  const maxManaWei = (padded + MAX_PRICE_PAD_DENOMINATOR - 1n) / MAX_PRICE_PAD_DENOMINATOR
  return { usdWei: price.itemSlotUsdWei * slots, manaWei, maxManaWei }
}

export type LinkedPublishQuote = { slotsToBuy: number; fee: LinkedPublishFee }

/** The slots a publish of `itemCount` items buys given `slots`, and what they cost. */
export function getLinkedPublishQuote(
  itemCount: number,
  slots: LinkedSlots,
  price: ThirdPartyPrice,
  thirdParty: { isProgrammatic: boolean; published: boolean }
): LinkedPublishQuote {
  const isPublished = slots.registered || thirdParty.published
  const slotsToBuy = getSlotsToBuy({
    itemsToPublish: itemCount,
    availableSlots: slots.available,
    isProgrammatic: thirdParty.isProgrammatic,
    isPublished,
    programmaticMinSlots: price.programmaticMinSlots
  })
  return {
    slotsToBuy,
    fee: getLinkedPublishFee(slotsToBuy, price, { isProgrammatic: thirdParty.isProgrammatic, isPublished })
  }
}

export type FreshLinkedQuote =
  | { kind: 'ready'; slots: LinkedSlots; price: ThirdPartyPrice; maxFeeManaWei: bigint }
  | { kind: 'fee_changed' }
  | { kind: 'read_failed' }

/**
 * Reads the slots and the price again right before paying, against the quote the creator confirmed. The slots an
 * earlier attempt bought must not be bought twice, and builder-server also answers 0 slots when it fails, so a 0 is
 * confirmed with a second read; a price or slot change that costs more than the confirmed fee stops for a new
 * confirmation. The confirmed fee caps what the run may pay.
 */
export async function readFreshLinkedQuote(
  itemCount: number,
  thirdParty: { isProgrammatic: boolean; published: boolean },
  confirmed: { slots: LinkedSlots; price: ThirdPartyPrice },
  reads: {
    readSlots: () => Promise<LinkedSlots | undefined>
    readPrice: () => Promise<ThirdPartyPrice | undefined>
  }
): Promise<FreshLinkedQuote> {
  const readSlots = async () => {
    const first = await reads.readSlots()
    return first?.available === 0 ? reads.readSlots() : first
  }
  const [slots, price] = await Promise.all([readSlots(), reads.readPrice()])
  if (!slots || !price) return { kind: 'read_failed' }
  const shown = getLinkedPublishQuote(itemCount, confirmed.slots, confirmed.price, thirdParty)
  const fresh = getLinkedPublishQuote(itemCount, slots, price, thirdParty)
  if (fresh.fee.maxManaWei > shown.fee.maxManaWei) return { kind: 'fee_changed' }
  return { kind: 'ready', slots, price, maxFeeManaWei: shown.fee.maxManaWei }
}

export type PublishLinkedParams = {
  address: string
  chainId: number
  collection: Collection
  thirdParty: ThirdParty
  itemsToPublish: Item[]
  itemsToPush: Item[]
  /** Signed by the wizard before this runs; null when nothing is published. */
  cheque: SlotsCheque | null
  email: string
  isProgrammatic: boolean
  slots: LinkedSlots
  price: ThirdPartyPrice
  /** The most the creator agreed to pay: a purchase that would cost more stops with `fee_changed` instead. */
  maxFeeManaWei?: bigint
  /**
   * A registration or slot purchase an earlier attempt already sent: the run waits for it to settle and reads the
   * slots again, so only what is still missing is bought.
   */
  paidTx?: LinkedPayment
}

/** The server and chain calls the sequence needs, so it can run against fakes in tests. */
export type PublishLinkedDeps = {
  /** The latest curation of every item of the collection, as builder-server holds them now. */
  fetchCurations: () => Promise<Map<string, ItemCuration>>
  saveTOS: (collection: Collection, email: string, hashes: string[]) => Promise<void>
  setThirdPartyKind: (thirdPartyId: string, isProgrammatic: boolean) => Promise<void>
  fetchManaAllowance: (owner: string, spender: string) => Promise<bigint>
  sendTransaction: (call: ContractCall) => Promise<string>
  settleTransaction: (
    txHash: string,
    sender?: TransactionSender,
    onSeen?: (sender: TransactionSender) => void
  ) => Promise<TransactionSettlement>
  /** The sender and nonce of a transaction just sent, recorded with the payment. */
  readSender?: (txHash: string) => Promise<TransactionSender | undefined>
  fetchSlots: () => Promise<LinkedSlots>
  deleteVirtualThirdParty: (thirdPartyId: string) => Promise<void>
  publishLinkedItems: (
    collectionId: string,
    itemIds: string[],
    cheque: SlotsCheque
  ) => Promise<PublishLinkedItemsResult>
  /** `curations` is read right before pushing: a pending request is refreshed, an approved item gets a new one. */
  pushChanges: (items: Item[], curations: Map<string, ItemCuration>) => Promise<PushLinkedResult>
  onStep?: (step: LinkedPublishStep) => void
  /**
   * The payment to resume if this run stops, as soon as it changes: set the moment a purchase is sent, cleared once it
   * reverts or is no longer needed. Persisting it here survives a reload mid-run, which a caller's error handler can't.
   */
  onPayment?: (payment: LinkedPayment | undefined) => void
  /** Injected so specs run the retry loops instantly. */
  sleep?: (ms: number) => Promise<void>
}

export type PublishLinkedResult = {
  /** The collection as the publish answered it; undefined when nothing was published. */
  collection?: Collection
  items: Item[]
  itemCurations: ItemCuration[]
  pushed: PushLinkedResult | null
  txHash: string | null
}

const TOS_RETRIES = 3
const TOS_RETRY_DELAY_MS = 500
// Legacy pace for the subgraph to index the registration or the bought slots.
const INDEXING_RETRIES = 20
const INDEXING_RETRY_DELAY_MS = 5000

const REPLACED_PURCHASE_READS = 4
const REPLACED_PURCHASE_READ_DELAY_MS = 5000

const defaultSleep = (ms: number) => new Promise<void>(resolve => setTimeout(resolve, ms))

async function retry<T>(
  times: number,
  delayMs: number,
  fn: () => Promise<T>,
  shouldRetry: (error: unknown) => boolean,
  sleep: (ms: number) => Promise<void>
): Promise<T> {
  let lastError: unknown
  for (let attempt = 0; attempt < times; attempt++) {
    try {
      return await fn()
    } catch (error) {
      lastError = error
      if (!shouldRetry(error) || attempt === times - 1) break
      await sleep(delayMs)
    }
  }
  throw lastError
}

const isTransientFailure = (error: unknown) => !isRefusedRequest(error)

/**
 * POST /collections/:id/publish, retried while the subgraph has not caught up with the slots just bought (409
 * "exceeds the available slots", or 401 while the registration is unknown to it). The server also creates the
 * collection's forum post inside that request: a Discourse outage makes it answer 500 after the item curations
 * were already written, so a retry then gets 409 "already has published items". That 409 only means this run's
 * earlier attempt went through, so it counts as success; a 409 on the first attempt is a real pending review.
 */
async function publishWithRetry(
  params: PublishLinkedParams,
  cheque: SlotsCheque,
  deps: PublishLinkedDeps,
  sleep: (ms: number) => Promise<void>
): Promise<PublishLinkedItemsResult | null> {
  const itemIds = params.itemsToPublish.map(item => item.id)
  let serverFailed = false
  for (let attempt = 0; ; attempt++) {
    try {
      return await deps.publishLinkedItems(params.collection.id, itemIds, cheque)
    } catch (error) {
      if (isPendingReviewError(error)) {
        if (serverFailed) return null
        // The request may have reached the server on an earlier attempt of this publish whose answer never came back
        // (a timeout): the items being pending means it did.
        const state = getPublishState(params.itemsToPublish, await deps.fetchCurations())
        if (state === 'already_published') return null
        throw error
      }
      // builder-server checks slots before pending curations: once an attempt failed after writing them (the forum
      // post), a retry hears "exceeds the available slots" for the very slots it used.
      if (serverFailed && isInsufficientSlotsError(error)) {
        const state = getPublishState(params.itemsToPublish, await deps.fetchCurations())
        if (state === 'already_published') return null
      }
      const failedNow = isServerFailure(error)
      serverFailed ||= failedNow
      const retriable = failedNow || isInsufficientSlotsError(error) || hasStatus(error, 401)
      if (!retriable || attempt >= INDEXING_RETRIES - 1) throw error
      await sleep(INDEXING_RETRY_DELAY_MS)
    }
  }
}

/**
 * Runs the whole sequence in the legacy order: ToS, third-party kind, MANA allowance, registration or slot
 * purchase, the virtual third party's removal once the chain has it, the publish, then the pushes. Every failure
 * is a LinkedPublishError carrying the step it happened at.
 */
export async function publishLinkedItems(
  params: PublishLinkedParams,
  deps: PublishLinkedDeps
): Promise<PublishLinkedResult> {
  const { address, chainId, collection, itemsToPublish, itemsToPush, cheque, email, isProgrammatic } = params
  const sleep = deps.sleep ?? defaultSleep
  let current: LinkedPublishStep = 'check'
  const step = (next: LinkedPublishStep) => {
    current = next
    deps.onStep?.(next)
  }
  // A dropped transaction was replaced: by a speed-up that may have bought the same slots, or by a cancel.
  const settle = async (txHash: string, sender?: TransactionSender): Promise<'mined' | 'dropped'> => {
    // A payment saved without its sender gets it once the node returns it, so a later watch can tell it was replaced.
    const outcome = await deps.settleTransaction(txHash, sender, seen => {
      if (payment?.txHash === txHash && !payment.sender) setPayment({ ...payment, sender: seen })
    })
    if (outcome === 'reverted') {
      throw new LinkedPublishError(current, 'reverted', `Transaction ${txHash} reverted`, undefined, txHash)
    }
    if (outcome === 'pending') {
      throw new LinkedPublishError(
        current,
        'payment_pending',
        `Transaction ${txHash} is still pending`,
        undefined,
        txHash
      )
    }
    return outcome
  }

  // Kept outside the try so any later failure carries it: a retry must resume from this payment, not repeat it.
  let payment: LinkedPayment | undefined = params.paidTx
  // Until an earlier payment has settled, its record is what stops a later publish from paying again.
  let paidTxSettled = !params.paidTx
  const setPayment = (next: LinkedPayment | undefined) => {
    payment = next
    deps.onPayment?.(next)
  }

  try {
    if (itemsToPublish.length > 0 && !cheque) {
      throw new LinkedPublishError('publish', 'generic', 'A signed cheque is required to publish items')
    }

    // Read before anything is paid: an earlier attempt may have published these items already.
    let alreadyPublished = false
    if (itemsToPublish.length > 0) {
      step('check')
      const state = getPublishState(itemsToPublish, await deps.fetchCurations())
      if (state === 'pending_review') throw new LinkedPublishError('check', 'pending_review')
      alreadyPublished = state === 'already_published'
      if (alreadyPublished && payment && paidTxSettled) setPayment(undefined)
    }

    step('tos')
    const hashes = [...itemsToPublish, ...itemsToPush]
      .map(item => item.currentContentHash)
      .filter((hash): hash is string => !!hash)
    await retry(
      TOS_RETRIES,
      TOS_RETRY_DELAY_MS,
      () => deps.saveTOS(collection, email, hashes),
      isTransientFailure,
      sleep
    )

    let thirdParty = params.thirdParty
    let txHash: string | null = params.paidTx?.txHash ?? null
    let published: PublishLinkedItemsResult | null = null

    if (itemsToPublish.length > 0 && cheque && !alreadyPublished) {
      let slots = params.slots
      const slotsToBuyFor = (read: LinkedSlots) =>
        getSlotsToBuy({
          itemsToPublish: itemsToPublish.length,
          availableSlots: read.available,
          isProgrammatic: thirdParty.isProgrammatic,
          isPublished: read.registered || thirdParty.published,
          programmaticMinSlots: params.price.programmaticMinSlots
        })
      if (params.paidTx) {
        step('wait_tx')
        let stillPending = false
        try {
          await settle(params.paidTx.txHash, params.paidTx.sender)
          paidTxSettled = true
        } catch (error) {
          if (error instanceof LinkedPublishError && error.reason === 'payment_pending') {
            stillPending = true
          } else {
            // It charged nothing: whatever is still missing is bought below, as on a first attempt.
            if (!(error instanceof LinkedPublishError && error.reason === 'reverted')) throw error
            paidTxSettled = true
            setPayment(undefined)
            txHash = null
          }
        }
        slots = await deps.fetchSlots()
        // A payment that never confirms still goes on once the registry holds the slots (bought under another
        // hash); otherwise its record stays, so nothing is bought twice while it may still land.
        if (stillPending && slotsToBuyFor(slots) > 0) {
          throw new LinkedPublishError(
            'wait_tx',
            'payment_pending',
            `Transaction ${params.paidTx.txHash} is still pending`,
            undefined,
            params.paidTx.txHash
          )
        }
        // The slots are there: nothing is left to resume from it, and later publishes must not wait on it again.
        if (stillPending) {
          paidTxSettled = true
          setPayment(undefined)
        }
      }
      let registered = slots.registered || thirdParty.published

      // The kind is only chosen before the first publish; the registry fixes it on registration.
      if (!registered && isProgrammatic !== thirdParty.isProgrammatic) {
        step('kind')
        await deps.setThirdPartyKind(thirdParty.id, isProgrammatic)
        thirdParty = { ...thirdParty, isProgrammatic }
      }

      const slotsToBuy = slotsToBuyFor(slots)
      if (slotsToBuy > 0) {
        const fee = getLinkedPublishFee(slotsToBuy, params.price, {
          isProgrammatic: thirdParty.isProgrammatic,
          isPublished: registered
        })
        if (params.maxFeeManaWei !== undefined && fee.maxManaWei > params.maxFeeManaWei) {
          throw new LinkedPublishError('buy_slots', 'fee_changed')
        }
        const registry = getThirdPartyRegistry(chainId)
        if (fee.maxManaWei > 0n) {
          step('allowance')
          const allowance = await deps.fetchManaAllowance(address, registry.address)
          if (allowance < fee.maxManaWei) {
            const approveTx = await deps.sendTransaction(
              buildManaApproveCall(chainId, registry.address, fee.maxManaWei)
            )
            if (
              (await settle(approveTx)) === 'dropped' &&
              (await deps.fetchManaAllowance(address, registry.address)) < fee.maxManaWei
            ) {
              throw new LinkedPublishError(
                current,
                'reverted',
                `Transaction ${approveTx} was replaced`,
                undefined,
                approveTx
              )
            }
          }
        }
        if (!registered) {
          // Each payment is recorded at once (a reload must not lose it), then again with its sender once the node has it.
          step('register')
          txHash = await deps.sendTransaction(buildAddThirdPartyCall(chainId, thirdParty, slotsToBuy, fee.maxManaWei))
          setPayment({ txHash, registration: true })
          const sender = await deps.readSender?.(txHash)
          if (sender) setPayment({ txHash, registration: true, sender })
        } else {
          step('buy_slots')
          txHash = await deps.sendTransaction(buildBuyItemSlotsCall(chainId, thirdParty.id, slotsToBuy, fee.maxManaWei))
          setPayment({ txHash, registration: false })
          const sender = await deps.readSender?.(txHash)
          if (sender) setPayment({ txHash, registration: false, sender })
        }
        step('wait_tx')
        if ((await settle(txHash, payment?.sender)) === 'dropped') {
          // A speed-up bought the slots under another hash; the registry may take a moment to show them.
          let covered = false
          for (let read = 0; read < REPLACED_PURCHASE_READS && !covered; read++) {
            if (read > 0) await sleep(REPLACED_PURCHASE_READ_DELAY_MS)
            covered = slotsToBuyFor(await deps.fetchSlots()) === 0
          }
          if (!covered) {
            throw new LinkedPublishError(current, 'reverted', `Transaction ${txHash} was replaced`, undefined, txHash)
          }
        }
        registered = true
      }

      // A resumed registration still owes the delete, even once the refreshed third party reads as published.
      if ((registered && !params.thirdParty.published) || params.paidTx?.registration) {
        step('delete_virtual')
        try {
          await retry(
            INDEXING_RETRIES,
            INDEXING_RETRY_DELAY_MS,
            () => deps.deleteVirtualThirdParty(thirdParty.id),
            error => hasStatus(error, 400),
            sleep
          )
        } catch (error) {
          // Already gone (e.g. an earlier attempt of this publish removed it).
          if (!hasStatus(error, 404)) throw error
        }
      }

      step('publish')
      published = await publishWithRetry(params, cheque, deps, sleep)
    }

    // The publish consumed the payment: nothing a later failure could resume from. An earlier payment this run never
    // waited for (its items turned out published already) stays until a later run sees it settle.
    if (payment && paidTxSettled) setPayment(undefined)

    let pushed: PushLinkedResult | null = null
    if (itemsToPush.length > 0) {
      step('push')
      // Read again: a retry must not push what an earlier attempt already sent, nor refresh it as if still approved.
      const curations = await deps.fetchCurations()
      const toPush = itemsToPush.filter(item => canPushLinkedItem(item, curations.get(item.id) ?? null))
      pushed = toPush.length > 0 ? await deps.pushChanges(toPush, curations) : { pushed: [], failed: [] }
      if (pushed.failed.length > 0) {
        throw new LinkedPublishError(
          'push',
          'push_failed',
          pushed.failed.map(failure => `${failure.item.id}: ${failure.message}`).join('; ')
        )
      }
    }

    return {
      collection: published?.collection,
      items: published?.items ?? [],
      itemCurations: published?.itemCurations ?? [],
      pushed,
      txHash
    }
  } catch (error) {
    const failure = toLinkedPublishError(error, current, payment)
    // A reverted payment charged nothing, and a pending review means the items went through or can't for now: either
    // way there is nothing left to resume from.
    if (failure.reason === 'reverted' || (failure.reason === 'pending_review' && paidTxSettled)) {
      deps.onPayment?.(undefined)
    }
    throw failure
  }
}

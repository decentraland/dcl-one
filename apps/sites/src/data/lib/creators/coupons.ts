// Collection discounts in the Decentraland Shop: a creator signs a discount coupon over their collection and the
// marketplace applies it to the collection's listings at checkout, so the listings keep their price and the
// discount ends by itself when its window closes. marketplace-server stores and validates the coupon
// (`/v1/coupons`); this module builds, signs, stores, lists and ends one. Ported from the shop's lib/coupons.
import { Network, type TradeChecks } from '@dcl/schemas'
import {
  ContractName,
  getContract,
  getCouponManager,
  getMarketplaceName,
  type ContractData,
  type MarketplaceName
} from '~/lib/chain/contracts'
import { StandardMerkleTree } from '@openzeppelin/merkle-tree'
import { encodeAbiParameters, numberToHex, parseAbiParameters, recoverTypedDataAddress, toHex, type Hex } from 'viem'
import { config } from '~/config'
import { signedFetch, type ContractCall, type TypedDataDomain, type TypedDataTypes } from '~/lib/auth'
import { OFFCHAIN_MARKETPLACE_TYPES, getOffchainMarketplaceContract, toTypedChecks } from '~/lib/trades'
import { isWalletRejection, isWalletStateError } from '~/lib/walletErrors'

/** Percentage discount, in parts per million on-chain: 30% is 300_000. The only type this app signs. */
export const DISCOUNT_TYPE_RATE = 1
export const MIN_DISCOUNT_PCT = 5
export const MAX_DISCOUNT_PCT = 70
export const MAX_DISCOUNT_DURATION_MS = 30 * 24 * 60 * 60 * 1000
/** The `uses` a discount is signed with when it has no unit limit: the contract needs a number nobody reaches. */
export const UNCAPPED_USES = 1_000_000

const COUPON_AUTH_METADATA = { signer: 'dcl:builder', intent: 'dcl:create-coupon' }

export type DiscountStatus = 'scheduled' | 'active' | 'ended' | 'cancelled' | 'exhausted' | 'revoked'

/** A discount as marketplace-server reports it (`GET /v1/coupons`). `checks` timestamps are milliseconds. */
export type CollectionDiscount = {
  id: string
  signer: string
  chainId: number
  network: string
  checks: TradeChecks
  couponManager: string
  couponAddress: string
  discountType: number
  discount: number
  root: string
  collections: string[]
  signature: string
  createdAt: number
  state: { uses: number; cancelled: boolean; revoked: boolean; checkedAt: number } | null
  status: DiscountStatus
  /** Set by this app while its cancellation is sent but not yet settled. */
  ending?: boolean
}

/** What `POST /v1/coupons` takes: the signed fields plus the collections the root was built from. */
export type CouponCreation = {
  signer: string
  chainId: number
  network: Network
  checks: TradeChecks
  couponAddress: string
  discountType: number
  discount: number
  collections: string[]
  signature: string
}

type CouponContracts = { couponManager: ContractData & { name: string; version: string }; coupon: string }

function managerOf(marketplace: MarketplaceName, chainId: number): CouponContracts['couponManager'] {
  const manager = getCouponManager(marketplace, chainId)
  return { ...manager, chainId }
}

/**
 * The coupon deployment a NEW discount is signed against, or null where coupons do not exist. Each marketplace
 * version trusts its own manager, so it is the manager of the marketplace this app lists on, never the chain's
 * `ContractName.CouponManager`, which on Polygon mainnet still names the previous version's.
 */
export function getCouponContracts(chainId: number): CouponContracts | null {
  try {
    const marketplace = getMarketplaceName(getOffchainMarketplaceContract(chainId).address)
    if (!marketplace) return null
    return {
      couponManager: managerOf(marketplace, chainId),
      coupon: getContract(ContractName.CollectionDiscountCoupon, chainId).address
    }
  } catch {
    return null
  }
}

/**
 * The manager whose coupons a listing on `marketplaceAddress` redeems, lower-cased, or null when that marketplace
 * pairs with none. A discount reaches a listing only when this is the manager it was signed against.
 */
export function getCouponManagerForMarketplace(marketplaceAddress: string, chainId: number): string | null {
  const marketplace = getMarketplaceName(marketplaceAddress)
  if (!marketplace) return null
  try {
    return getCouponManager(marketplace, chainId).address.toLowerCase()
  } catch {
    return null
  }
}

const MARKETPLACES_WITH_COUPONS = [ContractName.OffChainMarketplaceV2, ContractName.OffChainMarketplaceV3] as const

/** The registry's manager at `address` on `chainId`, so a cancellation is only ever sent to a contract it vouches for. */
function findCouponManager(chainId: number, address: string): CouponContracts['couponManager'] | null {
  const wanted = address.toLowerCase()
  for (const marketplace of MARKETPLACES_WITH_COUPONS) {
    try {
      const manager = managerOf(marketplace, chainId)
      if (manager.address.toLowerCase() === wanted) return manager
    } catch {
      continue
    }
  }
  return null
}

/** Lower-cased and de-duplicated, so the same set of collections always hashes to the same root. */
export function normalizeCollections(collections: string[]): string[] {
  return [...new Set(collections.map(collection => collection.toLowerCase()))]
}

/**
 * The Merkle root the coupon is signed over, laid out with OpenZeppelin's StandardMerkleTree: marketplace-server
 * rebuilds it the same way, and a different layout would make it reject the coupon.
 */
export function collectionsRoot(collections: string[]): string {
  const unique = normalizeCollections(collections)
  if (unique.length === 0) throw new DiscountInputError('collections')
  return StandardMerkleTree.of(
    unique.map(collection => [collection]),
    ['address']
  ).root
}

/** `abi.encode(CollectionDiscountCouponData)`: what the creator signs and the contract decodes. */
export function encodeCouponData(discount: number, root: string, discountType = DISCOUNT_TYPE_RATE): string {
  return encodeAbiParameters(parseAbiParameters('uint256, uint256, bytes32'), [
    BigInt(discountType),
    BigInt(discount),
    root as Hex
  ])
}

export const COUPON_TYPES: TypedDataTypes = {
  Coupon: [
    { name: 'checks', type: 'Checks' },
    { name: 'couponAddress', type: 'address' },
    { name: 'data', type: 'bytes' }
  ],
  Checks: OFFCHAIN_MARKETPLACE_TYPES.Checks,
  ExternalCheck: OFFCHAIN_MARKETPLACE_TYPES.ExternalCheck
}

export function couponDomain(chainId: number, contracts: CouponContracts): TypedDataDomain {
  return {
    name: contracts.couponManager.name,
    version: contracts.couponManager.version,
    salt: numberToHex(chainId, { size: 32 }),
    verifyingContract: contracts.couponManager.address
  }
}

/** The EIP-712 values for a coupon: `checks` in seconds, the way the contract reads them. */
export function couponTypedValues(checks: TradeChecks, couponAddress: string, data: string) {
  return { checks: toTypedChecks(checks), couponAddress, data }
}

export type DiscountInputProblem = 'collections' | 'pct' | 'window' | 'duration' | 'uses'

/** Terms the contract, the server or the product would refuse. The UI maps the code to copy. */
export class DiscountInputError extends Error {
  constructor(public problem: DiscountInputProblem) {
    super(`Invalid discount: ${problem}`)
    this.name = 'DiscountInputError'
  }
}

export type DiscountTerms = {
  collections: string[]
  /** Whole percent, 5 to 70. */
  discountPct: number
  /** Epoch ms. Omitted = starts now. */
  startsAtMs?: number
  /** Epoch ms. */
  endsAtMs: number
  /** How many discounted purchases it allows. Omitted = no limit. */
  uses?: number
}

/** Rejects terms the contract, the server or the product would refuse, before anything is signed. */
export function validateDiscountTerms(terms: DiscountTerms, now = Date.now()): void {
  if (normalizeCollections(terms.collections).length === 0) throw new DiscountInputError('collections')
  if (
    !Number.isInteger(terms.discountPct) ||
    terms.discountPct < MIN_DISCOUNT_PCT ||
    terms.discountPct > MAX_DISCOUNT_PCT
  ) {
    throw new DiscountInputError('pct')
  }
  const startsAt = terms.startsAtMs ?? now
  if (!(terms.endsAtMs > now) || !(terms.endsAtMs > startsAt)) throw new DiscountInputError('window')
  if (terms.endsAtMs - Math.max(startsAt, now) > MAX_DISCOUNT_DURATION_MS) throw new DiscountInputError('duration')
  if (terms.uses !== undefined && (!Number.isInteger(terms.uses) || terms.uses < 1))
    throw new DiscountInputError('uses')
}

/** `pending`: the cancellation was sent but not seen mined in time; it may still go through, so never resend it. */
/** `wallet`: the wallet is on another account or chain, which the creator can fix themselves. */
/**
 * `unconfirmed`: a signed coupon was posted with no readable answer, so it may be stored; only that same coupon may
 * be retried. `refused`: the server answered no to it, so it is not stored and can be dropped.
 */
export type DiscountFailureReason = 'rejected' | 'pending' | 'wallet' | 'unconfirmed' | 'refused' | 'generic'

export class DiscountError extends Error {
  reason: DiscountFailureReason
  /** The underlying failure, when this one wraps it. */
  cause?: unknown

  constructor(reason: DiscountFailureReason, message: string = reason, cause?: unknown) {
    super(message)
    this.name = 'DiscountError'
    this.reason = reason
    this.cause = cause
  }
}

export function toDiscountError(error: unknown): DiscountError {
  if (error instanceof DiscountError) return error
  const message = error instanceof Error ? error.message : String(error)
  if (isWalletRejection(error)) return new DiscountError('rejected', message, error)
  if (isWalletStateError(error)) return new DiscountError('wallet', message, error)
  return new DiscountError('generic', message, error)
}

export type CreateDiscountDeps = {
  /** The manager's `contractSignatureIndex` and the signer's `signerSignatureIndex`. */
  readIndexes: (
    manager: ContractData,
    signer: string
  ) => Promise<{ contractSignatureIndex: number; signerSignatureIndex: number }>
  signTypedData: (
    domain: TypedDataDomain,
    types: TypedDataTypes,
    primaryType: string,
    value: Record<string, unknown>
  ) => Promise<string>
  postCoupon: (payload: CouponCreation) => Promise<CollectionDiscount>
  /** The stored discount with this signature, if the server has it: what settles an unanswered POST. */
  findStored: (payload: CouponCreation) => Promise<CollectionDiscount | undefined>
  /** The wallet prompt is over; the discount is being stored. */
  onSigned?: () => void
  /** The coupon is signed: until it is confirmed stored, a retry must resend this one, never sign another. */
  onSignedPayload?: (payload: CouponCreation) => void
  now?: () => number
  randomSalt?: () => string
}

/** Signs the discount (one EIP-712 signature, no transaction) and stores it, so the Shop starts applying it. */
export async function createDiscount(
  params: DiscountTerms & { signer: string; chainId: number },
  deps: CreateDiscountDeps
): Promise<CollectionDiscount> {
  const now = deps.now ? deps.now() : Date.now()
  validateDiscountTerms(params, now)
  const contracts = getCouponContracts(params.chainId)
  if (!contracts) throw new DiscountError('generic', `Coupons are not available on chain ${params.chainId}`)

  try {
    const signer = params.signer.toLowerCase()
    const collections = normalizeCollections(params.collections)
    const indexes = await deps.readIndexes(contracts.couponManager, signer)
    const checks: TradeChecks = {
      uses: params.uses ?? UNCAPPED_USES,
      expiration: params.endsAtMs,
      effective: params.startsAtMs ?? now,
      salt: deps.randomSalt ? deps.randomSalt() : toHex(crypto.getRandomValues(new Uint8Array(32))),
      contractSignatureIndex: indexes.contractSignatureIndex,
      signerSignatureIndex: indexes.signerSignatureIndex,
      allowedRoot: '0x',
      externalChecks: []
    }
    const discount = params.discountPct * 10_000
    const data = encodeCouponData(discount, collectionsRoot(collections))
    const signature = await deps.signTypedData(
      couponDomain(params.chainId, contracts),
      COUPON_TYPES,
      'Coupon',
      couponTypedValues(checks, contracts.coupon, data)
    )
    deps.onSigned?.()
    const payload: CouponCreation = {
      signer,
      chainId: params.chainId,
      network: Network.MATIC,
      checks,
      couponAddress: contracts.coupon,
      discountType: DISCOUNT_TYPE_RATE,
      discount,
      collections,
      signature
    }
    deps.onSignedPayload?.(payload)
    return await storeSignedDiscount(payload, deps)
  } catch (error) {
    throw toDiscountError(error)
  }
}

/**
 * Stores an already signed coupon, settling an earlier POST that got no answer: a coupon the server already has is
 * the answer (a repost of it is a 409), a refusal means it was never stored, and anything else stays unconfirmed so
 * the creator retries this same coupon rather than signing a second one.
 */
export async function storeSignedDiscount(
  payload: CouponCreation,
  deps: Pick<CreateDiscountDeps, 'postCoupon' | 'findStored'>
): Promise<CollectionDiscount> {
  try {
    return await deps.postCoupon(payload)
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error)
    // An earlier POST of this coupon may have been stored with its answer lost, so this request's refusal alone
    // proves nothing: only a lookup that answers, and does not find it, shows it never was.
    let stored: CollectionDiscount | undefined
    try {
      stored = await deps.findStored(payload)
    } catch {
      throw new DiscountError('unconfirmed', message)
    }
    if (stored) return stored
    if (error instanceof DiscountError && error.reason === 'refused') throw error
    throw new DiscountError('unconfirmed', message)
  }
}

/** Stores a signed discount. Signed request: the server checks the caller is the signer. */
export async function postCoupon(address: string, payload: CouponCreation): Promise<CollectionDiscount> {
  const response = await signedFetch(
    address,
    config.get('MARKETPLACE_SERVER_URL'),
    '/v1/coupons',
    { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify(payload) },
    COUPON_AUTH_METADATA
  )
  const json = (await response.json().catch(() => ({}))) as {
    ok?: boolean
    data?: CollectionDiscount
    message?: string
  }
  if (response.ok && json.ok && json.data) return json.data
  const message = json.message ?? `marketplace-server did not store the discount (${response.status})`
  // Only a validation refusal says no to this coupon. A 409 is it already stored; 401/403/408/429 are about the
  // request (an expired identity, a gateway, a rate limit); a 5xx or an unreadable 2xx says nothing.
  const refused =
    response.status >= 400 && response.status < 500 && ![401, 403, 408, 409, 429].includes(response.status)
  throw new DiscountError(refused ? 'refused' : 'unconfirmed', message)
}

const DISCOUNTS_PAGE = 100
// Bounds the walk for a creator with an implausible number of discounts.
const MAX_DISCOUNT_PAGES = 50

/**
 * A creator's discounts, newest first, with their last known state, read page by page up to
 * `MAX_DISCOUNT_PAGES`: the server orders them across every collection, so an older one still running can sit past
 * the first page. Public: coupons are public signatures.
 */
export async function fetchCreatorDiscounts(address: string): Promise<CollectionDiscount[]> {
  const discounts: CollectionDiscount[] = []
  for (let page = 0; page < MAX_DISCOUNT_PAGES; page++) {
    const query = new URLSearchParams({
      signer: address.toLowerCase(),
      limit: String(DISCOUNTS_PAGE),
      offset: String(page * DISCOUNTS_PAGE)
    })
    const response = await fetch(`${config.get('MARKETPLACE_SERVER_URL')}/v1/coupons?${query}`)
    if (!response.ok) {
      await response.body?.cancel()
      throw new Error(`marketplace-server request failed: /v1/coupons (${response.status})`)
    }
    const rows = ((await response.json()) as { data?: CollectionDiscount[] }).data ?? []
    discounts.push(...rows)
    if (rows.length < DISCOUNTS_PAGE) break
  }
  // A discount created mid-walk shifts the offsets and can repeat a row across two pages.
  return [...new Map(discounts.map(discount => [discount.id, discount])).values()]
}

/** The account's stored discount with this payload's signature, if any. */
export async function findStoredDiscount(
  address: string,
  payload: CouponCreation
): Promise<CollectionDiscount | undefined> {
  const signature = payload.signature.toLowerCase()
  return (await fetchCreatorDiscounts(address)).find(d => d.signature.toLowerCase() === signature)
}

/** The server's status, refreshed against the clock: a discount it called active may have ended since. */
export function liveDiscountStatus(
  discount: Pick<CollectionDiscount, 'status' | 'checks'>,
  now = Date.now()
): DiscountStatus {
  if (discount.status === 'cancelled' || discount.status === 'revoked' || discount.status === 'exhausted') {
    return discount.status
  }
  if (discount.checks.expiration <= now) return 'ended'
  if (discount.checks.effective > now) return 'scheduled'
  return 'active'
}

/** The collection's discounts that still matter (running or yet to start), soonest-ending first. */
export function openDiscountsFor(
  discounts: CollectionDiscount[],
  contractAddress: string,
  now = Date.now()
): CollectionDiscount[] {
  const collection = contractAddress.toLowerCase()
  return (
    discounts
      // Only percentage discounts, the kind this app signs and can describe.
      .filter(discount => discount.discountType === DISCOUNT_TYPE_RATE)
      .filter(discount => discount.collections.some(c => c.toLowerCase() === collection))
      .filter(discount => {
        const status = liveDiscountStatus(discount, now)
        return status === 'active' || status === 'scheduled'
      })
      .sort((a, b) => a.checks.expiration - b.checks.expiration)
  )
}

/** Whether the creator limited how many discounted purchases it allows. */
export function isDiscountCapped(discount: Pick<CollectionDiscount, 'checks'>): boolean {
  return discount.checks.uses < UNCAPPED_USES
}

/** Whole percent off, as the creator typed it. */
export function discountPct(discount: Pick<CollectionDiscount, 'discount'>): number {
  return Math.round(discount.discount / 10_000)
}

/**
 * `CouponManager.cancelSignature([coupon])` on the manager the discount was SIGNED against: cancelling on another
 * manager succeeds and changes nothing where the coupon redeems. The CouponManager takes no meta-transactions, so
 * this has to be sent from the wallet on the coupon's chain.
 */
export async function buildEndDiscountCall(discount: CollectionDiscount): Promise<ContractCall> {
  const manager = findCouponManager(discount.chainId, discount.couponManager)
  if (!manager) {
    throw new DiscountError('generic', `Unknown coupon manager ${discount.couponManager} on chain ${discount.chainId}`)
  }
  // The manager records `keccak256(caller, digest)` without checking the signature, so a coupon rebuilt with any
  // field off mines fine and cancels nothing. Rebuilt from the row's own fields, and sent only if it is provably
  // the coupon its signer signed.
  const checks = toTypedChecks(discount.checks)
  const data = encodeCouponData(discount.discount, discount.root, discount.discountType)
  const domain = couponDomain(discount.chainId, { couponManager: manager, coupon: discount.couponAddress })
  let recovered: string
  try {
    recovered = await recoverTypedDataAddress({
      domain,
      types: COUPON_TYPES,
      primaryType: 'Coupon',
      message: { checks, couponAddress: discount.couponAddress, data },
      signature: discount.signature as Hex
    } as Parameters<typeof recoverTypedDataAddress>[0])
  } catch {
    recovered = ''
  }
  if (recovered.toLowerCase() !== discount.signer.toLowerCase()) {
    throw new DiscountError('generic', `Discount ${discount.id} does not rebuild to the coupon its signer signed`)
  }
  return {
    contract: manager,
    method: 'cancelSignature',
    args: [
      [
        {
          signature: discount.signature,
          checks: { ...checks, allowedProof: [] },
          couponAddress: discount.couponAddress,
          data,
          callerData: '0x'
        }
      ]
    ]
  }
}

export type EndDiscountDeps = {
  /** `onSent` gets the hash the moment the wallet returns it, before the send call itself resolves. */
  sendTransaction: (call: ContractCall, onSent: (txHash: string) => void) => Promise<{ hash: string; nonce?: number }>
  waitForTransaction: (txHash: string) => Promise<boolean>
  /** The wallet prompt is over; the cancellation is mining. */
  onSigned?: () => void
  /** The cancellation is on its way under this hash: record it before waiting, so no reload can offer a second. */
  onBroadcast?: (txHash: string, nonce?: number) => void
}

/** Ends a discount early and waits for the receipt; the Shop stops applying it on the server's next state read. */
export async function endDiscount(discount: CollectionDiscount, deps: EndDiscountDeps): Promise<void> {
  // Once the cancellation is broadcast, only a confirmed revert is a failure: a timeout, an RPC error while
  // polling or a wallet that throws after broadcasting all leave it possibly mining, and a retry would pay twice.
  const call = await buildEndDiscountCall(discount)
  let broadcast: string | undefined
  let txHash: string
  let nonce: number | undefined
  try {
    ;({ hash: txHash, nonce } = await deps.sendTransaction(call, hash => {
      broadcast = hash
      deps.onBroadcast?.(hash)
      deps.onSigned?.()
    }))
  } catch (error) {
    if (broadcast) throw new DiscountError('pending', `The end discount transaction ${broadcast} may still mine`)
    throw toDiscountError(error)
  }
  // Recorded again with the nonce, which lets a later check tell a replaced cancellation from a pending one.
  deps.onBroadcast?.(txHash, nonce)
  if (!broadcast) deps.onSigned?.()
  let mined: boolean
  try {
    mined = await deps.waitForTransaction(txHash)
  } catch {
    throw new DiscountError('pending', `The end discount transaction ${txHash} was not seen mined`)
  }
  if (!mined) throw new DiscountError('generic', 'The end discount transaction reverted')
}

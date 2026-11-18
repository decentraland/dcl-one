import { useEffect, useMemo, useState } from 'react'
import { useIsMutating, useMutation, useQuery, useQueryClient, type QueryClient } from '@tanstack/react-query'
import {
  readTransactionOutcome,
  requireAccount,
  sendWalletTransaction,
  signTypedData,
  waitForTransaction,
  type Session
} from '~/lib/auth'
import { errorCode, track } from '~/lib/analytics'
import {
  DiscountError,
  createDiscount,
  endDiscount,
  fetchCreatorDiscounts,
  liveDiscountStatus,
  findStoredDiscount,
  openDiscountsFor,
  postCoupon,
  storeSignedDiscount,
  toDiscountError,
  type CollectionDiscount,
  type CouponCreation,
  type DiscountTerms
} from '~/lib/coupons'
import { type Collection } from '~/lib/collections'
import {
  clearPendingEnd,
  clearUnconfirmedCreate,
  pendingEndOf,
  pendingEnds,
  resetDiscountAttempts,
  savePendingEnd,
  saveUnconfirmedCreate,
  updatePendingEnd,
  type PendingEnd
} from '~/lib/discountAttempts'
import { FeatureFlag } from '~/lib/featureFlags'
import { getMaticChainId } from '~/lib/publishCollection'
import { readSignatureIndexes } from '~/lib/trades'
import { useFeatureFlag } from '~/hooks/useFeatureFlag'
import { captureError } from '~/lib/monitoring'

export const DISCOUNT_OPENED_EVENT = 'Open create collection discount'
const DISCOUNT_CREATED_EVENT = 'Create collection discount'
const DISCOUNT_FAILED_EVENT = 'Create collection discount error'
const DISCOUNT_ENDED_EVENT = 'End collection discount'
const DISCOUNT_END_FAILED_EVENT = 'End collection discount error'

// The last discount this tab created per collection, so a dialog reopened during a create can tell it went through.
const createdFor = new Map<string, string>()

/** Whether any discount is being created or ended in this tab, dialog open or not, for the leave-page guard. */
export function useAnyDiscountInFlight(): boolean {
  return useIsMutating({ mutationKey: CREATE_KEY }) + useIsMutating({ mutationKey: END_KEY }) > 0
}

/** Forgets this tab's outcomes; for specs, which share the module between cases. */
export function resetDiscountSession(): void {
  createdFor.clear()
  serverReads.clear()
  resetDiscountAttempts()
}

/**
 * Whether a discount's cancellation is recorded: `ended` once seen mined, `pending` until the chain settles it.
 * Recorded at broadcast and kept across reloads, so End is never offered while either holds.
 */
export function endOutcome(address: string, discountId: string): 'ended' | 'pending' | undefined {
  const end = pendingEndOf(address, discountId)
  if (!end) return undefined
  return end.minedAt ? 'ended' : 'pending'
}

// A cancellation usually settles within minutes: poll often while it is fresh, then back off, since one whose
// discount never shows up again in a list keeps its record until it expires.
const FRESH_END_MS = 10 * 60_000

/** How often to re-check recorded cancellations, or false when there are none. */
export function pollEvery(ends: PendingEnd[], now = Date.now()): number | false {
  if (ends.length === 0) return false
  return ends.some(end => now - end.sentAt < FRESH_END_MS) ? 30_000 : 5 * 60_000
}

const isRunning = (discount: CollectionDiscount, now: number) =>
  ['active', 'scheduled'].includes(liveDiscountStatus(discount, now))

/**
 * Settles each recorded cancellation against the chain and the server's view of its discount. A record is only
 * dropped on evidence that no second cancellation is needed or that the first can no longer execute:
 * - the server reports that discount ended (cancelled, exhausted, revoked or past its end). A discount missing from
 *   a list proves nothing: the list may predate it or not reach it;
 * - the receipt says it reverted;
 * - its nonce went to another transaction AND the server read the coupon since and still reports it running.
 * A receipt that says it mined keeps the record (as ended) until the server catches up. No receipt and an unused
 * nonce, or a failed read, keep it pending, however old.
 */
export async function reconcilePendingEnds(
  address: string,
  rows: CollectionDiscount[] | undefined,
  now = Date.now()
): Promise<{ ending: string[]; ended: string[] }> {
  for (const end of pendingEnds(address, now)) {
    const row = rows?.find(discount => discount.id === end.discountId)
    if (row && !isRunning(row, now)) {
      clearPendingEnd(address, end.discountId)
      continue
    }
    if (end.minedAt) continue
    if (end.supersededAt) {
      if (row?.state && row.state.checkedAt > end.supersededAt) clearPendingEnd(address, end.discountId)
      continue
    }
    const outcome = await readTransactionOutcome(end.chainId, end.txHash, address, end.nonce)
    if (outcome === 'mined') updatePendingEnd(address, end.discountId, { minedAt: now })
    else if (outcome === 'reverted') clearPendingEnd(address, end.discountId)
    else if (outcome === 'superseded') updatePendingEnd(address, end.discountId, { supersededAt: now })
  }
  const remaining = pendingEnds(address, now)
  return {
    ending: remaining.filter(end => !end.minedAt).map(end => end.discountId),
    ended: remaining.filter(end => end.minedAt).map(end => end.discountId)
  }
}

/** The id of the last discount this tab created for the collection. */
export function lastCreatedDiscount(collectionId: string): string | undefined {
  return createdFor.get(collectionId)
}

// A sped-up or replaced transaction gets a new hash this one never sees mined, so the wait gives up.
const END_TIMEOUT_MS = 5 * 60_000

const CREATE_KEY = ['create-collection-discount']
const END_KEY = ['end-collection-discount']

// Expected states the creator resolves themselves are tracked as events, not reported as errors.
const isReportable = (error: unknown) =>
  !['rejected', 'wallet', 'pending', 'unconfirmed'].includes(toDiscountError(error).reason)

/**
 * Whether a discount is still being created for the collection, or ended, in this tab. Read from the mutation
 * cache, so it survives closing and reopening the dialog while a wallet prompt is still open.
 */
export function useDiscountInFlight({ collectionId, discountId }: { collectionId?: string; discountId?: string }) {
  const creating = useIsMutating({
    mutationKey: CREATE_KEY,
    predicate: mutation =>
      !!collectionId &&
      (mutation.state.variables as CreateDiscountVariables | undefined)?.collection.id === collectionId
  })
  const ending = useIsMutating({
    mutationKey: END_KEY,
    predicate: mutation =>
      !!discountId && (mutation.state.variables as EndDiscountVariables | undefined)?.discount.id === discountId
  })
  return { creating: creating > 0, ending: ending > 0 }
}

const discountsKey = (address: string | undefined) => ['collection-discounts', address?.toLowerCase()]

// What the server itself last answered, per account, apart from the query cache this app patches for display:
// only a real server read may settle a recorded cancellation.
const serverReads = new Map<string, { rows: CollectionDiscount[]; at: number }>()

async function readFromServer(address: string): Promise<CollectionDiscount[]> {
  const rows = await fetchCreatorDiscounts(address)
  serverReads.set(address.toLowerCase(), { rows, at: Date.now() })
  return rows
}

export type DiscountFlags = {
  /** The Shop applies creator discounts for buyers, so running ones matter and can be ended here. */
  shopApplies: boolean
  /** Creators may also create discounts here. */
  enabled: boolean
  /** The Shop discounts MANA listings too. `undefined` while that is still being read. */
  manaListings: boolean | undefined
}

/** Both switches a discount needs (this app's and the Shop's), plus whether it reaches MANA listings. */
export function useDiscountFlags(): DiscountFlags {
  const shop = useFeatureFlag(FeatureFlag.SHOP_CREATOR_SALES)
  const own = useFeatureFlag(FeatureFlag.COLLECTION_DISCOUNTS, { enabled: shop.enabled })
  const enabled = own.enabled && shop.enabled
  const mana = useFeatureFlag(FeatureFlag.SHOP_LEGACY_COUPONS, { enabled })
  return { shopApplies: shop.enabled, enabled, manaListings: mana.isLoading ? undefined : mana.enabled }
}

/** The collection's running and scheduled discounts, signed by `address`. Off without either. */
export function useCollectionDiscounts(address: string | undefined, contractAddress: string | undefined) {
  const [now, setNow] = useState(() => Date.now())
  const query = useQuery({
    queryKey: discountsKey(address),
    queryFn: () => readFromServer(address!),
    enabled: !!address && !!contractAddress,
    staleTime: 30_000,
    // While a cancellation is recorded, re-read the server until it reports the discount ended.
    refetchInterval: () => (address ? pollEvery(pendingEnds(address)) : false)
  })
  // Re-evaluated every minute, so a discount that ran out loses its End button without a refetch.
  useEffect(() => {
    const timer = setInterval(() => setNow(Date.now()), 60_000)
    return () => clearInterval(timer)
  }, [])
  // Polled while any cancellation is unsettled, including one sent before a reload; re-run on every server read.
  const reconcile = useQuery({
    queryKey: ['collection-discount-ends', address?.toLowerCase(), query.dataUpdatedAt],
    queryFn: () => reconcilePendingEnds(address!, serverReads.get(address!.toLowerCase())?.rows),
    enabled: !!address && !!contractAddress && query.isSuccess,
    refetchInterval: () => (address ? pollEvery(pendingEnds(address).filter(end => !end.minedAt)) : false)
  })
  const discounts = useMemo(() => {
    if (!query.data || !contractAddress || !address) return undefined
    const recorded = new Map(pendingEnds(address).map(end => [end.discountId, end]))
    return (
      openDiscountsFor(query.data, contractAddress, now)
        // Seen mined, even if the server still reports it running.
        .filter(discount => !recorded.get(discount.id)?.minedAt && !reconcile.data?.ended.includes(discount.id))
        // Still listed while its cancellation is unsettled: it keeps applying until that mines.
        .map(discount => (recorded.has(discount.id) ? { ...discount, ending: true } : discount))
    )
  }, [query.data, contractAddress, address, now, reconcile.data])
  return { ...query, discounts, now }
}

function updateDiscounts(
  queryClient: QueryClient,
  address: string,
  update: (discounts: CollectionDiscount[]) => CollectionDiscount[]
) {
  const key = discountsKey(address)
  // A list that never loaded is refetched rather than seeded: seeding it would hide every other discount.
  if (queryClient.getQueryData<CollectionDiscount[]>(key)) {
    queryClient.setQueryData<CollectionDiscount[]>(key, current => update(current!))
  } else {
    void queryClient.invalidateQueries({ queryKey: key })
  }
}

export type CreateDiscountVariables = Omit<DiscountTerms, 'collections'> & {
  collection: Collection
  /** A coupon already signed whose storing is unconfirmed: it is stored again (or found stored), never re-signed. */
  resume?: CouponCreation
  /** Listings the discount re-prices, for the event. */
  discountedCount: number
  /** The wallet prompt is over; the discount is being stored. */
  onSigned?: () => void
}

function discountProps(variables: CreateDiscountVariables) {
  return {
    collectionId: variables.collection.id,
    discountPct: variables.discountPct,
    durationDays: Math.round((variables.endsAtMs - (variables.startsAtMs ?? Date.now())) / 86_400_000),
    capped: variables.uses !== undefined,
    discountedCount: variables.discountedCount
  }
}

/** Signs a discount over the collection and stores it; the Shop applies it as soon as it starts. */
export function useCreateDiscount(session: Session | null) {
  const queryClient = useQueryClient()
  const chainId = getMaticChainId()
  return useMutation({
    mutationKey: CREATE_KEY,
    mutationFn: async ({
      collection,
      discountPct,
      startsAtMs,
      endsAtMs,
      uses,
      onSigned,
      resume
    }: CreateDiscountVariables) => {
      if (!session) throw new Error('Wallet disconnected')
      const address = session.address
      const store = {
        postCoupon: (payload: CouponCreation) => postCoupon(address, payload),
        findStored: (payload: CouponCreation) => findStoredDiscount(address, payload)
      }
      let signed = resume
      try {
        let discount: CollectionDiscount
        if (resume) {
          // Only a coupon this account signed, for this collection and chain, is ever posted again.
          const mine =
            resume.signer.toLowerCase() === address.toLowerCase() &&
            resume.chainId === chainId &&
            resume.collections.some(c => c.toLowerCase() === collection.contractAddress?.toLowerCase())
          if (!mine) throw new DiscountError('refused', 'The stored attempt is not a coupon for this collection')
          discount = await storeSignedDiscount(resume, store)
        } else {
          discount = await createDiscount(
            {
              signer: address,
              chainId,
              collections: [collection.contractAddress!],
              discountPct,
              startsAtMs,
              endsAtMs,
              uses
            },
            {
              readIndexes: readSignatureIndexes,
              // The coupon is posted as `session.address`: signed by another account, the server would refuse it.
              signTypedData: async (domain, types, primaryType, value) => {
                await requireAccount(session)
                return signTypedData(session, domain, types, primaryType, value)
              },
              ...store,
              onSigned,
              onSignedPayload: payload => {
                signed = payload
                saveUnconfirmedCreate(address, { collectionId: collection.id, payload })
              }
            }
          )
        }
        if (signed) clearUnconfirmedCreate(address, signed.signature)
        return discount
      } catch (error) {
        // Known never stored: the creator may sign a new one. Unconfirmed stays recorded for a retry of this one.
        if (signed && toDiscountError(error).reason === 'refused') clearUnconfirmedCreate(address, signed.signature)
        throw error
      }
    },
    onSuccess: (discount, variables) => {
      track(DISCOUNT_CREATED_EVENT, { ...discountProps(variables), discountId: discount.id })
      createdFor.set(variables.collection.id, discount.id)
      if (session) {
        updateDiscounts(queryClient, session.address, current => [
          discount,
          ...current.filter(entry => entry.id !== discount.id)
        ])
      }
    },
    // Here rather than in the dialog: a prompt the creator backed out of, or closed the dialog on, still fails.
    onError: (error, variables) => {
      track(DISCOUNT_FAILED_EVENT, { ...discountProps(variables), error: errorCode(error) })
      if (isReportable(error)) {
        captureError(error, { flow: 'create_collection_discount', collectionId: variables.collection.id })
      }
    }
  })
}

export type EndDiscountVariables = {
  discount: CollectionDiscount
  collection: Collection
  /** The wallet prompt is over; the cancellation is mining. */
  onSigned?: () => void
}

/** Ends a discount from the wallet (the coupon manager takes no meta-transactions) and waits until it is mined. */
export function useEndDiscount(session: Session | null) {
  const queryClient = useQueryClient()
  return useMutation({
    mutationKey: END_KEY,
    mutationFn: ({ discount, onSigned }: EndDiscountVariables) => {
      if (!session) throw new Error('Wallet disconnected')
      // Cancellations are keyed by the caller: only the signer's own cancellation ends the coupon.
      if (session.address.toLowerCase() !== discount.signer.toLowerCase()) {
        throw new DiscountError(
          'generic',
          `Discount ${discount.id} was signed by ${discount.signer}, not ${session.address}`
        )
      }
      return endDiscount(discount, {
        sendTransaction: (call, onSent) => sendWalletTransaction(session, call, onSent),
        waitForTransaction: hash => waitForTransaction(discount.chainId, hash, END_TIMEOUT_MS),
        onSigned,
        onBroadcast: (txHash, nonce) =>
          savePendingEnd(session.address, {
            discountId: discount.id,
            txHash,
            nonce,
            chainId: discount.chainId,
            sentAt: Date.now(),
            expiresAt: discount.checks.expiration
          })
      })
    },
    onSuccess: (_, { discount, collection }) => {
      track(DISCOUNT_ENDED_EVENT, { collectionId: collection.id, discountId: discount.id })
      if (session) {
        // Kept, as ended, until the server stops reporting it running: a reload must not offer End again.
        const recorded = pendingEndOf(session.address, discount.id)
        if (recorded) updatePendingEnd(session.address, discount.id, { minedAt: Date.now() })
        else {
          savePendingEnd(session.address, {
            discountId: discount.id,
            txHash: '',
            chainId: discount.chainId,
            sentAt: Date.now(),
            minedAt: Date.now(),
            expiresAt: discount.checks.expiration
          })
        }
        // A re-render only: the row is hidden by its mined record, and the server's own row stays as it answered.
        updateDiscounts(queryClient, session.address, current => [...current])
      }
    },
    onError: (error, { discount, collection }) => {
      // Pending stays recorded (it was saved at broadcast) until reconciled against the chain; a confirmed revert
      // ended nothing, so End comes back.
      if (session) {
        if (toDiscountError(error).reason !== 'pending') clearPendingEnd(session.address, discount.id)
        updateDiscounts(queryClient, session.address, current => [...current])
        void queryClient.invalidateQueries({ queryKey: ['collection-discount-ends', session.address.toLowerCase()] })
      }
      if (isReportable(error)) {
        captureError(error, { flow: 'end_collection_discount', collectionId: collection.id, discountId: discount.id })
      }
      track(DISCOUNT_END_FAILED_EVENT, {
        collectionId: collection.id,
        discountId: discount.id,
        error: errorCode(error)
      })
    }
  })
}

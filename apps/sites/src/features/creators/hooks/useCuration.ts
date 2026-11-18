import { useMemo } from 'react'
import { useTranslation } from '~/intl'
import { keepPreviousData, useMutation, useQuery, useQueryClient, type QueryClient } from '@tanstack/react-query'
import { errorCode, track } from '~/lib/analytics'
import { sendContractTransaction, waitForTransaction, type Session } from '~/lib/auth'
import {
  isCollectionBusy,
  isValidationRunning,
  ValidationLimitError,
  fetchCollection,
  fetchCollectionCuration,
  fetchCurationCollections,
  fetchCurations,
  pushCollectionCuration,
  updateCollectionCuration
} from '~/lib/builder'
import { type Collection } from '~/lib/collections'
import { buildSetApprovedCall } from '~/lib/collectionApproval'
import { APPROVAL_INDEX_TIMEOUT_MS, waitForIndexer } from '~/lib/approveCollection'
import { orderCurators, type CollectionCuration, type CurationFilters } from '~/lib/curation'
import { shortenAddress } from '~/lib/address'
import { type RejectReasonCode } from '~/lib/events'
import { captureError } from '~/lib/monitoring'
import { postAssignee } from '~/hooks/useForumPost'
import { useCommittee } from '~/hooks/useCommittee'
import { useProfiles } from '~/hooks/useProfile'
import { getMaticChainId } from '~/lib/publishCollection'
import { isExpectedWalletError } from '~/lib/walletErrors'

/** The committee as picker options, the signed-in curator first and marked, after `leading` and a divider. */
type CuratorOption = { value: string; label: string; dividerBefore?: boolean }

export function useCuratorOptions(address: string | undefined, leading: CuratorOption) {
  const { t } = useTranslation()
  const { members } = useCommittee(address)
  const curators = useMemo(() => orderCurators(members, address), [members, address])
  const profiles = useProfiles(curators)
  const self = address?.toLowerCase()
  return useMemo<CuratorOption[]>(
    () => [
      leading,
      ...curators.map((curator, index) => {
        const name = profiles[index]?.name || shortenAddress(curator)
        return {
          value: curator,
          label: curator === self ? t('curation_page.filter.you', { name }) : name,
          dividerBefore: index === 0
        }
      })
    ],
    [leading, curators, profiles, self, t]
  )
}

export function collectionCurationKey(address: string | undefined, collectionId: string | undefined) {
  return ['collection-curation', address, collectionId] as const
}

export function collectionEventsKey(address: string | undefined, collectionId: string | undefined) {
  return ['collection-events', address, collectionId] as const
}

/** The collection's latest review request, `null` when it was never requested. */
export function useCollectionCuration(address: string | undefined, collection: Collection | undefined) {
  return useQuery({
    queryKey: collectionCurationKey(address, collection?.id),
    queryFn: () => fetchCollectionCuration(address!, collection!.id),
    enabled: !!address && !!collection?.isPublished,
    staleTime: 30_000,
    retry: 1
  })
}

/** Every collection's latest review request, keyed by collection id; the list endpoint doesn't carry them. */
export function useCurationsByCollection(address: string | undefined, enabled: boolean) {
  const query = useQuery({
    queryKey: ['curations', address],
    queryFn: () => fetchCurations(address!),
    enabled: !!address && enabled,
    staleTime: 30_000
  })
  const byCollection = useMemo(
    () => new Map((query.data ?? []).map(curation => [curation.collectionId, curation])),
    [query.data]
  )
  return { ...query, byCollection }
}

export function useCurationCollections(address: string | undefined, filters: CurationFilters, enabled: boolean) {
  return useQuery({
    queryKey: ['curation-collections', address, filters],
    queryFn: () => fetchCurationCollections(address!, filters),
    enabled: !!address && enabled,
    placeholderData: keepPreviousData,
    staleTime: 30_000
  })
}

/** Rereads a collection's review: its timeline, its request and the request list. */
export function refreshReview(queryClient: QueryClient, address: string | undefined, collectionId: string) {
  void queryClient.invalidateQueries({ queryKey: collectionEventsKey(address, collectionId) })
  void queryClient.invalidateQueries({ queryKey: collectionCurationKey(address, collectionId) })
  void queryClient.invalidateQueries({ queryKey: ['curations', address] })
}

/** Shows a collection's new latest curation everywhere it is read, without refetching. */
export function storeCuration(queryClient: QueryClient, address: string | undefined, curation: CollectionCuration) {
  queryClient.setQueryData(collectionCurationKey(address, curation.collectionId), curation)
  queryClient.setQueryData<CollectionCuration[]>(['curations', address], current =>
    current ? [...current.filter(existing => existing.collectionId !== curation.collectionId), curation] : current
  )
  void queryClient.invalidateQueries({ queryKey: ['curation-collections'] })
  void queryClient.invalidateQueries({ queryKey: collectionEventsKey(address, curation.collectionId) })
}

function useStoreCuration(address: string | undefined) {
  const queryClient = useQueryClient()
  return (curation: CollectionCuration) => storeCuration(queryClient, address, curation)
}

export type AssignVariables = {
  collection: Collection
  curation: CollectionCuration | null
  assignee: string | null
}

/** Assigns, reassigns or unassigns the curator of a collection; a never-requested collection gets its first request. */
export function useAssignCurator(address: string | undefined) {
  const queryClient = useQueryClient()
  const store = useStoreCuration(address)
  return useMutation({
    mutationFn: ({ collection, curation, assignee }: AssignVariables) => {
      if (!address) throw new Error('Wallet disconnected')
      return curation
        ? updateCollectionCuration(address, collection.id, { assignee })
        : pushCollectionCuration(address, collection.id, assignee)
    },
    onSuccess: (curation, { collection, assignee }) => {
      track(assignee ? 'Assign curator' : 'Unassign curator', { collectionId: collection.id, assignee })
      store(curation)
      if (address) void postAssignee(queryClient, address, collection, assignee)
    },
    onError: (error, { collection }) => {
      track('Assign curator error', { collectionId: collection.id, error: errorCode(error) })
      captureError(error, { flow: 'curation_assign', collectionId: collection.id })
    }
  })
}

export type RejectDecision = { rejectionReasons: RejectReasonCode[]; rejectionMessage?: string }

export type RejectVariables = {
  collection: Collection
  curation: CollectionCuration | null
  /** The reasons and message the creator reads; only sent where builder-server's auto-curation accepts them. */
  decision?: RejectDecision
}

/**
 * Rejects the collection's review request. A collection nobody requested a review for has no request to
 * reject yet, so one is opened first: otherwise the rejection would leave no trace for the creator.
 */
export function useRejectCuration(address: string | undefined) {
  const queryClient = useQueryClient()
  const store = useStoreCuration(address)
  return useMutation({
    mutationFn: async ({ collection, curation, decision }: RejectVariables) => {
      if (!address) throw new Error('Wallet disconnected')
      let current = curation
      if (current?.status !== 'pending') {
        current = await pushCollectionCuration(address, collection.id)
        // Kept even if the PATCH below fails, so a retry patches this request instead of opening another.
        store(current)
      }
      return updateCollectionCuration(address, current.collectionId, { status: 'rejected', ...decision })
    },
    onSuccess: (curation, { collection, curation: previous, decision }) => {
      track('Reject curation', {
        collectionId: collection.id,
        first_review: !previous,
        reasons: decision?.rejectionReasons ?? []
      })
      store(curation)
    },
    onError: (error, { collection }) => {
      // The request may have been opened before the rejection failed; a retry must PATCH it, not POST again.
      void queryClient.invalidateQueries({ queryKey: collectionCurationKey(address, collection.id) })
      void queryClient.invalidateQueries({ queryKey: ['curations', address] })
      track('Reject curation error', { collectionId: collection.id, error: errorCode(error) })
      captureError(error, { flow: 'curation_reject', collectionId: collection.id })
    }
  })
}

/** The creator's "Publish changes": asks the committee to review the collection's changes again. */
export type PushVariables = {
  collection: Collection
  /** What changed, for the curator; only a server with the review timeline keeps it. */
  note?: string
}

export function usePushCuration(address: string | undefined) {
  const queryClient = useQueryClient()
  const store = useStoreCuration(address)
  return useMutation({
    mutationFn: ({ collection, note }: PushVariables) => {
      if (!address) throw new Error('Wallet disconnected')
      return note
        ? pushCollectionCuration(address, collection.id, undefined, note)
        : pushCollectionCuration(address, collection.id)
    },
    onSuccess: (curation, { collection, note }) => {
      track('Push curation', { collectionId: collection.id, first_review: !collection.isApproved, has_note: !!note })
      store(curation)
    },
    onError: (error, { collection }) => {
      track('Push curation error', { collectionId: collection.id, error: errorCode(error) })
      // A running review, the daily limit and a busy collection are answers the creator acts on, not failures.
      const expected = error instanceof ValidationLimitError || isValidationRunning(error) || isCollectionBusy(error)
      if (!expected) {
        captureError(error, { flow: 'curation_push', collectionId: collection.id })
        return
      }
      // The answer says the review moved on (a run started, the day's runs spent) or will (busy): reread it.
      refreshReview(queryClient, address, collection.id)
    }
  })
}

/** Disables an approved collection on chain (not mintable anymore) and waits until it is mined. */
export type DisableVariables = { collection: Collection; onSigned?: () => void }

export function useDisableCollection(session: Session | null) {
  const queryClient = useQueryClient()
  const chainId = getMaticChainId()
  return useMutation({
    mutationFn: async ({ collection, onSigned }: DisableVariables) => {
      if (!session) throw new Error('Wallet disconnected')
      const txHash = await sendContractTransaction(session, buildSetApprovedCall(chainId, collection, false))
      onSigned?.()
      if (!(await waitForTransaction(chainId, txHash))) throw new Error(`Disable ${txHash} reverted`)
      // Like the approval, done means builder-server's lagging subgraph reads it too, so a refetch can't bring back
      // `is_approved: true`; past the timeout the cache carries the on-chain result.
      const indexed = await waitForIndexer(
        () => fetchCollection(session.address, collection.id),
        fetched => !fetched.isApproved,
        APPROVAL_INDEX_TIMEOUT_MS
      ).catch(() => ({ ...collection, isApproved: false }))
      return { txHash, indexed }
    },
    onSuccess: ({ txHash, indexed }, { collection }) => {
      // The legacy builder named the on-chain disable "Reject collection".
      track('Reject collection', { collectionId: collection.id, txHash })
      if (!session) return
      queryClient.setQueryData<Collection>(['collection', session.address, collection.id], indexed)
      void queryClient.invalidateQueries({ queryKey: ['curation-collections'] })
    },
    onError: (error, { collection }) => {
      track('Reject collection error', { collectionId: collection.id, error: errorCode(error) })
      if (!isExpectedWalletError(error)) {
        captureError(error, { flow: 'curation_disable', collectionId: collection.id })
      }
    }
  })
}

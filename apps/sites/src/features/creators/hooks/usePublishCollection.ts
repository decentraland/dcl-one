import { useEffect, useMemo } from 'react'
import { useMutation, useQuery, useQueryClient, type QueryClient } from '@tanstack/react-query'
import { ContractName, getContract } from '~/lib/chain/contracts'
import { errorCode, track } from '~/lib/analytics'
import { captureError } from '~/lib/monitoring'
import {
  deleteItem,
  fetchAllCollectionItems,
  fetchContent,
  fetchItemContents,
  fetchRarities,
  lockCollection,
  publishCollectionItems,
  fetchCollectionCuration,
  pushCollectionCuration,
  saveCollection,
  saveCollectionTOS,
  saveItem
} from '~/lib/builder'
import { isContractDeployed, sendContractTransaction, waitForTransaction, type Session } from '~/lib/auth'
import { isCollectionLocked, isLinkedCollection, type Collection } from '~/lib/collections'
import { authorizePublication } from '~/lib/credits'
import { withRehashedContents } from '~/lib/itemFactory'
import { type Item } from '~/lib/items'
import { buildManaApproveCall } from '~/lib/mana'
import { useNotifications } from '~/lib/notifications'
import {
  PublishCollectionError,
  PublishTransactionRevertedError,
  consolidatePublishedCollection,
  getMaticChainId,
  isRecentPublication,
  openFirstReviewRequest,
  publishCollection,
  syncPublishedItems,
  type PaymentMethod,
  type PublishResult
} from '~/lib/publishCollection'
import { type PublicationFee } from '~/lib/publishFee'
import { buildCollectionInitializeData } from '~/lib/saveCollection'
import { useTranslation } from '~/intl'
import { postToForum } from '~/hooks/useForumPost'
import { storeCuration } from '~/hooks/useCuration'
import { isPublishSyncing, usePublishSyncStore } from '~/store/publishSync'

export function useRarities(address: string | undefined) {
  return useQuery({
    queryKey: ['rarities'],
    queryFn: () => fetchRarities(address),
    enabled: !!address,
    staleTime: 60_000
  })
}

/** Approves the CollectionManager to spend `amountWei` of MANA (never unlimited) and waits until the approval is mined. */
export function useApproveMana(session: Session | null) {
  const chainId = getMaticChainId()
  return useMutation({
    mutationFn: async (amountWei: bigint) => {
      if (!session) throw new Error('Wallet disconnected')
      const spender = getContract(ContractName.CollectionManager, chainId).address
      const txHash = await sendContractTransaction(session, buildManaApproveCall(chainId, spender, amountWei))
      const mined = await waitForTransaction(chainId, txHash)
      if (!mined) throw new Error('MANA approval reverted')
    }
  })
}

export function invalidateCollectionItems(queryClient: ReturnType<typeof useQueryClient>, collectionId: string) {
  void queryClient.invalidateQueries({
    queryKey: ['collection-items-all'],
    predicate: q => q.queryKey.includes(collectionId)
  })
  void queryClient.invalidateQueries({ queryKey: ['collection'], predicate: q => q.queryKey.includes(collectionId) })
  void queryClient.invalidateQueries({
    queryKey: ['collection-preview'],
    predicate: q => q.queryKey.includes(collectionId)
  })
  // Linked collections page their items and curations server-side under their own keys.
  void queryClient.invalidateQueries({ queryKey: ['linked-items'], predicate: q => q.queryKey.includes(collectionId) })
  void queryClient.invalidateQueries({
    queryKey: ['item-curations'],
    predicate: q => q.queryKey.includes(collectionId)
  })
}

export function useDeleteItem(address: string | undefined) {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: async (item: Item) => {
      if (!address) throw new Error('Wallet disconnected')
      await deleteItem(address, item.id)
      return item
    },
    onSuccess: item => {
      track('Delete item', { itemId: item.id })
      if (item.collectionId) invalidateCollectionItems(queryClient, item.collectionId)
    },
    onError: (error, item) => track('Delete item error', { itemId: item.id, error: errorCode(error) })
  })
}

export type PublishVariables = {
  collection: Collection
  items: Item[]
  paymentMethod: PaymentMethod
  fee: PublicationFee
  email: string
}

/**
 * Runs the publish sequence for the connected wallet. On success the collection is locked in the
 * cache and the chain→server consolidation continues in the background; the caller's UI is done.
 */
export function usePublishCollection(session: Session | null) {
  const queryClient = useQueryClient()
  const chainId = getMaticChainId()
  const { t } = useTranslation()

  return useMutation({
    mutationFn: async (variables: PublishVariables): Promise<PublishResult> => {
      if (!session) throw new Error('Wallet disconnected')
      const { address } = session
      return publishCollection(
        { ...variables, address, chainId },
        {
          saveCollection: (collection, items) =>
            saveCollection(address, collection, buildCollectionInitializeData(collection, items, address)),
          fetchItems: collectionId => fetchAllCollectionItems(address, collectionId),
          rehashItem: async item => {
            const built = await withRehashedContents(item, fetchContent)
            return saveItem(address, built.item, built.blobs)
          },
          saveTOS: (collection, email) => saveCollectionTOS(address, collection, email),
          authorizePublication: params => authorizePublication(address, params),
          sendTransaction: call => sendContractTransaction(session, call),
          lockCollection: collectionId => lockCollection(address, collectionId)
        }
      )
    },
    onSuccess: ({ collection, txHash }, { collection: submitted, paymentMethod, fee }) => {
      // Legacy prop names (txHash, collectionId, usedCredits, creditsAmount) so the event lines up with
      // the same one from the old builder; `isFiat` is always false here — this app has no fiat publish.
      track('Publish collection', {
        collectionId: collection.id,
        txHash,
        isFiat: false,
        usedCredits: paymentMethod === 'credits',
        creditsAmount: paymentMethod === 'credits' ? fee.total.credits : 0,
        item_count: fee.itemCount,
        isRetry: isCollectionLocked(submitted)
      })
      const address = session?.address
      queryClient.setQueryData(['collection', address, collection.id], collection)
      void queryClient.invalidateQueries({ queryKey: ['collections'] })
      void queryClient.invalidateQueries({ queryKey: ['credits-balance', address] })
      void queryClient.invalidateQueries({ queryKey: ['mana-balance', address] })
      // Detached on purpose: the modal closes right away and the server catches up on its own.
      usePublishSyncStore.getState().start(collection.id)
      const { contractAddress } = collection
      consolidatePublishedCollection(collection.id, txHash, {
        waitForTransaction: hash => waitForTransaction(chainId, hash),
        isCollectionDeployed: contractAddress ? () => isContractDeployed(chainId, contractAddress) : undefined,
        publishCollectionItems: collectionId => publishCollectionItems(address!, collectionId),
        // Not awaited: the forum's retries must not hold the sync's cleanup below.
        onSynced: () => {
          void postToForum(queryClient, address!, collection, 'publish')
          void requestFirstReview(queryClient, address!, collection)
        }
      })
        .catch((error: unknown) => {
          console.error('Collection consolidation failed', error)
          const reverted = error instanceof PublishTransactionRevertedError
          if (reverted) {
            track('Publish collection reverted', {
              collectionId: collection.id,
              txHash,
              usedCredits: paymentMethod === 'credits'
            })
          }
          // The modal is long gone: a lasting toast is the only way the creator learns the publish did not land.
          const key = reverted
            ? 'publish_collection_modal.consolidation.reverted'
            : 'publish_collection_modal.consolidation.sync_failed'
          useNotifications.getState().showToast(t(key, { name: collection.name }), {
            type: 'error',
            durationMs: CONSOLIDATION_TOAST_MS
          })
        })
        .finally(() => {
          usePublishSyncStore.getState().end(collection.id)
          invalidateCollectionItems(queryClient, collection.id)
        })
    },
    onError: (error, { collection, paymentMethod }) => {
      track('Publish collection error', {
        collectionId: collection.id,
        isFiat: false,
        usedCredits: paymentMethod === 'credits',
        isRetry: isCollectionLocked(collection),
        error: errorCode(error)
      })
      // Every publish needs the ToS record, so a failure to write it blocks creators until someone looks.
      if (error instanceof PublishCollectionError && error.reason === 'tos_failed') {
        captureError(error.cause ?? error, { flow: 'publish', step: 'tos', collectionId: collection.id })
      }
    }
  })
}

const CONSOLIDATION_TOAST_MS = 20_000

/** Opens the collection's first review request once it is published; never fails the publish. */
export async function requestFirstReview(
  queryClient: QueryClient,
  address: string,
  collection: Pick<Collection, 'id' | 'urn'>
): Promise<void> {
  if (isLinkedCollection(collection)) return
  try {
    const curation = await openFirstReviewRequest(collection.id, {
      fetchCollectionCuration: id => fetchCollectionCuration(address, id),
      pushCollectionCuration: id => pushCollectionCuration(address, id)
    })
    if (curation) storeCuration(queryClient, address, curation)
  } catch (error) {
    captureError(error, { flow: 'publish_review_request', collectionId: collection.id })
  }
}

/**
 * Recovery for a publish whose consolidation never finished (tab closed before the transaction was
 * mined): once the server reports the collection published while its items still lack token ids,
 * asks builder-server to sync them — what the legacy builder did on every collection fetch.
 */
export function useSyncPublishedItems(address: string | undefined, collection: Collection | undefined, items: Item[]) {
  const queryClient = useQueryClient()
  const collectionId = collection?.id
  const urn = collection?.urn
  const isPublished = !!collection?.isPublished
  const needsSync = useMemo(() => isPublished && items.some(item => !item.tokenId), [isPublished, items])
  const opensReview = useMemo(() => !!collection && isRecentPublication(collection), [collection])

  useEffect(() => {
    if (!address || !collectionId || !urn || !needsSync || isPublishSyncing(collectionId)) return
    usePublishSyncStore.getState().start(collectionId)
    syncPublishedItems(collectionId, { publishCollectionItems: id => publishCollectionItems(address, id) })
      .then(async () => {
        if (opensReview) await requestFirstReview(queryClient, address, { id: collectionId, urn })
      })
      .catch(error => console.error('Collection items sync failed', error))
      .finally(() => {
        usePublishSyncStore.getState().end(collectionId)
        invalidateCollectionItems(queryClient, collectionId)
      })
  }, [address, collectionId, urn, needsSync, opensReview, queryClient])
}

/**
 * Whether a locked draft's contract is already on chain: an earlier attempt landed but isn't indexed yet, so a retry
 * would only revert. Off for any other collection.
 */
export function useLockedCollectionDeployed(collection: Collection | undefined) {
  const chainId = getMaticChainId()
  const contractAddress = collection && isCollectionLocked(collection) ? collection.contractAddress : undefined
  return useQuery({
    queryKey: ['collection-deployed', chainId, contractAddress],
    queryFn: () => isContractDeployed(chainId, contractAddress!),
    enabled: !!contractAddress,
    staleTime: 30_000
  })
}

/** The files of a saved item as blobs, for the thumbnail editor; off until an item is given. */
export function useItemContents(item: Item | null) {
  return useQuery({
    queryKey: ['item-contents', item?.id, item?.updatedAt],
    queryFn: () => fetchItemContents(item!),
    enabled: item !== null,
    staleTime: Infinity
  })
}

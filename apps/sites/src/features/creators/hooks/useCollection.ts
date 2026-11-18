import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { errorCode, track } from '~/lib/analytics'
import { fetchAllCollectionItems, fetchCollection, saveCollection, deleteCollection } from '~/lib/builder'
import { buildCollectionInitializeData } from '~/lib/saveCollection'
import { useCollectionCuration } from '~/hooks/useCuration'
import {
  getCollectionDisplayStatus,
  isLinkedCollection,
  withoutCollection,
  type Collection,
  type CollectionDisplayStatus
} from '~/lib/collections'

export const ITEMS_PAGE_SIZE = 20

export function useCollection(address: string | undefined, collectionId: string | undefined) {
  return useQuery({
    queryKey: ['collection', address, collectionId],
    queryFn: () => fetchCollection(address!, collectionId!),
    enabled: !!address && !!collectionId,
    staleTime: 30_000
  })
}

/** The collection's status, undefined until everything it depends on has loaded so the pill never flips. */
export function useCollectionStatus(
  address: string | undefined,
  collection: Collection | undefined
): CollectionDisplayStatus | undefined {
  const curation = useCollectionCuration(
    address,
    collection && !isLinkedCollection(collection) ? collection : undefined
  )
  if (!collection) return undefined
  if (!collection.isPublished || isLinkedCollection(collection)) return getCollectionDisplayStatus(collection, null)
  // Without its curation the status could be wrong, so a failed request keeps the pill hidden.
  if (!curation.isSuccess) return undefined
  return getCollectionDisplayStatus(collection, curation.data?.status ?? null)
}

/** Every item of the collection: the detail page filters and pages them client-side. */
/** The cache key of the all-items query, so callers can read or patch it without restating it. */
export function allCollectionItemsKey(address: string | undefined, collectionId: string | undefined) {
  return ['collection-items-all', address, collectionId] as const
}

export function useAllCollectionItems(address: string | undefined, collectionId: string | undefined) {
  return useQuery({
    queryKey: allCollectionItemsKey(address, collectionId),
    queryFn: () => fetchAllCollectionItems(address!, collectionId!),
    enabled: !!address && !!collectionId,
    staleTime: 30_000
  })
}

/**
 * Create or rename a collection. Mirrors the legacy save-collection saga: for a collection that
 * already has items the initialize calldata is regenerated over all of them, so the server derives
 * the same contract address it would in the legacy builder.
 */
export function useSaveCollection(address: string | undefined) {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: async (collection: Collection) => {
      if (!address) throw new Error('Wallet disconnected')
      // A linked collection never goes on chain as a contract: the server takes it without initialize data.
      if (isLinkedCollection(collection)) return saveCollection(address, collection, '')
      const items = collection.itemCount > 0 ? await fetchAllCollectionItems(address, collection.id) : []
      const data = buildCollectionInitializeData(collection, items, address)
      return saveCollection(address, collection, data)
    },
    onSuccess: saved => {
      track('Save collection', { collectionId: saved.id, item_count: saved.itemCount })
      queryClient.setQueryData(['collection', address, saved.id], saved)
      void queryClient.invalidateQueries({ queryKey: ['collections'] })
    },
    onError: (error, collection) =>
      track('Save collection error', { collectionId: collection.id, error: errorCode(error) })
  })
}

export function useDeleteCollection(address: string | undefined) {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: async (collectionId: string) => {
      if (!address) throw new Error('Wallet disconnected')
      await deleteCollection(address, collectionId)
      return collectionId
    },
    onSuccess: async collectionId => {
      track('Delete collection', { collectionId })
      queryClient.removeQueries({ queryKey: ['collection', address, collectionId] })
      queryClient.removeQueries({ queryKey: ['collection-preview', address, collectionId] })
      // The lists keep their previous data on screen while they refetch: drop the deleted collection from them
      // first, or its row mounts again and its cover requests previews of a collection that is gone (a 404).
      // A list fetch that started before the delete would write the old page back, so cancel it first.
      await queryClient.cancelQueries({ queryKey: ['collections', address] })
      queryClient.setQueriesData({ queryKey: ['collections', address] }, (data: unknown) =>
        withoutCollection(data, collectionId)
      )
      void queryClient.invalidateQueries({ queryKey: ['collections'] })
    },
    onError: (error, collectionId) => track('Delete collection error', { collectionId, error: errorCode(error) })
  })
}

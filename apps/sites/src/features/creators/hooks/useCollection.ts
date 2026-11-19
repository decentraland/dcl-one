import { privateKey, privateDomain } from '@/data/lib/creators/queryKeys'
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { errorCode, track } from '@/data/lib/creators/analytics'
import { fetchAllCollectionItems, fetchCollection, fetchItem, saveCollection, deleteCollection } from '@/data/lib/creators/builder'
import { buildCollectionInitializeData } from '@/data/lib/creators/saveCollection'
import { useCollectionCuration } from './useCuration'
import {
  getCollectionDisplayStatus,
  isLinkedCollection,
  withoutCollection,
  type Collection,
  type CollectionDisplayStatus
} from '@/data/lib/creators/collections'

export const ITEMS_PAGE_SIZE = 20

export function useCollection(address: string | undefined, collectionId: string | undefined) {
  return useQuery({
    queryKey: privateKey(address, 'collection', collectionId),
    queryFn: () => fetchCollection(address!, collectionId!),
    enabled: !!address && !!collectionId,
    staleTime: 30_000
  })
}

/** The collection an item belongs to, for a link that names only the item; enabled only when `itemId` is set. */
export function useItemCollectionId(address: string | undefined, itemId: string | null) {
  return useQuery({
    queryKey: privateKey(address, 'item-collection', itemId),
    queryFn: async () => (await fetchItem(address!, itemId!)).collectionId ?? null,
    enabled: !!address && !!itemId,
    staleTime: Infinity
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
  return privateKey(address, 'collection-items-all', collectionId)
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
      track('bd_collection_saved', { collection_id: saved.id, item_count: saved.itemCount })
      queryClient.setQueryData(privateKey(address, 'collection', saved.id), saved)
      void queryClient.invalidateQueries(privateDomain('collections'))
    },
    onError: (error, collection) =>
      track('bd_collection_save_failed', { collection_id: collection.id, error: errorCode(error) })
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
      track('bd_collection_deleted', { collection_id: collectionId })
      queryClient.removeQueries({ queryKey: privateKey(address, 'collection', collectionId) })
      queryClient.removeQueries({ queryKey: privateKey(address, 'collection-preview', collectionId) })
      // The lists keep their previous data on screen while they refetch: drop the deleted collection from them
      // first, or its row mounts again and its cover requests previews of a collection that is gone (a 404).
      // A list fetch that started before the delete would write the old page back, so cancel it first.
      await queryClient.cancelQueries({ queryKey: privateKey(address, 'collections') })
      queryClient.setQueriesData({ queryKey: privateKey(address, 'collections') }, (data: unknown) =>
        withoutCollection(data, collectionId)
      )
      void queryClient.invalidateQueries(privateDomain('collections'))
    },
    onError: (error, collectionId) =>
      track('bd_collection_delete_failed', { collection_id: collectionId, error: errorCode(error) })
  })
}
